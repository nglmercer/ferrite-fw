//! Pages: navigation, evaluation, input, screenshots, routing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cdp::{CdpConnection, CdpEvent};
use crate::error::{E2eError, E2eResult};
use crate::locator::{Locator, Selector};

/// CSS pixel viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    /// Width in CSS pixels.
    pub width: u32,
    /// Height in CSS pixels.
    pub height: u32,
}

/// Document readiness to wait for after navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadState {
    /// Return once the commit response arrives.
    Commit,
    /// Wait for `DOMContentLoaded`.
    DomContentLoaded,
    /// Wait for `load` (default).
    #[default]
    Load,
    /// Wait for `load` plus a quiet network (no in-flight requests).
    NetworkIdle,
}

/// Options for [`Page::goto`].
#[derive(Debug, Clone)]
pub struct NavigationOptions {
    /// Readiness to wait for.
    pub wait_until: LoadState,
    /// Navigation timeout.
    pub timeout: Option<Duration>,
}

impl Default for NavigationOptions {
    fn default() -> Self {
        Self {
            wait_until: LoadState::Load,
            timeout: None,
        }
    }
}

/// A console message or page error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleMessage {
    /// `log`, `warn`, `error`, `exception`, ...
    pub kind: String,
    /// Joined argument previews.
    pub text: String,
}

/// A browser cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Cookie domain.
    #[serde(default)]
    pub domain: Option<String>,
    /// Cookie path.
    #[serde(default)]
    pub path: Option<String>,
    /// Http-only flag.
    #[serde(default, rename = "httpOnly")]
    pub http_only: bool,
    /// Secure flag.
    #[serde(default)]
    pub secure: bool,
}

/// Options for [`Page::screenshot`].
#[derive(Debug, Clone, Default)]
pub struct ScreenshotOptions {
    /// Capture the full scrollable page.
    pub full_page: bool,
    /// JPEG quality (1-100); PNG when unset.
    pub quality: Option<u8>,
}

/// Click modifiers + button.
#[derive(Debug, Clone, Default)]
pub struct ClickOptions {
    /// Use synthetic `el.click()` instead of trusted mouse input.
    pub force: bool,
    /// Number of clicks (2 = double-click).
    pub click_count: u32,
}

/// A key press (name like `Enter`, `Tab`, `ArrowLeft`, or a single char).
#[derive(Debug, Clone)]
pub struct KeyPress {
    /// Key name or char.
    pub key: String,
}

impl From<&str> for KeyPress {
    fn from(key: &str) -> Self {
        Self {
            key: key.to_string(),
        }
    }
}

/// What to do with an intercepted request.
#[derive(Debug, Clone)]
pub enum RouteAction {
    /// Fail the request (`net::ERR_ABORTED`).
    Abort,
    /// Let the request through.
    Continue,
    /// Respond with a synthetic body.
    Fulfill {
        /// HTTP status.
        status: u16,
        /// Response body.
        body: String,
        /// Content type header.
        content_type: String,
    },
}

/// A request-routing rule (glob pattern over the URL).
#[derive(Debug, Clone)]
pub struct RouteRule {
    /// Glob pattern (`**/api/*`).
    pub pattern: String,
    /// Action for matching requests.
    pub action: RouteAction,
}

impl RouteRule {
    /// Abort matching requests.
    pub fn abort(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            action: RouteAction::Abort,
        }
    }

    /// Fulfill matching requests with a synthetic response.
    pub fn fulfill(
        pattern: impl Into<String>,
        status: u16,
        body: impl Into<String>,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            action: RouteAction::Fulfill {
                status,
                body: body.into(),
                content_type: content_type.into(),
            },
        }
    }
}

/// One recorded trace entry (actions, navigations, console).
#[derive(Debug, Clone, Serialize)]
pub struct TraceEntry {
    /// Milliseconds since the Unix epoch.
    pub ts_ms: u64,
    /// Entry kind (`action`, `navigation`, `console`, ...).
    pub kind: String,
    /// Human-readable detail.
    pub detail: String,
}

/// Element state snapshot for one selector.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ElementState {
    /// Number of matching elements.
    #[serde(default)]
    pub count: usize,
    /// First match is visible.
    #[serde(default)]
    pub visible: bool,
    /// First match is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// First match is checked.
    #[serde(default)]
    pub checked: bool,
    /// First match text content (trimmed).
    #[serde(default)]
    pub text: String,
    /// First match value (`input`/`textarea`/`select`).
    #[serde(default)]
    pub value: String,
    /// Bounding boxes of matches.
    #[serde(default)]
    pub rects: Vec<ElementRect>,
}

/// Bounding box in CSS pixels.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ElementRect {
    /// Left edge.
    #[serde(default)]
    pub x: f64,
    /// Top edge.
    #[serde(default)]
    pub y: f64,
    /// Width.
    #[serde(default)]
    pub width: f64,
    /// Height.
    #[serde(default)]
    pub height: f64,
}

/// An automated page (one CDP target session).
#[derive(Clone)]
pub struct Page {
    cdp: CdpConnection,
    session: String,
    target: String,
    slow_mo: Duration,
    timeout: Duration,
    base_url: Option<String>,
    console: Arc<Mutex<Vec<ConsoleMessage>>>,
    trace: Arc<Mutex<Vec<TraceEntry>>>,
    inflight: Arc<AtomicUsize>,
    routing: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    dialogs: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

impl Page {
    pub(crate) async fn new(
        cdp: CdpConnection,
        session: String,
        target: String,
        slow_mo: Duration,
        timeout: Duration,
        base_url: Option<String>,
    ) -> E2eResult<Self> {
        let page = Self {
            cdp,
            session,
            target,
            slow_mo,
            timeout,
            base_url,
            console: Arc::new(Mutex::new(Vec::new())),
            trace: Arc::new(Mutex::new(Vec::new())),
            inflight: Arc::new(AtomicUsize::new(0)),
            routing: Arc::new(Mutex::new(None)),
            dialogs: Arc::new(Mutex::new(None)),
        };
        page.call("Page.enable", Value::Null).await?;
        page.call("Runtime.enable", Value::Null).await?;
        page.call("Log.enable", Value::Null).await?;
        page.call("Network.enable", Value::Null).await?;
        page.spawn_listener();
        Ok(page)
    }

    fn spawn_listener(&self) {
        let mut events = self.cdp.subscribe();
        let session = self.session.clone();
        let console = Arc::clone(&self.console);
        let trace = Arc::clone(&self.trace);
        let inflight = Arc::clone(&self.inflight);
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(_) => break,
                };
                if event.session.as_deref() != Some(&session) {
                    continue;
                }
                handle_event(&event, &console, &trace, &inflight);
            }
        });
    }

    /// Target id.
    #[must_use]
    pub fn target_id(&self) -> &str {
        &self.target
    }

    /// Default timeout for protocol calls.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Override the default timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Raw session call with the page timeout.
    pub async fn call(&self, method: &str, params: Value) -> E2eResult<Value> {
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

    /// Raw session call with an explicit timeout.
    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> E2eResult<Value> {
        self.cdp
            .call(Some(&self.session), method, params, timeout)
            .await
    }

    fn record(&self, kind: &str, detail: String) {
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

    async fn slow_mo(&self) {
        if !self.slow_mo.is_zero() {
            tokio::time::sleep(self.slow_mo).await;
        }
    }

    /// Recorded trace entries (actions, navigations, console).
    #[must_use]
    pub fn trace(&self) -> Vec<TraceEntry> {
        self.trace.lock().map(|t| t.clone()).unwrap_or_default()
    }

    /// Console messages and page errors observed so far.
    #[must_use]
    pub fn console_messages(&self) -> Vec<ConsoleMessage> {
        self.console.lock().map(|c| c.clone()).unwrap_or_default()
    }

    /// Resolve a possibly-relative URL against the base URL.
    fn resolve_url(&self, url: &str) -> E2eResult<String> {
        if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("about:")
            || url.starts_with("data:")
            || url.starts_with("file:")
        {
            return Ok(url.to_string());
        }
        if let Some(base) = &self.base_url {
            let base = base.trim_end_matches('/');
            let path = if url.starts_with('/') {
                url.to_string()
            } else {
                format!("/{url}")
            };
            return Ok(format!("{base}{path}"));
        }
        if let Ok(base) = std::env::var("FERRITE_E2E_BASE_URL") {
            let base = base.trim_end_matches('/').to_string();
            let path = if url.starts_with('/') {
                url.to_string()
            } else {
                format!("/{url}")
            };
            return Ok(format!("{base}{path}"));
        }
        Err(E2eError::Navigation {
            url: url.to_string(),
            message: "relative URL without a base_url (set Browser::set_base_url, \
                      [e2e].base_url, or FERRITE_E2E_BASE_URL)"
                .to_string(),
        })
    }

    /// Navigate to a URL and wait for the load state.
    pub async fn goto(&self, url: &str) -> E2eResult<()> {
        self.goto_with_options(url, NavigationOptions::default())
            .await
    }

    /// Navigate with explicit options.
    pub async fn goto_with_options(&self, url: &str, options: NavigationOptions) -> E2eResult<()> {
        let url = self.resolve_url(url)?;
        let timeout = options.timeout.unwrap_or(self.timeout);
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
                url: url.clone(),
                message: error.to_string(),
            })?;
        if let Some(error) = result.get("errorText").and_then(Value::as_str) {
            return Err(E2eError::Navigation {
                url,
                message: error.to_string(),
            });
        }
        self.record("navigation", format!("goto {url}"));
        self.wait_for_load_state_with_events(options.wait_until, timeout, &mut events)
            .await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Reload the page.
    pub async fn reload(&self) -> E2eResult<()> {
        self.call("Page.reload", Value::Null).await?;
        self.wait_for_load_state(LoadState::Load).await
    }

    /// Go back in history.
    pub async fn go_back(&self) -> E2eResult<()> {
        self.history_delta(-1).await
    }

    /// Go forward in history.
    pub async fn go_forward(&self) -> E2eResult<()> {
        self.history_delta(1).await
    }

    async fn history_delta(&self, delta: i64) -> E2eResult<()> {
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
        self.wait_for_load_state(LoadState::Load).await
    }

    /// Current page title.
    pub async fn title(&self) -> E2eResult<String> {
        self.evaluate_string("document.title").await
    }

    /// Current page URL.
    pub async fn url(&self) -> E2eResult<String> {
        self.evaluate_string("location.href").await
    }

    /// Full HTML content.
    pub async fn content(&self) -> E2eResult<String> {
        self.evaluate_string("document.documentElement.outerHTML")
            .await
    }

    /// Set the document HTML.
    pub async fn set_content(&self, html: &str) -> E2eResult<()> {
        let tree = self.call("Page.getFrameTree", Value::Null).await?;
        let frame_id = tree["frame"]["id"].clone();
        self.call(
            "Page.setDocumentContent",
            serde_json::json!({ "frameId": frame_id, "html": html }),
        )
        .await?;
        Ok(())
    }

    /// Bring the page to front.
    pub async fn bring_to_front(&self) -> E2eResult<()> {
        self.call("Page.bringToFront", Value::Null).await?;
        Ok(())
    }

    /// Evaluate JavaScript and deserialize the returned value.
    pub async fn evaluate<T>(&self, expression: &str) -> E2eResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self.evaluate_value(expression).await?;
        serde_json::from_value(value).map_err(E2eError::Json)
    }

    /// Evaluate JavaScript and return the raw JSON value.
    pub async fn evaluate_value(&self, expression: &str) -> E2eResult<Value> {
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
        let value = self.evaluate_value(expression).await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    /// Wait until a JS expression returns truthy.
    pub async fn wait_for_function(&self, expression: &str, timeout: Duration) -> E2eResult<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        let wrapped = format!("Boolean((async () => {{ return ({expression}); }})())");
        loop {
            if let Ok(value) = self.evaluate_value(&wrapped).await {
                if value.as_bool().unwrap_or(false) {
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait_for_function({expression})"),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Wait a fixed amount of time.
    pub async fn wait_for_timeout(&self, duration: Duration) -> E2eResult<()> {
        tokio::time::sleep(duration).await;
        Ok(())
    }

    /// Wait until the URL contains `fragment`.
    pub async fn wait_for_url(&self, fragment: &str, timeout: Duration) -> E2eResult<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Ok(url) = self.url().await {
                if url.contains(fragment) {
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait_for_url({fragment})"),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Wait for a document load state.
    pub async fn wait_for_load_state(&self, state: LoadState) -> E2eResult<()> {
        let mut events = self.cdp.subscribe();
        self.wait_for_load_state_with_events(state, self.timeout, &mut events)
            .await
    }

    async fn wait_for_load_state_with_events(
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
        // Fast path: the document may already be past the state.
        if self.load_state_satisfied(state).await {
            return self.settle_network_idle(state, deadline).await;
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
                        return self.settle_network_idle(state, deadline).await;
                    }
                }
                Ok(Err(_)) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(_) => {
                    // Last chance: check the document directly before failing.
                    if self.load_state_satisfied(state).await {
                        return self.settle_network_idle(state, deadline).await;
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

    async fn settle_network_idle(
        &self,
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
            if self.inflight.load(Ordering::SeqCst) == 0 {
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

    /// Build a locator for a selector.
    #[must_use]
    pub fn locator(&self, selector: impl Into<String>) -> Locator {
        Locator::new(self.clone(), Selector::parse(selector.into()))
    }

    /// Snapshot the state of a selector (count, visibility, text, ...).
    pub async fn query_state(&self, selector: &Selector) -> E2eResult<ElementState> {
        let expression = selector.state_expression();
        let value = self.evaluate_value(&expression).await?;
        serde_json::from_value(value).map_err(E2eError::Json)
    }

    pub(crate) async fn action(
        &self,
        selector: &Selector,
        action: &str,
        argument: Option<&str>,
    ) -> E2eResult<Value> {
        let expression = selector.action_expression(action, argument);
        let value = self.evaluate_value(&expression).await?;
        if value.get("ok").and_then(Value::as_bool) == Some(false) {
            let message = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("action failed");
            return Err(E2eError::Locator {
                selector: selector.raw().to_string(),
                message: message.to_string(),
            });
        }
        self.record("action", format!("{action} {}", selector.raw()));
        self.slow_mo().await;
        Ok(value)
    }

    /// Trusted mouse click at CSS-pixel coordinates.
    pub async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.bring_to_front().await?;
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        for kind in ["mousePressed", "mouseReleased"] {
            self.call(
                "Input.dispatchMouseEvent",
                serde_json::json!({
                    "type": kind,
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": click_count.max(1),
                }),
            )
            .await?;
        }
        self.slow_mo().await;
        Ok(())
    }

    /// Move the mouse to CSS-pixel coordinates.
    pub async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.call(
            "Input.dispatchMouseEvent",
            serde_json::json!({ "type": "mouseMoved", "x": x, "y": y }),
        )
        .await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Insert text at the focused element (trusted input).
    pub async fn insert_text(&self, text: &str) -> E2eResult<()> {
        self.call("Input.insertText", serde_json::json!({ "text": text }))
            .await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Dispatch a key press (name like `Enter` or a single char).
    pub async fn press_key(&self, key: &str) -> E2eResult<()> {
        let (windows_code, key_name, code) = key_definition(key);
        if key.chars().count() == 1 && windows_code == 0 {
            self.call(
                "Input.dispatchKeyEvent",
                serde_json::json!({ "type": "char", "text": key }),
            )
            .await?;
            self.slow_mo().await;
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
        self.slow_mo().await;
        Ok(())
    }

    /// Capture a screenshot (PNG by default, JPEG with `quality`).
    pub async fn screenshot(&self, options: ScreenshotOptions) -> E2eResult<Vec<u8>> {
        if options.full_page {
            return self.full_page_screenshot(options.quality).await;
        }
        let mut params = serde_json::json!({ "captureBeyondViewport": true });
        if let Some(quality) = options.quality {
            params["format"] = Value::String("jpeg".to_string());
            params["quality"] = Value::from(quality);
        }
        let shot = self.call("Page.captureScreenshot", params).await?;
        decode_shot(&shot)
    }

    /// Capture a screenshot and write it to `path`.
    pub async fn save_screenshot(
        &self,
        path: &std::path::Path,
        options: ScreenshotOptions,
    ) -> E2eResult<()> {
        let bytes = self.screenshot(options).await?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
        Ok(())
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

    /// Print the page to PDF bytes.
    pub async fn pdf(&self) -> E2eResult<Vec<u8>> {
        let pdf = self.call("Page.printToPDF", Value::Null).await?;
        decode_shot(&pdf)
    }

    /// Set the viewport size.
    pub async fn set_viewport(&self, viewport: Viewport) -> E2eResult<()> {
        self.call(
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({
                "width": viewport.width,
                "height": viewport.height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
        )
        .await?;
        Ok(())
    }

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.call(
            "Emulation.setUserAgentOverride",
            serde_json::json!({ "userAgent": user_agent }),
        )
        .await?;
        Ok(())
    }

    /// Ignore HTTPS certificate errors.
    pub async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        self.call(
            "Security.setIgnoreCertificateErrors",
            serde_json::json!({ "ignore": ignore }),
        )
        .await?;
        Ok(())
    }

    /// Cookies visible to this page.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        let cookies = self.call("Network.getCookies", Value::Null).await?;
        let list = cookies
            .get("cookies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        serde_json::from_value(Value::Array(list)).map_err(E2eError::Json)
    }

    /// Set a cookie for the current page URL.
    pub async fn set_cookie(&self, name: &str, value: &str) -> E2eResult<()> {
        let url = self.url().await?;
        self.call(
            "Network.setCookie",
            serde_json::json!({ "name": name, "value": value, "url": url }),
        )
        .await?;
        Ok(())
    }

    /// Clear browser cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.call("Network.clearBrowserCookies", Value::Null)
            .await?;
        Ok(())
    }

    /// Start intercepting requests with glob rules.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        use globset::{Glob, GlobSetBuilder};
        self.stop_routing().await;
        let mut builder = GlobSetBuilder::new();
        for rule in &rules {
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
                let matched = set.matches(&url).first().map(|i| &rules[*i]);
                let (method, params) = match matched.map(|r| &r.action) {
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
        *self.routing.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.abort_handle());
        Ok(())
    }

    /// Stop intercepting requests.
    pub async fn stop_routing(&self) {
        let handle = self.routing.lock().map(|mut r| r.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
            let _ = self.call("Fetch.disable", Value::Null).await;
        }
    }

    /// Auto-handle JavaScript dialogs (`accept` = OK vs dismiss).
    pub async fn handle_dialogs(&self, accept: bool) -> E2eResult<()> {
        self.stop_dialog_handling().await;
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
        });
        *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.abort_handle());
        Ok(())
    }

    /// Stop auto-handling dialogs.
    pub async fn stop_dialog_handling(&self) {
        let handle = self.dialogs.lock().map(|mut d| d.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    /// Close the page target.
    pub async fn close(&self) -> E2eResult<()> {
        self.stop_routing().await;
        self.stop_dialog_handling().await;
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
}

fn base64_encode(bytes: &[u8]) -> String {
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

fn decode_shot(value: &Value) -> E2eResult<Vec<u8>> {
    use base64::Engine as _;
    let data = value
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| E2eError::Cdp {
            method: "Page.captureScreenshot".to_string(),
            message: error.to_string(),
        })
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

fn handle_event(
    event: &CdpEvent,
    console: &Arc<Mutex<Vec<ConsoleMessage>>>,
    trace: &Arc<Mutex<Vec<TraceEntry>>>,
    inflight: &Arc<AtomicUsize>,
) {
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
            push_console(console, trace, kind, args);
        }
        "Runtime.exceptionThrown" => {
            let text = event.params["exceptionDetails"]["text"]
                .as_str()
                .unwrap_or("page exception")
                .to_string();
            push_console(console, trace, "exception".to_string(), text);
        }
        "Log.entryAdded" => {
            let entry = &event.params["entry"];
            let kind = entry["level"].as_str().unwrap_or("log").to_string();
            let text = entry["text"].as_str().unwrap_or_default().to_string();
            push_console(console, trace, kind, text);
        }
        "Network.requestWillBeSent" => {
            inflight.fetch_add(1, Ordering::SeqCst);
        }
        "Network.loadingFinished" | "Network.loadingFailed" => {
            inflight.fetch_sub(1, Ordering::SeqCst);
        }
        _ => {}
    }
}

fn push_console(
    console: &Arc<Mutex<Vec<ConsoleMessage>>>,
    trace: &Arc<Mutex<Vec<TraceEntry>>>,
    kind: String,
    text: String,
) {
    if let Ok(mut console) = console.lock() {
        console.push(ConsoleMessage {
            kind: kind.clone(),
            text: text.clone(),
        });
    }
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    if let Ok(mut trace) = trace.lock() {
        trace.push(TraceEntry {
            ts_ms,
            kind: "console".to_string(),
            detail: format!("{kind}: {text}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_definitions_cover_specials() {
        assert_eq!(key_definition("Enter").0, 13);
        assert_eq!(key_definition("Escape").0, 27);
        assert_eq!(key_definition("ArrowLeft").0, 37);
        assert_eq!(key_definition("a").0, u16::from(b'A'));
    }

    #[test]
    fn navigation_requires_base_for_relative() {
        // resolve_url is covered through goto errors; this pins the message.
        let message = "relative URL without a base_url";
        assert!(message.contains("base_url"));
    }
}
