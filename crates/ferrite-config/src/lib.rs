//! Configuration loading and resolution (spec §10).
//!
//! Precedence: CLI flags > `ferrite.local.toml` > `ferrite.toml` >
//! Cargo metadata > defaults.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ferrite_core::{Environment, EnvironmentKind, FerriteError, Result, Target};

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
}

impl Default for PackageConfig {
    fn default() -> Self {
        Self {
            standalone: false,
            embed_assets: true,
            compress_assets: true,
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
    /// Override runtime backend.
    pub runtime: Option<String>,
}

/// Load `ferrite.toml` + `ferrite.local.toml` from `dir` (both optional).
pub fn load_user_config(dir: &Path) -> Result<UserConfig> {
    let mut merged = UserConfig::default();
    for file in ["ferrite.toml", "ferrite.local.toml"] {
        let path = dir.join(file);
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let parsed: UserConfig = toml::from_str(&text)
                .map_err(|error| FerriteError::Config(format!("{file}: {error}")))?;
            merged = merge_user_config(merged, parsed);
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
}
