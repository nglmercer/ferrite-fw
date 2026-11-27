//! QuickJS embedded backend implemented with `rquickjs`.
//!
//! This is the replacement execution engine for pure JavaScript SSR and
//! embedded compiler workloads. Native `.node` addons are intentionally not
//! supported by this backend.

use crate::{
    CompiledModuleGraph, JsHandle, JsRuntime, JsStreamResponse, JsValue, ModuleNamespace,
    RuntimeEnvironment,
};
use ferrite_core::{FerriteError, Result};
use rquickjs::function::Args;
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::{Array, Context, Function, IntoJs, Module, Object, Persistent, Runtime, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type Reply<T> = tokio::sync::oneshot::Sender<std::result::Result<T, String>>;

#[derive(Clone)]
struct ExecutionControl {
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl ExecutionControl {
    fn new(timeout_ms: u64, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            shutdown,
            deadline: (timeout_ms != 0).then(|| Instant::now() + Duration::from_millis(timeout_ms)),
        }
    }

    fn interrupted(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
            || self.shutdown.load(Ordering::Relaxed)
            || self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
    }

    fn error(&self) -> String {
        if self.cancelled.load(Ordering::Relaxed) || self.shutdown.load(Ordering::Relaxed) {
            "rquickjs request cancelled or worker shut down".into()
        } else {
            "rquickjs request deadline exceeded (including queue time)".into()
        }
    }
}

enum Job {
    EvalGraph {
        graph: CompiledModuleGraph,
        reply: Reply<serde_json::Value>,
    },
    InvokeGraph {
        graph: CompiledModuleGraph,
        export: String,
        args: Vec<JsValue>,
        reply: Reply<serde_json::Value>,
    },
    Call {
        handle: u64,
        args: Vec<JsValue>,
        reply: Reply<serde_json::Value>,
    },
}

impl Job {
    fn cancelled(&self) -> bool {
        match self {
            Self::EvalGraph { reply, .. }
            | Self::InvokeGraph { reply, .. }
            | Self::Call { reply, .. } => reply.is_closed(),
        }
    }
}

struct Request {
    job: Job,
    control: ExecutionControl,
}

struct CancelGuard(Arc<AtomicBool>);
impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Options for the embedded QuickJS runtime.
#[derive(Debug, Clone, Default)]
pub struct QuickJsOptions {
    /// End-to-end request timeout in milliseconds; zero means no timeout.
    pub request_timeout_ms: u64,
}

/// QuickJS [`JsRuntime`] backend.
#[derive(Clone)]
pub struct QuickJsRuntime {
    jobs: Arc<Mutex<Option<mpsc::SyncSender<Request>>>>,
    shutdown: Arc<AtomicBool>,
    timeout_ms: u64,
}

impl QuickJsRuntime {
    #[must_use]
    pub fn new(options: QuickJsOptions) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Request>(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        thread::Builder::new()
            .name("ferrite-rquickjs".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || worker_loop(rx, worker_shutdown))
            .expect("spawn rquickjs worker");
        Self {
            jobs: Arc::new(Mutex::new(Some(tx))),
            shutdown,
            timeout_ms: options.request_timeout_ms,
        }
    }

    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(QuickJsOptions::default())
    }

    /// Stop accepting requests and cooperatively interrupt active guest code.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.jobs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
    }

    fn send(&self, job: Job) -> Result<CancelGuard> {
        if self.shutdown.load(Ordering::Relaxed) {
            return Err(FerriteError::Runtime("rquickjs worker shut down".into()));
        }
        let control = ExecutionControl::new(self.timeout_ms, self.shutdown.clone());
        let guard = CancelGuard(control.cancelled.clone());
        self.jobs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .ok_or_else(|| FerriteError::Runtime("rquickjs worker stopped".into()))?
            .try_send(Request { job, control })
            .map_err(|error| {
                FerriteError::Runtime(format!("rquickjs worker queue unavailable: {error}"))
            })?;
        Ok(guard)
    }

    async fn receive<T>(
        &self,
        rx: tokio::sync::oneshot::Receiver<std::result::Result<T, String>>,
    ) -> Result<T> {
        rx.await
            .map_err(|_| FerriteError::Runtime("rquickjs worker stopped".into()))?
            .map_err(FerriteError::Runtime)
    }
}

#[async_trait::async_trait]
impl JsRuntime for QuickJsRuntime {
    fn name(&self) -> &'static str {
        "rquickjs"
    }

    async fn evaluate_module(
        &self,
        module: crate::CompiledModule,
        env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        self.evaluate_module_graph(
            CompiledModuleGraph {
                entry: module.id.clone(),
                modules: vec![module],
            },
            env,
        )
        .await
    }

    async fn evaluate_module_graph(
        &self,
        graph: CompiledModuleGraph,
        _env: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::EvalGraph { graph, reply: tx })?;
        let json = self.receive(rx).await?;
        namespace_from_json(&json)
    }

    async fn invoke_module_graph(
        &self,
        graph: CompiledModuleGraph,
        export: &str,
        args: Vec<JsValue>,
        _env: RuntimeEnvironment,
    ) -> Result<JsValue> {
        graph.validate()?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::InvokeGraph {
            graph,
            export: export.into(),
            args,
            reply: tx,
        })?;
        Ok(JsValue::from(self.receive::<serde_json::Value>(rx).await?))
    }

    async fn invoke_module_graph_stream(
        &self,
        graph: CompiledModuleGraph,
        _export: &str,
        _args: Vec<JsValue>,
        _env: RuntimeEnvironment,
    ) -> Result<JsStreamResponse> {
        graph.validate()?;
        Err(FerriteError::Runtime(
            "rquickjs streaming invocation is not implemented yet; use the non-streaming SSR path explicitly"
                .into(),
        ))
    }

    async fn call(&self, handle: &JsHandle, args: Vec<JsValue>) -> Result<JsValue> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::Call {
            handle: handle.id,
            args,
            reply: tx,
        })?;
        Ok(JsValue::from(self.receive::<serde_json::Value>(rx).await?))
    }
}

fn namespace_from_json(json: &serde_json::Value) -> Result<ModuleNamespace> {
    let mut namespace = ModuleNamespace::default();
    let object = json
        .as_object()
        .ok_or_else(|| FerriteError::Runtime("rquickjs malformed export record".into()))?;
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

fn explain_error(
    ctx: &rquickjs::Ctx<'_>,
    error: rquickjs::Error,
    control: &ExecutionControl,
) -> String {
    let mut detail = if matches!(error, rquickjs::Error::Exception) {
        let exception = ctx.catch();
        if let Some(exception) = exception.as_exception() {
            let message = exception
                .message()
                .unwrap_or_else(|| "uncatchable exception".into());
            let stack = exception.stack().unwrap_or_default();
            if stack.is_empty() {
                message
            } else {
                format!("{message}\n{stack}")
            }
        } else {
            format!("uncatchable or non-error exception: {exception:?}")
        }
    } else {
        format!("{error:?}")
    };
    if control.interrupted() {
        detail.push_str(&format!("; {}", control.error()));
    }
    detail
}

struct GraphResolver;
impl Resolver for GraphResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &rquickjs::Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        if !name.starts_with('.') {
            return Ok(name.into());
        }
        let mut parts: Vec<&str> = base
            .rsplit_once('/')
            .map(|parts| parts.0)
            .unwrap_or("")
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        for part in name.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                part => parts.push(part),
            }
        }
        Ok(format!("/{}", parts.join("/")))
    }
}

struct GraphLoader(Arc<Mutex<HashMap<String, String>>>);
impl Loader for GraphLoader {
    fn load<'js>(
        &mut self,
        ctx: &rquickjs::Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, rquickjs::module::Declared>> {
        let code = self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(name)
            .cloned()
            .ok_or_else(|| {
                rquickjs::Error::new_loading_message(name, "module missing from compiled graph")
            })?;
        Module::declare(ctx.clone(), name, code)
    }
}

struct Engine {
    runtime: Runtime,
    context: Context,
    handles: RefCell<HashMap<u64, Persistent<rquickjs::Function<'static>>>>,
    next_handle: Arc<AtomicU64>,
    active_control: Arc<Mutex<ExecutionControl>>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.handles.borrow_mut().clear();
    }
}

impl Engine {
    fn new(
        control: ExecutionControl,
        next_handle: Arc<AtomicU64>,
    ) -> std::result::Result<Self, String> {
        let runtime =
            Runtime::new().map_err(|error| format!("rquickjs runtime init: {error:?}"))?;
        runtime.set_max_stack_size(4 * 1024 * 1024);
        let active_control = Arc::new(Mutex::new(control));
        let interrupt_control = active_control.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            interrupt_control
                .lock()
                .map(|control| control.interrupted())
                .unwrap_or(true)
        })));
        let context =
            Context::full(&runtime).map_err(|error| format!("rquickjs context init: {error:?}"))?;
        context
            .with(|ctx| {
                ctx.eval::<(), _>(
                    r#"
if (typeof console === 'undefined') globalThis.console = {log(){},info(){},warn(){},error(){},debug(){},trace(){}};
if (typeof TextEncoder === 'undefined') {
  class FerriteTextEncoder {
    encode(input) {
      input = input === undefined ? '' : String(input);
      const bytes = [];
      for (let index = 0; index < input.length; index++) {
        let code = input.charCodeAt(index);
        if (code >= 0xd800 && code <= 0xdbff && index + 1 < input.length) {
          const next = input.charCodeAt(index + 1);
          if (next >= 0xdc00 && next <= 0xdfff) {
            code = 0x10000 + ((code - 0xd800) << 10) + next - 0xdc00;
            index++;
          }
        }
        if (code < 0x80) bytes.push(code);
        else if (code < 0x800) bytes.push(0xc0 | (code >> 6), 0x80 | (code & 63));
        else if (code < 0x10000) bytes.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
        else bytes.push(0xf0 | (code >> 18), 0x80 | ((code >> 12) & 63), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
      }
      return new Uint8Array(bytes);
    }
    encodeInto(input, destination) {
      const bytes = this.encode(input);
      const length = Math.min(bytes.length, destination.length);
      destination.set(bytes.subarray(0, length));
      return { read: length, written: length };
    }
  }
  globalThis.TextEncoder = FerriteTextEncoder;
}
if (typeof TextDecoder === 'undefined') {
  class FerriteTextDecoder {
    decode(input) {
      if (input === undefined) return '';
      const bytes = ArrayBuffer.isView(input)
        ? new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
        : new Uint8Array(input);
      let output = '';
      for (let index = 0; index < bytes.length;) {
        const first = bytes[index++];
        let code = 0, needed = 0;
        if (first < 0x80) code = first;
        else if (first >= 0xc2 && first < 0xe0) { code = first & 31; needed = 1; }
        else if (first >= 0xe0 && first < 0xf0) { code = first & 15; needed = 2; }
        else if (first >= 0xf0 && first < 0xf5) { code = first & 7; needed = 3; }
        else { output += '\ufffd'; continue; }
        let valid = index + needed <= bytes.length;
        for (let part = 0; valid && part < needed; part++) {
          const next = bytes[index + part];
          valid = (next & 0xc0) === 0x80;
          code = (code << 6) | (next & 63);
        }
        if (!valid) { output += '\ufffd'; continue; }
        index += needed;
        output += String.fromCodePoint(code);
      }
      return output;
    }
  }
  globalThis.TextDecoder = FerriteTextDecoder;
}
if (typeof structuredClone === 'undefined') {
  globalThis.structuredClone = function structuredClone(value, seen = new Map()) {
    if (value === null || typeof value !== 'object') return value;
    if (seen.has(value)) return seen.get(value);
    if (value instanceof Date) return new Date(value.getTime());
    if (ArrayBuffer.isView(value)) return new value.constructor(value);
    if (value instanceof ArrayBuffer) return value.slice(0);
    const clone = Array.isArray(value) ? [] : Object.create(Object.getPrototypeOf(value) || Object.prototype);
    seen.set(value, clone);
    for (const key of Object.keys(value)) clone[key] = structuredClone(value[key], seen);
    return clone;
  };
}
"#,
                )
            })
            .map_err(|error| format!("rquickjs host init: {error:?}"))?;
        Ok(Self {
            runtime,
            context,
            handles: RefCell::new(HashMap::new()),
            next_handle,
            active_control,
        })
    }

    fn set_control(&self, control: ExecutionControl) {
        *self
            .active_control
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = control;
    }

    fn check_control(&self) -> std::result::Result<(), String> {
        let control = self
            .active_control
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (!control.interrupted())
            .then_some(())
            .ok_or_else(|| control.error())
    }

    fn eval_graph(
        &mut self,
        graph: &CompiledModuleGraph,
        control: ExecutionControl,
    ) -> std::result::Result<serde_json::Value, String> {
        self.set_control(control.clone());
        self.check_control()?;
        let modules: HashMap<_, _> = graph
            .modules
            .iter()
            .map(|module| (module.id.clone(), module.code.clone()))
            .collect();
        self.runtime
            .set_loader(GraphResolver, GraphLoader(Arc::new(Mutex::new(modules))));
        let entry = graph
            .modules
            .iter()
            .find(|module| module.id == graph.entry)
            .ok_or_else(|| format!("entry {} missing", graph.entry))?;
        self.context.with(|ctx| {
            let (module, promise) =
                Module::declare(ctx.clone(), entry.id.clone(), entry.code.clone())
                    .and_then(Module::eval)
                    .map_err(|error| {
                        format!(
                            "rquickjs graph entry `{}` evaluation failed: {}",
                            graph.entry,
                            explain_error(&ctx, error, &control)
                        )
                    })?;
            promise.finish::<()>().map_err(|error| {
                format!(
                    "rquickjs graph entry `{}` promise failed: {}",
                    graph.entry,
                    explain_error(&ctx, error, &control)
                )
            })?;
            self.check_control()?;
            let namespace = module
                .namespace()
                .map_err(|error| format!("rquickjs namespace failed: {error:?}"))?;
            let mut out = serde_json::Map::new();
            for key in namespace.keys::<String>() {
                let key = key.map_err(|error| format!("rquickjs export key failed: {error:?}"))?;
                let value: Value = namespace
                    .get(key.as_str())
                    .map_err(|error| format!("rquickjs export `{key}` failed: {error:?}"))?;
                out.insert(key.clone(), self.export_json(&ctx, value));
            }
            Ok::<_, String>(serde_json::Value::Object(out))
        })
    }

    fn export_json<'js>(&self, ctx: &rquickjs::Ctx<'js>, value: Value<'js>) -> serde_json::Value {
        if value.is_function() {
            if let Ok(function) = Function::from_value(value.clone()) {
                let id = self.next_handle.fetch_add(1, Ordering::Relaxed) + 1;
                self.handles
                    .borrow_mut()
                    .insert(id, Persistent::save(ctx, function));
                return serde_json::json!({ "handle": id });
            }
        }
        serde_json::json!({ "value": self.json_value(&value, 0) })
    }

    fn json_value(&self, value: &Value<'_>, depth: usize) -> serde_json::Value {
        if depth > 64 {
            return serde_json::Value::Null;
        }
        if value.is_null() || value.is_undefined() {
            return serde_json::Value::Null;
        }
        if let Some(value) = value.as_bool() {
            return serde_json::Value::Bool(value);
        }
        if let Some(value) = value.as_number() {
            return serde_json::Number::from_f64(value)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null);
        }
        if value.is_string() {
            return value
                .clone()
                .into_string()
                .and_then(|value| value.to_string().ok())
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null);
        }
        if value.is_array() {
            if let Some(array) = value.clone().into_array() {
                let mut out = Vec::new();
                for index in 0..array.len() {
                    if let Ok(item) = array.get::<Value>(index) {
                        out.push(self.json_value(&item, depth + 1));
                    }
                }
                return serde_json::Value::Array(out);
            }
        }
        if value.is_object() {
            if let Some(object) = value.as_object() {
                let mut out = serde_json::Map::new();
                for key in object.keys::<String>().flatten() {
                    if let Ok(item) = object.get::<_, Value>(key.as_str()) {
                        out.insert(key, self.json_value(&item, depth + 1));
                    }
                }
                return serde_json::Value::Object(out);
            }
        }
        serde_json::Value::Null
    }

    fn js_value<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        value: &JsValue,
    ) -> rquickjs::Result<Value<'js>> {
        match value {
            JsValue::Undefined => Ok(Value::new_undefined(ctx.clone())),
            JsValue::Null => Ok(Value::new_null(ctx.clone())),
            JsValue::Bool(value) => (*value).into_js(ctx),
            JsValue::Number(value) => (*value).into_js(ctx),
            JsValue::String(value) => value.clone().into_js(ctx),
            JsValue::Array(items) => {
                let array = Array::new(ctx.clone())?;
                for (index, item) in items.iter().enumerate() {
                    array.set(index, self.js_value(ctx, item)?)?;
                }
                Ok(array.into_value())
            }
            JsValue::Object(map) => {
                let object = Object::new(ctx.clone())?;
                for (key, item) in map {
                    object.set(key, self.js_value(ctx, item)?)?;
                }
                Ok(object.into_value())
            }
            JsValue::Bytes(bytes) => {
                let array = Array::new(ctx.clone())?;
                for (index, byte) in bytes.iter().enumerate() {
                    array.set(index, *byte)?;
                }
                Ok(array.into_value())
            }
            JsValue::Handle(_) => Err(rquickjs::Error::Unknown),
        }
    }

    fn call(
        &mut self,
        handle: u64,
        args: &[JsValue],
        control: ExecutionControl,
    ) -> std::result::Result<serde_json::Value, String> {
        self.set_control(control.clone());
        self.check_control()?;
        self.context.with(|ctx| {
            let persistent = self.handles.borrow().get(&handle).cloned().ok_or_else(|| {
                format!("rquickjs function handle {handle} is retired or unknown")
            })?;
            let function = persistent
                .restore(&ctx)
                .map_err(|error| format!("rquickjs handle restore failed: {error:?}"))?;
            let mut call_args = Args::new(ctx.clone(), args.len());
            for arg in args {
                let value = self
                    .js_value(&ctx, arg)
                    .map_err(|error| format!("rquickjs argument conversion failed: {error:?}"))?;
                call_args
                    .push_arg(value)
                    .map_err(|error| format!("rquickjs argument passing failed: {error:?}"))?;
            }
            let result: Value = function.call_arg(call_args).map_err(|error| {
                format!(
                    "rquickjs call failed: {}",
                    explain_error(&ctx, error, &control)
                )
            })?;
            let result = settle(ctx.clone(), result).map_err(|error| {
                format!(
                    "rquickjs promise failed: {}",
                    explain_error(&ctx, error, &control)
                )
            })?;
            self.check_control()?;
            Ok(self.json_value(&result, 0))
        })
    }
}

fn settle<'js>(ctx: rquickjs::Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Value<'js>> {
    if value.is_promise() {
        value.into_promise().unwrap().finish::<Value>()
    } else {
        while ctx.execute_pending_job() {}
        Ok(value)
    }
}

fn worker_loop(rx: mpsc::Receiver<Request>, shutdown: Arc<AtomicBool>) {
    let mut engine: Option<Engine> = None;
    let next_handle = Arc::new(AtomicU64::new(0));
    for request in rx {
        let Request { job, control } = request;
        if job.cancelled() || control.interrupted() || shutdown.load(Ordering::Relaxed) {
            continue;
        }
        match job {
            Job::EvalGraph { graph, reply } => {
                let result =
                    Engine::new(control.clone(), next_handle.clone()).and_then(|mut next| {
                        let result = next.eval_graph(&graph, control.clone());
                        if result.is_ok() {
                            engine = Some(next);
                        } else {
                            engine = None;
                        }
                        result
                    });
                let _ = reply.send(result);
            }
            Job::InvokeGraph {
                graph,
                export,
                args,
                reply,
            } => {
                let result =
                    Engine::new(control.clone(), next_handle.clone()).and_then(|mut engine| {
                        let namespace = engine.eval_graph(&graph, control.clone())?;
                        let handle = namespace
                            .get(&export)
                            .and_then(|value| value.get("handle"))
                            .and_then(serde_json::Value::as_u64)
                            .ok_or_else(|| {
                                format!("entry `{}` has no function export `{export}`", graph.entry)
                            })?;
                        engine.call(handle, &args, control.clone())
                    });
                let _ = reply.send(result);
            }
            Job::Call {
                handle,
                args,
                reply,
            } => {
                let result = engine
                    .as_mut()
                    .ok_or_else(|| "rquickjs runtime state is unavailable".to_string())
                    .and_then(|engine| engine.call(handle, &args, control.clone()));
                let _ = reply.send(result);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompiledModule, JsRuntime};

    fn graph(code: &str) -> CompiledModuleGraph {
        CompiledModuleGraph {
            entry: "/entry.js".into(),
            modules: vec![
                CompiledModule {
                    id: "/entry.js".into(),
                    code: code.into(),
                    url: None,
                },
                CompiledModule {
                    id: "/dep.js".into(),
                    code: "export const value = 41;".into(),
                    url: None,
                },
            ],
        }
    }

    #[tokio::test]
    async fn quickjs_evaluates_graph_and_calls_export() {
        let runtime = QuickJsRuntime::with_defaults();
        let graph = graph(
            "import { value } from './dep.js'; export const answer = value + 1; export function render(name) { return 'hello ' + name; }",
        );
        let env = RuntimeEnvironment {
            ssr: true,
            request_id: None,
        };
        let namespace = runtime
            .evaluate_module_graph(graph.clone(), env.clone())
            .await
            .unwrap();
        assert!(matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 42.0));
        let result = runtime
            .call(
                namespace.get_function("render").unwrap(),
                vec![JsValue::String("ferrite".into())],
            )
            .await
            .unwrap();
        assert!(matches!(result, JsValue::String(value) if value == "hello ferrite"));
        let invoked = runtime
            .invoke_module_graph(graph, "render", vec![JsValue::String("atomic".into())], env)
            .await
            .unwrap();
        assert!(matches!(invoked, JsValue::String(value) if value == "hello atomic"));
    }

    #[tokio::test]
    async fn quickjs_settles_promises_and_supports_many_args() {
        let runtime = QuickJsRuntime::with_defaults();
        let graph = graph(
            "export async function sum(a,b,c,d,e,f) { await Promise.resolve(); return [a,b,c,d,e,f].reduce((x,y)=>x+y,0); }",
        );
        let result = runtime
            .invoke_module_graph(
                graph,
                "sum",
                (1..=6).map(|value| JsValue::Number(value as f64)).collect(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(result, JsValue::Number(value) if value == 21.0));
    }

    #[tokio::test]
    async fn quickjs_retires_handles_across_replacement_graphs() {
        let runtime = QuickJsRuntime::with_defaults();
        let env = RuntimeEnvironment {
            ssr: true,
            request_id: None,
        };
        let first = runtime
            .evaluate_module_graph(
                graph("export function render() { return 'first'; }"),
                env.clone(),
            )
            .await
            .unwrap();
        let old = first.get_function("render").unwrap().clone();
        runtime
            .evaluate_module_graph(graph("export function render() { return 'second'; }"), env)
            .await
            .unwrap();
        assert!(runtime.call(&old, vec![]).await.is_err());
    }

    #[tokio::test]
    async fn quickjs_timeout_interrupts_guest_loop() {
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            request_timeout_ms: 50,
        });
        let error = runtime
            .invoke_module_graph(
                graph("export function render() { while (true) {} }"),
                "render",
                vec![],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("deadline") || error.to_string().contains("interrupted"),
            "{error}"
        );
    }
}
