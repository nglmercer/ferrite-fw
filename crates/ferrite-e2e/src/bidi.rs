//! Minimal async WebDriver BiDi client (Firefox).
//!
//! One WebSocket per Firefox process (`ws://127.0.0.1:<port>/session`,
//! exactly one session per connection). Commands carry an incrementing id;
//! responses resolve pending oneshots while `type: "event"` frames fan out
//! as broadcast events. Verified against Firefox 156.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};
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
    closed: crate::CancellationToken,
    user_context_preloads: AtomicBool,
    lifecycle_events: AtomicU8,
    tx: mpsc::UnboundedSender<Outbound>,
    pending: Mutex<HashMap<u64, oneshot::Sender<E2eResult<Value>>>>,
    intercepts: Mutex<HashMap<u64, InterceptEntry>>,
    events: broadcast::Sender<BidiEvent>,
    popup_events: broadcast::Sender<BidiEvent>,
    popup_captures: Mutex<Option<Arc<crate::popup_capture::PopupCaptures>>>,
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

// Response ownership must be established in the reader, before waking the caller.
// Otherwise cancellation between response delivery and polling loses the native ID.
enum InterceptEntry {
    Pending {
        lease: Weak<InterceptLease>,
        context: String,
        frames: Weak<Mutex<crate::lifecycle_events::FrameEvents>>,
        blocked: Vec<String>,
        overflow: crate::CancellationToken,
    },
    Removing {
        intercept: String,
        failed: bool,
    },
}
struct InterceptLease {
    inner: Weak<Inner>,
    command: AtomicU64,
    frames: Weak<Mutex<crate::lifecycle_events::FrameEvents>>,
    overflow: crate::CancellationToken,
    id: Mutex<Option<String>>,
}
impl Drop for InterceptLease {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        let mut entries = inner.intercepts.lock().unwrap_or_else(|e| e.into_inner());
        let command = self.command.load(Ordering::Acquire);
        if let Some(InterceptEntry::Pending { blocked, .. }) = entries.get_mut(&command) {
            for request in blocked.drain(..) {
                continue_request_unobserved(&inner, &request);
            }
        }
        if let Some(id) = self.id.get_mut().unwrap_or_else(|e| e.into_inner()).take() {
            entries.remove(&command);
            remove_intercept_unobserved(&inner, &mut entries, &id);
        }
    }
}
fn continue_request_unobserved(inner: &Inner, request: &str) {
    if inner.closed.is_cancelled() {
        return;
    }
    let id = inner.next_id.fetch_add(1, Ordering::SeqCst);
    let frame = serde_json::json!({"id":id,"method":"network.continueRequest","params":{"request":request}});
    let _ = inner.tx.send(Outbound::Text(frame.to_string()));
}
fn remove_intercept_unobserved(
    inner: &Inner,
    entries: &mut HashMap<u64, InterceptEntry>,
    intercept: &str,
) {
    if inner.closed.is_cancelled() {
        return;
    }
    let id = inner.next_id.fetch_add(1, Ordering::SeqCst);
    let frame = serde_json::json!({"id":id,"method":"network.removeIntercept","params":{"intercept":intercept}});
    entries.insert(
        id,
        InterceptEntry::Removing {
            intercept: intercept.to_owned(),
            failed: false,
        },
    );
    if inner.tx.send(Outbound::Text(frame.to_string())).is_err() {
        entries.remove(&id);
    }
}

// removeIntercept does not release requests Firefox already paused. Continue
// abandoned startup's requests in the reader, including the removal ACK window.
fn recover_abandoned_interception(inner: &Inner, event: &BidiEvent) {
    if event.method != "network.beforeRequestSent" || event.params["isBlocked"] != true {
        return;
    }
    let Some(request) = event.params["request"]["request"]
        .as_str()
        .filter(|id| !id.is_empty())
    else {
        return;
    };
    let mut entries = inner.intercepts.lock().unwrap_or_else(|e| e.into_inner());
    let mut abandoned = false;
    for entry in entries.values_mut() {
        match entry {
            InterceptEntry::Pending {
                lease,
                context,
                frames,
                blocked,
                overflow,
            } => {
                if !event.context().is_some_and(|id| {
                    id == context
                        || frames.upgrade().is_some_and(|frames| {
                            frames
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .contains(id)
                        })
                }) {
                    continue;
                }
                if lease.strong_count() == 0 || overflow.is_cancelled() {
                    abandoned = true;
                } else if !blocked.iter().any(|id| id == request) {
                    if blocked.len() >= 256 {
                        overflow.cancel_with_reason(
                            "256 blocked requests during BiDi intercept startup",
                        );
                        abandoned = true;
                    } else {
                        blocked.push(request.to_owned());
                    }
                }
            }
            InterceptEntry::Removing { intercept, .. } => {
                abandoned |= event.params["intercepts"]
                    .as_array()
                    .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(intercept)));
            }
        }
    }
    if abandoned {
        continue_request_unobserved(inner, request);
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
            closed: crate::CancellationToken::new(),
            user_context_preloads: AtomicBool::new(false),
            lifecycle_events: AtomicU8::new(0),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            intercepts: Mutex::new(HashMap::new()),
            events,
            popup_events: broadcast::channel(256).0,
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
    pub(crate) fn subscribe_popups(&self) -> broadcast::Receiver<BidiEvent> {
        self.inner.popup_events.subscribe()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BidiEvent> {
        self.inner.events.subscribe()
    }

    pub(crate) fn set_popup_captures(&self, captures: Arc<crate::popup_capture::PopupCaptures>) {
        *self
            .inner
            .popup_captures
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(captures);
    }
    pub(crate) fn popup_captures(&self) -> Option<Arc<crate::popup_capture::PopupCaptures>> {
        self.inner
            .popup_captures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(crate) fn release_popup_context(&self, id: Option<&str>) {
        if let Some(captures) = self.popup_captures() {
            captures.release_context(id);
        }
    }

    pub(crate) fn disconnection(&self) -> crate::CancellationToken {
        self.inner.closed.clone()
    }

    pub(crate) fn set_browser_version(&self, version: &str) {
        let supported = version
            .split('.')
            .next()
            .and_then(|major| major.parse::<u32>().ok())
            .is_some_and(|major| major >= 136);
        self.inner
            .user_context_preloads
            .store(supported, Ordering::Release);
    }
    pub(crate) fn supports_user_context_preloads(&self) -> bool {
        self.inner.user_context_preloads.load(Ordering::Acquire)
    }
    pub(crate) fn supports_lifecycle_event(&self, event: &str) -> bool {
        self.inner.lifecycle_events.load(Ordering::Acquire) & lifecycle_event_bit(event) != 0
    }

    /// Send a command and await its `result`.
    pub async fn call(&self, method: &str, params: Value, timeout: Duration) -> E2eResult<Value> {
        self.call_owned(method, params, timeout, None).await
    }

    pub(crate) async fn add_intercept(
        &self,
        params: Value,
        timeout: Duration,
        frames: Weak<Mutex<crate::lifecycle_events::FrameEvents>>,
    ) -> E2eResult<String> {
        let lease = Arc::new(InterceptLease {
            inner: Arc::downgrade(&self.inner),
            command: AtomicU64::new(0),
            frames,
            overflow: crate::CancellationToken::new(),
            id: Mutex::new(None),
        });
        lease
            .overflow
            .run(self.call_owned(
                "network.addIntercept",
                params,
                timeout,
                Some(Arc::downgrade(&lease)),
            ))
            .await
            .map_err(|error| match error {
                E2eError::Cancelled(reason) if lease.overflow.is_cancelled() => {
                    E2eError::Config(reason)
                }
                other => other,
            })?;
        let id = {
            let mut entries = self
                .inner
                .intercepts
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            entries.remove(&lease.command.load(Ordering::Acquire));
            lease.id.lock().unwrap_or_else(|e| e.into_inner()).take()
        };
        id.ok_or_else(|| {
            E2eError::Config("BiDi addIntercept returned no nonempty intercept ID".into())
        })
    }

    async fn call_owned(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        intercept: Option<Weak<InterceptLease>>,
    ) -> E2eResult<Value> {
        if let Some(reason) = self.inner.closed.reason() {
            return Err(E2eError::Disconnected(reason));
        }
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        if let Some(intercept) = intercept {
            let mut pending = self
                .inner
                .intercepts
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if pending
                .values()
                .any(|entry| matches!(entry, InterceptEntry::Removing { failed: true, .. }))
            {
                return Err(E2eError::Config(
                    "BiDi abandoned intercept cleanup failed; reconnect before reinstalling routes"
                        .into(),
                ));
            }
            let context = params["contexts"][0].as_str().unwrap_or_default();
            if pending.values().any(|entry| matches!(entry, InterceptEntry::Pending { context: existing, .. } if existing == context)) {
                return Err(E2eError::Config("BiDi intercept startup for this context is still awaiting its native response".into()));
            }
            if pending.len() >= 256 {
                return Err(E2eError::Config(
                    "256 pending BiDi intercept installations; await responses or reconnect".into(),
                ));
            }
            let (frames, overflow) = if let Some(lease) = intercept.upgrade() {
                lease.command.store(id, Ordering::Release);
                (lease.frames.clone(), lease.overflow.clone())
            } else {
                Default::default()
            };
            let context = params["contexts"][0]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            pending.insert(
                id,
                InterceptEntry::Pending {
                    lease: intercept,
                    context,
                    frames,
                    blocked: Vec::new(),
                    overflow,
                },
            );
        }
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
        if self.inner.tx.send(Outbound::Text(text)).is_err() {
            self.inner
                .intercepts
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            return Err(E2eError::Disconnected("bidi writer gone".into()));
        }
        let result=crate::operation::Deadline::new(timeout)
            .run(format!("bidi {method}"), async {
                tokio::select! {biased;
                    reason=self.inner.closed.cancelled()=>Err(E2eError::Disconnected(reason)),
                    result=rx=>result.map_err(|_|E2eError::Disconnected(format!("bidi {method} dropped")))?,
                }
            })
            .await?;
        if method == "session.subscribe" {
            if let Some(events) = frame["params"]["events"].as_array() {
                for event in events.iter().filter_map(Value::as_str) {
                    self.inner
                        .lifecycle_events
                        .fetch_or(lifecycle_event_bit(event), Ordering::Release);
                }
            }
        }
        Ok(result)
    }

    /// Close the underlying socket.
    pub fn close(&self) {
        let _ = self.inner.tx.send(Outbound::Close);
    }

    /// Whether the transport reader and writer are still connected.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.inner.tx.is_closed() && !self.inner.closed.is_cancelled()
    }
}

fn lifecycle_event_bit(event: &str) -> u8 {
    match event {
        "browsingContext.navigationCommitted" => 1,
        "browsingContext.fragmentNavigated" => 2,
        "browsingContext.historyUpdated" => 4,
        "browsingContext.userPromptClosed" => 8,
        _ => 0,
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
            recover_abandoned_interception(inner, &event);
            let captures = inner
                .popup_captures
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if let Some(captures) = captures {
                captures.bidi(&event, text.len());
            }
            if matches!(
                event.method.as_str(),
                "browsingContext.contextCreated" | "browsingContext.contextDestroyed"
            ) {
                let _ = inner.popup_events.send(event.clone());
            }
            let _ = inner.events.send(event);
        }
        return;
    }
    if let Some(id) = frame.get("id").and_then(Value::as_u64) {
        // Keep an upgraded lease alive until the table lock is released: its
        // destructor also locks this table when the caller has just disappeared.
        let mut live_lease = None;
        {
            let mut entries = inner.intercepts.lock().unwrap_or_else(|e| e.into_inner());
            match entries.remove(&id) {
                Some(InterceptEntry::Pending {
                    lease,
                    context,
                    frames,
                    blocked,
                    overflow,
                }) => {
                    if kind == "error"
                        || frame["result"]["intercept"]
                            .as_str()
                            .filter(|id| !id.is_empty())
                            .is_none()
                    {
                        for request in &blocked {
                            continue_request_unobserved(inner, request);
                        }
                    }
                    if kind != "error" {
                        if let Some(intercept) = frame["result"]["intercept"]
                            .as_str()
                            .filter(|id| !id.is_empty())
                        {
                            if let Some(owner) = lease.upgrade() {
                                *owner.id.lock().unwrap_or_else(|e| e.into_inner()) =
                                    Some(intercept.to_owned());
                                entries.insert(
                                    id,
                                    InterceptEntry::Pending {
                                        lease,
                                        context,
                                        frames,
                                        blocked,
                                        overflow,
                                    },
                                );
                                live_lease = Some(owner);
                            } else {
                                for request in blocked {
                                    continue_request_unobserved(inner, &request);
                                }
                                remove_intercept_unobserved(inner, &mut entries, intercept);
                            }
                        }
                    }
                }
                Some(InterceptEntry::Removing { intercept, .. }) if kind == "error" => {
                    entries.insert(
                        id,
                        InterceptEntry::Removing {
                            intercept,
                            failed: true,
                        },
                    );
                }
                _ => {}
            }
        }
        drop(live_lease);
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
    let captures = inner
        .popup_captures
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(captures) = captures {
        captures.disconnect(reason);
    }
    inner.closed.cancel_with_reason(reason);
    inner
        .intercepts
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
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
    fn intercept_fixture() -> (BidiConnection, mpsc::UnboundedReceiver<Outbound>) {
        let (tx, outgoing) = mpsc::unbounded_channel();
        (
            BidiConnection {
                inner: Arc::new(Inner {
                    closed: crate::CancellationToken::new(),
                    user_context_preloads: AtomicBool::new(false),
                    lifecycle_events: AtomicU8::new(0),
                    tx,
                    pending: Mutex::new(HashMap::new()),
                    intercepts: Mutex::new(HashMap::new()),
                    events: broadcast::channel(4).0,
                    popup_events: broadcast::channel(4).0,
                    popup_captures: Mutex::new(None),
                    next_id: AtomicU64::new(1),
                }),
            },
            outgoing,
        )
    }
    fn outgoing_frame(outgoing: &mut mpsc::UnboundedReceiver<Outbound>) -> Value {
        let Outbound::Text(text) = outgoing.try_recv().expect("queued native command") else {
            panic!("unexpected close")
        };
        serde_json::from_str(&text).unwrap()
    }
    fn response(connection: &BidiConnection, command: &Value, result: Value) {
        handle_frame(
            &connection.inner,
            &json!({"type":"success","id":command["id"],"result":result}).to_string(),
        );
    }
    fn blocked(connection: &BidiConnection, context: &str, intercept: &str, request: &str) {
        handle_frame(&connection.inner, &json!({"type":"event","method":"network.beforeRequestSent","params":{"context":context,"isBlocked":true,"intercepts":[intercept],"request":{"request":request}}}).to_string());
    }

    #[tokio::test]
    async fn cancelled_intercept_releases_late_id_and_already_blocked_requests() {
        let (connection, mut outgoing) = intercept_fixture();
        let frames = Arc::new(Mutex::new(crate::lifecycle_events::FrameEvents::default()));
        frames.lock().unwrap().seed_bidi_tree(
            "page",
            &json!([{"context":"page","children":[{"context":"child"}]}]),
        );
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Arc::downgrade(&frames),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        blocked(&connection, "child", "late", "already-observed");
        assert!(outgoing.try_recv().is_err());
        drop(setup);
        assert_eq!(
            outgoing_frame(&mut outgoing)["params"]["request"],
            "already-observed"
        );
        assert!(connection.inner.pending.lock().unwrap().is_empty());
        blocked(&connection, "other", "late", "unrelated");
        assert!(outgoing.try_recv().is_err());
        blocked(&connection, "page", "late", "before-reply");
        let resumed = outgoing_frame(&mut outgoing);
        assert_eq!(resumed["method"], "network.continueRequest");
        assert_eq!(resumed["params"]["request"], "before-reply");
        blocked(&connection, "child", "late", "child-before-reply");
        assert_eq!(
            outgoing_frame(&mut outgoing)["params"]["request"],
            "child-before-reply"
        );
        response(&connection, &command, json!({"intercept":"late"}));
        let removal = outgoing_frame(&mut outgoing);
        assert_eq!(removal["method"], "network.removeIntercept");
        assert_eq!(removal["params"]["intercept"], "late");
        // Intercept IDs also correlate descendant-context requests after the ID arrives.
        blocked(&connection, "child", "late", "after-reply");
        assert_eq!(
            outgoing_frame(&mut outgoing)["params"]["request"],
            "after-reply"
        );
        response(&connection, &removal, json!({}));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        assert!(connection.is_open());
    }

    #[tokio::test]
    async fn delivered_intercept_response_still_has_an_owner_until_consumed() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":"delivered"}));
        assert_eq!(connection.inner.intercepts.lock().unwrap().len(), 1);
        drop(setup);
        let removal = outgoing_frame(&mut outgoing);
        assert_eq!(removal["params"]["intercept"], "delivered");
        response(&connection, &removal, json!({}));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());

        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":"committed"}));
        assert_eq!(setup.await.unwrap(), "committed");
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        assert!(outgoing.try_recv().is_err());
    }

    #[tokio::test]
    async fn disconnect_after_intercept_delivery_does_not_recreate_cleanup_metadata() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":"disconnected"}));
        fail_all(&connection.inner, "fixture disconnected after delivery");
        assert!(matches!(setup.await, Err(E2eError::Disconnected(_))));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        assert!(outgoing.try_recv().is_err());
    }

    #[tokio::test]
    async fn intercept_admission_stays_bounded_until_native_removal_acknowledgement() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut commands = Vec::new();
        for index in 0..256 {
            let mut setup = Box::pin(connection.add_intercept(
                json!({"contexts":[format!("page-{index}")]}),
                Duration::ZERO,
                Default::default(),
            ));
            assert!(futures::poll!(&mut setup).is_pending());
            commands.push(outgoing_frame(&mut outgoing));
            drop(setup);
        }
        assert!(matches!(
            connection
                .add_intercept(
                    json!({"contexts":["page"]}),
                    Duration::ZERO,
                    Default::default()
                )
                .await,
            Err(E2eError::Config(_))
        ));
        assert!(outgoing.try_recv().is_err());
        response(&connection, &commands[0], json!({"intercept":"retiring"}));
        let removal = outgoing_frame(&mut outgoing);
        assert_eq!(connection.inner.intercepts.lock().unwrap().len(), 256);
        assert!(matches!(
            connection
                .add_intercept(
                    json!({"contexts":["page"]}),
                    Duration::ZERO,
                    Default::default()
                )
                .await,
            Err(E2eError::Config(_))
        ));
        response(&connection, &removal, json!({}));
        assert_eq!(connection.inner.intercepts.lock().unwrap().len(), 255);
        fail_all(&connection.inner, "fixture disconnected");
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        for command in commands.iter().skip(1) {
            response(&connection, command, json!({"intercept":"obsolete"}));
        }
        assert!(outgoing.try_recv().is_err());
    }

    #[tokio::test]
    async fn blocked_startup_overflow_fails_loudly_and_releases_all_observed_requests() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        for index in 0..256 {
            blocked(&connection, "page", "overflow", &format!("request-{index}"));
        }
        assert!(outgoing.try_recv().is_err());
        blocked(&connection, "page", "overflow", "request-256");
        assert!(
            matches!(setup.await, Err(E2eError::Config(message)) if message.contains("256 blocked requests"))
        );
        let mut released = std::collections::HashSet::new();
        for _ in 0..257 {
            let continued = outgoing_frame(&mut outgoing);
            assert_eq!(continued["method"], "network.continueRequest");
            assert!(released.insert(continued["params"]["request"].as_str().unwrap().to_owned()));
        }
        assert!(outgoing.try_recv().is_err());
        assert!(
            matches!(connection.add_intercept(json!({"contexts":["page"]}), Duration::ZERO, Default::default()).await, Err(E2eError::Config(message)) if message.contains("still awaiting"))
        );
        response(&connection, &command, json!({"intercept":"overflow"}));
        let removal = outgoing_frame(&mut outgoing);
        response(&connection, &removal, json!({}));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        let mut retry = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut retry).is_pending());
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":"retried"}));
        assert_eq!(retry.await.unwrap(), "retried");
        assert!(outgoing.try_recv().is_err());
    }

    #[tokio::test]
    async fn rejected_cleanup_stays_visible_and_releases_blocked_requests() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        drop(setup);
        response(&connection, &command, json!({"intercept":"failed-removal"}));
        let removal = outgoing_frame(&mut outgoing);
        handle_frame(&connection.inner, &json!({"type":"error","id":removal["id"],"error":"unknown error","message":"fixture refused cleanup"}).to_string());
        assert_eq!(connection.inner.intercepts.lock().unwrap().len(), 1);
        assert!(
            matches!(connection.add_intercept(json!({"contexts":["page"]}), Duration::ZERO, Default::default()).await, Err(E2eError::Config(message)) if message.contains("cleanup failed"))
        );
        blocked(
            &connection,
            "child",
            "failed-removal",
            "recover-after-error",
        );
        assert_eq!(
            outgoing_frame(&mut outgoing)["params"]["request"],
            "recover-after-error"
        );
        assert!(outgoing.try_recv().is_err());
        fail_all(&connection.inner, "fixture disconnected");
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_intercept_reply_and_timeout_preserve_explicit_outcomes() {
        let (connection, mut outgoing) = intercept_fixture();
        let mut setup = Box::pin(connection.add_intercept(
            json!({"contexts":["page"]}),
            Duration::ZERO,
            Default::default(),
        ));
        assert!(futures::poll!(&mut setup).is_pending());
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":""}));
        assert!(matches!(setup.await, Err(E2eError::Config(_))));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
        assert!(matches!(
            connection
                .add_intercept(
                    json!({"contexts":["page"]}),
                    Duration::from_millis(1),
                    Default::default()
                )
                .await,
            Err(E2eError::Timeout { .. })
        ));
        let command = outgoing_frame(&mut outgoing);
        response(&connection, &command, json!({"intercept":"timed-out"}));
        let removal = outgoing_frame(&mut outgoing);
        assert_eq!(removal["params"]["intercept"], "timed-out");
        response(&connection, &removal, json!({}));
        assert!(connection.inner.intercepts.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn zero_timeout_waits_for_reply_and_cancellation_reclaims_pending_call() {
        let (events, _) = broadcast::channel(4);
        let (tx, mut outgoing) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            closed: crate::CancellationToken::new(),
            user_context_preloads: AtomicBool::new(false),
            lifecycle_events: AtomicU8::new(0),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            intercepts: Mutex::new(HashMap::new()),
            events,
            popup_events: broadcast::channel(256).0,
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
