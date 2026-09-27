//! Hot module replacement (spec §32–§35).
//!
//! Defines the WebSocket protocol spoken at `/@ferrite/hmr`, the server-side
//! broadcast hub, the graph-based update planner, and the injected browser
//! client (`/@ferrite/client`).

use ferrite_core::{Diagnostic, ModuleId};
use ferrite_graph::ModuleGraph;
use tokio::sync::broadcast;

/// WebSocket endpoint path.
pub const HMR_ENDPOINT: &str = "/@ferrite/hmr";
/// Injected client module id.
pub const CLIENT_ID: &str = "/@ferrite/client";

/// Protocol messages sent from server to client (§32).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum HmrMessage {
    /// Initial handshake.
    Connected {
        /// Protocol version.
        version: String,
    },
    /// One or more modules changed.
    Update {
        /// Module updates.
        updates: Vec<HmrUpdate>,
    },
    /// No boundary accepted the change; reload the page.
    #[serde(rename = "full-reload")]
    FullReload {
        /// Changed path, when known.
        path: Option<String>,
    },
    /// Plugin-defined event.
    Custom {
        /// Event name.
        event: String,
        /// Event payload.
        data: serde_json::Value,
    },
    /// Transform/resolve/runtime error for the overlay.
    Error {
        /// Error payload.
        err: Diagnostic,
    },
    /// Prune dead modules from the client graph.
    Prune {
        /// Paths to prune.
        paths: Vec<String>,
    },
}

/// A single module update (§32).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HmrUpdate {
    /// `js-update` or `css-update`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Changed module path.
    pub path: String,
    /// Boundary path that accepted the update.
    #[serde(rename = "acceptedPath")]
    pub accepted_path: String,
    /// Change timestamp (unix millis).
    pub timestamp: u64,
    /// CSS-only updates skip re-execution.
    #[serde(default, skip_serializing_if = "is_false")]
    pub css_only: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

/// Planned HMR action for a file change.
#[derive(Debug, Clone)]
pub enum HmrPlan {
    /// Send an update chain.
    Update(Vec<HmrUpdate>),
    /// Send a full reload.
    FullReload,
}

/// Plan an HMR action for `changed` using the graph boundary walk (§34).
pub fn plan_update(graph: &ModuleGraph, changed: &ModuleId, timestamp: u64) -> HmrPlan {
    match graph.hmr_boundaries(changed) {
        Some(chain) => {
            let accepted = chain.first().cloned().unwrap_or_else(|| changed.clone());
            let node = graph.get(changed);
            let is_css = node
                .as_ref()
                .is_some_and(|node| node.module_type == ferrite_core::ModuleType::Css);
            HmrPlan::Update(vec![HmrUpdate {
                kind: if is_css {
                    "css-update".to_string()
                } else {
                    "js-update".to_string()
                },
                path: changed.0.clone(),
                accepted_path: accepted.0,
                timestamp,
                css_only: is_css,
            }])
        }
        None => HmrPlan::FullReload,
    }
}

/// Server-side HMR broadcast hub.
#[derive(Debug, Clone)]
pub struct HmrServer {
    /// Broadcast channel for protocol messages.
    tx: broadcast::Sender<String>,
}

impl HmrServer {
    /// Create a hub with room for `capacity` buffered messages.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(16));
        Self { tx }
    }

    /// Subscribe to protocol messages (one subscription per socket).
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Broadcast a protocol message.
    pub fn send(&self, message: &HmrMessage) {
        if let Ok(text) = serde_json::to_string(message) {
            let _ = self.tx.send(text);
        }
    }

    /// Broadcast module updates.
    pub fn send_update(&self, updates: Vec<HmrUpdate>) {
        self.send(&HmrMessage::Update { updates });
    }

    /// Broadcast a full reload.
    pub fn send_full_reload(&self, path: Option<String>) {
        self.send(&HmrMessage::FullReload { path });
    }

    /// Broadcast an error for the overlay (§35).
    pub fn send_error(&self, err: Diagnostic) {
        self.send(&HmrMessage::Error { err });
    }

    /// Number of connected subscribers.
    #[must_use]
    pub fn receivers(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for HmrServer {
    fn default() -> Self {
        Self::new(64)
    }
}

/// The injected browser client (`/@ferrite/client`).
///
/// Implements `import.meta.hot` (`accept`/`dispose`/`prune`/`invalidate`/
#[must_use]
pub fn client_source() -> &'static str {
    include_str!("client.js")
}

/// TypeScript declarations for the HMR client API (§33).
#[must_use]
pub fn client_dts() -> &'static str {
    r#"interface HotContext {
  data: Record<string, any>;
  accept(): void;
  accept(cb: (mod: any) => void): void;
  accept(dep: string, cb?: (mod: any) => void): void;
  accept(deps: string[], cb?: (mods: any[]) => void): void;
  dispose(cb: (data: Record<string, any>) => void): void;
  prune(cb: () => void): void;
  invalidate(message?: string): void;
  on(event: string, cb: (...args: any[]) => void): void;
  off(event: string, cb: (...args: any[]) => void): void;
  send(event: string, data?: unknown): void;
}
interface ImportMeta {
  hot?: HotContext;
  env: Record<string, string | boolean>;
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_serializes() {
        let message = HmrMessage::Update {
            updates: vec![HmrUpdate {
                kind: "js-update".to_string(),
                path: "/src/App.tsx".to_string(),
                accepted_path: "/src/App.tsx".to_string(),
                timestamp: 123,
                css_only: false,
            }],
        };
        let text = serde_json::to_string(&message).unwrap();
        assert!(text.contains("\"type\":\"update\""));
        assert!(text.contains("acceptedPath"));
    }

    #[test]
    fn css_update_planned() {
        let graph = ModuleGraph::new();
        let node = ferrite_graph::ModuleNode::new(
            ModuleId::new("/a.css"),
            "/a.css".to_string(),
            ferrite_core::ModuleType::Css,
        );
        graph.upsert(node);
        let plan = plan_update(&graph, &ModuleId::new("/a.css"), 1);
        assert!(matches!(plan, HmrPlan::Update(_)));
    }
}
