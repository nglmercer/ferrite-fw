//! Bundler module loading primitives.

use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_graph::ImportKind;

/// Extracted CSS carried by a module (§29).
#[derive(Debug, Clone)]
pub struct CssExtract {
    /// Stylesheet text (refs rewritten, minified in production).
    pub text: String,
    /// True for CSS modules (the JS stub holds the exports map and is a
    /// real chunk; plain CSS stubs are dropped after extraction).
    pub is_modules: bool,
}

/// A module loaded for bundling.
#[derive(Debug, Clone)]
pub struct LoadedModule {
    /// Module id.
    pub id: ModuleId,
    /// Final code (transformed, defines applied).
    pub code: String,
    /// Resolved imports (`(specifier, id, kind)`).
    pub imports: Vec<(String, ModuleId, ImportKind)>,
    /// Package `sideEffects` flag, when known.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Source map JSON, when produced.
    pub map: Option<String>,
    /// Extracted CSS (concatenated per entry into hashed `.css` assets).
    pub css: Option<CssExtract>,
    /// Statement-DCE facts (`None` = opaque, keep everything).
    pub shake: Option<ferrite_transform::ShakeInfo>,
}

/// Loads transformed modules for the bundler (implemented by the facade).
#[async_trait::async_trait]
pub trait ModuleLoader: Send + Sync {
    /// Load + transform a module for `env` (`client`/`ssr`).
    async fn load(&self, id: &ModuleId, env: &str) -> Result<LoadedModule>;
}
