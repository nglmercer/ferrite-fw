//! Engine drivers: CDP (Chromium) and WebDriver BiDi (Firefox).
//!
//! [`Driver`] is the engine-agnostic surface [`Page`](crate::Page) programs
//! against. Engine differences (sessions vs contexts, RemoteValue decoding,
//! interception mechanisms) stay inside the two drivers.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::bidi::{bytes_to_string, remote_to_json, BidiConnection, BidiEvent};
use crate::cdp::{CdpConnection, CdpEvent};
use crate::error::{E2eError, E2eResult};
use crate::page::{
    ColorScheme, ConsoleMessage, Cookie, DialogInfo, ElementRect, FrameInfo, LoadState,
    RecordedRequest, ReducedMotion, RouteAction, RouteRule, TraceEntry,
};
use crate::video::{assemble_webm, SpooledFrame, VideoFrame, VideoOptions};

/// Sinks shared between a page and its driver's background listeners.
#[derive(Clone)]
pub struct ConsoleSink {
    /// Console messages and page errors.
    pub console: Arc<Mutex<Vec<ConsoleMessage>>>,
    /// Action/navigation/console trace.
    pub trace: Arc<Mutex<Vec<TraceEntry>>>,
    /// In-flight network requests (network-idle waits).
    pub inflight: Arc<AtomicUsize>,
    /// Dialogs observed while auto-handling (oldest first).
    pub dialogs: Arc<Mutex<Vec<DialogInfo>>>,
    /// Recorded network requests (oldest first, capped).
    requests: Arc<Mutex<VecDeque<RecordedRequest>>>,
}

/// Maximum recorded requests per page (oldest dropped first).
const MAX_RECORDED_REQUESTS: usize = 4096;

impl ConsoleSink {
    /// Empty sinks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            console: Arc::new(Mutex::new(Vec::new())),
            trace: Arc::new(Mutex::new(Vec::new())),
            inflight: Arc::new(AtomicUsize::new(0)),
            dialogs: Arc::new(Mutex::new(Vec::new())),
            requests: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Record a console message (also appended to the trace).
    pub fn push_console(&self, kind: String, text: String) {
        if let Ok(mut console) = self.console.lock() {
            console.push(ConsoleMessage {
                kind: kind.clone(),
                text: text.clone(),
            });
        }
        self.record("console", format!("{kind}: {text}"));
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
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        if let Ok(mut trace) = self.trace.lock() {
            trace.push(TraceEntry {
                ts_ms,
                kind: kind.to_string(),
                detail,
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
    cdp: CdpConnection,
    session: String,
    target: String,
    timeout: Duration,
    sink: ConsoleSink,
    /// Held modifier bitmask (CDP does not track it across calls).
    modifiers: Arc<Mutex<u8>>,
    /// Whether the left button is held (CDP moves default to buttons=0,
    /// which breaks pointer capture and buttons-gated drag handlers).
    pressed: Arc<Mutex<bool>>,
    /// Owning browser context (`None` = default; scopes permission grants).
    browser_context: Option<String>,
}

/// BiDi-backed driver (Firefox): one browsing context.
#[derive(Clone)]
pub struct BidiDriver {
    bidi: BidiConnection,
    context: String,
    timeout: Duration,
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
    /// Target/context id for diagnostics.
    #[must_use]
    pub fn target_id(&self) -> &str {
        match self {
            Self::Cdp(driver) => &driver.target,
            Self::Bidi(driver) => &driver.context,
        }
    }

    /// Default protocol timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        match self {
            Self::Cdp(driver) => driver.timeout,
            Self::Bidi(driver) => driver.timeout,
        }
    }

    /// Override the protocol timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        match self {
            Self::Cdp(driver) => driver.timeout = timeout,
            Self::Bidi(driver) => driver.timeout = timeout,
        }
    }
}

// --- construction ------------------------------------------------------------

impl CdpDriver {
    /// Enable domains and spawn the event listener.
    pub async fn spawn(
        cdp: CdpConnection,
        session: String,
        target: String,
        timeout: Duration,
        sink: ConsoleSink,
        browser_context: Option<String>,
    ) -> E2eResult<Self> {
        let driver = Self {
            cdp,
            session,
            target,
            timeout,
            sink,
            modifiers: Arc::new(Mutex::new(0)),
            pressed: Arc::new(Mutex::new(false)),
            browser_context,
        };
        driver.call("Page.enable", Value::Null).await?;
        driver.call("Runtime.enable", Value::Null).await?;
        driver.call("Log.enable", Value::Null).await?;
        driver.call("Network.enable", Value::Null).await?;
        driver.spawn_listener();
        Ok(driver)
    }

    fn spawn_listener(&self) {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let sink = self.sink.clone();
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                handle_cdp_event(&event, &sink);
            }
        });
    }
}

impl BidiDriver {
    /// Spawn the event listener (session-wide subscription already active).
    pub fn spawn(
        bidi: BidiConnection,
        context: String,
        timeout: Duration,
        insecure_certs: bool,
        sink: ConsoleSink,
        user_context: Option<String>,
    ) -> Self {
        let driver = Self {
            bidi,
            context,
            timeout,
            insecure_certs,
            intercept: Arc::new(Mutex::new(None)),
            sink,
            user_context,
        };
        driver.spawn_listener();
        driver
    }

    fn spawn_listener(&self) {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let sink = self.sink.clone();
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.context() != Some(context.as_str()) {
                    continue;
                }
                handle_bidi_event(&event, &sink);
            }
        });
    }
}

// --- Driver dispatch ---------------------------------------------------------

impl Driver {
    /// Navigate and wait for the load state.
    pub async fn navigate(&self, url: &str, wait: LoadState, timeout: Duration) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.navigate(url, wait, timeout).await,
            Self::Bidi(driver) => driver.navigate(url, wait, timeout).await,
        }
    }

    /// Reload the page.
    pub async fn reload(&self) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.reload().await,
            Self::Bidi(driver) => driver.reload().await,
        }
    }

    /// Traverse history by `delta` entries.
    pub async fn traverse(&self, delta: i64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.traverse(delta).await,
            Self::Bidi(driver) => driver.traverse(delta).await,
        }
    }

    /// Evaluate JavaScript, returning plain JSON.
    pub async fn evaluate(&self, expression: &str) -> E2eResult<Value> {
        match self {
            Self::Cdp(driver) => driver.evaluate(expression).await,
            Self::Bidi(driver) => driver.evaluate(expression).await,
        }
    }

    /// Bring the page to front.
    pub async fn bring_to_front(&self) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.bring_to_front().await,
            Self::Bidi(driver) => driver.bring_to_front().await,
        }
    }

    /// Set the document HTML.
    pub async fn set_content(&self, html: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_content(html).await,
            Self::Bidi(driver) => driver.set_content(html).await,
        }
    }

    /// Capture a screenshot.
    pub async fn screenshot(&self, full_page: bool, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        match self {
            Self::Cdp(driver) => driver.screenshot(full_page, quality).await,
            Self::Bidi(driver) => driver.screenshot(full_page, quality).await,
        }
    }

    /// Print to PDF bytes.
    pub async fn print_pdf(&self) -> E2eResult<Vec<u8>> {
        match self {
            Self::Cdp(driver) => driver.print_pdf().await,
            Self::Bidi(driver) => driver.print_pdf().await,
        }
    }

    /// Set the viewport size.
    pub async fn set_viewport(&self, width: u32, height: u32) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_viewport(width, height).await,
            Self::Bidi(driver) => driver.set_viewport(width, height).await,
        }
    }

    /// Emulate a device (Chromium: metrics + touch; Firefox: unsupported).
    pub async fn emulate_device(&self, device: crate::page::DeviceDescriptor) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.emulate_device(device).await,
            Self::Bidi(driver) => driver.emulate_device(device).await,
        }
    }

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_user_agent(user_agent).await,
            Self::Bidi(driver) => driver.set_user_agent(user_agent).await,
        }
    }

    /// Direct downloads to `dir` (Chromium; Firefox is launch-time only).
    pub async fn set_download_dir(&self, dir: &Path) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_download_dir(dir).await,
            Self::Bidi(driver) => driver.set_download_dir(dir).await,
        }
    }

    /// Run `source` before page scripts in every future document.
    pub async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.add_init_script(source).await,
            Self::Bidi(driver) => driver.add_init_script(source).await,
        }
    }

    /// List frames (main frame first).
    pub async fn frames(&self) -> E2eResult<Vec<FrameInfo>> {
        match self {
            Self::Cdp(driver) => driver.frames().await,
            Self::Bidi(driver) => driver.frames().await,
        }
    }

    /// Evaluate in a frame by listing id.
    pub async fn frame_evaluate(&self, frame_id: &str, expression: &str) -> E2eResult<Value> {
        match self {
            Self::Cdp(driver) => driver.frame_evaluate(frame_id, expression).await,
            Self::Bidi(driver) => driver.frame_evaluate(frame_id, expression).await,
        }
    }

    /// Ignore HTTPS certificate errors.
    pub async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_ignore_https_errors(ignore).await,
            Self::Bidi(driver) => driver.set_ignore_https_errors(ignore).await,
        }
    }

    /// Visible cookies.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        match self {
            Self::Cdp(driver) => driver.cookies().await,
            Self::Bidi(driver) => driver.cookies().await,
        }
    }

    /// Set a cookie (url hint for domain derivation).
    pub async fn set_cookie(&self, name: &str, value: &str, url: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_cookie(name, value, url).await,
            Self::Bidi(driver) => driver.set_cookie(name, value, url).await,
        }
    }

    /// Set full-fidelity cookies (domain defaults to `url`'s host).
    pub async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.add_cookies(cookies, url).await,
            Self::Bidi(driver) => driver.add_cookies(cookies, url).await,
        }
    }

    /// Clear cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.clear_cookies().await,
            Self::Bidi(driver) => driver.clear_cookies().await,
        }
    }

    /// Move the mouse.
    pub async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_move(x, y).await,
            Self::Bidi(driver) => driver.mouse_move(x, y).await,
        }
    }

    /// Trusted click at coordinates.
    pub async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_click(x, y, click_count).await,
            Self::Bidi(driver) => driver.mouse_click(x, y, click_count).await,
        }
    }

    /// Insert text at the focused element.
    pub async fn insert_text(&self, text: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.insert_text(text).await,
            Self::Bidi(driver) => driver.insert_text(text).await,
        }
    }

    /// Dispatch a key press.
    pub async fn press_key(&self, key: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.press_key(key).await,
            Self::Bidi(driver) => driver.press_key(key).await,
        }
    }

    /// Press the left mouse button at coordinates.
    pub async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_down(x, y).await,
            Self::Bidi(driver) => driver.mouse_down(x, y).await,
        }
    }

    /// Release the left mouse button at coordinates.
    pub async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_up(x, y).await,
            Self::Bidi(driver) => driver.mouse_up(x, y).await,
        }
    }

    /// Drag from one point to another in `steps` paced moves.
    pub async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_drag(from, to, steps).await,
            Self::Bidi(driver) => driver.mouse_drag(from, to, steps).await,
        }
    }

    /// Scroll a wheel at coordinates by (`delta_x`, `delta_y`).
    pub async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.mouse_wheel(x, y, delta_x, delta_y).await,
            Self::Bidi(driver) => driver.mouse_wheel(x, y, delta_x, delta_y).await,
        }
    }

    /// Hold a key down (pair with [`Driver::key_up`]).
    pub async fn key_down(&self, key: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.key_down(key).await,
            Self::Bidi(driver) => driver.key_down(key).await,
        }
    }

    /// Release a held key.
    pub async fn key_up(&self, key: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.key_up(key).await,
            Self::Bidi(driver) => driver.key_up(key).await,
        }
    }

    /// Tap at coordinates with the touchscreen.
    pub async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.touchscreen_tap(x, y).await,
            Self::Bidi(driver) => driver.touchscreen_tap(x, y).await,
        }
    }

    /// Grant permissions.
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.grant_permissions(permissions).await,
            Self::Bidi(driver) => driver.grant_permissions(permissions).await,
        }
    }

    /// Override the geolocation coordinates.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_geolocation(latitude, longitude).await,
            Self::Bidi(driver) => driver.set_geolocation(latitude, longitude).await,
        }
    }

    /// Emulate offline mode.
    pub async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_offline(offline).await,
            Self::Bidi(driver) => driver.set_offline(offline).await,
        }
    }

    /// Set extra HTTP headers for subsequent requests.
    pub async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_extra_http_headers(headers).await,
            Self::Bidi(driver) => driver.set_extra_http_headers(headers).await,
        }
    }

    /// Override the locale.
    pub async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_locale(locale).await,
            Self::Bidi(driver) => driver.set_locale(locale).await,
        }
    }

    /// Override the timezone.
    pub async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_timezone(timezone_id).await,
            Self::Bidi(driver) => driver.set_timezone(timezone_id).await,
        }
    }

    /// Emulate media features.
    pub async fn emulate_media(
        &self,
        color_scheme: Option<ColorScheme>,
        reduced_motion: Option<ReducedMotion>,
    ) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.emulate_media(color_scheme, reduced_motion).await,
            Self::Bidi(driver) => driver.emulate_media(color_scheme, reduced_motion).await,
        }
    }

    /// Start intercepting requests; returns the handler task handle.
    pub async fn start_routing(
        &self,
        rules: Arc<Vec<RouteRule>>,
    ) -> E2eResult<tokio::task::AbortHandle> {
        match self {
            Self::Cdp(driver) => driver.start_routing(rules).await,
            Self::Bidi(driver) => driver.start_routing(rules).await,
        }
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
    ) -> E2eResult<tokio::task::AbortHandle> {
        match self {
            Self::Cdp(driver) => Ok(driver.start_dialogs(accept, prompt_text)),
            Self::Bidi(driver) => Ok(driver.start_dialogs(accept, prompt_text)),
        }
    }

    /// Wait for a document load state.
    pub async fn wait_for_load(&self, state: LoadState, timeout: Duration) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.wait_for_load(state, timeout).await,
            Self::Bidi(driver) => driver.wait_for_load(state, timeout).await,
        }
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
        match self {
            Self::Cdp(driver) => driver.raw(method, params, timeout).await,
            Self::Bidi(driver) => driver.raw(method, params, timeout).await,
        }
    }

    /// Start a live frame stream (screencast on Chromium, paced
    /// screenshots on Firefox). Returns the stream plus the pump handle;
    /// call [`Driver::stop_frame_stream`] after aborting the pump.
    pub async fn start_frame_stream(
        &self,
        opts: &VideoOptions,
    ) -> E2eResult<(FrameStream, tokio::task::AbortHandle)> {
        match self {
            Self::Cdp(driver) => driver.start_frame_stream(opts).await,
            Self::Bidi(driver) => driver.start_frame_stream(opts).await,
        }
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
        match self {
            Self::Cdp(driver) => driver.start_recording(opts).await,
            Self::Bidi(driver) => driver.start_recording(opts).await,
        }
    }

    /// Stop a recording and produce the output video.
    pub async fn stop_recording(
        &self,
        state: RecordingState,
        output: &std::path::Path,
    ) -> E2eResult<PathBuf> {
        match self {
            Self::Cdp(driver) => driver.stop_recording(state, output).await,
            Self::Bidi(driver) => driver.stop_recording(state, output).await,
        }
    }

    /// Discard a recording without producing output.
    pub async fn cancel_recording(&self, state: RecordingState) {
        match self {
            Self::Cdp(driver) => driver.cancel_recording(state).await,
            Self::Bidi(driver) => driver.cancel_recording(state).await,
        }
    }

    /// Screenshot one element box.
    pub async fn screenshot_clip(
        &self,
        rect: &ElementRect,
        quality: Option<u8>,
    ) -> E2eResult<Vec<u8>> {
        match self {
            Self::Cdp(driver) => driver.screenshot_clip(rect, quality).await,
            Self::Bidi(driver) => driver.screenshot_clip(rect, quality).await,
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
            .call(Some(&self.session), method, params, self.timeout)
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
        self.wait_for_load(LoadState::Load, self.timeout).await
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
        self.wait_for_load(LoadState::Load, self.timeout).await
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
            let text = exception
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("js exception");
            return Err(E2eError::Cdp {
                method: "Runtime.evaluate".to_string(),
                message: format!("{text}: {expression}"),
            });
        }
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
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

    async fn evaluate_string(&self, expression: &str) -> E2eResult<String> {
        Ok(self
            .evaluate(expression)
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
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

    async fn screenshot(&self, full_page: bool, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        if full_page {
            return self.full_page_screenshot(quality).await;
        }
        let mut params = serde_json::json!({ "captureBeyondViewport": true });
        if let Some(quality) = quality {
            params["format"] = Value::String("jpeg".to_string());
            params["quality"] = Value::from(quality);
        }
        let shot = self.call("Page.captureScreenshot", params).await?;
        decode_shot(&shot)
    }

    async fn full_page_screenshot(&self, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        let metrics = self.call("Page.getLayoutMetrics", Value::Null).await?;
        let size = &metrics["contentSize"];
        let width = size.get("width").and_then(Value::as_u64).unwrap_or(1280);
        let height = size.get("height").and_then(Value::as_u64).unwrap_or(800);
        self.call(
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({
                "width": width, "height": height,
                "deviceScaleFactor": 1, "mobile": false,
            }),
        )
        .await?;
        let mut params = serde_json::json!({ "captureBeyondViewport": true });
        if let Some(quality) = quality {
            params["format"] = Value::String("jpeg".to_string());
            params["quality"] = Value::from(quality);
        }
        let shot = self.call("Page.captureScreenshot", params).await;
        let _ = self
            .call("Emulation.clearDeviceMetricsOverride", Value::Null)
            .await;
        decode_shot(&shot?)
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
        self.cdp
            .call(
                None,
                "Browser.setDownloadBehavior",
                serde_json::json!({
                    "behavior": "allow",
                    "downloadPath": dir.to_string_lossy(),
                }),
                self.timeout,
            )
            .await?;
        Ok(())
    }

    async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        self.call(
            "Page.addScriptToEvaluateOnNewDocument",
            serde_json::json!({ "source": source }),
        )
        .await?;
        Ok(())
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
        let cookies = self.call("Network.getCookies", Value::Null).await?;
        let list = cookies
            .get("cookies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        serde_json::from_value(Value::Array(list)).map_err(E2eError::Json)
    }

    async fn set_cookie(&self, name: &str, value: &str, url: &str) -> E2eResult<()> {
        self.call(
            "Network.setCookie",
            serde_json::json!({ "name": name, "value": value, "url": url }),
        )
        .await?;
        Ok(())
    }

    async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        for cookie in cookies {
            let mut params = serde_json::json!({
                "name": cookie.name, "value": cookie.value, "url": url,
            });
            if let Some(path) = &cookie.path {
                params["path"] = Value::String(path.clone());
            }
            if cookie.secure {
                params["secure"] = Value::Bool(true);
            }
            if cookie.http_only {
                params["httpOnly"] = Value::Bool(true);
            }
            if let Some(expires) = cookie.expires.filter(|expires| *expires >= 0) {
                params["expires"] = Value::from(expires);
            }
            self.call("Network.setCookie", params).await?;
        }
        Ok(())
    }

    async fn clear_cookies(&self) -> E2eResult<()> {
        self.call("Network.clearBrowserCookies", Value::Null)
            .await?;
        Ok(())
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

    async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        let modifiers = self.held_modifiers();
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": kind, "x": x, "y": y, "modifiers": modifiers,
                    "button": "left", "clickCount": click_count.max(1),
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

    async fn press_key(&self, key: &str) -> E2eResult<()> {
        let (windows_code, key_name, code) = key_definition(key);
        if key.chars().count() == 1 && windows_code == 0 {
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
            if kind == "rawKeyDown" && key.chars().count() == 1 {
                params["text"] = Value::String(key.to_string());
            }
            self.call("Input.dispatchKeyEvent", params).await?;
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
        // Report the release with the key still held, then clear it.
        let modifiers = self.held_modifiers();
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

    async fn emulate_media(
        &self,
        color_scheme: Option<ColorScheme>,
        reduced_motion: Option<ReducedMotion>,
    ) -> E2eResult<()> {
        let mut features = Vec::new();
        if let Some(scheme) = color_scheme {
            let value = match scheme {
                ColorScheme::Dark => "dark",
                ColorScheme::Light => "light",
            };
            features.push(serde_json::json!({ "name": "prefers-color-scheme", "value": value }));
        }
        if let Some(motion) = reduced_motion {
            let value = match motion {
                ReducedMotion::Reduce => "reduce",
                ReducedMotion::NoPreference => "no-preference",
            };
            features.push(serde_json::json!({ "name": "prefers-reduced-motion", "value": value }));
        }
        if features.is_empty() {
            return Ok(());
        }
        self.call(
            "Emulation.setEmulatedMedia",
            serde_json::json!({ "features": features }),
        )
        .await?;
        Ok(())
    }

    async fn start_routing(
        &self,
        rules: Arc<Vec<RouteRule>>,
    ) -> E2eResult<tokio::task::AbortHandle> {
        use globset::{Glob, GlobSetBuilder};
        let mut builder = GlobSetBuilder::new();
        for rule in rules.iter() {
            let glob =
                Glob::new(&rule.pattern).map_err(|error| E2eError::Config(error.to_string()))?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|error| E2eError::Config(error.to_string()))?;
        let mut patterns = vec![serde_json::json!({ "urlPattern": "*" })];
        if rules
            .iter()
            .any(|rule| matches!(rule.action, RouteAction::ModifyResponse { .. }))
        {
            patterns.push(serde_json::json!({ "urlPattern": "*", "requestStage": "Response" }));
        }
        self.call("Fetch.enable", serde_json::json!({ "patterns": patterns }))
            .await?;
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let timeout = self.timeout;
        let handle = tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session)
                    || event.method != "Fetch.requestPaused"
                {
                    continue;
                }
                let request_id = event.params["requestId"].clone();
                let url = event.params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let action = set.matches(&url).first().map(|i| &rules[*i].action);
                if event.params.get("responseStatusCode").is_some()
                    || event.params.get("responseHeaders").is_some()
                {
                    answer_response_pause(
                        &cdp,
                        &session,
                        timeout,
                        &request_id,
                        &event.params,
                        action,
                    )
                    .await;
                    continue;
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
                    Some(RouteAction::Fulfill {
                        status,
                        body,
                        content_type,
                    }) => (
                        "Fetch.fulfillRequest",
                        serde_json::json!({
                            "requestId": request_id,
                            "responseCode": status,
                            "body": base64_encode(body.as_bytes()),
                            "responseHeaders": [
                                { "name": "Content-Type", "value": content_type },
                            ],
                        }),
                    ),
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
                let result = cdp.call(Some(&session), method, params, timeout).await;
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
            }
        });
        Ok(handle.abort_handle())
    }

    async fn stop_routing(&self) {
        let _ = self.call("Fetch.disable", Value::Null).await;
    }

    fn start_request_capture(&self) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let sink = self.sink.clone();
        tokio::spawn(async move {
            let mut pending: HashMap<String, PendingRequest> = HashMap::new();
            loop {
                let event = match events.recv().await {
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
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status,
                                headers: request.headers,
                                post_data: request.post_data,
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                            });
                        }
                    }
                    "Network.loadingFailed" => {
                        let id = event.params["requestId"].as_str().unwrap_or_default();
                        if let Some(request) = pending.remove(id) {
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status: 0,
                                headers: request.headers,
                                post_data: request.post_data,
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
        .abort_handle()
    }

    fn start_dialogs(&self, accept: bool, prompt_text: Option<String>) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout;
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
                sink.push_dialog(DialogInfo {
                    dialog_type: event.params["type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    message: event.params["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                });
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
        let deadline = tokio::time::Instant::now() + timeout;
        if self.load_state_satisfied(state).await {
            return settle_quiet(&self.sink.inflight, state, deadline).await;
        }
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait for {want}"),
                ));
            }
            match tokio::time::timeout(remaining, events.recv()).await {
                Ok(Ok(event)) => {
                    if event.session.as_deref() == Some(&self.session) && event.method == want {
                        return settle_quiet(&self.sink.inflight, state, deadline).await;
                    }
                }
                Ok(Err(_)) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(_) => {
                    if self.load_state_satisfied(state).await {
                        return settle_quiet(&self.sink.inflight, state, deadline).await;
                    }
                    return Err(E2eError::Timeout(
                        timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                        format!("wait for {want}"),
                    ));
                }
            }
        }
    }

    async fn load_state_satisfied(&self, state: LoadState) -> bool {
        let ready = self
            .evaluate_string("document.readyState")
            .await
            .unwrap_or_default();
        match state {
            LoadState::Commit => true,
            LoadState::DomContentLoaded => ready == "interactive" || ready == "complete",
            LoadState::Load | LoadState::NetworkIdle => ready == "complete",
        }
    }

    async fn close(&self) -> E2eResult<()> {
        let _ = self
            .cdp
            .call(
                None,
                "Target.detachFromTarget",
                serde_json::json!({ "sessionId": self.session }),
                Duration::from_secs(5),
            )
            .await;
        let _ = self
            .cdp
            .call(
                None,
                "Target.closeTarget",
                serde_json::json!({ "targetId": self.target }),
                Duration::from_secs(5),
            )
            .await;
        Ok(())
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
        let timeout = self.timeout;
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
        let timeout = self.timeout;
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

    async fn screenshot_clip(&self, rect: &ElementRect, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        let mut params = serde_json::json!({
            "clip": {
                "x": rect.x, "y": rect.y,
                "width": rect.width, "height": rect.height,
                "scale": 1,
            },
            "captureBeyondViewport": true,
        });
        if let Some(quality) = quality {
            params["format"] = Value::String("jpeg".to_string());
            params["quality"] = Value::from(quality);
        }
        let shot = self.call("Page.captureScreenshot", params).await?;
        decode_shot(&shot)
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
            .call(method, params, self.timeout)
            .await
            .map_err(|error| match error {
                E2eError::Cdp { message, .. } => E2eError::Cdp {
                    method: method.to_string(),
                    message,
                },
                other => other,
            })
    }

    async fn evaluate_string(&self, expression: &str) -> E2eResult<String> {
        Ok(self
            .evaluate(expression)
            .await?
            .as_str()
            .unwrap_or_default()
            .to_string())
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
        let deadline = tokio::time::Instant::now() + timeout;
        settle_quiet(&self.sink.inflight, wait, deadline).await
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
                self.timeout,
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
                self.timeout,
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

    async fn screenshot(&self, full_page: bool, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        let origin = if full_page { "document" } else { "viewport" };
        let mut params = serde_json::json!({ "origin": origin });
        if let Some(quality) = quality {
            params["format"] = serde_json::json!({
                "type": "image/jpeg",
                "quality": f64::from(quality).clamp(1.0, 100.0) / 100.0,
            });
        }
        let shot = self
            .call("browsingContext.captureScreenshot", params)
            .await?;
        decode_shot(&shot)
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

    async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        // BiDi preload scripts must be function declarations.
        let function = format!("() => {{ {source} }}");
        self.bidi
            .call(
                "script.addPreloadScript",
                serde_json::json!({
                    "functionDeclaration": function,
                    "contexts": [self.context],
                }),
                self.timeout,
            )
            .await?;
        Ok(())
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
        let result = self
            .bidi
            .call(
                "storage.getCookies",
                serde_json::json!({
                    "partition": { "type": "context", "context": self.context },
                }),
                self.timeout,
            )
            .await?;
        let mut cookies = Vec::new();
        if let Some(list) = result.get("cookies").and_then(Value::as_array) {
            for cookie in list {
                cookies.push(Cookie {
                    name: cookie["name"].as_str().unwrap_or_default().to_string(),
                    value: bytes_to_string(&cookie["value"]),
                    domain: cookie["domain"].as_str().map(str::to_string),
                    path: cookie["path"].as_str().map(str::to_string),
                    http_only: cookie["httpOnly"].as_bool().unwrap_or(false),
                    secure: cookie["secure"].as_bool().unwrap_or(false),
                    expires: cookie["expiry"]
                        .as_i64()
                        .or_else(|| cookie["expiry"].as_str().and_then(|raw| raw.parse().ok())),
                });
            }
        }
        Ok(cookies)
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
                    "partition": { "type": "context", "context": self.context },
                }),
                self.timeout,
            )
            .await?;
        Ok(())
    }

    async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        let host = url_host(url);
        for cookie in cookies {
            let mut params = serde_json::json!({
                "name": cookie.name,
                "value": { "type": "string", "value": cookie.value },
                "path": cookie.path.as_deref().unwrap_or("/"),
            });
            if let Some(domain) = cookie.domain.as_deref().or(host.as_deref()) {
                params["domain"] = Value::String(domain.to_string());
            }
            if cookie.secure {
                params["secure"] = Value::Bool(true);
            }
            if cookie.http_only {
                params["httpOnly"] = Value::Bool(true);
            }
            if let Some(expiry) = cookie.expires.filter(|expiry| *expiry >= 0) {
                params["expiry"] = Value::from(expiry);
            }
            self.bidi
                .call(
                    "storage.setCookie",
                    serde_json::json!({
                        "cookie": params,
                        "partition": { "type": "context", "context": self.context },
                    }),
                    self.timeout,
                )
                .await?;
        }
        Ok(())
    }

    async fn clear_cookies(&self) -> E2eResult<()> {
        self.bidi
            .call(
                "storage.deleteCookies",
                serde_json::json!({
                    "partition": { "type": "context", "context": self.context },
                }),
                self.timeout,
            )
            .await?;
        Ok(())
    }

    async fn perform(&self, actions: Value) -> E2eResult<()> {
        self.bidi
            .call(
                "input.performActions",
                serde_json::json!({ "context": self.context, "actions": actions }),
                self.timeout,
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

    async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.bring_to_front().await?;
        let mut actions = vec![serde_json::json!({ "type": "pointerMove", "x": x, "y": y })];
        for _ in 0..click_count.max(1) {
            actions.push(serde_json::json!({ "type": "pointerDown", "button": 0 }));
            actions.push(serde_json::json!({ "type": "pointerUp", "button": 0 }));
        }
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-mouse",
            "parameters": { "pointerType": "mouse" },
            "actions": actions,
        }]))
        .await
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

    async fn press_key(&self, key: &str) -> E2eResult<()> {
        let value = bidi_key_value(key);
        self.perform(serde_json::json!([{
            "type": "key", "id": "ferrite-keyboard",
            "actions": [
                { "type": "keyDown", "value": value },
                { "type": "keyUp", "value": value },
            ],
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
        // Button state does not survive across `performActions` calls, so the
        // whole drag runs as one action sequence with paced intermediate moves.
        self.bring_to_front().await?;
        let mut actions = vec![
            serde_json::json!({ "type": "pointerMove", "x": from.0, "y": from.1 }),
            serde_json::json!({ "type": "pointerDown", "button": 0 }),
        ];
        for step in 1..=steps.max(1) {
            let t = f64::from(step) / f64::from(steps.max(1));
            actions.push(serde_json::json!({
                "type": "pointerMove",
                "x": from.0 + (to.0 - from.0) * t,
                "y": from.1 + (to.1 - from.1) * t,
                "duration": 16,
            }));
        }
        actions.push(serde_json::json!({ "type": "pointerUp", "button": 0 }));
        self.perform(serde_json::json!([{
            "type": "pointer", "id": "ferrite-mouse",
            "parameters": { "pointerType": "mouse" },
            "actions": actions,
        }]))
        .await
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
        .await
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

    async fn emulate_media(
        &self,
        _color_scheme: Option<ColorScheme>,
        _reduced_motion: Option<ReducedMotion>,
    ) -> E2eResult<()> {
        Err(E2eError::Config(
            "firefox media emulation is not supported \
             (BiDi has no emulated-media override)"
                .to_string(),
        ))
    }

    async fn start_routing(
        &self,
        rules: Arc<Vec<RouteRule>>,
    ) -> E2eResult<tokio::task::AbortHandle> {
        use globset::{Glob, GlobSetBuilder};
        let mut builder = GlobSetBuilder::new();
        for rule in rules.iter() {
            let glob =
                Glob::new(&rule.pattern).map_err(|error| E2eError::Config(error.to_string()))?;
            builder.add(glob);
        }
        let set = builder
            .build()
            .map_err(|error| E2eError::Config(error.to_string()))?;
        // Firefox accepts `url` overrides but aborts the redirected request,
        // so fail fast instead of breaking the page's fetch.
        if rules
            .iter()
            .any(|rule| matches!(&rule.action, RouteAction::ContinueWith { url: Some(_), .. }))
        {
            return Err(E2eError::Config(
                "continue_with url overrides are not supported on Firefox \
                 (BiDi aborts the redirected request)"
                    .to_string(),
            ));
        }
        // Firefox rejects every `provideResponse` override at `responseStarted`
        // (status, headers and body are request-phase-only), so response
        // edits fail fast instead of silently passing the original through.
        if rules
            .iter()
            .any(|rule| matches!(rule.action, RouteAction::ModifyResponse { .. }))
        {
            return Err(E2eError::Config(
                "modify_response is not supported on Firefox \
                 (BiDi provideResponse overrides are request-phase-only)"
                    .to_string(),
            ));
        }
        // Firefox rejects `*` in URL patterns, so intercept everything with
        // the empty match-all pattern and filter client-side with globset.
        // Scoped to this page: a global intercept would block sibling pages
        // whose pumps never see (or no longer handle) the events.
        let added = self
            .bidi
            .call(
                "network.addIntercept",
                serde_json::json!({
                    "phases": ["beforeRequestSent"],
                    "contexts": [self.context.clone()],
                    "urlPatterns": [{ "type": "pattern" }],
                }),
                self.timeout,
            )
            .await?;
        if let Some(intercept) = added.get("intercept").and_then(Value::as_str) {
            *self.intercept.lock().unwrap_or_else(|e| e.into_inner()) = Some(intercept.to_string());
        }
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let bidi = self.bidi.clone();
        let timeout = self.timeout;
        let handle = tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.context() != Some(context.as_str())
                    || event.method != "network.beforeRequestSent"
                    || event.params.get("isBlocked").and_then(Value::as_bool) != Some(true)
                {
                    continue;
                }
                let request = event.params["request"]["request"].clone();
                let url = event.params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let action = set.matches(&url).first().map(|i| &rules[*i].action);
                let is_override = matches!(action, Some(RouteAction::ContinueWith { .. }));
                let (method, params) = match action {
                    Some(RouteAction::Abort) => (
                        "network.failRequest",
                        serde_json::json!({ "request": request }),
                    ),
                    Some(RouteAction::Fulfill {
                        status,
                        body,
                        content_type,
                    }) => (
                        "network.provideResponse",
                        serde_json::json!({
                            "request": request,
                            "statusCode": status,
                            "headers": [{
                                "name": "Content-Type",
                                "value": { "type": "string", "value": content_type },
                            }],
                            "body": { "type": "base64", "value": base64_encode(body.as_bytes()) },
                        }),
                    ),
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
                let result = bidi.call(method, params, timeout).await;
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
            }
        });
        Ok(handle.abort_handle())
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
                            sink.push_request(RecordedRequest {
                                method: request.method,
                                url: request.url,
                                status,
                                headers: request.headers,
                                post_data: request.post_data,
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
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
                                duration_ms: Some(
                                    request
                                        .started
                                        .elapsed()
                                        .as_millis()
                                        .min(u128::from(u64::MAX))
                                        as u64,
                                ),
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
        .abort_handle()
    }

    fn start_dialogs(&self, accept: bool, prompt_text: Option<String>) -> tokio::task::AbortHandle {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let bidi = self.bidi.clone();
        let sink = self.sink.clone();
        let timeout = self.timeout;
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
                sink.push_dialog(DialogInfo {
                    dialog_type: event.params["type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    message: event.params["message"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                });
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
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.load_state_satisfied(state).await {
                return settle_quiet(&self.sink.inflight, state, deadline).await;
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait for {state:?}"),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn load_state_satisfied(&self, state: LoadState) -> bool {
        let ready = self
            .evaluate_string("document.readyState")
            .await
            .unwrap_or_default();
        match state {
            LoadState::Commit => true,
            LoadState::DomContentLoaded => ready == "interactive" || ready == "complete",
            LoadState::Load | LoadState::NetworkIdle => ready == "complete",
        }
    }

    async fn close(&self) -> E2eResult<()> {
        let _ = self
            .bidi
            .call(
                "browsingContext.close",
                serde_json::json!({ "context": self.context }),
                Duration::from_secs(5),
            )
            .await;
        Ok(())
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
        let timeout = self.timeout;
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
                self.timeout,
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

    async fn screenshot_clip(&self, rect: &ElementRect, quality: Option<u8>) -> E2eResult<Vec<u8>> {
        let mut params = serde_json::json!({
            "clip": {
                "type": "box",
                "x": rect.x, "y": rect.y,
                "width": rect.width, "height": rect.height,
            },
        });
        if let Some(quality) = quality {
            params["format"] = serde_json::json!({
                "type": "image/jpeg",
                "quality": f64::from(quality).clamp(1.0, 100.0) / 100.0,
            });
        }
        let shot = self
            .call("browsingContext.captureScreenshot", params)
            .await?;
        decode_shot(&shot)
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

fn handle_bidi_event(event: &BidiEvent, sink: &ConsoleSink) {
    match event.method.as_str() {
        "log.entryAdded" => {
            let entry_type = event.params["type"].as_str().unwrap_or("console");
            let kind = if entry_type == "console" {
                event.params["method"].as_str().unwrap_or("log").to_string()
            } else {
                event.params["level"]
                    .as_str()
                    .unwrap_or("error")
                    .to_string()
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
            sink.push_console(kind, text);
        }
        "network.beforeRequestSent" => {
            sink.inflight.fetch_add(1, Ordering::SeqCst);
        }
        "network.responseCompleted" | "network.fetchError" => {
            sink.inflight.fetch_sub(1, Ordering::SeqCst);
        }
        _ => {}
    }
}

/// Wait for network quiet when `state` is [`LoadState::NetworkIdle`].
async fn settle_quiet(
    inflight: &AtomicUsize,
    state: LoadState,
    deadline: tokio::time::Instant,
) -> E2eResult<()> {
    if state != LoadState::NetworkIdle {
        return Ok(());
    }
    let quiet_for = Duration::from_millis(500);
    let mut quiet_since = None;
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(E2eError::Timeout(0, "network never went idle".to_string()));
        }
        if inflight.load(Ordering::SeqCst) == 0 {
            match quiet_since {
                None => quiet_since = Some(tokio::time::Instant::now()),
                Some(since) if since.elapsed() >= quiet_for => return Ok(()),
                Some(_) => {}
            }
        } else {
            quiet_since = None;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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

/// Collect frames from a CDP frame tree (main frame first, depth-first).
fn collect_cdp_frames(tree: &Value, out: &mut Vec<FrameInfo>) {
    let frame = &tree["frame"];
    out.push(FrameInfo {
        id: frame["id"].as_str().unwrap_or_default().to_string(),
        name: frame["name"].as_str().unwrap_or_default().to_string(),
        url: frame["url"].as_str().unwrap_or_default().to_string(),
    });
    if let Some(children) = tree.get("childFrames").and_then(Value::as_array) {
        for child in children {
            collect_cdp_frames(child, out);
        }
    }
}

/// Collect frames from a BiDi context tree (names are not reported).
fn collect_bidi_frames(node: &Value, out: &mut Vec<FrameInfo>) {
    out.push(FrameInfo {
        id: node["context"].as_str().unwrap_or_default().to_string(),
        name: String::new(),
        url: node["url"].as_str().unwrap_or_default().to_string(),
    });
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_bidi_frames(child, out);
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
}

/// Header pairs from a CDP request object.
fn cdp_header_pairs(headers: &Value) -> Vec<(String, String)> {
    headers
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(name, value)| (name.clone(), value.as_str().unwrap_or_default().to_string()))
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
    match event.method.as_str() {
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
            sink.push_console(kind, args);
        }
        "Runtime.exceptionThrown" => {
            let text = event.params["exceptionDetails"]["text"]
                .as_str()
                .unwrap_or("page exception")
                .to_string();
            sink.push_console("exception".to_string(), text);
        }
        "Log.entryAdded" => {
            let entry = &event.params["entry"];
            let kind = entry["level"].as_str().unwrap_or("log").to_string();
            let text = entry["text"].as_str().unwrap_or_default().to_string();
            sink.push_console(kind, text);
        }
        "Network.requestWillBeSent" => {
            sink.inflight.fetch_add(1, Ordering::SeqCst);
        }
        "Network.loadingFinished" | "Network.loadingFailed" => {
            sink.inflight.fetch_sub(1, Ordering::SeqCst);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
