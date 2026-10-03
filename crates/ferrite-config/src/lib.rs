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
    /// Explicit official framework compiler configuration, separate from SSR runtime.
    pub framework: Option<FrameworkConfig>,
    /// Explicit foreign hook profiles. Some(empty) disables inherited profiles.
    pub foreign_plugins: Option<Vec<ForeignPluginConfig>>,
    /// Embedded runtime options.
    pub runtime: RuntimeConfig,
    /// End-to-end test options.
    pub e2e: E2eConfig,
}

/// Explicit local Node hook profile; separate from compiler and SSR hosts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ForeignPluginConfig {
    pub name: String,
    pub entry: PathBuf,
    pub host: Option<String>,
    pub node: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub options: serde_json::Value,
}
impl Default for ForeignPluginConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            entry: PathBuf::new(),
            host: None,
            node: None,
            timeout_ms: None,
            options: serde_json::json!({}),
        }
    }
}

/// Explicit opt-in to official framework compilation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FrameworkConfig {
    /// Component owners to enable (`react`, `vue`, `svelte`). An empty list disables this integration.
    pub enabled: Vec<String>,
    /// Compiler host: native React or explicitly selected Node for Vue/Svelte.
    pub compiler_host: Option<String>,
    /// Optional Node executable; never used for SSR runtime selection.
    pub node: Option<PathBuf>,
    /// Per compiler request deadline in milliseconds.
    pub timeout_ms: u64,
}
impl Default for FrameworkConfig {
    fn default() -> Self {
        Self {
            enabled: Vec::new(),
            compiler_host: None,
            node: None,
            timeout_ms: 10_000,
        }
    }
}
impl FrameworkConfig {
    /// Reject unavailable profiles before starting a worker or writing projections.
    pub fn validate(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for name in &self.enabled {
            if !matches!(name.as_str(), "react" | "vue" | "svelte") {
                return Err(FerriteError::Config(format!("framework `{name}` has no configured compiler adapter; available: react (native), vue, svelte (node)")));
            }
            if !seen.insert(name) {
                return Err(FerriteError::Config(format!(
                    "framework `{name}` is configured twice"
                )));
            }
        }
        if self.enabled.iter().any(|name| name == "react") {
            if self.enabled.len() != 1 || self.compiler_host.as_deref() != Some("native") {
                return Err(FerriteError::Config("React requires framework.compiler_host = \"native\"; mixed native/Node framework profiles are unavailable in one framework configuration; SSR runtime is separate".into()));
            }
        } else if !self.enabled.is_empty() && self.compiler_host.as_deref() != Some("node") {
            return Err(FerriteError::Config("official framework compilation requires explicit framework.compiler_host = \"node\"; native and embedded compiler profiles are unavailable; SSR runtime is configured separately".into()));
        }
        if self.timeout_ms == 0 || self.timeout_ms > 300_000 {
            return Err(FerriteError::Config(
                "framework.timeout_ms must be between 1 and 300000".into(),
            ));
        }
        Ok(())
    }
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

/// React plugin options (§69). Setters record explicit resets to default values.
#[derive(Debug, Clone)]
pub struct ReactConfig {
    /// Enable React Refresh.
    pub refresh: bool,
    /// JSX runtime (`automatic` or `classic`).
    pub runtime: String,
    /// Automatic JSX runtime package (defaults to React).
    pub import_source: Option<String>,
    /// Classic JSX element factory.
    pub factory: Option<String>,
    /// Classic JSX fragment factory.
    pub fragment: Option<String>,
    explicit_runtime: bool,
    explicit_refresh: bool,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReactConfigFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    import_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    factory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fragment: Option<String>,
}
impl<'de> serde::Deserialize<'de> for ReactConfig {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let fields = ReactConfigFields::deserialize(deserializer)?;
        Ok(Self {
            explicit_runtime: fields.runtime.is_some(),
            explicit_refresh: fields.refresh.is_some(),
            runtime: fields.runtime.unwrap_or_else(|| "automatic".into()),
            refresh: fields.refresh.unwrap_or(true),
            import_source: fields.import_source,
            factory: fields.factory,
            fragment: fields.fragment,
        })
    }
}
impl serde::Serialize for ReactConfig {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        ReactConfigFields {
            runtime: (self.explicit_runtime || self.runtime != "automatic")
                .then(|| self.runtime.clone()),
            refresh: (self.explicit_refresh || !self.refresh).then_some(self.refresh),
            import_source: self.import_source.clone(),
            factory: self.factory.clone(),
            fragment: self.fragment.clone(),
        }
        .serialize(serializer)
    }
}
impl ReactConfig {
    /// Select a runtime explicitly, including resetting an inherited classic runtime.
    pub fn set_runtime(&mut self, runtime: impl Into<String>) {
        self.runtime = runtime.into();
        self.explicit_runtime = true;
    }
    /// Select Refresh explicitly, including re-enabling an inherited disabled setting.
    pub fn set_refresh(&mut self, refresh: bool) {
        self.refresh = refresh;
        self.explicit_refresh = true;
    }
}
impl Default for ReactConfig {
    fn default() -> Self {
        Self {
            refresh: true,
            runtime: "automatic".into(),
            import_source: None,
            factory: None,
            fragment: None,
            explicit_runtime: false,
            explicit_refresh: false,
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
    /// Maximum queued embedded-worker jobs (0 selects 64).
    pub queue_capacity: usize,
    /// Maximum encoded embedded-worker request bytes (0 selects 8 MiB).
    pub max_request_bytes: usize,
    /// Guest call-depth cap (0 selects the engine default).
    pub max_call_depth: usize,
    /// Guest jobs per drain cap (0 selects the engine default).
    pub max_jobs_per_drain: usize,
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
            queue_capacity: 0,
            max_request_bytes: 0,
            max_call_depth: 0,
            max_jobs_per_drain: 0,
            fuel_budget: 0,
            loop_budget: 0,
            native_allow: Vec::new(),
            native_integrity: HashMap::new(),
        }
    }
}

/// JSON-safe metadata with deterministic key serialization.
pub type E2eMetadata = std::collections::BTreeMap<String, serde_json::Value>;

/// Bounded slow-result reporting. Zero maximum disables reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SlowTestOptions {
    pub threshold_ms: u64,
    pub max: usize,
}
impl Default for SlowTestOptions {
    fn default() -> Self {
        Self {
            threshold_ms: 300_000,
            max: 5,
        }
    }
}
impl SlowTestOptions {
    pub fn validate(self) -> std::result::Result<(), String> {
        if self.max > 1_000 {
            return Err("slow-test maximum cannot exceed 1000".into());
        }
        Ok(())
    }
}

/// Serializable project settings for the E2E CLI/library bridge.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct E2eProjectConfig {
    pub name: String,
    pub metadata: Option<E2eMetadata>,
    pub browser: Option<String>,
    pub headless: Option<bool>,
    pub executable_path: Option<String>,
    pub args: Option<Vec<String>>,
    pub viewport: Option<ViewportConfig>,
    pub grep: Option<String>,
    pub grep_invert: Option<String>,
    pub retries: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub repeat_each: Option<u32>,
    pub output_dir: Option<String>,
    pub snapshot_dir: Option<String>,
    /// Template for snapshot baseline paths; inherited by projects when unset.
    pub snapshot_path_template: Option<String>,
}

/// End-to-end test options (`ferrite e2e`, Chromium-first).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct E2eConfig {
    pub run_name: Option<String>,
    pub metadata: Option<E2eMetadata>,
    /// None preserves the legacy disabled summary; Some(max=0) disables explicitly.
    pub report_slow_tests: Option<SlowTestOptions>,
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
    /// Download directory (unset = managed directory in the browser profile).
    pub download_dir: Option<String>,
    /// Base URL for relative navigations (`page.goto("/")`).
    /// Defaults to the booted dev server or `FERRITE_E2E_BASE_URL`.
    pub base_url: Option<String>,
    /// Per-test timeout in milliseconds.
    pub timeout_ms: u64,
    /// Whole-run timeout (zero disables it).
    pub global_timeout_ms: u64,
    /// Stop scheduling after this many unexpected failures (zero disables it).
    pub max_failures: usize,
    /// Fail the aggregate run when retries recover an unexpected attempt.
    pub fail_on_flaky_tests: bool,
    /// Reject registered test/suite focus before scheduling. CI also enables it.
    pub forbid_only: bool,
    /// Shared timeout per attempt, worker retirement or run-final cleanup scope
    /// (zero disables it; local fixture limits cannot extend it).
    pub cleanup_timeout_ms: u64,
    /// Default assertion retry window in milliseconds.
    pub expect_timeout_ms: u64,
    /// Retries per test after the first attempt.
    pub retries: u32,
    /// Parallel test workers.
    pub workers: usize,
    /// Reporter (`list`, `json`, `junit`, `html`, comma-separated).
    pub reporter: String,
    /// Artifact directory (screenshots, traces, reports).
    pub output_dir: String,
    /// Runner-owned attempt output retention: always, never or failures-only.
    pub preserve_output: String,
    /// Snapshot baseline directory; unset uses output_dir/snapshots.
    pub snapshot_dir: Option<String>,
    /// Template for snapshot baseline paths; inherited by projects when unset.
    pub snapshot_path_template: Option<String>,
    /// Repetitions per selected test (zero normalizes to one).
    pub repeat_each: u32,
    /// Name-or-tag substring filters.
    pub filter: Option<String>,
    pub grep: Option<String>,
    pub grep_invert: Option<String>,
    /// One-based shard index and count.
    pub shard: Option<(usize, usize)>,
    /// Named project settings and optional selection.
    pub projects: Vec<E2eProjectConfig>,
    pub selected_projects: Vec<String>,
    /// Screenshot policy (`on`, `off`, `only-on-failure`).
    pub screenshot: String,
    /// Video policy (`on`, `off`, `only-on-failure`).
    pub video: String,
    /// Recording frames per second.
    pub video_fps: u32,
    /// Slow down each action by this many milliseconds.
    pub slow_mo_ms: u64,
    /// Snapshot update mode (`missing`, `changed`, `all`, `none`).
    pub update_snapshots: String,
    /// Default viewport.
    pub viewport: Option<ViewportConfig>,
    /// Web server to boot before tests (dev server by default).
    pub web_server: Option<WebServerConfig>,
}

impl Default for E2eConfig {
    fn default() -> Self {
        Self {
            run_name: None,
            metadata: None,
            report_slow_tests: None,
            browser: "chromium".to_string(),
            headless: true,
            executable_path: None,
            args: Vec::new(),
            user_agent: None,
            proxy_server: None,
            ignore_https_errors: false,
            download_dir: None,
            base_url: None,
            timeout_ms: 30_000,
            global_timeout_ms: 0,
            max_failures: 0,
            fail_on_flaky_tests: false,
            forbid_only: false,
            cleanup_timeout_ms: 5_000,
            expect_timeout_ms: 5_000,
            retries: 0,
            workers: 4,
            reporter: "list".to_string(),
            output_dir: "test-results".to_string(),
            preserve_output: "always".to_string(),
            snapshot_dir: None,
            snapshot_path_template: None,
            repeat_each: 1,
            filter: None,
            grep: None,
            grep_invert: None,
            shard: None,
            projects: Vec::new(),
            selected_projects: Vec::new(),
            screenshot: "only-on-failure".to_string(),
            video: "off".to_string(),
            video_fps: 10,
            slow_mo_ms: 0,
            update_snapshots: "missing".to_string(),
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
    /// Explicit framework compiler options.
    pub framework: Option<FrameworkConfig>,
    /// Explicit foreign hook profiles.
    pub foreign_plugins: Option<Vec<ForeignPluginConfig>>,
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
        let mut environment = self.environments.get("client").cloned().unwrap_or_else(|| {
            let mut env = Environment::new("client", EnvironmentKind::Client);
            env.target = self.target();
            env.define.clone_from(&self.define);
            env
        });
        environment
            .define
            .entry("process.env.NODE_ENV".into())
            .or_insert_with(|| {
                if self.is_production {
                    "\"production\"".into()
                } else {
                    "\"development\"".into()
                }
            });
        environment
    }

    /// Default SSR environment.
    #[must_use]
    pub fn ssr_env(&self) -> Environment {
        let mut environment = self.environments.get("ssr").cloned().unwrap_or_else(|| {
            let mut env = Environment::new("ssr", EnvironmentKind::Ssr);
            env.target = self.target();
            env.define.clone_from(&self.define);
            env
        });
        environment
            .define
            .entry("process.env.NODE_ENV".into())
            .or_insert_with(|| {
                if self.is_production {
                    "\"production\"".into()
                } else {
                    "\"development\"".into()
                }
            });
        environment
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
    /// Command default used only when no explicit mode is selected.
    pub default_mode: Option<String>,
    /// Compilation behavior, separate from the environment-file mode name.
    pub is_production: Option<bool>,
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

/// Load an explicitly selected directory or config file, without falling back
/// to a different file. Explicit files do not merge implicit local overrides.
pub fn load_user_config_path(path: &Path) -> Result<UserConfig> {
    if path.is_dir() {
        return load_user_config(path);
    }
    if !path.is_file() {
        return Err(FerriteError::Config(format!(
            "selected configuration `{}` is not a file or directory",
            path.display()
        )));
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("toml") => {
            let text = std::fs::read_to_string(path)?;
            toml::from_str(&text).map_err(|error| FerriteError::Config(format!("{}: {error}", path.display())))
        }
        Some("mts" | "cts" | "ts" | "tsx" | "mjs" | "cjs" | "js" | "jsx") => {
            Ok(js_config::load_one(path)?.config)
        }
        _ => Err(FerriteError::Config(format!("unsupported selected configuration `{}`; use TOML or a supported JavaScript/TypeScript configuration file", path.display()))),
    }
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
    if over.react.explicit_runtime || over.react.runtime != ReactConfig::default().runtime {
        if base.react.runtime != over.react.runtime {
            if over.react.runtime == "automatic" {
                base.react.factory = None;
                base.react.fragment = None;
            } else if over.react.runtime == "classic" {
                base.react.import_source = None;
            }
        }
        base.react.runtime = over.react.runtime;
        base.react.explicit_runtime = true;
    }
    if over.react.explicit_refresh || !over.react.refresh {
        base.react.refresh = over.react.refresh;
        base.react.explicit_refresh = true;
    }
    if over.react.import_source.is_some() {
        base.react.import_source = over.react.import_source;
    }
    if over.react.factory.is_some() {
        base.react.factory = over.react.factory;
    }
    if over.react.fragment.is_some() {
        base.react.fragment = over.react.fragment;
    }
    if over.framework.is_some() {
        base.framework = over.framework;
    }
    if over.foreign_plugins.is_some() {
        base.foreign_plugins = over.foreign_plugins;
    }
    base.runtime = merge_runtime(base.runtime, over.runtime);
    base.e2e = merge_e2e(base.e2e, over.e2e);
    base
}

fn merge_e2e(mut base: E2eConfig, over: E2eConfig) -> E2eConfig {
    if over.run_name.is_some() {
        base.run_name = over.run_name;
    }
    if over.metadata.is_some() {
        base.metadata = over.metadata;
    }
    if over.report_slow_tests.is_some() {
        base.report_slow_tests = over.report_slow_tests;
    }
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
    if over.download_dir.is_some() {
        base.download_dir = over.download_dir;
    }
    if over.base_url.is_some() {
        base.base_url = over.base_url;
    }
    if over.timeout_ms != defaults.timeout_ms {
        base.timeout_ms = over.timeout_ms;
    }
    if over.global_timeout_ms != defaults.global_timeout_ms {
        base.global_timeout_ms = over.global_timeout_ms;
    }
    if over.max_failures != defaults.max_failures {
        base.max_failures = over.max_failures;
    }
    if over.fail_on_flaky_tests {
        base.fail_on_flaky_tests = true;
    }
    if over.forbid_only {
        base.forbid_only = true;
    }
    if over.cleanup_timeout_ms != defaults.cleanup_timeout_ms {
        base.cleanup_timeout_ms = over.cleanup_timeout_ms;
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
    if over.preserve_output != defaults.preserve_output {
        base.preserve_output = over.preserve_output;
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
    if over.queue_capacity != 0 {
        base.queue_capacity = over.queue_capacity;
    }
    if over.max_request_bytes != 0 {
        base.max_request_bytes = over.max_request_bytes;
    }
    if over.max_call_depth != 0 {
        base.max_call_depth = over.max_call_depth;
    }
    if over.max_jobs_per_drain != 0 {
        base.max_jobs_per_drain = over.max_jobs_per_drain;
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
    if [
        user.react.import_source.as_deref(),
        user.react.factory.as_deref(),
        user.react.fragment.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| value.trim().is_empty())
    {
        return Err(FerriteError::Config(
            "react JSX settings must not be empty".into(),
        ));
    }
    if !matches!(user.react.runtime.as_str(), "automatic" | "classic") {
        return Err(FerriteError::Config(
            "react.runtime must be automatic or classic".into(),
        ));
    }
    if user.react.runtime == "classic" && user.react.import_source.is_some() {
        return Err(FerriteError::Config("react.import_source requires automatic JSX runtime; use factory/fragment for classic JSX".into()));
    }
    if user.react.runtime == "automatic"
        && (user.react.factory.is_some() || user.react.fragment.is_some())
    {
        return Err(FerriteError::Config(
            "react.factory/fragment require classic JSX runtime".into(),
        ));
    }
    if let Some(framework) = &user.framework {
        framework.validate()?;
    }
    let mut names = std::collections::HashSet::new();
    for plugin in user.foreign_plugins.iter().flatten() {
        if plugin.name.trim().is_empty() || !names.insert(&plugin.name) {
            return Err(FerriteError::Config(
                "foreign_plugins requires unique nonempty names".into(),
            ));
        }
        if plugin.host.as_deref() != Some("node") {
            return Err(FerriteError::Config(format!("foreign plugin {} requires explicit host = \"node\"; other hook hosts are unavailable; SSR runtime is configured separately", plugin.name)));
        }
        if plugin.entry.as_os_str().is_empty()
            || !(1..=300_000).contains(&plugin.timeout_ms.unwrap_or(10_000))
        {
            return Err(FerriteError::Config(format!(
                "foreign plugin {} requires an entry and timeout_ms between 1 and 300000",
                plugin.name
            )));
        }
    }
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
        .or(overrides.default_mode)
        .unwrap_or_else(|| "development".to_string());
    let is_production = overrides.is_production.unwrap_or(mode == "production");
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
        framework: user.framework.clone(),
        foreign_plugins: user.foreign_plugins.clone(),
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
    #[test]
    fn explicit_default_jsx_values_override_inheritance_and_survive_roundtrips() {
        let base: UserConfig = toml::from_str("[react]\nruntime = 'classic'\nfactory = 'UI.h'\nfragment = 'UI.Fragment'\nrefresh = false").unwrap();
        let overlay: UserConfig = toml::from_str(
            "[react]\nruntime = 'automatic'\nrefresh = true\nimport_source = 'selected-runtime'",
        )
        .unwrap();
        let merged = merge_user_config(base.clone(), overlay);
        assert_eq!(merged.react.runtime, "automatic");
        assert!(merged.react.refresh);
        assert!(merged.react.factory.is_none() && merged.react.fragment.is_none());
        assert_eq!(
            merged.react.import_source.as_deref(),
            Some("selected-runtime")
        );
        let inherited = merge_user_config(merged, UserConfig::default());
        assert_eq!(inherited.react.runtime, "automatic");
        assert!(inherited.react.refresh);
        let explicit: ReactConfig =
            serde_json::from_str(r#"{"runtime":"automatic","refresh":true}"#).unwrap();
        let explicit: ReactConfig =
            serde_json::from_str(&serde_json::to_string(&explicit).unwrap()).unwrap();
        let overlay = UserConfig {
            react: explicit,
            ..Default::default()
        };
        let merged = merge_user_config(base.clone(), overlay);
        assert_eq!(merged.react.runtime, "automatic");
        assert!(merged.react.refresh);
        let unset: ReactConfig =
            serde_json::from_str(&serde_json::to_string(&ReactConfig::default()).unwrap()).unwrap();
        let overlay = UserConfig {
            react: unset,
            ..Default::default()
        };
        let merged = merge_user_config(base.clone(), overlay);
        assert_eq!(merged.react.runtime, "classic");
        assert!(!merged.react.refresh);
        let mut overlay = UserConfig::default();
        overlay.react.set_runtime("automatic");
        overlay.react.set_refresh(true);
        let merged = merge_user_config(base, overlay);
        assert_eq!(merged.react.runtime, "automatic");
        assert!(merged.react.refresh);
        let base: UserConfig = toml::from_str("[react]\nimport_source = 'other'").unwrap();
        let overlay: UserConfig = toml::from_str("[react]\nruntime = 'classic'").unwrap();
        let merged = merge_user_config(base, overlay);
        assert!(merged.react.import_source.is_none());
        assert!(resolve_config(merged, None, Default::default()).is_ok());
        assert!(toml::from_str::<UserConfig>("[react]\nmisspelled = true").is_err());
    }

    #[test]
    fn jsx_file_settings_survive_default_overlay_and_invalid_profiles_fail() {
        let file: UserConfig = toml::from_str("[react]\nruntime = 'classic'\nfactory = 'UI.h'\nfragment = 'UI.Fragment'\nrefresh = false").unwrap();
        let merged = merge_user_config(file, UserConfig::default());
        let resolved = resolve_config(merged, None, CliOverrides::default()).unwrap();
        assert_eq!(resolved.react.runtime, "classic");
        assert_eq!(resolved.react.factory.as_deref(), Some("UI.h"));
        assert_eq!(resolved.react.fragment.as_deref(), Some("UI.Fragment"));
        assert!(!resolved.react.refresh);
        for text in [
            "[react]\nruntime = 'unknown'",
            "[react]\nruntime = 'classic'\nimport_source = 'react'",
            "[react]\nfactory = 'h'",
        ] {
            assert!(
                resolve_config(toml::from_str(text).unwrap(), None, Default::default()).is_err()
            );
        }
    }

    #[test]
    fn explicit_config_selection_never_substitutes_discovered_files() {
        let dir =
            std::env::temp_dir().join(format!("ferrite-explicit-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ferrite.toml"), "invalid TOML!").unwrap();
        std::fs::write(dir.join("ferrite.local.toml"), "[server]\nport = 9000").unwrap();
        let selected = dir.join("custom.toml");
        std::fs::write(
            &selected,
            "[server]\nport = 4321\n[framework]\nenabled = []",
        )
        .unwrap();
        assert_eq!(load_user_config_path(&selected).unwrap().server.port, 4321);
        assert!(load_user_config_path(&selected)
            .unwrap()
            .framework
            .unwrap()
            .enabled
            .is_empty());
        assert!(load_user_config_path(&dir).is_err());
        let js = dir.join("custom.ts");
        std::fs::write(&js, "export default {server: {port: 7654}};").unwrap();
        assert_eq!(load_user_config_path(&js).unwrap().server.port, 7654);
        std::fs::write(&selected, "bad TOML!").unwrap();
        assert!(load_user_config_path(&selected).is_err());
        assert!(load_user_config_path(&dir.join("missing.toml")).is_err());
        let unknown = dir.join("custom.txt");
        std::fs::write(&unknown, "[server]\nport = 1234").unwrap();
        assert!(load_user_config_path(&unknown).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    use super::*;

    #[test]
    fn framework_host_requires_explicit_opt_in_and_validates_ownership() {
        let parse = |text: &str| toml::from_str::<UserConfig>(text).unwrap();
        for text in [
            "[framework]\nenabled=['vue']",
            "[framework]\nenabled=['vue']\ncompiler_host='embedded'",
            "[framework]\nenabled=['angular']\ncompiler_host='node'",
            "[framework]\nenabled=['vue','vue']\ncompiler_host='node'",
            "[framework]\nenabled=['svelte']\ncompiler_host='node'\ntimeout_ms=0",
            "[framework]\nenabled=['react']\ncompiler_host='node'",
            "[framework]\nenabled=['react']",
            "[framework]\nenabled=['react','vue']\ncompiler_host='native'",
        ] {
            assert!(
                resolve_config(parse(text), None, CliOverrides::default()).is_err(),
                "{text}"
            );
        }
        let enabled = parse(
            "[framework]\nenabled=['vue','svelte']\ncompiler_host='node'\n[runtime]\nbackend='boa'",
        );
        let merged = merge_user_config(enabled.clone(), UserConfig::default());
        assert_eq!(merged.framework.unwrap().enabled, ["vue", "svelte"]);
        let disabled = merge_user_config(enabled.clone(), parse("[framework]\nenabled=[]"));
        assert!(disabled.framework.unwrap().enabled.is_empty());
        let resolved = resolve_config(enabled, None, CliOverrides::default()).unwrap();
        assert_eq!(resolved.runtime.backend, "boa");
        assert_eq!(
            resolved.framework.unwrap().compiler_host.as_deref(),
            Some("node")
        );
        assert!(toml::from_str::<UserConfig>("[framework]\ncompiler_hosts='node'").is_err());
        let native = resolve_config(
            parse(
                "[framework]\nenabled=['react']\ncompiler_host='native'\n[runtime]\nbackend='boa'",
            ),
            None,
            CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(
            native.framework.unwrap().compiler_host.as_deref(),
            Some("native")
        );
        assert_eq!(native.runtime.backend, "boa");
    }

    #[test]
    fn e2e_project_toml_roundtrip_and_old_json_defaults() {
        let user: UserConfig = toml::from_str(
            r#"
            [e2e]
            repeat_each=3
            snapshot_dir="baselines"
            grep="smoke"
            grep_invert="slow"
            shard=[1,2]
            selected_projects=["desktop"]
            [[e2e.projects]]
            name="desktop"
            browser="firefox"
            retries=0
            timeout_ms=0
            repeat_each=0
            output_dir="desktop output"
            snapshot_dir="desktop baselines"
            grep="app"
            grep_invert="blocked"
            args=[]
            [e2e.projects.viewport]
            width=640
            height=480
        "#,
        )
        .unwrap();
        let config = &user.e2e;
        assert_eq!(config.repeat_each, 3);
        assert_eq!(config.shard, Some((1, 2)));
        assert_eq!(config.selected_projects, ["desktop"]);
        let project = &config.projects[0];
        assert_eq!(project.retries, Some(0));
        assert_eq!(project.timeout_ms, Some(0));
        assert_eq!(project.repeat_each, Some(0));
        assert_eq!(project.viewport.as_ref().unwrap().width, 640);
        let decoded: E2eConfig =
            serde_json::from_str(&serde_json::to_string(config).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(config).unwrap()
        );
        let old: E2eConfig = serde_json::from_str(r#"{"workers":2,"retries":1}"#).unwrap();
        assert_eq!(old.workers, 2);
        assert_eq!(old.repeat_each, 1);
        assert!(old.projects.is_empty());
        assert!(old.snapshot_dir.is_none());
        assert!(old.grep_invert.is_none());
    }

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
             max_call_depth = 16\n\
             max_jobs_per_drain = 32\n\
             queue_capacity = 8\n\
             max_request_bytes = 1048576\n\
             fuel_budget = 10000000\n\
             native_allow = [\"native/addon.node\"]\n\
             [runtime.native_integrity]\n\
             \"native/addon.node\" = \"ab12\"\n",
        )
        .unwrap();
        assert_eq!(user.runtime.backend, "napi-vm");
        assert_eq!(user.runtime.max_call_depth, 16);
        assert_eq!(user.runtime.max_jobs_per_drain, 32);
        assert_eq!(user.runtime.queue_capacity, 8);
        assert_eq!(user.runtime.max_request_bytes, 1_048_576);
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
        over.runtime.queue_capacity = 4;
        over.runtime.max_request_bytes = 1024;
        let merged = merge_user_config(base, over);
        assert_eq!(merged.runtime.backend, "napi-vm");
        assert_eq!(merged.runtime.loop_budget, 5);
        assert_eq!(merged.runtime.queue_capacity, 4);
        assert_eq!(merged.runtime.max_request_bytes, 1024);
        let preserved = merge_user_config(merged, UserConfig::default());
        assert_eq!(preserved.runtime.queue_capacity, 4);
        assert_eq!(preserved.runtime.max_request_bytes, 1024);

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
        base.server
            .proxy
            .insert("/a".to_string(), "http://a".to_string());
        let mut over = UserConfig::default();
        over.env.prefix = vec!["APP_".to_string()];
        over.server
            .proxy
            .insert("/b".to_string(), "http://b".to_string());
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
        over.e2e.fail_on_flaky_tests = true;
        over.e2e.forbid_only = true;
        let merged = merge_user_config(base, over);
        assert_eq!(merged.e2e.retries, 3);
        assert!(!merged.e2e.headless);
        assert!(merged.e2e.fail_on_flaky_tests && merged.e2e.forbid_only);
        let inherited = merge_user_config(merged, UserConfig::default());
        assert!(inherited.e2e.fail_on_flaky_tests && inherited.e2e.forbid_only);
        assert_eq!(inherited.e2e.workers, E2eConfig::default().workers);
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
    #[test]
    fn e2e_output_retention_merge_and_legacy_default() {
        let legacy: E2eConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.preserve_output, "always");
        let base = E2eConfig {
            preserve_output: "never".into(),
            ..Default::default()
        };
        assert_eq!(
            merge_e2e(base.clone(), E2eConfig::default()).preserve_output,
            "never"
        );
        let over = E2eConfig {
            preserve_output: "failures-only".into(),
            ..Default::default()
        };
        assert_eq!(merge_e2e(base, over).preserve_output, "failures-only");
    }
    #[test]
    fn e2e_metadata_layers_preserve_explicit_empty_and_disabled_values() {
        let base = E2eConfig {
            run_name: Some("base".into()),
            metadata: Some([("key".into(), serde_json::json!(1))].into_iter().collect()),
            report_slow_tests: Some(Default::default()),
            ..Default::default()
        };
        let retained = merge_e2e(base.clone(), E2eConfig::default());
        assert_eq!(retained.metadata, base.metadata);
        assert_eq!(retained.run_name, base.run_name);
        let overridden = merge_e2e(
            base,
            E2eConfig {
                run_name: Some(String::new()),
                metadata: Some(Default::default()),
                report_slow_tests: Some(SlowTestOptions {
                    threshold_ms: 0,
                    max: 0,
                }),
                ..Default::default()
            },
        );
        assert_eq!(overridden.run_name.as_deref(), Some(""));
        assert!(overridden.metadata.unwrap().is_empty());
        assert_eq!(overridden.report_slow_tests.unwrap().max, 0);
        assert!(serde_json::from_str::<E2eConfig>(r#"{"metadata": "invalid"}"#).is_err());
        assert!(serde_json::from_str::<E2eConfig>(r#"{"report_slow_tests":{"max":-1}}"#).is_err());
    }
    #[test]
    fn runtime_mode_define_is_defaulted_and_explicit_values_win() {
        let development =
            resolve_config(UserConfig::default(), None, CliOverrides::default()).unwrap();
        for environment in [development.client_env(), development.ssr_env()] {
            assert_eq!(
                environment.define["process.env.NODE_ENV"],
                "\"development\""
            );
        }
        let production = resolve_config(
            UserConfig::default(),
            None,
            CliOverrides {
                mode: Some("production".into()),
                ..Default::default()
            },
        )
        .unwrap();
        for environment in [production.client_env(), production.ssr_env()] {
            assert_eq!(environment.define["process.env.NODE_ENV"], "\"production\"");
        }
        let mut user = UserConfig::default();
        user.define
            .insert("process.env.NODE_ENV".into(), "\"custom\"".into());
        let configured = resolve_config(user, None, CliOverrides::default()).unwrap();
        assert_eq!(
            configured.client_env().define["process.env.NODE_ENV"],
            "\"custom\""
        );
        assert_eq!(
            configured.ssr_env().define["process.env.NODE_ENV"],
            "\"custom\""
        );
    }
}

#[cfg(test)]
mod foreign_profile_tests {
    use super::*;

    #[test]
    fn explicit_profiles_validate_and_empty_overrides_disable_inheritance() {
        let text = "[[foreign_plugins]]\nname = 'fixture'\nentry = 'plugin.mjs'\nhost = 'node'\n[foreign_plugins.options]\nprefix = 'value'\n";
        let user: UserConfig = toml::from_str(text).unwrap();
        let roundtrip: UserConfig = toml::from_str(&toml::to_string(&user).unwrap()).unwrap();
        assert_eq!(
            roundtrip.foreign_plugins.as_ref().unwrap()[0].options["prefix"],
            "value"
        );
        assert_eq!(
            resolve_config(user.clone(), None, Default::default())
                .unwrap()
                .foreign_plugins
                .unwrap()
                .len(),
            1
        );
        let over: UserConfig = toml::from_str("foreign_plugins = []").unwrap();
        assert!(merge_user_config(user.clone(), over)
            .foreign_plugins
            .unwrap()
            .is_empty());
        for invalid in [
            text.replace("host = 'node'", ""),
            text.replace("host = 'node'", "host = 'embedded'"),
            text.replace("entry = 'plugin.mjs'", "entry = ''"),
            text.replace("name = 'fixture'", "name = ''"),
            text.replace("host = 'node'", "host = 'node'\ntimeout_ms = 0"),
        ] {
            let user: UserConfig = toml::from_str(&invalid).unwrap();
            assert!(
                resolve_config(user, None, Default::default()).is_err(),
                "{invalid}"
            );
        }
        let mut duplicated = user;
        let duplicate = duplicated.foreign_plugins.as_ref().unwrap()[0].clone();
        duplicated.foreign_plugins.as_mut().unwrap().push(duplicate);
        assert!(resolve_config(duplicated, None, Default::default()).is_err());
        assert!(toml::from_str::<UserConfig>(
            &text.replace("host = 'node'", "host = 'node'\nunsupported = true")
        )
        .is_err());
    }
}

/// Select a physical JavaScript/TypeScript server entry for development or build.
/// Explicit configuration wins; the legacy Rust default selects conventional
/// discovery. This selects source, not a renderer or runtime backend.
pub fn resolve_js_server_entry(config: &ResolvedConfig) -> Result<Option<String>> {
    if let Some(entry) = config
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
        if !config.root.join(path).is_file() {
            return Err(FerriteError::Build(format!(
                "configured SSR entry `{entry}` is not a file; create it or correct [ssr].entry"
            )));
        }
        if !ferrite_core::ModuleType::from_path(entry).is_js_like() {
            return Err(FerriteError::Build(format!("configured SSR entry `{entry}` requires a JavaScript/TypeScript module; Rust entries are unavailable in the JavaScript server pipeline")));
        }
        return Ok(Some(entry.to_string()));
    }
    for candidate in [
        "src/entry-server.ts",
        "src/entry-server.tsx",
        "src/entry-server.mts",
        "src/entry-server.cts",
        "src/entry-server.js",
        "src/entry-server.jsx",
        "src/entry-server.mjs",
        "src/entry-server.cjs",
        "src/server.ts",
        "src/server.js",
    ] {
        if config.root.join(candidate).is_file() {
            return Ok(Some(candidate.to_string()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod server_entry_tests {
    use super::*;

    #[test]
    fn server_entry_discovery_accepts_each_script_extension_and_explicit_override() {
        for extension in ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir(root.path().join("src")).unwrap();
            let entry = format!("src/entry-server.{extension}");
            std::fs::write(root.path().join(&entry), "export const value = 42;").unwrap();
            let mut config = resolve_config(
                Default::default(),
                Some(root.path().into()),
                Default::default(),
            )
            .unwrap();
            assert_eq!(resolve_js_server_entry(&config).unwrap(), Some(entry));
            std::fs::write(
                root.path().join("custom.ts"),
                "export const selected = true;",
            )
            .unwrap();
            config.ssr.entry = Some("custom.ts".into());
            assert_eq!(
                resolve_js_server_entry(&config).unwrap(),
                Some("custom.ts".into())
            );
            config.ssr.entry = Some("missing.ts".into());
            assert!(resolve_js_server_entry(&config)
                .unwrap_err()
                .to_string()
                .contains("missing.ts"));
        }
    }
}
