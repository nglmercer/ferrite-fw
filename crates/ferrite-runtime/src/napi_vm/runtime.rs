//! napi-vm runtime backend.

use super::jobs::*;
use super::options::*;
use super::worker::*;
use crate::js_value_to_json;
use crate::CompiledModule;
use crate::JsHandle;
use crate::JsRuntime;
use crate::JsValue;
use crate::ModuleNamespace;
use crate::RuntimeEnvironment;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

/// napi-vm [`JsRuntime`] backend.
#[derive(Clone)]
pub struct NapiVmRuntime {
    worker: Arc<Worker>,
}

impl NapiVmRuntime {
    /// Spawn the worker thread with `options`.
    #[must_use]
    pub fn new(options: NapiVmOptions) -> Self {
        let (jobs, rx) = mpsc::channel::<Job>();
        let thread = thread::Builder::new()
            .name("ferrite-napi-vm".to_string())
            .spawn(move || worker_loop(options, rx))
            .expect("spawn napi-vm worker");
        Self {
            worker: Arc::new(Worker {
                jobs: Some(jobs),
                thread: Mutex::new(Some(thread)),
            }),
        }
    }

    /// Default options (no CJS roots, no native addons).
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(NapiVmOptions::default())
    }

    /// CJS-require `request` (JS, JSON, or an allowlisted `.node` binary).
    pub async fn require(&self, request: &str, parent: Option<&str>) -> Result<JsValue> {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::Require {
            request: request.to_string(),
            parent: parent.map(str::to_string),
            reply,
        })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".to_string()))?
            .map_err(FerriteError::Runtime)?;
        Ok(JsValue::from(json))
    }

    fn send(&self, job: Job) -> Result<()> {
        self.worker
            .jobs
            .as_ref()
            .ok_or_else(|| FerriteError::Runtime("napi-vm worker stopped".to_string()))?
            .send(job)
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".to_string()))
    }
}

#[async_trait::async_trait]
impl JsRuntime for NapiVmRuntime {
    fn name(&self) -> &'static str {
        "napi-vm"
    }

    async fn evaluate_module(
        &self,
        module: CompiledModule,
        _env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::EvalModule {
            id: module.id,
            code: module.code,
            reply,
        })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".to_string()))?
            .map_err(FerriteError::Runtime)?;
        namespace_from_json(&json)
    }

    async fn evaluate_module_graph(
        &self,
        graph: crate::CompiledModuleGraph,
        _env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::EvalGraph { graph, reply })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".into()))?
            .map_err(FerriteError::Runtime)?;
        namespace_from_json(&json)
    }

    async fn call(&self, handle: &JsHandle, args: Vec<JsValue>) -> Result<JsValue> {
        let mut encoded = Vec::with_capacity(args.len());
        for arg in &args {
            encoded.push(js_value_to_json(arg)?);
        }
        let args_json = serde_json::to_string(&encoded).map_err(FerriteError::Json)?;
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::Call {
            handle: handle.id,
            args_json,
            reply,
        })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".to_string()))?
            .map_err(FerriteError::Runtime)?;
        Ok(JsValue::from(json))
    }
}

/// Build a namespace from the worker's export record.
pub(crate) fn namespace_from_json(json: &serde_json::Value) -> Result<ModuleNamespace> {
    let mut namespace = ModuleNamespace::default();
    let object = json
        .as_object()
        .ok_or_else(|| FerriteError::Runtime("napi-vm: malformed export record".to_string()))?;
    for (name, entry) in object {
        let value = match entry.get("handle").and_then(serde_json::Value::as_u64) {
            Some(id) => JsValue::Handle(JsHandle {
                id,
                label: Some(format!("function {name}")),
            }),
            None => entry
                .get("value")
                .cloned()
                .map(JsValue::from)
                .unwrap_or(JsValue::Undefined),
        };
        namespace.exports.insert(name.clone(), value);
    }
    Ok(namespace)
}

// --- worker ---------------------------------------------------------------
