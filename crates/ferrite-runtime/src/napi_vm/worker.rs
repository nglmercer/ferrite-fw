//! napi-vm worker thread.

use super::jobs::*;
use super::options::*;
use super::util::*;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

/// Dedicated-thread event loop; owns the `!Send` interpreter.
pub(crate) fn worker_loop(options: NapiVmOptions, rx: Receiver<Job>) {
    let mut state = match WorkerState::new(options) {
        Ok(state) => state,
        Err(error) => {
            // Fatal init error: fail every job loudly.
            for job in rx {
                match job {
                    Job::EvalModule { reply, .. }
                    | Job::EvalGraph { reply, .. }
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
            Job::EvalGraph { graph, reply } => {
                let result = graph
                    .validate()
                    .map_err(|error| error.to_string())
                    .and_then(|()| {
                        for module in &graph.modules {
                            state.interp.remove_module(&module.id);
                        }
                        for module in &graph.modules {
                            state.interp.define_module(&module.id, module.code.clone());
                        }
                        let entry = graph
                            .modules
                            .iter()
                            .find(|module| module.id == graph.entry)
                            .expect("validated entry");
                        state.eval_module(&entry.id, &entry.code)
                    });
                let _ = reply.send(result);
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
pub(crate) struct WorkerState {
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
