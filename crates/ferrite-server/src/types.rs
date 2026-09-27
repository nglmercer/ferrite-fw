//! Dev server pipeline data types.

use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_graph::ImportKind;

/// A transformed module in the dev/build pipeline.
#[derive(Debug, Clone)]
pub struct PipelineModule {
    /// Module id.
    pub id: ModuleId,
    /// Final code.
    pub code: String,
    /// Resolved imports.
    pub imports: Vec<(String, ModuleId, ImportKind)>,
    /// Package `sideEffects`, when known.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Source map JSON.
    pub map: Option<String>,
    /// True when the code uses ESM syntax.
    pub has_module_syntax: bool,
    /// True when `import.meta.hot` is used.
    pub uses_import_meta_hot: bool,
    /// True when the response must be raw file bytes.
    pub is_raw_bytes: bool,
    /// Statement-DCE facts (`None` = opaque, keep everything).
    pub shake: Option<ferrite_transform::ShakeInfo>,
}

impl PipelineModule {
    /// Code-only module.
    #[must_use]
    pub fn code_only(id: ModuleId, code: String, module_type: ModuleType) -> Self {
        Self {
            id,
            code,
            imports: Vec::new(),
            side_effects: None,
            module_type,
            map: None,
            has_module_syntax: true,
            uses_import_meta_hot: false,
            is_raw_bytes: false,
            shake: None,
        }
    }

    /// Raw-bytes module (served verbatim).
    #[must_use]
    pub fn raw(id: ModuleId, module_type: ModuleType) -> Self {
        Self {
            id,
            code: String::new(),
            imports: Vec::new(),
            side_effects: None,
            module_type,
            map: None,
            has_module_syntax: false,
            uses_import_meta_hot: false,
            is_raw_bytes: true,
            shake: None,
        }
    }

    /// Rebuild from cache.
    #[must_use]
    pub fn from_cached(id: ModuleId, cached: CachedTransform) -> Self {
        Self {
            id,
            code: cached.code,
            imports: cached
                .imports
                .into_iter()
                .map(|(specifier, resolved, kind)| (specifier, ModuleId::new(resolved), kind))
                .collect(),
            side_effects: cached.side_effects,
            module_type: cached.module_type,
            map: cached.map,
            has_module_syntax: true,
            uses_import_meta_hot: cached.uses_import_meta_hot,
            is_raw_bytes: false,
            shake: cached.shake,
        }
    }
}

/// Cached transform payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedTransform {
    /// Code.
    pub code: String,
    /// Imports (`specifier`, `resolved`, `kind`).
    pub imports: Vec<(String, String, ImportKind)>,
    /// Side effects.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Map JSON.
    pub map: Option<String>,
    /// HMR flag.
    pub uses_import_meta_hot: bool,
    /// Statement-DCE facts (`None` for stale caches → keep everything).
    #[serde(default)]
    pub shake: Option<ferrite_transform::ShakeInfo>,
}

impl CachedTransform {
    /// Snapshot a module.
    #[must_use]
    pub fn from_module(module: &PipelineModule) -> Self {
        Self {
            code: module.code.clone(),
            imports: module
                .imports
                .iter()
                .map(|(specifier, id, kind)| (specifier.clone(), id.0.clone(), kind.clone()))
                .collect(),
            side_effects: module.side_effects,
            module_type: module.module_type.clone(),
            map: module.map.clone(),
            uses_import_meta_hot: module.uses_import_meta_hot,
            shake: module.shake.clone(),
        }
    }
}

/// An HTTP-ready pipeline response.
#[derive(Debug)]
pub struct PipelineResponse {
    /// Body bytes.
    pub body: Vec<u8>,
    /// Content type.
    pub content_type: String,
}

impl PipelineResponse {
    /// Text response.
    #[must_use]
    pub fn code(code: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            body: code.into().into_bytes(),
            content_type: content_type.into(),
        }
    }

    /// Binary response.
    #[must_use]
    pub fn bytes(bytes: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            body: bytes,
            content_type: content_type.into(),
        }
    }
}
