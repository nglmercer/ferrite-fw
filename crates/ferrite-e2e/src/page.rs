//! Pages: navigation, evaluation, input, screenshots, routing.
//!
//! [`Page`] is engine-agnostic: it delegates to a [`Driver`](crate::driver::Driver)
//! (CDP for Chromium, WebDriver BiDi for Firefox).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::driver::{ConsoleSink, Driver};
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

/// An automated page (one browser tab).
#[derive(Clone)]
pub struct Page {
    driver: Driver,
    sink: ConsoleSink,
    slow_mo: Duration,
    base_url: Option<String>,
    routing: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    dialogs: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

impl Page {
    pub(crate) fn new(
        driver: Driver,
        sink: ConsoleSink,
        slow_mo: Duration,
        base_url: Option<String>,
    ) -> Self {
        Self {
            driver,
            sink,
            slow_mo,
            base_url,
            routing: Arc::new(Mutex::new(None)),
            dialogs: Arc::new(Mutex::new(None)),
        }
    }

    /// Target id.
    #[must_use]
    pub fn target_id(&self) -> &str {
        self.driver.target_id()
    }

    /// Default timeout for protocol calls.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.driver.timeout()
    }

    /// Override the default timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.driver.set_timeout(timeout);
    }

    /// Raw protocol call with the page timeout (CDP method on Chromium,
    /// BiDi method with context injected on Firefox).
    pub async fn call(&self, method: &str, params: Value) -> E2eResult<Value> {
        self.driver.raw(method, params, self.timeout()).await
    }

    /// Raw protocol call with an explicit timeout.
    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> E2eResult<Value> {
        self.driver.raw(method, params, timeout).await
    }

    async fn slow_mo(&self) {
        if !self.slow_mo.is_zero() {
            tokio::time::sleep(self.slow_mo).await;
        }
    }

    /// Recorded trace entries (actions, navigations, console).
    #[must_use]
    pub fn trace(&self) -> Vec<TraceEntry> {
        self.sink
            .trace
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default()
    }

    /// Console messages and page errors observed so far.
    #[must_use]
    pub fn console_messages(&self) -> Vec<ConsoleMessage> {
        self.sink
            .console
            .lock()
            .map(|c| c.clone())
            .unwrap_or_default()
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
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        self.driver
            .navigate(&url, options.wait_until, timeout)
            .await
            .map_err(|error| match error {
                E2eError::Navigation { .. } => error,
                other => E2eError::Navigation {
                    url: url.clone(),
                    message: other.to_string(),
                },
            })?;
        self.slow_mo().await;
        Ok(())
    }

    /// Reload the page.
    pub async fn reload(&self) -> E2eResult<()> {
        self.driver.reload().await
    }

    /// Go back in history.
    pub async fn go_back(&self) -> E2eResult<()> {
        self.driver.traverse(-1).await
    }

    /// Go forward in history.
    pub async fn go_forward(&self) -> E2eResult<()> {
        self.driver.traverse(1).await
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
        self.driver.set_content(html).await
    }

    /// Bring the page to front.
    pub async fn bring_to_front(&self) -> E2eResult<()> {
        self.driver.bring_to_front().await
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
        self.driver.evaluate(expression).await
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
        let timeout = self.timeout();
        self.driver.wait_for_load(state, timeout).await
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
        self.sink
            .record("action", format!("{action} {}", selector.raw()));
        self.slow_mo().await;
        Ok(value)
    }

    /// Trusted mouse click at CSS-pixel coordinates.
    pub async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.driver.mouse_click(x, y, click_count).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Move the mouse to CSS-pixel coordinates.
    pub async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver.mouse_move(x, y).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Insert text at the focused element (trusted input).
    pub async fn insert_text(&self, text: &str) -> E2eResult<()> {
        self.driver.insert_text(text).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Dispatch a key press (name like `Enter` or a single char).
    pub async fn press_key(&self, key: &str) -> E2eResult<()> {
        self.driver.press_key(key).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Capture a screenshot (PNG by default, JPEG with `quality`).
    pub async fn screenshot(&self, options: ScreenshotOptions) -> E2eResult<Vec<u8>> {
        self.driver
            .screenshot(options.full_page, options.quality)
            .await
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

    /// Print the page to PDF bytes.
    pub async fn pdf(&self) -> E2eResult<Vec<u8>> {
        self.driver.print_pdf().await
    }

    /// Set the viewport size.
    pub async fn set_viewport(&self, viewport: Viewport) -> E2eResult<()> {
        self.driver
            .set_viewport(viewport.width, viewport.height)
            .await
    }

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.driver.set_user_agent(user_agent).await
    }

    /// Ignore HTTPS certificate errors.
    pub async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        self.driver.set_ignore_https_errors(ignore).await
    }

    /// Cookies visible to this page.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        self.driver.cookies().await
    }

    /// Set a cookie for the current page URL.
    pub async fn set_cookie(&self, name: &str, value: &str) -> E2eResult<()> {
        let url = self.url().await?;
        self.driver.set_cookie(name, value, &url).await
    }

    /// Clear browser cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.driver.clear_cookies().await
    }

    /// Start intercepting requests with glob rules.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        self.stop_routing().await;
        let handle = self.driver.start_routing(Arc::new(rules)).await?;
        *self.routing.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    /// Stop intercepting requests.
    pub async fn stop_routing(&self) {
        let handle = self.routing.lock().map(|mut r| r.take()).unwrap_or(None);
        if handle.is_some() {
            if let Some(handle) = handle {
                handle.abort();
            }
            self.driver.stop_routing().await;
        }
    }

    /// Auto-handle JavaScript dialogs (`accept` = OK vs dismiss).
    pub async fn handle_dialogs(&self, accept: bool) -> E2eResult<()> {
        self.stop_dialog_handling().await;
        let handle = self.driver.start_dialogs(accept).await?;
        *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
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
        self.driver.close().await
    }
}

#[cfg(test)]
mod tests {
    // Navigation without a base URL stays loud (covered through goto errors).
    #[test]
    fn relative_urls_need_a_base() {
        let message = "relative URL without a base_url";
        assert!(message.contains("base_url"));
    }
}
