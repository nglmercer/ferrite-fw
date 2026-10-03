//! Dev server pipeline data types.

use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_graph::ImportKind;

/// Compiled stylesheet retained separately from its JavaScript wrapper.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PipelineStylesheet {
    /// CSS before browser URL rewriting, ready for build asset emission.
    pub code: String,
    /// CSS module exports, in deterministic order.
    pub exports: std::collections::BTreeMap<String, String>,
    /// Whether the stylesheet uses CSS module scoping.
    pub is_modules: bool,
    /// Source map supplied by source/compiler transforms, before CSS lowering.
    pub input_map: Option<String>,
}

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
    /// Compiled CSS independent of JavaScript wrapper transforms.
    pub stylesheet: Option<PipelineStylesheet>,
    /// True when the code uses ESM syntax.
    pub has_module_syntax: bool,
    /// True when `import.meta.hot` is used.
    pub uses_import_meta_hot: bool,
    /// True when the response must be raw file bytes.
    pub is_raw_bytes: bool,
    /// Compiler/preprocessor files whose changes invalidate this module.
    pub dependencies: Vec<String>,
    /// Statement-DCE facts (`None` = opaque, keep everything).
    pub shake: Option<ferrite_transform::ShakeInfo>,
    /// Static CommonJS facts retained for facade export discovery.
    pub commonjs: Option<CommonJsMetadata>,
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
            stylesheet: None,
            has_module_syntax: true,
            uses_import_meta_hot: false,
            is_raw_bytes: false,
            dependencies: Vec::new(),
            shake: None,
            commonjs: None,
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
            stylesheet: None,
            has_module_syntax: false,
            uses_import_meta_hot: false,
            is_raw_bytes: true,
            dependencies: Vec::new(),
            shake: None,
            commonjs: None,
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
            stylesheet: cached.stylesheet,
            has_module_syntax: cached.has_module_syntax,
            uses_import_meta_hot: cached.uses_import_meta_hot,
            is_raw_bytes: false,
            dependencies: cached.dependency_state.keys().cloned().collect(),
            shake: cached.shake,
            commonjs: cached.commonjs,
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
    /// Compiled stylesheet payload.
    #[serde(default)]
    pub stylesheet: Option<PipelineStylesheet>,
    /// Exact compiler input dependency state (missing files are represented explicitly).
    #[serde(default)]
    pub dependency_state: std::collections::BTreeMap<String, Option<String>>,
    #[serde(default)]
    pub has_module_syntax: bool,
    /// HMR flag.
    pub uses_import_meta_hot: bool,
    /// Statement-DCE facts (`None` for stale caches → keep everything).
    #[serde(default)]
    pub shake: Option<ferrite_transform::ShakeInfo>,
    #[serde(default)]
    pub commonjs: Option<CommonJsMetadata>,
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
            stylesheet: module.stylesheet.clone(),
            dependency_state: module
                .dependencies
                .iter()
                .map(|path| (path.clone(), dependency_hash(path)))
                .collect(),
            has_module_syntax: module.has_module_syntax,
            uses_import_meta_hot: module.uses_import_meta_hot,
            shake: module.shake.clone(),
            commonjs: module.commonjs.clone(),
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

/// Missing dependencies must differ from newly created files, even when empty.
pub(crate) fn dependency_hash(path: &str) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|bytes| ferrite_core::Hash::of_bytes(&bytes).0)
}

impl CachedTransform {
    pub(crate) fn dependencies_current(&self) -> bool {
        self.dependency_state
            .iter()
            .all(|(path, hash)| dependency_hash(path) == *hash)
    }
}

/// Statically discovered export names and resolved require re-exports.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CommonJsMetadata {
    /// Statically discovered property names.
    pub names: std::collections::BTreeSet<String>,
    /// Resolved dependency module IDs (without the factory query).
    pub reexports: Vec<String>,
}
