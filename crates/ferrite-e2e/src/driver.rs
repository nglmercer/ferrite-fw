//! Engine drivers: CDP (Chromium) and WebDriver BiDi (Firefox).
//!
//! [`Driver`] is the engine-agnostic surface [`Page`](crate::Page) programs
//! against. Engine differences (sessions vs contexts, RemoteValue decoding,
//! interception mechanisms) stay inside the two drivers.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::bidi::{remote_to_json, BidiConnection, BidiEvent};
use crate::cdp::{CdpConnection, CdpEvent};
use crate::error::{E2eError, E2eResult};
use crate::jshandle::JSHandle;
use crate::page::{
    ConsoleMessage, Cookie, DialogDecision, DialogHandler, DialogInfo, ElementRect, FrameInfo,
    LoadState, NetworkRequest, PageEvent, RecordedRequest, RouteAction, RouteInfo, TraceEntry,
    WebSocketDirection,
};
use crate::video::{assemble_webm, SpooledFrame, VideoFrame, VideoOptions};

type ContextEventForwarding = (
    tokio::sync::broadcast::WeakSender<crate::ContextEvent>,
    String,
    std::sync::Weak<Mutex<Vec<ConsoleMessage>>>,
);
type IdleRequestScope = (Option<String>, Option<String>);
struct NetworkListenerGuard(ConsoleSink);
#[derive(Default)]
struct BodyCaptureControl {
    generation: u64,
    active: bool,
}
struct BodyCaptureGuard {
    sink: ConsoleSink,
    generation: u64,
}
impl Drop for BodyCaptureGuard {
    fn drop(&mut self) {
        self.sink.end_body_capture(
            Some(self.generation),
            "body capture ended (stop, lag, disposal or disconnect)",
        );
    }
}

/// Native listeners must also stop when setup is canceled before a Page exists.
struct PageInitialization {
    lifecycle: crate::CancellationToken,
    complete: bool,
}
impl Drop for PageInitialization {
    fn drop(&mut self) {
        if !self.complete {
            self.lifecycle
                .cancel_with_reason("page initialization interrupted");
        }
    }
}
impl Drop for NetworkListenerGuard {
    fn drop(&mut self) {
        self.0
            .mark_native_observation_lost("native event listener ended");
        // Popup observation belongs to transport ingress and can outlive a
        // failed driver initialization. Ingress closes its independent state.
        if self.0.popup_capture.is_some() {
            return;
        }
        self.0.close_network("native event listener ended");
        self.0
            .frame_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

struct MainWorldListenerGuard(Arc<Mutex<HashMap<String, i64>>>);
impl Drop for MainWorldListenerGuard {
    fn drop(&mut self) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// Live request-start/response-header observation, separate from HAR/body capture.
#[derive(Clone)]
pub(crate) struct NetworkObservation {
    pub request: RecordedRequest,
    pub response: bool,
}

/// Sinks shared between a page and its driver's background listeners.
#[derive(Clone)]
pub struct ConsoleSink {
    /// Console messages and page errors.
    pub console: Arc<Mutex<Vec<ConsoleMessage>>>,
    /// Action/navigation/console trace.
    pub trace: Arc<Mutex<Vec<TraceEntry>>>,
    /// In-flight network requests (network-idle waits).
    pub inflight: Arc<AtomicUsize>,
    network_activity: Arc<AtomicU64>,
    /// Dialogs observed while auto-handling (oldest first).
    pub dialogs: Arc<Mutex<Vec<DialogInfo>>>,
    /// Recorded network requests (oldest first, capped).
    requests: Arc<Mutex<VecDeque<RecordedRequest>>>,
    active_requests: Arc<Mutex<HashMap<String, NetworkRequest>>>,
    idle_requests: Arc<Mutex<HashMap<String, IdleRequestScope>>>,
    observed_network: Arc<Mutex<HashMap<String, RecordedRequest>>>,
    pub(crate) network_log: Arc<Mutex<crate::network::NetworkLog>>,
    body_capture_control: Arc<Mutex<BodyCaptureControl>>,
    network_events: tokio::sync::broadcast::Sender<NetworkObservation>,
    /// WebSocket request id to URL (resolves frame events to sockets).
    pub(crate) socket_log: Arc<Mutex<crate::websocket::SocketLog>>,
    /// Page event broadcast (console, dialogs, network, downloads, popups).
    events: tokio::sync::broadcast::Sender<PageEvent>,
    popup_loss: Arc<tokio::sync::watch::Sender<Option<String>>>,
    native_observation: crate::CancellationToken,
    context_source_loss: Arc<Mutex<std::sync::Weak<tokio::sync::watch::Sender<Option<String>>>>>,
    pub(crate) download_dir: Arc<Mutex<Option<PathBuf>>>,
    downloads_emitted: Arc<Mutex<HashMap<PathBuf, (u64, SystemTime)>>>,
    context_events: Arc<Mutex<Option<ContextEventForwarding>>>,
    page_id: Arc<Mutex<Option<String>>>,
    frame_events: Arc<Mutex<crate::lifecycle_events::FrameEvents>>,
    native_closed: Arc<AtomicBool>,
    pub(crate) popup_capture: Option<Arc<crate::popup_capture::PopupCapture>>,
    pub(crate) popup_diagnostics: Option<std::sync::Weak<Mutex<crate::popup_capture::PopupRecord>>>,
}

/// Maximum recorded requests per page (oldest dropped first).
const MAX_RECORDED_REQUESTS: usize = 4096;

/// Maximum captured response body per request (1 MiB; larger bodies are
/// dropped with `body_truncated` set).
pub(crate) const MAX_RESPONSE_BODY: usize = 1024 * 1024;

/// Buffered page events per page (lagged readers skip ahead).
const MAX_EVENT_BUFFER: usize = 256;

impl ConsoleSink {
    pub(crate) fn bind_context_source_loss(
        &self,
        source: &Arc<tokio::sync::watch::Sender<Option<String>>>,
    ) {
        *self
            .context_source_loss
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Arc::downgrade(source);
    }
    pub(crate) fn mark_native_observation_lost(&self, reason: &str) {
        self.native_observation.cancel_with_reason(reason);
        if !self.native_closed() {
            let source = self
                .context_source_loss
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .upgrade();
            if let Some(source) = source {
                source.send_replace(Some(format!(
                    "context page event source unavailable: {reason}"
                )));
            }
        }
    }
    pub(crate) fn native_observation_loss(&self) -> crate::CancellationToken {
        self.native_observation.clone()
    }
    pub(crate) fn popup_losses(&self) -> tokio::sync::watch::Receiver<Option<String>> {
        self.popup_loss.subscribe()
    }
    pub(crate) fn popup_observation_lost(&self, reason: &str) {
        self.popup_loss.send_replace(Some(reason.into()));
    }
    /// Empty sinks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            console: Arc::new(Mutex::new(Vec::new())),
            trace: Arc::new(Mutex::new(Vec::new())),
            inflight: Arc::new(AtomicUsize::new(0)),
            network_activity: Arc::new(AtomicU64::new(0)),
            dialogs: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(VecDeque::new())),
            socket_log: Arc::new(Mutex::new(crate::websocket::SocketLog::default())),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
            idle_requests: Arc::new(Mutex::new(HashMap::new())),
            observed_network: Arc::new(Mutex::new(HashMap::new())),
            network_log: Arc::new(Mutex::new(crate::network::NetworkLog::default())),
            body_capture_control: Arc::new(Mutex::new(BodyCaptureControl::default())),
            network_events: tokio::sync::broadcast::channel(MAX_EVENT_BUFFER).0,
            events: tokio::sync::broadcast::channel(MAX_EVENT_BUFFER).0,
            popup_loss: Arc::new(tokio::sync::watch::channel(None).0),
            native_observation: crate::CancellationToken::new(),
            context_source_loss: Arc::new(Mutex::new(std::sync::Weak::new())),
            context_events: Arc::new(Mutex::new(None)),
            page_id: Arc::new(Mutex::new(None)),
            frame_events: Arc::new(Mutex::new(crate::lifecycle_events::FrameEvents::default())),
            native_closed: Arc::new(AtomicBool::new(false)),
            popup_capture: None,
            popup_diagnostics: None,
            download_dir: Arc::new(Mutex::new(None)),
            downloads_emitted: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn start_network_request(
        &self,
        request: NetworkRequest,
        frame: Option<String>,
        document: Option<String>,
    ) {
        // CDP reuses identifiers for redirects. The old hop has finished.
        self.finish_network_request_observation(&request.request_id, None, false);
        self.active_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(request.request_id.clone(), request.clone());
        self.idle_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(request.request_id.clone(), (frame, document));
        self.inflight.fetch_add(1, Ordering::SeqCst);
        self.network_activity.fetch_add(1, Ordering::SeqCst);
        self.emit(PageEvent::Request {
            request_id: request.request_id,
            method: request.method,
            url: request.url,
        });
    }

    pub(crate) fn subscribe_network(&self) -> tokio::sync::broadcast::Receiver<NetworkObservation> {
        self.network_events.subscribe()
    }
    fn observe_request(&self, request: RecordedRequest, details: crate::network::RequestDetails) {
        let state = self
            .network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .start(
                request.clone(),
                details,
                self.page_id
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone(),
            );
        if let Some(record) = self
            .popup_diagnostics
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
        {
            record
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .request(state);
        }
        if let Some(id) = &request.request_id {
            self.observed_network
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(id.clone(), request.clone());
        }
        let _ = self.network_events.send(NetworkObservation {
            request,
            response: false,
        });
    }
    fn observe_response(
        &self,
        id: &str,
        response: &Value,
        headers: Vec<(String, String)>,
        extra: Option<bool>,
    ) {
        let mut records = self
            .observed_network
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(request) = records.get_mut(id) {
            if let Some(url) = response["url"].as_str() {
                request.url = url.into();
            }
            request.status = response["status"]
                .as_u64()
                .unwrap_or(0)
                .min(u64::from(u16::MAX)) as u16;
            request.status_text = response["statusText"].as_str().unwrap_or_default().into();
            request.mime_type = response["mimeType"].as_str().unwrap_or_default().into();
            request.response_headers = headers;
            request.duration_ms = request
                .started_ms
                .map(|start| now_ms().saturating_sub(start));
            self.network_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .response(id, request.clone(), extra);
            let _ = self.network_events.send(NetworkObservation {
                request: request.clone(),
                response: true,
            });
        }
    }
    fn finish_network_request(&self, id: &str, failure: Option<(String, Option<bool>)>) {
        self.finish_network_request_observation(id, failure, true);
    }
    fn finish_network_request_observation(
        &self,
        id: &str,
        failure: Option<(String, Option<bool>)>,
        native_terminal: bool,
    ) {
        if native_terminal {
            self.network_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .finish(
                    id,
                    failure
                        .as_ref()
                        .map(|(error_text, cancelled)| crate::RequestFailure {
                            error_text: error_text.clone(),
                            cancelled: *cancelled,
                        }),
                );
        }
        self.observed_network
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        let request = self
            .active_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        if let Some(request) = request {
            if self
                .idle_requests
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(id)
                .is_some()
            {
                self.inflight.fetch_sub(1, Ordering::SeqCst);
            }
            self.network_activity.fetch_add(1, Ordering::SeqCst);
            self.emit(match failure {
                Some((error_text, cancelled)) => PageEvent::RequestFailed {
                    request,
                    error_text,
                    cancelled,
                },
                None => PageEvent::RequestFinished(request),
            });
        }
    }

    pub(crate) fn close_network(&self, reason: &str) {
        self.end_body_capture(None, reason);
        self.socket_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unavailable(reason);
        self.network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .close(reason);
        self.active_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.observed_network
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.idle_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.inflight.store(0, Ordering::SeqCst);
    }

    fn routed_headers(
        &self,
        pause_id: &str,
        id: &str,
        url: &str,
        action: Option<&RouteAction>,
    ) -> Option<crate::network::RouteHeadersGuard> {
        let RouteAction::Fulfill {
            headers,
            content_type,
            ..
        } = action?
        else {
            return None;
        };
        let mut headers = headers.clone();
        if !content_type.is_empty()
            && !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        {
            headers.push(("Content-Type".into(), content_type.clone()));
        }
        self.network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .routed_headers(pause_id, id, url, headers)
    }

    pub(crate) fn network_activity(&self) -> (usize, u64) {
        (
            self.inflight.load(Ordering::SeqCst),
            self.network_activity.load(Ordering::SeqCst),
        )
    }

    /// A replaced document's suspended/canceled resources must not prevent the
    /// new document from becoming idle. Keep their native lifecycle observations
    /// pending: this is idle accounting, not a fabricated terminal event.
    fn commit_network_document(&self, frame: &str, document: &str, main: bool) {
        let mut idle = self.idle_requests.lock().unwrap_or_else(|e| e.into_inner());
        let before = idle.len();
        idle.retain(|_, (owner, loader)| {
            if main {
                owner.as_deref() == Some(frame) && loader.as_deref() == Some(document)
            } else {
                owner.as_deref() != Some(frame) || loader.as_deref() == Some(document)
            }
        });
        let removed = before - idle.len();
        if removed != 0 {
            self.inflight.fetch_sub(removed, Ordering::SeqCst);
            self.network_activity.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Subscribe to page events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PageEvent> {
        self.events.subscribe()
    }

    fn observe_lifecycle(&self, method: &str, params: &Value, cdp: bool) {
        let Some(page) = self
            .page_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        else {
            return;
        };
        let events = {
            let mut frames = self.frame_events.lock().unwrap_or_else(|e| e.into_inner());
            if cdp {
                frames.cdp(&page, method, params)
            } else {
                frames.bidi(&page, method, params)
            }
        };
        for event in events {
            self.emit(event);
        }
    }

    pub(crate) fn contains_frame(&self, id: &str) -> bool {
        self.frame_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(id)
    }

    /// Emit a page event (dropped when nobody listens).
    pub(crate) fn emit(&self, event: PageEvent) {
        if matches!(event, PageEvent::Closed) {
            if self.native_closed.swap(true, Ordering::AcqRel) {
                return;
            }
            self.socket_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .unavailable("page closed without observed socket closure");
        }
        if let PageEvent::Download(path) = &event {
            if let Ok(meta) = path.metadata() {
                let stamp = (meta.len(), meta.modified().unwrap_or(UNIX_EPOCH));
                if self
                    .downloads_emitted
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(path.clone(), stamp)
                    == Some(stamp)
                {
                    return;
                }
            }
        }
        if let Some((weak, page_id, _)) = &*self
            .context_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
        {
            if let Some(sender) = weak.upgrade() {
                let _ = sender.send(crate::ContextEvent::PageEvent {
                    page_id: page_id.clone(),
                    event: event.clone(),
                });
            }
        }
        let _ = self.events.send(event);
    }

    pub(crate) fn forward_context(
        &self,
        events: &tokio::sync::broadcast::Sender<crate::ContextEvent>,
        page_id: &str,
        console: &Arc<Mutex<Vec<ConsoleMessage>>>,
    ) {
        *self
            .context_events
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some((
            events.downgrade(),
            page_id.to_owned(),
            Arc::downgrade(console),
        ));
    }
    pub(crate) fn native_closed(&self) -> bool {
        self.native_closed.load(Ordering::Acquire)
    }
    pub(crate) fn seed_popup_closed(&self) {
        self.native_closed.store(true, Ordering::Release);
    }
    pub(crate) fn seed_popup(&self, id: &str, cdp: bool) {
        *self.page_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(id.into());
        if !cdp {
            self.frame_events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .seed_bidi(id);
        }
    }
    pub(crate) fn observe_popup_cdp(&self, event: &CdpEvent) {
        handle_cdp_event(event, self);
    }
    pub(crate) fn observe_popup_bidi(&self, event: &BidiEvent) {
        handle_bidi_event(event, self);
    }
    pub(crate) fn stop_popup_observation(&self, reason: &str) {
        self.close_network(reason);
        self.frame_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    fn completed_download(&self, filename: &str, filepath: Option<&str>) {
        let path = filepath.map(PathBuf::from).or_else(|| {
            self.download_dir
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map(|dir| dir.join(filename))
        });
        if let Some(path) = path {
            let sink = self.clone();
            tokio::spawn(async move {
                let deadline = crate::operation::Deadline::new(Duration::from_secs(5));
                loop {
                    if path.is_file() {
                        sink.emit(PageEvent::Download(path));
                        break;
                    }
                    if deadline.expired() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            });
        }
    }
    /// Record a console message (also appended to the trace).
    pub fn push_console(&self, mut message: ConsoleMessage) {
        message.page_id = self
            .page_id
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(record) = self
            .popup_diagnostics
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
        {
            record
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .console(message.clone());
        }
        self.console
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(message.clone());
        if let Some((_, _, context_console)) = &*self
            .context_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
        {
            if let Some(console) = context_console.upgrade() {
                console
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(message.clone());
            }
        }
        self.record_with_console(
            "console",
            format!("{}: {}", message.kind, message.text),
            Some(Box::new(message.clone())),
        );
        self.emit(PageEvent::Console(message));
    }

    /// Record an observed dialog (also appended to the trace).
    pub fn push_dialog(&self, dialog: DialogInfo) {
        if let Ok(mut dialogs) = self.dialogs.lock() {
            dialogs.push(dialog.clone());
        }
        self.record(
            "dialog",
            format!("{}: {}", dialog.dialog_type, dialog.message),
        );
        self.emit(PageEvent::Dialog(dialog));
    }

    /// Record an observed request (drops the oldest past the cap).
    pub fn push_request(&self, request: RecordedRequest) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push_back(request);
            while requests.len() > MAX_RECORDED_REQUESTS {
                requests.pop_front();
            }
        }
    }

    /// Attach a response body to the most recent record with `request_id`
    /// (bodies arrive at `loadingFinished`, after the record is pushed).
    pub fn set_response_body(&self, request_id: &str, body: Option<Vec<u8>>, truncated: bool) {
        if let Ok(mut requests) = self.requests.lock() {
            if let Some(found) = requests
                .iter_mut()
                .rev()
                .find(|record| record.request_id.as_deref() == Some(request_id))
            {
                found.body = body;
                found.body_truncated = truncated;
            }
            let mut bytes: usize = requests
                .iter()
                .map(|record| record.body.as_ref().map_or(0, Vec::len))
                .sum();
            for record in requests.iter_mut() {
                if bytes <= 16 * 1024 * 1024 {
                    break;
                }
                if let Some(body) = record.body.take() {
                    bytes -= body.len();
                }
            }
        }
    }

    fn begin_body_capture(&self) -> u64 {
        let mut control = self
            .body_capture_control
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        control.generation += 1;
        control.active = true;
        self.network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin_body_capture(control.generation);
        control.generation
    }
    pub(crate) fn end_body_capture(&self, generation: Option<u64>, reason: &str) {
        let mut control = self
            .body_capture_control
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if generation.is_some_and(|generation| generation != control.generation) {
            return;
        }
        control.active = false;
        self.network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .end_body_capture(reason);
    }
    fn publish_captured_body(
        &self,
        id: &str,
        generation: u64,
        result: Result<Vec<u8>, crate::BodyCaptureState>,
    ) {
        let control = self
            .body_capture_control
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !control.active || control.generation != generation {
            return;
        }
        match &result {
            Ok(body) => self.set_response_body(id, Some(body.clone()), false),
            Err(crate::BodyCaptureState::Truncated { .. }) => {
                self.set_response_body(id, None, true)
            }
            Err(_) => {}
        }
        self.network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .captured_body(id, generation, result);
    }

    /// Recorded requests (oldest first).
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .map(|requests| requests.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drop recorded requests.
    pub fn clear_requests(&self) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.clear();
        }
    }

    /// Record a trace entry.
    pub fn record(&self, kind: &str, detail: String) {
        self.record_with_console(kind, detail, None);
    }
    fn record_with_console(
        &self,
        kind: &str,
        detail: String,
        console: Option<Box<ConsoleMessage>>,
    ) {
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        if let Ok(mut trace) = self.trace.lock() {
            trace.push(TraceEntry {
                ts_ms,
                kind: kind.to_string(),
                detail,
                console,
            });
        }
    }
}

impl Default for ConsoleSink {
    fn default() -> Self {
        Self::new()
    }
}

/// CDP-backed driver (Chromium): one flattened target session.
#[derive(Clone)]
pub struct CdpDriver {
    main_worlds: Arc<Mutex<HashMap<String, i64>>>,
    cdp: CdpConnection,
    session: String,
    target: String,
    timeout: Arc<Mutex<Duration>>,
    lifecycle: crate::CancellationToken,
    cancellation: crate::CancellationToken,
    context_cancellation: crate::CancellationToken,
    sink: ConsoleSink,
    /// Held modifier bitmask (CDP does not track it across calls).
    modifiers: Arc<Mutex<u8>>,
    /// Whether the left button is held (CDP moves default to buttons=0,
    /// which breaks pointer capture and buttons-gated drag handlers).
    pressed: Arc<Mutex<bool>>,
    /// Owning browser context (`None` = default; scopes permission grants).
    browser_context: Option<String>,
    /// Fetch-domain sharing between routing and auth challenges.
    fetch_auth: Arc<Mutex<FetchAuthState>>,
}

/// Shared Fetch-domain state: routing owns `patterns`, credentials own
/// `handleAuthRequests`, and every change re-enables with the merge.
#[derive(Debug, Default)]
struct FetchAuthState {
    /// Active routing patterns (`None` = routing off).
    routing_patterns: Option<Vec<Value>>,
    /// Credentials for `Fetch.authRequired` (`None` = no challenges).
    creds: Option<(String, String)>,
    /// Auth-challenge pump (alive while `creds` is set).
    auth_task: Option<tokio::task::AbortHandle>,
}

// Construct before the first native await; failed or dropped startup must not
// leave Fetch enabled without a request pump. The ordered writer runs cleanup
// after the startup command, even if its response arrives after cancellation.
struct FetchRoutingStartup {
    driver: CdpDriver,
    committed: bool,
}
impl Drop for FetchRoutingStartup {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut state = self
            .driver
            .fetch_auth
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        state.routing_patterns = None;
        self.driver
            .cdp
            .enqueue_cleanup(&self.driver.session, "Fetch.disable", Value::Null);
        if let Some(params) = fetch_enable_params(None, state.creds.is_some()) {
            self.driver
                .cdp
                .enqueue_cleanup(&self.driver.session, "Fetch.enable", params);
        }
    }
}

/// Merged `Fetch.enable` params (`None` = `Fetch.disable`).
///
/// Auth-only mode intercepts requests as required by CDP; the auth pump
/// continues those requests when no routing pump is active.
fn fetch_enable_params(patterns: Option<&[Value]>, handle_auth: bool) -> Option<Value> {
    if patterns.is_none() && !handle_auth {
        return None;
    }
    Some(serde_json::json!({
        "patterns": patterns.map(|patterns| patterns.to_vec()).unwrap_or_else(|| vec![serde_json::json!({"urlPattern":"*"})]),
        "handleAuthRequests": handle_auth,
    }))
}

/// BiDi-backed driver (Firefox): one browsing context.
#[derive(Clone)]
pub struct BidiDriver {
    modifiers: Arc<Mutex<u8>>,
    bidi: BidiConnection,
    context: String,
    timeout: Arc<Mutex<Duration>>,
    lifecycle: crate::CancellationToken,
    cancellation: crate::CancellationToken,
    context_cancellation: crate::CancellationToken,
    insecure_certs: bool,
    intercept: Arc<Mutex<Option<String>>>,
    sink: ConsoleSink,
    /// Owning user context (`None` = default; scopes permission grants).
    user_context: Option<String>,
}

/// Engine-agnostic page driver.
#[derive(Clone)]
pub enum Driver {
    /// Chromium over CDP.
    Cdp(CdpDriver),
    /// Firefox over WebDriver BiDi.
    Bidi(BidiDriver),
}

impl Driver {
    pub(crate) fn is_disconnected(&self) -> bool {
        match self {
            Self::Cdp(driver) => !driver.cdp.is_open(),
            Self::Bidi(driver) => !driver.bidi.is_open(),
        }
    }
    pub(crate) fn share_timeout(&mut self, timeout: Arc<Mutex<Duration>>) {
        match self {
            Self::Cdp(driver) => driver.timeout = timeout,
            Self::Bidi(driver) => driver.timeout = timeout,
        }
    }
    pub(crate) fn with_cancellation(mut self, token: crate::CancellationToken) -> Self {
        match &mut self {
            Self::Cdp(driver) => driver.cancellation = token,
            Self::Bidi(driver) => driver.cancellation = token,
        }
        self
    }
    pub(crate) fn route_fetch_owner(
        &self,
        registry: std::sync::Weak<Mutex<Vec<crate::BrowserContext>>>,
        context_id: Option<String>,
        timeout: Arc<Mutex<Duration>>,
    ) -> crate::route_options::RouteFetchOwner {
        let (page, context, transport) = match self {
            Self::Cdp(d) => (
                d.lifecycle.clone(),
                d.context_cancellation.clone(),
                d.cdp.disconnection(),
            ),
            Self::Bidi(d) => (
                d.lifecycle.clone(),
                d.context_cancellation.clone(),
                d.bidi.disconnection(),
            ),
        };
        crate::route_options::RouteFetchOwner {
            registry,
            context_id,
            timeout,
            page,
            context,
            transport,
        }
    }
    pub(crate) fn caller_cancellation(&self) -> crate::CancellationToken {
        match self {
            Self::Cdp(driver) => driver.cancellation.clone(),
            Self::Bidi(driver) => driver.cancellation.clone(),
        }
    }
    pub(crate) fn cancel_lifecycle(&self) {
        match self {
            Self::Cdp(driver) => driver.lifecycle.cancel_with_reason("page closed"),
            Self::Bidi(driver) => driver.lifecycle.cancel_with_reason("page closed"),
        }
    }
    pub(crate) fn bind_context_cancellation(&mut self, token: crate::CancellationToken) {
        match self {
            Self::Cdp(driver) => driver.context_cancellation = token,
            Self::Bidi(driver) => driver.context_cancellation = token,
        }
    }
    pub(crate) fn run<'a, T: 'a>(
        &'a self,
        future: impl std::future::Future<Output = E2eResult<T>> + 'a,
    ) -> impl std::future::Future<Output = E2eResult<T>> + 'a {
        let future = Box::pin(future);
        async move {
            let (lifecycle, context, cancellation) = match self {
                Self::Cdp(d) => (&d.lifecycle, &d.context_cancellation, &d.cancellation),
                Self::Bidi(d) => (&d.lifecycle, &d.context_cancellation, &d.cancellation),
            };
            let transport = match self {
                Self::Cdp(driver) => driver.cdp.disconnection(),
                Self::Bidi(driver) => driver.bidi.disconnection(),
            };
            if let Some(reason) = transport.reason() {
                return Err(E2eError::Disconnected(reason));
            }
            lifecycle.check()?;
            context.check()?;
            cancellation.check()?;
            tokio::select! {biased;
                reason=lifecycle.cancelled()=>Err(E2eError::Cancelled(reason)),
                reason=context.cancelled()=>Err(E2eError::Cancelled(reason)),
                reason=cancellation.cancelled()=>Err(E2eError::Cancelled(reason)),
                reason=transport.cancelled()=>Err(E2eError::Disconnected(reason)),
                result=future=>result,
            }
        }
    }
    pub(crate) fn run_close_wait<'a, T: 'a>(
        &'a self,
        future: impl std::future::Future<Output = E2eResult<T>> + 'a,
    ) -> impl std::future::Future<Output = E2eResult<T>> + 'a {
        let token = match self {
            Self::Cdp(driver) => &driver.cancellation,
            Self::Bidi(driver) => &driver.cancellation,
        };
        token.run(future)
    }
    pub(crate) async fn coverage_call(&self, method: &str, params: Value) -> E2eResult<Value> {
        match self {
            Self::Cdp(driver) => {
                self.run(
                    driver
                        .cdp
                        .call(Some(&driver.session), method, params, self.timeout()),
                )
                .await
            }
            Self::Bidi(_) => Err(E2eError::Config("coverage requires Chromium".into())),
        }
    }

    /// Target/context id for diagnostics.
    #[must_use]
    pub fn target_id(&self) -> &str {
        match self {
            Self::Cdp(driver) => &driver.target,
            Self::Bidi(driver) => &driver.context,
        }
    }

    pub(crate) fn cdp_connection(&self) -> E2eResult<CdpConnection> {
        match self {
            Self::Cdp(driver) => Ok(driver.cdp.clone()),
            Self::Bidi(_) => Err(E2eError::Config(
                "scoped CDP sessions require Chromium".into(),
            )),
        }
    }

    pub(crate) fn cdp_events(
        &self,
    ) -> E2eResult<(String, tokio::sync::broadcast::Receiver<CdpEvent>)> {
        match self {
            Self::Cdp(driver) => Ok((driver.session.clone(), driver.cdp.subscribe())),
            Self::Bidi(_) => Err(E2eError::Config("CDP events require Chromium".into())),
        }
    }

    /// Default protocol timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        match self {
            Self::Cdp(driver) => *driver.timeout.lock().unwrap_or_else(|e| e.into_inner()),
            Self::Bidi(driver) => *driver.timeout.lock().unwrap_or_else(|e| e.into_inner()),
        }
    }

    /// Override the protocol timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        match self {
            Self::Cdp(driver) => {
                *driver.timeout.lock().unwrap_or_else(|e| e.into_inner()) = timeout
            }
            Self::Bidi(driver) => {
                *driver.timeout.lock().unwrap_or_else(|e| e.into_inner()) = timeout
            }
        }
    }
}

// --- construction ------------------------------------------------------------

impl CdpDriver {
    fn timeout(&self) -> Duration {
        *self.timeout.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Enable domains and spawn the event listener.
    pub async fn spawn(
        cdp: CdpConnection,
        session: String,
        target: String,
        timeout: Duration,
        sink: ConsoleSink,
        browser_context: Option<String>,
    ) -> E2eResult<Self> {
        *sink.page_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(target.clone());
        let driver = Self {
            main_worlds: Arc::new(Mutex::new(HashMap::new())),
            cdp,
            session,
            target,
            timeout: Arc::new(Mutex::new(timeout)),
            lifecycle: crate::CancellationToken::new(),
            cancellation: crate::CancellationToken::new(),
            context_cancellation: crate::CancellationToken::new(),
            sink,
            modifiers: Arc::new(Mutex::new(0)),
            pressed: Arc::new(Mutex::new(false)),
            browser_context,
            fetch_auth: Arc::new(Mutex::new(FetchAuthState::default())),
        };
        let mut initialization = PageInitialization {
            lifecycle: driver.lifecycle.clone(),
            complete: false,
        };
        if let Some(capture) = &driver.sink.popup_capture {
            capture.bind_lifecycle(driver.lifecycle.clone());
        }
        driver.spawn_listener();
        let initialized = async {
            let tree = driver.call("Page.getFrameTree", Value::Null).await?;
            driver
                .sink
                .frame_events
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .seed_cdp(&driver.target, &tree["frameTree"]);
            driver.call("Page.enable", Value::Null).await?;
            driver.call("Runtime.enable", Value::Null).await?;
            driver.call("Log.enable", Value::Null).await?;
            driver.call("Network.enable", Value::Null).await?;
            Ok::<_, E2eError>(())
        }
        .await;
        if let Err(error) = initialized {
            driver
                .lifecycle
                .cancel_with_reason("page initialization failed");
            return Err(error);
        }
        initialization.complete = true;
        Ok(driver)
    }

    fn spawn_listener(&self) {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let sink = self.sink.clone();
        let worlds = self.main_worlds.clone();
        let lifecycle = self.lifecycle.clone();
        let transport = self.cdp.disconnection();
        let world_guard = MainWorldListenerGuard(worlds.clone());
        let listener_guard = NetworkListenerGuard(sink.clone());
        tokio::spawn(async move {
            let _world_guard = world_guard;
            let _guard = listener_guard;
            let mut downloads = HashMap::new();
            loop {
                let received = tokio::select! { biased; _ = lifecycle.cancelled() => Err(tokio::sync::broadcast::error::RecvError::Closed), _=transport.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed), event = events.recv() => event };
                let event = match received {
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        sink.socket_log
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .unavailable(&format!("native event channel lagged by {count}"));
                        break;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                if event.method == "Browser.downloadWillBegin"
                    && event.params["frameId"]
                        .as_str()
                        .is_some_and(|id| sink.contains_frame(id))
                {
                    downloads.insert(
                        event.params["guid"].as_str().unwrap_or_default().to_owned(),
                        event.params["suggestedFilename"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    );
                }
                if event.method == "Browser.downloadProgress"
                    && event.params["state"] == "completed"
                {
                    if let Some(filename) = event.params["guid"]
                        .as_str()
                        .and_then(|guid| downloads.remove(guid))
                    {
                        sink.completed_download(&filename, event.params["filePath"].as_str());
                    }
                }
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                match event.method.as_str() {
                    "Runtime.executionContextCreated" => {
                        let context = &event.params["context"];
                        if context["auxData"]["isDefault"].as_bool() == Some(true) {
                            if let (Some(frame), Some(id)) = (
                                context["auxData"]["frameId"].as_str(),
                                context["id"].as_i64(),
                            ) {
                                worlds
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .insert(frame.into(), id);
                            }
                        }
                    }
                    "Runtime.executionContextDestroyed" => {
                        let id = event.params["executionContextId"].as_i64();
                        worlds
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .retain(|_, value| Some(*value) != id);
                    }
                    "Runtime.executionContextsCleared" => {
                        worlds.lock().unwrap_or_else(|e| e.into_inner()).clear()
                    }
                    _ => {}
                }
                if sink.popup_capture.is_none() {
                    handle_cdp_event(&event, &sink);
                }
            }
        });
    }
}

impl BidiDriver {
    fn timeout(&self) -> Duration {
        *self.timeout.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Spawn the event listener (session-wide subscription already active).
    pub async fn spawn(
        bidi: BidiConnection,
        context: String,
        timeout: Duration,
        insecure_certs: bool,
        sink: ConsoleSink,
        user_context: Option<String>,
    ) -> E2eResult<Self> {
        *sink.page_id.lock().unwrap_or_else(|e| e.into_inner()) = Some(context.clone());
        sink.frame_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .seed_bidi(&context);
        let driver = Self {
            modifiers: Arc::new(Mutex::new(0)),
            bidi,
            context,
            timeout: Arc::new(Mutex::new(timeout)),
            lifecycle: crate::CancellationToken::new(),
            cancellation: crate::CancellationToken::new(),
            context_cancellation: crate::CancellationToken::new(),
            insecure_certs,
            intercept: Arc::new(Mutex::new(None)),
            sink,
            user_context,
        };
        let mut initialization = PageInitialization {
            lifecycle: driver.lifecycle.clone(),
            complete: false,
        };
        if let Some(capture) = &driver.sink.popup_capture {
            capture.bind_lifecycle(driver.lifecycle.clone());
        }
        driver.spawn_listener();
        let tree = driver
            .call(
                "browsingContext.getTree",
                serde_json::json!({"root":driver.context}),
            )
            .await?;
        driver
            .sink
            .frame_events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .seed_bidi_tree(&driver.context, &tree["contexts"]);
        initialization.complete = true;
        Ok(driver)
    }

    pub(crate) fn supports_lifecycle_event(&self, event: &str) -> bool {
        self.bidi.supports_lifecycle_event(event)
    }

    fn spawn_listener(&self) {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let sink = self.sink.clone();
        let lifecycle = self.lifecycle.clone();
        let transport = self.bidi.disconnection();
        let listener_guard = NetworkListenerGuard(sink.clone());
        tokio::spawn(async move {
            let _guard = listener_guard;
            let mut downloads = HashMap::new();
            while let Ok(event) = tokio::select! {biased; _=lifecycle.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed), _=transport.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed), event=events.recv()=>event}
            {
                let belongs = if event.method == "browsingContext.contextCreated" {
                    event.params["parent"]
                        .as_str()
                        .is_some_and(|parent| sink.contains_frame(parent))
                } else {
                    event
                        .context()
                        .is_some_and(|id| id == context || sink.contains_frame(id))
                };
                if !belongs {
                    continue;
                }
                if event.method == "browsingContext.downloadWillBegin" {
                    downloads.insert(
                        event.params["download"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        event.params["suggestedFilename"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                    );
                }
                if event.method == "browsingContext.downloadEnd"
                    && event.params["status"] == "complete"
                {
                    let filename = event.params["download"]
                        .as_str()
                        .and_then(|id| downloads.remove(id))
                        .unwrap_or_default();
                    sink.completed_download(&filename, event.params["filepath"].as_str());
                }
                if sink.popup_capture.is_none() {
                    handle_bidi_event(&event, &sink);
                }
            }
        });
    }
}

// --- Driver dispatch ---------------------------------------------------------

impl Driver {
    /// Navigate and wait for the load state.
    pub async fn navigate(&self, url: &str, wait: LoadState, timeout: Duration) -> E2eResult<()> {
        self.run(
            crate::operation::Deadline::new(timeout).run("navigation", async {
                match self {
                    Self::Cdp(driver) => driver.navigate(url, wait, timeout).await,
                    Self::Bidi(driver) => driver.navigate(url, wait, timeout).await,
                }
            }),
        )
        .await
    }

    /// Reload the page.
    pub async fn reload(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.reload().await,
                Self::Bidi(driver) => driver.reload().await,
            }
        })
        .await
    }

    /// Traverse history by `delta` entries.
    pub async fn traverse(&self, delta: i64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.traverse(delta).await,
                Self::Bidi(driver) => driver.traverse(delta).await,
            }
        })
        .await
    }

    /// Evaluate JavaScript, returning plain JSON.
    pub async fn evaluate(&self, expression: &str) -> E2eResult<Value> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.evaluate(expression).await,
                Self::Bidi(driver) => driver.evaluate(expression).await,
            }
        })
        .await
    }

    /// Evaluate `expression` and keep the result alive as a [`JSHandle`].
    pub async fn evaluate_handle(&self, expression: &str) -> E2eResult<JSHandle> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.evaluate_handle(expression).await,
                Self::Bidi(driver) => driver.evaluate_handle(expression).await,
            }
        })
        .await
    }

    /// Serialize a handle's value to JSON.
    pub async fn handle_json_value(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
    ) -> E2eResult<Value> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.handle_json_value(remote_id, value).await,
                Self::Bidi(driver) => driver.handle_json_value(remote_id, value).await,
            }
        })
        .await
    }

    /// A handle to the named property of a handle's value.
    pub async fn handle_get_property(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        name: &str,
    ) -> E2eResult<JSHandle> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.handle_get_property(remote_id, value, name).await,
                Self::Bidi(driver) => driver.handle_get_property(remote_id, value, name).await,
            }
        })
        .await
    }

    /// Run `function` with a handle's value as its first argument.
    pub async fn handle_evaluate(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<Value> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.handle_evaluate(remote_id, value, function).await,
                Self::Bidi(driver) => driver.handle_evaluate(remote_id, value, function).await,
            }
        })
        .await
    }

    pub async fn handle_evaluate_handle(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<JSHandle> {
        self.run(async {
            match self {
                Self::Cdp(driver) => {
                    driver
                        .handle_evaluate_handle(remote_id, value, function)
                        .await
                }
                Self::Bidi(driver) => {
                    driver
                        .handle_evaluate_handle(remote_id, value, function)
                        .await
                }
            }
        })
        .await
    }

    /// Release a handle's remote reference.
    pub async fn handle_dispose(&self, remote_id: Option<&str>) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.handle_dispose(remote_id).await,
                Self::Bidi(driver) => driver.handle_dispose(remote_id).await,
            }
        })
        .await
    }

    /// Bring the page to front.
    pub async fn bring_to_front(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.bring_to_front().await,
                Self::Bidi(driver) => driver.bring_to_front().await,
            }
        })
        .await
    }

    /// Set the document HTML.
    pub async fn set_content(&self, html: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_content(html).await,
                Self::Bidi(driver) => driver.set_content(html).await,
            }
        })
        .await
    }

    pub(crate) async fn capture_region(
        &self,
        rect: &ElementRect,
        document: bool,
        scale: f64,
        quality: Option<u8>,
    ) -> E2eResult<Vec<u8>> {
        let result = match self {
            Self::Cdp(d) => {
                let mut params = serde_json::json!({"captureBeyondViewport":true,"clip":{"x":rect.x,"y":rect.y,"width":rect.width,"height":rect.height,"scale":scale}});
                if let Some(q) = quality {
                    params["format"] = "jpeg".into();
                    params["quality"] = q.into();
                }
                d.call("Page.captureScreenshot", params).await?
            }
            Self::Bidi(d) => {
                let mut params = serde_json::json!({"origin":if document {"document"} else {"viewport"},"clip":{"type":"box","x":rect.x,"y":rect.y,"width":rect.width,"height":rect.height}});
                if let Some(q) = quality {
                    params["format"] =
                        serde_json::json!({"type":"image/jpeg","quality":f64::from(q)/100.0});
                }
                d.call("browsingContext.captureScreenshot", params).await?
            }
        };
        decode_shot(&result)
    }

    /// Print to PDF bytes.
    pub async fn print_pdf(&self) -> E2eResult<Vec<u8>> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.print_pdf().await,
                Self::Bidi(driver) => driver.print_pdf().await,
            }
        })
        .await
    }

    pub(crate) async fn print_pdf_with(&self, parameters: Value) -> E2eResult<Vec<u8>> {
        self.run(async {
            match self {
                Self::Cdp(driver) => {
                    let result = driver.call("Page.printToPDF", parameters).await?;
                    decode_shot(&result)
                }
                Self::Bidi(_) => Err(E2eError::Config("PDF options require Chromium".into())),
            }
        })
        .await
    }

    /// Set the viewport size.
    pub async fn set_viewport(&self, width: u32, height: u32) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_viewport(width, height).await,
                Self::Bidi(driver) => driver.set_viewport(width, height).await,
            }
        })
        .await
    }

    /// Emulate a device (Chromium: metrics + touch; Firefox: unsupported).
    pub async fn emulate_device(&self, device: crate::page::DeviceDescriptor) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.emulate_device(device).await,
                Self::Bidi(driver) => driver.emulate_device(device).await,
            }
        })
        .await
    }

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_user_agent(user_agent).await,
                Self::Bidi(driver) => driver.set_user_agent(user_agent).await,
            }
        })
        .await
    }

    /// Direct downloads to `dir` (Chromium; Firefox is launch-time only).
    pub async fn set_download_dir(&self, dir: &Path) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_download_dir(dir).await,
                Self::Bidi(driver) => driver.set_download_dir(dir).await,
            }
        })
        .await
    }

    /// Cancel in-flight downloads (Chromium only).
    pub async fn cancel_downloads(&self) -> E2eResult<usize> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.cancel_downloads().await,
                Self::Bidi(driver) => driver.cancel_downloads().await,
            }
        })
        .await
    }

    /// Tracked downloads for URL/failure matching (Chromium only).
    pub fn download_records(&self) -> Vec<crate::cdp::DownloadRecord> {
        match self {
            Self::Cdp(driver) => driver.cdp.download_records(),
            Self::Bidi(_) => Vec::new(),
        }
    }

    /// Run `source` before page scripts in every future document.
    pub async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        self.add_owned_init_script(source).await.map(|_| ())
    }

    pub(crate) async fn add_owned_init_script(&self, source: &str) -> E2eResult<String> {
        self.run(async {
            // Receive the native ID even if the caller drops this registration.
            // A dropped receiver removes an orphan preload rather than losing it.
            let driver = self.clone().with_cancellation(crate::CancellationToken::new());
            let source = source.to_owned();
            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let result = driver.run(async {
                    let result = match &driver {
                        Self::Cdp(driver) => driver.call("Page.addScriptToEvaluateOnNewDocument", serde_json::json!({"source": source})).await?,
                        Self::Bidi(driver) => driver.bidi.call("script.addPreloadScript", serde_json::json!({"functionDeclaration": format!("() => {{ {source} }}"), "contexts": [driver.context]}), driver.timeout()).await?,
                    };
                    let key = if matches!(driver, Self::Cdp(_)) { "identifier" } else { "script" };
                    result[key].as_str().map(str::to_owned).ok_or_else(|| E2eError::Config("native preload ID unavailable".into()))
                }).await;
                if let Err(Ok(id)) = tx.send(result) { let _ = driver.remove_owned_init_script(&id).await; }
            });
            rx.await.map_err(|_| E2eError::Disconnected("owned preload registration dropped".into()))?
        }).await
    }

    pub(crate) async fn remove_owned_init_script(&self, id: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => {
                    driver
                        .call(
                            "Page.removeScriptToEvaluateOnNewDocument",
                            serde_json::json!({"identifier": id}),
                        )
                        .await?;
                }
                Self::Bidi(driver) => {
                    driver
                        .bidi
                        .call(
                            "script.removePreloadScript",
                            serde_json::json!({"script": id}),
                            driver.timeout(),
                        )
                        .await?;
                }
            }
            Ok(())
        })
        .await
    }

    /// List frames (main frame first).
    pub async fn frames(&self) -> E2eResult<Vec<FrameInfo>> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.frames().await,
                Self::Bidi(driver) => driver.frames().await,
            }
        })
        .await
    }

    // Only main-world cache consumers depend on the console/network listener.
    // Isolated-world frame evaluation and BiDi's direct context evaluation do not.
    pub(crate) fn observe_main_world<'a, T: 'a>(
        &'a self,
        future: impl std::future::Future<Output = E2eResult<T>> + 'a,
    ) -> impl std::future::Future<Output = E2eResult<T>> + 'a {
        self.run(async {
            match self {
                Self::Cdp(driver) => {
                    let source = driver.sink.native_observation_loss();
                    tokio::select! { biased;
                        reason = source.cancelled() => Err(E2eError::Config(format!("frame main-world observation source unavailable: {reason}"))),
                        result = future => result,
                    }
                }
                Self::Bidi(_) => future.await,
            }
        })
    }

    pub(crate) async fn frame_main_world_evaluate(
        &self,
        frame_id: &str,
        expression: &str,
    ) -> E2eResult<Value> {
        self.observe_main_world(async {
            match self {
                Self::Cdp(driver) => {
                    let id = driver
                        .main_worlds
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(frame_id)
                        .copied()
                        .ok_or_else(|| {
                            E2eError::Config("frame main world is unavailable or detached".into())
                        })?;
                    driver.evaluate_with_context(expression, Some(id)).await
                }
                Self::Bidi(driver) => driver.frame_evaluate(frame_id, expression).await,
            }
        })
        .await
    }

    /// Evaluate in a frame by listing id.
    pub async fn frame_evaluate(&self, frame_id: &str, expression: &str) -> E2eResult<Value> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.frame_evaluate(frame_id, expression).await,
                Self::Bidi(driver) => driver.frame_evaluate(frame_id, expression).await,
            }
        })
        .await
    }

    /// Navigate one frame (subframe ids from [`Driver::frames`]).
    pub async fn frame_navigate(&self, frame_id: &str, url: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.frame_navigate(frame_id, url).await,
                Self::Bidi(driver) => driver.frame_navigate(frame_id, url).await,
            }
        })
        .await
    }

    /// Request a garbage collection (Chromium only).
    pub async fn request_gc(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.request_gc().await,
                Self::Bidi(driver) => driver.request_gc().await,
            }
        })
        .await
    }

    /// Ignore HTTPS certificate errors.
    pub async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_ignore_https_errors(ignore).await,
                Self::Bidi(driver) => driver.set_ignore_https_errors(ignore).await,
            }
        })
        .await
    }

    /// Visible cookies.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.cookies().await,
                Self::Bidi(driver) => driver.cookies().await,
            }
        })
        .await
    }

    /// Set a cookie (url hint for domain derivation).
    pub async fn set_cookie(&self, name: &str, value: &str, url: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_cookie(name, value, url).await,
                Self::Bidi(driver) => driver.set_cookie(name, value, url).await,
            }
        })
        .await
    }

    /// Set full-fidelity cookies (domain defaults to `url`'s host).
    pub async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.add_cookies(cookies, url).await,
                Self::Bidi(driver) => driver.add_cookies(cookies, url).await,
            }
        })
        .await
    }

    /// Clear cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.clear_cookies().await,
                Self::Bidi(driver) => driver.clear_cookies().await,
            }
        })
        .await
    }

    /// Move the mouse.
    pub async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_move(x, y).await,
                Self::Bidi(driver) => driver.mouse_move(x, y).await,
            }
        })
        .await
    }

    /// Trusted click at coordinates.
    pub async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.run(async {
            let options = crate::page::MouseClickOptions {
                click_count,
                ..crate::page::MouseClickOptions::default()
            };
            self.mouse_click_with(x, y, &options).await
        })
        .await
    }

    /// Trusted click with button/count/delay options.
    pub async fn mouse_click_with(
        &self,
        x: f64,
        y: f64,
        options: &crate::page::MouseClickOptions,
    ) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_click_with(x, y, options).await,
                Self::Bidi(driver) => driver.mouse_click_with(x, y, options).await,
            }
        })
        .await
    }

    /// Insert text at the focused element.
    pub async fn insert_text(&self, text: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.insert_text(text).await,
                Self::Bidi(driver) => driver.insert_text(text).await,
            }
        })
        .await
    }

    /// Dispatch a key press.
    pub async fn press_key(&self, key: &str) -> E2eResult<()> {
        self.run(async {
            self.press_key_with(key, &crate::page::KeyPressOptions::default())
                .await
        })
        .await
    }

    /// Dispatch a key press with down/up delay.
    pub async fn press_key_with(
        &self,
        key: &str,
        options: &crate::page::KeyPressOptions,
    ) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.press_key_with(key, options).await,
                Self::Bidi(driver) => driver.press_key_with(key, options).await,
            }
        })
        .await
    }

    /// Press the left mouse button at coordinates.
    pub async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_down(x, y).await,
                Self::Bidi(driver) => driver.mouse_down(x, y).await,
            }
        })
        .await
    }

    /// Release the left mouse button at coordinates.
    pub async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_up(x, y).await,
                Self::Bidi(driver) => driver.mouse_up(x, y).await,
            }
        })
        .await
    }

    /// Drag from one point to another in `steps` paced moves.
    pub async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_drag(from, to, steps).await,
                Self::Bidi(driver) => driver.mouse_drag(from, to, steps).await,
            }
        })
        .await
    }

    /// Scroll a wheel at coordinates by (`delta_x`, `delta_y`).
    pub async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.mouse_wheel(x, y, delta_x, delta_y).await,
                Self::Bidi(driver) => driver.mouse_wheel(x, y, delta_x, delta_y).await,
            }
        })
        .await
    }

    pub(crate) fn modifier_held(&self, key: &str) -> bool {
        let held = match self {
            Self::Cdp(driver) => driver.held_modifiers(),
            Self::Bidi(driver) => *driver.modifiers.lock().unwrap_or_else(|e| e.into_inner()),
        };
        held & modifier_bit(key) != 0
    }
    pub(crate) async fn release_mouse_button(
        &self,
        button: crate::MouseButton,
        x: f64,
        y: f64,
    ) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver)=>{
                    driver.call("Input.dispatchMouseEvent",serde_json::json!({"type":"mouseReleased","x":x,"y":y,"button":button.as_cdp(),"clickCount":1,"modifiers":driver.held_modifiers()})).await?;
                    if button==crate::MouseButton::Left {*driver.pressed.lock().unwrap_or_else(|e|e.into_inner())=false;}
                }
                Self::Bidi(driver)=>driver.perform(serde_json::json!([{"type":"pointer","id":"ferrite-mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerUp","button":button.as_bidi()}]}])).await?,
            }
            Ok(())
        }).await
    }

    /// Hold a key down (pair with [`Driver::key_up`]).
    pub async fn key_down(&self, key: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.key_down(key).await,
                Self::Bidi(driver) => driver.key_down(key).await,
            }
        })
        .await
    }

    /// Release a held key.
    pub async fn key_up(&self, key: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.key_up(key).await,
                Self::Bidi(driver) => driver.key_up(key).await,
            }
        })
        .await
    }

    /// Tap at coordinates with the touchscreen.
    pub async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.touchscreen_tap(x, y).await,
                Self::Bidi(driver) => driver.touchscreen_tap(x, y).await,
            }
        })
        .await
    }

    /// Grant permissions.
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.grant_permissions(permissions).await,
                Self::Bidi(driver) => driver.grant_permissions(permissions).await,
            }
        })
        .await
    }

    /// Reset granted permissions.
    pub async fn clear_permissions(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.clear_permissions().await,
                Self::Bidi(driver) => driver.clear_permissions().await,
            }
        })
        .await
    }

    /// Override the geolocation coordinates.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_geolocation(latitude, longitude).await,
                Self::Bidi(driver) => driver.set_geolocation(latitude, longitude).await,
            }
        })
        .await
    }

    /// Clear the geolocation override.
    pub async fn clear_geolocation(&self) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.clear_geolocation().await,
                Self::Bidi(driver) => driver.clear_geolocation().await,
            }
        })
        .await
    }

    /// Emulate offline mode.
    pub async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_offline(offline).await,
                Self::Bidi(driver) => driver.set_offline(offline).await,
            }
        })
        .await
    }

    /// Set extra HTTP headers for subsequent requests.
    pub async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_extra_http_headers(headers).await,
                Self::Bidi(driver) => driver.set_extra_http_headers(headers).await,
            }
        })
        .await
    }

    /// Override the locale.
    pub async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_locale(locale).await,
                Self::Bidi(driver) => driver.set_locale(locale).await,
            }
        })
        .await
    }

    /// Override the timezone.
    pub async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_timezone(timezone_id).await,
                Self::Bidi(driver) => driver.set_timezone(timezone_id).await,
            }
        })
        .await
    }

    /// Enable or disable JavaScript execution.
    pub async fn set_java_script_enabled(&self, enabled: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_java_script_enabled(enabled).await,
                Self::Bidi(driver) => driver.set_java_script_enabled(enabled).await,
            }
        })
        .await
    }

    /// Bypass Content-Security-Policy checks.
    pub async fn set_bypass_csp(&self, bypass: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_bypass_csp(bypass).await,
                Self::Bidi(driver) => driver.set_bypass_csp(bypass).await,
            }
        })
        .await
    }

    /// Allow or deny downloads browser-wide.
    pub async fn set_downloads_allowed(&self, allowed: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_downloads_allowed(allowed).await,
                Self::Bidi(driver) => driver.set_downloads_allowed(allowed).await,
            }
        })
        .await
    }

    /// Block service workers (Chromium bypasses them; see note there).
    pub async fn set_service_workers_blocked(&self, blocked: bool) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.set_service_workers_blocked(blocked).await,
                Self::Bidi(driver) => driver.set_service_workers_blocked(blocked).await,
            }
        })
        .await
    }

    /// Clear `storage_types` (e.g. `indexeddb`) for one origin.
    pub async fn clear_data_for_origin(&self, origin: &str, storage_types: &str) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.clear_data_for_origin(origin, storage_types).await,
                Self::Bidi(driver) => driver.clear_data_for_origin(origin, storage_types).await,
            }
        })
        .await
    }

    /// Answer HTTP auth challenges with `username`/`password` (`None` clears).
    pub async fn set_auth_credentials(
        &self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> E2eResult<()> {
        self.run(async {
            let creds = match (username, password) {
                (Some(user), Some(pass)) => Some((user.to_string(), pass.to_string())),
                (None, None) => None,
                _ => {
                    return Err(E2eError::Config(
                    "set_auth_credentials needs both username and password (or neither to clear)"
                        .to_string(),
                ));
                }
            };
            match self {
                Self::Cdp(driver) => driver.set_auth_credentials(creds).await,
                Self::Bidi(driver) => driver.set_auth_credentials(creds).await,
            }
        })
        .await
    }

    pub(crate) fn validate_routing(
        &self,
        configuration: &crate::routing::RouteConfiguration,
    ) -> E2eResult<()> {
        configuration.validate_for(matches!(self, Self::Bidi(_)))
    }
    pub(crate) async fn start_routing(
        &self,
        runtime: Arc<crate::routing::RouteRuntime>,
        slot: std::sync::Weak<crate::routing::PumpSlot>,
    ) -> E2eResult<crate::routing::RoutePump> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.start_routing(runtime, slot).await,
                Self::Bidi(driver) => driver.start_routing(runtime, slot).await,
            }
        })
        .await
    }

    /// Disable interception.
    pub async fn stop_routing(&self) {
        match self {
            Self::Cdp(driver) => driver.stop_routing().await,
            Self::Bidi(driver) => driver.stop_routing().await,
        }
    }

    /// Auto-handle dialogs; returns the handler task handle.
    /// Start recording requests into the shared sink.
    pub fn start_request_capture(&self) -> tokio::task::AbortHandle {
        match self {
            Self::Cdp(driver) => driver.start_request_capture(),
            Self::Bidi(driver) => driver.start_request_capture(),
        }
    }

    pub async fn start_dialogs(
        &self,
        accept: bool,
        prompt_text: Option<String>,
        handler: Option<DialogHandler>,
    ) -> E2eResult<tokio::task::AbortHandle> {
        self.run(async {
            match self {
                Self::Cdp(driver) => Ok(driver.start_dialogs(accept, prompt_text, handler)),
                Self::Bidi(driver) => Ok(driver.start_dialogs(accept, prompt_text, handler)),
            }
        })
        .await
    }

    /// Wait for a document load state.
    pub async fn wait_for_load(&self, state: LoadState, timeout: Duration) -> E2eResult<()> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.wait_for_load(state, timeout).await,
                Self::Bidi(driver) => driver.wait_for_load(state, timeout).await,
            }
        })
        .await
    }

    /// Close the page target.
    pub async fn close(&self) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.close().await,
            Self::Bidi(driver) => driver.close().await,
        }
    }

    /// Raw protocol call (CDP method or BiDi method with context injected).
    pub async fn raw(&self, method: &str, params: Value, timeout: Duration) -> E2eResult<Value> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.raw(method, params, timeout).await,
                Self::Bidi(driver) => driver.raw(method, params, timeout).await,
            }
        })
        .await
    }

    /// Start a live frame stream (screencast on Chromium, paced
    /// screenshots on Firefox). Returns the stream plus the pump handle;
    /// call [`Driver::stop_frame_stream`] after aborting the pump.
    pub async fn start_frame_stream(
        &self,
        opts: &VideoOptions,
    ) -> E2eResult<(FrameStream, tokio::task::AbortHandle)> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.start_frame_stream(opts).await,
                Self::Bidi(driver) => driver.start_frame_stream(opts).await,
            }
        })
        .await
    }

    /// Stop a frame stream at the protocol level (best effort).
    pub async fn stop_frame_stream(&self) {
        match self {
            Self::Cdp(driver) => driver.stop_frame_stream().await,
            Self::Bidi(driver) => driver.stop_frame_stream().await,
        }
    }

    /// Start recording video to `opts.dir`.
    pub async fn start_recording(&self, opts: &VideoOptions) -> E2eResult<RecordingState> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.start_recording(opts).await,
                Self::Bidi(driver) => driver.start_recording(opts).await,
            }
        })
        .await
    }

    /// Stop a recording and produce the output video.
    pub async fn stop_recording(
        &self,
        state: RecordingState,
        output: &std::path::Path,
    ) -> E2eResult<PathBuf> {
        self.run(async {
            match self {
                Self::Cdp(driver) => driver.stop_recording(state, output).await,
                Self::Bidi(driver) => driver.stop_recording(state, output).await,
            }
        })
        .await
    }

    /// Discard a recording without producing output.
    pub async fn cancel_recording(&self, state: RecordingState) {
        match self {
            Self::Cdp(driver) => driver.cancel_recording(state).await,
            Self::Bidi(driver) => driver.cancel_recording(state).await,
        }
    }
}

/// Live frame stream.
#[derive(Debug)]
pub struct FrameStream {
    receiver: tokio::sync::mpsc::UnboundedReceiver<VideoFrame>,
}

impl FrameStream {
    /// Next frame, or `None` when the stream ends.
    pub async fn next(&mut self) -> Option<VideoFrame> {
        self.receiver.recv().await
    }

    /// Next frame, or `None` on timeout or stream end (Chromium emits on
    /// repaint only, so static pages yield nothing until damage occurs).
    pub async fn next_timeout(&mut self, timeout: Duration) -> Option<VideoFrame> {
        tokio::time::timeout(timeout, self.receiver.recv())
            .await
            .ok()?
    }
}

/// Opaque per-engine recording state.
pub enum RecordingState {
    /// Chromium: frame spool + pump task.
    Cdp {
        /// Spool directory with `frame-%06d.jpg` + `manifest.jsonl`.
        spool: PathBuf,
        /// Pump task handle.
        pump: tokio::task::AbortHandle,
        /// Target fps for assembly.
        fps: u32,
        /// Recording start (for the trailing frame duration).
        started: std::time::Instant,
    },
    /// Firefox: native screencast id + reported path.
    Bidi {
        /// Screencast uuid for `stopScreencast`.
        screencast: String,
        /// Path reported by `startScreencast`.
        path: PathBuf,
    },
}

// --- CDP driver --------------------------------------------------------------

impl CdpDriver {
    async fn call(&self, method: &str, params: Value) -> E2eResult<Value> {
        self.cdp
            .call(Some(&self.session), method, params, self.timeout())
            .await
            .map_err(|error| match error {
                E2eError::Cdp { message, .. } => E2eError::Cdp {
                    method: method.to_string(),
                    message,
                },
                other => other,
            })
    }

    async fn navigate(&self, url: &str, wait: LoadState, timeout: Duration) -> E2eResult<()> {
        let mut events = self.cdp.subscribe();
        let result = self
            .cdp
            .call(
                Some(&self.session),
                "Page.navigate",
                serde_json::json!({ "url": url }),
                timeout,
            )
            .await
            .map_err(|error| E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            })?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            return Err(E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            });
        }
        self.sink.record("navigation", format!("goto {url}"));
        self.wait_for_load_with_events(wait, timeout, &mut events)
            .await
    }

    async fn reload(&self) -> E2eResult<()> {
        self.call("Page.reload", Value::Null).await?;
        self.wait_for_load(LoadState::Load, self.timeout()).await
    }

    async fn traverse(&self, delta: i64) -> E2eResult<()> {
        let history = self.call("Page.getNavigationHistory", Value::Null).await?;
        let index = history
            .get("currentIndex")
            .and_then(Value::as_u64)
            .unwrap_or(0) as i64;
        let entries = history
            .get("entries")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0) as i64;
        let next = (index + delta).clamp(0, entries.saturating_sub(1));
        if next == index {
            return Ok(());
        }
        let id = history["entries"][next as usize]["id"].clone();
        self.call(
            "Page.navigateToHistoryEntry",
            serde_json::json!({ "entryId": id }),
        )
        .await?;
        self.wait_for_load(LoadState::Load, self.timeout()).await
    }

    async fn evaluate(&self, expression: &str) -> E2eResult<Value> {
        self.evaluate_with_context(expression, None).await
    }

    async fn evaluate_with_context(
        &self,
        expression: &str,
        context_id: Option<i64>,
    ) -> E2eResult<Value> {
        let mut params = serde_json::json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true,
        });
        if let Some(id) = context_id {
            params["contextId"] = Value::from(id);
        }
        let result = self.call("Runtime.evaluate", params).await?;
        if let Some(exception) = result.get("exceptionDetails") {
            let message = exception["exception"]["description"]
                .as_str()
                .or_else(|| exception["text"].as_str())
                .unwrap_or("JavaScript exception");
            return Err(E2eError::Cdp {
                method: "Runtime.evaluate".into(),
                message: message.into(),
            });
        }

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn evaluate_handle(&self, expression: &str) -> E2eResult<JSHandle> {
        let result = self
            .call(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": false,
                    "awaitPromise": true,
                }),
            )
            .await?;
        check_cdp_exception("Runtime.evaluate", &result, expression)?;
        Ok(self.cdp_handle(&result["result"]))
    }

    /// Wrap a CDP remote object: `objectId` stays remote, plain values inline.
    fn cdp_handle(&self, remote: &Value) -> JSHandle {
        let remote_id = remote
            .get("objectId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let value = if remote_id.is_none() {
            remote.get("value").cloned()
        } else {
            None
        };
        JSHandle::new(Driver::Cdp(self.clone()), remote_id, value)
    }

    async fn handle_json_value(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
    ) -> E2eResult<Value> {
        if let Some(value) = value {
            return Ok(value);
        }
        let Some(id) = remote_id else {
            return Ok(Value::Null);
        };
        let result = self
            .call(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": "function() { return this; }",
                    "objectId": id,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
        check_cdp_exception("Runtime.callFunctionOn", &result, "json_value")?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn handle_get_property(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        name: &str,
    ) -> E2eResult<JSHandle> {
        // Inlined primitives resolve locally (JSON indexing).
        if let Some(value) = value {
            let prop = value.get(name).cloned().unwrap_or(Value::Null);
            return Ok(JSHandle::new(Driver::Cdp(self.clone()), None, Some(prop)));
        }
        let Some(id) = remote_id else {
            return Err(E2eError::Cdp {
                method: "Runtime.callFunctionOn".to_string(),
                message: "get_property needs a remote handle or a value".to_string(),
            });
        };
        let result = self
            .call(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": "function(name) { return this[name]; }",
                    "objectId": id,
                    "arguments": [{ "value": name }],
                    "returnByValue": false,
                    "awaitPromise": true,
                }),
            )
            .await?;
        check_cdp_exception("Runtime.callFunctionOn", &result, name)?;
        Ok(self.cdp_handle(&result["result"]))
    }

    async fn handle_evaluate(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<Value> {
        // Inlined primitives apply the function to a JSON literal in-page.
        if let Some(value) = value {
            let literal = serde_json::to_string(&value)?;
            return self.evaluate(&format!("({function})({literal})")).await;
        }
        let Some(id) = remote_id else {
            return Err(E2eError::Cdp {
                method: "Runtime.callFunctionOn".to_string(),
                message: "handle evaluate needs a remote handle or a value".to_string(),
            });
        };
        let result = self
            .call(
                "Runtime.callFunctionOn",
                serde_json::json!({
                    "functionDeclaration": format!("function() {{ return ({function})(this); }}"),
                    "objectId": id,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
        check_cdp_exception("Runtime.callFunctionOn", &result, function)?;
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    async fn handle_evaluate_handle(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<JSHandle> {
        if let Some(value) = value {
            return self
                .evaluate_handle(&format!("({function})({})", serde_json::to_string(&value)?))
                .await;
        }
        let id = remote_id.ok_or_else(|| E2eError::Config("remote handle is missing".into()))?;
        let result = self.call("Runtime.callFunctionOn", serde_json::json!({"functionDeclaration":format!("function() {{ return ({function})(this); }}"),"objectId":id,"returnByValue":false,"awaitPromise":true})).await?;
        check_cdp_exception("Runtime.callFunctionOn", &result, function)?;
        let remote = &result["result"];
        Ok(JSHandle::new(
            Driver::Cdp(self.clone()),
            remote["objectId"].as_str().map(str::to_string),
            remote.get("value").cloned(),
        ))
    }

    async fn handle_dispose(&self, remote_id: Option<&str>) -> E2eResult<()> {
        let Some(id) = remote_id else {
            return Ok(());
        };
        self.call(
            "Runtime.releaseObject",
            serde_json::json!({ "objectId": id }),
        )
        .await?;
        Ok(())
    }

    async fn frames(&self) -> E2eResult<Vec<FrameInfo>> {
        let tree = self.call("Page.getFrameTree", Value::Null).await?;
        let mut out = Vec::new();
        collect_cdp_frames(&tree["frameTree"], &mut out);
        Ok(out)
    }

    async fn frame_evaluate(&self, frame_id: &str, expression: &str) -> E2eResult<Value> {
        let world = self
            .call(
                "Page.createIsolatedWorld",
                serde_json::json!({ "frameId": frame_id, "worldName": "ferrite" }),
            )
            .await?;
        let context_id = world.get("executionContextId").and_then(Value::as_i64);
        self.evaluate_with_context(expression, context_id).await
    }

    async fn frame_navigate(&self, frame_id: &str, url: &str) -> E2eResult<()> {
        let result = self
            .call(
                "Page.navigate",
                serde_json::json!({ "url": url, "frameId": frame_id }),
            )
            .await
            .map_err(|error| E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            })?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            return Err(E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            });
        }
        self.sink.record("navigation", format!("frame goto {url}"));
        Ok(())
    }

    async fn request_gc(&self) -> E2eResult<()> {
        self.call("HeapProfiler.collectGarbage", Value::Null)
            .await?;
        Ok(())
    }

    async fn bring_to_front(&self) -> E2eResult<()> {
        self.call("Page.bringToFront", Value::Null).await?;
        Ok(())
    }

    async fn set_content(&self, html: &str) -> E2eResult<()> {
        let tree = self.call("Page.getFrameTree", Value::Null).await?;
        let frame_id = tree["frameTree"]["frame"]["id"].clone();
        self.call(
            "Page.setDocumentContent",
            serde_json::json!({ "frameId": frame_id, "html": html }),
        )
        .await?;
        Ok(())
    }

    async fn print_pdf(&self) -> E2eResult<Vec<u8>> {
        let pdf = self.call("Page.printToPDF", Value::Null).await?;
        decode_shot(&pdf)
    }

    async fn set_viewport(&self, width: u32, height: u32) -> E2eResult<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({
                "width": width, "height": height,
                "deviceScaleFactor": 1, "mobile": false,
            }),
        )
        .await?;
        Ok(())
    }

    async fn emulate_device(&self, device: crate::page::DeviceDescriptor) -> E2eResult<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({
                "width": device.viewport.width, "height": device.viewport.height,
                "deviceScaleFactor": device.device_scale_factor,
                "mobile": device.mobile,
            }),
        )
        .await?;
        let mut touch = serde_json::json!({ "enabled": device.has_touch });
        if device.has_touch {
            touch["maxTouchPoints"] = serde_json::Value::from(5);
        }
        self.call("Emulation.setTouchEmulationEnabled", touch)
            .await?;
        Ok(())
    }

    async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.call(
            "Emulation.setUserAgentOverride",
            serde_json::json!({ "userAgent": user_agent }),
        )
        .await?;
        Ok(())
    }

    async fn set_download_dir(&self, dir: &Path) -> E2eResult<()> {
        std::fs::create_dir_all(dir)?;
        let mut params = serde_json::json!({"behavior":"allow", "downloadPath":dir.to_string_lossy(), "eventsEnabled":true});
        if let Some(context) = &self.browser_context {
            params["browserContextId"] = Value::String(context.clone());
        }
        self.cdp
            .call(None, "Browser.setDownloadBehavior", params, self.timeout())
            .await?;
        Ok(())
    }

    /// Cancel in-flight downloads tracked on this connection.
    async fn cancel_downloads(&self) -> E2eResult<usize> {
        let mut canceled = 0;
        for record in self.cdp.download_records() {
            if record.state != "inProgress" {
                continue;
            }
            let mut params = serde_json::json!({"guid":record.guid});
            if let Some(context) = &self.browser_context {
                params["browserContextId"] = Value::String(context.clone());
            }
            let done = self
                .cdp
                .call(None, "Browser.cancelDownload", params, self.timeout())
                .await;
            if done.is_ok() {
                canceled += 1;
            }
        }
        Ok(canceled)
    }

    async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        self.call(
            "Security.setIgnoreCertificateErrors",
            serde_json::json!({ "ignore": ignore }),
        )
        .await?;
        Ok(())
    }

    async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        crate::context_cookies::cookies(
            &crate::browser::Backend::Cdp(self.cdp.clone()),
            self.browser_context.as_deref(),
            self.timeout(),
        )
        .await
    }

    async fn set_cookie(&self, name: &str, value: &str, url: &str) -> E2eResult<()> {
        self.add_cookies(
            &[Cookie {
                name: name.into(),
                value: value.into(),
                domain: None,
                path: None,
                http_only: false,
                secure: false,
                same_site: None,
                expires: None,
            }],
            url,
        )
        .await
    }

    async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        crate::context_cookies::add_cookies(
            &crate::browser::Backend::Cdp(self.cdp.clone()),
            self.browser_context.as_deref(),
            self.timeout(),
            cookies,
            url,
        )
        .await
    }

    async fn clear_cookies(&self) -> E2eResult<()> {
        crate::context_cookies::clear_cookies(
            &crate::browser::Backend::Cdp(self.cdp.clone()),
            self.browser_context.as_deref(),
            self.timeout(),
        )
        .await
    }

    /// Currently held modifier bitmask.
    fn held_modifiers(&self) -> u8 {
        self.modifiers.lock().map(|held| *held).unwrap_or_default()
    }

    /// Button to report on moves (`left` while held, else `none`).
    fn held_button(&self) -> &'static str {
        if self.pressed.lock().map(|held| *held).unwrap_or_default() {
            "left"
        } else {
            "none"
        }
    }

    async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        let modifiers = self.held_modifiers();
        let button = self.held_button();
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseMoved", "x": x, "y": y, "modifiers": modifiers,
                "button": button,
            }),
        )
        .await?;
        Ok(())
    }

    async fn mouse_click_with(
        &self,
        x: f64,
        y: f64,
        options: &crate::page::MouseClickOptions,
    ) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        let modifiers = self.held_modifiers();
        let button = options.button.as_cdp();
        for click_count in 1..=options.click_count.max(1) {
            self.call(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mousePressed", "x": x, "y": y, "modifiers": modifiers,
                    "button": button, "clickCount": click_count,
                }),
            )
            .await?;
            if !options.delay.is_zero() {
                tokio::time::sleep(options.delay).await;
            }
            self.call(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": "mouseReleased", "x": x, "y": y, "modifiers": modifiers,
                    "button": button, "clickCount": click_count,
                }),
            )
            .await?;
        }
        Ok(())
    }

    async fn insert_text(&self, text: &str) -> E2eResult<()> {
        self.call("Input.insertText", serde_json::json!({ "text": text }))
            .await?;
        Ok(())
    }

    async fn press_key_with(
        &self,
        key: &str,
        options: &crate::page::KeyPressOptions,
    ) -> E2eResult<()> {
        let (windows_code, key_name, code) = key_definition(key);
        if key.chars().count() == 1 && windows_code == 0 {
            // Single-char input has no down/up pair; the delay is a no-op.
            self.call(
                "Input.dispatchKeyEvent",
                serde_json::json!({ "type": "char", "text": key }),
            )
            .await?;
            return Ok(());
        }
        // A transient press reports its own modifier bit without touching
        // the held tracker.
        let modifiers = self.held_modifiers() | modifier_bit(key);
        for kind in ["rawKeyDown", "keyUp"] {
            let mut params = serde_json::json!({
                "type": kind,
                "windowsVirtualKeyCode": windows_code,
                "key": key_name,
                "code": code,
                "modifiers": modifiers,
            });
            if kind == "rawKeyDown" {
                if key.chars().count() == 1 {
                    params["text"] = Value::String(key.to_string());
                }
                if key.chars().count() == 1 {
                    params["type"] = Value::String("keyDown".into());
                }
                self.call("Input.dispatchKeyEvent", params).await?;
                if !options.delay.is_zero() {
                    tokio::time::sleep(options.delay).await;
                }
            } else {
                self.call("Input.dispatchKeyEvent", params).await?;
            }
        }
        Ok(())
    }

    async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        let modifiers = self.held_modifiers();
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mousePressed", "x": x, "y": y, "modifiers": modifiers,
                "button": "left", "clickCount": 1,
            }),
        )
        .await?;
        if let Ok(mut pressed) = self.pressed.lock() {
            *pressed = true;
        }
        Ok(())
    }

    async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        self.mouse_move(x, y).await?;
        let modifiers = self.held_modifiers();
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseReleased", "x": x, "y": y, "modifiers": modifiers,
                "button": "left", "clickCount": 1,
            }),
        )
        .await?;
        if let Ok(mut pressed) = self.pressed.lock() {
            *pressed = false;
        }
        Ok(())
    }

    async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        self.mouse_move(from.0, from.1).await?;
        self.mouse_down(from.0, from.1).await?;
        for step in 1..=steps.max(1) {
            let t = f64::from(step) / f64::from(steps.max(1));
            self.mouse_move(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
                .await?;
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
        self.mouse_up(to.0, to.1).await
    }

    async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        let modifiers = self.held_modifiers();
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseWheel", "x": x, "y": y, "modifiers": modifiers,
                "deltaX": delta_x, "deltaY": delta_y,
            }),
        )
        .await?;
        Ok(())
    }

    async fn key_down(&self, key: &str) -> E2eResult<()> {
        let (windows_code, key_name, code) = key_definition(key);
        let bit = modifier_bit(key);
        let modifiers = if bit == 0 {
            self.held_modifiers()
        } else {
            self.modifiers
                .lock()
                .map(|mut held| {
                    *held |= bit;
                    *held
                })
                .unwrap_or(bit)
        };
        let mut params = serde_json::json!({
            "type": "rawKeyDown",
            "windowsVirtualKeyCode": windows_code,
            "key": key_name,
            "code": code,
            "modifiers": modifiers,
        });
        if key.chars().count() == 1 {
            params["text"] = Value::String(key.to_string());
        }
        self.call("Input.dispatchKeyEvent", params).await?;
        Ok(())
    }

    async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        for (kind, points) in [
            ("touchStart", serde_json::json!([{ "x": x, "y": y }])),
            ("touchEnd", serde_json::json!([])),
        ] {
            self.call(
                "Input.dispatchTouchEvent",
                serde_json::json!({ "type": kind, "touchPoints": points }),
            )
            .await?;
        }
        Ok(())
    }

    async fn key_up(&self, key: &str) -> E2eResult<()> {
        let (windows_code, key_name, code) = key_definition(key);
        // Keyup observes the released modifier cleared, like native UI events.
        let modifiers = self.held_modifiers() & !modifier_bit(key);
        self.call(
            "Input.dispatchKeyEvent",
            serde_json::json!({
                "type": "keyUp",
                "windowsVirtualKeyCode": windows_code,
                "key": key_name,
                "code": code,
                "modifiers": modifiers,
            }),
        )
        .await?;
        let bit = modifier_bit(key);
        if bit != 0 {
            if let Ok(mut held) = self.modifiers.lock() {
                *held &= !bit;
            }
        }
        Ok(())
    }

    async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        let mut params = serde_json::json!({ "permissions": permissions });
        if let Some(context) = &self.browser_context {
            params["browserContextId"] = Value::String(context.clone());
        }
        self.call("Browser.grantPermissions", params).await?;
        Ok(())
    }

    async fn clear_permissions(&self) -> E2eResult<()> {
        let mut params = serde_json::json!({});
        if let Some(context) = &self.browser_context {
            params["browserContextId"] = Value::String(context.clone());
        }
        self.call("Browser.resetPermissions", params).await?;
        Ok(())
    }

    async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.call(
            "Emulation.setGeolocationOverride",
            serde_json::json!({
                "latitude": latitude, "longitude": longitude, "accuracy": 100,
            }),
        )
        .await?;
        Ok(())
    }

    async fn clear_geolocation(&self) -> E2eResult<()> {
        self.call("Emulation.clearGeolocationOverride", Value::Null)
            .await?;
        Ok(())
    }

    async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        let (download, upload) = if offline { (0, 0) } else { (-1, -1) };
        self.call(
            "Network.emulateNetworkConditions",
            serde_json::json!({
                "offline": offline, "latency": 0,
                "downloadThroughput": download, "uploadThroughput": upload,
            }),
        )
        .await?;
        Ok(())
    }

    async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        let map: serde_json::Map<String, Value> = headers
            .iter()
            .map(|(name, value)| (name.to_string(), Value::String(value.to_string())))
            .collect();
        self.call(
            "Network.setExtraHTTPHeaders",
            serde_json::json!({ "headers": map }),
        )
        .await?;
        Ok(())
    }

    async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        self.call(
            "Emulation.setLocaleOverride",
            serde_json::json!({ "locale": locale }),
        )
        .await?;
        Ok(())
    }

    async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        self.call(
            "Emulation.setTimezoneOverride",
            serde_json::json!({ "timezoneId": timezone_id }),
        )
        .await?;
        Ok(())
    }

    async fn set_java_script_enabled(&self, enabled: bool) -> E2eResult<()> {
        self.call(
            "Emulation.setScriptExecutionDisabled",
            serde_json::json!({ "value": !enabled }),
        )
        .await?;
        Ok(())
    }

    async fn set_bypass_csp(&self, bypass: bool) -> E2eResult<()> {
        self.call(
            "Page.setBypassCSP",
            serde_json::json!({ "enabled": bypass }),
        )
        .await?;
        Ok(())
    }

    async fn set_downloads_allowed(&self, allowed: bool) -> E2eResult<()> {
        let behavior = if allowed { "default" } else { "deny" };
        let mut params = serde_json::json!({"behavior":behavior});
        if let Some(context) = &self.browser_context {
            params["browserContextId"] = Value::String(context.clone());
        }
        self.cdp
            .call(None, "Browser.setDownloadBehavior", params, self.timeout())
            .await?;
        Ok(())
    }

    async fn set_service_workers_blocked(&self, blocked: bool) -> E2eResult<()> {
        // CDP cannot block worker *registration*; bypassing is the closest
        // runtime lever (fetches skip workers entirely).
        self.call(
            "Network.setBypassServiceWorker",
            serde_json::json!({ "bypass": blocked }),
        )
        .await?;
        Ok(())
    }

    async fn clear_data_for_origin(&self, origin: &str, storage_types: &str) -> E2eResult<()> {
        self.call(
            "Storage.clearDataForOrigin",
            serde_json::json!({ "origin": origin, "storageTypes": storage_types }),
        )
        .await?;
        Ok(())
    }

    async fn start_routing(
        &self,
        runtime: Arc<crate::routing::RouteRuntime>,
        slot: std::sync::Weak<crate::routing::PumpSlot>,
    ) -> E2eResult<crate::routing::RoutePump> {
        let mut events = self.cdp.subscribe();
        let mut startup = FetchRoutingStartup {
            driver: self.clone(),
            committed: false,
        };
        if let Ok(mut shared) = self.fetch_auth.lock() {
            shared.routing_patterns = Some(vec![
                serde_json::json!({ "urlPattern": "*" }),
                serde_json::json!({ "urlPattern": "*", "requestStage": "Response" }),
            ]);
        }
        self.apply_fetch_config().await?;
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout();
        let lifecycle = self.lifecycle.clone();
        let context_lifecycle = self.context_cancellation.clone();
        let transport = self.cdp.disconnection();
        let driver = Driver::Cdp(self.clone());
        let stopped = crate::CancellationToken::new();
        let completion = crate::routing::PumpCompletion(stopped.clone());
        let handle = tokio::spawn(async move {
            let _completion = completion;
            let decided = Arc::new(Mutex::new(HashMap::<String, CachedRouteDecision>::new()));
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                if route_pump_idle(&runtime, &slot, &driver, || async {
                    while let Ok(event) = events.try_recv() {
                        if event.session.as_deref() == Some(&session)
                            && event.method == "Fetch.requestPaused"
                        {
                            let response = event.params.get("responseStatusCode").is_some()
                                || event.params.get("responseHeaders").is_some()
                                || event.params.get("responseErrorReason").is_some();
                            let method = if response {
                                "Fetch.continueResponse"
                            } else {
                                "Fetch.continueRequest"
                            };
                            if let Err(error) = cdp
                                .call(
                                    Some(&session),
                                    method,
                                    serde_json::json!({ "requestId": event.params["requestId"] }),
                                    Duration::from_millis(750),
                                )
                                .await
                            {
                                sink.record(
                                    "route_cleanup",
                                    format!("queued pause release failed: {error}"),
                                );
                            }
                        }
                    }
                })
                .await
                {
                    return;
                }
                let event = tokio::select! { biased;
                    _ = lifecycle.cancelled() => break,
                    _ = context_lifecycle.cancelled() => break,
                    _ = transport.cancelled() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => continue,
                    _ = runtime.changed.notified() => continue,
                    event = events.recv(), if tasks.len() < 256 => match event { Ok(event) => event, Err(error) => { sink.record("route", format!("interception event stream ended: {error}")); break; } },
                };
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                if matches!(
                    event.method.as_str(),
                    "Network.loadingFailed" | "Network.loadingFinished"
                ) {
                    let id = event.params["requestId"].as_str();
                    decided
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .retain(|_, pending| pending.network_id.as_deref() != id);
                    continue;
                }
                if event.method != "Fetch.requestPaused" {
                    continue;
                }
                let runtime = runtime.clone();
                let request_guard = runtime.request();
                let sink = sink.clone();
                let session = session.clone();
                let cdp = cdp.clone();
                let decided = decided.clone();
                tasks.spawn(async move {
                    let request_id = event.params["requestId"].clone();
                    let url = event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    let response_stage = event.params.get("responseStatusCode").is_some()
                        || event.params.get("responseHeaders").is_some()
                        || event.params.get("responseErrorReason").is_some();
                    let mut request_guard = Some(request_guard);
                    let mut handler = None;
                    let cached = if response_stage {
                        decided
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(request_id.as_str().unwrap_or_default())
                    } else {
                        None
                    };
                    let decided_action = if response_stage {
                        cached.as_ref().filter(|c| !c._request.forward.is_cancelled()).and_then(|c| c.action.clone())
                    } else {
                        let (post_data, body_state)=crate::route_options::cdp_body(&event.params["request"]);
                        let info = RouteInfo::new(url.clone(), event.params["request"]["method"].as_str().unwrap_or_default(), cdp_header_pairs(&event.params["request"]["headers"]), post_data).native(body_state);
                        let forward = request_guard.as_ref().unwrap().forward.clone();
                        let decision = runtime.decide(info, &sink, &forward);
                        tokio::pin!(decision);
                        let (action, active) = tokio::select! { biased;
                            _ = forward.cancelled() => {
                                decided.lock().unwrap_or_else(|e| e.into_inner()).insert(request_id.as_str().unwrap_or_default().to_string(), CachedRouteDecision { action: None, _handler: None, _request: request_guard.take().unwrap(), network_id: event.params["networkId"].as_str().map(str::to_string) });
                                let result = cdp.call(Some(&session), "Fetch.continueRequest", serde_json::json!({ "requestId": request_id }), timeout).await;
                                if result.is_err() { decided.lock().unwrap_or_else(|e| e.into_inner()).remove(request_id.as_str().unwrap_or_default()); }
                                // Preserve the callback and error policy, but its route is already released.
                                let _ = decision.await;
                                return;
                            },
                            result = &mut decision => result,
                        };
                        handler = active;
                        // Response interception outlives the request-stage RPC.
                        // Keep every native request owned until its response pause
                        // or terminal event, so idle removal cannot disable Fetch
                        // between the two stages (including unrelated traffic).
                        let edit = action.as_ref().filter(|action| matches!(action, RouteAction::ModifyResponse { .. })).cloned();
                        let mut pending = decided.lock().unwrap_or_else(|e| e.into_inner());
                        if pending.len() >= 4096 { pending.clear(); sink.record("route", "response decision history exceeded its bound".into()); }
                        pending.insert(request_id.as_str().unwrap_or_default().to_string(), CachedRouteDecision { action: edit.clone(), _handler: if edit.is_some() { handler.take() } else { None }, _request: request_guard.take().unwrap(), network_id: event.params["networkId"].as_str().map(str::to_string) });
                        drop(pending);
                        action
                    };
                    let action = decided_action.as_ref();
                    if response_stage {
                        answer_response_pause(
                            &cdp,
                            &session,
                            timeout,
                            &request_id,
                            &event.params,
                            action,
                        )
                        .await;
                        drop(cached);
                        return;
                    }
                    let is_override = matches!(action, Some(RouteAction::ContinueWith { .. }));
                    let (method, params) = match action {
                        Some(RouteAction::Abort) => (
                            "Fetch.failRequest",
                            serde_json::json!({
                                "requestId": request_id,
                                "errorReason": "Aborted",
                            }),
                        ),
                        Some(RouteAction::AbortWith(reason)) => (
                            "Fetch.failRequest",
                            serde_json::json!({
                                "requestId": request_id,
                                "errorReason": reason.as_cdp(),
                            }),
                        ),
                        Some(RouteAction::Fulfill {
                            status,
                            status_text,
                            headers,
                            body,
                            content_type,
                        }) => {
                            let mut response_headers: Vec<Value> = headers
                            .iter()
                            .map(
                                |(name, value)| serde_json::json!({ "name": name, "value": value }),
                            )
                            .collect();
                            if !content_type.is_empty()
                                && !headers
                                    .iter()
                                    .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                            {
                                response_headers.push(serde_json::json!({
                                    "name": "Content-Type",
                                    "value": content_type,
                                }));
                            }
                            let mut params = serde_json::json!({
                                "requestId": request_id,
                                "responseCode": status,
                                "body": base64_encode(body),
                                "responseHeaders": response_headers,
                            });
                            if !status_text.is_empty() {
                                params["responsePhrase"] = Value::String(status_text.clone());
                            }
                            ("Fetch.fulfillRequest", params)
                        }
                        Some(RouteAction::ContinueWith {
                            url,
                            method,
                            headers,
                            body,
                        }) => {
                            let mut params = serde_json::json!({ "requestId": request_id });
                            if let Some(url) = url {
                                params["url"] = Value::String(url.clone());
                            }
                            if let Some(method) = method {
                                params["method"] = Value::String(method.clone());
                            }
                            if let Some(headers) = headers {
                                params["headers"] = Value::Array(
                                headers
                                    .iter()
                                    .map(|(name, value)| {
                                        serde_json::json!({ "name": name, "value": value })
                                    })
                                    .collect(),
                            );
                            }
                            if let Some(body) = body {
                                params["postData"] = Value::String(base64_encode(body));
                            }
                            ("Fetch.continueRequest", params)
                        }
                        // Response edits apply at the response stage; the request
                        // must reach the server first.
                        Some(RouteAction::ModifyResponse { .. }) | None => (
                            "Fetch.continueRequest",
                            serde_json::json!({ "requestId": request_id }),
                        ),
                        _ => (
                            "Fetch.continueRequest",
                            serde_json::json!({ "requestId": request_id }),
                        ),
                    };
                    let supplied = event.params["networkId"].as_str().and_then(|id| {
                        request_id
                            .as_str()
                            .and_then(|pause_id| sink.routed_headers(pause_id, id, &url, action))
                    });
                    let result = cdp.call(Some(&session), method, params, timeout).await;
                    if let Err(error) = &result { sink.record("route", format!("native route decision failed: {error}")); }
                    if result.is_ok() {
                        if let Some(supplied) = supplied {
                            supplied.accept();
                        }
                    }
                    if result.is_err() && is_override {
                        // Rejected overrides must not hang the page: let it through.
                        let _ = cdp
                            .call(
                                Some(&session),
                                "Fetch.continueRequest",
                                serde_json::json!({ "requestId": event.params["requestId"] }),
                                timeout,
                            )
                            .await;
                    }
                    if result.is_err() {
                        decided
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .remove(request_id.as_str().unwrap_or_default());
                    }
                    drop(cached);
                    drop(handler);
                    drop(request_guard);
                });
            }
            tasks.abort_all();
            tasks.shutdown().await;
            decided.lock().unwrap_or_else(|e| e.into_inner()).clear();
            runtime.clear();
            let _ = tokio::time::timeout(Duration::from_millis(750), driver.stop_routing()).await;
        });
        startup.committed = true;
        Ok(crate::routing::RoutePump::new(
            handle.abort_handle(),
            stopped,
        ))
    }

    async fn stop_routing(&self) {
        if let Ok(mut shared) = self.fetch_auth.lock() {
            shared.routing_patterns = None;
        }
        // Auth-only mode keeps the domain enabled (empty patterns).
        let _ = self.apply_fetch_config().await;
    }

    /// Re-enable the Fetch domain from merged routing/auth state.
    async fn apply_fetch_config(&self) -> E2eResult<()> {
        let params = self
            .fetch_auth
            .lock()
            .map(|shared| {
                fetch_enable_params(shared.routing_patterns.as_deref(), shared.creds.is_some())
            })
            .unwrap_or(None);
        match params {
            Some(params) => self.call("Fetch.enable", params).await.map(|_| ()),
            None => self.call("Fetch.disable", Value::Null).await.map(|_| ()),
        }
    }

    /// Answer `Fetch.authRequired` with `creds` (`None` clears).
    async fn set_auth_credentials(&self, creds: Option<(String, String)>) -> E2eResult<()> {
        let spawn_pump = {
            let mut shared = self.fetch_auth.lock().unwrap_or_else(|e| e.into_inner());
            shared.creds = creds;
            match (&shared.creds, &shared.auth_task) {
                (Some(_), None) => true,
                (None, Some(task)) => {
                    task.abort();
                    shared.auth_task = None;
                    false
                }
                _ => false,
            }
        };
        if spawn_pump {
            let task = self.spawn_auth_pump();
            if let Ok(mut shared) = self.fetch_auth.lock() {
                shared.auth_task = Some(task);
            }
        }
        self.apply_fetch_config().await
    }

    /// Pump answering auth challenges (ignores `requestPaused`: the routing
    /// pump owns those, so the two never race a request).
    fn spawn_auth_pump(&self) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let timeout = self.timeout();
        let shared = Arc::clone(&self.fetch_auth);
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                if event.method == "Fetch.requestPaused" {
                    let auth_only = shared
                        .lock()
                        .map(|state| state.routing_patterns.is_none())
                        .unwrap_or(false);
                    if auth_only {
                        let method = if event.params.get("responseStatusCode").is_some() {
                            "Fetch.continueResponse"
                        } else {
                            "Fetch.continueRequest"
                        };
                        cdp.call(
                            Some(&session),
                            method,
                            serde_json::json!({"requestId":event.params["requestId"]}),
                            timeout,
                        )
                        .await
                        .ok();
                    }
                    continue;
                }
                if event.method != "Fetch.authRequired" {
                    continue;
                }
                let creds = shared
                    .lock()
                    .map(|state| state.creds.clone())
                    .unwrap_or(None);
                let Some((username, password)) = creds else {
                    continue;
                };
                let _ = cdp
                    .call(
                        Some(&session),
                        "Fetch.continueWithAuth",
                        serde_json::json!({
                            "requestId": event.params["requestId"],
                            "authChallengeResponse": {
                                "response": "ProvideCredentials",
                                "username": username,
                                "password": password,
                            },
                        }),
                        timeout,
                    )
                    .await;
            }
        })
        .abort_handle()
    }

    fn start_request_capture(&self) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let sink = self.sink.clone();
        let cdp = self.cdp.clone();
        let timeout = self.timeout();
        let lifecycle = self.lifecycle.clone();
        let context = self.context_cancellation.clone();
        let transport = self.cdp.disconnection();
        let generation = sink.begin_body_capture();
        // Construct before spawning so an abort before the first poll still finalizes pending state.
        let guard = BodyCaptureGuard {
            sink: sink.clone(),
            generation,
        };
        tokio::spawn(async move {
            let _guard=guard;
            let mut pending: HashMap<String, PendingRequest> = HashMap::new();
            let mut responded: std::collections::HashSet<String> = std::collections::HashSet::new();
            loop {
                let received=tokio::select!{biased;
                    _=lifecycle.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed),
                    _=context.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed),
                    _=transport.cancelled()=>Err(tokio::sync::broadcast::error::RecvError::Closed),
                    result=events.recv()=>result,
                };
                let event = match received {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                match event.method.as_str() {
                    "Network.requestWillBeSent" => {
                        let id = event.params["requestId"].as_str().unwrap_or_default();
                        // Redirect chains re-send; keep the first method/URL.
                        if !id.is_empty() && !pending.contains_key(id) {
                            let method = event.params["request"]["method"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let url = event.params["request"]["url"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let headers = cdp_header_pairs(&event.params["request"]["headers"]);
                            let post_data = event.params["request"]["postData"]
                                .as_str()
                                .map(str::to_string);
                            pending.insert(
                                id.to_string(),
                                PendingRequest {
                                    method,
                                    url,
                                    headers,
                                    post_data,
                                    started: tokio::time::Instant::now(),
                                    started_ms: now_ms(),
                                },
                            );
                        }
                    }
                    "Network.responseReceived" => {
                        let id = event.params["requestId"].as_str().unwrap_or_default();
                        if let Some(request) = pending.remove(id) {
                            let status = event.params["response"]["status"]
                                .as_u64()
                                .unwrap_or(0)
                                .min(u64::from(u16::MAX))
                                as u16;
                            let response = &event.params["response"];
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: response["url"].as_str().unwrap_or(&request.url).to_owned(),
                                status,
                                headers: request.headers,
                                post_data: request.post_data,
                                status_text: response["statusText"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                mime_type: response["mimeType"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                response_headers: cdp_header_pairs(&response["headers"]),
                                started_ms: Some(request.started_ms),
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                                request_id: Some(id.to_string()),
                                body: None,
                                body_truncated: false,
                            });
                            if responded.len() > 4096 {
                                responded.clear();
                            }
                            responded.insert(id.to_string());
                        }
                    }
                    "Network.loadingFinished" => {
                        let id = event.params["requestId"].as_str().unwrap_or_default();
                        if !id.is_empty() && responded.remove(id) {
                            let result=tokio::select!{biased;
                                reason=lifecycle.cancelled()=>Err(crate::BodyCaptureState::Unavailable(reason)),
                                reason=context.cancelled()=>Err(crate::BodyCaptureState::Unavailable(reason)),
                                reason=transport.cancelled()=>Err(crate::BodyCaptureState::Unavailable(reason)),
                                reply=cdp.call(Some(&session),"Network.getResponseBody",serde_json::json!({"requestId":id}),timeout)=>{
                                    match reply {
                                        Ok(reply)=>crate::captured_body::decode_body(&reply),
                                        Err(error)=>Err(crate::BodyCaptureState::Unavailable(error.to_string())),
                                    }
                                }
                            };
                            sink.publish_captured_body(id,generation,result);
                        }
                    }
                    "Network.loadingFailed" => {
                        let id = event.params["requestId"].as_str().unwrap_or_default();
                        responded.remove(id);
                        if let Some(request) = pending.remove(id) {
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status: 0,
                                headers: request.headers,
                                post_data: request.post_data,
                                status_text: String::new(),
                                mime_type: String::new(),
                                response_headers: Vec::new(),
                                started_ms: Some(request.started_ms),
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                                request_id: Some(id.to_string()),
                                body: None,
                                body_truncated: false,
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
        .abort_handle()
    }

    fn start_dialogs(
        &self,
        accept: bool,
        prompt_text: Option<String>,
        handler: Option<DialogHandler>,
    ) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout();
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session)
                    || event.method != "Page.javascriptDialogOpening"
                {
                    continue;
                }
                let info = DialogInfo {
                    dialog_type: event.params["type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    message: event.params["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                };
                let (accept, prompt_text) =
                    decide_dialog(handler.as_ref(), &info, accept, &prompt_text);
                sink.push_dialog(info);
                let mut params = serde_json::json!({ "accept": accept });
                if let Some(text) = &prompt_text {
                    params["promptText"] = Value::String(text.clone());
                }
                let _ = cdp
                    .call(
                        Some(&session),
                        "Page.handleJavaScriptDialog",
                        params,
                        timeout,
                    )
                    .await;
            }
        })
        .abort_handle()
    }

    async fn wait_for_load(&self, state: LoadState, timeout: Duration) -> E2eResult<()> {
        let mut events = self.cdp.subscribe();
        self.wait_for_load_with_events(state, timeout, &mut events)
            .await
    }

    async fn wait_for_load_with_events(
        &self,
        state: LoadState,
        timeout: Duration,
        events: &mut tokio::sync::broadcast::Receiver<CdpEvent>,
    ) -> E2eResult<()> {
        if state == LoadState::Commit {
            return Ok(());
        }
        let want = match state {
            LoadState::DomContentLoaded => "Page.domContentEventFired",
            LoadState::Load | LoadState::NetworkIdle => "Page.loadEventFired",
            LoadState::Commit => return Ok(()),
        };
        let deadline = crate::operation::Deadline::new(timeout);
        deadline
            .run(format!("wait for {want}"), async {
                if self.load_state_satisfied(state).await? {
                    return settle_quiet(&self.sink, state, deadline).await;
                }
                loop {
                    match deadline
                        .run(format!("wait for {want}"), async {
                            events.recv().await.map_err(|error| match error {
                                tokio::sync::broadcast::error::RecvError::Lagged(count) => {
                                    E2eError::Config(format!(
                                        "load event subscription lost {count} events"
                                    ))
                                }
                                tokio::sync::broadcast::error::RecvError::Closed => {
                                    E2eError::Disconnected("load event stream closed".into())
                                }
                            })
                        })
                        .await
                    {
                        Ok(event)
                            if event.session.as_deref() == Some(&self.session)
                                && event.method == want =>
                        {
                            return settle_quiet(&self.sink, state, deadline).await;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            if self.load_state_satisfied(state).await? {
                                return settle_quiet(&self.sink, state, deadline).await;
                            }
                            return Err(error);
                        }
                    }
                }
            })
            .await
    }

    async fn load_state_satisfied(&self, state: LoadState) -> E2eResult<bool> {
        match self.evaluate(crate::url_wait::DOCUMENT_OBSERVATION).await {
            Ok(observed) => Ok(crate::url_wait::document_ready(&observed, state)),
            Err(error) if error.is_control_flow() => Err(error),
            Err(_) => Ok(false),
        }
    }

    async fn close(&self) -> E2eResult<()> {
        let timeout = Duration::from_secs(5);
        crate::native_disposal::confirmed_close(
            async {
                let result = self
                    .cdp
                    .call(
                        None,
                        "Target.closeTarget",
                        serde_json::json!({"targetId":self.target}),
                        timeout,
                    )
                    .await?;
                if result["success"] != Value::Bool(true) {
                    return Err(E2eError::Cdp {
                        method: "Target.closeTarget".into(),
                        message: "native close was not acknowledged".into(),
                    });
                }
                Ok(())
            },
            || async {
                let result = self
                    .cdp
                    .call(None, "Target.getTargets", serde_json::json!({}), timeout)
                    .await?;
                let targets = result["targetInfos"].as_array().ok_or_else(|| {
                    E2eError::Config(
                        "Target.getTargets omitted targetInfos while confirming disposal".into(),
                    )
                })?;
                if targets.iter().any(|target| target["targetId"].as_str().is_none_or(str::is_empty)) {
                    return Err(E2eError::Config("Target.getTargets returned invalid target identity while confirming disposal".into()));
                }
                Ok(!targets
                    .iter()
                    .any(|target| target["targetId"].as_str() == Some(self.target.as_str())))
            },
            timeout,
        )
        .await
    }

    async fn raw(&self, method: &str, params: Value, timeout: Duration) -> E2eResult<Value> {
        self.cdp
            .call(Some(&self.session), method, params, timeout)
            .await
    }

    async fn start_frame_stream(
        &self,
        opts: &VideoOptions,
    ) -> E2eResult<(FrameStream, tokio::task::AbortHandle)> {
        self.start_screencast(opts).await?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let timeout = self.timeout();
        let started = tokio::time::Instant::now();
        let min_gap = Duration::from_millis((1000 / u64::from(opts.fps.max(1))).max(1));
        let handle = tokio::spawn(async move {
            let mut index = 0u64;
            let mut last_kept = None;
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session)
                    || event.method != "Page.screencastFrame"
                {
                    continue;
                }
                // Ack immediately so the encoder never stalls on backpressure.
                let frame_session = event.params["sessionId"].clone();
                let _ = cdp
                    .call(
                        Some(&session),
                        "Page.screencastFrameAck",
                        serde_json::json!({ "sessionId": frame_session }),
                        timeout,
                    )
                    .await;
                // Client-side throttle to target fps (the compositor emits
                // every repaint; sampling server-side drops sparse damage).
                let now = tokio::time::Instant::now();
                if let Some(last) = last_kept {
                    if now.duration_since(last) < min_gap {
                        continue;
                    }
                }
                let data = event.params["data"].as_str().unwrap_or_default();
                let Ok(bytes) = decode_base64(data) else {
                    continue;
                };
                last_kept = Some(now);
                let frame = VideoFrame {
                    index,
                    timestamp_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    data: bytes,
                };
                index += 1;
                if tx.send(frame).is_err() {
                    break;
                }
            }
        });
        Ok((FrameStream { receiver: rx }, handle.abort_handle()))
    }

    async fn stop_frame_stream(&self) {
        let _ = self.call("Page.stopScreencast", Value::Null).await;
    }

    async fn start_recording(&self, opts: &VideoOptions) -> E2eResult<RecordingState> {
        let spool = spool_dir(&opts.dir)?;
        self.start_screencast(opts).await?;
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let timeout = self.timeout();
        let started = std::time::Instant::now();
        let min_gap = Duration::from_millis((1000 / u64::from(opts.fps.max(1))).max(1));
        let pump_spool = spool.clone();
        let pump = tokio::spawn(async move {
            let mut index = 0u64;
            let mut last_kept = None;
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session)
                    || event.method != "Page.screencastFrame"
                {
                    continue;
                }
                let frame_session = event.params["sessionId"].clone();
                let _ = cdp
                    .call(
                        Some(&session),
                        "Page.screencastFrameAck",
                        serde_json::json!({ "sessionId": frame_session }),
                        timeout,
                    )
                    .await;
                let now = tokio::time::Instant::now();
                if let Some(last) = last_kept {
                    if now.duration_since(last) < min_gap {
                        continue;
                    }
                }
                let data = event.params["data"].as_str().unwrap_or_default();
                let Ok(bytes) = decode_base64(data) else {
                    continue;
                };
                last_kept = Some(now);
                let name = format!("frame-{index:06}.jpg");
                if std::fs::write(pump_spool.join(&name), bytes).is_err() {
                    break;
                }
                let t_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                let line = serde_json::json!({ "file": name, "t": t_ms }).to_string();
                if append_line(&pump_spool.join("manifest.jsonl"), &line).is_err() {
                    break;
                }
                index += 1;
            }
        });
        Ok(RecordingState::Cdp {
            spool,
            pump: pump.abort_handle(),
            fps: opts.fps,
            started,
        })
    }

    async fn stop_recording(
        &self,
        state: RecordingState,
        output: &std::path::Path,
    ) -> E2eResult<PathBuf> {
        let RecordingState::Cdp {
            spool,
            pump,
            fps,
            started,
        } = state
        else {
            return Err(E2eError::Config("recording/engine mismatch".to_string()));
        };
        pump.abort();
        self.stop_frame_stream().await;
        // Give the pump a beat to flush its last write before assembling.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let frames = read_manifest(&spool.join("manifest.jsonl"));
        let total_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let assembled = assemble_webm(
            &spool,
            &frames,
            total_ms,
            fps,
            output,
            Duration::from_secs(120),
        )
        .await;
        let _ = std::fs::remove_dir_all(&spool);
        assembled?;
        Ok(output.to_path_buf())
    }

    async fn cancel_recording(&self, state: RecordingState) {
        if let RecordingState::Cdp { spool, pump, .. } = state {
            pump.abort();
            self.stop_frame_stream().await;
            let _ = std::fs::remove_dir_all(&spool);
        }
    }

    async fn start_screencast(&self, opts: &VideoOptions) -> E2eResult<()> {
        // Always every frame: server-side sampling drops sparse damage to
        // zero frames (observed); throttle client-side instead.
        let mut params = serde_json::json!({
            "format": "jpeg",
            "quality": opts.quality,
            "everyNthFrame": 1,
        });
        if let Some(width) = opts.max_width {
            params["maxWidth"] = Value::from(width);
        }
        self.call("Page.startScreencast", params).await?;
        Ok(())
    }
}

/// Map a key name to (windowsVirtualKeyCode, key, code).
fn key_definition(key: &str) -> (u16, &str, &str) {
    match key {
        "Enter" => (13, "Enter", "Enter"),
        "Tab" => (9, "Tab", "Tab"),
        "Escape" | "Esc" => (27, "Escape", "Escape"),
        "Backspace" => (8, "Backspace", "Backspace"),
        "Delete" => (46, "Delete", "Delete"),
        "ArrowLeft" => (37, "ArrowLeft", "ArrowLeft"),
        "ArrowUp" => (38, "ArrowUp", "ArrowUp"),
        "ArrowRight" => (39, "ArrowRight", "ArrowRight"),
        "ArrowDown" => (40, "ArrowDown", "ArrowDown"),
        "Home" => (36, "Home", "Home"),
        "End" => (35, "End", "End"),
        "PageUp" => (33, "PageUp", "PageUp"),
        "PageDown" => (34, "PageDown", "PageDown"),
        "Shift" => (16, "Shift", "ShiftLeft"),
        "Control" | "Ctrl" => (17, "Control", "ControlLeft"),
        "Alt" => (18, "Alt", "AltLeft"),
        "Meta" | "Command" => (91, "Meta", "MetaLeft"),
        " " => (32, " ", "Space"),
        single if single.chars().count() == 1 => {
            let ch = single.chars().next().unwrap_or_default();
            if ch.is_ascii_alphabetic() {
                (ch.to_ascii_uppercase() as u16, key, key)
            } else {
                (0, key, key)
            }
        }
        _ => (0, key, key),
    }
}

/// CDP modifier bitmask for a key name (0 when not a modifier).
fn modifier_bit(key: &str) -> u8 {
    match key {
        "Alt" => 1,
        "Control" | "Ctrl" => 2,
        "Meta" | "Command" => 4,
        "Shift" => 8,
        _ => 0,
    }
}

// --- BiDi driver -------------------------------------------------------------

impl BidiDriver {
    fn context_param(&self) -> Value {
        Value::String(self.context.clone())
    }

    async fn call(&self, method: &str, mut params: Value) -> E2eResult<Value> {
        if params.is_null() {
            params = Value::Object(serde_json::Map::new());
        }
        if let Some(object) = params.as_object_mut() {
            object
                .entry("context")
                .or_insert_with(|| self.context_param());
        }
        self.bidi
            .call(method, params, self.timeout())
            .await
            .map_err(|error| match error {
                E2eError::Cdp { message, .. } => E2eError::Cdp {
                    method: method.to_string(),
                    message,
                },
                other => other,
            })
    }

    async fn navigate(&self, url: &str, wait: LoadState, timeout: Duration) -> E2eResult<()> {
        let wait_param = match wait {
            LoadState::Commit => "none",
            LoadState::DomContentLoaded => "interactive",
            LoadState::Load | LoadState::NetworkIdle => "complete",
        };
        self.bidi
            .call(
                "browsingContext.navigate",
                serde_json::json!({
                    "context": self.context,
                    "url": url,
                    "wait": wait_param,
                }),
                timeout,
            )
            .await
            .map_err(|error| E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            })?;
        self.sink.record("navigation", format!("goto {url}"));
        let deadline = crate::operation::Deadline::new(timeout);
        settle_quiet(&self.sink, wait, deadline).await
    }

    async fn reload(&self) -> E2eResult<()> {
        self.call(
            "browsingContext.reload",
            serde_json::json!({ "wait": "complete" }),
        )
        .await?;
        Ok(())
    }

    async fn traverse(&self, delta: i64) -> E2eResult<()> {
        match self
            .call(
                "browsingContext.traverseHistory",
                serde_json::json!({ "delta": delta }),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(E2eError::Cdp { message, .. })
                if message.contains("no such history entry")
                    || message.contains("History entry") =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    async fn evaluate(&self, expression: &str) -> E2eResult<Value> {
        self.evaluate_in_context(expression, self.context.as_str())
            .await
    }

    async fn frames(&self) -> E2eResult<Vec<FrameInfo>> {
        let tree = self
            .bidi
            .call(
                "browsingContext.getTree",
                serde_json::json!({ "root": self.context }),
                self.timeout(),
            )
            .await?;
        let mut out = Vec::new();
        if let Some(contexts) = tree.get("contexts").and_then(Value::as_array) {
            for context in contexts {
                collect_bidi_frames(context, &mut out);
            }
        }
        Ok(out)
    }

    async fn frame_evaluate(&self, frame_id: &str, expression: &str) -> E2eResult<Value> {
        self.evaluate_in_context(expression, frame_id).await
    }

    async fn frame_navigate(&self, frame_id: &str, url: &str) -> E2eResult<()> {
        self.bidi
            .call(
                "browsingContext.navigate",
                serde_json::json!({
                    "context": frame_id,
                    "url": url,
                    "wait": "complete",
                }),
                self.timeout(),
            )
            .await
            .map_err(|error| E2eError::Navigation {
                url: url.to_string(),
                message: error.to_string(),
            })?;
        self.sink.record("navigation", format!("frame goto {url}"));
        Ok(())
    }

    async fn request_gc(&self) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox garbage collection is not supported \
             (BiDi has no heap-profiler command)"
                .to_string(),
        ))
    }

    async fn evaluate_in_context(&self, expression: &str, context: &str) -> E2eResult<Value> {
        let result = self
            .bidi
            .call(
                "script.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "target": { "context": context },
                    "awaitPromise": true,
                }),
                self.timeout(),
            )
            .await
            .map_err(|error| match error {
                E2eError::Cdp { message, .. } => E2eError::Cdp {
                    method: "script.evaluate".to_string(),
                    message,
                },
                other => other,
            })?;
        if result.get("type").and_then(Value::as_str) == Some("exception") {
            let text = result["exceptionDetails"]["text"]
                .as_str()
                .unwrap_or("js exception");
            return Err(E2eError::Cdp {
                method: "script.evaluate".to_string(),
                message: format!("{text}: {expression}"),
            });
        }
        Ok(result
            .get("result")
            .map(remote_to_json)
            .unwrap_or(Value::Null))
    }

    async fn evaluate_handle(&self, expression: &str) -> E2eResult<JSHandle> {
        let result = self
            .bidi
            .call(
                "script.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "target": { "context": self.context },
                    "awaitPromise": true,
                    "resultOwnership": "root",
                }),
                self.timeout(),
            )
            .await
            .map_err(|error| script_call_error("script.evaluate", error))?;
        check_script_exception("script.evaluate", &result, expression)?;
        Ok(self.bidi_handle(&result["result"]))
    }

    /// Wrap a BiDi remote value: `handle` stays remote, plain values inline.
    fn bidi_handle(&self, remote: &Value) -> JSHandle {
        let remote_id = remote
            .get("handle")
            .and_then(Value::as_str)
            .map(str::to_string);
        let value = if remote_id.is_none() {
            Some(remote_to_json(remote))
        } else {
            None
        };
        JSHandle::new(Driver::Bidi(self.clone()), remote_id, value)
    }

    async fn call_script_function(&self, params: Value) -> E2eResult<Value> {
        self.bidi
            .call("script.callFunction", params, self.timeout())
            .await
            .map_err(|error| script_call_error("script.callFunction", error))
    }

    async fn handle_json_value(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
    ) -> E2eResult<Value> {
        if let Some(value) = value {
            return Ok(value);
        }
        let Some(id) = remote_id else {
            return Ok(Value::Null);
        };
        // No `resultOwnership`: the value comes back serialized by value.
        let result = self
            .call_script_function(serde_json::json!({
                "functionDeclaration": "(value) => value",
                "arguments": [{ "handle": id }],
                "target": { "context": self.context },
                "awaitPromise": true,
            }))
            .await?;
        check_script_exception("script.callFunction", &result, "json_value")?;
        Ok(result
            .get("result")
            .map(remote_to_json)
            .unwrap_or(Value::Null))
    }

    async fn handle_get_property(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        name: &str,
    ) -> E2eResult<JSHandle> {
        // Inlined primitives resolve locally (JSON indexing).
        if let Some(value) = value {
            let prop = value.get(name).cloned().unwrap_or(Value::Null);
            return Ok(JSHandle::new(Driver::Bidi(self.clone()), None, Some(prop)));
        }
        let Some(id) = remote_id else {
            return Err(E2eError::Cdp {
                method: "script.callFunction".to_string(),
                message: "get_property needs a remote handle or a value".to_string(),
            });
        };
        let result = self
            .call_script_function(serde_json::json!({
                "functionDeclaration": "function(name) { return this[name]; }",
                "this": { "handle": id },
                "arguments": [{ "type": "string", "value": name }],
                "target": { "context": self.context },
                "awaitPromise": true,
                "resultOwnership": "root",
            }))
            .await?;
        check_script_exception("script.callFunction", &result, name)?;
        Ok(self.bidi_handle(&result["result"]))
    }

    async fn handle_evaluate(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<Value> {
        // Inlined primitives apply the function to a JSON literal in-page.
        if let Some(value) = value {
            let literal = serde_json::to_string(&value)?;
            return self.evaluate(&format!("({function})({literal})")).await;
        }
        let Some(id) = remote_id else {
            return Err(E2eError::Cdp {
                method: "script.callFunction".to_string(),
                message: "handle evaluate needs a remote handle or a value".to_string(),
            });
        };
        let result = self
            .call_script_function(serde_json::json!({
                "functionDeclaration": function,
                "arguments": [{ "handle": id }],
                "target": { "context": self.context },
                "awaitPromise": true,
            }))
            .await?;
        check_script_exception("script.callFunction", &result, function)?;
        Ok(result
            .get("result")
            .map(remote_to_json)
            .unwrap_or(Value::Null))
    }

    async fn handle_evaluate_handle(
        &self,
        remote_id: Option<&str>,
        value: Option<Value>,
        function: &str,
    ) -> E2eResult<JSHandle> {
        if let Some(value) = value {
            return self
                .evaluate_handle(&format!("({function})({})", serde_json::to_string(&value)?))
                .await;
        }
        let id = remote_id.ok_or_else(|| E2eError::Config("remote handle is missing".into()))?;
        let result = self.bidi.call("script.callFunction", serde_json::json!({"functionDeclaration":function,"target":{"context":self.context},"arguments":[{"handle":id}],"awaitPromise":true,"resultOwnership":"root"}), self.timeout()).await?;
        check_script_exception("script.callFunction", &result, function)?;
        Ok(self.bidi_handle(&result["result"]))
    }

    async fn handle_dispose(&self, remote_id: Option<&str>) -> E2eResult<()> {
        let Some(id) = remote_id else {
            return Ok(());
        };
        self.bidi
            .call(
                "script.disown",
                serde_json::json!({
                    "target": { "context": self.context },
                    "handles": [id],
                }),
                self.timeout(),
            )
            .await
            .map_err(|error| script_call_error("script.disown", error))?;
        Ok(())
    }

    async fn bring_to_front(&self) -> E2eResult<()> {
        self.call("browsingContext.activate", Value::Null).await?;
        Ok(())
    }

    async fn set_content(&self, html: &str) -> E2eResult<()> {
        let literal = serde_json::to_string(html)?;
        self.evaluate(&format!(
            "(() => {{ document.open(); document.write({literal}); document.close(); }})()"
        ))
        .await?;
        Ok(())
    }

    async fn print_pdf(&self) -> E2eResult<Vec<u8>> {
        let pdf = self.call("browsingContext.print", Value::Null).await?;
        decode_shot(&pdf)
    }

    async fn set_viewport(&self, width: u32, height: u32) -> E2eResult<()> {
        self.call(
            "browsingContext.setViewport",
            serde_json::json!({ "viewport": { "width": width, "height": height } }),
        )
        .await?;
        Ok(())
    }

    async fn set_user_agent(&self, _user_agent: &str) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox user agent is launch-wide (BiDi has no per-page override); \
             set LaunchOptions::user_agent or [e2e] user_agent"
                .to_string(),
        ))
    }

    async fn set_download_dir(&self, _dir: &Path) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox download dir is launch-wide (BiDi has no per-page override); \
             set LaunchOptions::download_dir or [e2e] download_dir"
                .to_string(),
        ))
    }

    async fn cancel_downloads(&self) -> E2eResult<usize> {
        Err(E2eError::Config(
            "firefox download cancel is not supported \
             (BiDi has no download-cancel command)"
                .to_string(),
        ))
    }

    async fn emulate_device(&self, _device: crate::page::DeviceDescriptor) -> E2eResult<()> {
        Err(E2eError::Config(
            "emulate_device is only supported on Chromium \
             (Firefox BiDi has no device emulation)"
                .to_string(),
        ))
    }

    async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        if !ignore || self.insecure_certs {
            return Ok(());
        }
        Err(E2eError::Config(
            "firefox certificate errors are accepted via the session capability; \
             set LaunchOptions::ignore_https_errors or [e2e] ignore_https_errors=true \
             before launching"
                .to_string(),
        ))
    }

    async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        crate::context_cookies::cookies(
            &crate::browser::Backend::Bidi {
                conn: self.bidi.clone(),
                insecure_certs: self.insecure_certs,
            },
            self.user_context.as_deref(),
            self.timeout(),
        )
        .await
    }

    async fn set_cookie(&self, name: &str, value: &str, url: &str) -> E2eResult<()> {
        let mut cookie = serde_json::json!({
            "name": name,
            "value": { "type": "string", "value": value },
            "path": "/",
        });
        if let Some(domain) = url_host(url) {
            cookie["domain"] = Value::String(domain);
        }
        self.bidi
            .call(
                "storage.setCookie",
                serde_json::json!({
                    "cookie": cookie,
                    "partition": { "type": "storageKey", "userContext": self.user_context.as_deref().unwrap_or("default") },
                }),
                self.timeout(),
            )
            .await?;
        Ok(())
    }

    async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        crate::context_cookies::add_cookies(
            &crate::browser::Backend::Bidi {
                conn: self.bidi.clone(),
                insecure_certs: self.insecure_certs,
            },
            self.user_context.as_deref(),
            self.timeout(),
            cookies,
            url,
        )
        .await
    }

    async fn clear_cookies(&self) -> E2eResult<()> {
        crate::context_cookies::clear_cookies(
            &crate::browser::Backend::Bidi {
                conn: self.bidi.clone(),
                insecure_certs: self.insecure_certs,
            },
            self.user_context.as_deref(),
            self.timeout(),
        )
        .await
    }

    async fn perform(&self, actions: Value) -> E2eResult<()> {
        self.bidi
            .call(
                "input.performActions",
                serde_json::json!({ "context": self.context, "actions": actions }),
                self.timeout(),
            )
            .await?;
        Ok(())
    }

    async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-mouse",
            "parameters": { "pointerType": "mouse" },
            "actions": [{ "type": "pointerMove", "x": x, "y": y }],
        }]))
        .await
    }

    async fn mouse_click_with(
        &self,
        x: f64,
        y: f64,
        options: &crate::MouseClickOptions,
    ) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        // Keep delay on the Rust side so caller cancellation can release
        // input promptly instead of waiting for a long native pause action.
        for _ in 0..options.click_count.max(1) {
            for down in [true, false] {
                self.perform(serde_json::json!([{
                    "type":"pointer","id":"ferrite-mouse","parameters":{"pointerType":"mouse"},
                    "actions":[{"type":if down {"pointerDown"} else {"pointerUp"},"button":options.button.as_bidi()}]
                }])).await?;
                if down && !options.delay.is_zero() {
                    tokio::time::sleep(options.delay).await;
                }
            }
        }
        Ok(())
    }

    async fn insert_text(&self, text: &str) -> E2eResult<()> {
        let mut actions = Vec::new();
        for ch in text.chars() {
            // BiDi types through key actions; newlines are Enter presses.
            let value = if ch == '\n' {
                BIDI_ENTER.to_string()
            } else {
                ch.to_string()
            };
            actions.push(serde_json::json!({ "type": "keyDown", "value": value }));
            actions.push(serde_json::json!({ "type": "keyUp", "value": value }));
        }
        if actions.is_empty() {
            return Ok(());
        }
        self.perform(serde_json::json!([{
            "type": "key", "id": "ferrite-keyboard", "actions": actions,
        }]))
        .await
    }

    async fn press_key_with(
        &self,
        key: &str,
        options: &crate::page::KeyPressOptions,
    ) -> E2eResult<()> {
        let value = bidi_key_value(key);
        let mut actions = vec![serde_json::json!({ "type": "keyDown", "value": value.clone() })];
        if !options.delay.is_zero() {
            actions.push(serde_json::json!({
                "type": "pause",
                "duration": options.delay.as_millis().min(u128::from(u64::MAX)) as u64,
            }));
        }
        actions.push(serde_json::json!({ "type": "keyUp", "value": value }));
        self.perform(serde_json::json!([{
            "type": "key", "id": "ferrite-keyboard",
            "actions": actions,
        }]))
        .await
    }

    async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-mouse",
            "parameters": { "pointerType": "mouse" },
            "actions": [
                { "type": "pointerMove", "x": x, "y": y },
                { "type": "pointerDown", "button": 0 },
            ],
        }]))
        .await
    }

    async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-mouse",
            "parameters": { "pointerType": "mouse" },
            "actions": [
                { "type": "pointerMove", "x": x, "y": y },
                { "type": "pointerUp", "button": 0 },
            ],
        }]))
        .await
    }

    async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        self.mouse_down(from.0, from.1).await?;
        for step in 1..=steps.max(1) {
            let t = f64::from(step) / f64::from(steps.max(1));
            self.mouse_move(
                (from.0 + (to.0 - from.0) * t).round(),
                (from.1 + (to.1 - from.1) * t).round(),
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(16)).await;
        }
        self.mouse_up(to.0, to.1).await
    }

    async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.perform(serde_json::json!([{
            "type": "wheel", "id": "ferrite-wheel",
            "actions": [{
                "type": "scroll", "x": x, "y": y,
                "deltaX": delta_x as i64, "deltaY": delta_y as i64,
            }],
        }]))
        .await
    }

    async fn key_down(&self, key: &str) -> E2eResult<()> {
        let value = bidi_key_value(key);
        *self.modifiers.lock().unwrap_or_else(|e| e.into_inner()) |= modifier_bit(key);
        self.perform(serde_json::json!([{
            "type": "key", "id": "ferrite-keyboard",
            "actions": [{ "type": "keyDown", "value": value }],
        }]))
        .await
    }

    async fn key_up(&self, key: &str) -> E2eResult<()> {
        let value = bidi_key_value(key);
        self.perform(serde_json::json!([{
            "type": "key", "id": "ferrite-keyboard",
            "actions": [{ "type": "keyUp", "value": value }],
        }]))
        .await?;
        *self.modifiers.lock().unwrap_or_else(|e| e.into_inner()) &= !modifier_bit(key);
        Ok(())
    }

    async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-touch",
            "parameters": { "pointerType": "touch" },
            "actions": [
                { "type": "pointerMove", "x": x, "y": y },
                { "type": "pointerDown", "button": 0 },
                { "type": "pointerUp", "button": 0 },
            ],
        }]))
        .await
    }

    async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        let origin = self
            .evaluate("location.origin")
            .await?
            .as_str()
            .filter(|origin| origin.starts_with("http"))
            .map(str::to_string);
        let Some(origin) = origin else {
            // Firefox (even 156) rejects origin-less grants, so fail loudly
            // instead of sending a call the engine refuses.
            return Err(E2eError::Config(
                "firefox permission grants need an http(s) page \
                 (navigate first, then grant)"
                    .to_string(),
            ));
        };
        for name in permissions {
            let mut params = serde_json::json!({
                "descriptor": { "name": name },
                "state": "granted",
                "origin": origin,
            });
            // Origin-only grants land in the default user context; pages in
            // a dedicated context would never see them.
            if let Some(user_context) = &self.user_context {
                params["userContext"] = Value::String(user_context.clone());
            }
            self.call("permissions.setPermission", params).await?;
        }
        Ok(())
    }

    async fn clear_permissions(&self) -> E2eResult<()> {
        let origin = self
            .evaluate("location.origin")
            .await?
            .as_str()
            .filter(|origin| origin.starts_with("http"))
            .map(str::to_string);
        // Without an origin there is nothing origin-scoped to reset.
        let Some(origin) = origin else { return Ok(()) };
        for name in [
            "geolocation",
            "notifications",
            "clipboard-read",
            "clipboard-write",
            "camera",
            "microphone",
        ] {
            let mut params = serde_json::json!({
                "descriptor": { "name": name },
                "state": "prompt",
                "origin": origin,
            });
            if let Some(user_context) = &self.user_context {
                params["userContext"] = Value::String(user_context.clone());
            }
            // Best-effort: unknown descriptors fail per-name, never the reset.
            let _ = self.call("permissions.setPermission", params).await;
        }
        Ok(())
    }

    async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.call(
            "emulation.setGeolocationOverride",
            serde_json::json!({
                "coordinates": { "latitude": latitude, "longitude": longitude },
                "contexts": [self.context.clone()],
            }),
        )
        .await
        .map_err(|error| {
            if is_unsupported_command(&error) {
                E2eError::Config(
                    "firefox geolocation override needs a newer build \
                     (BiDi emulation.setGeolocationOverride is unknown)"
                        .to_string(),
                )
            } else {
                error
            }
        })?;
        Ok(())
    }

    async fn clear_geolocation(&self) -> E2eResult<()> {
        // BiDi clears via explicit null coordinates (contexts-only is
        // rejected: "Expected coordinates to be an object").
        let result = self
            .call(
                "emulation.setGeolocationOverride",
                serde_json::json!({
                    "coordinates": null,
                    "contexts": [self.context.clone()],
                }),
            )
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unsupported_command(&error) => Ok(()),
            Err(error) => Err(error),
        }
    }

    async fn set_offline(&self, _offline: bool) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox offline emulation is not supported \
             (BiDi has no network-conditions override)"
                .to_string(),
        ))
    }

    async fn set_extra_http_headers(&self, _headers: &[(&str, &str)]) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox extra HTTP headers are not supported \
             (BiDi has no global header override)"
                .to_string(),
        ))
    }

    async fn set_locale(&self, _locale: &str) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox locale override is not supported \
             (BiDi has no locale emulation)"
                .to_string(),
        ))
    }

    async fn set_timezone(&self, _timezone_id: &str) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox timezone override is not supported \
             (BiDi has no timezone emulation)"
                .to_string(),
        ))
    }

    async fn set_java_script_enabled(&self, _enabled: bool) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox JavaScript toggle is not supported \
             (BiDi has no script-execution override)"
                .to_string(),
        ))
    }

    async fn set_bypass_csp(&self, _bypass: bool) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox CSP bypass is not supported \
             (BiDi has no CSP override)"
                .to_string(),
        ))
    }

    async fn set_downloads_allowed(&self, _allowed: bool) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox download allow/deny is not supported \
             (BiDi has no download-behavior override)"
                .to_string(),
        ))
    }

    async fn set_service_workers_blocked(&self, _blocked: bool) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox service-worker blocking is not supported \
             (BiDi has no worker override)"
                .to_string(),
        ))
    }

    async fn clear_data_for_origin(&self, _origin: &str, _storage_types: &str) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox per-origin storage clear is not supported \
             (BiDi has no storage-clear command)"
                .to_string(),
        ))
    }

    async fn set_auth_credentials(&self, creds: Option<(String, String)>) -> E2eResult<()> {
        if creds.is_none() {
            return Ok(());
        }
        Err(E2eError::Config(
            "firefox HTTP auth challenges are not supported \
             (BiDi has no auth-challenge response)"
                .to_string(),
        ))
    }

    async fn start_routing(
        &self,
        runtime: Arc<crate::routing::RouteRuntime>,
        slot: std::sync::Weak<crate::routing::PumpSlot>,
    ) -> E2eResult<crate::routing::RoutePump> {
        let mut events = self.bidi.subscribe();
        let intercept = self
            .bidi
            .add_intercept(
                serde_json::json!({
                    "phases": ["beforeRequestSent"],
                    "contexts": [self.context.clone()],
                    "urlPatterns": [{ "type": "pattern" }],
                }),
                self.timeout(),
                Arc::downgrade(&self.sink.frame_events),
            )
            .await?;
        *self.intercept.lock().unwrap_or_else(|e| e.into_inner()) = Some(intercept);
        let context = self.context.clone();
        let bidi = self.bidi.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout();
        let lifecycle = self.lifecycle.clone();
        let context_lifecycle = self.context_cancellation.clone();
        let transport = self.bidi.disconnection();
        let driver = Driver::Bidi(self.clone());
        let stopped = crate::CancellationToken::new();
        let completion = crate::routing::PumpCompletion(stopped.clone());
        let handle = tokio::spawn(async move {
            let _completion = completion;
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                if route_pump_idle(&runtime, &slot, &driver, || async {
                    // removeIntercept does not release Firefox requests already
                    // blocked before its acknowledgement. Drain while registration
                    // remains locked so this cannot consume a new intercept's calls.
                    while let Ok(event) = events.try_recv() {
                        if event.context() == Some(context.as_str()) && event.method == "network.beforeRequestSent" && event.params["isBlocked"].as_bool() == Some(true) {
                            let _ = bidi.call("network.continueRequest", serde_json::json!({ "request": event.params["request"]["request"] }), Duration::from_millis(750)).await;
                        }
                    }
                }).await {
                    return;
                }
                let event = tokio::select! { biased;
                    _ = lifecycle.cancelled() => break,
                    _ = context_lifecycle.cancelled() => break,
                    _ = transport.cancelled() => break,
                    Some(_) = tasks.join_next(), if !tasks.is_empty() => continue,
                    _ = runtime.changed.notified() => continue,
                    event = events.recv(), if tasks.len() < 256 => match event { Ok(event) => event, Err(error) => { sink.record("route", format!("interception event stream ended: {error}")); break; } },
                };
                if event.context() != Some(context.as_str())
                    || event.method != "network.beforeRequestSent"
                    || event.params.get("isBlocked").and_then(Value::as_bool) != Some(true)
                {
                    continue;
                }
                let runtime = runtime.clone();
                let request_guard = runtime.request();
                let sink = sink.clone();
                let bidi = bidi.clone();
                tasks.spawn(async move {
                    let _request_guard = request_guard;
                    let request = event.params["request"]["request"].clone();
                    let url = event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    let info = RouteInfo::new(url.clone(), event.params["request"]["method"].as_str().unwrap_or_default(), bidi_header_pairs(&event.params["request"]["headers"]), None).native(crate::route_options::bidi_body(&event.params["request"]));
                    let forward = _request_guard.forward.clone();
                    let decision = runtime.decide(info, &sink, &forward);
                    tokio::pin!(decision);
                    let (mut decided_action, _handler) = tokio::select! { biased;
                        _ = forward.cancelled() => {
                            let _ = bidi.call("network.continueRequest", serde_json::json!({ "request": request }), timeout).await;
                            let _ = decision.await;
                            return;
                        },
                        result = &mut decision => result,
                    };
                    if matches!(
                        decided_action,
                        Some(RouteAction::ModifyResponse { .. })
                            | Some(RouteAction::ContinueWith { url: Some(_), .. })
                    ) {
                        sink.record(
                            "route",
                            "handler decision not supported on Firefox, aborting".into(),
                        );
                        decided_action = Some(RouteAction::Abort);
                    }
                    let action = decided_action.as_ref();
                    let is_override = matches!(action, Some(RouteAction::ContinueWith { .. }));
                    let (method, params) = match action {
                        Some(RouteAction::Abort) | Some(RouteAction::AbortWith(_)) => (
                            "network.failRequest",
                            serde_json::json!({ "request": request }),
                        ),
                        Some(RouteAction::Fulfill {
                            status,
                            status_text,
                            headers,
                            body,
                            content_type,
                        }) => {
                            let mut all: Vec<(String, String)> = headers.clone();
                            if !content_type.is_empty()
                                && !all
                                    .iter()
                                    .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
                            {
                                all.push(("Content-Type".to_string(), content_type.clone()));
                            }
                            let mut params = serde_json::json!({
                                "request": request,
                                "statusCode": status,
                                "headers": all.iter().map(|(name, value)| {
                                    serde_json::json!({
                                        "name": name,
                                        "value": { "type": "string", "value": value },
                                    })
                                }).collect::<Vec<_>>(),
                                "body": { "type": "base64", "value": base64_encode(body) },
                            });
                            if !status_text.is_empty() {
                                params["reasonPhrase"] = Value::String(status_text.clone());
                            }
                            ("network.provideResponse", params)
                        }
                        Some(RouteAction::ContinueWith {
                            url,
                            method,
                            headers,
                            body,
                        }) => {
                            let mut params = serde_json::json!({ "request": request });
                            if let Some(url) = url {
                                params["url"] = Value::String(url.clone());
                            }
                            if let Some(method) = method {
                                params["method"] = Value::String(method.clone());
                            }
                            // Firefox keeps a stale Content-Length when only the body
                            // is replaced, truncating the server-side read; carry the
                            // right length (over the user's set, else the original's).
                            let effective = match (headers, body) {
                                (Some(user), Some(replacement)) => {
                                    Some(with_content_length(user.clone(), replacement.len()))
                                }
                                (None, Some(replacement)) => Some(with_content_length(
                                    bidi_header_pairs(&event.params["request"]["headers"]),
                                    replacement.len(),
                                )),
                                (Some(user), None) => Some(user.clone()),
                                (None, None) => None,
                            };
                            if let Some(list) = effective {
                                params["headers"] = Value::Array(
                                    list.iter()
                                        .map(|(name, value)| {
                                            serde_json::json!({
                                                "name": name,
                                                "value": { "type": "string", "value": value },
                                            })
                                        })
                                        .collect(),
                                );
                            }
                            if let Some(body) = body {
                                params["body"] = serde_json::json!({
                                    "type": "base64",
                                    "value": base64_encode(body),
                                });
                            }
                            ("network.continueRequest", params)
                        }
                        _ => (
                            "network.continueRequest",
                            serde_json::json!({ "request": request }),
                        ),
                    };
                    let supplied = request.as_str().and_then(|id| {
                        sink.routed_headers(
                            &format!(
                                "bidi:{id}:{}",
                                event.params["redirectCount"].as_u64().unwrap_or(0)
                            ),
                            id,
                            &url,
                            action,
                        )
                    });
                    let result = bidi.call(method, params, timeout).await;
                    if let Err(error) = &result { sink.record("route", format!("native route decision failed: {error}")); }
                    if result.is_ok() {
                        if let Some(supplied) = supplied {
                            supplied.accept();
                        }
                    }
                    if result.is_err() && is_override {
                        // Rejected overrides must not hang the page: let it through.
                        let _ = bidi
                        .call(
                            "network.continueRequest",
                            serde_json::json!({ "request": event.params["request"]["request"] }),
                            timeout,
                        )
                        .await;
                    }
                });
            }
            tasks.abort_all();
            tasks.shutdown().await;
            runtime.clear();
            let _ = tokio::time::timeout(Duration::from_millis(750), driver.stop_routing()).await;
        });
        Ok(crate::routing::RoutePump::new(
            handle.abort_handle(),
            stopped,
        ))
    }

    async fn stop_routing(&self) {
        let intercept = self
            .intercept
            .lock()
            .map(|mut slot| slot.take())
            .unwrap_or(None);
        if let Some(intercept) = intercept {
            let _ = self
                .bidi
                .call(
                    "network.removeIntercept",
                    serde_json::json!({ "intercept": intercept }),
                    Duration::from_secs(5),
                )
                .await;
        }
    }

    fn start_request_capture(&self) -> tokio::task::AbortHandle {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let sink = self.sink.clone();
        tokio::spawn(async move {
            let mut pending: HashMap<String, PendingRequest> = HashMap::new();
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.context() != Some(context.as_str()) {
                    continue;
                }
                match event.method.as_str() {
                    "network.beforeRequestSent" => {
                        let id = event.params["request"]["request"]
                            .as_str()
                            .unwrap_or_default();
                        if !id.is_empty() && !pending.contains_key(id) {
                            let method = event.params["request"]["method"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let url = event.params["request"]["url"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let headers = bidi_header_pairs(&event.params["request"]["headers"]);
                            pending.insert(
                                id.to_string(),
                                PendingRequest {
                                    method,
                                    url,
                                    headers,
                                    post_data: None,
                                    started: tokio::time::Instant::now(),
                                    started_ms: now_ms(),
                                },
                            );
                        }
                    }
                    "network.responseCompleted" => {
                        let id = event.params["request"]["request"]
                            .as_str()
                            .unwrap_or_default();
                        if let Some(request) = pending.remove(id) {
                            let status = event.params["response"]["status"]
                                .as_u64()
                                .unwrap_or(0)
                                .min(u64::from(u16::MAX))
                                as u16;
                            let response = &event.params["response"];
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status,
                                headers: request.headers,
                                post_data: request.post_data,
                                status_text: response["statusText"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                mime_type: response["mimeType"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                response_headers: bidi_header_pairs(&response["headers"]),
                                started_ms: Some(request.started_ms),
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                                request_id: Some(id.to_string()),
                                body: None,
                                body_truncated: false,
                            });
                        }
                    }
                    "network.fetchError" => {
                        let id = event.params["request"]["request"]
                            .as_str()
                            .unwrap_or_default();
                        if let Some(request) = pending.remove(id) {
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status: 0,
                                headers: request.headers,
                                post_data: request.post_data,
                                status_text: String::new(),
                                mime_type: String::new(),
                                response_headers: Vec::new(),
                                started_ms: Some(request.started_ms),
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                                request_id: Some(id.to_string()),
                                body: None,
                                body_truncated: false,
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
        .abort_handle()
    }

    fn start_dialogs(
        &self,
        accept: bool,
        prompt_text: Option<String>,
        handler: Option<DialogHandler>,
    ) -> tokio::task::AbortHandle {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let bidi = self.bidi.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout();
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.context() != Some(context.as_str())
                    || event.method != "browsingContext.userPromptOpened"
                {
                    continue;
                }
                let info = DialogInfo {
                    dialog_type: event.params["type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    message: event.params["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                };
                let (accept, prompt_text) =
                    decide_dialog(handler.as_ref(), &info, accept, &prompt_text);
                sink.push_dialog(info);
                let mut params = serde_json::json!({ "context": context, "accept": accept });
                if let Some(text) = &prompt_text {
                    params["userText"] = Value::String(text.clone());
                }
                let _ = bidi
                    .call("browsingContext.handleUserPrompt", params, timeout)
                    .await;
            }
        })
        .abort_handle()
    }

    async fn wait_for_load(&self, state: LoadState, timeout: Duration) -> E2eResult<()> {
        if state == LoadState::Commit {
            return Ok(());
        }
        let deadline = crate::operation::Deadline::new(timeout);
        deadline
            .run(format!("wait for {state:?}"), async {
                loop {
                    if self.load_state_satisfied(state).await? {
                        return settle_quiet(&self.sink, state, deadline).await;
                    }
                    if deadline.expired() {
                        return Err(E2eError::Timeout(
                            timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                            format!("wait for {state:?}"),
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
    }

    async fn load_state_satisfied(&self, state: LoadState) -> E2eResult<bool> {
        match self.evaluate(crate::url_wait::DOCUMENT_OBSERVATION).await {
            Ok(observed) => Ok(crate::url_wait::document_ready(&observed, state)),
            Err(error) if error.is_control_flow() => Err(error),
            Err(_) => Ok(false),
        }
    }

    async fn close(&self) -> E2eResult<()> {
        let timeout = Duration::from_secs(5);
        crate::native_disposal::confirmed_close(
            async {
                self.bidi
                    .call(
                        "browsingContext.close",
                        serde_json::json!({"context":self.context}),
                        timeout,
                    )
                    .await?;
                Ok(())
            },
            || async {
                let result = self
                    .bidi
                    .call(
                        "browsingContext.getTree",
                        serde_json::json!({"maxDepth":0}),
                        timeout,
                    )
                    .await?;
                let contexts = result["contexts"].as_array().ok_or_else(|| {
                    E2eError::Config(
                        "browsingContext.getTree omitted contexts while confirming disposal".into(),
                    )
                })?;
                if contexts.iter().any(|context| context["context"].as_str().is_none_or(str::is_empty)) {
                    return Err(E2eError::Config("browsingContext.getTree returned invalid context identity while confirming disposal".into()));
                }
                Ok(!contexts
                    .iter()
                    .any(|context| context["context"].as_str() == Some(self.context.as_str())))
            },
            timeout,
        )
        .await
    }

    async fn raw(&self, method: &str, mut params: Value, timeout: Duration) -> E2eResult<Value> {
        if let Some(object) = params.as_object_mut() {
            object
                .entry("context")
                .or_insert_with(|| self.context_param());
        }
        self.bidi.call(method, params, timeout).await
    }

    async fn start_frame_stream(
        &self,
        opts: &VideoOptions,
    ) -> E2eResult<(FrameStream, tokio::task::AbortHandle)> {
        // BiDi screencast is file-based (no frame events), so live frames
        // fall back to paced screenshots.
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let bidi = self.bidi.clone();
        let context = self.context.clone();
        let timeout = self.timeout();
        let period = Duration::from_millis((1000 / u64::from(opts.fps.max(1))).max(50));
        let started = tokio::time::Instant::now();
        let handle = tokio::spawn(async move {
            let mut index = 0u64;
            loop {
                let shot = bidi
                    .call(
                        "browsingContext.captureScreenshot",
                        serde_json::json!({ "context": context }),
                        timeout,
                    )
                    .await;
                if let Ok(shot) = shot {
                    let data = shot.get("data").and_then(Value::as_str).unwrap_or_default();
                    if let Ok(bytes) = decode_base64(data) {
                        let frame = VideoFrame {
                            index,
                            timestamp_ms: started.elapsed().as_millis().min(u128::from(u64::MAX))
                                as u64,
                            data: bytes,
                        };
                        index += 1;
                        if tx.send(frame).is_err() {
                            break;
                        }
                    }
                }
                tokio::time::sleep(period).await;
            }
        });
        Ok((FrameStream { receiver: rx }, handle.abort_handle()))
    }

    async fn stop_frame_stream(&self) {
        // Polling pump stops with its task; nothing protocol-level to undo.
    }

    async fn start_recording(&self, opts: &VideoOptions) -> E2eResult<RecordingState> {
        std::fs::create_dir_all(&opts.dir)?;
        let dir = opts.dir.display().to_string();
        let started = self
            .bidi
            .call(
                "browsingContext.startScreencast",
                serde_json::json!({ "context": self.context, "destinationFolder": dir }),
                self.timeout(),
            )
            .await?;
        let screencast = started
            .get("screencast")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::Launch("BiDi startScreencast returned no id".to_string()))?
            .to_string();
        let path = started
            .get("path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| opts.dir.join("screencast.webm"));
        Ok(RecordingState::Bidi { screencast, path })
    }

    async fn stop_recording(
        &self,
        state: RecordingState,
        output: &std::path::Path,
    ) -> E2eResult<PathBuf> {
        let RecordingState::Bidi { screencast, path } = state else {
            return Err(E2eError::Config("recording/engine mismatch".to_string()));
        };
        let stopped = self
            .bidi
            .call(
                "browsingContext.stopScreencast",
                serde_json::json!({ "context": self.context, "screencast": screencast }),
                Duration::from_secs(30),
            )
            .await?;
        // Firefox may ignore destinationFolder (observed on 156): always move
        // by the reported path so nothing is stranded in Downloads.
        let src = stopped
            .get("path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .unwrap_or(path);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if src != output {
            move_file(&src, output).map_err(|error| {
                E2eError::Config(format!(
                    "move recording {} -> {}: {error}",
                    src.display(),
                    output.display()
                ))
            })?;
        }
        Ok(output.to_path_buf())
    }

    async fn cancel_recording(&self, state: RecordingState) {
        if let RecordingState::Bidi { screencast, path } = state {
            let stopped = self
                .bidi
                .call(
                    "browsingContext.stopScreencast",
                    serde_json::json!({ "context": self.context, "screencast": screencast }),
                    Duration::from_secs(10),
                )
                .await;
            let src = stopped
                .ok()
                .and_then(|s| s.get("path").and_then(Value::as_str).map(PathBuf::from))
                .unwrap_or(path);
            let _ = std::fs::remove_file(src);
        }
    }
}

/// BiDi Enter key (private-use code point, WebDriver convention).
const BIDI_ENTER: &str = "\u{E007}";

/// Map a key name to a BiDi key value: single chars pass through, named
/// keys use WebDriver private-use code points (Firefox rejects names here).
fn bidi_key_value(key: &str) -> String {
    match key {
        "Enter" => BIDI_ENTER.to_string(),
        "Tab" => "\u{E004}".to_string(),
        "Escape" | "Esc" => "\u{E00C}".to_string(),
        "Backspace" => "\u{E003}".to_string(),
        "Delete" => "\u{E017}".to_string(),
        "ArrowLeft" => "\u{E012}".to_string(),
        "ArrowUp" => "\u{E013}".to_string(),
        "ArrowRight" => "\u{E014}".to_string(),
        "ArrowDown" => "\u{E015}".to_string(),
        "Home" => "\u{E011}".to_string(),
        "End" => "\u{E010}".to_string(),
        "PageUp" => "\u{E00E}".to_string(),
        "PageDown" => "\u{E00F}".to_string(),
        "Shift" => "\u{E008}".to_string(),
        "Control" | "Ctrl" => "\u{E009}".to_string(),
        "Alt" => "\u{E00A}".to_string(),
        "Meta" | "Command" => "\u{E00D}".to_string(),
        other => other.to_string(),
    }
}

/// True when a protocol error means "this build does not know the command".
fn is_unsupported_command(error: &E2eError) -> bool {
    match error {
        E2eError::Cdp { method, message } => {
            let text = format!("{method} {message}").to_lowercase();
            text.contains("unknown command")
                || text.contains("unsupported")
                || text.contains("unimplemented")
                || text.contains("not implemented")
        }
        _ => false,
    }
}

fn console_metadata(
    kind: String,
    text: String,
    details: &Value,
    timestamp: &Value,
) -> ConsoleMessage {
    let frame = details["stackTrace"]["callFrames"]
        .as_array()
        .and_then(|frames| frames.first());
    let location = frame
        .and_then(console_location)
        .or_else(|| console_location(details));
    let timestamp_ms = timestamp.as_u64().or_else(|| {
        timestamp
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.0 && *n < u64::MAX as f64)
            .map(|n| n as u64)
    });
    ConsoleMessage {
        kind,
        text,
        location,
        timestamp_ms,
        page_id: None,
        arguments: None,
        error: None,
    }
}
fn console_location(value: &Value) -> Option<crate::ConsoleLocation> {
    let url = value["url"].as_str()?.to_owned();
    if url.is_empty() {
        return None;
    }
    Some(crate::ConsoleLocation {
        url,
        line: value["lineNumber"].as_u64()?.min(u64::from(u32::MAX)) as u32,
        column: value["columnNumber"]
            .as_u64()
            .unwrap_or(0)
            .min(u64::from(u32::MAX)) as u32,
    })
}

fn handle_bidi_event(event: &BidiEvent, sink: &ConsoleSink) {
    sink.observe_lifecycle(&event.method, &event.params, false);
    match event.method.as_str() {
        "log.entryAdded" => {
            let entry_type = event.params["type"].as_str().unwrap_or("console");
            let kind = if entry_type == "console" {
                event.params["method"].as_str().unwrap_or("log").to_string()
            } else {
                "exception".to_string()
            };
            let text = event.params["text"].as_str().unwrap_or_default();
            let text = if text.is_empty() {
                event.params["args"]
                    .as_array()
                    .map(|args| {
                        args.iter()
                            .map(|arg| remote_to_json(arg).to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default()
            } else {
                text.to_string()
            };
            let mut message =
                console_metadata(kind, text, &event.params, &event.params["timestamp"]);
            if entry_type == "console" {
                message.arguments = crate::console::arguments(&event.params["args"], true);
            } else if entry_type == "javascript" {
                message.error = Some(Box::new(crate::console::error(&event.params, true)));
            }
            sink.push_console(message);
        }
        "network.beforeRequestSent" => {
            if event.params["redirectCount"].as_u64().unwrap_or(0) > 0 {
                sink.network_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_redirect(
                        event.params["request"]["request"]
                            .as_str()
                            .unwrap_or_default(),
                    );
            }
            sink.start_network_request(
                NetworkRequest {
                    request_id: event.params["request"]["request"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    method: event.params["request"]["method"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    url: event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                },
                event.params["context"].as_str().map(str::to_owned),
                event.params["navigation"].as_str().map(str::to_owned),
            );
            sink.observe_request(
                RecordedRequest {
                    method: event.params["request"]["method"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    url: event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    status: 0,
                    headers: bidi_header_pairs(&event.params["request"]["headers"]),
                    post_data: event.params["request"]["postData"]
                        .as_str()
                        .map(str::to_owned),
                    started_ms: Some(now_ms()),
                    request_id: event.params["request"]["request"]
                        .as_str()
                        .map(str::to_owned),
                    status_text: String::new(),
                    mime_type: String::new(),
                    response_headers: Vec::new(),
                    duration_ms: None,
                    body: None,
                    body_truncated: false,
                },
                crate::network::RequestDetails {
                    frame_id: event.params["context"].as_str().map(str::to_owned),
                    resource_type: event.params["request"]["destination"]
                        .as_str()
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned),
                    is_navigation: event.params.get("navigation").map(|value| !value.is_null()),
                    redirect: event.params["redirectCount"].as_u64().unwrap_or(0) > 0,
                    raw_headers: false,
                },
            );
            if event.params["isBlocked"].as_bool() == Some(true) {
                let id = event.params["request"]["request"]
                    .as_str()
                    .unwrap_or_default();
                let pause_id = format!(
                    "bidi:{id}:{}",
                    event.params["redirectCount"].as_u64().unwrap_or(0)
                );
                sink.network_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .paused(
                        &pause_id,
                        id,
                        event.params["request"]["url"].as_str().unwrap_or_default(),
                    );
            }
        }
        "network.responseStarted" => {
            sink.observe_response(
                event.params["request"]["request"]
                    .as_str()
                    .unwrap_or_default(),
                &event.params["response"],
                bidi_header_pairs(&event.params["response"]["headers"]),
                None,
            );
            sink.emit(PageEvent::Response {
                request_id: event.params["request"]["request"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                url: event.params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                status: event.params["response"]["status"].as_u64().unwrap_or(0) as u16,
            });
        }
        "network.responseCompleted" => {
            sink.finish_network_request(
                event.params["request"]["request"]
                    .as_str()
                    .unwrap_or_default(),
                None,
            );
        }
        "network.fetchError" => {
            sink.finish_network_request(
                event.params["request"]["request"]
                    .as_str()
                    .unwrap_or_default(),
                Some((
                    event.params["errorText"]
                        .as_str()
                        .unwrap_or("network request failed")
                        .into(),
                    None,
                )),
            );
        }
        _ => {}
    }
}

/// Wait for network quiet when `state` is [`LoadState::NetworkIdle`].
async fn settle_quiet(
    sink: &ConsoleSink,
    state: LoadState,
    deadline: crate::operation::Deadline,
) -> E2eResult<()> {
    if state != LoadState::NetworkIdle {
        return Ok(());
    }
    let source = sink.native_observation_loss();
    deadline.run("network idle", async {
        let quiet_for = Duration::from_millis(500);
        let mut quiet_since = None;
        let mut previous_activity = None;
        loop {
            if let Some(reason) = source.reason() {
                return Err(E2eError::Config(format!("network idle observation source unavailable: {reason}")));
            }
            let (active, activity) = sink.network_activity();
            if previous_activity != Some(activity) { quiet_since = None; }
            previous_activity = Some(activity);
            if active == 0 {
                match quiet_since {
                    None => quiet_since = Some(tokio::time::Instant::now()),
                    Some(since) if since.elapsed() >= quiet_for => return Ok(()),
                    Some(_) => {}
                }
            } else { quiet_since = None; }
            tokio::select! { biased;
                reason = source.cancelled() => return Err(E2eError::Config(format!("network idle observation source unavailable: {reason}"))),
                () = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        }
    }).await
}

/// Host (without port) of an http(s) URL.
fn url_host(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1)?;
    let authority = after_scheme.split('/').next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

pub(crate) fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let mut n: u32 = 0;
        for (i, byte) in chunk.iter().enumerate() {
            n |= (*byte as u32) << (16 - 8 * i);
        }
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6 & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub(crate) fn decode_shot(value: &Value) -> E2eResult<Vec<u8>> {
    let data = value
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    decode_base64(data).map_err(|error| E2eError::Cdp {
        method: "captureScreenshot".to_string(),
        message: error,
    })
}

fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| error.to_string())
}

/// Decode a `Network.getResponseBody` result (`None` when undecodable).
/// Collect frames from a CDP frame tree (main frame first, depth-first).
/// Surface a CDP `exceptionDetails` payload as an [`E2eError`].
fn check_cdp_exception(method: &str, result: &Value, expression: &str) -> E2eResult<()> {
    if let Some(exception) = result.get("exceptionDetails") {
        let text = exception
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("js exception");
        return Err(E2eError::Cdp {
            method: method.to_string(),
            message: format!("{text}: {expression}"),
        });
    }
    Ok(())
}

/// Surface a BiDi script `exception` result as an [`E2eError`].
fn check_script_exception(method: &str, result: &Value, expression: &str) -> E2eResult<()> {
    if result.get("type").and_then(Value::as_str) == Some("exception") {
        let text = result["exceptionDetails"]["text"]
            .as_str()
            .unwrap_or("js exception");
        return Err(E2eError::Cdp {
            method: method.to_string(),
            message: format!("{text}: {expression}"),
        });
    }
    Ok(())
}

/// Bound and retain native socket observations before forwarding legacy events.
fn emit_socket(sink: &ConsoleSink, event: &CdpEvent, direction: WebSocketDirection) {
    let observation = sink
        .socket_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .observe(&event.params, direction);
    if let Some(observation) = observation {
        sink.emit(PageEvent::WebSocket(observation));
    }
}

/// Retag a transport error with the script method that caused it.
fn script_call_error(method: &str, error: E2eError) -> E2eError {
    match error {
        E2eError::Cdp { message, .. } => E2eError::Cdp {
            method: method.to_string(),
            message,
        },
        other => other,
    }
}

fn collect_cdp_frames(tree: &Value, out: &mut Vec<FrameInfo>) {
    collect_cdp_frames_inner(tree, None, out);
}

fn collect_cdp_frames_inner(tree: &Value, parent: Option<String>, out: &mut Vec<FrameInfo>) {
    let frame = &tree["frame"];
    let id = frame["id"].as_str().unwrap_or_default().to_string();
    out.push(FrameInfo {
        id: id.clone(),
        parent_id: parent,
        name: frame["name"].as_str().unwrap_or_default().to_string(),
        url: frame["url"].as_str().unwrap_or_default().to_string(),
    });
    if let Some(children) = tree.get("childFrames").and_then(Value::as_array) {
        for child in children {
            collect_cdp_frames_inner(child, Some(id.clone()), out);
        }
    }
}

/// Collect frames from a BiDi context tree (names are not reported).
fn collect_bidi_frames(node: &Value, out: &mut Vec<FrameInfo>) {
    collect_bidi_frames_inner(node, None, out);
}

fn collect_bidi_frames_inner(node: &Value, parent: Option<String>, out: &mut Vec<FrameInfo>) {
    let id = node["context"].as_str().unwrap_or_default().to_string();
    out.push(FrameInfo {
        id: id.clone(),
        parent_id: parent,
        name: String::new(),
        url: node["url"].as_str().unwrap_or_default().to_string(),
    });
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_bidi_frames_inner(child, Some(id.clone()), out);
        }
    }
}

/// An in-flight capture entry (completed at response/failure time).
struct PendingRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    post_data: Option<String>,
    started: tokio::time::Instant,
    started_ms: u64,
}

/// Milliseconds since the Unix epoch (0 on clock failure).
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

/// Header pairs from a CDP request object.
fn cdp_header_pairs(headers: &Value) -> Vec<(String, String)> {
    headers
        .as_object()
        .map(|map| {
            map.iter()
                .flat_map(|(name, value)| {
                    value
                        .as_str()
                        .unwrap_or_default()
                        .split('\n')
                        .map(move |value| (name.clone(), value.trim_end_matches('\r').to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Header pairs from a BiDi request object (base64 values decoded lossily).
fn bidi_header_pairs(headers: &Value) -> Vec<(String, String)> {
    headers
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|header| {
                    let name = header.get("name")?.as_str()?.to_string();
                    let value = header.get("value")?;
                    let text = match value.get("type")?.as_str()? {
                        "base64" => decode_base64(value.get("value")?.as_str()?)
                            .ok()
                            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())?,
                        _ => value.get("value")?.as_str()?.to_string(),
                    };
                    Some((name, text))
                })
                .flat_map(|(name, text)| {
                    text.split('\n')
                        .map(|value| (name.clone(), value.trim_end_matches('\r').to_owned()))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Set (or add) Content-Length on a header list.
/// Answer a CDP response-stage pause by merging `ModifyResponse` overrides
/// over the real response. Anything else (or an unreadable original body)
/// continues untouched: falling back beats corrupting.
async fn answer_response_pause(
    cdp: &CdpConnection,
    session: &str,
    timeout: Duration,
    request_id: &Value,
    params: &Value,
    action: Option<&RouteAction>,
) {
    let pass = serde_json::json!({ "requestId": request_id });
    let Some(RouteAction::ModifyResponse {
        status,
        headers,
        body,
    }) = action
    else {
        let _ = cdp
            .call(Some(session), "Fetch.continueResponse", pass, timeout)
            .await;
        return;
    };
    if status.is_none() && headers.is_none() && body.is_none() {
        let _ = cdp
            .call(Some(session), "Fetch.continueResponse", pass, timeout)
            .await;
        return;
    }
    let original = match cdp
        .call(
            Some(session),
            "Fetch.getResponseBody",
            serde_json::json!({ "requestId": request_id }),
            timeout,
        )
        .await
    {
        Ok(original) => original,
        Err(_) => {
            let _ = cdp
                .call(Some(session), "Fetch.continueResponse", pass, timeout)
                .await;
            return;
        }
    };
    let original_body: Vec<u8> =
        if original.get("base64Encoded").and_then(Value::as_bool) == Some(true) {
            let encoded = original
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default();
            match decode_base64(encoded) {
                Ok(bytes) => bytes,
                Err(_) => {
                    let _ = cdp
                        .call(Some(session), "Fetch.continueResponse", pass, timeout)
                        .await;
                    return;
                }
            }
        } else {
            original
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .as_bytes()
                .to_vec()
        };
    let code = status
        .as_ref()
        .map(|code| u64::from(*code))
        .unwrap_or_else(|| {
            params
                .get("responseStatusCode")
                .and_then(Value::as_u64)
                .unwrap_or(200)
        });
    let mut merged: Vec<(String, String)> = match headers {
        Some(list) => list.clone(),
        None => params
            .get("responseHeaders")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some((
                            item.get("name")?.as_str()?.to_string(),
                            item.get("value")?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default(),
    };
    let final_body = body.clone().unwrap_or(original_body);
    if body.is_some() || headers.is_some() {
        merged = with_content_length(merged, final_body.len());
    }
    let result = cdp
        .call(
            Some(session),
            "Fetch.fulfillRequest",
            serde_json::json!({
                "requestId": request_id,
                "responseCode": code,
                "responseHeaders": merged
                    .iter()
                    .map(|(name, value)| serde_json::json!({"name": name, "value": value}))
                    .collect::<Vec<_>>(),
                "body": base64_encode(&final_body),
            }),
            timeout,
        )
        .await;
    if result.is_err() {
        let _ = cdp
            .call(
                Some(session),
                "Fetch.continueResponse",
                serde_json::json!({ "requestId": request_id }),
                timeout,
            )
            .await;
    }
}

/// Resolve a dialog answer: the handler decision wins, else the armed
/// accept/prompt defaults.
fn decide_dialog(
    handler: Option<&DialogHandler>,
    info: &DialogInfo,
    accept: bool,
    prompt_text: &Option<String>,
) -> (bool, Option<String>) {
    match handler.map(|decide| decide(info.clone())) {
        Some(DialogDecision::Accept(text)) => (true, text),
        Some(DialogDecision::Dismiss) => (false, None),
        None => (accept, prompt_text.clone()),
    }
}

struct CachedRouteDecision {
    action: Option<RouteAction>,
    _handler: Option<crate::routing::HandlerGuard>,
    _request: crate::routing::RequestGuard,
    network_id: Option<String>,
}
async fn route_pump_idle<F: std::future::Future<Output = ()>>(
    runtime: &crate::routing::RouteRuntime,
    slot: &std::sync::Weak<crate::routing::PumpSlot>,
    driver: &Driver,
    drain: impl FnOnce() -> F,
) -> bool {
    if !runtime.idle() {
        return false;
    }
    let Some(slot) = slot.upgrade() else {
        return true;
    };
    let mut slot = slot.lock().await;
    if !runtime.idle() {
        return false;
    }
    let _ = tokio::time::timeout(Duration::from_millis(750), driver.stop_routing()).await;
    let _ = tokio::time::timeout(Duration::from_millis(750), drain()).await;
    slot.take();
    true
}

#[derive(Clone)]
pub(crate) enum RouteMatcher {
    Shared(crate::UrlMatcher),
    Legacy(globset::GlobMatcher),
}
pub(crate) fn routing_matchers<'a>(
    patterns: impl IntoIterator<Item = (&'a str, Option<&'a crate::UrlMatcher>)>,
) -> E2eResult<Vec<RouteMatcher>> {
    patterns
        .into_iter()
        .map(|(pattern, matcher)| match matcher {
            Some(matcher) => Ok(RouteMatcher::Shared(matcher.clone())),
            None => crate::url_matcher::legacy_glob(pattern).map(RouteMatcher::Legacy),
        })
        .collect()
}
/// Registration order is shared by typed matchers and legacy glob fallbacks.
pub(crate) fn sorted_matches(set: &[RouteMatcher], url: &str) -> Vec<usize> {
    set.iter()
        .enumerate()
        .filter_map(|(index, matcher)| {
            let matches = match matcher {
                RouteMatcher::Shared(m) => m.matches(url),
                RouteMatcher::Legacy(m) => m.is_match(url),
            };
            matches.then_some(index)
        })
        .collect()
}

fn with_content_length(mut headers: Vec<(String, String)>, len: usize) -> Vec<(String, String)> {
    match headers
        .iter_mut()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        Some(slot) => slot.1 = len.to_string(),
        None => headers.push(("Content-Length".to_string(), len.to_string())),
    }
    headers
}

/// Unique frame spool directory inside `dir`.
fn spool_dir(dir: &std::path::Path) -> E2eResult<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let spool = dir.join(format!(".spool-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&spool)?;
    Ok(spool)
}

/// Append one line to a manifest file.
fn append_line(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

/// Move a file, falling back to copy+remove across filesystems
/// (Firefox records into Downloads, often a different mount than /tmp).
fn move_file(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(src, dst)?;
            std::fs::remove_file(src)
        }
        Err(error) => Err(error),
    }
}

/// Read a `manifest.jsonl` spool manifest (tolerates a missing file).
fn read_manifest(path: &std::path::Path) -> Vec<SpooledFrame> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|entry| {
            Some(SpooledFrame {
                file: entry.get("file")?.as_str()?.to_string(),
                timestamp_ms: entry.get("t")?.as_u64()?,
            })
        })
        .collect()
}

fn handle_cdp_event(event: &CdpEvent, sink: &ConsoleSink) {
    sink.observe_lifecycle(&event.method, &event.params, true);
    match event.method.as_str() {
        "Fetch.requestPaused" => {
            if let (Some(pause_id), Some(native_id), Some(url)) = (
                event.params["requestId"].as_str(),
                event.params["networkId"].as_str(),
                event.params["request"]["url"].as_str(),
            ) {
                sink.network_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .paused(pause_id, native_id, url);
            }
        }
        "Page.frameNavigated" => {
            let frame = &event.params["frame"];
            if let (Some(id), Some(loader)) = (frame["id"].as_str(), frame["loaderId"].as_str()) {
                sink.commit_network_document(id, loader, frame["parentId"].is_null());
            }
        }
        "Runtime.consoleAPICalled" => {
            let kind = event.params["type"].as_str().unwrap_or("log").to_string();
            let args = event.params["args"]
                .as_array()
                .map(|args| {
                    args.iter()
                        .map(|arg| {
                            arg.get("value")
                                .map(|v| v.to_string())
                                .or_else(|| arg.get("description").map(|d| d.to_string()))
                                .unwrap_or_else(|| "...".to_string())
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            let mut message =
                console_metadata(kind, args, &event.params, &event.params["timestamp"]);
            message.arguments = crate::console::arguments(&event.params["args"], false);
            sink.push_console(message);
        }
        "Runtime.exceptionThrown" => {
            let details = &event.params["exceptionDetails"];
            let text = details["exception"]["description"]
                .as_str()
                .or_else(|| details["text"].as_str())
                .unwrap_or("page exception")
                .to_owned();
            let mut message = console_metadata(
                "exception".into(),
                text,
                details,
                &event.params["timestamp"],
            );
            message.error = Some(Box::new(crate::console::error(details, false)));
            sink.push_console(message);
        }
        "Log.entryAdded" => {
            let entry = &event.params["entry"];
            let kind = entry["level"].as_str().unwrap_or("log").to_string();
            let text = entry["text"].as_str().unwrap_or_default().to_string();
            let mut message = console_metadata(kind, text, entry, &entry["timestamp"]);
            message.arguments = crate::console::arguments(&entry["args"], false);
            sink.push_console(message);
        }
        "Network.requestWillBeSent" => {
            let id = event.params["requestId"].as_str().unwrap_or_default();
            if let Some(response) = event.params.get("redirectResponse") {
                sink.observe_response(
                    id,
                    response,
                    cdp_header_pairs(&response["headers"]),
                    event.params["redirectHasExtraInfo"].as_bool(),
                );
                sink.emit(PageEvent::Response {
                    request_id: id.into(),
                    url: response["url"].as_str().unwrap_or_default().into(),
                    status: response["status"].as_u64().unwrap_or(0) as u16,
                });
                sink.network_log
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .finish_redirect(id);
            }
            sink.start_network_request(
                NetworkRequest {
                    request_id: id.into(),
                    method: event.params["request"]["method"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    url: event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                },
                event.params["frameId"].as_str().map(str::to_owned),
                event.params["loaderId"].as_str().map(str::to_owned),
            );
            sink.observe_request(
                RecordedRequest {
                    method: event.params["request"]["method"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    url: event.params["request"]["url"]
                        .as_str()
                        .unwrap_or_default()
                        .into(),
                    status: 0,
                    headers: cdp_header_pairs(&event.params["request"]["headers"]),
                    post_data: event.params["request"]["postData"]
                        .as_str()
                        .map(str::to_owned),
                    started_ms: Some(now_ms()),
                    request_id: event.params["requestId"].as_str().map(str::to_owned),
                    status_text: String::new(),
                    mime_type: String::new(),
                    response_headers: Vec::new(),
                    duration_ms: None,
                    body: None,
                    body_truncated: false,
                },
                crate::network::RequestDetails {
                    frame_id: event.params["frameId"].as_str().map(str::to_owned),
                    resource_type: event.params["type"].as_str().map(str::to_ascii_lowercase),
                    is_navigation: event.params["type"]
                        .as_str()
                        .map(|value| value == "Document"),
                    redirect: event.params.get("redirectResponse").is_some(),
                    raw_headers: true,
                },
            );
        }
        "Network.requestWillBeSentExtraInfo" | "Network.responseReceivedExtraInfo" => {
            let id = event.params["requestId"].as_str().unwrap_or_default();
            sink.network_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extra_headers(
                    id,
                    event.method == "Network.responseReceivedExtraInfo",
                    cdp_header_pairs(&event.params["headers"]),
                );
        }
        "Network.responseReceived" => {
            sink.observe_response(
                event.params["requestId"].as_str().unwrap_or_default(),
                &event.params["response"],
                cdp_header_pairs(&event.params["response"]["headers"]),
                event.params["hasExtraInfo"].as_bool(),
            );
            sink.emit(PageEvent::Response {
                request_id: event.params["requestId"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                url: event.params["response"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                status: event.params["response"]["status"].as_u64().unwrap_or(0) as u16,
            });
        }
        "Network.loadingFinished" => {
            sink.finish_network_request(
                event.params["requestId"].as_str().unwrap_or_default(),
                None,
            );
        }
        "Network.loadingFailed" => {
            sink.finish_network_request(
                event.params["requestId"].as_str().unwrap_or_default(),
                Some((
                    event.params["errorText"]
                        .as_str()
                        .unwrap_or("network request failed")
                        .into(),
                    Some(event.params["canceled"].as_bool().unwrap_or(false)),
                )),
            );
        }
        "Network.webSocketCreated" => emit_socket(sink, event, WebSocketDirection::Created),
        "Network.webSocketFrameSent" => emit_socket(sink, event, WebSocketDirection::Sent),
        "Network.webSocketFrameReceived" => emit_socket(sink, event, WebSocketDirection::Received),
        "Network.webSocketFrameError" => emit_socket(sink, event, WebSocketDirection::Error),
        "Network.webSocketClosed" => emit_socket(sink, event, WebSocketDirection::Closed),
        _ => {}
    }
}

#[cfg(test)]
mod initialization_tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use serde_json::json;

    #[tokio::test]
    async fn lost_main_world_source_interrupts_cached_rpc_but_not_direct_evaluation() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (observed, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            while let Some(Ok(message)) = socket.next().await {
                let tokio_tungstenite::tungstenite::Message::Text(text) = message else {
                    continue;
                };
                let request: Value = serde_json::from_str(&text).unwrap();
                if request["method"] == "Runtime.evaluate" && request["params"]["contextId"] == 42 {
                    observed.send(()).unwrap();
                    continue;
                }
                let result = if request["method"] == "Page.getFrameTree" {
                    json!({"frameTree":{"frame":{"id":"root","url":"about:blank","loaderId":"document"}}})
                } else if request["method"] == "Runtime.evaluate" {
                    json!({"result":{"type":"number","value":7}})
                } else {
                    json!({})
                };
                socket
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        json!({"id":request["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
            }
        });
        let connection = CdpConnection::connect(&format!("ws://{address}"))
            .await
            .unwrap();
        let sink = ConsoleSink::new();
        let native = CdpDriver::spawn(
            connection,
            "session".into(),
            "page".into(),
            Duration::ZERO,
            sink.clone(),
            None,
        )
        .await
        .unwrap();
        native.main_worlds.lock().unwrap().insert("root".into(), 42);
        let driver = Driver::Cdp(native);
        let query = driver.frame_main_world_evaluate("root", "7");
        let loss = async {
            requests.recv().await.unwrap();
            sink.mark_native_observation_lost("test listener exit");
        };
        let (result, ()) =
            tokio::time::timeout(Duration::from_secs(2), async { tokio::join!(query, loss) })
                .await
                .unwrap();
        assert!(
            matches!(result, Err(E2eError::Config(message)) if message.contains("main-world observation source unavailable"))
        );
        assert!(
            matches!(driver.frame_main_world_evaluate("root", "7").await, Err(E2eError::Config(message)) if message.contains("main-world observation source unavailable"))
        );
        assert_eq!(driver.evaluate("7").await.unwrap(), json!(7));
        server.abort();
    }

    async fn interrupted_fetch_startup(auth: bool, reject: bool) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (observed, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut late = None;
            while let Some(Ok(message)) = socket.next().await {
                let tokio_tungstenite::tungstenite::Message::Text(text) = message else {
                    continue;
                };
                let request: Value = serde_json::from_str(&text).unwrap();
                if request["method"]
                    .as_str()
                    .is_some_and(|method| method.starts_with("Fetch."))
                {
                    observed.send(request.clone()).unwrap();
                }
                let startup = request["method"] == "Fetch.enable"
                    && request["params"]["patterns"]
                        .as_array()
                        .is_some_and(|patterns| patterns.len() == 2);
                if startup && !reject {
                    late = Some(request);
                    continue;
                }
                if request["method"] == "Fetch.disable" {
                    if let Some(original) = late.take() {
                        // Deliver enable's ACK after cancellation; cleanup is still ordered after it.
                        socket
                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                json!({"id":original["id"],"result":{}}).to_string().into(),
                            ))
                            .await
                            .unwrap();
                    }
                }
                let reply = if startup {
                    json!({"id":request["id"],"error":{"code":-1,"message":"setup refused"}})
                } else {
                    let result = if request["method"] == "Page.getFrameTree" {
                        json!({"frameTree":{"frame":{"id":"root","url":"about:blank","loaderId":"document"}}})
                    } else {
                        json!({})
                    };
                    json!({"id":request["id"],"result":result})
                };
                socket
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        reply.to_string().into(),
                    ))
                    .await
                    .unwrap();
            }
        });
        let connection = CdpConnection::connect(&format!("ws://{address}"))
            .await
            .unwrap();
        let driver = CdpDriver::spawn(
            connection.clone(),
            "session".into(),
            "page".into(),
            Duration::ZERO,
            ConsoleSink::new(),
            None,
        )
        .await
        .unwrap();
        if auth {
            driver.fetch_auth.lock().unwrap().creds = Some(("user".into(), "password".into()));
        }
        let owned = driver.clone();
        let setup = tokio::spawn(async move {
            owned
                .start_routing(
                    Arc::new(crate::routing::RouteRuntime::default()),
                    Default::default(),
                )
                .await
        });
        let enabled = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(enabled["method"], "Fetch.enable");
        if reject {
            assert!(matches!(setup.await.unwrap(), Err(E2eError::Cdp { .. })));
        } else {
            setup.abort();
            assert!(matches!(setup.await, Err(error) if error.is_cancelled()));
        }
        let disabled = tokio::time::timeout(Duration::from_secs(2), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(disabled["method"], "Fetch.disable");
        assert_eq!(disabled["sessionId"], "session");
        assert!(driver.fetch_auth.lock().unwrap().routing_patterns.is_none());
        if auth {
            let restored = tokio::time::timeout(Duration::from_secs(2), requests.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(restored["method"], "Fetch.enable");
            assert_eq!(restored["params"], fetch_enable_params(None, true).unwrap());
            assert!(driver.fetch_auth.lock().unwrap().creds.is_some());
        }
        connection
            .call(
                None,
                "test.transportAlive",
                json!({}),
                Duration::from_secs(1),
            )
            .await
            .unwrap();
        assert!(requests.try_recv().is_err());
        assert!(connection.is_open());
        connection.close();
        server.abort();
    }
    #[tokio::test]
    async fn readiness_rpc_uses_local_and_enclosing_budgets_and_can_retry_on_same_transport() {
        for bidi in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let release = Arc::new(AtomicBool::new(false));
            let ready = release.clone();
            let (observed, mut requests) = tokio::sync::mpsc::unbounded_channel();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) =
                    socket.next().await
                {
                    let request: Value = serde_json::from_str(&text).unwrap();
                    let evaluate = request["method"] == "Runtime.evaluate"
                        || request["method"] == "script.evaluate";
                    if evaluate {
                        observed.send(()).unwrap();
                        if !ready.load(Ordering::Acquire) {
                            continue;
                        }
                    }
                    let result = if evaluate {
                        if bidi {
                            json!({"type":"success","result":{"type":"object","value":[["ready",{"type":"string","value":"complete"}]]}})
                        } else {
                            json!({"result":{"type":"object","value":{"ready":"complete"}}})
                        }
                    } else if request["method"] == "Page.getFrameTree" {
                        json!({"frameTree":{"frame":{"id":"frame","url":"about:blank","loaderId":"document"}}})
                    } else if request["method"] == "browsingContext.getTree" {
                        json!({"contexts":[{"context":"page","children":[]}]})
                    } else {
                        json!({})
                    };
                    let reply = if bidi {
                        json!({"type":"success","id":request["id"],"result":result})
                    } else {
                        json!({"id":request["id"],"result":result})
                    };
                    socket
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            reply.to_string().into(),
                        ))
                        .await
                        .unwrap();
                }
            });
            let driver = if bidi {
                Driver::Bidi(
                    BidiDriver::spawn(
                        BidiConnection::connect(&format!("ws://{address}"))
                            .await
                            .unwrap(),
                        "page".into(),
                        Duration::ZERO,
                        false,
                        ConsoleSink::new(),
                        None,
                    )
                    .await
                    .unwrap(),
                )
            } else {
                Driver::Cdp(
                    CdpDriver::spawn(
                        CdpConnection::connect(&format!("ws://{address}"))
                            .await
                            .unwrap(),
                        "session".into(),
                        "page".into(),
                        Duration::ZERO,
                        ConsoleSink::new(),
                        None,
                    )
                    .await
                    .unwrap(),
                )
            };
            let result = tokio::time::timeout(
                Duration::from_millis(200),
                driver.wait_for_load(LoadState::Load, Duration::from_millis(20)),
            )
            .await
            .expect("readiness RPC must share its own load-state deadline");
            assert!(matches!(result, Err(E2eError::Timeout(..))));
            tokio::time::timeout(Duration::from_secs(1), requests.recv())
                .await
                .expect("native readiness command delivery")
                .expect("native readiness command was dispatched");
            let result = crate::operation::Deadline::new(Duration::from_millis(10))
                .run(
                    "outer readiness",
                    driver.wait_for_load(LoadState::Load, Duration::ZERO),
                )
                .await;
            assert!(matches!(result, Err(E2eError::Timeout(..))));
            tokio::time::timeout(Duration::from_secs(1), requests.recv())
                .await
                .expect("zero-local-timeout readiness delivery")
                .expect("zero-local-timeout readiness was dispatched");
            release.store(true, Ordering::Release);
            driver
                .wait_for_load(LoadState::Load, Duration::from_secs(1))
                .await
                .unwrap();
            match driver {
                Driver::Cdp(driver) => {
                    assert!(driver.cdp.is_open());
                    driver.cdp.close();
                }
                Driver::Bidi(driver) => {
                    assert!(driver.bidi.is_open());
                    driver.bidi.close();
                }
            }
            server.abort();
        }
    }

    #[tokio::test]
    async fn dropped_fetch_startup_disables_interception_and_preserves_auth_profile() {
        interrupted_fetch_startup(false, false).await;
        interrupted_fetch_startup(true, false).await;
    }
    #[tokio::test]
    async fn rejected_fetch_startup_disables_interception_and_preserves_auth_profile() {
        interrupted_fetch_startup(false, true).await;
        interrupted_fetch_startup(true, true).await;
    }

    #[tokio::test]
    async fn dropping_page_initialization_releases_its_listener_without_closing_transport() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (started, ready) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut started = Some(started);
            while let Some(Ok(message)) = socket.next().await {
                let tokio_tungstenite::tungstenite::Message::Text(text) = message else {
                    continue;
                };
                let request: Value = serde_json::from_str(&text).unwrap();
                if request["method"] == "Runtime.enable" {
                    started.take().unwrap().send(()).unwrap();
                    std::future::pending::<()>().await;
                }
                let result = if request["method"] == "Page.getFrameTree" {
                    json!({"frameTree":{"frame":{"id":"native-root","url":"about:blank","loaderId":"document"}}})
                } else {
                    json!({})
                };
                let reply =
                    json!({"id":request["id"],"sessionId":request["sessionId"],"result":result});
                socket
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        reply.to_string().into(),
                    ))
                    .await
                    .unwrap();
            }
        });
        let connection = CdpConnection::connect(&format!("ws://{address}"))
            .await
            .unwrap();
        let sink = ConsoleSink::new();
        let retained = Arc::downgrade(&sink.frame_events);
        let transport = connection.clone();
        let setup = tokio::spawn(async move {
            CdpDriver::spawn(
                transport,
                "session".into(),
                "page".into(),
                Duration::ZERO,
                sink,
                None,
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), ready)
            .await
            .unwrap()
            .unwrap();
        setup.abort();
        assert!(matches!(setup.await,Err(error) if error.is_cancelled()));
        tokio::time::timeout(Duration::from_secs(1), async {
            while retained.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("interrupted initialization must drop its native listener and frame metadata");
        assert!(
            connection.is_open(),
            "a page's interrupted initialization must not close the shared browser transport"
        );
        connection.close();
        server.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn network_idle_rejects_lost_sources_before_and_during_quiet_accounting() {
        let sink = ConsoleSink::new();
        let deadline = crate::operation::Deadline::new(Duration::ZERO);
        let (result, ()) = tokio::join!(
            settle_quiet(&sink, LoadState::NetworkIdle, deadline),
            async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                sink.mark_native_observation_lost("injected listener exit");
            }
        );
        assert!(
            matches!(result, Err(E2eError::Config(message)) if message.contains("source unavailable"))
        );
        assert!(matches!(
            settle_quiet(&sink, LoadState::NetworkIdle, deadline).await,
            Err(E2eError::Config(_))
        ));
        // Document readiness itself uses native evaluation and does not need the activity listener.
        settle_quiet(&sink, LoadState::Load, deadline)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn network_idle_sleep_respects_the_shared_deadline() {
        let sink = ConsoleSink::new();
        tokio::select! { biased;
            result = settle_quiet(&sink, LoadState::NetworkIdle, crate::operation::Deadline::new(Duration::from_millis(10))) => {
                assert!(matches!(result, Err(E2eError::Timeout(..))));
            }
            () = tokio::time::sleep(Duration::from_millis(25)) => {
                panic!("quiet polling must not wait for a full 50ms polling interval after its shared deadline");
            }
        }
    }

    #[test]
    fn host_parses() {
        assert_eq!(
            url_host("http://127.0.0.1:5190/app"),
            Some("127.0.0.1".to_string())
        );
        assert_eq!(
            url_host("https://example.com/"),
            Some("example.com".to_string())
        );
        assert_eq!(url_host("data:text/html,x"), None);
        assert_eq!(url_host("about:blank"), None);
    }

    #[test]
    fn fetch_config_merges_routing_and_auth() {
        // Neither: disable the domain.
        assert_eq!(fetch_enable_params(None, false), None);
        // Routing only: patterns without challenges.
        let patterns = vec![serde_json::json!({ "urlPattern": "*" })];
        assert_eq!(
            fetch_enable_params(Some(&patterns), false),
            Some(serde_json::json!({
                "patterns": [{ "urlPattern": "*" }],
                "handleAuthRequests": false,
            }))
        );
        // Auth only: empty patterns (nothing pauses) with challenges on.
        assert_eq!(
            fetch_enable_params(None, true),
            Some(serde_json::json!({
                "patterns": [{"urlPattern":"*"}],
                "handleAuthRequests": true,
            }))
        );
        // Both: routing patterns plus challenges.
        assert_eq!(
            fetch_enable_params(Some(&patterns), true),
            Some(serde_json::json!({
                "patterns": [{ "urlPattern": "*" }],
                "handleAuthRequests": true,
            }))
        );
    }

    #[test]
    fn base64_round_trips() {
        use base64::Engine as _;
        for text in ["", "f", "fo", "foo", "hello world", "{\"a\":1}"] {
            let encoded = base64_encode(text.as_bytes());
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .unwrap();
            assert_eq!(decoded, text.as_bytes());
        }
    }

    #[test]
    fn key_definitions_cover_specials() {
        assert_eq!(key_definition("Enter").0, 13);
        assert_eq!(key_definition("Escape").0, 27);
        assert_eq!(key_definition("ArrowLeft").0, 37);
        assert_eq!(key_definition("a").0, u16::from(b'A'));
    }

    #[test]
    fn key_definitions_cover_modifiers() {
        assert_eq!(key_definition("Shift"), (16, "Shift", "ShiftLeft"));
        assert_eq!(key_definition("Control"), (17, "Control", "ControlLeft"));
        assert_eq!(key_definition("Ctrl"), (17, "Control", "ControlLeft"));
        assert_eq!(key_definition("Alt"), (18, "Alt", "AltLeft"));
        assert_eq!(key_definition("Meta"), (91, "Meta", "MetaLeft"));
        assert_eq!(bidi_key_value("Shift"), "\u{E008}");
        assert_eq!(bidi_key_value("Control"), "\u{E009}");
        assert_eq!(bidi_key_value("Alt"), "\u{E00A}");
        assert_eq!(bidi_key_value("Meta"), "\u{E00D}");
        assert_eq!(bidi_key_value("Enter"), "\u{E007}");
    }
}

#[cfg(test)]
mod network_lifecycle_tests {
    use super::*;
    use crate::PageEventKind;
    fn cdp(method: &str, params: Value) -> CdpEvent {
        CdpEvent {
            session: None,
            method: method.into(),
            params,
        }
    }
    #[test]
    fn document_replacement_excludes_old_idle_work_without_faking_completion() {
        let sink = ConsoleSink::new();
        let mut events = sink.subscribe();
        for (id, frame, loader) in [
            ("old", "main", "before"),
            ("child", "child", "before-child"),
            ("new", "main", "after"),
        ] {
            handle_cdp_event(
                &cdp(
                    "Network.requestWillBeSent",
                    serde_json::json!({"requestId":id,"frameId":frame,"loaderId":loader,"request":{"method":"GET","url":format!("http://host/{id}")}}),
                ),
                &sink,
            );
            assert!(matches!(
                events.try_recv().unwrap(),
                PageEvent::Request { .. }
            ));
        }
        assert_eq!(sink.network_activity().0, 3);
        handle_cdp_event(
            &cdp(
                "Page.frameNavigated",
                serde_json::json!({"frame":{"id":"main","loaderId":"after"}}),
            ),
            &sink,
        );
        assert_eq!(sink.network_activity().0, 1);
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"old"}),
            ),
            &sink,
        );
        assert_eq!(sink.network_activity().0, 1);
        assert!(
            matches!(events.try_recv().unwrap(),PageEvent::RequestFinished(r) if r.request_id=="old")
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFailed",
                serde_json::json!({"requestId":"child","errorText":"aborted","canceled":true}),
            ),
            &sink,
        );
        assert_eq!(sink.network_activity().0, 1);
        assert!(
            matches!(events.try_recv().unwrap(),PageEvent::RequestFailed {request,..} if request.request_id=="child")
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"new"}),
            ),
            &sink,
        );
        assert_eq!(sink.network_activity().0, 0);
        assert!(
            matches!(events.try_recv().unwrap(),PageEvent::RequestFinished(r) if r.request_id=="new")
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"old"}),
            ),
            &sink,
        );
        assert_eq!(sink.network_activity().0, 0);
        assert!(events.try_recv().is_err());
    }
    #[test]
    fn redirects_complete_each_hop_and_terminals_do_not_underflow_idle() {
        let sink = ConsoleSink::new();
        let mut events = sink.subscribe();
        handle_cdp_event(
            &cdp(
                "Network.requestWillBeSent",
                serde_json::json!({"requestId":"id","request":{"method":"GET","url":"http://host/old"}}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.requestWillBeSent",
                serde_json::json!({"requestId":"id","redirectResponse":{"url":"http://host/old","status":302},"request":{"method":"GET","url":"http://host/new"}}),
            ),
            &sink,
        );
        assert_eq!(sink.inflight.load(Ordering::SeqCst), 1);
        handle_cdp_event(
            &cdp(
                "Network.responseReceived",
                serde_json::json!({"requestId":"id","response":{"url":"http://host/new","status":500}}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"id"}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"id"}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFailed",
                serde_json::json!({"requestId":"unknown"}),
            ),
            &sink,
        );
        assert_eq!(sink.inflight.load(Ordering::SeqCst), 0);
        let observed: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
        assert_eq!(
            observed.iter().map(PageEvent::kind).collect::<Vec<_>>(),
            [
                PageEventKind::Request,
                PageEventKind::Response,
                PageEventKind::RequestFinished,
                PageEventKind::Request,
                PageEventKind::Response,
                PageEventKind::RequestFinished
            ]
        );
        assert!(
            matches!(&observed[2], PageEvent::RequestFinished(request) if request.url.ends_with("old"))
        );
    }
    #[test]
    fn cdp_failure_after_headers_keeps_identity_error_and_cancellation() {
        let sink = ConsoleSink::new();
        let mut events = sink.subscribe();
        handle_cdp_event(
            &cdp(
                "Network.requestWillBeSent",
                serde_json::json!({"requestId":"post-id","request":{"method":"POST","url":"http://host/body"}}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.responseReceived",
                serde_json::json!({"requestId":"post-id","response":{"url":"http://host/body","status":200}}),
            ),
            &sink,
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFailed",
                serde_json::json!({"requestId":"post-id","errorText":"net::ERR_ABORTED","canceled":true}),
            ),
            &sink,
        );
        events.try_recv().unwrap();
        events.try_recv().unwrap();
        assert!(
            matches!(events.try_recv().unwrap(), PageEvent::RequestFailed {request, error_text, cancelled:Some(true)} if request.method == "POST" && request.request_id == "post-id" && error_text == "net::ERR_ABORTED")
        );
        assert_eq!(sink.inflight.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn bidi_failure_is_not_a_synthetic_response() {
        let sink = ConsoleSink::new();
        let mut events = sink.subscribe();
        let params = serde_json::json!({"request":{"request":"id","method":"GET","url":"http://host/offline"},"errorText":"connection refused"});
        handle_bidi_event(
            &BidiEvent {
                method: "network.beforeRequestSent".into(),
                params: params.clone(),
            },
            &sink,
        );
        handle_bidi_event(
            &BidiEvent {
                method: "network.fetchError".into(),
                params,
            },
            &sink,
        );
        assert_eq!(events.try_recv().unwrap().kind(), PageEventKind::Request);
        assert!(
            matches!(events.try_recv().unwrap(), PageEvent::RequestFailed {error_text, cancelled:None, ..} if error_text == "connection refused")
        );
        assert!(events.try_recv().is_err());
        assert_eq!(sink.inflight.load(Ordering::SeqCst), 0);
    }
}

#[cfg(test)]
mod diagnostic_observation_tests {
    use super::*;
    #[test]
    fn unpolled_listener_drop_clears_worlds_and_marks_source_loss() {
        let worlds = Arc::new(Mutex::new(HashMap::from([("root".into(), 42)])));
        let sink = ConsoleSink::new();
        let source = sink.native_observation_loss();
        let world_guard = MainWorldListenerGuard(worlds.clone());
        let listener_guard = NetworkListenerGuard(sink);
        let listener = async move {
            let _world_guard = world_guard;
            let _listener_guard = listener_guard;
            std::future::pending::<()>().await;
        };
        drop(listener);
        assert!(worlds.lock().unwrap().is_empty());
        assert!(source.is_cancelled());
    }

    #[test]
    fn native_listener_guard_marks_observation_source_terminal() {
        let sink = ConsoleSink::new();
        let source = sink.native_observation_loss();
        let context_source = Arc::new(tokio::sync::watch::channel(None).0);
        let receiver = context_source.subscribe();
        sink.bind_context_source_loss(&context_source);
        assert_eq!(Arc::strong_count(&context_source), 1);
        assert!(!source.is_cancelled());
        drop(NetworkListenerGuard(sink));
        assert!(receiver.has_changed().unwrap());
        assert!(receiver
            .borrow()
            .as_ref()
            .unwrap()
            .contains("source unavailable"));
        assert_eq!(
            source.reason().as_deref(),
            Some("native event listener ended")
        );
    }

    #[test]
    fn context_source_is_not_retained_and_closed_pages_do_not_report_listener_loss() {
        let sink = ConsoleSink::new();
        let context_source = Arc::new(tokio::sync::watch::channel(None).0);
        let weak = Arc::downgrade(&context_source);
        let receiver = context_source.subscribe();
        sink.bind_context_source_loss(&context_source);
        sink.seed_popup_closed();
        drop(NetworkListenerGuard(sink));
        assert!(!receiver.has_changed().unwrap());
        drop(context_source);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn console_protocol_metadata_is_preserved_in_events_buffers_and_context_history() {
        let sink = ConsoleSink::new();
        *sink.page_id.lock().unwrap() = Some("page-1".into());
        let context_log = Arc::new(Mutex::new(Vec::new()));
        let context_events = tokio::sync::broadcast::channel(16).0;
        sink.forward_context(&context_events, "page-1", &context_log);
        let mut events = sink.subscribe();
        handle_cdp_event(
            &CdpEvent {
                session: None,
                method: "Runtime.consoleAPICalled".into(),
                params: serde_json::json!({"type":"log","args":[{"value":"message"}],"timestamp":1234.9,"stackTrace":{"callFrames":[{"url":"https://app/script.js","lineNumber":0,"columnNumber":7}]}}),
            },
            &sink,
        );
        let PageEvent::Console(message) = events.try_recv().unwrap() else {
            panic!("console event");
        };
        assert_eq!(message.location.as_ref().unwrap().line, 0);
        assert_eq!(message.location.as_ref().unwrap().column, 7);
        assert_eq!(message.timestamp_ms, Some(1234));
        assert_eq!(message.page_id.as_deref(), Some("page-1"));
        assert_eq!(
            context_log.lock().unwrap()[0].timestamp_ms,
            message.timestamp_ms
        );
        handle_bidi_event(
            &BidiEvent {
                method: "log.entryAdded".into(),
                params: serde_json::json!({"type":"javascript","text":"Error: boom","timestamp":4321,"source":{"context":"page-1"},"stackTrace":{"callFrames":[{"url":"https://app/error.js","lineNumber":4,"columnNumber":2}]}}),
            },
            &sink,
        );
        let PageEvent::Console(error) = events.try_recv().unwrap() else {
            panic!("console error event");
        };
        assert_eq!(error.kind, "exception");
        assert_eq!(error.location.unwrap().url, "https://app/error.js");
        assert_eq!(error.timestamp_ms, Some(4321));
        let unknown = console_metadata("log".into(), "unknown".into(), &Value::Null, &Value::Null);
        assert!(unknown.location.is_none() && unknown.timestamp_ms.is_none());
        let legacy: ConsoleMessage =
            serde_json::from_value(serde_json::json!({"kind":"log","text":"old"})).unwrap();
        assert!(
            legacy.location.is_none() && legacy.page_id.is_none() && legacy.timestamp_ms.is_none()
        );
    }
    #[test]
    fn live_network_observations_precede_capture_and_keep_redirect_identity() {
        let sink = ConsoleSink::new();
        let mut events = sink.subscribe_network();
        let cdp = |method: &str, params| CdpEvent {
            session: None,
            method: method.into(),
            params,
        };
        handle_cdp_event(
            &cdp(
                "Network.requestWillBeSent",
                serde_json::json!({"requestId":"r","request":{"method":"POST","url":"https://app/old","headers":{"x-key":"yes"},"postData":"body"}}),
            ),
            &sink,
        );
        let request = events.try_recv().unwrap();
        assert!(!request.response);
        assert_eq!(request.request.status, 0);
        assert_eq!(request.request.post_data.as_deref(), Some("body"));
        assert!(sink.requests().is_empty());
        handle_cdp_event(
            &cdp(
                "Network.requestWillBeSent",
                serde_json::json!({"requestId":"r","redirectResponse":{"url":"https://app/old","status":302,"headers":{"location":"https://app/new"}},"request":{"method":"GET","url":"https://app/new"}}),
            ),
            &sink,
        );
        let redirect = events.try_recv().unwrap();
        assert!(redirect.response);
        assert_eq!(redirect.request.status, 302);
        assert_eq!(redirect.request.method, "POST");
        let next = events.try_recv().unwrap();
        assert!(!next.response);
        assert_eq!(next.request.method, "GET");
        handle_cdp_event(
            &cdp(
                "Network.responseReceived",
                serde_json::json!({"requestId":"r","response":{"url":"https://app/new","status":200,"headers":{"x-reply":"ready"}}}),
            ),
            &sink,
        );
        let response = events.try_recv().unwrap();
        assert!(response.response);
        assert_eq!(response.request.request_id.as_deref(), Some("r"));
        assert_eq!(
            response.request.response_headers,
            [("x-reply".into(), "ready".into())]
        );
        handle_cdp_event(
            &cdp(
                "Network.loadingFinished",
                serde_json::json!({"requestId":"r"}),
            ),
            &sink,
        );
        assert!(sink.observed_network.lock().unwrap().is_empty());
    }
}
