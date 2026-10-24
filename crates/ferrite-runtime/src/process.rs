//! Linux fresh-process graph invocation with address-space/CPU ceilings.
//! This profile does not restrict filesystem or network access.
use crate::{
    CompiledModule, CompiledModuleGraph, JsRuntime, JsValue, ModuleNamespace, RuntimeEnvironment,
};
use ferrite_core::{FerriteError, Result};
use std::{
    collections::BTreeSet,
    os::fd::{AsRawFd, FromRawFd},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Semaphore,
};
/// Hidden entry point handled before application argument parsing.
pub const WORKER_ARG: &str = "--ferrite-runtime-worker";
mod streaming;
const MAX_FRAME: usize = 64 * 1024 * 1024;
/// Zero resource settings select bounded defaults.
#[derive(Clone, Debug, Default)]
pub struct ProcessOptions {
    /// Worker binary (defaults to current executable, which must handle WORKER_ARG).
    pub executable: Option<PathBuf>,
    /// Virtual address space ceiling (default 2 GiB).
    pub memory_limit_bytes: u64,
    /// Simultaneous children (default four).
    pub concurrency: usize,
    /// Engine settings; zero deadline selects five seconds for this profile.
    pub engine: crate::napi_vm::NapiVmOptions,
}
struct State {
    slots: Arc<Semaphore>,
    active: Mutex<BTreeSet<u32>>,
}
/// Clones share admission and shutdown. Each render starts a fresh child.
#[derive(Clone)]
pub struct ProcessRuntime {
    options: ProcessOptions,
    state: Arc<State>,
}
fn error(value: impl std::fmt::Display) -> FerriteError {
    FerriteError::Runtime(format!("process runtime: {value}"))
}
impl ProcessRuntime {
    /// Construct without starting a worker.
    pub fn new(options: ProcessOptions) -> Self {
        let concurrency = if options.concurrency == 0 {
            4
        } else {
            options.concurrency
        };
        Self {
            options,
            state: Arc::new(State {
                slots: Arc::new(Semaphore::new(concurrency)),
                active: Mutex::new(BTreeSet::new()),
            }),
        }
    }
    /// Registered child leaders; queued invocations are not included.
    #[must_use]
    pub fn active_children(&self) -> usize {
        self.state.active.lock().unwrap().len()
    }

    /// Reject queued/new work and kill active child process groups.
    pub fn shutdown(&self) {
        self.state.slots.close();
        for pid in self.state.active.lock().unwrap().iter() {
            kill_group(*pid);
        }
    }
    fn spawn_worker(
        &self,
        timeout: u64,
        worker_arg: &str,
    ) -> Result<(tokio::net::UnixStream, ChildGuard, u64, u64)> {
        let (parent, child_socket) = std::os::unix::net::UnixStream::pair().map_err(error)?;
        parent.set_nonblocking(true).map_err(error)?;
        let socket = tokio::net::UnixStream::from_std(parent).map_err(error)?;
        let fd = child_socket.as_raw_fd();
        let memory = if self.options.memory_limit_bytes == 0 {
            2 * 1024 * 1024 * 1024
        } else {
            self.options.memory_limit_bytes
        };
        let cpu = timeout.div_ceil(1000).max(1);
        let executable = match &self.options.executable {
            Some(path) => path.clone(),
            None => std::env::current_exe().map_err(error)?,
        };
        let mut command = tokio::process::Command::new(executable);
        command
            .arg(worker_arg)
            .env("TOKIO_WORKER_THREADS", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        // Only async-signal-safe syscalls between fork and exec. Native
        // stdout cannot corrupt the dedicated protocol descriptor.
        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) != 0
                    || libc::dup2(fd, 3) < 0
                    || libc::fcntl(3, libc::F_SETFD, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                let address = libc::rlimit {
                    rlim_cur: memory,
                    rlim_max: memory,
                };
                let seconds = libc::rlimit {
                    rlim_cur: cpu,
                    rlim_max: cpu,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &address) != 0
                    || libc::setrlimit(libc::RLIMIT_CPU, &seconds) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let (child, pid) = {
            // Registration and shutdown are serialized. No untracked child
            // can be spawned after shutdown.
            let mut active = self.state.active.lock().unwrap();
            if self.state.slots.is_closed() {
                return Err(error("shut down"));
            }
            let child = command.spawn().map_err(error)?;
            let pid = child.id().ok_or_else(|| error("missing child PID"))?;
            active.insert(pid);
            (child, pid)
        };
        let guard = ChildGuard {
            child,
            pid,
            state: self.state.clone(),
        };
        drop(child_socket);
        Ok((socket, guard, memory, cpu))
    }
    async fn invoke(&self, request: Request) -> Result<JsValue> {
        let timeout = if self.options.engine.request_timeout_ms == 0 {
            5000
        } else {
            self.options.engine.request_timeout_ms
        };
        tokio::time::timeout(Duration::from_millis(timeout), async {
            let _permit = self
                .state
                .slots
                .acquire()
                .await
                .map_err(|_| error("shut down"))?;
            let payload = serde_json::to_vec(&request).map_err(error)?;
            let limit = if self.options.engine.max_request_bytes == 0 {
                8 * 1024 * 1024
            } else {
                self.options.engine.max_request_bytes.min(MAX_FRAME)
            };
            if payload.len() > limit {
                return Err(error("request exceeds payload limit"));
            }
            let (mut socket, mut guard, memory, cpu) = self.spawn_worker(timeout, WORKER_ARG)?;
            socket.write_all(&payload).await.map_err(error)?;
            socket.shutdown().await.map_err(error)?;
            let mut response = Vec::new();
            (&mut socket)
                .take((MAX_FRAME + 1) as u64)
                .read_to_end(&mut response)
                .await
                .map_err(error)?;
            if response.len() > MAX_FRAME {
                return Err(error("response exceeds payload limit"));
            }
            // Signal descendants while leader is unreaped; never signal a
            // recycled PID. Drop/timeout follows the same ordering.
            guard.stop();
            guard.child.wait().await.map_err(error)?;
            let response: Response = serde_json::from_slice(&response)
                .map_err(|e| error(format!("worker exited without a valid response: {e}")))?;
            if response.address_space != memory || response.cpu_seconds != cpu {
                return Err(error(
                    "worker resource limits differ from requested ceilings",
                ));
            }
            response.result.map_err(error)
        })
        .await
        .map_err(|_| error("request deadline exceeded"))?
    }
}
fn kill_group(pid: u32) {
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}
struct ChildGuard {
    child: tokio::process::Child,
    pid: u32,
    state: Arc<State>,
}
impl ChildGuard {
    fn stop(&mut self) {
        if self.state.active.lock().unwrap().remove(&self.pid) {
            kill_group(self.pid);
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.stop();
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    graph: CompiledModuleGraph,
    export: String,
    args: Vec<JsValue>,
    env: RuntimeEnvironment,
    options: crate::napi_vm::NapiVmOptions,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    result: std::result::Result<JsValue, String>,
    address_space: u64,
    cpu_seconds: u64,
}
#[async_trait::async_trait]
impl JsRuntime for ProcessRuntime {
    fn name(&self) -> &'static str {
        "napi-vm-process"
    }
    async fn evaluate_module(
        &self,
        _: CompiledModule,
        _: RuntimeEnvironment,
    ) -> Result<ModuleNamespace> {
        Err(error(
            "only atomic graph invocation is supported; handles cannot cross processes",
        ))
    }
    async fn invoke_module_graph(
        &self,
        graph: CompiledModuleGraph,
        export: &str,
        args: Vec<JsValue>,
        env: RuntimeEnvironment,
    ) -> Result<JsValue> {
        graph.validate()?;
        for arg in &args {
            crate::js_value_to_json(arg)?;
        }
        self.invoke(Request {
            version: 1,
            graph,
            export: export.into(),
            args,
            env,
            options: self.options.engine.clone(),
        })
        .await
    }
    async fn invoke_module_graph_stream(
        &self,
        graph: CompiledModuleGraph,
        export: &str,
        args: Vec<JsValue>,
        env: RuntimeEnvironment,
    ) -> Result<crate::JsStreamResponse> {
        streaming::invoke(self, graph, export, args, env).await
    }
}
/// Handle the worker flag before parsing application arguments. Exit when true.
pub async fn run_worker_if_requested() -> bool {
    let arg = std::env::args_os().nth(1);
    let result = if arg.as_deref() == Some(std::ffi::OsStr::new(WORKER_ARG)) {
        worker().await
    } else if arg.as_deref() == Some(std::ffi::OsStr::new(streaming::WORKER_ARG)) {
        streaming::worker().await
    } else {
        return false;
    };
    if result.is_err() {
        std::process::exit(1);
    }
    true
}
async fn worker() -> Result<()> {
    let mut stream = worker_socket()?;
    let mut bytes = Vec::new();
    (&mut stream)
        .take((MAX_FRAME + 1) as u64)
        .read_to_end(&mut bytes)
        .await
        .map_err(error)?;
    if bytes.len() > MAX_FRAME {
        return Err(error("request exceeds transport limit"));
    }
    let request: Request = serde_json::from_slice(&bytes).map_err(error)?;
    if request.version != 1 {
        return Err(error("unsupported protocol"));
    }
    let runtime = crate::napi_vm::NapiVmRuntime::new(request.options);
    let result = runtime
        .invoke_module_graph(request.graph, &request.export, request.args, request.env)
        .await
        .and_then(|value| {
            crate::js_value_to_json(&value)?;
            Ok(value)
        })
        .map_err(|e| e.to_string());
    let mut address = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let mut cpu = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut address) } != 0
        || unsafe { libc::getrlimit(libc::RLIMIT_CPU, &mut cpu) } != 0
    {
        return Err(error("cannot verify child limits"));
    }
    let bytes = serde_json::to_vec(&Response {
        result,
        address_space: address.rlim_cur,
        cpu_seconds: cpu.rlim_cur,
    })
    .map_err(error)?;
    if bytes.len() > MAX_FRAME {
        return Err(error("response exceeds transport limit"));
    }
    stream.write_all(&bytes).await.map_err(error)?;
    stream.shutdown().await.map_err(error)?;
    Ok(())
}

fn worker_socket() -> Result<tokio::net::UnixStream> {
    // Reject manual invocations without a protocol socket before taking ownership.
    let mut kind: libc::c_int = 0;
    let mut length = std::mem::size_of_val(&kind) as libc::socklen_t;
    if unsafe {
        libc::getsockopt(
            3,
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut kind as *mut libc::c_int).cast(),
            &mut length,
        )
    } != 0
        || kind != libc::SOCK_STREAM
    {
        return Err(error("worker requires its private protocol socket"));
    }
    // Descriptor 3 belongs exclusively to the parent protocol.
    let stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(3) };
    stream.set_nonblocking(true).map_err(error)?;
    tokio::net::UnixStream::from_std(stream).map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime(timeout: u64, memory: u64) -> ProcessRuntime {
        let executable = std::env::var_os("FERRITE_CLI_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/ferrite")
            });
        assert!(
            executable.is_file(),
            "build ferrite-cli --features napi-vm first"
        );
        ProcessRuntime::new(ProcessOptions {
            executable: Some(executable),
            memory_limit_bytes: memory,
            concurrency: 1,
            engine: crate::napi_vm::NapiVmOptions {
                request_timeout_ms: timeout,
                fuel_budget: u64::MAX,
                loop_budget: u64::MAX,
                ..Default::default()
            },
        })
    }
    async fn render(runtime: &ProcessRuntime, code: &str) -> Result<JsValue> {
        runtime
            .invoke_module_graph(
                CompiledModuleGraph {
                    entry: "entry".into(),
                    modules: vec![CompiledModule {
                        id: "entry".into(),
                        code: code.into(),
                        url: None,
                    }],
                },
                "render",
                vec![],
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await
    }
    async fn active_pid(runtime: &ProcessRuntime) -> u32 {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let pid = runtime.state.active.lock().unwrap().iter().next().copied();
                if let Some(pid) = pid {
                    break pid;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    #[ignore = "requires the feature-enabled Ferrite CLI worker executable"]
    async fn process_deadline_cancellation_limits_and_recovery() {
        let runtime = runtime(1000, 0);
        assert!(
            matches!(render(&runtime, "export function render(){return 'ok'}").await.unwrap(), JsValue::String(value) if value == "ok")
        );
        let error = render(&runtime, "export function render(){while(true){}}")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("deadline"), "{error}");
        assert!(runtime.state.active.lock().unwrap().is_empty());
        assert!(render(&runtime, "export function render(){return 42}")
            .await
            .is_ok());
        let running = runtime.clone();
        let task = tokio::spawn(async move {
            render(&running, "export function render(){while(true){}}").await
        });
        let pid = active_pid(&runtime).await;
        task.abort();
        let _ = task.await;
        tokio::time::timeout(Duration::from_secs(2), async {
            while unsafe { libc::kill(pid as i32, 0) } == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled child must be reaped");
        assert!(runtime.state.active.lock().unwrap().is_empty());
        let running = runtime.clone();
        let task = tokio::spawn(async move {
            render(&running, "export function render(){while(true){}}").await
        });
        active_pid(&runtime).await;
        runtime.shutdown();
        assert!(task.await.unwrap().is_err());
        assert!(render(&runtime, "export function render(){return 1}")
            .await
            .unwrap_err()
            .to_string()
            .contains("shut down"));
        let limited = self::runtime(2000, 8 * 1024 * 1024);
        assert!(render(&limited, "export function render(){return 1}")
            .await
            .is_err());
        let recovered = self::runtime(2000, 0);
        assert!(render(&recovered, "export function render(){return 1}")
            .await
            .is_ok());
    }
    #[tokio::test]
    #[ignore = "requires the feature-enabled Ferrite CLI worker executable"]
    async fn process_queue_and_payload_are_bounded() {
        let mut runtime = runtime(300, 0);
        runtime.options.engine.max_request_bytes = 100;
        assert!(render(&runtime, "export function render(){return 1}")
            .await
            .unwrap_err()
            .to_string()
            .contains("payload limit"));
        runtime.options.engine.max_request_bytes = 0;
        let permit = runtime.state.slots.acquire().await.unwrap();
        assert!(render(&runtime, "export function render(){return 1}")
            .await
            .unwrap_err()
            .to_string()
            .contains("deadline"));
        assert!(runtime.state.active.lock().unwrap().is_empty());
        drop(permit);
    }
}
