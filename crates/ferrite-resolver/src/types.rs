//! Resolver request/response types.

use ferrite_core::EnvironmentKind;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;

/// What kind of import triggered resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveKind {
    /// Static ESM import / re-export.
    Import,
    /// CommonJS `require()`.
    Require,
    /// Dynamic `import()`.
    DynamicImport,
    /// CSS `@import` / CSS file import.
    Css,
    /// `?url` / `new URL(..., import.meta.url)`.
    Url,
    /// Worker import.
    Worker,
    /// WASM import.
    Wasm,
}

/// A resolution request (§15).
#[derive(Debug, Clone)]
pub struct ResolveRequest<'a> {
    /// Raw specifier text.
    pub specifier: &'a str,
    /// Importing module, if any.
    pub importer: Option<&'a ModuleId>,
    /// Target environment.
    pub environment: EnvironmentKind,
    /// Import kind.
    pub kind: ResolveKind,
}

/// A resolved module id (§15).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedId {
    /// Resolved module id.
    pub id: ModuleId,
    /// True when the module must not be bundled/transformed.
    pub external: bool,
    /// Package `sideEffects` flag, when known.
    pub side_effects: Option<bool>,
    /// Detected module type, when known.
    pub module_type: Option<ModuleType>,
    /// Extra metadata (`node_builtin`, `remote`, `package`, ...).
    pub meta: serde_json::Value,
}

impl ResolvedId {
    /// Create an internal resolved id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: ModuleId::new(id.into()),
            external: false,
            side_effects: None,
            module_type: None,
            meta: serde_json::Value::Null,
        }
    }

    /// Create an external resolved id.
    #[must_use]
    pub fn external(id: impl Into<String>) -> Self {
        let mut resolved = Self::new(id);
        resolved.external = true;
        resolved
    }

    /// Attach metadata.
    #[must_use]
    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = meta;
        self
    }
}

/// Minimal `package.json` view used by the resolver.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PackageJson {
    /// Package name.
    pub name: Option<String>,
    /// Package version.
    pub version: Option<String>,
    /// CJS entry.
    pub main: Option<String>,
    /// ESM entry.
    pub module: Option<String>,
    /// Export map.
    pub exports: Option<serde_json::Value>,
    /// Import map (`#` specifiers).
    pub imports: Option<serde_json::Value>,
    /// Side-effects flag.
    #[serde(rename = "sideEffects")]
    pub side_effects: Option<SideEffects>,
    /// Whether the package is ESM (`"type": "module"`).
    #[serde(rename = "type")]
    pub package_type: Option<String>,
}

/// `sideEffects` field shapes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SideEffects {
    /// Boolean form.
    Bool(bool),
    /// Glob list form.
    Globs(Vec<String>),
}
