//! Engine drivers: CDP (Chromium) and WebDriver BiDi (Firefox).
//!
//! [`Driver`] is the engine-agnostic surface [`Page`](crate::Page) programs
//! against. Engine differences (sessions vs contexts, RemoteValue decoding,
//! interception mechanisms) stay inside the two drivers.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::bidi::{bytes_to_string, remote_to_json, BidiConnection, BidiEvent};
use crate::cdp::{CdpConnection, CdpEvent};
use crate::error::{E2eError, E2eResult};
use crate::page::{ConsoleMessage, Cookie, LoadState, RouteAction, RouteRule, TraceEntry};

/// Sinks shared between a page and its driver's background listeners.
#[derive(Clone)]
pub struct ConsoleSink {
    /// Console messages and page errors.
    pub console: Arc<Mutex<Vec<ConsoleMessage>>>,
    /// Action/navigation/console trace.
    pub trace: Arc<Mutex<Vec<TraceEntry>>>,
    /// In-flight network requests (network-idle waits).
    pub inflight: Arc<AtomicUsize>,
}

impl ConsoleSink {
    /// Empty sinks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            console: Arc::new(Mutex::new(Vec::new())),
            trace: Arc::new(Mutex::new(Vec::new())),
            inflight: Arc::new(AtomicUsize::new(0)),
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
    ) -> E2eResult<Self> {
        let driver = Self {
            cdp,
            session,
            target,
            timeout,
            sink,
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
    ) -> Self {
        let driver = Self {
            bidi,
            context,
            timeout,
            insecure_certs,
            intercept: Arc::new(Mutex::new(None)),
            sink,
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

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        match self {
            Self::Cdp(driver) => driver.set_user_agent(user_agent).await,
            Self::Bidi(driver) => driver.set_user_agent(user_agent).await,
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
    pub async fn start_dialogs(&self, accept: bool) -> E2eResult<tokio::task::AbortHandle> {
        match self {
            Self::Cdp(driver) => Ok(driver.start_dialogs(accept)),
            Self::Bidi(driver) => Ok(driver.start_dialogs(accept)),
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
        let result = self
            .call(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;
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
        let frame_id = tree["frame"]["id"].clone();
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

    async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.call(
            "Emulation.setUserAgentOverride",
            serde_json::json!({ "userAgent": user_agent }),
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

    async fn clear_cookies(&self) -> E2eResult<()> {
        self.call("Network.clearBrowserCookies", Value::Null)
            .await?;
        Ok(())
    }

    async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        Ok(())
    }

    async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.mouse_move(x, y).await?;
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": kind, "x": x, "y": y,
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
        for kind in ["rawKeyDown", "keyUp"] {
            let mut params = serde_json::json!({
                "type": kind,
                "windowsVirtualKeyCode": windows_code,
                "key": key_name,
                "code": code,
            });
            if kind == "rawKeyDown" && key.chars().count() == 1 {
                params["text"] = Value::String(key.to_string());
            }
            self.call("Input.dispatchKeyEvent", params).await?;
        }
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
        self.call(
            "Fetch.enable",
            serde_json::json!({ "patterns": [{ "urlPattern": "*" }] }),
        )
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
                let (method, params) = match set.matches(&url).first().map(|i| &rules[*i].action) {
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
                    _ => (
                        "Fetch.continueRequest",
                        serde_json::json!({ "requestId": request_id }),
                    ),
                };
                let _ = cdp.call(Some(&session), method, params, timeout).await;
            }
        });
        Ok(handle.abort_handle())
    }

    async fn stop_routing(&self) {
        let _ = self.call("Fetch.disable", Value::Null).await;
    }

    fn start_dialogs(&self, accept: bool) -> tokio::task::AbortHandle {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let cdp = self.cdp.clone();
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
                let _ = cdp
                    .call(
                        Some(&session),
                        "Page.handleJavaScriptDialog",
                        serde_json::json!({ "accept": accept }),
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
        let result = self
            .bidi
            .call(
                "script.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "target": { "context": self.context },
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
        // Firefox rejects `*` in URL patterns, so intercept everything with
        // the empty match-all pattern and filter client-side with globset.
        let added = self
            .bidi
            .call(
                "network.addIntercept",
                serde_json::json!({
                    "phases": ["beforeRequestSent"],
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
                let (method, params) = match set.matches(&url).first().map(|i| &rules[*i].action) {
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
                    _ => (
                        "network.continueRequest",
                        serde_json::json!({ "request": request }),
                    ),
                };
                let _ = bidi.call(method, params, timeout).await;
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

    fn start_dialogs(&self, accept: bool) -> tokio::task::AbortHandle {
        let mut events = self.bidi.subscribe();
        let context = self.context.clone();
        let bidi = self.bidi.clone();
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
                let _ = bidi
                    .call(
                        "browsingContext.handleUserPrompt",
                        serde_json::json!({ "context": context, "accept": accept }),
                        timeout,
                    )
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
        other => other.to_string(),
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
    use base64::Engine as _;
    let data = value
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| E2eError::Cdp {
            method: "captureScreenshot".to_string(),
            message: error.to_string(),
        })
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
}
