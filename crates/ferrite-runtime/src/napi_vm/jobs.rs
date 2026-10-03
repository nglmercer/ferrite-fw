//! napi-vm worker jobs.

use std::sync::mpsc::SyncSender;
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

/// Shared worker state (last `Arc` drop stops and joins the thread).
pub(crate) struct Worker {
    pub(crate) jobs: Option<SyncSender<Job>>,
    pub(crate) max_request_bytes: usize,
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
