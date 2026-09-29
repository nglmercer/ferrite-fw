//! Minimal async WebDriver BiDi client (Firefox).
//!
//! One WebSocket per Firefox process (`ws://127.0.0.1:<port>/session`,
//! exactly one session per connection). Commands carry an incrementing id;
//! responses resolve pending oneshots while `type: "event"` frames fan out
//! as broadcast events. Verified against Firefox 156.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{SinkExt as _, StreamExt as _};
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{E2eError, E2eResult};

/// A BiDi event frame (`type: "event"`).
#[derive(Debug, Clone)]
pub struct BidiEvent {
    /// Event method (`browsingContext.load`, `log.entryAdded`, ...).
    pub method: String,
    /// Event params (carries `context` for context-scoped events).
    pub params: Value,
}

impl BidiEvent {
    /// Browsing context id for context-scoped events.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.params
            .get("context")
            .and_then(Value::as_str)
            .or_else(|| {
                self.params
                    .get("source")
                    .and_then(|s| s.get("context"))
                    .and_then(Value::as_str)
            })
    }
}

enum Outbound {
    Text(String),
    Close,
}

struct Inner {
    tx: mpsc::UnboundedSender<Outbound>,
    pending: Mutex<HashMap<u64, oneshot::Sender<E2eResult<Value>>>>,
    events: broadcast::Sender<BidiEvent>,
    next_id: AtomicU64,
}

/// Reclaims a command when its waiting future is dropped.
struct PendingCall {
    inner: Arc<Inner>,
    id: u64,
}
impl Drop for PendingCall {
    fn drop(&mut self) {
        self.inner
            .pending
            .lock()
            .map(|mut pending| pending.remove(&self.id))
            .ok();
    }
}

/// Cloneable handle to a browser-level BiDi connection.
#[derive(Clone)]
pub struct BidiConnection {
    inner: Arc<Inner>,
}

impl BidiConnection {
    /// Connect to a BiDi WebSocket URL (`.../session`).
    pub async fn connect(ws_url: &str) -> E2eResult<Self> {
        let (stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|error| E2eError::Launch(format!("bidi connect {ws_url}: {error}")))?;
        let (mut sink, mut stream) = stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Outbound>();
        let (events, _) = broadcast::channel::<BidiEvent>(4096);
        let inner = Arc::new(Inner {
            tx,
            pending: Mutex::new(HashMap::new()),
            events,
            next_id: AtomicU64::new(1),
        });

        let writer_pending = Arc::clone(&inner);
        tokio::spawn(async move {
            while let Some(outbound) = rx.recv().await {
                if matches!(outbound, Outbound::Close) {
                    let _ = sink.send(Message::Close(None)).await;
                    let _ = sink.close().await;
                    break;
                }
                let Outbound::Text(text) = outbound else {
                    continue;
                };
                if sink.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            fail_all(&writer_pending, "bidi writer ended");
        });

        let reader = Arc::clone(&inner);
        tokio::spawn(async move {
            loop {
                match stream.next().await {
                    Some(Ok(Message::Text(text))) => handle_frame(&reader, text.as_str()),
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            fail_all(&reader, "bidi connection closed");
        });

        Ok(Self { inner })
    }

    /// Subscribe to protocol events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<BidiEvent> {
        self.inner.events.subscribe()
    }

    /// Send a command and await its `result`.
    pub async fn call(&self, method: &str, params: Value, timeout: Duration) -> E2eResult<Value> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let frame = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
        });
        let text = serde_json::to_string(&frame)?;
        let (tx, rx) = oneshot::channel();
        self.inner
            .pending
            .lock()
            .map_err(|_| E2eError::Disconnected("pending lock poisoned".to_string()))?
            .insert(id, tx);
        let _pending = PendingCall {
            inner: self.inner.clone(),
            id,
        };
        self.inner
            .tx
            .send(Outbound::Text(text))
            .map_err(|_| E2eError::Disconnected("bidi writer gone".to_string()))?;
        crate::operation::Deadline::new(timeout)
            .run(format!("bidi {method}"), async {
                rx.await
                    .map_err(|_| E2eError::Disconnected(format!("bidi {method} dropped")))?
            })
            .await
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
    let kind = frame
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind == "event" {
        if let Some(method) = frame.get("method").and_then(Value::as_str) {
            let event = BidiEvent {
                method: method.to_string(),
                params: frame.get("params").cloned().unwrap_or(Value::Null),
            };
            let _ = inner.events.send(event);
        }
        return;
    }
    if let Some(id) = frame.get("id").and_then(Value::as_u64) {
        let sender = inner
            .pending
            .lock()
            .map(|mut pending| pending.remove(&id))
            .unwrap_or(None);
        if let Some(sender) = sender {
            if kind == "error" {
                let code = frame.get("error").and_then(Value::as_str).unwrap_or("bidi");
                let message = frame
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown bidi error");
                let _ = sender.send(Err(E2eError::Cdp {
                    method: format!("bidi:{code}"),
                    message: message.to_string(),
                }));
            } else {
                let result = frame.get("result").cloned().unwrap_or(Value::Null);
                let _ = sender.send(Ok(result));
            }
        }
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

/// Convert a BiDi RemoteValue to plain JSON.
///
/// Primitives clone through; `array` maps items; `object` maps its
/// `[[key, value]]` entries; `undefined`/unknown shapes become null.
#[must_use]
pub fn remote_to_json(value: &Value) -> Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        return value.clone();
    };
    match kind {
        "string" | "number" | "boolean" | "bigint" => {
            object.get("value").cloned().unwrap_or(Value::Null)
        }
        "array" | "set" => object
            .get("value")
            .and_then(Value::as_array)
            .map(|items| Value::Array(items.iter().map(remote_to_json).collect()))
            .unwrap_or(Value::Null),
        "object" | "map" => {
            let mut map = serde_json::Map::new();
            if let Some(entries) = object.get("value").and_then(Value::as_array) {
                for entry in entries {
                    if let Some(pair) = entry.as_array() {
                        if let (Some(key), Some(val)) = (pair.first(), pair.get(1)) {
                            let key = key
                                .as_str()
                                .map_or_else(|| remote_to_json(key).to_string(), str::to_string);
                            map.insert(key, remote_to_json(val));
                        }
                    }
                }
            }
            Value::Object(map)
        }
        _ => Value::Null,
    }
}

/// Extract a BytesValue (`{type, value}`) string.
#[must_use]
pub fn bytes_to_string(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    value
        .get("value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn zero_timeout_waits_for_reply_and_cancellation_reclaims_pending_call() {
        let (events, _) = broadcast::channel(4);
        let (tx, mut outgoing) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            tx,
            pending: Mutex::new(HashMap::new()),
            events,
            next_id: AtomicU64::new(1),
        });
        let connection = BidiConnection {
            inner: inner.clone(),
        };
        let token = crate::CancellationToken::new();
        let (result, ()) = tokio::join!(
            token.run(connection.call("test", Value::Null, Duration::ZERO)),
            async {
                outgoing.recv().await.unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
                assert_eq!(inner.pending.lock().unwrap().len(), 1);
                token.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert!(inner.pending.lock().unwrap().is_empty());
    }

    use serde_json::json;

    #[test]
    fn primitives_convert() {
        assert_eq!(
            remote_to_json(&json!({"type": "string", "value": "hi"})),
            json!("hi")
        );
        assert_eq!(
            remote_to_json(&json!({"type": "number", "value": 3})),
            json!(3)
        );
        assert_eq!(
            remote_to_json(&json!({"type": "boolean", "value": true})),
            json!(true)
        );
        assert_eq!(remote_to_json(&json!({"type": "null"})), Value::Null);
        assert_eq!(remote_to_json(&json!({"type": "undefined"})), Value::Null);
    }

    #[test]
    fn nested_objects_convert() {
        // Observed Firefox 156 shape.
        let remote = json!({
            "type": "object",
            "value": [
                ["a", {"type": "number", "value": 1}],
                ["b", {"type": "array", "value": [{"type": "boolean", "value": true}, {"type": "null"}]}],
                ["c", {"type": "object", "value": [["d", {"type": "string", "value": "x"}]]}],
            ],
        });
        assert_eq!(
            remote_to_json(&remote),
            json!({"a": 1, "b": [true, null], "c": {"d": "x"}})
        );
    }

    #[test]
    fn unknown_shapes_become_null() {
        assert_eq!(remote_to_json(&json!({"type": "window"})), Value::Null);
        assert_eq!(remote_to_json(&json!({"type": "promise"})), Value::Null);
        // Plain JSON passes through untouched.
        assert_eq!(remote_to_json(&json!([1, 2])), json!([1, 2]));
    }

    #[test]
    fn bytes_value_unwraps() {
        assert_eq!(
            bytes_to_string(&json!({"type": "string", "value": "v"})),
            "v"
        );
        assert_eq!(bytes_to_string(&json!("raw")), "raw");
    }

    #[test]
    fn event_context_reads_params_and_source() {
        let direct = BidiEvent {
            method: "browsingContext.load".to_string(),
            params: json!({"context": "abc"}),
        };
        assert_eq!(direct.context(), Some("abc"));
        let nested = BidiEvent {
            method: "log.entryAdded".to_string(),
            params: json!({"source": {"context": "def"}}),
        };
        assert_eq!(nested.context(), Some("def"));
        let none = BidiEvent {
            method: "session.status".to_string(),
            params: json!({}),
        };
        assert_eq!(none.context(), None);
    }
}
