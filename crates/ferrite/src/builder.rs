//! Production builder.

use crate::loader::*;
use crate::report::*;
use ferrite_bundler::BuildBundleConfig;
use ferrite_bundler::BundleHooks;
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
    pub fn new(mut config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Self {
        config.is_production = true;
        Self { config, plugins }
    }

    /// Build every configured environment (client + SSR when present).
    pub async fn build_app(&self) -> Result<Vec<BuildReport>> {
        let has_server_entry = self.ssr_entry()?.is_some();
        let mut reports = vec![self.build("client").await?];
        if has_server_entry {
            reports.push(self.build("ssr").await?);
        } else {
            tracing::info!("no SSR entry found; skipping ssr build");
        }
        Ok(reports)
    }

    /// Build one environment (`client` / `ssr`).
    ///
    /// Lifecycle: `options` → `buildStart` → bundle (`renderStart` /
    /// `renderChunk` / `augmentChunkHash` / wrappers) → `generateBundle` →
    /// write → `writeBundle` → `closeBundle` → `buildEnd`. On failure,
    /// `buildEnd` runs with the error, then `closeBundle`, and the original
    /// error is returned.
    pub async fn build(&self, env: &str) -> Result<BuildReport> {
        if !matches!(env, "client" | "ssr") {
            return Err(FerriteError::Build(format!(
                "unsupported build environment `{env}`; select `client` or `ssr`. Use mode to select environment files"
            )));
        }
        let entries = self.default_entries(env)?;
        let mut config = self.config.clone();
        config.is_production = true;
        let server = DevServer::new_without_watcher(config.clone(), self.plugins.clone()).await?;
        let environment = if env == "ssr" {
            config.ssr_env()
        } else {
            config.client_env()
        };
        let ctx = ferrite_plugin::PluginContext {
            graph: &server.inner().graph,
            resolver: if environment.kind.is_ssr() {
                &server.inner().ssr_resolver
            } else {
                &server.inner().client_resolver
            },
            environment: &environment,
            emitted: &server.inner().emitted,
            watch_files: &server.inner().watch_files,
            warnings: &server.inner().warnings,
        };
        let container = PluginContainer::new(self.plugins.clone(), Apply::Build);
        // Input/output options (Rollup `options` / `outputOptions`).
        let mut options = ferrite_plugin::BundleOptions {
            entries,
            treeshake: config.build.minify,
            minify: config.build.minify,
            sourcemap: config.build.sourcemap.enabled(),
            scope_hoist: config.build.scope_hoist,
        };
        container.hook_options(&ctx, &mut options).await?;
        let defaults = BuildBundleConfig::from_resolved(&config);
        let mut output_options = ferrite_plugin::OutputOptions {
            chunk_pattern: defaults.chunk_pattern.clone(),
            css_pattern: defaults.css_pattern.clone(),
            asset_pattern: defaults.asset_pattern.clone(),
        };
        container
            .hook_output_options(&ctx, &mut output_options)
            .await?;
        // Lifecycle: buildStart (§78).
        container.hook_build_start(&ctx).await?;
        match self
            .build_inner(
                &server,
                &container,
                &ctx,
                &config,
                env,
                &options,
                &output_options,
            )
            .await
        {
            Ok(report) => {
                container.hook_close_bundle().await?;
                container
                    .hook_build_end(&ctx, ferrite_plugin::BuildEnd { error: None })
                    .await?;
                Ok(report)
            }
            Err(error) => {
                let message = error.to_string();
                if let Err(hook_error) = container
                    .hook_build_end(
                        &ctx,
                        ferrite_plugin::BuildEnd {
                            error: Some(message),
                        },
                    )
                    .await
                {
                    tracing::warn!("build_end hook failed on the error path: {hook_error}");
                }
                if let Err(hook_error) = container.hook_close_bundle().await {
                    tracing::warn!("close_bundle hook failed on the error path: {hook_error}");
                }
                Err(error)
            }
        }
    }

    /// Default `options.entries` seed for `env` (HTML entries for client,
    /// SSR entry candidate for SSR).
    fn default_entries(&self, env: &str) -> Result<Vec<String>> {
        if env == "ssr" {
            Ok(self.ssr_entry()?.into_iter().collect())
        } else {
            Ok(self.config.build.entries.clone())
        }
    }

    /// Inner build: entries → bundle → generate/write → manifests → HTML.
    #[allow(clippy::too_many_arguments)]
    async fn build_inner(
        &self,
        server: &DevServer,
        container: &PluginContainer,
        ctx: &ferrite_plugin::PluginContext<'_>,
        config: &ResolvedConfig,
        env: &str,
        options: &ferrite_plugin::BundleOptions,
        output_options: &ferrite_plugin::OutputOptions,
    ) -> Result<BuildReport> {
        // Entries (honoring hooked `options.entries`).
        let entries = self.discover_entries(server, env, &options.entries).await?;
        if entries.is_empty() {
            return Err(FerriteError::Build(format!(
                "no entries found for env `{env}`"
            )));
        }
        // Bundle.
        let base = with_trailing_slash(&config.base);
        let loader = Arc::new(BuildLoader {
            server: server.clone(),
            minify: options.minify,
            sourcemap: options.sourcemap,
            base,
            assets: Mutex::new(HashMap::new()),
        });
        let bundler = FerriteBundler::new(loader.clone());
        let out_dir = if env == "ssr" {
            config.out_dir().join("server")
        } else {
            config.out_dir()
        };
        let mut bundle_config = BuildBundleConfig {
            out_dir: out_dir.clone(),
            ..BuildBundleConfig::from_resolved(config)
        };
        if !output_options.chunk_pattern.is_empty() {
            bundle_config.chunk_pattern = output_options.chunk_pattern.clone();
        }
        if !output_options.css_pattern.is_empty() {
            bundle_config.css_pattern = output_options.css_pattern.clone();
        }
        if !output_options.asset_pattern.is_empty() {
            bundle_config.asset_pattern = output_options.asset_pattern.clone();
        }
        let render_hooks = ContainerRenderHooks { container, ctx };
        let mut output = bundler
            .bundle(
                &server.inner().graph,
                &bundle_config,
                BundleRequest {
                    entries: entries.clone(),
                    env: env.to_string(),
                    minify: false, // loader minifies per-module
                    sourcemap: options.sourcemap,
                    map_comment: !config.build.sourcemap.hidden(),
                    // Statement shake rides the production minify flag.
                    treeshake: options.treeshake,
                    engine: config.compiler.engine.clone(),
                    scope_hoist: options.scope_hoist,
                },
                &render_hooks,
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
        // generateBundle → write → writeBundle (§78; closeBundle/buildEnd
        // run in the `build` wrapper so failures also notify hooks).
        container
            .hook_generate_bundle(ctx, &mut output.bundle)
            .await?;
        std::fs::create_dir_all(&out_dir)?;
        for file in output.bundle.files.values() {
            let target = out_dir.join(&file.name);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, &file.contents)?;
        }
        container.hook_write_bundle(ctx, &output.bundle).await?;
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
            self.write_html(config, &output.manifest)?;
            copy_public(config)?;
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

    /// Discover entry module ids for `env` (§77), honoring hooked
    /// `options.entries` (HTML files or module specifiers).
    async fn discover_entries(
        &self,
        server: &DevServer,
        env: &str,
        hooked: &[String],
    ) -> Result<Vec<ModuleId>> {
        if env == "ssr" {
            let mut entries = Vec::new();
            for entry in hooked {
                let spec = format!("/{}", entry.trim_start_matches('/'));
                entries.push(server.resolve_entry(&spec, env).await?);
            }
            return Ok(entries);
        }
        // Library mode (§55): the lib entry wins unless hooks replaced it.
        if let Some(lib) = &self.config.build.lib {
            if hooked == self.config.build.entries.as_slice() {
                let spec = format!("/{}", lib.entry.trim_start_matches('/'));
                return Ok(vec![server.resolve_entry(&spec, env).await?]);
            }
            let mut entries = Vec::new();
            for entry in hooked {
                let spec = format!("/{}", entry.trim_start_matches('/'));
                entries.push(server.resolve_entry(&spec, env).await?);
            }
            return Ok(entries);
        }
        // HTML entries (§77); missing files fall back to module resolution
        // so hooked entries may name modules directly.
        let mut entries = Vec::new();
        for html_entry in hooked {
            let file = self.config.root.join(html_entry);
            if !file.exists() {
                let spec = format!("/{}", html_entry.trim_start_matches('/'));
                match server.resolve_entry(&spec, env).await {
                    Ok(id) => entries.push(id),
                    Err(_) => tracing::warn!("entry `{html_entry}` not found; skipping"),
                }
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
    fn ssr_entry(&self) -> Result<Option<String>> {
        if let Some(entry) = self
            .config
            .ssr
            .entry
            .as_deref()
            .filter(|entry| *entry != "src/server.rs")
        {
            let path = std::path::Path::new(entry);
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(FerriteError::Build(format!("SSR entry `{entry}` must be a project-relative module path without parent traversal")));
            }
            if !self.config.root.join(path).is_file() {
                return Err(FerriteError::Build(format!("configured SSR entry `{entry}` is not a file; create it or correct [ssr].entry")));
            }
            if !ferrite_core::ModuleType::from_path(entry).is_js_like() {
                return Err(FerriteError::Build(format!("configured SSR entry `{entry}` requires a JavaScript/TypeScript module; Rust and framework renderer execution are unavailable in this build path")));
            }
            return Ok(Some(entry.to_string()));
        }
        for candidate in [
            "src/entry-server.ts",
            "src/entry-server.tsx",
            "src/entry-server.js",
            "src/server.ts",
            "src/server.js",
        ] {
            if self.config.root.join(candidate).is_file() {
                return Ok(Some(candidate.to_string()));
            }
        }
        Ok(None)
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

/// [`BundleHooks`] over a [`PluginContainer`]: the builder side of the
/// render pipeline (`renderStart` / `renderChunk` / `augmentChunkHash` /
/// `banner` / `intro` / `outro` / `footer`).
struct ContainerRenderHooks<'a> {
    container: &'a PluginContainer,
    ctx: &'a ferrite_plugin::PluginContext<'a>,
}

#[async_trait::async_trait]
impl BundleHooks for ContainerRenderHooks<'_> {
    async fn render_start(&self, entries: &[ModuleId]) -> Result<()> {
        self.container
            .hook_render_start(
                self.ctx,
                ferrite_plugin::RenderStart {
                    entries: entries.to_vec(),
                },
            )
            .await
    }

    async fn render_chunk(&self, id: &str, code: String, is_entry: bool) -> Result<Option<String>> {
        if self.container.is_empty() {
            return Ok(None);
        }
        let result = self
            .container
            .hook_render_chunk(
                self.ctx,
                ferrite_plugin::RenderChunk {
                    id: id.to_string(),
                    code,
                    is_entry,
                },
            )
            .await?;
        Ok(result.code)
    }

    async fn chunk_hash_extra(&self, chunk_id: &str) -> Result<Vec<String>> {
        if self.container.is_empty() {
            return Ok(Vec::new());
        }
        self.container
            .hook_augment_chunk_hash(self.ctx, chunk_id)
            .await
    }

    async fn chunk_wrapper(
        &self,
        id: &str,
        code: &str,
        is_entry: bool,
    ) -> Result<ferrite_plugin::ChunkWrapper> {
        if self.container.is_empty() {
            return Ok(ferrite_plugin::ChunkWrapper::default());
        }
        self.container
            .hook_chunk_wrapper(
                self.ctx,
                ferrite_plugin::RenderChunk {
                    id: id.to_string(),
                    code: code.to_string(),
                    is_entry,
                },
            )
            .await
    }
}
