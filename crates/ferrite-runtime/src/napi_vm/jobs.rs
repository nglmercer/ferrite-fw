//! napi-vm worker jobs.

use std::sync::mpsc::SyncSender;
use std::sync::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Worker jobs (all interpreter interaction stays on the worker thread).
pub(crate) enum Job {
    /// Define + evaluate an ES module; reply with its export record as JSON.
    EvalModule {
        id: String,
        code: String,
        ssr: bool,
        reply: tokio::sync::oneshot::Sender<std::result::Result<serde_json::Value, String>>,
    },
    /// Register a complete compiled graph before evaluating its entry.
    EvalGraph {
        graph: crate::CompiledModuleGraph,
        ssr: bool,
        reply: tokio::sync::oneshot::Sender<std::result::Result<serde_json::Value, String>>,
    },
    /// Evaluate and invoke without allowing another job between the steps.
    InvokeGraph {
        graph: crate::CompiledModuleGraph,
        export: String,
        args_json: String,
        ssr: bool,
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

impl Job {
    pub(crate) fn cancelled(&self) -> bool {
        match self {
            Self::EvalModule { reply, .. }
            | Self::EvalGraph { reply, .. }
            | Self::InvokeGraph { reply, .. }
            | Self::Call { reply, .. }
            | Self::Require { reply, .. } => reply.is_closed(),
        }
    }

    pub(crate) fn payload_bytes(&self) -> usize {
        let graph_bytes = |graph: &crate::CompiledModuleGraph| {
            graph
                .modules
                .iter()
                .fold(graph.entry.len(), |size, module| {
                    size.saturating_add(module.id.len())
                        .saturating_add(module.code.len())
                        .saturating_add(module.url.as_ref().map_or(0, String::len))
                })
        };
        match self {
            Self::EvalModule { id, code, .. } => id.len().saturating_add(code.len()),
            Self::EvalGraph { graph, .. } => graph_bytes(graph),
            Self::InvokeGraph {
                graph,
                export,
                args_json,
                ..
            } => graph_bytes(graph)
                .saturating_add(export.len())
                .saturating_add(args_json.len()),
            Self::Call { args_json, .. } => args_json.len(),
            Self::Require {
                request, parent, ..
            } => request
                .len()
                .saturating_add(parent.as_ref().map_or(0, String::len)),
        }
    }
}

/// One end-to-end request budget, including time spent waiting in the queue.
#[derive(Clone)]
pub(crate) struct RequestControl {
    pub(crate) cancellation: napi_vm::CancellationToken,
    pub(crate) deadline: Option<Instant>,
}

impl RequestControl {
    pub(crate) fn new(timeout_ms: u64) -> Self {
        Self {
            cancellation: napi_vm::CancellationToken::default(),
            deadline: (timeout_ms != 0).then(|| Instant::now() + Duration::from_millis(timeout_ms)),
        }
    }

    pub(crate) fn check(&self) -> Result<(), String> {
        if self.cancellation.is_cancelled() {
            return Err("napi-vm request cancelled or worker shut down".into());
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err("napi-vm request deadline exceeded (including queue time)".into());
        }
        Ok(())
    }
}

pub(crate) struct Request {
    pub(crate) job: Job,
    pub(crate) control: RequestControl,
}

/// Cancels running guest code when an awaiting future is dropped.
pub(crate) struct RequestGuard(pub(crate) napi_vm::CancellationToken);
impl std::fmt::Debug for RequestGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RequestGuard")
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[derive(Default)]
pub(crate) struct Lifecycle {
    pub(crate) stopped: AtomicBool,
    pub(crate) active: Mutex<Option<napi_vm::CancellationToken>>,
}

impl Lifecycle {
    pub(crate) fn stop(&self) {
        // Synchronize with admission so shutdown cannot miss a starting request.
        let active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.stopped.store(true, Ordering::Release);
        if let Some(token) = active.as_ref() {
            token.cancel();
        }
    }
}

/// Shared worker state. Dropping the last owner cancels active guest execution.
pub(crate) struct Worker {
    pub(crate) jobs: Mutex<Option<SyncSender<Request>>>,
    pub(crate) max_request_bytes: usize,
    pub(crate) request_timeout_ms: u64,
    pub(crate) shutdown_timeout_ms: u64,
    pub(crate) lifecycle: Arc<Lifecycle>,
    pub(crate) thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.lifecycle.stop();
        self.jobs
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(handle) = self
            .thread
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let deadline = Instant::now() + Duration::from_millis(self.shutdown_timeout_ms);
            while !handle.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
            // A native host call may be uninterruptible. Detach after the bound;
            // do not claim hard process isolation or block the caller forever.
        }
    }
}
