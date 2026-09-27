//! Transform request/response types.

use ferrite_core::EnvironmentKind;
use ferrite_core::ModuleType;
use ferrite_core::SourceMap;
use ferrite_core::Target;
use std::collections::HashMap;

/// Parse request.
#[derive(Debug, Clone)]
pub struct ParseRequest {
    /// Module id (used for source-type inference).
    pub id: String,
    /// Source code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
}

/// A parsed import specifier with its source range.
#[derive(Debug, Clone)]
pub struct ParsedImport {
    /// Raw specifier text (unquoted).
    pub specifier: String,
    /// Byte range of the quoted literal in the source.
    pub range: (usize, usize),
    /// Static import/re-export vs dynamic `import()`.
    pub kind: ParsedImportKind,
    /// True for `import type ...` (elided at runtime).
    pub is_type: bool,
    /// Bindings pulled through this import (empty = unknown: treat as all).
    pub bindings: Vec<ImportBinding>,
}

/// Static vs dynamic import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedImportKind {
    /// `import`, `export ... from`.
    Static,
    /// `import()`.
    Dynamic,
}

/// One binding pulled through an import (statement-level DCE, §38).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportBinding {
    /// `import { name }` (the DEP-side exported name).
    Named(String),
    /// `import name` (dep `default`).
    Default,
    /// `import * as ns` (uses everything).
    Namespace,
    /// Bare `import "…"` (uses nothing).
    SideEffect,
    /// `export … from "…"` (usage propagates, not direct use).
    Reexport,
}

/// One export with its local binding (§38).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedExport {
    /// Exported name (`default`, or `*` for star re-exports).
    pub exported: String,
    /// Local name, when locally declared.
    pub local: Option<String>,
    /// Source specifier, when re-exported.
    pub from: Option<String>,
    /// Source-side name, when re-exported (`export { a as b }` → `a`).
    pub imported: Option<String>,
    /// Resolved source module, filled by the pipeline (not the parser).
    #[serde(default)]
    pub target: Option<ferrite_core::ModuleId>,
}

/// Statement-DCE facts for one module (§38). `None` (stale caches,
/// non-JS shims) means *opaque*: consumers must keep everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ShakeInfo {
    /// Bindings per resolved import, aligned with the import list.
    pub import_bindings: Vec<Vec<ImportBinding>>,
    /// Declared exports with re-export provenance.
    pub exports: Vec<ParsedExport>,
}

/// A parsed module (§5, decoupled from compiler ASTs).
#[derive(Debug, Clone)]
pub struct ParsedModule {
    /// Module id.
    pub id: String,
    /// Extracted imports with AST ranges.
    pub imports: Vec<ParsedImport>,
    /// Exported names (`default` for default exports, `*` for star re-exports).
    pub exports: Vec<String>,
    /// Export details with local bindings (statement-level DCE).
    pub export_details: Vec<ParsedExport>,
    /// True when the module uses ESM syntax.
    pub has_module_syntax: bool,
    /// True when `import.meta.hot` is referenced.
    pub uses_import_meta_hot: bool,
    /// True when `import.meta.env` is referenced.
    pub uses_import_meta_env: bool,
}

/// Transform request.
#[derive(Debug, Clone)]
pub struct TransformRequest {
    /// Module id.
    pub id: String,
    /// Source code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
    /// Target environment.
    pub environment: EnvironmentKind,
    /// True for SSR transforms.
    pub ssr: bool,
    /// Compilation target.
    pub target: Target,
    /// Minify after transforming.
    pub minify: bool,
    /// Emit a source map.
    pub sourcemap: bool,
    /// Compile-time defines.
    pub define: HashMap<String, String>,
    /// JSX runtime (`automatic` or `classic`).
    pub jsx_runtime: String,
    /// Enable development helpers (JSX dev, refresh preamble hooks).
    pub development: bool,
}

impl TransformRequest {
    /// Minimal request for tests and one-shot transforms.
    #[must_use]
    pub fn new(id: impl Into<String>, code: impl Into<String>, module_type: ModuleType) -> Self {
        Self {
            id: id.into(),
            code: code.into(),
            module_type,
            environment: EnvironmentKind::Client,
            ssr: false,
            target: Target::default(),
            minify: false,
            sourcemap: false,
            define: HashMap::new(),
            jsx_runtime: "automatic".to_string(),
            development: true,
        }
    }
}

/// Transform result (§41).
#[derive(Debug, Clone)]
pub struct TransformResult {
    /// Transformed code.
    pub code: String,
    /// Source map, when requested.
    pub map: Option<SourceMap>,
    /// Extra file dependencies discovered during transform.
    pub dependencies: Vec<String>,
    /// Imports extracted from the transformed code.
    pub imports: Vec<ParsedImport>,
    /// Exported names.
    pub exports: Vec<String>,
}

/// Minify request.
#[derive(Debug, Clone)]
pub struct MinifyRequest {
    /// Module id.
    pub id: String,
    /// Source code.
    pub code: String,
    /// Emit a source map (chained through `input_map` when present).
    pub sourcemap: bool,
    /// Map from a previous step (`code` → original); chained, not dropped.
    pub input_map: Option<SourceMap>,
}

/// Minify result.
#[derive(Debug, Clone)]
pub struct MinifyResult {
    /// Minified code.
    pub code: String,
    /// Source map, when requested.
    pub map: Option<SourceMap>,
}
