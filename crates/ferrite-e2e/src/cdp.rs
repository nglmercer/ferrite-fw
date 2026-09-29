//! Minimal async Chrome DevTools Protocol (CDP) client.
//!
//! One WebSocket to the browser endpoint multiplexes every page target via
//! flattened sessions (`Target.attachToTarget` with `flatten: true`).
//! Commands carry an incrementing id; responses resolve pending oneshots
//! while method-only frames fan out as broadcast events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{SinkExt as _, StreamExt as _};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{E2eError, E2eResult};

/// A CDP event frame (`method` + optional `sessionId`).
#[derive(Debug, Clone)]
pub struct CdpEvent {
    /// Session id for flattened target events.
    pub session: Option<String>,
    /// Event method (`Page.loadEventFired`, ...).
    pub method: String,
    /// Event params.
    pub params: Value,
}

enum Outbound {
    Text(String),
    Close,
}

struct Inner {
    tx: mpsc::UnboundedSender<Outbound>,
    pending: Mutex<HashMap<u64, oneshot::Sender<E2eResult<Value>>>>,
    events: broadcast::Sender<CdpEvent>,
    next_id: AtomicU64,
}

/// Cloneable handle to a browser-level CDP connection.
#[derive(Clone)]
pub struct CdpConnection {
    inner: Arc<Inner>,
}

impl CdpConnection {
    /// Connect to a browser WebSocket debugger URL.
    pub async fn connect(ws_url: &str) -> E2eResult<Self> {
        let (stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|error| E2eError::Launch(format!("cdp connect {ws_url}: {error}")))?;
        let (mut sink, mut stream) = stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Outbound>();
        let (events, _) = broadcast::channel::<CdpEvent>(4096);
        let inner = Arc::new(Inner {
            tx,
            pending: Mutex::new(HashMap::new()),
            events,
            next_id: AtomicU64::new(1),
        });

        // Writer task.
        let writer_pending = Arc::clone(&inner);
        tokio::spawn(async move {
            while let Some(outbound) = rx.recv().await {
                let done = matches!(outbound, Outbound::Close);
                if !done {
                    let Outbound::Text(text) = outbound else {
                        continue;
                    };
                    if sink.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                } else {
                    let _ = sink.send(Message::Close(None)).await;
                    let _ = sink.close().await;
                    break;
                }
            }
            fail_all(&writer_pending, "cdp writer ended");
        });

        // Reader task.
        let reader = Arc::clone(&inner);
        tokio::spawn(async move {
            loop {
                match stream.next().await {
                    Some(Ok(Message::Text(text))) => handle_frame(&reader, &text),
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            fail_all(&reader, "cdp connection closed");
        });

        Ok(Self { inner })
    }

    /// Subscribe to protocol events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.inner.events.subscribe()
    }

    /// Send a command on an optional session and await its `result`.
    pub async fn call(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> E2eResult<Value> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let mut frame = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(session) = session {
            frame["sessionId"] = Value::String(session.to_string());
        }
        let text = serde_json::to_string(&frame)?;
        let (tx, rx) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .map_err(|_| E2eError::Disconnected("pending lock poisoned".to_string()))?
            .insert(id, tx);
        self.inner
            .tx
            .send(Outbound::Text(text))
            .map_err(|_| E2eError::Disconnected("cdp writer gone".to_string()))?;
        let timeout_ms = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| {
                self.inner.pending.lock().map(|mut p| p.remove(&id)).ok();
                E2eError::Timeout(timeout_ms, format!("cdp {method}"))
            })?
            .map_err(|_| E2eError::Disconnected(format!("cdp {method} dropped")))?
    }

    /// Fire-and-forget variant that still surfaces transport errors.
    pub async fn send(&self, session: Option<&str>, method: &str, params: Value) -> E2eResult<()> {
        self.call(session, method, params, Duration::from_secs(30))
            .await?;
        Ok(())
    }

    /// Close the underlying socket.
    pub fn close(&self) {
        let _ = self.inner.tx.send(Outbound::Close);
    }

    /// Whether the writer end is still open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.inner.tx.is_closed()
    }
}

fn handle_frame(inner: &Arc<Inner>, text: &str) {
    let Ok(frame) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if let Some(id) = frame.get("id").and_then(Value::as_u64) {
        let sender = inner
            .pending
            .lock()
            .map(|mut pending| pending.remove(&id))
            .unwrap_or(None);
        if let Some(sender) = sender {
            if let Some(error) = frame.get("error") {
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown cdp error")
                    .to_string();
                let _ = sender.send(Err(E2eError::Cdp {
                    method: "cdp".to_string(),
                    message,
                }));
            } else {
                let result = frame.get("result").cloned().unwrap_or(Value::Null);
                let _ = sender.send(Ok(result));
            }
        }
        return;
    }
    if let Some(method) = frame.get("method").and_then(Value::as_str) {
        let event = CdpEvent {
            session: frame
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_string),
            method: method.to_string(),
            params: frame.get("params").cloned().unwrap_or(Value::Null),
        };
        let _ = inner.events.send(event);
    }
}

fn fail_all(inner: &Arc<Inner>, reason: &str) {
    let senders = inner
        .pending
        .lock()
        .map(|mut pending| std::mem::take(&mut *pending))
        .unwrap_or_default();
    for (_, sender) in senders {
        let _ = sender.send(Err(E2eError::Disconnected(reason.to_string())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_frame_parses() {
        let text = r#"{"method":"Page.loadEventFired","sessionId":"ABC","params":{"timestamp":1}}"#;
        let frame: Value = serde_json::from_str(text).unwrap();
        assert_eq!(frame["method"], "Page.loadEventFired");
        assert_eq!(frame["sessionId"], "ABC");
    }
}
