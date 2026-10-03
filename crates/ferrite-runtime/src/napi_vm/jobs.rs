//! napi-vm worker jobs.

use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread::JoinHandle;

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
pub(crate) struct Worker {
    pub(crate) jobs: Option<Sender<Job>>,
    pub(crate) thread: Mutex<Option<JoinHandle<()>>>,
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
