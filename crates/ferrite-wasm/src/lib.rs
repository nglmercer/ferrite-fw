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
//! v0.1 serves prebuilt artifacts (build the crate first, or let the plugin
//! error guide you); automatic `cargo build` orchestration is roadmap.

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

/// Built-in Rust/WASM plugin.
#[derive(Debug, Default)]
pub struct RustWasmPlugin {
    /// Directory with prebuilt `wasm-bindgen` glue (`<name>.js` + `<name>_bg.wasm`).
    pub glue_dir: Option<std::path::PathBuf>,
}

impl RustWasmPlugin {
    /// Create a plugin that looks for glue in `dir`.
    #[must_use]
    pub fn new(glue_dir: Option<std::path::PathBuf>) -> Self {
        Self { glue_dir }
    }
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
        Err(FerriteError::Other(format!(
            "cannot load `rust:{crate_name}`: no prebuilt wasm-bindgen glue found. Build it with:\n\
             \n  cargo build -p {crate_name} --target wasm32-unknown-unknown\n  wasm-bindgen target/wasm32-unknown-unknown/debug/{crate_name}.wasm \\\n    --out-dir <glue-dir> --target web\n  \n\
             then point `RustWasmPlugin::new(Some(glue_dir))` at the output. \
             Automatic cargo orchestration is on the roadmap (§45)."
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
}
