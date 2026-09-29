//! Configuration loading and resolution (spec §10).
//!
//! Precedence: CLI flags > `ferrite.local.toml` > `ferrite.toml` >
//! Cargo metadata > defaults.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ferrite_core::{Environment, EnvironmentKind, FerriteError, Result, Target};

mod js_config;

pub use js_config::{load_config_from_file, LoadedConfigFile};

/// Top-level user configuration (`ferrite.toml`).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct UserConfig {
    /// Project root (defaults to the config file directory).
    pub root: Option<String>,
    /// Public base path.
    pub base: Option<String>,
    /// Build mode (`development` / `production`).
    pub mode: Option<String>,
    /// Dev server options.
    pub server: ServerConfig,
    /// Build options.
    pub build: BuildConfig,
    /// SSR options.
    pub ssr: SsrConfig,
    /// Resolver options.
    pub resolve: ResolveConfig,
    /// npm options.
    pub npm: NpmConfig,
    /// Compiler options.
    pub compiler: CompilerConfig,
    /// Environment variable options.
    pub env: EnvConfig,
    /// Compile-time defines.
    pub define: HashMap<String, String>,
    /// Node compatibility layer options.
    pub node_compat: NodeCompatConfig,
    /// Remote import options.
    pub remote: RemoteConfig,
    /// Packaging options.
    pub package: PackageConfig,
    /// React plugin options.
    pub react: ReactConfig,
    /// Embedded runtime options.
    pub runtime: RuntimeConfig,
    /// End-to-end test options.
    pub e2e: E2eConfig,
}

/// Dev server options.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// Bind host.
    pub host: String,
    /// Bind port.
    pub port: u16,
    /// Fail instead of picking another port when busy.
    pub strict_port: bool,
    /// Open a browser on start.
    pub open: bool,
    /// Enable HMR.
    pub hmr: bool,
    /// Serve in middleware mode (no listener owned by Ferrite).
    pub middleware_mode: bool,
    /// Dev/preview proxy rules: path prefix → target origin
    /// (`/api` → `http://localhost:3000`), Vite `server.proxy` shorthand.
    pub proxy: HashMap<String, String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 5173,
            strict_port: false,
            open: false,
            hmr: true,
            middleware_mode: false,
            proxy: HashMap::new(),
        }
    }
}

/// Build options.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct BuildConfig {
    /// Output directory.
    pub out_dir: String,
    /// Emit source maps (`true` = external, plus `inline`/`hidden` handling).
    pub sourcemap: SourceMapConfig,
    /// Minify output.
    pub minify: bool,
    /// Compilation target.
    pub target: String,
    /// HTML entries.
    pub entries: Vec<String>,
    /// Library mode options.
    pub lib: Option<LibConfig>,
    /// Scope-hoist each entry closure into one file (§91).
    pub scope_hoist: bool,
}

/// Source map output mode (§41).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SourceMapConfig {
    /// Boolean shorthand (`true` = external).
    Bool(bool),
    /// Explicit mode.
    Mode(String),
}

impl Default for SourceMapConfig {
    fn default() -> Self {
        Self::Bool(true)
    }
}

impl SourceMapConfig {
    /// True unless explicitly disabled.
    #[must_use]
    pub fn enabled(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Mode(mode) => mode != "disabled",
        }
    }

    /// True for inline maps.
    #[must_use]
    pub fn inline(&self) -> bool {
        matches!(self, Self::Mode(mode) if mode == "inline")
    }

    /// True for hidden maps (emitted, no comment).
    #[must_use]
    pub fn hidden(&self) -> bool {
        matches!(self, Self::Mode(mode) if mode == "hidden")
    }
}

/// Library mode options (§55).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibConfig {
    /// Library entry.
    pub entry: String,
    /// Global name for UMD/IIFE.
    pub name: Option<String>,
    /// Output formats (`es`, `cjs`).
    pub formats: Vec<String>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            out_dir: "dist".to_string(),
            sourcemap: SourceMapConfig::default(),
            minify: true,
            target: "es2022".to_string(),
            entries: vec!["index.html".to_string()],
            lib: None,
            scope_hoist: false,
        }
    }
}

/// SSR options (§23).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SsrConfig {
    /// SSR entry.
    pub entry: Option<String>,
    /// Dependencies to leave external.
    pub external: Vec<String>,
    /// Dependencies to always bundle.
    pub no_external: Vec<String>,
    /// Bundle every dependency (§23 `bundle_all`).
    pub bundle_all: bool,
}

impl Default for SsrConfig {
    fn default() -> Self {
        Self {
            entry: Some("src/server.rs".to_string()),
            external: Vec::new(),
            no_external: Vec::new(),
            bundle_all: false,
        }
    }
}

/// Resolver options.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ResolveConfig {
    /// Export conditions.
    pub conditions: Vec<String>,
    /// File extensions to probe.
    pub extensions: Vec<String>,
    /// Import aliases (`@` -> `./src`).
    pub alias: HashMap<String, String>,
    /// Preserve symlinks instead of resolving them.
    pub preserve_symlinks: bool,
}

impl Default for ResolveConfig {
    fn default() -> Self {
        Self {
            conditions: ["browser", "module", "import"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            extensions: [".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            alias: HashMap::new(),
            preserve_symlinks: false,
        }
    }
}

/// npm options (§16).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NpmConfig {
    /// Registry base URL.
    pub registry: String,
    /// Lockfile name.
    pub lockfile: String,
    /// Dev serving strategy (`rewrite` or `import-map`).
    pub dev_strategy: String,
}

impl Default for NpmConfig {
    fn default() -> Self {
        Self {
            registry: "https://registry.npmjs.org".to_string(),
            lockfile: "ferrite.lock".to_string(),
            dev_strategy: "rewrite".to_string(),
        }
    }
}

/// Compiler options (§5).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct CompilerConfig {
    /// Engine name (`oxc` or `swc`).
    pub engine: String,
    /// Fallback engine options.
    pub fallback: Option<Box<CompilerConfig>>,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            engine: "oxc".to_string(),
            fallback: None,
        }
    }
}

/// Environment variable options (§42).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct EnvConfig {
    /// Prefixes exposed to the client.
    pub prefix: Vec<String>,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            prefix: vec!["FERRITE_".to_string(), "PUBLIC_".to_string()],
        }
    }
}

/// Node compatibility options (§19).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct NodeCompatConfig {
    /// Enable browser shims for `node:*` imports.
    pub enabled: bool,
    /// Shim mode (`browser-shims`).
    pub mode: String,
}

impl Default for NodeCompatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "browser-shims".to_string(),
        }
    }
}

/// Remote import options (§72).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RemoteConfig {
    /// Enable `https://` imports (disabled by default).
    pub enabled: bool,
    /// Allowed hosts.
    pub allow: Vec<String>,
}

/// Packaging options (§51).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PackageConfig {
    /// Produce a standalone executable.
    pub standalone: bool,
    /// Embed assets into the binary.
    pub embed_assets: bool,
    /// Compress embedded assets.
    pub compress_assets: bool,
    /// Cross-compilation triple (`cargo build --target`).
    pub target: Option<String>,
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            standalone: false,
            embed_assets: true,
            compress_assets: true,
            target: None,
        }
    }
}

/// React plugin options (§69).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ReactConfig {
    /// Enable React Refresh.
    pub refresh: bool,
    /// JSX runtime (`automatic` or `classic`).
    pub runtime: String,
}

impl Default for ReactConfig {
    fn default() -> Self {
        Self {
            refresh: true,
            runtime: "automatic".to_string(),
        }
    }
}

/// Embedded JS runtime options (§20).
///
/// The runtime is **disabled by default**: `backend = "auto"` means pure-Rust
/// SSR with no JS engine. Set `backend = "napi-vm"` (and build with
/// `--features napi-vm`) for in-process JS evaluation and `.node` loading.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Backend name (`auto`, `none`, `napi-vm`).
    pub backend: String,
    /// Guest fuel budget (0 = engine default).
    pub fuel_budget: u64,
    /// Guest loop budget (0 = engine default).
    pub loop_budget: u64,
    /// Allowlisted `.node` paths (root-relative or absolute).
    pub native_allow: Vec<String>,
    /// Expected SHA-256 (hex) per allowlisted path.
    pub native_integrity: HashMap<String, String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            fuel_budget: 0,
            loop_budget: 0,
            native_allow: Vec::new(),
            native_integrity: HashMap::new(),
        }
    }
}

/// End-to-end test options (`ferrite e2e`, Chromium-first).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct E2eConfig {
    /// Browser engine (`chromium` or `firefox`).
    pub browser: String,
    /// Launch the browser headless.
    pub headless: bool,
    /// Explicit browser executable path (overrides auto-detection).
    pub executable_path: Option<String>,
    /// Extra browser CLI args.
    pub args: Vec<String>,
    /// Browser-wide user agent override.
    pub user_agent: Option<String>,
    /// Browser-wide proxy (`host:port`, `http(s)://…`, `socks5://…`).
    pub proxy_server: Option<String>,
    /// Accept insecure TLS certificates session-wide.
    pub ignore_https_errors: bool,
    /// Base URL for relative navigations (`page.goto("/")`).
    /// Defaults to the booted dev server or `FERRITE_E2E_BASE_URL`.
    pub base_url: Option<String>,
    /// Per-test timeout in milliseconds.
    pub timeout_ms: u64,
    /// Default assertion retry window in milliseconds.
    pub expect_timeout_ms: u64,
    /// Retries per test after the first attempt.
    pub retries: u32,
    /// Parallel test workers.
    pub workers: usize,
    /// Reporter (`list`, `json`, `junit`, comma-separated).
    pub reporter: String,
    /// Artifact directory (screenshots, traces, reports).
    pub output_dir: String,
    /// Screenshot policy (`on`, `off`, `only-on-failure`).
    pub screenshot: String,
    /// Video policy (`on`, `off`, `only-on-failure`).
    pub video: String,
    /// Recording frames per second.
    pub video_fps: u32,
    /// Slow down each action by this many milliseconds.
    pub slow_mo_ms: u64,
    /// Default viewport.
    pub viewport: Option<ViewportConfig>,
    /// Web server to boot before tests (dev server by default).
    pub web_server: Option<WebServerConfig>,
}

impl Default for E2eConfig {
    fn default() -> Self {
        Self {
            browser: "chromium".to_string(),
            headless: true,
            executable_path: None,
            args: Vec::new(),
            user_agent: None,
            proxy_server: None,
            ignore_https_errors: false,
            base_url: None,
            timeout_ms: 30_000,
            expect_timeout_ms: 5_000,
            retries: 0,
            workers: 4,
            reporter: "list".to_string(),
            output_dir: "test-results".to_string(),
            screenshot: "only-on-failure".to_string(),
            video: "off".to_string(),
            video_fps: 10,
            slow_mo_ms: 0,
            viewport: None,
            web_server: None,
        }
    }
}

impl E2eConfig {
    /// True for Chromium (`chromium` / `chrome`).
    #[must_use]
    pub fn is_chromium(&self) -> bool {
        self.browser == "chromium" || self.browser == "chrome"
    }

    /// True for Firefox (`firefox` / `ff`).
    #[must_use]
    pub fn is_firefox(&self) -> bool {
        self.browser == "firefox" || self.browser == "ff"
    }

    /// True when failures must capture a screenshot.
    #[must_use]
    pub fn screenshot_on_failure(&self) -> bool {
        self.screenshot == "only-on-failure" || self.screenshot == "on"
    }

    /// True when every test captures a screenshot.
    #[must_use]
    pub fn screenshot_always(&self) -> bool {
        self.screenshot == "on"
    }
}

/// Default viewport size.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewportConfig {
    /// Viewport width in CSS pixels.
    pub width: u32,
    /// Viewport height in CSS pixels.
    pub height: u32,
}

/// Web server lifecycle for e2e runs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct WebServerConfig {
    /// Command to run (`ferrite dev` equivalent when unset).
    pub command: Option<String>,
    /// URL to wait for before running tests.
    pub url: Option<String>,
    /// Readiness timeout in milliseconds.
    pub timeout_ms: u64,
    /// Reuse an already-listening server instead of booting.
    pub reuse_existing: bool,
}

impl Default for WebServerConfig {
    fn default() -> Self {
        Self {
            command: None,
            url: None,
            timeout_ms: 60_000,
            reuse_existing: true,
        }
    }
}

/// Fully resolved configuration.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    /// Project root (absolute).
    pub root: PathBuf,
    /// Public base path.
    pub base: String,
    /// Build mode.
    pub mode: String,
    /// True in production mode.
    pub is_production: bool,
    /// Server options.
    pub server: ServerConfig,
    /// Build options.
    pub build: BuildConfig,
    /// SSR options.
    pub ssr: SsrConfig,
    /// Resolver options.
    pub resolve: ResolveConfig,
    /// npm options.
    pub npm: NpmConfig,
    /// Compiler options.
    pub compiler: CompilerConfig,
    /// Env options.
    pub env: EnvConfig,
    /// Defines.
    pub define: HashMap<String, String>,
    /// Node compat options.
    pub node_compat: NodeCompatConfig,
    /// Remote options.
    pub remote: RemoteConfig,
    /// Package options.
    pub package: PackageConfig,
    /// React options.
    pub react: ReactConfig,
    /// Runtime options.
    pub runtime: RuntimeConfig,
    /// E2E options.
    pub e2e: E2eConfig,
    /// Configured environments.
    pub environments: HashMap<String, Environment>,
}

impl ResolvedConfig {
    /// Default client environment.
    #[must_use]
    pub fn client_env(&self) -> Environment {
        self.environments.get("client").cloned().unwrap_or_else(|| {
            let mut env = Environment::new("client", EnvironmentKind::Client);
            env.target = self.target();
            env.define.clone_from(&self.define);
            env
        })
    }

    /// Default SSR environment.
    #[must_use]
    pub fn ssr_env(&self) -> Environment {
        self.environments.get("ssr").cloned().unwrap_or_else(|| {
            let mut env = Environment::new("ssr", EnvironmentKind::Ssr);
            env.target = self.target();
            env.define.clone_from(&self.define);
            env
        })
    }

    /// Parsed build target.
    #[must_use]
    pub fn target(&self) -> Target {
        self.build.target.parse().unwrap_or_default()
    }

    /// Absolute output directory.
    #[must_use]
    pub fn out_dir(&self) -> PathBuf {
        self.root.join(&self.build.out_dir)
    }

    /// Absolute lockfile path.
    #[must_use]
    pub fn lockfile(&self) -> PathBuf {
        self.root.join(&self.npm.lockfile)
    }
}

/// CLI-level overrides applied on top of file configuration.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// Override root.
    pub root: Option<PathBuf>,
    /// Override mode.
    pub mode: Option<String>,
    /// Override host.
    pub host: Option<String>,
    /// Override port.
    pub port: Option<u16>,
    /// Override base.
    pub base: Option<String>,
    /// Override out dir.
    pub out_dir: Option<String>,
    /// Override minify.
    pub minify: Option<bool>,
    /// Override standalone.
    pub standalone: Option<bool>,
    /// Override the cross-compilation target.
    pub target: Option<String>,
    /// Override scope hoisting.
    pub scope_hoist: Option<bool>,
    /// Override runtime backend.
    pub runtime: Option<String>,
}

/// Load `ferrite.toml` + `ferrite.local.toml` from `dir` (both optional).
///
/// When neither TOML file exists, falls back to a statically parsed
/// `ferrite.config.*` / `vite.config.*` (see [`load_config_from_file`]);
/// TOML always wins when both exist. JS-config warnings are dropped here —
/// call [`load_config_from_file`] directly to surface them.
pub fn load_user_config(dir: &Path) -> Result<UserConfig> {
    let mut merged = UserConfig::default();
    let mut found_toml = false;
    for file in ["ferrite.toml", "ferrite.local.toml"] {
        let path = dir.join(file);
        if path.exists() {
            found_toml = true;
            let text = std::fs::read_to_string(&path)?;
            let parsed: UserConfig = toml::from_str(&text)
                .map_err(|error| FerriteError::Config(format!("{file}: {error}")))?;
            merged = merge_user_config(merged, parsed);
        }
    }
    if !found_toml {
        if let Some(js) = load_config_from_file(dir)? {
            merged = merge_user_config(merged, js.config);
        }
    }
    Ok(merged)
}

/// Merge `over` on top of `base` (field-wise; `over` wins when set).
#[must_use]
pub fn merge_user_config(mut base: UserConfig, over: UserConfig) -> UserConfig {
    if over.root.is_some() {
        base.root = over.root;
    }
    if over.base.is_some() {
        base.base = over.base;
    }
    if over.mode.is_some() {
        base.mode = over.mode;
    }
    base.server = merge_server(base.server, over.server);
    base.build = merge_build(base.build, over.build);
    base.ssr = merge_ssr(base.ssr, over.ssr);
    base.resolve = merge_resolve(base.resolve, over.resolve);
    base.npm = merge_npm(base.npm, over.npm);
    if over.compiler.engine != "oxc" || over.compiler.fallback.is_some() {
        base.compiler = over.compiler;
    }
    if !over.env.prefix.is_empty() {
        base.env = over.env;
    }
    base.define.extend(over.define);
    base.node_compat = over.node_compat;
    base.remote = over.remote;
    base.package = over.package;
    base.react = over.react;
    base.runtime = merge_runtime(base.runtime, over.runtime);
    base.e2e = merge_e2e(base.e2e, over.e2e);
    base
}

fn merge_e2e(mut base: E2eConfig, over: E2eConfig) -> E2eConfig {
    let defaults = E2eConfig::default();
    if over.browser != defaults.browser {
        base.browser = over.browser;
    }
    if over.headless != defaults.headless {
        base.headless = over.headless;
    }
    if over.executable_path.is_some() {
        base.executable_path = over.executable_path;
    }
    if !over.args.is_empty() {
        base.args = over.args;
    }
    if over.user_agent.is_some() {
        base.user_agent = over.user_agent;
    }
    if over.proxy_server.is_some() {
        base.proxy_server = over.proxy_server;
    }
    if over.ignore_https_errors {
        base.ignore_https_errors = true;
    }
    if over.base_url.is_some() {
        base.base_url = over.base_url;
    }
    if over.timeout_ms != defaults.timeout_ms {
        base.timeout_ms = over.timeout_ms;
    }
    if over.expect_timeout_ms != defaults.expect_timeout_ms {
        base.expect_timeout_ms = over.expect_timeout_ms;
    }
    if over.retries != defaults.retries {
        base.retries = over.retries;
    }
    if over.workers != defaults.workers {
        base.workers = over.workers;
    }
    if over.reporter != defaults.reporter {
        base.reporter = over.reporter;
    }
    if over.output_dir != defaults.output_dir {
        base.output_dir = over.output_dir;
    }
    if over.screenshot != defaults.screenshot {
        base.screenshot = over.screenshot;
    }
    if over.video != defaults.video {
        base.video = over.video;
    }
    if over.video_fps != defaults.video_fps {
        base.video_fps = over.video_fps;
    }
    if over.slow_mo_ms != defaults.slow_mo_ms {
        base.slow_mo_ms = over.slow_mo_ms;
    }
    if over.viewport.is_some() {
        base.viewport = over.viewport;
    }
    if over.web_server.is_some() {
        base.web_server = over.web_server;
    }
    base
}

fn merge_runtime(mut base: RuntimeConfig, over: RuntimeConfig) -> RuntimeConfig {
    let defaults = RuntimeConfig::default();
    if over.backend != defaults.backend {
        base.backend = over.backend;
    }
    if over.fuel_budget != 0 {
        base.fuel_budget = over.fuel_budget;
    }
    if over.loop_budget != 0 {
        base.loop_budget = over.loop_budget;
    }
    if !over.native_allow.is_empty() {
        base.native_allow = over.native_allow;
    }
    if !over.native_integrity.is_empty() {
        base.native_integrity = over.native_integrity;
    }
    base
}

fn merge_server(mut base: ServerConfig, over: ServerConfig) -> ServerConfig {
    let defaults = ServerConfig::default();
    if over.host != defaults.host {
        base.host = over.host;
    }
    if over.port != defaults.port {
        base.port = over.port;
    }
    base.strict_port |= over.strict_port;
    base.open |= over.open;
    base.hmr &= over.hmr;
    base.middleware_mode |= over.middleware_mode;
    base.proxy.extend(over.proxy);
    base
}

fn merge_build(mut base: BuildConfig, over: BuildConfig) -> BuildConfig {
    let defaults = BuildConfig::default();
    if over.out_dir != defaults.out_dir {
        base.out_dir = over.out_dir;
    }
    if over.minify != defaults.minify {
        base.minify = over.minify;
    }
    if over.target != defaults.target {
        base.target = over.target;
    }
    if over.lib.is_some() {
        base.lib = over.lib;
    }
    if over.scope_hoist {
        base.scope_hoist = true;
    }
    if over.entries != defaults.entries {
        base.entries = over.entries;
    }
    base.sourcemap = over.sourcemap;
    base
}

fn merge_ssr(mut base: SsrConfig, over: SsrConfig) -> SsrConfig {
    if over.entry.is_some() {
        base.entry = over.entry;
    }
    if !over.external.is_empty() {
        base.external = over.external;
    }
    if !over.no_external.is_empty() {
        base.no_external = over.no_external;
    }
    base.bundle_all |= over.bundle_all;
    base
}

fn merge_resolve(mut base: ResolveConfig, over: ResolveConfig) -> ResolveConfig {
    let defaults = ResolveConfig::default();
    if over.conditions != defaults.conditions {
        base.conditions = over.conditions;
    }
    if over.extensions != defaults.extensions {
        base.extensions = over.extensions;
    }
    base.alias.extend(over.alias);
    base.preserve_symlinks |= over.preserve_symlinks;
    base
}

fn merge_npm(mut base: NpmConfig, over: NpmConfig) -> NpmConfig {
    let defaults = NpmConfig::default();
    if over.registry != defaults.registry {
        base.registry = over.registry;
    }
    if over.lockfile != defaults.lockfile {
        base.lockfile = over.lockfile;
    }
    if over.dev_strategy != defaults.dev_strategy {
        base.dev_strategy = over.dev_strategy;
    }
    base
}

/// Identity helper, the `defineConfig` equivalent: pins the programmatic
/// config type so a misplaced field fails to compile at the call site.
///
/// ```rust
/// use ferrite_config::{define_config, UserConfig};
/// let config = define_config(UserConfig {
///     base: Some("/app/".to_string()),
///     ..Default::default()
/// });
/// ```
#[must_use]
pub fn define_config(config: UserConfig) -> UserConfig {
    config
}

/// Deep-merge two user configs, the `mergeConfig` equivalent.
///
/// Objects merge per key, `over` winning on conflict; maps (`alias`,
/// `define`, `proxy`) merge per entry; `env.prefix` concatenates and
/// dedupes; scalars and remaining arrays take `over` when it differs from
/// the default.
#[must_use]
pub fn merge_config(base: UserConfig, over: UserConfig) -> UserConfig {
    let mut merged = merge_user_config(base.clone(), over.clone());
    // `merge_user_config` replaces prefixes; `mergeConfig` unions them.
    if !over.env.prefix.is_empty() && !base.env.prefix.is_empty() {
        let mut prefixes = base.env.prefix.clone();
        for prefix in &over.env.prefix {
            if !prefixes.contains(prefix) {
                prefixes.push(prefix.clone());
            }
        }
        merged.env.prefix = prefixes;
    }
    merged
}

/// Resolve a user config into a concrete config.
pub fn resolve_config(
    user: UserConfig,
    root_hint: Option<PathBuf>,
    overrides: CliOverrides,
) -> Result<ResolvedConfig> {
    let mut root = root_hint
        .or_else(|| user.root.clone().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    if let Some(root_override) = &overrides.root {
        root = root_override.clone();
    }
    let root = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(root)
    };
    let mode = overrides
        .mode
        .or(user.mode.clone())
        .unwrap_or_else(|| "development".to_string());
    let is_production = mode == "production";
    let mut server = user.server.clone();
    if let Some(host) = overrides.host {
        server.host = host;
    }
    if let Some(port) = overrides.port {
        server.port = port;
    }
    let mut build = user.build.clone();
    if let Some(out_dir) = overrides.out_dir {
        build.out_dir = out_dir;
    }
    if let Some(minify) = overrides.minify {
        build.minify = minify;
    }
    let mut package = user.package.clone();
    if let Some(standalone) = overrides.standalone {
        package.standalone = standalone;
    }
    if let Some(target) = overrides.target {
        package.target = Some(target);
    }
    if let Some(scope_hoist) = overrides.scope_hoist {
        build.scope_hoist = scope_hoist;
    }
    let base = overrides
        .base
        .or(user.base.clone())
        .unwrap_or_else(|| "/".to_string());

    let mut environments = HashMap::new();
    let mut client = Environment::new("client", EnvironmentKind::Client);
    client.target = build.target.parse().unwrap_or_default();
    client.define.clone_from(&user.define);
    let mut ssr = Environment::new("ssr", EnvironmentKind::Ssr);
    ssr.target = client.target.clone();
    ssr.define.clone_from(&user.define);
    environments.insert("client".to_string(), client);
    environments.insert("ssr".to_string(), ssr);

    Ok(ResolvedConfig {
        root,
        base,
        mode,
        is_production,
        server,
        build,
        ssr: user.ssr.clone(),
        resolve: user.resolve.clone(),
        npm: user.npm.clone(),
        compiler: user.compiler.clone(),
        env: user.env.clone(),
        define: user.define.clone(),
        node_compat: user.node_compat.clone(),
        remote: user.remote.clone(),
        package,
        react: user.react.clone(),
        runtime: {
            let mut runtime = user.runtime.clone();
            if let Some(backend) = overrides.runtime {
                runtime.backend = backend;
            }
            runtime
        },
        e2e: user.e2e.clone(),
        environments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_resolve() {
        let config = resolve_config(
            UserConfig::default(),
            Some(PathBuf::from("/tmp/x")),
            CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(config.server.port, 5173);
        assert_eq!(config.base, "/");
        assert!(!config.is_production);
    }

    #[test]
    fn local_overrides_win() {
        let base = UserConfig::default();
        let mut over = UserConfig::default();
        over.server.port = 3000;
        let merged = merge_user_config(base, over);
        assert_eq!(merged.server.port, 3000);
    }

    #[test]
    fn runtime_toml_shape_parses() {
        let user: UserConfig = toml::from_str(
            "[runtime]\n\
             backend = \"napi-vm\"\n\
             fuel_budget = 10000000\n\
             native_allow = [\"native/addon.node\"]\n\
             [runtime.native_integrity]\n\
             \"native/addon.node\" = \"ab12\"\n",
        )
        .unwrap();
        assert_eq!(user.runtime.backend, "napi-vm");
        assert_eq!(user.runtime.fuel_budget, 10_000_000);
        assert_eq!(user.runtime.native_allow, vec!["native/addon.node"]);
        assert_eq!(
            user.runtime
                .native_integrity
                .get("native/addon.node")
                .unwrap(),
            "ab12"
        );
        // Disabled by default.
        assert_eq!(UserConfig::default().runtime.backend, "auto");
    }

    #[test]
    fn runtime_merge_and_cli_override() {
        let base = UserConfig::default();
        let mut over = UserConfig::default();
        over.runtime.backend = "napi-vm".to_string();
        over.runtime.loop_budget = 5;
        let merged = merge_user_config(base, over);
        assert_eq!(merged.runtime.backend, "napi-vm");
        assert_eq!(merged.runtime.loop_budget, 5);

        let resolved = resolve_config(
            UserConfig::default(),
            Some(PathBuf::from("/tmp/x")),
            CliOverrides {
                runtime: Some("none".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.runtime.backend, "none");
    }

    #[test]
    fn merge_config_unions_prefixes_and_proxies() {
        let mut base = UserConfig::default();
        base.server.proxy.insert("/a".to_string(), "http://a".to_string());
        let mut over = UserConfig::default();
        over.env.prefix = vec!["APP_".to_string()];
        over.server.proxy.insert("/b".to_string(), "http://b".to_string());
        let merged = merge_config(base, over);
        assert!(merged.env.prefix.contains(&"APP_".to_string()));
        assert!(merged.env.prefix.contains(&"FERRITE_".to_string()));
        assert_eq!(merged.server.proxy.len(), 2);
    }

    #[test]
    fn e2e_toml_shape_parses() {
        let user: UserConfig = toml::from_str(
            "[e2e]\n\
             browser = \"chromium\"\n\
             headless = false\n\
             retries = 2\n\
             workers = 8\n\
             reporter = \"list,json\"\n\
             video = \"only-on-failure\"\n\
             video_fps = 15\n\
             [e2e.viewport]\n\
             width = 1280\n\
             height = 720\n\
             [e2e.web_server]\n\
             command = \"ferrite dev --port 5190\"\n\
             url = \"http://127.0.0.1:5190/\"\n",
        )
        .unwrap();
        assert!(!user.e2e.headless);
        assert_eq!(user.e2e.retries, 2);
        assert_eq!(user.e2e.workers, 8);
        assert_eq!(user.e2e.reporter, "list,json");
        assert_eq!(user.e2e.video, "only-on-failure");
        assert_eq!(user.e2e.video_fps, 15);
        assert_eq!(user.e2e.viewport.as_ref().unwrap().width, 1280);
        let server = user.e2e.web_server.as_ref().unwrap();
        assert_eq!(server.url.as_deref(), Some("http://127.0.0.1:5190/"));
        assert!(server.reuse_existing);
        // Defaults stay headless with failure screenshots and no video.
        let defaults = E2eConfig::default();
        assert!(defaults.headless);
        assert!(defaults.screenshot_on_failure());
        assert!(!defaults.screenshot_always());
        assert_eq!(defaults.video, "off");
        assert_eq!(defaults.video_fps, 10);
    }

    #[test]
    fn e2e_merge_layering() {
        let base = UserConfig::default();
        let mut over = UserConfig::default();
        over.e2e.retries = 3;
        over.e2e.headless = false;
        let merged = merge_user_config(base, over);
        assert_eq!(merged.e2e.retries, 3);
        assert!(!merged.e2e.headless);
        assert_eq!(merged.e2e.workers, E2eConfig::default().workers);
    }

    #[test]
    fn define_config_is_identity() {
        let user = UserConfig {
            base: Some("/x/".to_string()),
            ..Default::default()
        };
        assert_eq!(define_config(user.clone()).base, user.base);
    }

    #[test]
    fn toml_wins_over_js_config() {
        let dir = std::env::temp_dir().join(format!("ferrite-cfgprec-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("vite.config.js"),
            "export default { server: { port: 1111 } };",
        )
        .unwrap();
        // No TOML: JS config applies.
        let user = load_user_config(&dir).unwrap();
        assert_eq!(user.server.port, 1111);
        // TOML present: TOML wins, JS ignored.
        std::fs::write(dir.join("ferrite.toml"), "[server]\nport = 2222\n").unwrap();
        let user = load_user_config(&dir).unwrap();
        assert_eq!(user.server.port, 2222);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
