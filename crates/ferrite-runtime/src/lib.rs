//! JavaScript execution abstraction (spec §20, §84–§87).
//!
//! [`JsRuntime`] keeps Ferrite independent of any single JS engine: embed
//! QuickJS/V8/Boa behind this trait, run pure-Rust SSR without one, or bridge
//! to an external adapter. v0.1 ships the trait, value bridge, runtime pool,
//! and honest backend stubs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ferrite_core::{FerriteError, Result};

/// A value crossing the Rust ↔ JS boundary (§87).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum JsValue {
    /// `undefined`.
    Undefined,
    /// `null`.
    Null,
    /// Boolean.
    Bool(bool),
    /// Number.
    Number(f64),
    /// String.
    String(String),
    /// Array.
    Array(Vec<JsValue>),
    /// Object (insertion-ordered).
    Object(indexmap::IndexMap<String, JsValue>),
    /// Raw bytes (buffer).
    Bytes(Vec<u8>),
    /// Opaque handle to a JS-side object/function.
    Handle(JsHandle),
}

impl JsValue {
    /// True for null/undefined.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Null | Self::Undefined)
    }
}

impl From<serde_json::Value> for JsValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Bool(value),
            serde_json::Value::Number(number) => Self::Number(number.as_f64().unwrap_or(f64::NAN)),
            serde_json::Value::String(value) => Self::String(value),
            serde_json::Value::Array(items) => {
                Self::Array(items.into_iter().map(Self::from).collect())
            }
            serde_json::Value::Object(map) => Self::Object(
                map.into_iter()
                    .map(|(key, value)| (key, Self::from(value)))
                    .collect(),
            ),
        }
    }
}

/// Opaque handle to a JS-side value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JsHandle {
    /// Runtime-local handle id.
    pub id: u64,
    /// Debug label (e.g. `function render`).
    pub label: Option<String>,
}

/// A compiled module ready for evaluation.
#[derive(Debug, Clone)]
pub struct CompiledModule {
    /// Module id.
    pub id: String,
    /// Executable code.
    pub code: String,
    /// Source URL for stack traces.
    pub url: Option<String>,
}

/// Runtime environment flags.
#[derive(Debug, Clone)]
pub struct RuntimeEnvironment {
    /// True for SSR evaluation.
    pub ssr: bool,
    /// Request id (for request-local globals).
    pub request_id: Option<String>,
}

/// An evaluated module namespace.
#[derive(Debug, Clone, Default)]
pub struct ModuleNamespace {
    /// Exported bindings.
    pub exports: indexmap::IndexMap<String, JsValue>,
}

impl ModuleNamespace {
    /// Look up an export.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&JsValue> {
        self.exports.get(name)
    }

    /// Look up a function export (must be a handle).
    pub fn get_function(&self, name: &str) -> Result<&JsHandle> {
        match self.exports.get(name) {
            Some(JsValue::Handle(handle)) => Ok(handle),
            Some(_) => Err(FerriteError::Runtime(format!(
                "export `{name}` is not a function"
            ))),
            None => Err(FerriteError::Runtime(format!("export `{name}` not found"))),
        }
    }
}

/// JS engine abstraction (§20).
#[async_trait::async_trait]
pub trait JsRuntime: Send + Sync {
    /// Backend name (`quickjs`, `v8`, `boa`, `native`, ...).
    fn name(&self) -> &'static str;

    /// Evaluate a module and return its namespace.
    async fn evaluate_module(
        &self,
        module: CompiledModule,
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace>;

    /// Call a function handle with arguments.
    async fn call(&self, _handle: &JsHandle, _args: Vec<JsValue>) -> Result<JsValue> {
        Err(FerriteError::Runtime(format!(
            "backend `{}` does not implement call()",
            self.name()
        )))
    }
}

/// Backend that is not compiled into this build (§20).
///
/// Returned when SSR requests embedded JS execution without an engine.
/// Pure-Rust SSR adapters do not need this.
#[derive(Debug, Default)]
pub struct UnavailableRuntime {
    /// Requested backend name.
    pub backend: String,
}

#[async_trait::async_trait]
impl JsRuntime for UnavailableRuntime {
    fn name(&self) -> &'static str {
        "unavailable"
    }

    async fn evaluate_module(
        &self,
        module: CompiledModule,
        _env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        Err(FerriteError::Runtime(format!(
            "cannot evaluate `{}`: embedded JS backend `{}` is not available in this build; \
             use a Rust SSR adapter (pure Rust SSR) or enable a JS engine backend",
            module.id, self.backend
        )))
    }
}

/// Pool of runtime workers (§84): reuse contexts across requests.
pub struct RuntimePool {
    /// Pooled runtimes (round-robin).
    runtimes: Vec<Arc<dyn JsRuntime>>,
    /// Round-robin cursor.
    cursor: AtomicU64,
}

impl RuntimePool {
    /// Create a pool from runtimes (must be non-empty).
    #[must_use]
    pub fn new(runtimes: Vec<Arc<dyn JsRuntime>>) -> Self {
        assert!(
            !runtimes.is_empty(),
            "runtime pool needs at least one runtime"
        );
        Self {
            runtimes,
            cursor: AtomicU64::new(0),
        }
    }

    /// Create a single-runtime pool.
    #[must_use]
    pub fn single(runtime: Arc<dyn JsRuntime>) -> Self {
        Self::new(vec![runtime])
    }

    /// Pick the next runtime (round-robin).
    #[must_use]
    pub fn pick(&self) -> Arc<dyn JsRuntime> {
        let index = self.cursor.fetch_add(1, Ordering::Relaxed) as usize % self.runtimes.len();
        self.runtimes[index].clone()
    }

    /// Pool size.
    #[must_use]
    pub fn len(&self) -> usize {
        self.runtimes.len()
    }

    /// True when empty (never for a constructed pool).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runtimes.is_empty()
    }

    /// Evaluate a module on the next runtime.
    pub async fn evaluate_module(
        &self,
        module: CompiledModule,
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        self.pick().evaluate_module(module, env).await
    }
}

/// Controlled SSR globals (§85). Documents what embedded runtimes provide;
// Nothing here pretends every Node API exists.
#[derive(Debug, Clone)]
pub struct SsrGlobals {
    /// Enabled globals.
    pub enabled: Vec<String>,
}

impl Default for SsrGlobals {
    fn default() -> Self {
        Self {
            enabled: [
                "console",
                "URL",
                "URLSearchParams",
                "TextEncoder",
                "TextDecoder",
                "crypto",
                "fetch",
                "Headers",
                "Request",
                "Response",
                "setTimeout",
                "clearTimeout",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        }
    }
}

/// `fetch()` bridge request (JS → Rust HTTP, §86).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchRequest {
    /// URL.
    pub url: String,
    /// Method.
    pub method: String,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Option<Vec<u8>>,
}

/// `fetch()` bridge response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FetchResponse {
    /// Status code.
    pub status: u16,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_bridge() {
        let value = JsValue::from(serde_json::json!({"a": [1, null]}));
        assert!(matches!(value, JsValue::Object(_)));
    }

    #[tokio::test]
    async fn unavailable_errors_clearly() {
        let runtime = UnavailableRuntime {
            backend: "quickjs".to_string(),
        };
        let error = runtime
            .evaluate_module(
                CompiledModule {
                    id: "x".to_string(),
                    code: String::new(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("quickjs"));
    }
}
