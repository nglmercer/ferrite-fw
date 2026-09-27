//! napi-vm embedded backend (feature `napi-vm`, **disabled by default**).
//!
//! Runs SSR JavaScript in-process through
//! [napi-vm](https://github.com/nglmercer/napi-vm) (pure-Rust core, no Node
//! required) and loads real `.node` binaries through its in-process Node-API
//! backend (`node-api-host`, via
//! [`Interpreter::enable_native_addons`](https://github.com/nglmercer/napi-vm/blob/main/docs/node-addon-runtime-plan.md)
//! with `RustNodeApiOptions`).
//!
//! Because the interpreter is `!Send`, it lives on a dedicated OS thread;
//! [`NapiVmRuntime`] itself is `Send + Sync` and communicates over a job
//! queue. Only JSON-serializable values cross the bridge; functions cross
//! as [`crate::JsHandle`]s resolved through a guest-side function table.
//!
//! Setup follows the napi-vm contract (`docs/node-addon-runtime-plan.md`,
//! `src/interpreter/native_addon.rs`): one `enable_native_addons` call
//! installs the CommonJS loader, host bridge, and entry together, so this
//! backend never pre-installs a plain loader on the native path. The
//! returned [`NativeAddonRuntime`](https://github.com/nglmercer/napi-vm)
//! handle is retained for startup [`preflight`](https://github.com/nglmercer/napi-vm)
//! checks and for owner-thread [`shutdown`](https://github.com/nglmercer/napi-vm)
//! (finalizers/cleanup hooks) when the worker stops.
//!
//! Scope (inherited from napi-vm): the in-process host covers the **Node-API
//! C ABI** (including napi-rs addons that only use supported Node-API
//! symbols). Addons needing V8, NAN, Node C++, or libuv ABIs stay on
//! napi-vm's Node sidecar backend — napi-vm reports those as explicit
//! errors, never silent mis-execution.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use ferrite_core::{FerriteError, Result};

use crate::{
    js_value_to_json, CompiledModule, JsHandle, JsRuntime, JsValue, ModuleNamespace,
    RuntimeEnvironment,
};

/// Pinned napi-vm revision backing this integration.
pub const NAPI_VM_PIN: &str = "0fa987d8860d620cd1008a84f2a17c9b67c495cd";

/// One allowlisted native addon.
#[derive(Debug, Clone)]
pub struct NativeAddonAllow {
    /// Addon path (absolute, or relative to the first configured root).
    pub path: PathBuf,
    /// Expected SHA-256 (hex) — `None` pins nothing (discouraged).
    pub sha256_hex: Option<String>,
}

/// napi-vm backend options.
#[derive(Debug, Clone, Default)]
pub struct NapiVmOptions {
    /// Filesystem roots for the CJS loader and addon resolution.
    pub roots: Vec<PathBuf>,
    /// CJS entry filename for top-level `require()` resolution.
    pub entry: Option<String>,
    /// Native addon allowlist (empty = native loading disabled).
    pub native_allow: Vec<NativeAddonAllow>,
    /// Fuel budget (0 = napi-vm default).
    pub fuel_budget: u64,
    /// Loop budget (0 = napi-vm default).
    pub loop_budget: u64,
    /// Highest Node-API version the host reports/accepts (0 = napi-vm
    /// default 10; otherwise must be 1–10).
    pub max_napi_version: u32,
}

/// Worker jobs (all interpreter interaction stays on the worker thread).
enum Job {
    /// Define + evaluate an ES module; reply with its export record as JSON.
    EvalModule {
        id: String,
        code: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<serde_json::Value, String>>,
    },
    /// Call a guest function handle with JSON args; reply with JSON result.
    Call {
        handle: u64,
        args_json: String,
        reply: tokio::sync::oneshot::Sender<std::result::Result<serde_json::Value, String>>,
    },
    /// CJS-require a path (JS, JSON, or allowlisted `.node`).
    Require {
        request: String,
        parent: Option<String>,
        reply: tokio::sync::oneshot::Sender<std::result::Result<serde_json::Value, String>>,
    },
}

/// Shared worker state (last `Arc` drop stops and joins the thread).
struct Worker {
    jobs: Option<Sender<Job>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.jobs.take();
        if let Ok(mut slot) = self.thread.lock() {
            if let Some(handle) = slot.take() {
                let _ = handle.join();
            }
        }
    }
}

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
fn namespace_from_json(json: &serde_json::Value) -> Result<ModuleNamespace> {
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

/// Dedicated-thread event loop; owns the `!Send` interpreter.
fn worker_loop(options: NapiVmOptions, rx: Receiver<Job>) {
    let mut state = match WorkerState::new(options) {
        Ok(state) => state,
        Err(error) => {
            // Fatal init error: fail every job loudly.
            for job in rx {
                match job {
                    Job::EvalModule { reply, .. }
                    | Job::Call { reply, .. }
                    | Job::Require { reply, .. } => {
                        let _ = reply.send(Err(error.clone()));
                    }
                }
            }
            return;
        }
    };
    for job in rx {
        match job {
            Job::EvalModule { id, code, reply } => {
                let _ = reply.send(state.eval_module(&id, &code));
            }
            Job::Call {
                handle,
                args_json,
                reply,
            } => {
                let _ = reply.send(state.call(handle, &args_json));
            }
            Job::Require {
                request,
                parent,
                reply,
            } => {
                let _ = reply.send(state.require(&request, parent.as_deref()));
            }
        }
    }
    // Owner-thread shutdown: run cleanup hooks/finalizers before unload.
    // Errors here are teardown noise on a thread that is already exiting.
    if let Some(runtime) = state.runtime {
        let _ = runtime.shutdown();
    }
}

/// Interpreter + guest function table, confined to the worker thread.
struct WorkerState {
    interp: napi_vm::Interpreter,
    runtime: Option<napi_vm::NativeAddonRuntime>,
    next_handle: u64,
}

impl WorkerState {
    fn new(options: NapiVmOptions) -> std::result::Result<Self, String> {
        let mut interp = napi_vm::Interpreter::with_builtins();
        if options.fuel_budget > 0 {
            interp.set_fuel_budget(options.fuel_budget);
        }
        if options.loop_budget > 0 {
            interp.set_loop_budget(options.loop_budget);
        }
        // `enable_native_addons` installs the loader, host bridge, and entry
        // together; only install a plain loader when native is disabled.
        let runtime = if options.native_allow.is_empty() {
            if !options.roots.is_empty() {
                let loader = napi_vm::FileCommonJsLoader::new(&options.roots).map_err(vmkind)?;
                interp
                    .set_commonjs_loader(std::rc::Rc::new(loader))
                    .map_err(vmkind)?;
            }
            if let Some(entry) = options.entry {
                interp.set_commonjs_entry(entry);
            }
            None
        } else {
            let roots = if options.roots.is_empty() {
                vec![PathBuf::from(".")]
            } else {
                options.roots.clone()
            };
            let mut native = napi_vm::RustNodeApiOptions::new(&roots);
            if let Some(entry) = options.entry {
                // Validated by napi-vm: must exist and stay under roots.
                native = native.entry(entry);
            }
            if options.max_napi_version > 0 {
                native = native.max_napi_version(options.max_napi_version);
            }
            let mut allow_paths = Vec::with_capacity(options.native_allow.len());
            for allow in &options.native_allow {
                // napi-vm canonicalizes against the process CWD, so resolve
                // root-relative entries first for deterministic behavior.
                let path = resolve_allow_path(&roots, &allow.path);
                match &allow.sha256_hex {
                    Some(hex) => {
                        let bytes = parse_sha256(hex).map_err(|error| {
                            format!("bad sha256 for {}: {error}", allow.path.display())
                        })?;
                        native = native.allow_native_addon_with_sha256(path.clone(), bytes);
                    }
                    None => {
                        native = native.allow_native_addon(path.clone());
                    }
                }
                allow_paths.push(path);
            }
            let runtime = interp.enable_native_addons(native).map_err(vmkind)?;
            // Fail fast on wrong format/arch without running initializers.
            for path in &allow_paths {
                runtime.preflight_addon(path).map_err(vmkind)?;
            }
            Some(runtime)
        };
        // Guest-side function table for exported handles.
        interp
            .eval_source("globalThis.__ferrite_fns__ = globalThis.__ferrite_fns__ || {};")
            .map_err(vmkind)?;
        Ok(Self {
            interp,
            runtime,
            next_handle: 1,
        })
    }

    /// Define + evaluate a module; return its export record as JSON.
    ///
    /// `id` must not start with `.`: guest `import` of a relative name has
    /// no module context to resolve against.
    fn eval_module(
        &mut self,
        id: &str,
        code: &str,
    ) -> std::result::Result<serde_json::Value, String> {
        if id.starts_with('.') {
            return Err(format!(
                "napi-vm: module id `{id}` must not be relative; pass an absolute id or stable name"
            ));
        }
        self.interp.define_module(id, code.to_string());
        self.interp.ensure_module(id).map_err(vmkind)?;
        // Read exports through a guest `import` (live bindings), not the
        // Rust-side snapshot.
        let probe = format!(
            "import * as __ferrite_ns__ from {};\n\
             JSON.stringify(Object.keys(__ferrite_ns__).map(k => {{\n\
             const v = __ferrite_ns__[k];\n\
             const t = typeof v;\n\
             return [k, t, t === \"function\" ? null : (v === undefined ? null : v)];\n\
             }}))",
            js_string(id)
        );
        let value = self.interp.eval_source(&probe).map_err(vmkind)?;
        let json = napi_vm::value_to_json(&mut self.interp, &value).map_err(vmkind)?;
        let descriptors = json
            .as_str()
            .ok_or_else(|| "export probe failed".to_string())?;
        let descriptors: Vec<(String, String, serde_json::Value)> =
            serde_json::from_str(descriptors).map_err(|error| error.to_string())?;
        // Allocate handles for function exports; keep serialized values.
        let mut record = serde_json::Map::new();
        let mut stashed: Vec<(String, u64)> = Vec::new();
        for (name, kind, data) in descriptors {
            if kind == "function" {
                let handle = self.next_handle;
                self.next_handle += 1;
                record.insert(name.clone(), serde_json::json!({"handle": handle}));
                stashed.push((name, handle));
            } else {
                record.insert(name, serde_json::json!({"value": data}));
            }
        }
        // Stash function exports in the guest table under their handles.
        if !stashed.is_empty() {
            let mut script = format!("import * as __ferrite_ns__ from {};\n", js_string(id));
            script.push_str("globalThis.__ferrite_fns__ = globalThis.__ferrite_fns__ || {};\n");
            for (name, handle) in &stashed {
                let access = if name == "default" {
                    "__ferrite_ns__.default".to_string()
                } else {
                    format!("__ferrite_ns__[{}]", js_string(name))
                };
                script.push_str(&format!(
                    "globalThis.__ferrite_fns__[{handle}] = {access};\n"
                ));
            }
            self.interp.eval_source(&script).map_err(vmkind)?;
        }
        Ok(serde_json::Value::Object(record))
    }

    /// Call a guest function handle.
    fn call(
        &mut self,
        handle: u64,
        args_json: &str,
    ) -> std::result::Result<serde_json::Value, String> {
        let script = format!(
            "(() => {{\n\
             globalThis.__ferrite_async__ = null;\n\
             globalThis.__ferrite_async_err__ = null;\n\
             const f = globalThis.__ferrite_fns__[{handle}];\n\
             if (typeof f !== \"function\") throw new Error(\"handle {handle} is not a function\");\n\
             const r = f(...JSON.parse({args}));\n\
             if (r && typeof r.then === \"function\") {{\n\
             r.then(\n\
             v => {{ globalThis.__ferrite_async__ = JSON.stringify(v === undefined ? null : v); }},\n\
             e => {{ globalThis.__ferrite_async_err__ = String((e && e.stack) || e); }});\n\
             return \"__FERRITE_ASYNC__\";\n\
             }}\n\
             return JSON.stringify(r === undefined ? null : r);\n\
             }})()",
            args = js_string(args_json)
        );
        let value = self.interp.eval_source(&script).map_err(vmkind)?;
        let json = napi_vm::value_to_json(&mut self.interp, &value).map_err(vmkind)?;
        if json.as_str() == Some("__FERRITE_ASYNC__") {
            self.interp.drain_jobs().map_err(vmkind)?;
            let err = self
                .interp
                .eval_source("globalThis.__ferrite_async_err__")
                .map_err(vmkind)?;
            let err = napi_vm::value_to_json(&mut self.interp, &err).map_err(vmkind)?;
            if let Some(message) = err.as_str() {
                return Err(format!("async guest error: {message}"));
            }
            let settled = self
                .interp
                .eval_source("globalThis.__ferrite_async__")
                .map_err(vmkind)?;
            let settled = napi_vm::value_to_json(&mut self.interp, &settled).map_err(vmkind)?;
            match settled.as_str() {
                Some(text) => serde_json::from_str(text)
                    .map_err(|error| format!("bad async result JSON: {error}")),
                None => Err("guest promise never settled".to_string()),
            }
        } else if let Some(text) = json.as_str() {
            serde_json::from_str(text).map_err(|error| format!("bad result JSON: {error}"))
        } else {
            Err("guest call did not return JSON".to_string())
        }
    }

    /// CJS-require a path.
    fn require(
        &mut self,
        request: &str,
        parent: Option<&str>,
    ) -> std::result::Result<serde_json::Value, String> {
        let value = self
            .interp
            .require_commonjs(request, parent)
            .map_err(vmkind)?;
        napi_vm::value_to_json(&mut self.interp, &value).map_err(vmkind)
    }
}

fn vmkind(error: napi_vm::VmErr) -> String {
    error.to_string()
}

/// Render a Rust string as a JS single-quoted literal.
fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

/// Resolve an allowlist path: absolute stays, relative joins the first root.
fn resolve_allow_path(roots: &[PathBuf], path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match roots.first() {
        Some(root) => root.join(path),
        None => path.to_path_buf(),
    }
}

/// Parse 64 hex chars into 32 bytes.
fn parse_sha256(hex: &str) -> std::result::Result<[u8; 32], String> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("expected 64 hex chars".to_string());
    }
    let mut bytes = [0u8; 32];
    for (index, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|error| error.to_string())?;
    }
    Ok(bytes)
}

/// Options snapshot the worker can report (debugging aid).
#[derive(Debug, Clone, Default)]
pub struct NapiVmInfo {
    /// Backend name.
    pub backend: String,
    /// Pinned revision.
    pub pin: String,
    /// Native loading enabled.
    pub native_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_string_escapes() {
        assert_eq!(js_string("a'b\\c"), "'a\\'b\\\\c'");
        assert_eq!(js_string("x\ny"), "'x\\ny'");
    }

    #[test]
    fn sha256_parses() {
        let hex = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        let bytes = parse_sha256(hex).unwrap();
        assert_eq!(bytes[0], 0xe3);
        assert!(parse_sha256("zz").is_err());
        assert!(parse_sha256(&hex[..62]).is_err());
    }

    #[tokio::test]
    async fn evaluates_and_calls_exports() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "entry".to_string(),
                    code: "export const answer = 40 + 2;\nexport function render(name) { return `hello ${name}`; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        assert!(
            matches!(namespace.get("answer"), Some(JsValue::Number(value)) if *value == 42.0),
            "{:?}",
            namespace.exports
        );
        let handle = namespace
            .get_function("render")
            .expect("render handle")
            .clone();
        let result = runtime
            .call(&handle, vec![JsValue::String("ferrite".to_string())])
            .await
            .expect("call");
        assert!(
            matches!(result, JsValue::String(ref text) if text == "hello ferrite"),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn awaits_async_exports() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "async-entry".to_string(),
                    code: "export async function fetch() { return { ok: true }; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        let handle = namespace.get_function("fetch").expect("handle").clone();
        let result = runtime.call(&handle, Vec::new()).await.expect("call");
        assert!(
            matches!(&result, JsValue::Object(map) if matches!(map.get("ok"), Some(JsValue::Bool(true)))),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn call_arguments_cross_verbatim() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "echo".to_string(),
                    code: "export function echo(x) { return [typeof x, x]; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("evaluate");
        let handle = namespace.get_function("echo").expect("handle").clone();
        let result = runtime
            .call(&handle, vec![JsValue::String("ferrite".to_string())])
            .await
            .expect("call");
        assert!(
            matches!(&result, JsValue::Array(items)
                if matches!(&items[..], [JsValue::String(t), JsValue::String(v)] if t == "string" && v == "ferrite")),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn relative_module_id_rejected_loudly() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .evaluate_module(
                CompiledModule {
                    id: "./relative.js".to_string(),
                    code: "export const x = 1;\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("must not be relative"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn absolute_path_id_evaluates() {
        let runtime = NapiVmRuntime::with_defaults();
        let namespace = runtime
            .evaluate_module(
                CompiledModule {
                    id: "/tmp/ferrite-smoke/src/entry-server.js".to_string(),
                    code: "export function render(url) { return url; }\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .expect("absolute id");
        assert!(namespace.get_function("render").is_ok());
    }

    #[tokio::test]
    async fn reports_guest_errors() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .evaluate_module(
                CompiledModule {
                    id: "broken".to_string(),
                    code: "export const x = ;;;\n".to_string(),
                    url: None,
                },
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().len() > 4, "{error}");
    }

    #[tokio::test]
    async fn native_requires_allowlist() {
        let runtime = NapiVmRuntime::with_defaults();
        let error = runtime
            .require("./addon.node", Some("/app/main.cjs"))
            .await
            .unwrap_err();
        // No loader configured at all: must fail loudly, never silently.
        assert!(error.to_string().len() > 4, "{error}");
    }

    #[tokio::test]
    async fn missing_allowlist_file_fails_loudly() {
        let options = NapiVmOptions {
            native_allow: vec![NativeAddonAllow {
                path: PathBuf::from("/nonexistent-ferrite addon/missing.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./x", None).await.unwrap_err();
        assert!(
            error.to_string().contains("missing.node"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn preflight_rejects_wrong_format_without_initializing() {
        let dir = std::env::temp_dir().join(format!("ferrite-preflight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: dir.join("fake.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        assert!(error.to_string().len() > 8, "unexpected error: {error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn relative_allow_resolves_against_first_root() {
        let dir = std::env::temp_dir().join(format!("ferrite-relallow-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: PathBuf::from("fake.node"),
                sha256_hex: None,
            }],
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        // Resolution hit the file (no "cannot allow"/not-found); the failure
        // is the later preflight format check.
        assert!(
            !error.to_string().contains("cannot allow")
                && !error.to_string().contains("No such file"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bad_napi_ceiling_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ferrite-napiver-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("fake.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            native_allow: vec![NativeAddonAllow {
                path: dir.join("fake.node"),
                sha256_hex: None,
            }],
            max_napi_version: 99,
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let error = runtime.require("./main.cjs", None).await.unwrap_err();
        assert!(
            error.to_string().contains("99"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn unlisted_native_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ferrite-native-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.cjs"), "module.exports = {};\n").unwrap();
        std::fs::write(dir.join("evil.node"), b"not-an-elf").unwrap();
        let options = NapiVmOptions {
            roots: vec![dir.clone()],
            entry: Some(dir.join("main.cjs").to_string_lossy().into_owned()),
            ..Default::default()
        };
        let runtime = NapiVmRuntime::new(options);
        let parent = dir.join("main.cjs").to_string_lossy().into_owned();
        let error = runtime
            .require("./evil.node", Some(&parent))
            .await
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("allow")
                || error.to_string().to_lowercase().contains("denied")
                || error.to_string().to_lowercase().contains("native"),
            "unexpected error: {error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
