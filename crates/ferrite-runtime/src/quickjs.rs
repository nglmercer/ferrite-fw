//! QuickJS embedded backend implemented with `rquickjs`.
//!
//! This is the replacement execution engine for pure JavaScript SSR and
//! embedded compiler workloads. Native `.node` addons are intentionally not
//! supported by this backend.

use crate::{CompiledModuleGraph, JsHandle, JsRuntime, JsStreamResponse, JsValue, ModuleNamespace, RuntimeEnvironment};
use ferrite_core::{FerriteError, Result};
use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::{Array, Context, Function, Module, Object, Persistent, Runtime, Value};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

type Reply<T> = tokio::sync::oneshot::Sender<std::result::Result<T, String>>;

enum Job {
    EvalGraph { graph: CompiledModuleGraph, fresh: bool, reply: Reply<serde_json::Value> },
    InvokeGraph { graph: CompiledModuleGraph, export: String, args: Vec<JsValue>, fresh: bool, reply: Reply<serde_json::Value> },
    Call { handle: u64, args: Vec<JsValue>, reply: Reply<serde_json::Value> },
}

impl Job {
    fn cancelled(&self) -> bool {
        match self {
            Self::EvalGraph { reply, .. } | Self::InvokeGraph { reply, .. } | Self::Call { reply, .. } => reply.is_closed(),
        }
    }
}

struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); }
}

/// Options retained for API compatibility during the engine migration.
#[derive(Debug, Clone, Default)]
pub struct QuickJsOptions {
    /// End-to-end request timeout in milliseconds; zero means no timeout.
    pub request_timeout_ms: u64,
}

/// QuickJS [`JsRuntime`] backend.
#[derive(Clone)]
pub struct QuickJsRuntime {
    jobs: Arc<Mutex<Option<mpsc::SyncSender<Job>>>>,
    cancel: Arc<AtomicBool>,
    timeout_ms: u64,
}

impl QuickJsRuntime {
    #[must_use]
    pub fn new(options: QuickJsOptions) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Job>(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        thread::Builder::new()
            .name("ferrite-rquickjs".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || worker_loop(rx, worker_cancel))
            .expect("spawn rquickjs worker");
        Self { jobs: Arc::new(Mutex::new(Some(tx))), cancel, timeout_ms: options.request_timeout_ms }
    }

    #[must_use]
    pub fn with_defaults() -> Self { Self::new(QuickJsOptions::default()) }

    pub fn shutdown(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).take();
    }

    fn send(&self, job: Job) -> Result<Cancel> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(FerriteError::Runtime("rquickjs worker shut down".into()));
        }
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            .ok_or_else(|| FerriteError::Runtime("rquickjs worker stopped".into()))?
            .try_send(job)
            .map_err(|e| FerriteError::Runtime(format!("rquickjs worker queue unavailable: {e}")))?;
        Ok(Cancel(Arc::new(AtomicBool::new(false))))
    }

    async fn receive<T>(&self, rx: tokio::sync::oneshot::Receiver<std::result::Result<T, String>>) -> Result<T> {
        rx.await.map_err(|_| FerriteError::Runtime("rquickjs worker stopped".into()))?
            .map_err(FerriteError::Runtime)
    }
}

fn encode_args(args: &[JsValue]) -> Result<Vec<serde_json::Value>> {
    args.iter().map(crate::js_value_to_json).collect()
}

#[async_trait::async_trait]
impl JsRuntime for QuickJsRuntime {
    fn name(&self) -> &'static str { "rquickjs" }

    async fn evaluate_module(&self, module: crate::CompiledModule, env: RuntimeEnvironment) -> Result<ModuleNamespace> {
        self.evaluate_module_graph(CompiledModuleGraph { entry: module.id.clone(), modules: vec![module] }, env).await
    }

    async fn evaluate_module_graph(&self, graph: CompiledModuleGraph, env: RuntimeEnvironment) -> Result<ModuleNamespace> {
        graph.validate()?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::EvalGraph { graph, fresh: env.ssr, reply: tx })?;
        let json = self.receive(rx).await?;
        namespace_from_json(&json)
    }

    async fn invoke_module_graph(&self, graph: CompiledModuleGraph, export: &str, args: Vec<JsValue>, env: RuntimeEnvironment) -> Result<JsValue> {
        graph.validate()?;
        let args = encode_args(&args)?.iter().map(|v| JsValue::from(v.clone())).collect();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::InvokeGraph { graph, export: export.into(), args, fresh: env.ssr, reply: tx })?;
        Ok(JsValue::from(self.receive::<serde_json::Value>(rx).await?))
    }

    async fn invoke_module_graph_stream(&self, graph: CompiledModuleGraph, _export: &str, _args: Vec<JsValue>, _env: RuntimeEnvironment) -> Result<JsStreamResponse> {
        graph.validate()?;
        Err(FerriteError::Runtime("rquickjs streaming invocation is not implemented yet; use the non-streaming SSR path explicitly".into()))
    }

    async fn call(&self, handle: &JsHandle, args: Vec<JsValue>) -> Result<JsValue> {
        let args = encode_args(&args)?.iter().map(|v| JsValue::from(v.clone())).collect();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::Call { handle: handle.id, args, reply: tx })?;
        Ok(JsValue::from(self.receive::<serde_json::Value>(rx).await?))
    }
}

fn namespace_from_json(json: &serde_json::Value) -> Result<ModuleNamespace> {
    let mut namespace = ModuleNamespace::default();
    let object = json.as_object().ok_or_else(|| FerriteError::Runtime("rquickjs malformed export record".into()))?;
    for (name, entry) in object {
        let value = match entry.get("handle").and_then(serde_json::Value::as_u64) {
            Some(id) => JsValue::Handle(JsHandle { id, label: Some(format!("function {name}")) }),
            None => entry.get("value").cloned().map(JsValue::from).unwrap_or(JsValue::Undefined),
        };
        namespace.exports.insert(name.clone(), value);
    }
    Ok(namespace)
}

struct GraphResolver;
impl Resolver for GraphResolver {
    fn resolve<'js>(&mut self, _ctx: &rquickjs::Ctx<'js>, base: &str, name: &str, _attributes: Option<ImportAttributes<'js>>) -> rquickjs::Result<String> {
        if !name.starts_with('.') { return Ok(name.into()); }
        let mut parts: Vec<&str> = base.rsplit_once('/').map(|x| x.0).unwrap_or("").split('/').filter(|x| !x.is_empty()).collect();
        for part in name.split('/') {
            match part { "" | "." => {}, ".." => { parts.pop(); }, x => parts.push(x) }
        }
        Ok(format!("/{}", parts.join("/")))
    }
}

struct GraphLoader(Arc<Mutex<HashMap<String, String>>>);
impl Loader for GraphLoader {
    fn load<'js>(&mut self, ctx: &rquickjs::Ctx<'js>, name: &str, _attributes: Option<ImportAttributes<'js>>) -> rquickjs::Result<Module<'js, rquickjs::module::Declared>> {
        let code = self.0.lock().unwrap().get(name).cloned()
            .ok_or_else(|| rquickjs::Error::new_loading_message(name, "module missing from compiled graph"))?;
        Module::declare(ctx.clone(), name, code)
    }
}

struct Engine {
    runtime: Runtime,
    context: Context,
    handles: RefCell<HashMap<u64, Persistent<rquickjs::Function<'static>>>>,
    next_handle: Cell<u64>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.handles.borrow_mut().clear();
    }
}

impl Engine {
    fn new(timeout_ms: u64, cancel: Arc<AtomicBool>) -> std::result::Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| format!("rquickjs runtime init: {e:?}"))?;
        runtime.set_max_stack_size(4 * 1024 * 1024);
        if timeout_ms != 0 {
            let started = Instant::now();
            let timeout = Duration::from_millis(timeout_ms);
            runtime.set_interrupt_handler(Some(Box::new(move || cancel.load(Ordering::Relaxed) || started.elapsed() >= timeout)));
        }
        let context = Context::full(&runtime).map_err(|e| format!("rquickjs context init: {e:?}"))?;
        context.with(|ctx| {
            ctx.eval::<(), _>(r#"if (typeof console === 'undefined') globalThis.console = {log(){},info(){},warn(){},error(){},debug(){},trace(){}};"#)
        }).map_err(|e| format!("rquickjs host init: {e:?}"))?;
        Ok(Self { runtime, context, handles: RefCell::new(HashMap::new()), next_handle: Cell::new(1) })
    }

    fn eval_graph(&mut self, graph: &CompiledModuleGraph, timeout_ms: u64, cancel: Arc<AtomicBool>) -> std::result::Result<serde_json::Value, String> {
        *self = Self::new(timeout_ms, cancel)?;
        let modules: HashMap<_, _> = graph.modules.iter().map(|m| (m.id.clone(), m.code.clone())).collect();
        let shared = Arc::new(Mutex::new(modules));
        self.runtime.set_loader(GraphResolver, GraphLoader(shared));
        let entry = graph.modules.iter().find(|m| m.id == graph.entry).ok_or_else(|| format!("entry {} missing", graph.entry))?;
        self.context.with(|ctx| {
            let module = Module::declare(ctx.clone(), entry.id.clone(), entry.code.clone())
                .and_then(|module| module.eval())
                .map_err(|e| format!("rquickjs graph entry `{}` evaluation failed: {e:?}", graph.entry))?;
            module.1.finish::<()>().map_err(|e| format!("rquickjs graph entry `{}` promise failed: {e:?}", graph.entry))?;
            let namespace = module.0.namespace().map_err(|e| format!("rquickjs namespace failed: {e:?}"))?;
            let mut out = serde_json::Map::new();
            for key in namespace.keys::<String>() {
                let key = key.map_err(|e| format!("rquickjs export key failed: {e:?}"))?;
                let value: Value = namespace.get(key.as_str()).map_err(|e| format!("rquickjs export `{key}` failed: {e:?}"))?;
                out.insert(key.clone(), self.export_json(&ctx, value));
            }
            Ok::<_, String>(serde_json::Value::Object(out))
        })
    }

    fn export_json<'js>(&self, ctx: &rquickjs::Ctx<'js>, value: Value<'js>) -> serde_json::Value {
        if value.is_function() {
            if let Ok(function) = Function::from_value(value.clone()) {
                let id = self.next_handle.get();
                self.next_handle.set(id + 1);
                self.handles.borrow_mut().insert(id, Persistent::save(ctx, function));
                return serde_json::json!({"handle": id});
            }
        }
        serde_json::json!({"value": self.json_value(&value)})
    }

    fn json_value(&self, value: &Value<'_>) -> serde_json::Value {
        if value.is_null() || value.is_undefined() { return serde_json::Value::Null; }
        if let Some(v) = value.as_bool() { return serde_json::Value::Bool(v); }
        if let Some(v) = value.as_number() { return serde_json::Number::from_f64(v).map(serde_json::Value::Number).unwrap_or(serde_json::Value::Null); }
        if value.is_string() { return value.clone().into_string().and_then(|s| s.to_string().ok()).map(serde_json::Value::String).unwrap_or(serde_json::Value::Null); }
        if value.is_array() {
            if let Some(array) = value.clone().into_array() {
                let mut out = Vec::new();
                for i in 0..array.len() {
                    if let Ok(v) = array.get::<Value>(i) { out.push(self.json_value(&v)); }
                }
                return serde_json::Value::Array(out);
            }
        }
        if value.is_object() {
            if let Some(object) = value.as_object() {
                let mut out = serde_json::Map::new();
                for key in object.keys::<String>().flatten() {
                    if let Ok(v) = object.get::<_, Value>(key.as_str()) { out.insert(key, self.json_value(&v)); }
                }
                return serde_json::Value::Object(out);
            }
        }
        serde_json::Value::Null
    }

    fn js_value<'js>(&self, ctx: &rquickjs::Ctx<'js>, value: &JsValue) -> rquickjs::Result<Value<'js>> {
        match value {
            JsValue::Undefined => Ok(Value::new_undefined(ctx.clone())),
            JsValue::Null => Ok(Value::new_null(ctx.clone())),
            JsValue::Bool(v) => (*v).into_js(ctx),
            JsValue::Number(v) => (*v).into_js(ctx),
            JsValue::String(v) => v.clone().into_js(ctx),
            JsValue::Array(items) => {
                let array = Array::new(ctx.clone())?;
                for (i, item) in items.iter().enumerate() { array.set(i, self.js_value(ctx, item)?)?; }
                Ok(array.into_value())
            }
            JsValue::Object(map) => {
                let object = Object::new(ctx.clone())?;
                for (key, item) in map { object.set(key, self.js_value(ctx, item)?)?; }
                Ok(object.into_value())
            }
            JsValue::Bytes(bytes) => {
                let array = Array::new(ctx.clone())?;
                for (i, byte) in bytes.iter().enumerate() { array.set(i, *byte)?; }
                Ok(array.into_value())
            }
            JsValue::Handle(_) => Err(rquickjs::Error::Unknown),
        }
    }

    fn call(&mut self, handle: u64, args: &[JsValue]) -> std::result::Result<serde_json::Value, String> {
        self.context.with(|ctx| {
            let persistent = self.handles.borrow().get(&handle).cloned().ok_or_else(|| format!("rquickjs function handle {handle} is retired or unknown"))?;
            let function = persistent.restore(&ctx).map_err(|e| format!("rquickjs handle restore failed: {e:?}"))?;
            let mut converted = Vec::new();
            for arg in args { converted.push(self.js_value(&ctx, arg).map_err(|e| format!("rquickjs argument conversion failed: {e:?}"))?); }
            let result = call_with_args(&function, converted).map_err(|e| format!("rquickjs call failed: {e:?}"))?;
            let result = settle(ctx.clone(), result).map_err(|e| format!("rquickjs promise failed: {e:?}"))?;
            Ok(self.json_value(&result))
        })
    }
}

fn call_with_args<'js>(function: &Function<'js>, args: Vec<Value<'js>>) -> rquickjs::Result<Value<'js>> {
    match args.len() {
        0 => function.call(()),
        1 => function.call((args[0].clone(),)),
        2 => function.call((args[0].clone(), args[1].clone())),
        3 => function.call((args[0].clone(), args[1].clone(), args[2].clone())),
        4 => function.call((args[0].clone(), args[1].clone(), args[2].clone(), args[3].clone())),
        _ => Err(rquickjs::Error::Unknown),
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

fn worker_loop(rx: mpsc::Receiver<Job>, shutdown: Arc<AtomicBool>) {
    let mut engine: Option<Engine> = None;
    for job in rx {
        if job.cancelled() || shutdown.load(Ordering::Relaxed) { continue; }
        match job {
            Job::EvalGraph { graph, fresh: _, reply } => {
                let timeout = 15000;
                let next = Engine::new(timeout, shutdown.clone()).map_err(|e| e);
                let result = next.and_then(|mut eng| {
                    let out = eng.eval_graph(&graph, timeout, shutdown.clone());
                    engine = Some(eng);
                    out
                });
                let _ = reply.send(result);
            }
            Job::InvokeGraph { graph, export, args, fresh: _, reply } => {
                let timeout = 15000;
                let result = Engine::new(timeout, shutdown.clone()).and_then(|mut eng| {
                    let ns = eng.eval_graph(&graph, timeout, shutdown.clone())?;
                    let handle = ns.get(&export).and_then(|v| v.get("handle")).and_then(serde_json::Value::as_u64)
                        .ok_or_else(|| format!("entry `{}` has no function export `{export}`", graph.entry))?;
                    eng.call(handle, &args)
                });
                let _ = reply.send(result);
            }
            Job::Call { handle, args, reply } => {
                let result = engine.as_mut().ok_or_else(|| "rquickjs runtime state is unavailable".to_string())
                    .and_then(|eng| eng.call(handle, &args));
                let _ = reply.send(result);
            }
        }
    }
}

use rquickjs::IntoJs;


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompiledModule, JsRuntime};

    #[tokio::test]
    async fn quickjs_evaluates_graph_and_calls_export() {
        let runtime = QuickJsRuntime::with_defaults();
        let graph = CompiledModuleGraph {
            entry: "/entry.js".into(),
            modules: vec![
                CompiledModule { id: "/entry.js".into(), code: "import { value } from './dep.js'; export const answer = value + 1; export function render(name) { return 'hello ' + name; }".into(), url: None },
                CompiledModule { id: "/dep.js".into(), code: "export const value = 41;".into(), url: None },
            ],
        };
        let env = RuntimeEnvironment { ssr: true, request_id: None };
        let namespace = runtime.evaluate_module_graph(graph.clone(), env.clone()).await.unwrap();
        assert!(matches!(namespace.get("answer"), Some(JsValue::Number(v)) if *v == 42.0));
        let result = runtime.call(namespace.get_function("render").unwrap(), vec![JsValue::String("ferrite".into())]).await.unwrap();
        assert!(matches!(result, JsValue::String(v) if v == "hello ferrite"));
        let invoked = runtime.invoke_module_graph(graph, "render", vec![JsValue::String("atomic".into())], env).await.unwrap();
        assert!(matches!(invoked, JsValue::String(v) if v == "hello atomic"));
    }
}
