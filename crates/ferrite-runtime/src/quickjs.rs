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
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::OwnedSemaphorePermit;

type Reply<T> = tokio::sync::oneshot::Sender<std::result::Result<T, String>>;

#[derive(Clone)]
struct ExecutionControl {
    cancelled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

impl ExecutionControl {
    fn new(timeout_ms: u64, shutdown: Arc<AtomicBool>) -> Self {
        Self::with_deadline(
            (timeout_ms != 0).then(|| Instant::now() + Duration::from_millis(timeout_ms)),
            shutdown,
        )
    }

    fn with_deadline(deadline: Option<Instant>, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            shutdown,
            deadline,
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
    Reset,
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
        interleave_timers: bool,
        reply: Reply<serde_json::Value>,
    },
}

impl Job {
    fn cancelled(&self) -> bool {
        match self {
            Self::Reset => false,
            Self::EvalGraph { reply, .. }
            | Self::InvokeGraph { reply, .. }
            | Self::Call { reply, .. } => reply.is_closed(),
        }
    }
}

fn js_value_bytes(value: &JsValue) -> usize {
    match value {
        JsValue::Undefined | JsValue::Null => 4,
        JsValue::Bool(_) => 1,
        JsValue::Number(_) => 8,
        JsValue::String(value) => value.len(),
        JsValue::Bytes(bytes) => bytes.len(),
        JsValue::Array(items) => items.iter().map(js_value_bytes).sum::<usize>() + 2,
        JsValue::Object(map) => {
            map.iter()
                .map(|(key, value)| key.len() + js_value_bytes(value) + 2)
                .sum::<usize>()
                + 2
        }
        JsValue::Handle(_) => 8,
    }
}

impl Job {
    fn payload_bytes(&self) -> usize {
        fn graph_bytes(graph: &CompiledModuleGraph) -> usize {
            graph.entry.len()
                + graph
                    .modules
                    .iter()
                    .map(|module| {
                        module.id.len()
                            + module.code.len()
                            + module.url.as_deref().unwrap_or("").len()
                    })
                    .sum::<usize>()
        }
        match self {
            Self::Reset => 0,
            Self::EvalGraph { graph, .. } => graph_bytes(graph),
            Self::InvokeGraph {
                graph,
                export,
                args,
                ..
            } => graph_bytes(graph) + export.len() + args.iter().map(js_value_bytes).sum::<usize>(),
            Self::Call { args, .. } => args.iter().map(js_value_bytes).sum::<usize>(),
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
#[derive(Debug, Clone)]
pub struct QuickJsOptions {
    /// End-to-end request timeout in milliseconds; zero means no timeout.
    pub request_timeout_ms: u64,
    /// Maximum queued requests.
    pub queue_capacity: usize,
    /// Maximum serialized request payload bytes; zero uses 8 MiB.
    pub max_request_bytes: usize,
}

impl Default for QuickJsOptions {
    fn default() -> Self {
        Self {
            request_timeout_ms: 5000,
            queue_capacity: 64,
            max_request_bytes: 8 * 1024 * 1024,
        }
    }
}

/// QuickJS [`JsRuntime`] backend.
#[derive(Clone)]
pub struct QuickJsRuntime {
    jobs: Arc<Mutex<Option<mpsc::SyncSender<Request>>>>,
    shutdown: Arc<AtomicBool>,
    timeout_ms: u64,
    max_request_bytes: usize,
    stream_workers: Arc<Mutex<Vec<Arc<StreamWorker>>>>,
    stream_worker_limit: usize,
}

struct StreamWorker {
    runtime: QuickJsRuntime,
    idle: Arc<tokio::sync::Semaphore>,
}

struct StreamWorkerLease {
    worker: Arc<StreamWorker>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for StreamWorkerLease {
    fn drop(&mut self) {
        self.worker.runtime.reset_worker();
    }
}

impl QuickJsRuntime {
    #[must_use]
    pub fn new(options: QuickJsOptions) -> Self {
        let capacity = if options.queue_capacity == 0 {
            64
        } else {
            options.queue_capacity
        };
        Self::new_inner(options, capacity)
    }

    fn new_inner(options: QuickJsOptions, stream_worker_limit: usize) -> Self {
        let capacity = if options.queue_capacity == 0 {
            64
        } else {
            options.queue_capacity
        };
        let (tx, rx) = mpsc::sync_channel::<Request>(capacity);
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
            max_request_bytes: if options.max_request_bytes == 0 {
                8 * 1024 * 1024
            } else {
                options.max_request_bytes
            },
            stream_workers: Arc::new(Mutex::new(Vec::new())),
            stream_worker_limit,
        }
    }

    fn reset_worker(&self) {
        if self.shutdown.load(Ordering::Relaxed) {
            return;
        }
        let control = ExecutionControl::new(self.timeout_ms, self.shutdown.clone());
        if let Some(sender) = self
            .jobs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
        {
            let _ = sender.try_send(Request {
                job: Job::Reset,
                control,
            });
        }
    }

    fn lease_stream_worker(&self) -> Result<StreamWorkerLease> {
        let mut workers = self
            .stream_workers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for worker in workers.iter() {
            if let Ok(permit) = worker.idle.clone().try_acquire_owned() {
                return Ok(StreamWorkerLease {
                    worker: worker.clone(),
                    _permit: permit,
                });
            }
        }
        if workers.len() >= self.stream_worker_limit {
            return Err(FerriteError::Runtime(format!(
                "rquickjs streaming capacity exhausted ({} concurrent streams); finish open responses or increase queue_capacity",
                self.stream_worker_limit
            )));
        }
        let worker = Arc::new(StreamWorker {
            runtime: QuickJsRuntime::new_inner(
                QuickJsOptions {
                    request_timeout_ms: self.timeout_ms,
                    queue_capacity: 2,
                    max_request_bytes: self.max_request_bytes,
                },
                0,
            ),
            idle: Arc::new(tokio::sync::Semaphore::new(1)),
        });
        let permit = worker
            .idle
            .clone()
            .try_acquire_owned()
            .expect("new stream worker permit");
        workers.push(worker.clone());
        Ok(StreamWorkerLease {
            worker,
            _permit: permit,
        })
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
        self.send_with_control(job, control)
    }

    fn send_with_control(&self, job: Job, control: ExecutionControl) -> Result<CancelGuard> {
        if job.payload_bytes() > self.max_request_bytes {
            return Err(FerriteError::Runtime(format!(
                "rquickjs request payload exceeds {} bytes; reduce the graph/arguments or explicitly configure max_request_bytes",
                self.max_request_bytes
            )));
        }
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

    async fn evaluate_module_graph_with_deadline(
        &self,
        graph: CompiledModuleGraph,
        deadline: Option<Instant>,
    ) -> Result<ModuleNamespace> {
        graph.validate()?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let control = ExecutionControl::with_deadline(deadline, self.shutdown.clone());
        let _guard = self.send_with_control(Job::EvalGraph { graph, reply: tx }, control)?;
        let json = self.receive(rx).await?;
        namespace_from_json(&json)
    }

    async fn call_with_deadline(
        &self,
        handle: &JsHandle,
        args: Vec<JsValue>,
        deadline: Option<Instant>,
    ) -> Result<JsValue> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let control = ExecutionControl::with_deadline(deadline, self.shutdown.clone());
        let _guard = self.send_with_control(
            Job::Call {
                handle: handle.id,
                args,
                interleave_timers: true,
                reply: tx,
            },
            control,
        )?;
        Ok(JsValue::from(self.receive::<serde_json::Value>(rx).await?))
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
        export: &str,
        args: Vec<JsValue>,
        _env: RuntimeEnvironment,
    ) -> Result<JsStreamResponse> {
        graph.validate()?;
        const WRAPPER_ID: &str = "/@ferrite/stream-session";
        if graph.modules.iter().any(|module| module.id == WRAPPER_ID) {
            return Err(FerriteError::Runtime(
                "reserved stream wrapper module identity".into(),
            ));
        }
        let mut graph = graph;
        let wrapper = include_str!("stream_wrapper.js")
            .replace(
                "__ENTRY__",
                &serde_json::to_string(&graph.entry)
                    .map_err(|error| FerriteError::Runtime(error.to_string()))?,
            )
            .replace(
                "__EXPORT__",
                &serde_json::to_string(export)
                    .map_err(|error| FerriteError::Runtime(error.to_string()))?,
            );
        graph.modules.push(crate::CompiledModule {
            id: WRAPPER_ID.into(),
            code: wrapper,
            url: None,
        });
        graph.entry = WRAPPER_ID.into();

        let payload = graph.entry.len()
            + graph
                .modules
                .iter()
                .map(|module| {
                    module.id.len() + module.code.len() + module.url.as_deref().unwrap_or("").len()
                })
                .sum::<usize>()
            + export.len()
            + args.iter().map(js_value_bytes).sum::<usize>();
        if payload > self.max_request_bytes {
            return Err(FerriteError::Runtime(format!(
                "rquickjs request payload exceeds {} bytes; reduce the graph/arguments or explicitly configure max_request_bytes",
                self.max_request_bytes
            )));
        }

        let request_timeout_ms = self.timeout_ms;
        let deadline = (request_timeout_ms != 0)
            .then(|| Instant::now() + Duration::from_millis(request_timeout_ms));
        // A stream leases one bounded pool worker so a later graph cannot
        // retire the context that its `next` handle still references. Workers
        // are reused after the response instead of creating one thread per stream.
        let lease = self.lease_stream_worker()?;
        let runtime = lease.worker.runtime.clone();
        let namespace = runtime
            .evaluate_module_graph_with_deadline(graph, deadline)
            .await?;
        let start = namespace.get_function("start")?.clone();
        let next = namespace.get_function("next")?.clone();
        let head = runtime.call_with_deadline(&start, args, deadline).await?;
        crate::js_value_to_json(&head)?;

        let body = futures::stream::try_unfold(
            (runtime, next, deadline, lease),
            move |(runtime, next, deadline, lease)| async move {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    return Err(FerriteError::Runtime(
                        "rquickjs stream deadline exceeded".into(),
                    ));
                }
                let value = runtime.call_with_deadline(&next, vec![], deadline).await?;
                let chunk = decode_stream_chunk(&value)?;
                if chunk.done {
                    Ok(None)
                } else {
                    Ok(Some((chunk.bytes, (runtime, next, deadline, lease))))
                }
            },
        );
        Ok(JsStreamResponse {
            head,
            body: Box::pin(body),
        })
    }

    async fn call(&self, handle: &JsHandle, args: Vec<JsValue>) -> Result<JsValue> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _guard = self.send(Job::Call {
            handle: handle.id,
            args,
            interleave_timers: false,
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

struct StreamChunk {
    done: bool,
    bytes: Vec<u8>,
}

fn decode_stream_chunk(value: &JsValue) -> Result<StreamChunk> {
    let json = crate::js_value_to_json(value)?;
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct EncodedChunk {
        done: bool,
        hex: String,
        error: Option<String>,
    }
    let chunk: EncodedChunk = serde_json::from_value(json).map_err(|error| {
        FerriteError::Runtime(format!("rquickjs malformed stream chunk: {error}"))
    })?;
    if let Some(error) = chunk.error {
        return Err(FerriteError::Runtime(format!(
            "rquickjs stream failed: {error}"
        )));
    }
    const CHUNK_BYTES: usize = 64 * 1024;
    if chunk.hex.len() > CHUNK_BYTES * 2 || !chunk.hex.len().is_multiple_of(2) {
        return Err(FerriteError::Runtime(
            "rquickjs invalid encoded stream chunk length".into(),
        ));
    }
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(FerriteError::Runtime(
            "rquickjs invalid encoded stream byte".into(),
        )),
    };
    let bytes = chunk
        .hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok((digit(pair[0])? << 4) | digit(pair[1])?))
        .collect::<Result<Vec<_>>>()?;
    if chunk.done && !bytes.is_empty() {
        return Err(FerriteError::Runtime(
            "rquickjs invalid terminal stream chunk".into(),
        ));
    }
    Ok(StreamChunk {
        done: chunk.done,
        bytes,
    })
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

struct TimerState {
    next_id: u64,
    timers: std::collections::BTreeMap<u64, Timer>,
}

struct Timer {
    due: Instant,
    callback: Persistent<rquickjs::Function<'static>>,
}

struct Engine {
    runtime: Runtime,
    context: Context,
    timers: Rc<RefCell<TimerState>>,
    handles: RefCell<HashMap<u64, Persistent<rquickjs::Function<'static>>>>,
    next_handle: Arc<AtomicU64>,
    active_control: Arc<Mutex<ExecutionControl>>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.handles.borrow_mut().clear();
        self.timers.borrow_mut().timers.clear();
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
        let timers = Rc::new(RefCell::new(TimerState {
            next_id: 1,
            timers: std::collections::BTreeMap::new(),
        }));
        context
            .with(|ctx| {
                let set_timers = timers.clone();
                let set_timeout = Function::new(ctx.clone(), move |callback: Function, delay: Option<f64>| {
                    let mut timers = set_timers.borrow_mut();
                    let id = timers.next_id;
                    timers.next_id += 1;
                    let delay = delay.unwrap_or(0.0).clamp(0.0, 2_147_483_647.0);
                    let due = Instant::now() + Duration::from_millis(delay as u64);
                    let callback_ctx = callback.ctx().clone();
                    let callback = Persistent::save(&callback_ctx, callback);
                    timers.timers.insert(id, Timer { due, callback });
                    id
                })?;
                let clear_timers = timers.clone();
                let clear_timeout = Function::new(ctx.clone(), move |id: Option<f64>| {
                    if let Some(id) = id {
                        clear_timers.borrow_mut().timers.remove(&(id as u64));
                    }
                })?;
                ctx.globals().set("__ferriteSetTimeout", set_timeout)?;
                ctx.globals().set("__ferriteClearTimeout", clear_timeout)?;
                ctx.eval::<(), _>(
                    r#"
if (typeof console === 'undefined') globalThis.console = {log(){},info(){},warn(){},error(){},debug(){},trace(){}};
if (typeof setTimeout === 'undefined') {
  globalThis.setTimeout = function setTimeout(callback, delay, ...args) {
    if (typeof callback !== 'function') throw new TypeError('setTimeout callback must be a function');
    return globalThis.__ferriteSetTimeout(function ferriteTimer() { callback(...args); }, delay);
  };
  globalThis.clearTimeout = function clearTimeout(id) { globalThis.__ferriteClearTimeout(id); };
  globalThis.setInterval = function setInterval() { throw new TypeError('setInterval is unsupported in the SSR runtime'); };
  globalThis.clearInterval = function clearInterval() {};
}
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

if (typeof globalThis.__ferriteStreamBytes === 'undefined') {
  globalThis.__ferriteStreamBytes = function ferriteStreamBytes(value, offset) {
    let bytes;
    if (typeof value === 'string') bytes = new TextEncoder().encode(value);
    else if (ArrayBuffer.isView(value)) bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    else if (value instanceof ArrayBuffer) bytes = new Uint8Array(value);
    else if (Array.isArray(value)) bytes = Uint8Array.from(value);
    else throw new TypeError('stream chunks must be strings, ArrayBuffers, typed arrays, or byte arrays');
    offset = Number(offset);
    if (!Number.isInteger(offset) || offset < 0 || offset > bytes.length) throw new RangeError('invalid stream byte offset');
    const end = Math.min(bytes.length, offset + 65536);
    let hex = '';
    for (let index = offset; index < end; index++) hex += bytes[index].toString(16).padStart(2, '0');
    return { hex, length: bytes.length };
  };
}
"#,
                )
            })
            .map_err(|error| format!("rquickjs host init: {error:?}"))?;
        Ok(Self {
            runtime,
            context,
            timers,
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
            self.settle_value(ctx.clone(), promise.into_value(), &control, false)
                .map_err(|error| {
                    format!(
                        "rquickjs graph entry `{}` promise failed: {error}",
                        graph.entry
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
                        if !item.is_undefined() {
                            out.insert(key, self.json_value(&item, depth + 1));
                        }
                    }
                }
                return serde_json::Value::Object(out);
            }
        }
        serde_json::Value::Null
    }

    fn drain_ready_work<'js>(
        &self,
        ctx: &rquickjs::Ctx<'js>,
        control: &ExecutionControl,
        interleave_timers: bool,
    ) -> std::result::Result<(), String> {
        loop {
            self.check_control()?;
            while ctx.execute_pending_job() {
                self.check_control()?;
            }
            let next = self
                .timers
                .borrow()
                .timers
                .iter()
                .map(|(id, timer)| (*id, timer.due))
                .min_by_key(|(_, due)| *due);
            let Some((id, due)) = next else {
                return Ok(());
            };
            if due > Instant::now() {
                return Ok(());
            }
            let timer = self.timers.borrow_mut().timers.remove(&id).unwrap();
            let callback = timer
                .callback
                .restore(ctx)
                .map_err(|error| format!("rquickjs timer restore failed: {error:?}"))?;
            let result: std::result::Result<Value, _> = callback.call(());
            if let Err(error) = result {
                return Err(format!(
                    "rquickjs timer failed: {}",
                    explain_error(ctx, error, control)
                ));
            }
            while ctx.execute_pending_job() {
                self.check_control()?;
            }
            // Streaming reads interleave one timer turn with chunk delivery.
            // Non-streaming invocation drains the ready timer cascade, matching
            // the event-loop progress that would occur before a later render.
            if interleave_timers {
                return Ok(());
            }
        }
    }

    fn settle_value<'js>(
        &self,
        ctx: rquickjs::Ctx<'js>,
        value: Value<'js>,
        control: &ExecutionControl,
        interleave_timers: bool,
    ) -> std::result::Result<Value<'js>, String> {
        if !value.is_promise() {
            self.drain_ready_work(&ctx, control, interleave_timers)?;
            return Ok(value);
        }
        let promise = value.into_promise().unwrap();
        loop {
            self.check_control()?;
            self.drain_ready_work(&ctx, control, interleave_timers)?;
            if let Some(result) = promise.result::<Value>() {
                return result.map_err(|error| explain_error(&ctx, error, control));
            }
            let next = self
                .timers
                .borrow()
                .timers
                .iter()
                .map(|(_, timer)| timer.due)
                .min();
            let Some(due) = next else {
                return Err("rquickjs promise cannot settle: no pending jobs or timers".into());
            };
            let now = Instant::now();
            if due > now {
                let wait = (due - now).min(Duration::from_millis(2));
                thread::sleep(wait);
            }
        }
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
        interleave_timers: bool,
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
            let result = self
                .settle_value(ctx.clone(), result, &control, interleave_timers)
                .map_err(|error| format!("rquickjs promise failed: {error}"))?;
            self.check_control()?;
            Ok(self.json_value(&result, 0))
        })
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
            Job::Reset => {
                engine = None;
            }
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
                        engine.call(handle, &args, control.clone(), false)
                    });
                let _ = reply.send(result);
            }
            Job::Call {
                handle,
                args,
                interleave_timers,
                reply,
            } => {
                let result = engine
                    .as_mut()
                    .ok_or_else(|| "rquickjs runtime state is unavailable".to_string())
                    .and_then(|engine| {
                        engine.call(handle, &args, control.clone(), interleave_timers)
                    });
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
    async fn quickjs_streams_renderer_body_chunks() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::with_defaults();
        let graph = graph(
            r#"export async function render(url) {
              let index = 0;
              const chunks = ['<h1>', url, '</h1>'];
              return {
                status: 202,
                headers: [['x-stream', 'yes']],
                body: new ReadableStream({
                  pull(controller) {
                    if (index < chunks.length) controller.enqueue(chunks[index++]);
                    else controller.close();
                  }
                })
              };
            }"#,
        );
        let response = runtime
            .invoke_module_graph_stream(
                graph,
                "render",
                vec![JsValue::String("/stream".into())],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        let head = crate::js_value_to_json(&response.head).unwrap();
        assert_eq!(head["status"], 202.0);
        let chunks = response.body.collect::<Vec<_>>().await;
        let body = chunks
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .unwrap()
            .concat();
        assert_eq!(body, b"<h1>/stream</h1>");
    }

    #[tokio::test]
    async fn quickjs_stream_fast_producer_is_not_aborted_by_queue_bound() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::with_defaults();
        let response = runtime
            .invoke_module_graph_stream(
                graph(
                    r#"export function render() {
                  return new ReadableStream({
                    start(controller) {
                      for (let index = 0; index < 32; index++) controller.enqueue(new Uint8Array(65536));
                      controller.close();
                    }
                  });
                }"#,
                ),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await
            .unwrap();
        let chunks = response.body.collect::<Vec<_>>().await;
        let bytes = chunks
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .unwrap()
            .concat();
        assert_eq!(bytes.len(), 32 * 65536);
    }

    #[tokio::test]
    async fn quickjs_stream_timers_close_even_while_chunks_are_ready() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            request_timeout_ms: 500,
            ..Default::default()
        });
        let response = runtime
            .invoke_module_graph_stream(
                graph(
                    r#"export function render() {
                  return new ReadableStream({
                    start(controller) { setTimeout(() => controller.close(), 10); },
                    pull(controller) { for (let index = 0; index < 10; index++) controller.enqueue('x'); }
                  });
                }"#,
                ),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await
            .unwrap();
        let chunks = response.body.collect::<Vec<_>>().await;
        let bytes = chunks
            .into_iter()
            .collect::<Result<Vec<_>>>()
            .unwrap()
            .concat();
        assert!(!bytes.is_empty());
    }

    #[tokio::test]
    async fn quickjs_stream_zero_timer_chain_does_not_block_ready_chunks() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            request_timeout_ms: 200,
            ..Default::default()
        });
        let response = runtime
            .invoke_module_graph_stream(
                graph(
                    r#"export function render() {
                  return new ReadableStream({
                    start(controller) {
                      controller.enqueue('ready');
                      function chain() { setTimeout(chain, 0); }
                      chain();
                    }
                  });
                }"#,
                ),
                "render",
                vec![],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        let mut body = response.body;
        assert_eq!(body.next().await.unwrap().unwrap(), b"ready");
    }

    #[tokio::test]
    async fn quickjs_nonstream_waits_for_chained_initialization_timers() {
        let runtime = QuickJsRuntime::with_defaults();
        let result = runtime
            .invoke_module_graph(
                graph(
                    r#"let initialized = false;
                  setTimeout(() => setTimeout(() => { initialized = true; }, 0), 0);
                  export function render() { return initialized ? 'ready' : 'uninitialized'; }"#,
                ),
                "render",
                vec![],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        assert!(matches!(result, JsValue::String(value) if value == "ready"));
    }

    #[tokio::test]
    async fn quickjs_timeout_interrupts_guest_loop() {
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            request_timeout_ms: 50,
            ..Default::default()
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

    #[tokio::test]
    async fn quickjs_timeout_zero_timers_settle_renderer_promises() {
        let runtime = QuickJsRuntime::with_defaults();
        let result = runtime
            .invoke_module_graph(
                graph("export async function render() { return await new Promise(resolve => setTimeout(() => resolve('timer-ok'), 0)); }"),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await
            .unwrap();
        assert!(matches!(result, JsValue::String(value) if value == "timer-ok"));
    }

    #[tokio::test]
    async fn quickjs_concurrent_streams_do_not_retire_each_other() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::with_defaults();
        let env = RuntimeEnvironment {
            ssr: true,
            request_id: None,
        };
        let source = r#"export function render(url) {
          let index = 0;
          const chunks = ['<' + url + '>', '</' + url + '>'];
          return new ReadableStream({pull(controller) {
            if (index < chunks.length) controller.enqueue(chunks[index++]);
            else controller.close();
          }});
        }"#;
        let mut first = runtime
            .invoke_module_graph_stream(
                graph(source),
                "render",
                vec![JsValue::String("one".into())],
                env.clone(),
            )
            .await
            .unwrap();
        assert_eq!(first.body.next().await.unwrap().unwrap(), b"<one>");
        let mut second = runtime
            .invoke_module_graph_stream(
                graph(source),
                "render",
                vec![JsValue::String("two".into())],
                env,
            )
            .await
            .unwrap();
        assert_eq!(second.body.next().await.unwrap().unwrap(), b"<two>");
        assert_eq!(first.body.next().await.unwrap().unwrap(), b"</one>");
        assert_eq!(second.body.next().await.unwrap().unwrap(), b"</two>");
        assert!(first.body.next().await.is_none());
        assert!(second.body.next().await.is_none());
    }

    #[tokio::test]
    async fn quickjs_stream_deadline_covers_all_body_reads() {
        use futures::StreamExt;
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            request_timeout_ms: 50,
            ..Default::default()
        });
        let response = runtime
            .invoke_module_graph_stream(
                graph(
                    r#"export function render() {
                  return new ReadableStream({
                    start(controller) { controller.enqueue('first'); },
                    pull() { while (true) {} }
                  });
                }"#,
                ),
                "render",
                vec![],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap();
        let chunks = response.body.collect::<Vec<_>>().await;
        assert_eq!(chunks[0].as_ref().unwrap(), b"first");
        let error = chunks[1].as_ref().unwrap_err().to_string();
        assert!(
            error.contains("deadline") || error.contains("interrupted"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn quickjs_omits_undefined_object_properties() {
        let runtime = QuickJsRuntime::with_defaults();
        let result = runtime
            .invoke_module_graph(
                graph("export function render() { return {html:'ok',status:undefined,headers:undefined}; }"),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await
            .unwrap();
        let JsValue::Object(object) = result else {
            panic!("expected object")
        };
        assert!(object.contains_key("html"));
        assert!(!object.contains_key("status"));
        assert!(!object.contains_key("headers"));
    }

    #[tokio::test]
    async fn quickjs_stream_payload_limit_is_combined() {
        let graph = graph(&format!(
            "export function render() {{ return '{}'; }}",
            "g".repeat(900)
        ));
        let args = vec![JsValue::String("a".repeat(700))];
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            max_request_bytes: 1400,
            ..Default::default()
        });
        let result = runtime
            .invoke_module_graph_stream(
                graph,
                "render",
                args,
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await;
        let error = match result {
            Ok(_) => panic!("combined streaming payload must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("payload exceeds"), "{error}");
    }

    #[test]
    fn quickjs_counts_binary_payload_bytes_exactly() {
        assert_eq!(js_value_bytes(&JsValue::Bytes(vec![0; 1024])), 1024);
    }

    #[tokio::test]
    async fn quickjs_stream_workers_are_bounded_and_reused() {
        let runtime = QuickJsRuntime::new(QuickJsOptions {
            queue_capacity: 1,
            ..Default::default()
        });
        let open = |runtime: QuickJsRuntime| async move {
            runtime
                .invoke_module_graph_stream(
                    graph("export function render() { return new ReadableStream({start(c){c.enqueue('x')}}); }"),
                    "render",
                    vec![],
                    RuntimeEnvironment { ssr: true, request_id: None },
                )
                .await
        };
        let first = open(runtime.clone()).await.unwrap();
        let error = match open(runtime.clone()).await {
            Ok(_) => panic!("second stream must respect the configured capacity"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("streaming capacity exhausted"),
            "{error}"
        );
        drop(first);
        open(runtime).await.unwrap();
    }

    #[tokio::test]
    async fn quickjs_timers_wait_wall_clock_and_support_cancellation() {
        let runtime = QuickJsRuntime::with_defaults();
        let started = Instant::now();
        let result = runtime
            .invoke_module_graph(
                graph("export async function render() { return await new Promise(resolve => setTimeout(() => resolve('delayed'), 25)); }"),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await
            .unwrap();
        assert!(matches!(result, JsValue::String(value) if value == "delayed"));
        assert!(
            started.elapsed() >= Duration::from_millis(25),
            "{:?}",
            started.elapsed()
        );
        let result = runtime
            .invoke_module_graph(
                graph("export async function render() { const id = setTimeout(() => {}, 0); clearTimeout(id); return 'cancelled'; }"),
                "render",
                vec![],
                RuntimeEnvironment { ssr: true, request_id: None },
            )
            .await;
        assert!(result.is_ok());
    }
}
