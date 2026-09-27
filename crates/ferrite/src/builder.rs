//! Production builder.

use crate::loader::*;
use crate::report::*;
use ferrite_bundler::BuildBundleConfig;
use ferrite_bundler::BundleRequest;
use ferrite_bundler::Bundler as _;
use ferrite_bundler::FerriteBundler;
use ferrite_config::ResolvedConfig;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_plugin::Apply;
use ferrite_plugin::Plugin;
use ferrite_plugin::PluginContainer;
use ferrite_server::DevServer;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// Production builder (spec §9, §78).
pub struct Builder {
    /// Resolved config.
    pub config: ResolvedConfig,
    /// Plugins.
    pub plugins: Vec<Arc<dyn Plugin>>,
}

impl Builder {
    /// Create a builder.
    #[must_use]
    pub fn new(config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Self {
        Self { config, plugins }
    }

    /// Build every configured environment (client + SSR when present).
    pub async fn build_app(&self) -> Result<Vec<BuildReport>> {
        let mut reports = vec![self.build("client").await?];
        if self.ssr_entry().is_some() {
            reports.push(self.build("ssr").await?);
        } else {
            tracing::info!("no SSR entry found; skipping ssr build");
        }
        Ok(reports)
    }

    /// Build one environment (`client` / `ssr`).
    pub async fn build(&self, env: &str) -> Result<BuildReport> {
        let config = self.config.clone();
        let server = DevServer::new_without_watcher(config.clone(), self.plugins.clone()).await?;
        let environment = if env == "ssr" {
            config.ssr_env()
        } else {
            config.client_env()
        };
        // Lifecycle: buildStart (§78).
        let ctx = ferrite_plugin::PluginContext {
            graph: &server.inner().graph,
            resolver: &server.inner().client_resolver,
            environment: &environment,
            emitted: &server.inner().emitted,
            watch_files: &server.inner().watch_files,
            warnings: &server.inner().warnings,
        };
        let container = PluginContainer::new(self.plugins.clone(), Apply::Build);
        container.hook_build_start(&ctx).await?;
        // Entries.
        let entries = self.discover_entries(&server, env).await?;
        if entries.is_empty() {
            return Err(FerriteError::Build(format!(
                "no entries found for env `{env}`"
            )));
        }
        // Bundle.
        let base = with_trailing_slash(&config.base);
        let loader = Arc::new(BuildLoader {
            server: server.clone(),
            minify: config.build.minify,
            sourcemap: config.build.sourcemap.enabled(),
            base,
            assets: Mutex::new(HashMap::new()),
        });
        let bundler = FerriteBundler::new(loader.clone());
        let out_dir = if env == "ssr" {
            config.out_dir().join("server")
        } else {
            config.out_dir()
        };
        let bundle_config = BuildBundleConfig {
            out_dir: out_dir.clone(),
            ..BuildBundleConfig::from_resolved(&config)
        };
        let mut output = bundler
            .bundle(
                &server.inner().graph,
                &bundle_config,
                BundleRequest {
                    entries: entries.clone(),
                    env: env.to_string(),
                    minify: false, // loader minifies per-module
                    sourcemap: config.build.sourcemap.enabled(),
                    map_comment: !config.build.sourcemap.hidden(),
                    // Statement shake rides the production minify flag.
                    treeshake: config.build.minify,
                    engine: config.compiler.engine.clone(),
                    scope_hoist: config.build.scope_hoist,
                },
            )
            .await?;
        // Merge loader-emitted assets.
        for (name, bytes) in loader.take_assets() {
            output.bundle.insert(ferrite_plugin::EmittedFile {
                name,
                contents: bytes,
                is_entry: false,
            });
        }
        // generateBundle → write → writeBundle → closeBundle (§78).
        container
            .hook_generate_bundle(&ctx, &mut output.bundle)
            .await?;
        std::fs::create_dir_all(&out_dir)?;
        for file in output.bundle.files.values() {
            let target = out_dir.join(&file.name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, &file.contents)?;
        }
        container.hook_write_bundle(&ctx, &output.bundle).await?;
        container.hook_close_bundle().await?;
        container
            .hook_build_end(&ctx, ferrite_plugin::BuildEnd { error: None })
            .await?;
        // Manifests (§40).
        if env == "ssr" {
            output
                .ssr_manifest
                .write(&out_dir.join("ssr-manifest.json"))?;
        } else {
            output.manifest.write(&out_dir.join("manifest.json"))?;
        }
        // HTML entries + public/.
        if env == "client" {
            self.write_html(&config, &output.manifest)?;
            copy_public(&config)?;
        }
        // Standalone single binary (§51–§54; blocking cargo builds run
        // off the async runtime).
        let mut standalone_binary = None;
        if config.package.standalone && env == "client" {
            let opts = crate::package::StandaloneOptions {
                embed_assets: config.package.embed_assets,
                compress_assets: config.package.compress_assets,
                target: config.package.target.clone(),
                cargo: None,
            };
            let out = out_dir.clone();
            let report =
                tokio::task::spawn_blocking(move || crate::package::write_standalone(&out, &opts))
                    .await
                    .map_err(|error| {
                        ferrite_core::FerriteError::Build(format!(
                            "standalone packaging failed: {error}"
                        ))
                    })??;
            tracing::info!(
                "standalone: {} files ({} bytes, {} embedded) → {}",
                report.files,
                report.bytes,
                report.embedded_bytes,
                report.dir.display()
            );
            standalone_binary = report.binary;
        }
        let chunks = output.bundle.files.len();
        Ok(BuildReport {
            env: env.to_string(),
            out_dir,
            entries: entries.into_iter().map(|id| id.0).collect(),
            chunks,
            bytes: output.stats.bytes,
            modules: output.stats.modules_emitted,
            dropped: output.stats.modules_dropped,
            standalone_binary,
        })
    }

    /// Discover entry module ids for `env` (§77).
    async fn discover_entries(&self, server: &DevServer, env: &str) -> Result<Vec<ModuleId>> {
        if env == "ssr" {
            if let Some(entry) = self.ssr_entry() {
                let spec = format!("/{}", entry.trim_start_matches('/'));
                return Ok(vec![server.resolve_entry(&spec, env).await?]);
            }
            return Ok(Vec::new());
        }
        // Library mode (§55).
        if let Some(lib) = &self.config.build.lib {
            let spec = format!("/{}", lib.entry.trim_start_matches('/'));
            return Ok(vec![server.resolve_entry(&spec, env).await?]);
        }
        // HTML entries (§77).
        let mut entries = Vec::new();
        for html_entry in &self.config.build.entries {
            let file = self.config.root.join(html_entry);
            if !file.exists() {
                tracing::warn!("entry `{html_entry}` not found; skipping");
                continue;
            }
            let html = std::fs::read_to_string(&file)?;
            for discovered in ferrite_html::discover_entries(&html) {
                match server.resolve_entry(&discovered.src, env).await {
                    Ok(id) => entries.push(id),
                    Err(error) => {
                        tracing::warn!("cannot resolve entry `{}`: {error}", discovered.src)
                    }
                }
            }
        }
        Ok(entries)
    }

    /// JS SSR entry candidate, when present.
    fn ssr_entry(&self) -> Option<String> {
        for candidate in [
            "src/entry-server.ts",
            "src/entry-server.tsx",
            "src/entry-server.js",
            "src/server.ts",
        ] {
            if self.config.root.join(candidate).exists() {
                return Some(candidate.to_string());
            }
        }
        None
    }

    /// Rewrite HTML entries to hashed outputs and copy them to `out_dir`.
    fn write_html(
        &self,
        config: &ResolvedConfig,
        manifest: &ferrite_manifest::BuildManifest,
    ) -> Result<()> {
        let base = with_trailing_slash(&config.base);
        for html_entry in &config.build.entries {
            let file = config.root.join(html_entry);
            if !file.exists() {
                continue;
            }
            let html = std::fs::read_to_string(&file)?;
            // Stylesheets for this page's entries (manifest css[], in order).
            let mut css_hrefs: Vec<String> = Vec::new();
            for discovered in ferrite_html::discover_entries(&html) {
                if !discovered.is_module {
                    continue;
                }
                if let Some(entry) = manifest_entry(manifest, &discovered.src) {
                    for css in &entry.css {
                        let href = format!("{base}{css}");
                        if !css_hrefs.contains(&href) {
                            css_hrefs.push(href);
                        }
                    }
                }
            }
            let rewritten = ferrite_html::rewrite_module_scripts(&html, |src| {
                if let Some(entry) = manifest_entry(manifest, src) {
                    return format!("{base}{}", entry.file);
                }
                src.to_string()
            });
            // Production: no dev client; ensure no /@ferrite/ refs remain.
            let rewritten = rewritten.replace(
                "<script type=\"module\" src=\"/@ferrite/client\"></script>",
                "",
            );
            let rewritten = inject_stylesheets(&rewritten, &css_hrefs);
            let target = config.out_dir().join(html_entry);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(target, rewritten)?;
        }
        Ok(())
    }
}

/// Look up a manifest entry by script src (tolerates leading slash).
pub(crate) fn manifest_entry<'a>(
    manifest: &'a ferrite_manifest::BuildManifest,
    src: &str,
) -> Option<&'a ferrite_manifest::ManifestEntry> {
    manifest.entries.get(src).or_else(|| {
        let alt = src.strip_prefix('/').unwrap_or(src);
        manifest.entries.get(&format!("/{alt}"))
    })
}

/// Inject stylesheet links before `</head>` (document start fallback).
pub(crate) fn inject_stylesheets(html: &str, hrefs: &[String]) -> String {
    if hrefs.is_empty() {
        return html.to_string();
    }
    let links = hrefs
        .iter()
        .map(|href| format!("<link rel=\"stylesheet\" href=\"{href}\">"))
        .collect::<Vec<_>>()
        .join("\n");
    match html.find("</head>") {
        Some(pos) => format!("{}{links}\n{}", &html[..pos], &html[pos..]),
        None => format!("{links}\n{html}"),
    }
}
