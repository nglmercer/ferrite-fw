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
        let capacity = if options.queue_capacity == 0 {
            64
        } else {
            options.queue_capacity
        };
        let max_request_bytes = if options.max_request_bytes == 0 {
            8 * 1024 * 1024
        } else {
            options.max_request_bytes
        };
        let (jobs, rx) = mpsc::sync_channel::<Job>(capacity);
        let thread = thread::Builder::new()
            .name("ferrite-napi-vm".to_string())
            .spawn(move || worker_loop(options, rx))
            .expect("spawn napi-vm worker");
        Self {
            worker: Arc::new(Worker {
                jobs: Some(jobs),
                max_request_bytes,
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
        if job.payload_bytes() > self.worker.max_request_bytes {
            return Err(FerriteError::Runtime(format!("napi-vm request payload exceeds {} bytes; reduce the graph/arguments or explicitly configure max_request_bytes", self.worker.max_request_bytes)));
        }
        self.worker.jobs.as_ref().ok_or_else(|| FerriteError::Runtime("napi-vm worker stopped".into()))?
            .try_send(job).map_err(|error| match error {
                mpsc::TrySendError::Full(_) => FerriteError::Runtime("napi-vm worker queue is full; retry after active requests finish or explicitly configure queue_capacity".into()),
                mpsc::TrySendError::Disconnected(_) => FerriteError::Runtime("napi-vm worker stopped".into()),
            })
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
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::EvalModule {
            id: module.id,
            code: module.code,
            ssr: env.ssr,
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
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::EvalGraph {
            graph,
            ssr: env.ssr,
            reply,
        })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".into()))?
            .map_err(FerriteError::Runtime)?;
        namespace_from_json(&json)
    }

    async fn invoke_module_graph(
        &self,
        graph: crate::CompiledModuleGraph,
        export: &str,
        args: Vec<JsValue>,
        env: RuntimeEnvironment,
    ) -> Result<JsValue> {
        graph.validate()?;
        let encoded: Result<Vec<_>> = args.iter().map(js_value_to_json).collect();
        let args_json = serde_json::to_string(&encoded?).map_err(FerriteError::Json)?;
        let (reply, receive) = tokio::sync::oneshot::channel();
        self.send(Job::InvokeGraph {
            graph,
            export: export.to_string(),
            args_json,
            ssr: env.ssr,
            reply,
        })?;
        let json = receive
            .await
            .map_err(|_| FerriteError::Runtime("napi-vm worker stopped".into()))?
            .map_err(FerriteError::Runtime)?;
        Ok(JsValue::from(json))
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

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn module_job(code: &str) -> Job {
        let (reply, _receive) = tokio::sync::oneshot::channel();
        Job::EvalModule {
            id: "entry".into(),
            code: code.into(),
            ssr: false,
            reply,
        }
    }

    #[test]
    fn bounded_queue_reports_overload_and_payload_limits_without_blocking() {
        let (jobs, receiver) = mpsc::sync_channel(1);
        let runtime = NapiVmRuntime {
            worker: Arc::new(Worker {
                jobs: Some(jobs),
                max_request_bytes: 32,
                thread: Mutex::new(None),
            }),
        };
        runtime.send(module_job("export const n = 1;")).unwrap();
        let error = runtime.send(module_job("export const n = 2;")).unwrap_err();
        assert!(error.to_string().contains("queue is full"), "{error}");
        let error = runtime.send(module_job(&"x".repeat(64))).unwrap_err();
        assert!(error.to_string().contains("payload exceeds 32"), "{error}");
        drop(receiver);
        let error = runtime.send(module_job("x")).unwrap_err();
        assert!(error.to_string().contains("stopped"), "{error}");
    }
}
