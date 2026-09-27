//! Ferrite — Rust-native SSR web toolchain (spec §9).
//!
//! Public API (Vite programmatic-API equivalent):
//!
//! ```rust,no_run
//! #[tokio::main]
//! async fn main() -> ferrite::Result<()> {
//!     let mut server = ferrite::create_server(ferrite::Config::default()).await?;
//!     server.listen().await?;
//!     Ok(())
//! }
//! ```
//!
//! Builder:
//!
//! ```rust,no_run
//! # async fn run() -> ferrite::Result<()> {
//! let builder = ferrite::create_builder(ferrite::Config::default()).await?;
//! builder.build_app().await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// --- re-exported crates ------------------------------------------------------
pub use ferrite_assets as assets;
pub use ferrite_bundler as bundler;
pub use ferrite_cache as cache;
pub use ferrite_config as config;
pub use ferrite_core as core;
pub use ferrite_css as css;
pub use ferrite_docs as docs;
pub use ferrite_frameworks as frameworks;
pub use ferrite_graph as graph;
pub use ferrite_hmr as hmr;
pub use ferrite_html as html;
pub use ferrite_manifest as manifest;
pub use ferrite_npm as npm;
pub use ferrite_plugin as plugin;
pub use ferrite_resolver as resolver;
pub use ferrite_runtime as runtime;
pub use ferrite_server as server;
pub use ferrite_ssr as ssr;
pub use ferrite_tailwind as tailwind;
pub use ferrite_transform as transform;
pub use ferrite_wasm as wasm;

pub mod package;

// --- re-exported vocabulary --------------------------------------------------
pub use ferrite_config::{
    load_user_config, merge_user_config, resolve_config, CliOverrides, ResolvedConfig, UserConfig,
};
pub use ferrite_core::{
    Environment, EnvironmentKind, FerriteError, Hash, ModuleId, ModuleType, Result, SourceMap,
    Target, VERSION,
};
pub use ferrite_plugin::{Apply, Enforce, Plugin, PluginContainer};
pub use ferrite_server::DevServer;

use ferrite_bundler::{
    BuildBundleConfig, BundleRequest, Bundler as _, CssExtract, FerriteBundler, LoadedModule,
    ModuleLoader,
};

/// Prelude for framework and plugin authors.
pub mod prelude {
    pub use crate::{
        Apply, Config, Enforce, Environment, EnvironmentKind, FerriteError, Hash, ModuleId,
        ModuleType, Plugin, PluginContainer, ResolvedConfig, Result, SourceMap, Target, UserConfig,
        VERSION,
    };
    pub use async_trait::async_trait;
}

/// Programmatic configuration (spec §10 Rust config).
///
/// ```rust
/// use ferrite::prelude::*;
/// let config = ferrite::Config::default().alias("@", "./src");
/// ```
#[derive(Default)]
pub struct Config {
    /// User configuration (merged over file config at resolve time).
    pub user: UserConfig,
    /// Plugins.
    pub plugins: Vec<Arc<dyn Plugin>>,
    /// Project root hint.
    pub root: Option<PathBuf>,
    /// CLI overrides (highest precedence).
    pub overrides: CliOverrides,
}

impl Config {
    /// Empty config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plugin.
    #[must_use]
    pub fn plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Arc::new(plugin));
        self
    }

    /// Add an import alias.
    #[must_use]
    pub fn alias(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.user.resolve.alias.insert(from.into(), to.into());
        self
    }

    /// Add a build entry (HTML file or module, §77).
    #[must_use]
    pub fn entry(mut self, entry: impl Into<String>) -> Self {
        let entry = entry.into();
        if !self.user.build.entries.contains(&entry) {
            self.user.build.entries.push(entry);
        }
        self
    }

    /// Resolve into concrete config + plugins (runs `config` hooks).
    pub async fn resolve(self) -> Result<(ResolvedConfig, Vec<Arc<dyn Plugin>>)> {
        let mut user = self.user;
        let container = PluginContainer::new(self.plugins.clone(), Apply::All);
        container.hook_config(&mut user).await?;
        // Merge file config under programmatic config.
        let root_hint = self.root.clone().unwrap_or_else(|| PathBuf::from("."));
        let file_config = load_user_config(&root_hint).unwrap_or_default();
        let merged = merge_user_config(file_config, user);
        let resolved = resolve_config(merged, Some(root_hint), self.overrides)?;
        Ok((resolved, self.plugins))
    }
}

/// Create a dev server (spec §9, §106).
pub async fn create_server(config: Config) -> Result<DevServer> {
    let (resolved, plugins) = config.resolve().await?;
    DevServer::new(resolved, plugins).await
}

/// Create a builder (spec §9).
pub async fn create_builder(config: Config) -> Result<Builder> {
    let mut config = config;
    // Builds default to production mode unless explicitly set.
    if config.overrides.mode.is_none() && config.user.mode.is_none() {
        config.overrides.mode = Some("production".to_string());
    }
    let (resolved, plugins) = config.resolve().await?;
    Ok(Builder::new(resolved, plugins))
}

/// One-shot production build (spec §9).
pub async fn build(config: Config) -> Result<Vec<BuildReport>> {
    create_builder(config).await?.build_app().await
}

/// Preview a production build (spec §9).
pub async fn preview(config: Config) -> Result<()> {
    let (resolved, _) = config.resolve().await?;
    preview_dir(&resolved.out_dir(), resolved.server.port).await
}

// --- Ferrite core struct (§99) -------------------------------------------------

/// First core type: config + plugins + graph + resolver + compiler.
pub struct Ferrite {
    /// Resolved config.
    pub config: ResolvedConfig,
    /// Plugin container.
    pub plugins: PluginContainer,
    /// Module graph.
    pub graph: Arc<ferrite_graph::ModuleGraph>,
    /// Resolver.
    pub resolver: Arc<ferrite_resolver::Resolver>,
    /// JS compiler.
    pub compiler: Arc<dyn ferrite_transform::JsCompiler>,
    /// Emitted files (plugin context backing).
    emitted: Mutex<HashMap<String, ferrite_plugin::EmittedFile>>,
    /// Watch files (plugin context backing).
    watch_files: Mutex<Vec<String>>,
    /// Warnings (plugin context backing).
    warnings: Mutex<Vec<String>>,
}

/// A module transform request.
#[derive(Debug, Clone)]
pub struct ModuleRequest {
    /// Specifier to resolve.
    pub specifier: String,
    /// Importer, if any.
    pub importer: Option<ModuleId>,
    /// Target environment.
    pub environment: EnvironmentKind,
}

impl Ferrite {
    /// Create from resolved config + plugins.
    #[must_use]
    pub fn new(config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Self {
        let mode = if config.is_production {
            Apply::Build
        } else {
            Apply::Serve
        };
        let resolver = ferrite_resolver::Resolver::for_environment(
            config.root.clone(),
            &config.resolve,
            &EnvironmentKind::Client,
        );
        let compiler: Arc<dyn ferrite_transform::JsCompiler> =
            ferrite_transform::compiler_for_engine(&config.compiler.engine).unwrap_or_else(|_| {
                Arc::new(ferrite_transform::OxcCompiler::new(Default::default()))
            });
        Self {
            config,
            plugins: PluginContainer::new(plugins, mode),
            graph: Arc::new(ferrite_graph::ModuleGraph::default()),
            resolver: Arc::new(resolver),
            compiler,
            emitted: Mutex::new(HashMap::new()),
            watch_files: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
        }
    }

    /// Resolve → load → transform one module (spec §99).
    pub async fn transform_request(
        &self,
        request: ModuleRequest,
    ) -> Result<ferrite_transform::TransformResult> {
        let environment = ferrite_core::Environment::new("client", request.environment.clone());
        let ctx = ferrite_plugin::PluginContext {
            graph: &self.graph,
            resolver: &self.resolver,
            environment: &environment,
            emitted: &self.emitted,
            watch_files: &self.watch_files,
            warnings: &self.warnings,
        };
        // Resolve (plugin first).
        let resolved = match self
            .plugins
            .hook_resolve_id(
                &ctx,
                ferrite_plugin::ResolveHookRequest {
                    specifier: &request.specifier,
                    importer: request.importer.as_ref(),
                    environment: request.environment.clone(),
                    ssr: request.environment.is_ssr(),
                },
            )
            .await?
        {
            Some(resolved) => resolved,
            None => self.resolver.resolve(&ferrite_resolver::ResolveRequest {
                specifier: &request.specifier,
                importer: request.importer.as_ref(),
                environment: request.environment.clone(),
                kind: ferrite_resolver::ResolveKind::Import,
            })?,
        };
        // Load (plugin first, then fs).
        let loaded = match self
            .plugins
            .hook_load(
                &ctx,
                ferrite_plugin::LoadRequest {
                    id: resolved.id.0.clone(),
                    environment: request.environment.clone(),
                },
            )
            .await?
        {
            Some(loaded) => loaded,
            None => {
                let file = self.config.root.join(resolved.id.0.trim_start_matches('/'));
                let code = std::fs::read_to_string(&file).map_err(|_| {
                    FerriteError::Resolve(format!("cannot load `{}`", resolved.id.0))
                })?;
                ferrite_plugin::LoadResult {
                    code,
                    module_type: ModuleType::from_path(&file),
                    dependencies: Vec::new(),
                }
            }
        };
        // Transform (compiler, then plugin chain).
        let ssr = request.environment.is_ssr();
        let compiled = self
            .compiler
            .transform(ferrite_transform::TransformRequest {
                id: resolved.id.0.clone(),
                code: loaded.code,
                module_type: loaded.module_type.clone(),
                environment: request.environment.clone(),
                ssr,
                target: self.config.target(),
                minify: false,
                sourcemap: false,
                define: HashMap::new(),
                jsx_runtime: self.config.react.runtime.clone(),
                development: !self.config.is_production,
            })?;
        let hooked = self
            .plugins
            .hook_transform(
                &ctx,
                ferrite_plugin::TransformRequest {
                    id: resolved.id.0,
                    code: compiled.code,
                    module_type: loaded.module_type,
                    environment: request.environment,
                    ssr,
                },
            )
            .await?;
        Ok(ferrite_transform::TransformResult {
            code: hooked.code,
            map: hooked.map.or(compiled.map),
            dependencies: compiled.dependencies,
            imports: compiled.imports,
            exports: compiled.exports,
        })
    }
}

// --- builder -------------------------------------------------------------------

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
            let opts = package::StandaloneOptions {
                embed_assets: config.package.embed_assets,
                compress_assets: config.package.compress_assets,
                target: config.package.target.clone(),
                cargo: None,
            };
            let out = out_dir.clone();
            let report =
                tokio::task::spawn_blocking(move || package::write_standalone(&out, &opts))
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
fn manifest_entry<'a>(
    manifest: &'a ferrite_manifest::BuildManifest,
    src: &str,
) -> Option<&'a ferrite_manifest::ManifestEntry> {
    manifest.entries.get(src).or_else(|| {
        let alt = src.strip_prefix('/').unwrap_or(src);
        manifest.entries.get(&format!("/{alt}"))
    })
}

/// Inject stylesheet links before `</head>` (document start fallback).
fn inject_stylesheets(html: &str, hrefs: &[String]) -> String {
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

/// A single-environment build report.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// Environment.
    pub env: String,
    /// Output directory.
    pub out_dir: PathBuf,
    /// Entry ids.
    pub entries: Vec<String>,
    /// Files written.
    pub chunks: usize,
    /// Code bytes.
    pub bytes: usize,
    /// Modules emitted.
    pub modules: usize,
    /// Modules dropped by tree-shaking.
    pub dropped: usize,
    /// Prebuilt standalone binary (`--target`), when produced.
    pub standalone_binary: Option<PathBuf>,
}

/// [`ModuleLoader`] over the dev-server pipeline (build mode).
pub struct BuildLoader {
    /// Pipeline server (production config, no watcher).
    pub server: DevServer,
    /// Minify per module.
    pub minify: bool,
    /// Emit source maps.
    pub sourcemap: bool,
    /// Public base with trailing slash.
    pub base: String,
    /// Loader-emitted assets (name → bytes).
    assets: Mutex<HashMap<String, Vec<u8>>>,
}

impl BuildLoader {
    /// Drain emitted assets.
    #[must_use]
    pub fn take_assets(&self) -> HashMap<String, Vec<u8>> {
        self.assets
            .lock()
            .map(|mut map| std::mem::take(&mut *map))
            .unwrap_or_default()
    }

    /// Emit raw bytes as a hashed asset; returns the public URL.
    fn emit_bytes(&self, file_name: &str, bytes: Vec<u8>) -> String {
        let hashed = ferrite_assets::hashed_name(file_name, &bytes);
        let name = format!("assets/{hashed}");
        let url = format!("{}{name}", self.base);
        if let Ok(mut assets) = self.assets.lock() {
            assets.insert(name, bytes);
        }
        url
    }

    /// Load a stylesheet for extraction (§29).
    ///
    /// Transforms (module scoping + minify), emits relative `url()` assets,
    /// and reports relative `@import`s as deps; the bundler turns those
    /// into hashed `.css` files. Absolute `/` refs point at `public/` and
    /// pass through; remote refs stay untouched.
    async fn load_css(&self, id: &ModuleId, env: &str) -> Result<LoadedModule> {
        let (source, _) = self.server.load_raw_source(id, env).await?;
        let is_modules = id.0.contains(".module.css");
        let result = ferrite_css::transform_css(
            &id.0,
            &source,
            &ferrite_css::CssOptions {
                modules: is_modules,
                minify: self.minify,
                dev: false,
            },
        );
        let env = self.server.inner().config.client_env();
        let mut url_mapping: HashMap<String, String> = HashMap::new();
        for found in &result.urls {
            let spec = &found.url;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            match self.resolve_css_ref(id, spec, &env) {
                Ok(dep) => match self.server.id_to_file(&dep).and_then(|file| {
                    let bytes = std::fs::read(&file)?;
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "asset".to_string());
                    Ok((name, bytes))
                }) {
                    Ok((name, bytes)) => {
                        url_mapping.insert(spec.clone(), self.emit_bytes(&name, bytes));
                    }
                    Err(error) => {
                        tracing::warn!("cannot emit CSS asset `{spec}` from `{id}`: {error}");
                    }
                },
                Err(error) => {
                    tracing::warn!("cannot resolve CSS asset `{spec}` from `{id}`: {error}");
                }
            }
        }
        let text =
            ferrite_css::rewrite_css_urls(&result.code, |spec| url_mapping.get(spec).cloned());
        let mut imports = Vec::new();
        for found in &result.imports {
            let spec = &found.specifier;
            if !spec.starts_with("./") && !spec.starts_with("../") {
                continue;
            }
            match self.resolve_css_ref(id, spec, &env) {
                Ok(dep) => imports.push((spec.clone(), dep, ferrite_graph::ImportKind::Static)),
                Err(error) => {
                    tracing::warn!("cannot resolve CSS @import `{spec}` from `{id}`: {error}");
                }
            }
        }
        // Deterministic stub (sorted export map — content-hashed downstream).
        let code = if is_modules {
            let exports: std::collections::BTreeMap<&String, &String> =
                result.exports.iter().collect();
            format!(
                "export default {};\n",
                serde_json::to_string(&exports).unwrap_or_else(|_| "{}".to_string())
            )
        } else {
            "export default undefined;\n".to_string()
        };
        // CSS `@import` edges pull styles, not JS bindings; the stub exports
        // only `default`.
        let shake = ferrite_transform::ShakeInfo {
            import_bindings: imports
                .iter()
                .map(|_| vec![ferrite_transform::ImportBinding::SideEffect])
                .collect(),
            exports: vec![ferrite_transform::ParsedExport {
                exported: "default".to_string(),
                local: None,
                from: None,
                imported: None,
                target: None,
            }],
        };
        Ok(LoadedModule {
            id: id.clone(),
            code,
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: Some(CssExtract { text, is_modules }),
            shake: Some(shake),
        })
    }

    /// Resolve a CSS-relative ref through the client resolver.
    fn resolve_css_ref(&self, id: &ModuleId, spec: &str, env: &Environment) -> Result<ModuleId> {
        let resolved =
            self.server
                .inner()
                .client_resolver
                .resolve(&ferrite_resolver::ResolveRequest {
                    specifier: spec,
                    importer: Some(id),
                    environment: env.kind.clone(),
                    kind: ferrite_resolver::ResolveKind::Css,
                })?;
        if resolved.external || !resolved.id.0.starts_with('/') {
            return Err(FerriteError::Resolve(format!(
                "CSS ref `{spec}` from `{id}` is not a local file"
            )));
        }
        Ok(resolved.id)
    }
}

#[async_trait::async_trait]
impl ModuleLoader for BuildLoader {
    async fn load(&self, id: &ModuleId, env: &str) -> Result<LoadedModule> {
        // CSS extraction (§29): plain styles ride `LoadedModule.css` into
        // hashed `.css` assets (query variants like `?inline` keep the
        // normal pipeline).
        let (path, query) = id.split_query();
        if query.is_none() && ModuleType::from_path(path) == ModuleType::Css {
            return self.load_css(id, env).await;
        }
        let module = self.server.pipeline_module(id, None, env).await?;
        // Raw assets become hashed files + URL shims.
        if module.is_raw_bytes {
            let file = self.server.id_to_file(id)?;
            let bytes = std::fs::read(&file)?;
            let name = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "asset".to_string());
            let url = self.emit_bytes(&name, bytes);
            return Ok(LoadedModule {
                id: id.clone(),
                code: ferrite_assets::asset_to_js(&url),
                imports: Vec::new(),
                side_effects: module.side_effects,
                module_type: ModuleType::Js,
                map: None,
                css: None,
                shake: Some(ferrite_transform::ShakeInfo::default()),
            });
        }
        // The pipeline already rewrote specifiers to resolved urls, so the
        // bundler must key replacements by the *resolved* text present in
        // `code` (not the original specifier).
        let mut code = module.code.clone();
        let mut imports = Vec::new();
        let mut import_bindings = Vec::new();
        for (index, (_specifier, dep, kind)) in module.imports.iter().enumerate() {
            // Dropped `?asset-shim` edges drop their bindings in step.
            let bindings = module
                .shake
                .as_ref()
                .and_then(|shake| shake.import_bindings.get(index).cloned());
            if let Some(plain) = dep.0.strip_suffix("?asset-shim") {
                // Eagerly resolve `?asset-shim` imports to final URLs.
                let asset_id = ModuleId::new(plain);
                match self.server.id_to_file(&asset_id).and_then(|file| {
                    let bytes = std::fs::read(&file)?;
                    let name = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "asset".to_string());
                    Ok((name, bytes))
                }) {
                    Ok((name, bytes)) => {
                        let url = self.emit_bytes(&name, bytes);
                        for quote in ['"', '\'', '`'] {
                            code = code.replace(
                                &format!("{quote}{}{quote}", dep.0),
                                &format!("{quote}{url}{quote}"),
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!("cannot emit asset `{plain}`: {error}");
                        imports.push((dep.0.clone(), dep.clone(), kind.clone()));
                        if let Some(bindings) = bindings {
                            import_bindings.push(bindings);
                        }
                    }
                }
            } else {
                imports.push((dep.0.clone(), dep.clone(), kind.clone()));
                if let Some(bindings) = bindings {
                    import_bindings.push(bindings);
                }
            }
        }
        // Alignment is all-or-nothing: a partial binding list would
        // misattribute names, so a short list voids the shake facts.
        let shake = module.shake.as_ref().and_then(|shake| {
            (import_bindings.len() == imports.len()).then(|| ferrite_transform::ShakeInfo {
                import_bindings,
                exports: shake.exports.clone(),
            })
        });
        // Minify per module (production).
        let mut map = if self.sourcemap { module.map } else { None };
        if self.minify && module.module_type.is_js_like() {
            let minified =
                self.server
                    .inner()
                    .compiler
                    .minify(ferrite_transform::MinifyRequest {
                        id: id.0.clone(),
                        code: code.clone(),
                        sourcemap: self.sourcemap,
                        input_map: map.clone().map(ferrite_core::SourceMap::external),
                    })?;
            code = minified.code;
            map = minified.map.map(|chained| chained.mappings);
        }
        Ok(LoadedModule {
            id: id.clone(),
            code,
            imports,
            side_effects: module.side_effects,
            module_type: module.module_type.clone(),
            map,
            css: None,
            shake,
        })
    }
}

// --- preview -------------------------------------------------------------------

/// Serve `dir` statically with SPA fallback (`preview`, §83 light).
pub async fn preview_dir(dir: &std::path::Path, port: u16) -> Result<()> {
    use axum::extract::State;
    use axum::http::{StatusCode, Uri};
    use axum::response::IntoResponse as _;
    #[derive(Clone)]
    struct PreviewState {
        dir: PathBuf,
    }
    async fn handler(
        State(state): State<PreviewState>,
        uri: Uri,
    ) -> impl axum::response::IntoResponse {
        let path = uri.path().trim_start_matches('/');
        let file = state.dir.join(path);
        let file = if file.is_file() {
            file
        } else {
            state.dir.join("index.html")
        };
        match std::fs::read(&file) {
            Ok(bytes) => {
                let content_type = ferrite_assets::content_type(file.to_string_lossy().as_ref());
                let immutable = file
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains('-'));
                let cache = if immutable {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                };
                (
                    StatusCode::OK,
                    [
                        (axum::http::header::CONTENT_TYPE, content_type),
                        (axum::http::header::CACHE_CONTROL, cache.to_string()),
                    ],
                    bytes,
                )
                    .into_response()
            }
            Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
        }
    }
    if !dir.exists() {
        return Err(FerriteError::Build(format!(
            "`{}` does not exist; run `ferrite build` first",
            dir.display()
        )));
    }
    let router = axum::Router::new()
        .fallback(handler)
        .with_state(PreviewState {
            dir: dir.to_path_buf(),
        });
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|error| FerriteError::Other(format!("cannot bind preview:{port}: {error}")))?;
    let addr = listener
        .local_addr()
        .map_err(|error| FerriteError::Other(error.to_string()))?;
    tracing::info!("preview at http://{addr}");
    println!("  preview at http://{addr}");
    axum::serve(listener, router)
        .await
        .map_err(|error| FerriteError::Other(error.to_string()))?;
    Ok(())
}

// --- helpers -------------------------------------------------------------------

/// Ensure a trailing slash on the base.
fn with_trailing_slash(base: &str) -> String {
    if base.ends_with('/') {
        base.to_string()
    } else {
        format!("{base}/")
    }
}

/// Copy `public/` → `out_dir/` (verbatim).
fn copy_public(config: &ResolvedConfig) -> Result<()> {
    let public = config.root.join("public");
    if !public.exists() {
        return Ok(());
    }
    copy_dir(&public, &config.out_dir())
}

/// Copy a directory tree.
fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
