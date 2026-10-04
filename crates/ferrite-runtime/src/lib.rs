//! JavaScript execution abstraction (spec §20, §84–§87).
//!
//! [`JsRuntime`] keeps Ferrite independent of any single JS engine: embed
//! QuickJS/V8/Boa behind this trait, run pure-Rust SSR without one, or bridge
//! to an external adapter. v0.1 ships the trait, value bridge, runtime pool,
//! and honest backend stubs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ferrite_core::{FerriteError, Result};

/// napi-vm embedded backend (feature `napi-vm`, disabled by default).
#[cfg(feature = "napi-vm")]
pub mod napi_vm;

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledModule {
    /// Module id.
    pub id: String,
    /// Executable code.
    pub code: String,
    /// Source URL for stack traces.
    pub url: Option<String>,
}

/// A complete compiled module graph supplied by the compilation pipeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledModuleGraph {
    /// Canonical entry module ID.
    pub entry: String,
    /// Compiled modules, including the entry and its dependencies.
    pub modules: Vec<CompiledModule>,
}

impl CompiledModuleGraph {
    /// Validate identities before registering any guest module.
    pub fn validate(&self) -> Result<()> {
        let mut ids = std::collections::HashSet::new();
        for module in &self.modules {
            if module.id.is_empty() || module.id.starts_with('.') {
                return Err(FerriteError::Runtime(format!(
                    "compiled graph module ID `{}` must be absolute or a stable name",
                    module.id
                )));
            }
            if !ids.insert(&module.id) {
                return Err(FerriteError::Runtime(format!(
                    "duplicate compiled graph module ID `{}`",
                    module.id
                )));
            }
        }
        if !ids.contains(&self.entry) {
            return Err(FerriteError::Runtime(format!(
                "compiled graph entry `{}` is missing",
                self.entry
            )));
        }
        Ok(())
    }
}

/// Runtime environment flags.
#[derive(Debug, Clone)]
pub struct RuntimeEnvironment {
    /// True for SSR evaluation.
    pub ssr: bool,
    /// Request id (for request-local globals).
    pub request_id: Option<String>,
}

/// Convert a bridge value to its plain-JSON equivalent for guest calls.
///
/// Explicit untagged mapping (the derived `Serialize` form is externally
/// tagged and would cross as `{"String": ...}` instead of `"..."`).
/// Handles cannot cross and are an error.
pub fn js_value_to_json(value: &JsValue) -> Result<serde_json::Value> {
    match value {
        JsValue::Undefined | JsValue::Null => Ok(serde_json::Value::Null),
        JsValue::Bool(value) => Ok(serde_json::Value::Bool(*value)),
        JsValue::Number(value) => Ok(serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null)),
        JsValue::String(value) => Ok(serde_json::Value::String(value.clone())),
        JsValue::Array(items) => Ok(serde_json::Value::Array(
            items
                .iter()
                .map(js_value_to_json)
                .collect::<Result<Vec<_>>>()?,
        )),
        JsValue::Object(map) => {
            let mut object = serde_json::Map::with_capacity(map.len());
            for (key, item) in map {
                object.insert(key.clone(), js_value_to_json(item)?);
            }
            Ok(serde_json::Value::Object(object))
        }
        // Bytes cross as number arrays (lossless plain JSON).
        JsValue::Bytes(bytes) => Ok(serde_json::Value::Array(
            bytes
                .iter()
                .map(|byte| serde_json::Value::Number((*byte).into()))
                .collect(),
        )),
        JsValue::Handle(handle) => Err(FerriteError::Runtime(format!(
            "cannot pass guest handle {} as a call argument",
            handle.id
        ))),
    }
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

    /// Evaluate an explicitly compiled graph without substituting another host.
    async fn evaluate_module_graph(
        &self,
        graph: CompiledModuleGraph,
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        if graph.modules.len() == 1 {
            return self
                .evaluate_module(
                    graph.modules.into_iter().next().expect("validated graph"),
                    env,
                )
                .await;
        }
        Err(FerriteError::Runtime(format!("backend `{}` does not support compiled module graphs; select a graph-capable runtime explicitly", self.name())))
    }

    /// Evaluate a compiled graph and invoke an entry export as one atomic
    /// worker operation. Backends must implement this explicitly; composing
    /// separate evaluation/call jobs does not provide request isolation.
    async fn invoke_module_graph(
        &self,
        graph: CompiledModuleGraph,
        _export: &str,
        _args: Vec<JsValue>,
        _env: RuntimeEnvironment,
    ) -> Result<JsValue> {
        graph.validate()?;
        Err(FerriteError::Runtime(format!(
            "backend `{}` does not support atomic compiled graph invocation",
            self.name()
        )))
    }

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

    async fn invoke_module_graph(
        &self,
        graph: CompiledModuleGraph,
        _export: &str,
        _args: Vec<JsValue>,
        _env: RuntimeEnvironment,
    ) -> Result<JsValue> {
        graph.validate()?;
        Err(FerriteError::Runtime(format!(
            "backend `{}` is unavailable for atomic compiled graph invocation",
            self.backend
        )))
    }

    async fn evaluate_module_graph(
        &self,
        graph: CompiledModuleGraph,
        _env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        Err(FerriteError::Runtime(format!("cannot evaluate compiled module graphs: embedded JS backend `{}` is unavailable in this build; select a graph-capable runtime explicitly", self.backend)))
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

/// Whether this build contains a backend for compiled graph invocation.
/// This metadata check starts no worker and executes no guest code. It does not
/// establish compatibility with any particular framework or language feature.
#[must_use]
pub fn compiled_graph_backend_available(backend: &str) -> bool {
    backend == "napi-vm" && cfg!(feature = "napi-vm")
}

/// Build the runtime for a backend name (`auto`/`none` need no engine).
///
/// `napi-vm` requires the `napi-vm` cargo feature; without it the returned
/// runtime explains how to enable it instead of failing at call time.
#[must_use]
pub fn runtime_for_backend(backend: &str) -> Arc<dyn JsRuntime> {
    match backend {
        #[cfg(feature = "napi-vm")]
        "napi-vm" => Arc::new(napi_vm::NapiVmRuntime::with_defaults()),
        #[cfg(not(feature = "napi-vm"))]
        "napi-vm" => Arc::new(UnavailableRuntime {
            backend: "napi-vm (rebuild with `--features napi-vm`)".to_string(),
        }),
        _ => Arc::new(UnavailableRuntime {
            backend: backend.to_string(),
        }),
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

    #[tokio::test]
    async fn compiled_graphs_validate_before_reporting_backend_capability() {
        let runtime = UnavailableRuntime {
            backend: "quickjs".into(),
        };
        let module = |id: &str| CompiledModule {
            id: id.into(),
            code: "export const value = 1;".into(),
            url: None,
        };
        let graph = CompiledModuleGraph {
            entry: "entry".into(),
            modules: vec![module("entry"), module("dependency")],
        };
        let error = runtime
            .evaluate_module_graph(
                graph.clone(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("compiled module graphs")
                && error.to_string().contains("quickjs")
        );
        let error = runtime
            .invoke_module_graph(
                graph.clone(),
                "render",
                Vec::new(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("quickjs") && error.to_string().contains("atomic"),
            "{error}"
        );
        let mut invalid = graph;
        invalid.modules[1].id = "./relative".into();
        assert!(invalid
            .validate()
            .unwrap_err()
            .to_string()
            .contains("stable name"));
        invalid.modules[1].id = String::new();
        assert!(invalid.validate().is_err());
    }

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
