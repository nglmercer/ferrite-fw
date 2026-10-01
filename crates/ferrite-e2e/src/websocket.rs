//! Bounded Chromium socket diagnostics. No socket or native owner is retained.
use crate::{E2eError, E2eResult, OperationOptions, Page, WebSocketDirection, WebSocketEvent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use tokio::sync::broadcast;

const MAX_SOCKETS: usize = 256;
const MAX_EVENTS: usize = 1024;
const MAX_HISTORY_BYTES: usize = 1024 * 1024;
const MAX_PAYLOAD: usize = 16 * 1024;
const MAX_TEXT: usize = 4096;

/// Observing means creation was observed, not that a handshake succeeded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum WebSocketState {
    Observing,
    Closed,
    Error(String),
    Unavailable(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSocketSnapshot {
    pub socket_id: String,
    pub url: String,
    pub state: WebSocketState,
    pub last_error: Option<String>,
    pub history_truncated: bool,
    pub url_truncated: bool,
}
impl WebSocketSnapshot {
    /// Only an observed native close is true; unavailable observation is unknown.
    pub fn is_closed(&self) -> Option<bool> {
        match self.state {
            WebSocketState::Closed => Some(true),
            WebSocketState::Unavailable(_) => None,
            _ => Some(false),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSocketDiagnostics {
    pub sockets: Vec<WebSocketSnapshot>,
    pub events: Vec<WebSocketEvent>,
    pub sockets_evicted: u64,
    pub events_dropped: u64,
    pub observation_lost: Option<String>,
}
pub(crate) struct SocketLog {
    sockets: VecDeque<WebSocketSnapshot>,
    events: VecDeque<WebSocketEvent>,
    bytes: usize,
    sequence: u64,
    sockets_evicted: u64,
    events_dropped: u64,
    lost: Option<String>,
    notices: broadcast::Sender<Option<WebSocketEvent>>,
}
impl Default for SocketLog {
    fn default() -> Self {
        Self {
            sockets: VecDeque::new(),
            events: VecDeque::new(),
            bytes: 0,
            sequence: 0,
            sockets_evicted: 0,
            events_dropped: 0,
            lost: None,
            notices: broadcast::channel(256).0,
        }
    }
}
fn bounded(text: &str, max: usize) -> (String, bool) {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].into(), text.len() > end)
}
fn event_bytes(event: &WebSocketEvent) -> usize {
    event.payload.len()
        + event.url.len()
        + event.socket_id.len()
        + event.error.as_ref().map_or(0, String::len)
}
impl SocketLog {
    pub(crate) fn unavailable(&mut self, reason: &str) {
        if self.lost.is_some() {
            return;
        }
        let reason = bounded(reason, MAX_TEXT).0;
        self.lost = Some(reason.clone());
        for socket in &mut self.sockets {
            if !matches!(socket.state, WebSocketState::Closed) {
                socket.state = WebSocketState::Unavailable(reason.clone());
            }
        }
        let _ = self.notices.send(None);
    }
    fn remove_event(&mut self) {
        if let Some(event) = self.events.pop_front() {
            self.bytes -= event_bytes(&event);
            self.events_dropped += 1;
            if let Some(socket) = self
                .sockets
                .iter_mut()
                .find(|s| s.socket_id == event.socket_id)
            {
                socket.history_truncated = true;
            }
        }
    }
    pub(crate) fn observe(
        &mut self,
        params: &Value,
        direction: WebSocketDirection,
    ) -> Option<WebSocketEvent> {
        if self.lost.is_some() {
            return None;
        }
        let id = params["requestId"].as_str().unwrap_or_default();
        if id.is_empty() || id.len() > 1024 {
            self.unavailable("missing or oversized native socket identity");
            return None;
        }
        if direction == WebSocketDirection::Created {
            if self.sockets.iter().any(|s| s.socket_id == id) {
                self.unavailable("native socket identity reused");
                return None;
            }
            if self.sockets.len() == MAX_SOCKETS {
                let removed = self.sockets.pop_front().unwrap();
                self.sockets_evicted += 1;
                self.events.retain(|event| {
                    if event.socket_id == removed.socket_id {
                        self.bytes -= event_bytes(event);
                        self.events_dropped += 1;
                        false
                    } else {
                        true
                    }
                });
                // Wake waits on an evicted socket even if the new creation doesn't match.
                let _ = self.notices.send(None);
            }
            let (url, url_truncated) =
                bounded(params["url"].as_str().unwrap_or_default(), MAX_TEXT);
            self.sockets.push_back(WebSocketSnapshot {
                socket_id: id.into(),
                url,
                state: WebSocketState::Observing,
                last_error: None,
                history_truncated: false,
                url_truncated,
            });
        }
        let Some(socket) = self.sockets.iter_mut().find(|s| s.socket_id == id) else {
            // A retired identity cannot be revived from a late frame/close.
            self.events_dropped += 1;
            return None;
        };
        let (payload, payload_truncated) = bounded(
            params["response"]["payloadData"]
                .as_str()
                .unwrap_or_default(),
            MAX_PAYLOAD,
        );
        let (error, error_truncated) = if direction == WebSocketDirection::Error {
            let (text, truncated) = bounded(
                params["errorMessage"]
                    .as_str()
                    .unwrap_or("native socket error"),
                MAX_TEXT,
            );
            (Some(text), truncated)
        } else {
            (None, false)
        };
        if let Some(error) = &error {
            socket.last_error = Some(error.clone());
            socket.state = WebSocketState::Error(error.clone());
        } else if direction == WebSocketDirection::Closed {
            socket.state = WebSocketState::Closed;
        }
        socket.history_truncated |= payload_truncated || error_truncated;
        self.sequence += 1;
        let event = WebSocketEvent {
            socket_id: id.into(),
            url: socket.url.clone(),
            direction,
            payload,
            opcode: params["response"]["opcode"].as_u64(),
            payload_truncated,
            error,
            sequence: self.sequence,
        };
        while self.events.len() >= MAX_EVENTS
            || self.bytes + event_bytes(&event) > MAX_HISTORY_BYTES
        {
            self.remove_event();
        }
        self.bytes += event_bytes(&event);
        self.events.push_back(event.clone());
        let _ = self.notices.send(Some(event.clone()));
        Some(event)
    }
    fn diagnostics(&self) -> WebSocketDiagnostics {
        WebSocketDiagnostics {
            sockets: self.sockets.iter().cloned().collect(),
            events: self.events.iter().cloned().collect(),
            sockets_evicted: self.sockets_evicted,
            events_dropped: self.events_dropped,
            observation_lost: self.lost.clone(),
        }
    }
    fn socket(&self, id: &str) -> E2eResult<&WebSocketSnapshot> {
        self.sockets
            .iter()
            .find(|s| s.socket_id == id)
            .ok_or_else(|| socket_error(format!("socket {id:?} was not observed or was evicted")))
    }
    fn check_lost(&self) -> E2eResult<()> {
        match &self.lost {
            Some(reason) => Err(socket_error(format!(
                "socket observation unavailable: {reason}"
            ))),
            None => Ok(()),
        }
    }
}
impl WebSocketEvent {
    /// Decode a complete native binary/control frame, or encode complete text as UTF-8.
    pub fn payload_bytes(&self) -> E2eResult<Vec<u8>> {
        if self.payload_truncated {
            return Err(socket_error("socket payload truncated".into()));
        }
        match self.opcode {
            Some(1) => Ok(self.payload.as_bytes().to_vec()),
            Some(_) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&self.payload)
                    .map_err(|error| socket_error(format!("invalid native socket base64: {error}")))
            }
            None => Err(E2eError::Config("observation has no frame opcode".into())),
        }
    }
}
fn socket_error(message: String) -> E2eError {
    E2eError::Network {
        url: "websocket observation".into(),
        message,
    }
}
fn receive_error(error: broadcast::error::RecvError) -> E2eError {
    socket_error(format!("socket event observation lost: {error}"))
}
impl Page {
    fn require_socket_capability(&self) -> E2eResult<()> {
        if matches!(self.driver, crate::driver::Driver::Bidi(_)) {
            return Err(E2eError::Config("websocket diagnostics are not supported on Firefox (BiDi has no socket-frame events)".into()));
        }
        Ok(())
    }
    /// Owned bounded history, without retaining this Page. Historical native closes survive disposal.
    pub fn websocket_diagnostics(&self) -> E2eResult<WebSocketDiagnostics> {
        self.require_socket_capability()?;
        let mut log = self
            .sink
            .socket_log
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if self.driver.is_disconnected() {
            log.unavailable("transport disconnected");
        } else if self.is_closed() {
            log.unavailable("page disposed");
        }
        Ok(log.diagnostics())
    }
    pub fn websocket_snapshot(&self, socket_id: &str) -> E2eResult<Option<WebSocketSnapshot>> {
        Ok(self
            .websocket_diagnostics()?
            .sockets
            .into_iter()
            .find(|s| s.socket_id == socket_id))
    }
    /// Wait for future creation by full URL; call/poll before creating the socket.
    pub async fn wait_for_websocket(
        &self,
        matcher: &crate::UrlMatcher,
        options: OperationOptions,
    ) -> E2eResult<WebSocketEvent> {
        self.require_socket_capability()?;
        let page = self.operation_page(&options);
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        page.run_operation(
            crate::operation::Deadline::new(timeout).run("socket creation", async {
                let mut receiver = {
                    let log = page
                        .sink
                        .socket_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    log.check_lost()?;
                    log.notices.subscribe()
                };
                loop {
                    let event = receiver.recv().await.map_err(receive_error)?;
                    page.sink
                        .socket_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .check_lost()?;
                    if let Some(event) = event {
                        if event.direction == WebSocketDirection::Created {
                            let log = page
                                .sink
                                .socket_log
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            if log.socket(&event.socket_id)?.url_truncated {
                                return Err(socket_error(
                                    "native socket URL truncated; full-URL matching unavailable"
                                        .into(),
                                ));
                            }
                            if matcher.matches(&event.url) {
                                return Ok(event);
                            }
                        }
                    }
                }
            }),
        )
        .await
    }
    /// Future frame/error events for one native identity. A retained native Closed
    /// observation can satisfy Closed immediately. Other waits fail on terminal state.
    pub async fn wait_for_websocket_event(
        &self,
        socket_id: &str,
        direction: WebSocketDirection,
        options: OperationOptions,
    ) -> E2eResult<WebSocketEvent> {
        self.require_socket_capability()?;
        let page = self.operation_page(&options);
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        page.run_operation(
            crate::operation::Deadline::new(timeout).run("socket event", async {
                let mut receiver = {
                    let log = page
                        .sink
                        .socket_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    let socket = log.socket(socket_id)?;
                    if matches!(socket.state, WebSocketState::Closed)
                        && direction == WebSocketDirection::Closed
                    {
                        return log
                            .events
                            .iter()
                            .rev()
                            .find(|e| e.socket_id == socket_id && e.direction == direction)
                            .cloned()
                            .ok_or_else(|| {
                                socket_error("native close event was evicted from history".into())
                            });
                    }
                    log.check_lost()?;
                    terminal_error(socket)?;
                    log.notices.subscribe()
                };
                loop {
                    let event = receiver.recv().await.map_err(receive_error)?;
                    page.sink
                        .socket_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .check_lost()?;
                    if let Some(event) = &event {
                        if event.socket_id == socket_id && event.direction == direction {
                            return Ok(event.clone());
                        }
                    }
                    let log = page
                        .sink
                        .socket_log
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    log.check_lost()?;
                    terminal_error(log.socket(socket_id)?)?;
                }
            }),
        )
        .await
    }
}
fn terminal_error(socket: &WebSocketSnapshot) -> E2eResult<()> {
    match &socket.state {
        WebSocketState::Observing => Ok(()),
        state => Err(socket_error(format!(
            "socket {:?} ended before requested event: {state:?}",
            socket.socket_id
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create(log: &mut SocketLog, id: &str) -> WebSocketEvent {
        log.observe(
            &json!({"requestId":id,"url":"ws://same/socket"}),
            WebSocketDirection::Created,
        )
        .unwrap()
    }
    fn frame(log: &mut SocketLog, id: &str, payload: &str, opcode: u64) -> WebSocketEvent {
        log.observe(
            &json!({"requestId":id,"response":{"opcode":opcode,"payloadData":payload}}),
            WebSocketDirection::Received,
        )
        .unwrap()
    }
    #[test]
    fn native_identity_opcode_errors_and_legacy_serde_are_distinct() {
        let mut log = SocketLog::default();
        create(&mut log, "one");
        create(&mut log, "two");
        let text = frame(&mut log, "one", "雪<&", 1);
        let binary = frame(&mut log, "two", "AP8=", 2);
        assert_ne!(text.socket_id, binary.socket_id);
        assert_eq!(text.url, binary.url);
        assert_eq!(text.payload_bytes().unwrap(), "雪<&".as_bytes());
        assert_eq!(binary.payload_bytes().unwrap(), vec![0, 255]);
        assert!(frame(&mut log, "two", "!bad!", 2).payload_bytes().is_err());
        let error = log
            .observe(
                &json!({"requestId":"one","errorMessage":"handshake failed"}),
                WebSocketDirection::Error,
            )
            .unwrap();
        assert_eq!(error.error.as_deref(), Some("handshake failed"));
        assert!(matches!(
            log.socket("one").unwrap().state,
            WebSocketState::Error(_)
        ));
        assert_eq!(log.socket("one").unwrap().is_closed(), Some(false));
        log.observe(&json!({"requestId":"one"}), WebSocketDirection::Closed);
        assert_eq!(log.socket("one").unwrap().is_closed(), Some(true));
        assert_eq!(
            log.socket("one").unwrap().last_error.as_deref(),
            Some("handshake failed")
        );
        let legacy: WebSocketEvent =
            serde_json::from_str(r#"{"url":"ws://old","direction":"Received","payload":"legacy"}"#)
                .unwrap();
        assert!(legacy.socket_id.is_empty());
        assert_eq!(legacy.sequence, 0);
        assert!(legacy.payload_bytes().is_err());
        let copied: WebSocketDiagnostics =
            serde_json::from_str(&serde_json::to_string(&log.diagnostics()).unwrap()).unwrap();
        assert_eq!(copied, log.diagnostics());
    }
    #[test]
    fn bounded_payload_history_and_socket_eviction_never_revive_retired_identity() {
        let mut log = SocketLog::default();
        create(&mut log, "first");
        let oversized = "雪".repeat(MAX_PAYLOAD);
        let event = frame(&mut log, "first", &oversized, 1);
        assert!(event.payload_truncated);
        assert!(event.payload.len() <= MAX_PAYLOAD);
        assert!(event.payload.is_char_boundary(event.payload.len()));
        assert!(event.payload_bytes().is_err());
        for _ in 0..1200 {
            frame(&mut log, "first", &"a".repeat(MAX_PAYLOAD), 1);
        }
        assert!(log.events.len() <= MAX_EVENTS);
        assert!(log.bytes <= MAX_HISTORY_BYTES);
        assert!(log.events_dropped > 0);
        assert!(log.socket("first").unwrap().history_truncated);
        for index in 0..MAX_SOCKETS {
            create(&mut log, &format!("socket-{index}"));
        }
        assert_eq!(log.sockets.len(), MAX_SOCKETS);
        assert_eq!(log.sockets_evicted, 1);
        assert!(log.socket("first").is_err());
        assert!(log
            .observe(&json!({"requestId":"first"}), WebSocketDirection::Closed)
            .is_none());
        assert!(log.socket("first").is_err());
        assert!(log.events.iter().all(|e| e.socket_id != "first"));
    }
    #[test]
    fn count_budget_and_lost_observation_preserve_only_real_closes() {
        let mut log = SocketLog::default();
        create(&mut log, "closed");
        create(&mut log, "pending");
        for _ in 0..(MAX_EVENTS + 20) {
            frame(&mut log, "pending", "x", 1);
        }
        assert_eq!(log.events.len(), MAX_EVENTS);
        log.observe(&json!({"requestId":"closed"}), WebSocketDirection::Closed);
        log.unavailable("transport disconnected");
        assert_eq!(log.socket("closed").unwrap().is_closed(), Some(true));
        assert_eq!(log.socket("pending").unwrap().is_closed(), None);
        assert!(log
            .observe(&json!({"requestId":"pending"}), WebSocketDirection::Closed)
            .is_none());
        assert!(log.check_lost().is_err());
        log.unavailable("another reason");
        assert_eq!(log.lost.as_deref(), Some("transport disconnected"));
    }
    #[tokio::test]
    async fn lag_and_lost_notifications_settle_without_fabricated_close() {
        let mut log = SocketLog::default();
        create(&mut log, "one");
        let mut receiver = log.notices.subscribe();
        for _ in 0..300 {
            frame(&mut log, "one", "x", 1);
        }
        let error = receiver.recv().await.unwrap_err();
        assert!(matches!(error, broadcast::error::RecvError::Lagged(_)));
        assert!(receive_error(error)
            .to_string()
            .contains("observation lost"));
        let mut receiver = log.notices.subscribe();
        log.unavailable("native channel lagged");
        assert!(receiver.recv().await.unwrap().is_none());
        assert_eq!(log.socket("one").unwrap().is_closed(), None);
    }
    #[test]
    fn malformed_or_reused_identity_and_large_metadata_fail_explicitly() {
        let mut malformed = SocketLog::default();
        assert!(malformed
            .observe(&json!({}), WebSocketDirection::Created)
            .is_none());
        assert!(malformed.check_lost().is_err());
        let mut log = SocketLog::default();
        let event = log
            .observe(
                &json!({"requestId":"one","url":"雪".repeat(5000)}),
                WebSocketDirection::Created,
            )
            .unwrap();
        assert!(event.url.len() <= MAX_TEXT);
        assert!(log.socket("one").unwrap().url_truncated);
        log.observe(
            &json!({"requestId":"one","errorMessage":"雪".repeat(5000)}),
            WebSocketDirection::Error,
        );
        assert!(
            log.socket("one")
                .unwrap()
                .last_error
                .as_ref()
                .unwrap()
                .len()
                <= MAX_TEXT
        );
        assert!(log
            .observe(&json!({"requestId":"one"}), WebSocketDirection::Created)
            .is_none());
        assert_eq!(log.socket("one").unwrap().is_closed(), None);
        assert!(log.socket("one").unwrap().history_truncated);
    }

    #[test]
    fn shared_native_and_popup_observation_loss_marks_sockets_unavailable() {
        let sink = crate::driver::ConsoleSink::new();
        create(&mut sink.socket_log.lock().unwrap(), "pending");
        sink.close_network("popup startup capture exceeded its native event budget");
        let log = sink.socket_log.lock().unwrap();
        assert_eq!(log.socket("pending").unwrap().is_closed(), None);
        assert!(log
            .check_lost()
            .unwrap_err()
            .to_string()
            .contains("native event budget"));
    }
}
