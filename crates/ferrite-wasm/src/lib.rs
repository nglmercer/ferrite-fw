//! Rust/WASM integration (spec §45, §74).
//!
//! Built-in handling for `.wasm` imports and `rust:<crate>` packages:
//!
//! ```text
//! rust:my_crypto
//!   → Cargo metadata
//!   → cargo build --target wasm32-unknown-unknown
//!   → wasm-bindgen glue
//!   → virtual JS entry
//! ```
//!
//! Serves prebuilt `wasm-bindgen` glue when present, else builds it on
//! demand through [`WasmBuildConfig`] (`cargo build` + `wasm-bindgen`,
//! re-run when crate sources are newer than the glue).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use ferrite_core::{FerriteError, ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};

/// True for internal `\0rust:<name>` ids.
#[must_use]
pub fn is_rust_package(id: &str) -> bool {
    id.starts_with("\0rust:")
}

/// Crate name from a `\0rust:<name>` id.
#[must_use]
pub fn rust_crate_name(id: &str) -> Option<&str> {
    id.strip_prefix("\0rust:")
}

/// JS loader shim for a `.wasm` URL (§45).
#[must_use]
pub fn wasm_loader_shim(wasm_url: &str) -> String {
    format!(
        "export async function init(input) {{\n\
         const source = input ?? {wasm_url:?};\n\
         const bytes = await fetch(source).then((r) => r.arrayBuffer());\n\
         const {{ instance }} = await WebAssembly.instantiate(bytes);\n\
         return instance.exports;\n\
         }}\n\
         export default init;\n"
    )
}

/// On-demand build recipe for one `rust:<crate>` package (§45).
#[derive(Debug, Clone)]
pub struct WasmBuildConfig {
    /// Crate directory (holds `Cargo.toml`).
    pub manifest_dir: PathBuf,
    /// Package passed to `cargo build -p` (defaults to the `rust:` name).
    pub package: Option<String>,
    /// Cargo profile (`debug` default).
    pub profile: String,
    /// Rust target triple (`wasm32-unknown-unknown` default).
    pub target: String,
    /// Glue output directory (holds `<name>.js` + `<name>_bg.wasm`).
    pub out_dir: PathBuf,
    /// `cargo` binary override (default: `PATH` lookup).
    pub cargo: Option<PathBuf>,
    /// `wasm-bindgen` binary override (default: `PATH` lookup).
    pub wasm_bindgen: Option<PathBuf>,
    /// Extra `cargo build` args (e.g. `--features`, keep minimal).
    pub extra_args: Vec<String>,
}

impl WasmBuildConfig {
    /// Recipe with defaults: debug profile, wasm32 target, `PATH` tools.
    #[must_use]
    pub fn new(manifest_dir: PathBuf, out_dir: PathBuf) -> Self {
        Self {
            manifest_dir,
            package: None,
            profile: "debug".to_string(),
            target: "wasm32-unknown-unknown".to_string(),
            out_dir,
            cargo: None,
            wasm_bindgen: None,
            extra_args: Vec::new(),
        }
    }
}

/// Built-in Rust/WASM plugin.
#[derive(Debug, Default)]
pub struct RustWasmPlugin {
    /// Directory with prebuilt `wasm-bindgen` glue (`<name>.js` + `<name>_bg.wasm`).
    pub glue_dir: Option<PathBuf>,
    /// On-demand build recipes by crate name.
    pub builds: HashMap<String, WasmBuildConfig>,
}

impl RustWasmPlugin {
    /// Create a plugin that looks for glue in `dir`.
    #[must_use]
    pub fn new(glue_dir: Option<PathBuf>) -> Self {
        Self {
            glue_dir,
            builds: HashMap::new(),
        }
    }

    /// Register an on-demand build recipe for `crate_name`.
    pub fn with_build(mut self, crate_name: &str, config: WasmBuildConfig) -> Self {
        self.builds.insert(crate_name.to_string(), config);
        self
    }

    /// Ensure fresh glue for `crate_name`, building on demand.
    ///
    /// Returns the glue JS path. Rebuilds when the glue is missing or any
    /// crate source is newer than this session's last build.
    pub fn ensure_glue(&self, crate_name: &str) -> Result<PathBuf> {
        let config = self.builds.get(crate_name).ok_or_else(|| {
            FerriteError::Other(format!(
                "cannot load `rust:{crate_name}`: no prebuilt glue and no build recipe. \
                 Serve prebuilt glue via `glue_dir`, or register \
                 `WasmBuildConfig::new(manifest_dir, out_dir)` for `{crate_name}`."
            ))
        })?;
        let package = config
            .package
            .clone()
            .unwrap_or_else(|| crate_name.to_string());
        let glue = config.out_dir.join(format!("{crate_name}.js"));
        if !glue_missing_or_stale(&glue, &config.manifest_dir) {
            return Ok(glue);
        }
        let cargo = resolve_tool(config.cargo.as_deref(), "cargo", "https://rustup.rs")?;
        if config.cargo.is_none() {
            // Explicit toolchains are caller-owned; otherwise fail fast
            // with the install hint instead of a cryptic cargo error.
            check_target(&config.target)?;
        }
        let manifest = config.manifest_dir.join("Cargo.toml");
        if !manifest.is_file() {
            return Err(FerriteError::Other(format!(
                "cannot build `rust:{crate_name}`: {} is not a file",
                manifest.display()
            )));
        }
        let target_dir = cargo_target_dir(&cargo, &manifest)?;
        run_cargo_build(&cargo, config, &package, &manifest)?;
        let wasm = target_dir
            .join(&config.target)
            .join(if config.profile == "dev" {
                "debug"
            } else {
                &config.profile
            })
            .join(format!("{}.wasm", package.replace('-', "_")));
        if !wasm.is_file() {
            return Err(FerriteError::Other(format!(
                "cargo build succeeded but {} is missing (cdylib crate-type?)",
                wasm.display()
            )));
        }
        let bindgen = resolve_tool(
            config.wasm_bindgen.as_deref(),
            "wasm-bindgen",
            "`cargo install wasm-bindgen-cli`",
        )?;
        std::fs::create_dir_all(&config.out_dir)?;
        run_wasm_bindgen(&bindgen, &wasm, &config.out_dir, crate_name)?;
        if !glue.is_file() {
            return Err(FerriteError::Other(format!(
                "wasm-bindgen succeeded but {} is missing",
                glue.display()
            )));
        }
        Ok(glue)
    }
}

/// True when the glue is missing or any crate source is newer.
fn glue_missing_or_stale(glue: &Path, manifest_dir: &Path) -> bool {
    let glue_time = std::fs::metadata(glue)
        .and_then(|meta| meta.modified())
        .ok();
    let Some(glue_time) = glue_time else {
        return true;
    };
    newest_source_time(manifest_dir).is_some_and(|newest| newest > glue_time)
}

/// Newest mtime under `manifest_dir` (`src/**`, `Cargo.toml`, `Cargo.lock`).
fn newest_source_time(manifest_dir: &Path) -> Option<std::time::SystemTime> {
    let mut newest = None;
    let mut stack = vec![manifest_dir.join("src")];
    for manifest in ["Cargo.toml", "Cargo.lock"] {
        stack.push(manifest_dir.join(manifest));
    }
    while let Some(path) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                stack.extend(entries.filter_map(|entry| entry.ok().map(|entry| entry.path())));
            }
            continue;
        }
        if let Ok(modified) = meta.modified() {
            newest =
                Some(newest.map_or(modified, |best: std::time::SystemTime| best.max(modified)));
        }
    }
    newest
}

/// Resolve a build tool: explicit path or `PATH` lookup with install hint.
fn resolve_tool(explicit: Option<&Path>, name: &str, hint: &str) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return Ok(path.to_path_buf());
        }
        return Err(FerriteError::Other(format!(
            "configured {name} `{}` is not a file",
            path.display()
        )));
    }
    find_on_path(name)
        .ok_or_else(|| FerriteError::Other(format!("`{name}` not found on PATH (install: {hint})")))
}

/// Minimal `PATH` lookup (no extra dependency).
fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .chain(std::env::split_paths(&path).map(|dir| dir.join(format!("{name}.exe"))))
        .find(|candidate| candidate.is_file())
}

/// Fail fast when the wasm target is not installed (via rustup, when present).
fn check_target(target: &str) -> Result<()> {
    let Some(rustup) = find_on_path("rustup") else {
        return Ok(()); // No rustup: let cargo surface the real error.
    };
    let output = Command::new(rustup)
        .args(["target", "list", "--installed"])
        .output()
        .map_err(|error| FerriteError::Other(format!("cannot run rustup: {error}")))?;
    let installed = String::from_utf8_lossy(&output.stdout);
    if installed.lines().any(|line| line.trim() == target) {
        return Ok(());
    }
    Err(FerriteError::Other(format!(
        "Rust target `{target}` is not installed (run `rustup target add {target}`)"
    )))
}

/// `cargo metadata` target directory (workspace-aware).
fn cargo_target_dir(cargo: &Path, manifest: &Path) -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .map_err(|error| FerriteError::Other(format!("cannot run cargo metadata: {error}")))?;
    if !output.status.success() {
        // Fall back to the conventional layout; the build error below is authoritative.
        return Ok(manifest.parent().unwrap_or(Path::new(".")).join("target"));
    }
    let meta: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| FerriteError::Other(format!("cannot parse cargo metadata: {error}")))?;
    meta.get("target_directory")
        .and_then(|dir| dir.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| FerriteError::Other("cargo metadata has no target_directory".to_string()))
}

/// Run `cargo build` for the wasm target; loud on failure.
fn run_cargo_build(
    cargo: &Path,
    config: &WasmBuildConfig,
    package: &str,
    manifest: &Path,
) -> Result<()> {
    let mut args = vec![
        "build".to_string(),
        "--manifest-path".to_string(),
        manifest.to_string_lossy().into_owned(),
        "-p".to_string(),
        package.to_string(),
        "--target".to_string(),
        config.target.clone(),
    ];
    if config.profile != "debug" && config.profile != "dev" {
        args.push("--profile".to_string());
        args.push(config.profile.clone());
    }
    args.extend(config.extra_args.iter().cloned());
    let output = Command::new(cargo)
        .args(&args)
        .output()
        .map_err(|error| FerriteError::Other(format!("cannot run cargo build: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(FerriteError::Other(format!(
        "cargo build -p {package} --target {} failed:\n{}",
        config.target,
        tail_utf8(&output.stderr, 4000)
    )))
}

/// Run `wasm-bindgen` for web glue; loud on failure.
fn run_wasm_bindgen(bindgen: &Path, wasm: &Path, out_dir: &Path, crate_name: &str) -> Result<()> {
    let output = Command::new(bindgen)
        .arg(wasm)
        .args(["--out-dir"])
        .arg(out_dir)
        .args(["--target", "web", "--out-name", crate_name])
        .output()
        .map_err(|error| FerriteError::Other(format!("cannot run wasm-bindgen: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(FerriteError::Other(format!(
        "wasm-bindgen for `{crate_name}` failed:\n{}",
        tail_utf8(&output.stderr, 4000)
    )))
}

/// Last `max` bytes of output as lossy text.
fn tail_utf8(bytes: &[u8], max: usize) -> String {
    let start = bytes.len().saturating_sub(max);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

#[async_trait::async_trait]
impl Plugin for RustWasmPlugin {
    fn name(&self) -> &'static str {
        "ferrite:rust-wasm"
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        let Some(crate_name) = rust_crate_name(&request.id) else {
            return Ok(None);
        };
        // Serve prebuilt glue when present.
        if let Some(dir) = &self.glue_dir {
            let glue = dir.join(format!("{crate_name}.js"));
            if glue.exists() {
                let code = std::fs::read_to_string(&glue)?;
                return Ok(Some(LoadResult {
                    code,
                    module_type: ModuleType::Js,
                    dependencies: vec![glue.to_string_lossy().into_owned()],
                }));
            }
        }
        // Else build on demand when a recipe is registered.
        if self.builds.contains_key(crate_name) {
            let glue = self.ensure_glue(crate_name)?;
            let code = std::fs::read_to_string(&glue)?;
            return Ok(Some(LoadResult {
                code,
                module_type: ModuleType::Js,
                dependencies: vec![glue.to_string_lossy().into_owned()],
            }));
        }
        Err(FerriteError::Other(format!(
            "cannot load `rust:{crate_name}`: no prebuilt glue and no build recipe. Serve glue \
             via `glue_dir`, or register `WasmBuildConfig::new(manifest_dir, out_dir)`."
        )))
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        // The core resolver already maps `rust:x` → `\0rust:x`; accept it here
        // so plugin ordering stays explicit.
        if request.specifier.starts_with("rust:") {
            let name = request.specifier.trim_start_matches("rust:");
            return Ok(Some(ferrite_resolver::ResolvedId::new(format!(
                "\0rust:{name}"
            ))));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rust_ids() {
        assert!(is_rust_package("\0rust:my_crypto"));
        assert_eq!(rust_crate_name("\0rust:my_crypto"), Some("my_crypto"));
        assert!(!is_rust_package("./local.ts"));
    }

    #[test]
    fn wasm_shim_instantiates() {
        let shim = wasm_loader_shim("/assets/mod-abc.wasm");
        assert!(shim.contains("WebAssembly.instantiate"));
        assert!(shim.contains("/assets/mod-abc.wasm"));
    }

    #[test]
    fn no_recipe_errors_with_hint() {
        let plugin = RustWasmPlugin::new(None);
        let error = plugin.ensure_glue("ghost").unwrap_err();
        assert!(error.to_string().contains("WasmBuildConfig"), "{error}");
    }

    /// Fake `cargo`: answers `metadata`, materializes `$target/.../debug/<pkg>.wasm` on `build`.
    #[cfg(unix)]
    const FAKE_CARGO: &str = r#"#!/bin/sh
prev=""
for a in "$@"; do
  case "$prev" in
    --manifest-path) MF="$a";;
    -p) PKG="$a";;
    --target) TGT="$a";;
  esac
  prev="$a"
done
if [ "$1" = "metadata" ]; then
  printf '{"target_directory":"%s/target"}\n' "$(dirname "$MF")"
  exit 0
fi
D="$(dirname "$MF")/target/$TGT/debug"
mkdir -p "$D"
printf 'fakewasm' > "$D/$PKG.wasm"
echo built >> "$(dirname "$MF")/build.log"
"#;

    /// Fake `wasm-bindgen`: writes `<name>.js` + `<name>_bg.wasm` into `--out-dir`.
    #[cfg(unix)]
    const FAKE_BINDGEN: &str = r#"#!/bin/sh
prev=""
for a in "$@"; do
  case "$prev" in
    --out-dir) OUT="$a";;
    --out-name) NAME="$a";;
  esac
  prev="$a"
done
mkdir -p "$OUT"
printf '// glue for %s\n' "$NAME" > "$OUT/$NAME.js"
printf 'fakebg' > "$OUT/${NAME}_bg.wasm"
"#;

    #[cfg(unix)]
    fn write_executable(path: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    fn fake_crate(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("ferrite-wasm-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let manifest_dir = dir.join("crates").join("demo");
        std::fs::create_dir_all(manifest_dir.join("src")).unwrap();
        std::fs::write(
            manifest_dir.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n[lib]\ncrate-type = [\"cdylib\"]\n",
        )
        .unwrap();
        std::fs::write(manifest_dir.join("src/lib.rs"), "// demo\n").unwrap();
        let tools = dir.join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        write_executable(&tools.join("cargo"), FAKE_CARGO);
        write_executable(&tools.join("wasm-bindgen"), FAKE_BINDGEN);
        (manifest_dir, tools)
    }

    #[cfg(unix)]
    fn test_plugin(manifest_dir: &Path, tools: &Path, out_dir: PathBuf) -> RustWasmPlugin {
        let mut config = WasmBuildConfig::new(manifest_dir.to_path_buf(), out_dir);
        config.cargo = Some(tools.join("cargo"));
        config.wasm_bindgen = Some(tools.join("wasm-bindgen"));
        RustWasmPlugin::new(None).with_build("demo", config)
    }

    #[cfg(unix)]
    #[test]
    fn orchestrates_fake_toolchain() {
        let (manifest_dir, tools) = fake_crate("ok");
        let out_dir = manifest_dir.join("glue");
        let plugin = test_plugin(&manifest_dir, &tools, out_dir.clone());
        let glue = plugin.ensure_glue("demo").unwrap();
        assert_eq!(glue, out_dir.join("demo.js"));
        assert!(glue.is_file());
        assert!(out_dir.join("demo_bg.wasm").is_file());
        let code = std::fs::read_to_string(&glue).unwrap();
        assert!(code.contains("glue for demo"), "{code}");
        let log = std::fs::read_to_string(manifest_dir.join("build.log")).unwrap();
        assert_eq!(log.lines().count(), 1);
        // Fresh glue: no rebuild.
        plugin.ensure_glue("demo").unwrap();
        let log = std::fs::read_to_string(manifest_dir.join("build.log")).unwrap();
        assert_eq!(log.lines().count(), 1);
        let _ = std::fs::remove_dir_all(manifest_dir.parent().unwrap().parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn rebuilds_when_source_newer() {
        let (manifest_dir, tools) = fake_crate("stale");
        let out_dir = manifest_dir.join("glue");
        let plugin = test_plugin(&manifest_dir, &tools, out_dir);
        plugin.ensure_glue("demo").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(manifest_dir.join("src/lib.rs"), "// demo v2\n").unwrap();
        plugin.ensure_glue("demo").unwrap();
        let log = std::fs::read_to_string(manifest_dir.join("build.log")).unwrap();
        assert_eq!(log.lines().count(), 2);
        let _ = std::fs::remove_dir_all(manifest_dir.parent().unwrap().parent().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn missing_manifest_errors_loudly() {
        let dir = std::env::temp_dir().join(format!("ferrite-wasm-noman-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tools = dir.join("tools");
        std::fs::create_dir_all(&tools).unwrap();
        write_executable(&tools.join("cargo"), FAKE_CARGO);
        let mut config = WasmBuildConfig::new(dir.join("empty"), dir.join("glue"));
        config.cargo = Some(tools.join("cargo"));
        let plugin = RustWasmPlugin::new(None).with_build("demo", config);
        let error = plugin.ensure_glue("demo").unwrap_err();
        assert!(error.to_string().contains("Cargo.toml"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn cargo_failure_propagates_stderr() {
        let (manifest_dir, tools) = fake_crate("fail");
        write_executable(
            &tools.join("cargo-fail"),
            "#!/bin/sh\necho 'error: exploded' >&2\nexit 1\n",
        );
        let mut config = WasmBuildConfig::new(manifest_dir.clone(), manifest_dir.join("glue"));
        config.cargo = Some(tools.join("cargo-fail"));
        config.wasm_bindgen = Some(tools.join("wasm-bindgen"));
        let plugin = RustWasmPlugin::new(None).with_build("demo", config);
        let error = plugin.ensure_glue("demo").unwrap_err();
        assert!(error.to_string().contains("exploded"), "{error}");
        let _ = std::fs::remove_dir_all(manifest_dir.parent().unwrap().parent().unwrap());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn load_serves_built_glue() {
        use std::sync::Mutex as StdMutex;
        let (manifest_dir, tools) = fake_crate("load");
        let out_dir = manifest_dir.join("glue");
        let plugin = test_plugin(&manifest_dir, &tools, out_dir);
        let graph = ferrite_graph::ModuleGraph::new();
        let resolver = ferrite_resolver::Resolver::new(
            manifest_dir.clone(),
            &ferrite_config::ResolveConfig::default(),
        );
        let environment =
            ferrite_core::Environment::new("client", ferrite_core::EnvironmentKind::Client);
        let emitted = StdMutex::new(HashMap::new());
        let watch_files = StdMutex::new(Vec::new());
        let warnings = StdMutex::new(Vec::new());
        let ctx = PluginContext {
            graph: &graph,
            resolver: &resolver,
            environment: &environment,
            emitted: &emitted,
            watch_files: &watch_files,
            warnings: &warnings,
        };
        let loaded = plugin
            .load(
                &ctx,
                LoadRequest {
                    id: "\0rust:demo".to_string(),
                    environment: ferrite_core::EnvironmentKind::Client,
                },
            )
            .await
            .unwrap()
            .expect("glue");
        assert!(loaded.code.contains("glue for demo"), "{}", loaded.code);
        assert_eq!(loaded.dependencies.len(), 1);
        let _ = std::fs::remove_dir_all(manifest_dir.parent().unwrap().parent().unwrap());
    }
}
