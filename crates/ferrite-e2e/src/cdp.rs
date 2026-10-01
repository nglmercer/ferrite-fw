//! Minimal async Chrome DevTools Protocol (CDP) client.
//!
//! One WebSocket to the browser endpoint multiplexes every page target via
//! flattened sessions (`Target.attachToTarget` with `flatten: true`).
//! Commands carry an incrementing id; responses resolve pending oneshots
//! while method-only frames fan out as broadcast events.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
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

/// One browser download tracked from `Browser.download*` events.
#[derive(Debug, Clone)]
pub struct DownloadRecord {
    /// Download GUID (for `Browser.cancelDownload`).
    pub guid: String,
    /// Source URL.
    pub url: String,
    /// Suggested file name.
    pub filename: String,
    /// Latest progress state (`inProgress`, `completed`, `canceled`,
    /// `interrupted`).
    pub state: String,
}

/// Tracked downloads kept per connection (oldest dropped first).
const MAX_DOWNLOAD_RECORDS: usize = 64;

enum Outbound {
    Text(String),
    Close,
}

struct Inner {
    closed: crate::CancellationToken,
    tx: mpsc::UnboundedSender<Outbound>,
    pending: Mutex<HashMap<u64, oneshot::Sender<E2eResult<Value>>>>,
    events: broadcast::Sender<CdpEvent>,
    popup_events: broadcast::Sender<CdpEvent>,
    popup_captures: Mutex<Option<Arc<crate::popup_capture::PopupCaptures>>>,
    downloads: Mutex<VecDeque<DownloadRecord>>,
    next_id: AtomicU64,
    attachments: Mutex<HashMap<u64, Weak<crate::cdp_session::SessionState>>>,
    sessions: Mutex<HashMap<String, Weak<crate::cdp_session::SessionState>>>,
}

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
            closed: crate::CancellationToken::new(),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            events,
            popup_events: broadcast::channel(256).0,
            downloads: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            attachments: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
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
    pub(crate) fn subscribe_popups(&self) -> broadcast::Receiver<CdpEvent> {
        self.inner.popup_events.subscribe()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.inner.events.subscribe()
    }

    /// Tracked browser downloads, oldest first (flows only while download
    /// events are enabled via [`crate::page::Page::set_download_dir`]).
    #[must_use]
    pub fn download_records(&self) -> Vec<DownloadRecord> {
        self.inner
            .downloads
            .lock()
            .map(|records| records.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn set_popup_captures(&self, captures: Arc<crate::popup_capture::PopupCaptures>) {
        let weak = Arc::downgrade(&self.inner);
        captures.set_resume(Arc::new(move |session| {
            if let Some(inner) = weak.upgrade() {
                let id = inner.next_id.fetch_add(1, Ordering::SeqCst);
                let frame = serde_json::json!({"id":id,"sessionId":session,"method":"Runtime.runIfWaitingForDebugger","params":{}});
                let _ = inner.tx.send(Outbound::Text(frame.to_string()));
            }
        }));
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

    /// Send a command on an optional session and await its `result`.
    pub async fn call(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> E2eResult<Value> {
        self.call_owned(session, method, params, timeout, None)
            .await
    }

    pub(crate) async fn call_owned(
        &self,
        session: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
        attachment: Option<Weak<crate::cdp_session::SessionState>>,
    ) -> E2eResult<Value> {
        if let Some(reason) = self.inner.closed.reason() {
            return Err(E2eError::Disconnected(reason));
        }
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        if let Some(attachment) = attachment {
            let mut attachments = self
                .inner
                .attachments
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if attachments.len() >= 256 {
                return Err(E2eError::Config(
                    "256 pending scoped CDP attachments; await responses or reconnect".into(),
                ));
            }
            attachments.insert(id, attachment);
        }
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
        let _pending = PendingCall {
            inner: self.inner.clone(),
            id,
        };
        if self.inner.tx.send(Outbound::Text(text)).is_err() {
            self.inner
                .attachments
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
            return Err(E2eError::Disconnected("cdp writer gone".into()));
        }
        crate::operation::Deadline::new(timeout)
            .run(format!("cdp {method}"), async {
                tokio::select! {biased;
                    reason=self.inner.closed.cancelled()=>Err(E2eError::Disconnected(reason)),
                    result=rx=>result.map_err(|_|E2eError::Disconnected(format!("cdp {method} dropped")))?,
                }
            })
            .await
    }

    pub(crate) fn forget_session(&self, session: &str) {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session);
    }

    pub(crate) fn detach_unobserved(&self, session: &str) {
        self.inner
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(session);
        detach_unobserved(&self.inner, session);
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

    /// Whether the transport reader and writer are still connected.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.inner.tx.is_closed() && !self.inner.closed.is_cancelled()
    }
}

fn handle_frame(inner: &Arc<Inner>, text: &str) {
    let Ok(frame) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if let Some(id) = frame.get("id").and_then(Value::as_u64) {
        let attachment = inner
            .attachments
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        if let Some(attachment) = attachment {
            if let Some(session) = frame["result"]["sessionId"].as_str() {
                if let Some(state) = attachment.upgrade() {
                    *state.id.lock().unwrap_or_else(|e| e.into_inner()) = Some(session.into());
                    inner
                        .sessions
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(session.into(), Arc::downgrade(&state));
                } else {
                    detach_unobserved(inner, session);
                }
            }
        }
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
        if method == "Browser.downloadWillBegin" || method == "Browser.downloadProgress" {
            track_download(inner, method, &frame);
        }
        let event = CdpEvent {
            session: frame
                .get("sessionId")
                .and_then(Value::as_str)
                .map(str::to_string),
            method: method.to_string(),
            params: frame.get("params").cloned().unwrap_or(Value::Null),
        };
        if method == "Target.detachedFromTarget" {
            if let Some(id) = event.params["sessionId"].as_str() {
                let state = inner
                    .sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(id)
                    .and_then(|s| s.upgrade());
                if let Some(state) = state {
                    state.native_gone.store(true, Ordering::Release);
                    state
                        .closed
                        .cancel_with_reason("CDP session detached by browser");
                }
            }
        }
        if let Some(id) = event.session.as_ref() {
            let state = inner
                .sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(id)
                .and_then(Weak::upgrade);
            if let Some(state) = state {
                let _ = state.events.send(event.clone());
            }
        }
        let captures = inner
            .popup_captures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(captures) = captures {
            captures.cdp(&event, text.len());
        }
        if matches!(
            event.method.as_str(),
            "Target.attachedToTarget" | "Target.targetDestroyed"
        ) {
            let _ = inner.popup_events.send(event.clone());
        }
        let _ = inner.events.send(event);
    }
}

/// Fold a download event into the connection's tracker (bounded).
fn track_download(inner: &Arc<Inner>, method: &str, frame: &Value) {
    let params = &frame["params"];
    let guid = params["guid"].as_str().unwrap_or_default();
    if guid.is_empty() {
        return;
    }
    let Ok(mut records) = inner.downloads.lock() else {
        return;
    };
    if method.ends_with("downloadWillBegin") {
        if records.iter().any(|record| record.guid == guid) {
            return;
        }
        records.push_back(DownloadRecord {
            guid: guid.to_string(),
            url: params["url"].as_str().unwrap_or_default().to_string(),
            filename: params["suggestedFilename"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            state: "inProgress".to_string(),
        });
    } else if let Some(record) = records.iter_mut().find(|record| record.guid == guid) {
        if let Some(state) = params["state"].as_str() {
            record.state = state.to_string();
        }
        if record.filename.is_empty() {
            record.filename = params["filePath"]
                .as_str()
                .and_then(|path| path.rsplit(['/', '\\']).next())
                .unwrap_or_default()
                .to_string();
        }
    }
    while records.len() > MAX_DOWNLOAD_RECORDS {
        records.pop_front();
    }
}

fn detach_unobserved(inner: &Arc<Inner>, session: &str) {
    let id = inner.next_id.fetch_add(1, Ordering::SeqCst);
    let frame = serde_json::json!({"id":id,"method":"Target.detachFromTarget","params":{"sessionId":session}});
    let _ = inner.tx.send(Outbound::Text(frame.to_string()));
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
        .attachments
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
    inner
        .sessions
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

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn zero_timeout_waits_for_reply_and_cancellation_reclaims_pending_call() {
        let (events, _) = broadcast::channel(4);
        let (tx, mut outgoing) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            closed: crate::CancellationToken::new(),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            events,
            popup_events: broadcast::channel(256).0,
            next_id: AtomicU64::new(1),
            attachments: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            downloads: Mutex::new(VecDeque::new()),
        });
        let connection = CdpConnection {
            inner: inner.clone(),
        };
        let token = crate::CancellationToken::new();
        let (result, ()) = tokio::join!(
            token.run(connection.call(None, "test", Value::Null, Duration::ZERO)),
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

    fn scoped_fixture() -> (CdpConnection, mpsc::UnboundedReceiver<Outbound>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            closed: crate::CancellationToken::new(),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            events: broadcast::channel(4).0,
            popup_events: broadcast::channel(256).0,
            downloads: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            attachments: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
        });
        (CdpConnection { inner }, rx)
    }
    #[tokio::test]
    async fn abandoned_attachment_late_reply_detaches_and_cap_releases_on_disconnect() {
        let (connection, mut outgoing) = scoped_fixture();
        // An expired weak owner models a cancelled attach before its native reply.
        let future = connection.call_owned(
            None,
            "Target.attachToTarget",
            Value::Null,
            Duration::from_millis(5),
            Some(Weak::new()),
        );
        assert!(matches!(future.await, Err(E2eError::Timeout(_, _))));
        assert!(connection.inner.pending.lock().unwrap().is_empty());
        assert_eq!(connection.inner.attachments.lock().unwrap().len(), 1);
        let Outbound::Text(command) = outgoing.recv().await.unwrap() else {
            panic!()
        };
        let id: Value = serde_json::from_str(&command).unwrap();
        handle_frame(
            &connection.inner,
            &serde_json::json!({"id":id["id"],"result":{"sessionId":"late"}}).to_string(),
        );
        let Outbound::Text(command) = outgoing.recv().await.unwrap() else {
            panic!()
        };
        let command: Value = serde_json::from_str(&command).unwrap();
        assert_eq!(command["method"], "Target.detachFromTarget");
        assert_eq!(command["params"]["sessionId"], "late");
        assert!(connection.inner.attachments.lock().unwrap().is_empty());
        for id in 100..356 {
            connection
                .inner
                .attachments
                .lock()
                .unwrap()
                .insert(id, Weak::new());
        }
        assert!(matches!(
            connection
                .call_owned(
                    None,
                    "Target.attachToTarget",
                    Value::Null,
                    Duration::ZERO,
                    Some(Weak::new())
                )
                .await,
            Err(E2eError::Config(_))
        ));
        assert!(outgoing.try_recv().is_err());
        fail_all(&connection.inner, "test disconnect");
        assert!(connection.inner.attachments.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn popup_channel_ignores_unrelated_traffic_and_reports_lifecycle_overflow() {
        let (connection, _) = scoped_fixture();
        let mut popups = connection.subscribe_popups();
        let mut general = connection.subscribe();
        for _ in 0..5000 {
            handle_frame(
                &connection.inner,
                r#"{"method":"Runtime.consoleAPICalled","sessionId":"main","params":{}}"#,
            );
        }
        assert!(matches!(
            general.recv().await,
            Err(broadcast::error::RecvError::Lagged(_))
        ));
        assert!(matches!(
            popups.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        handle_frame(
            &connection.inner,
            r#"{"method":"Target.attachedToTarget","params":{"sessionId":"popup"}}"#,
        );
        assert_eq!(
            popups.recv().await.unwrap().method,
            "Target.attachedToTarget"
        );
        for _ in 0..300 {
            handle_frame(
                &connection.inner,
                r#"{"method":"Target.targetDestroyed","params":{"targetId":"popup"}}"#,
            );
        }
        assert!(matches!(
            popups.recv().await,
            Err(broadcast::error::RecvError::Lagged(_))
        ));
    }

    #[test]
    fn event_frame_parses() {
        let text = r#"{"method":"Page.loadEventFired","sessionId":"ABC","params":{"timestamp":1}}"#;
        let frame: Value = serde_json::from_str(text).unwrap();
        assert_eq!(frame["method"], "Page.loadEventFired");
        assert_eq!(frame["sessionId"], "ABC");
    }

    #[test]
    fn download_tracker_folds_events() {
        let (events, _) = broadcast::channel::<CdpEvent>(4);
        let (tx, _) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            closed: crate::CancellationToken::new(),
            tx,
            popup_captures: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            events,
            popup_events: broadcast::channel(256).0,
            downloads: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            attachments: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
        });
        let begin = serde_json::json!({
            "method": "Browser.downloadWillBegin",
            "params": {
                "guid": "g1",
                "url": "http://x.test/f.bin",
                "suggestedFilename": "f.bin",
            },
        });
        track_download(&inner, "Browser.downloadWillBegin", &begin);
        // Duplicate begins do not duplicate records.
        track_download(&inner, "Browser.downloadWillBegin", &begin);
        let progress = serde_json::json!({
            "method": "Browser.downloadProgress",
            "params": { "guid": "g1", "state": "completed" },
        });
        track_download(&inner, "Browser.downloadProgress", &progress);
        // Progress for unknown GUIDs is ignored.
        let stray = serde_json::json!({
            "method": "Browser.downloadProgress",
            "params": { "guid": "nope", "state": "completed" },
        });
        track_download(&inner, "Browser.downloadProgress", &stray);
        let records = inner.downloads.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].url, "http://x.test/f.bin");
        assert_eq!(records[0].filename, "f.bin");
        assert_eq!(records[0].state, "completed");
    }
}
