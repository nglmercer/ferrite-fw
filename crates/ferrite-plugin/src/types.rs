//! Plugin hook types.

use ferrite_config::ResolvedConfig;
use ferrite_core::Environment;
use ferrite_core::EnvironmentKind;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_core::SourceMap;
use ferrite_graph::ModuleGraph;
use ferrite_graph::ModuleNode;
use ferrite_resolver::ResolveKind;
use ferrite_resolver::ResolvedId;
use ferrite_resolver::Resolver;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

/// Hook ordering (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Enforce {
    /// Run before core plugins.
    Pre,
    /// Default order.
    Normal,
    /// Run after core plugins.
    Post,
}

/// Transform position relative to the JavaScript/TypeScript compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformPhase {
    BeforeLowering,
    AfterLowering,
}

/// When a plugin applies (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// Dev server only.
    Serve,
    /// Build only.
    Build,
    /// Both.
    All,
}

impl Apply {
    /// True when the plugin applies to `mode`.
    #[must_use]
    pub fn applies(&self, mode: Apply) -> bool {
        matches!(self, Self::All) || *self == mode
    }
}

/// Narrow control surface a plugin can use during `configure_server`.
///
/// (The full `DevServer` type lives in `ferrite-server`, which depends on
/// this crate; the trait keeps the dependency one-directional.)
pub trait ServerControl: Send + Sync {
    /// Resolved configuration.
    fn resolved_config(&self) -> &ResolvedConfig;
    /// Project root.
    fn root(&self) -> &Path;
    /// Module graph handle (`server.moduleGraph`), when available.
    fn module_graph(&self) -> Option<&ModuleGraph> {
        None
    }
    /// Bound socket address (`server.httpServer` equivalent), when listening.
    fn local_addr(&self) -> Option<SocketAddr> {
        None
    }
    /// Display URLs for this server.
    fn server_urls(&self) -> ServerUrls {
        let config = self.resolved_config();
        ServerUrls {
            local: format!("http://{}:{}/", config.server.host, config.server.port),
            network: None,
        }
    }
    /// Print Local/Network URLs, the `printUrls` equivalent.
    fn print_urls(&self) {
        let urls = self.server_urls();
        println!("  Local:   {}", urls.local);
        match &urls.network {
            Some(network) => println!("  Network: {network}"),
            None => println!("  Network: use --host to expose"),
        }
    }
    /// Connected HMR clients (`server.ws` equivalent count).
    fn hmr_clients(&self) -> usize {
        0
    }
    /// Broadcast a full reload to HMR clients.
    fn send_full_reload(&self, _path: Option<&str>) {}
    /// True when the file watcher is running.
    fn watcher_alive(&self) -> bool {
        false
    }
    /// Watch an extra path, the `server.watcher.add` equivalent.
    fn watcher_add(&self, _path: &Path) {}
}

/// Display URLs for a server.
#[derive(Debug, Clone)]
pub struct ServerUrls {
    /// Local URL (`http://127.0.0.1:5173/`).
    pub local: String,
    /// LAN URL, when the host is reachable off-machine.
    pub network: Option<String>,
}

/// Plugin hook context (§13).
pub struct PluginContext<'a> {
    /// Module graph.
    pub graph: &'a ModuleGraph,
    /// Resolver.
    pub resolver: &'a Resolver,
    /// Current environment.
    pub environment: &'a Environment,
    /// Emitted files (shared with the build/dev pipeline).
    pub emitted: &'a Mutex<HashMap<String, EmittedFile>>,
    /// Extra watch files registered by plugins.
    pub watch_files: &'a Mutex<Vec<String>>,
    /// Collected warnings.
    pub warnings: &'a Mutex<Vec<String>>,
}

impl PluginContext<'_> {
    /// Resolve a specifier (plugin `resolveId` equivalent helper).
    pub fn resolve(&self, specifier: &str, importer: Option<&ModuleId>) -> Result<ResolvedId> {
        self.resolver.resolve(&ferrite_resolver::ResolveRequest {
            specifier,
            importer,
            environment: self.environment.kind.clone(),
            kind: ResolveKind::Import,
        })
    }

    /// Emit a file into the bundle / dev pipeline.
    pub fn emit_file(&self, file: EmittedFile) {
        if let Ok(mut emitted) = self.emitted.lock() {
            emitted.insert(file.name.clone(), file);
        }
    }

    /// Look up an emitted file name.
    #[must_use]
    pub fn get_file_name(&self, reference_id: &str) -> Option<String> {
        self.emitted
            .lock()
            .ok()
            .and_then(|emitted| emitted.get(reference_id).map(|file| file.name.clone()))
    }

    /// Module info for an id.
    #[must_use]
    pub fn get_module_info(&self, id: &ModuleId) -> Option<ModuleNode> {
        self.graph.get(id)
    }

    /// All known module ids.
    #[must_use]
    pub fn get_module_ids(&self) -> Vec<ModuleId> {
        self.graph.module_ids()
    }

    /// Register an extra watch file.
    pub fn add_watch_file(&self, file: &str) {
        if let Ok(mut watch) = self.watch_files.lock() {
            if !watch.iter().any(|entry| entry == file) {
                watch.push(file.to_string());
            }
        }
    }

    /// Record a warning.
    pub fn warn(&self, message: &str) {
        if let Ok(mut warnings) = self.warnings.lock() {
            warnings.push(message.to_string());
        }
    }
}

/// A file emitted into the output bundle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmittedFile {
    /// Output file name.
    pub name: String,
    /// File bytes.
    #[serde(skip)]
    pub contents: Vec<u8>,
    /// True for entry chunks.
    pub is_entry: bool,
}

/// Output bundle passed to `generate_bundle` / `write_bundle`.
#[derive(Debug, Default)]
pub struct OutputBundle {
    /// Files by output name.
    pub files: HashMap<String, EmittedFile>,
}

impl OutputBundle {
    /// Insert or replace a file.
    pub fn insert(&mut self, file: EmittedFile) {
        self.files.insert(file.name.clone(), file);
    }

    /// Remove a file.
    pub fn remove(&mut self, name: &str) -> Option<EmittedFile> {
        self.files.remove(name)
    }
}

/// `resolveId` hook request.
#[derive(Debug, Clone)]
pub struct ResolveHookRequest<'a> {
    /// Raw specifier.
    pub specifier: &'a str,
    /// Importer, if any.
    pub importer: Option<&'a ModuleId>,
    /// Environment.
    pub environment: EnvironmentKind,
    /// True for SSR resolution.
    pub ssr: bool,
}

/// `load` hook request (§68).
#[derive(Debug, Clone)]
pub struct LoadRequest {
    /// Resolved id to load.
    pub id: String,
    /// Environment.
    pub environment: EnvironmentKind,
}

/// `load` hook result (§68).
#[derive(Debug, Clone)]
pub struct LoadResult {
    /// Module source.
    pub code: String,
    /// Module type override.
    pub module_type: ModuleType,
    /// Extra watched filesystem paths (absolute or project-root-relative; never browser URLs).
    pub dependencies: Vec<String>,
    /// Map back to the original source.
    pub map: Option<SourceMap>,
    /// Explicit side-effect information, when supplied by the loader.
    pub side_effects: Option<bool>,
}

impl Default for LoadResult {
    fn default() -> Self {
        Self {
            code: String::new(),
            module_type: ModuleType::Js,
            dependencies: Vec::new(),
            map: None,
            side_effects: None,
        }
    }
}

/// `transform` hook request.
#[derive(Debug, Clone)]
pub struct TransformRequest {
    /// Module id.
    pub id: String,
    /// Current code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
    /// Environment.
    pub environment: EnvironmentKind,
    /// True for SSR transforms.
    pub ssr: bool,
}

/// `transform` hook result.
#[derive(Debug, Clone)]
pub struct TransformResult {
    /// Transformed code.
    pub code: String,
    /// Source map.
    pub map: Option<SourceMap>,
    /// Extra dependencies.
    pub dependencies: Vec<String>,
    /// Optional type override; pre transforms may return JavaScript from components.
    pub module_type: Option<ModuleType>,
}

/// HTML transformation context (`transformIndexHtml`).
#[derive(Debug, Clone)]
pub struct HtmlTransformContext {
    /// Current HTML.
    pub html: String,
    /// Request path.
    pub path: String,
    /// True for SSR HTML.
    pub ssr: bool,
}

/// HTML transformation result.
#[derive(Debug, Clone, Default)]
pub struct HtmlTransformResult {
    /// Replacement HTML (when fully overridden).
    pub html: Option<String>,
    /// Tags to inject.
    pub tags: Vec<HtmlTag>,
}

/// An injected HTML tag.
#[derive(Debug, Clone)]
pub struct HtmlTag {
    /// Tag name (`script`, `link`, `meta`, ...).
    pub tag: String,
    /// Attributes.
    pub attrs: HashMap<String, String>,
    /// Inner HTML.
    pub children: Option<String>,
    /// Injection point.
    pub inject_to: HtmlInjectTo,
}

/// HTML injection points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HtmlInjectTo {
    /// `<head>` (default).
    #[default]
    Head,
    /// Start of `<head>`.
    HeadPrepend,
    /// `<body>`.
    Body,
    /// Start of `<body>`.
    BodyPrepend,
}

/// Hot-update event (`handleHotUpdate`).
#[derive(Debug, Clone)]
pub struct HotUpdateEvent {
    /// Changed file (root-relative URL).
    pub file: String,
    /// Affected modules.
    pub modules: Vec<ModuleId>,
    /// Timestamp (unix millis).
    pub timestamp: u64,
}

/// Hot-update result: custom boundary or full reload.
#[derive(Debug, Clone, Default)]
pub struct HotUpdateResult {
    /// Modules to send as the update boundary.
    pub modules: Vec<ModuleId>,
    /// Force a full reload.
    pub full_reload: bool,
}

/// `moduleParsed` hook payload.
#[derive(Debug, Clone)]
pub struct ModuleParsed {
    /// Parsed module id.
    pub id: ModuleId,
}

/// `buildEnd` hook payload.
#[derive(Debug, Clone, Default)]
pub struct BuildEnd {
    /// Error message when the build failed, if any.
    pub error: Option<String>,
}

/// `renderStart` hook payload.
#[derive(Debug, Clone)]
pub struct RenderStart {
    /// Entry ids.
    pub entries: Vec<ModuleId>,
}

/// `renderChunk` hook payload.
#[derive(Debug, Clone)]
pub struct RenderChunk {
    /// Chunk id.
    pub id: String,
    /// Current chunk code.
    pub code: String,
    /// True for entry chunks.
    pub is_entry: bool,
}

/// `renderChunk` hook result.
#[derive(Debug, Clone, Default)]
pub struct RenderChunkResult {
    /// Replacement code.
    pub code: Option<String>,
    /// Replacement map.
    pub map: Option<SourceMap>,
}

/// Input options for the `options` hook (Rollup `options` equivalent).
#[derive(Debug, Clone)]
pub struct BundleOptions {
    /// Entry specifiers (HTML files or module ids).
    pub entries: Vec<String>,
    /// Drop unused export statements, then re-minify.
    pub treeshake: bool,
    /// Minify output.
    pub minify: bool,
    /// Emit source maps.
    pub sourcemap: bool,
    /// Scope-hoist each entry closure into one file.
    pub scope_hoist: bool,
}

/// Output options for the `output_options` hook (Rollup equivalent).
#[derive(Debug, Clone)]
pub struct OutputOptions {
    /// Chunk file pattern (`[name]` / `[hash]` placeholders).
    pub chunk_pattern: String,
    /// Extracted CSS file pattern.
    pub css_pattern: String,
    /// Asset file pattern (`[name]` / `[hash]` / `[ext]`).
    pub asset_pattern: String,
}

/// `resolveDynamicImport` hook request (Rollup equivalent).
#[derive(Debug, Clone)]
pub struct DynamicImportRequest {
    /// Raw dynamic specifier.
    pub specifier: String,
    /// Importer module.
    pub importer: ModuleId,
    /// Environment.
    pub environment: EnvironmentKind,
}

/// `shouldTransformCachedModule` hook payload (Vite equivalent).
#[derive(Debug, Clone)]
pub struct CachedModuleInfo {
    /// Cached module id.
    pub id: ModuleId,
    /// Environment the cache entry was built for.
    pub environment: EnvironmentKind,
}

/// File-watcher event for the `watchChange` hook (Vite equivalent).
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// Changed path (absolute).
    pub path: PathBuf,
    /// Change kind.
    pub kind: WatchKind,
}

/// Watcher change kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchKind {
    /// File created.
    Create,
    /// File modified.
    Modify,
    /// File removed.
    Remove,
}

/// `resolveFileUrl` hook request (Vite equivalent): map an emitted file to
/// its public URL.
#[derive(Debug, Clone)]
pub struct ResolveFileUrlRequest {
    /// Output-relative file name (`assets/logo-abc123.png`).
    pub file_name: String,
}

/// Chunk wrapper pieces from `banner` / `intro` / `outro` / `footer` hooks
/// (Rollup equivalents).
#[derive(Debug, Clone, Default)]
pub struct ChunkWrapper {
    /// Prepended before everything (e.g. license comments).
    pub banner: Option<String>,
    /// Prepended inside the chunk, after the banner.
    pub intro: Option<String>,
    /// Appended inside the chunk, before the footer.
    pub outro: Option<String>,
    /// Appended after everything.
    pub footer: Option<String>,
}

impl ChunkWrapper {
    /// True when no piece is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.banner.is_none()
            && self.intro.is_none()
            && self.outro.is_none()
            && self.footer.is_none()
    }

    /// Wrap `code` in the configured pieces.
    #[must_use]
    pub fn apply(&self, code: &str) -> String {
        if self.is_empty() {
            return code.to_string();
        }
        let mut out = String::with_capacity(code.len() + 256);
        for text in [&self.banner, &self.intro].into_iter().flatten() {
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
        out.push_str(code);
        if !code.ends_with('\n') {
            out.push('\n');
        }
        for text in [&self.outro, &self.footer].into_iter().flatten() {
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }
}

/// A dev/preview proxy rule: path prefix → target origin.
#[derive(Debug, Clone)]
pub struct ProxyRule {
    /// Path prefix (`/api`).
    pub prefix: String,
    /// Target origin (`http://localhost:3000`).
    pub target: String,
}

impl ProxyRule {
    /// True when `path` falls under this rule (segment-boundary match).
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        path == self.prefix || path.starts_with(&format!("{}/", self.prefix.trim_end_matches('/')))
    }

    /// Target URL for `path_and_query` (path preserved, Vite default).
    #[must_use]
    pub fn forward_url(&self, path_and_query: &str) -> String {
        format!("{}{path_and_query}", self.target.trim_end_matches('/'))
    }
}

/// An extra static directory mounted into the preview server.
#[derive(Debug, Clone)]
pub struct PreviewMount {
    /// URL prefix (`/docs`).
    pub prefix: String,
    /// Directory served there.
    pub dir: PathBuf,
}

/// Control surface for the `configure_preview` hook.
///
/// Collects plugin contributions (extra headers, static mounts, proxy
/// rules) that the preview server applies around its static + SPA-fallback
/// handler.
#[derive(Debug, Clone)]
pub struct PreviewControl {
    /// Resolved configuration.
    pub config: ResolvedConfig,
    /// Extra response headers for every preview response.
    pub headers: Vec<(String, String)>,
    /// Extra static mounts (checked before the output dir).
    pub mounts: Vec<PreviewMount>,
    /// Proxy rules (checked before static files).
    pub proxies: Vec<ProxyRule>,
}

impl PreviewControl {
    /// Empty control for `config` (seeded with `[server] proxy` rules).
    #[must_use]
    pub fn new(config: ResolvedConfig) -> Self {
        let mut proxies: Vec<ProxyRule> = config
            .server
            .proxy
            .iter()
            .map(|(prefix, target)| ProxyRule {
                prefix: prefix.clone(),
                target: target.clone(),
            })
            .collect();
        proxies.sort_by_key(|rule| Reverse(rule.prefix.len()));
        Self {
            config,
            headers: Vec::new(),
            mounts: Vec::new(),
            proxies,
        }
    }

    /// Add a response header.
    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.headers.push((name.into(), value.into()));
    }

    /// Mount `dir` at `prefix`.
    pub fn add_mount(&mut self, prefix: impl Into<String>, dir: PathBuf) {
        self.mounts.push(PreviewMount {
            prefix: prefix.into(),
            dir,
        });
    }

    /// Add a proxy rule (longest prefix wins at match time).
    pub fn add_proxy(&mut self, prefix: impl Into<String>, target: impl Into<String>) {
        self.proxies.push(ProxyRule {
            prefix: prefix.into(),
            target: target.into(),
        });
        self.proxies.sort_by_key(|rule| Reverse(rule.prefix.len()));
    }

    /// First matching proxy rule for `path`, if any.
    #[must_use]
    pub fn match_proxy(&self, path: &str) -> Option<&ProxyRule> {
        self.proxies.iter().find(|rule| rule.matches(path))
    }
}

impl ServerControl for PreviewControl {
    fn resolved_config(&self) -> &ResolvedConfig {
        &self.config
    }

    fn root(&self) -> &Path {
        &self.config.root
    }
}

/// Hook filter (§12): skip hooks cheaply before invoking them.
#[derive(Debug, Clone, Default)]
pub struct HookFilter {
    /// Only match these ids (regex).
    pub id: Option<String>,
    /// Only match when code matches (regex).
    pub code: Option<String>,
    /// Only match these queries (regex over the `?query` part).
    pub query: Option<String>,
}

impl HookFilter {
    /// True when the hook should run for (`id`, `code`).
    #[must_use]
    pub fn matches(&self, id: &str, code: Option<&str>) -> bool {
        if let Some(pattern) = &self.id {
            if !regex_match(pattern, id) {
                return false;
            }
        }
        if let Some(pattern) = &self.query {
            let query = id.split_once('?').map_or("", |(_, query)| query);
            if !regex_match(pattern, query) {
                return false;
            }
        }
        if let Some(pattern) = &self.code {
            if !code.is_some_and(|code| regex_match(pattern, code)) {
                return false;
            }
        }
        true
    }
}

pub(crate) fn regex_match(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|regex| regex.is_match(text))
}
