//! Concat input/output types.

use crate::ShakeInfo;

/// One import edge, aligned with `ShakeInfo.import_bindings`.
#[derive(Debug, Clone)]
pub struct ConcatImport {
    /// Specifier text as it appears in `code` (resolved URL).
    pub specifier: String,
    /// Resolved target module id.
    pub target: String,
    /// True for static imports (only these dissolve).
    pub is_static: bool,
}

/// One module of the concat closure, in emit order.
#[derive(Debug, Clone)]
pub struct ConcatModule {
    /// Module id.
    pub id: String,
    /// Loaded code (transformed, minified).
    pub code: String,
    /// Statement-DCE facts.
    pub shake: ShakeInfo,
    /// Import edges (aligned with `shake.import_bindings`).
    pub imports: Vec<ConcatImport>,
    /// True for the chunk entry (exactly one).
    pub is_entry: bool,
    /// Map JSON (`code` → original), when the loader produced one.
    pub input_map: Option<String>,
}

/// Concatenated chunk.
#[derive(Debug, Clone)]
pub struct ConcatOutput {
    /// Single-scope ESM code.
    pub code: String,
    /// Concatenated map JSON, when requested.
    pub map: Option<String>,
    /// Entry export names preserved.
    pub exports: Vec<String>,
}
