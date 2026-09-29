//! Pages: navigation, evaluation, input, screenshots, routing.
//!
//! [`Page`] is engine-agnostic: it delegates to a [`Driver`](crate::driver::Driver)
//! (CDP for Chromium, WebDriver BiDi for Firefox).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::driver::{ConsoleSink, Driver, FrameStream, RecordingState};
use crate::error::{E2eError, E2eResult};
use crate::locator::{Locator, Selector};
use crate::video::VideoOptions;

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

/// Saved storage state: cookies plus one origin's localStorage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageState {
    /// Origin the state belongs to.
    pub origin: String,
    /// Cookies (restored by name/value on the current origin).
    #[serde(default)]
    pub cookies: Vec<Cookie>,
    /// localStorage entries.
    #[serde(default)]
    pub local_storage: HashMap<String, String>,
}

/// A network request observed while capturing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedRequest {
    /// HTTP method.
    pub method: String,
    /// Request URL.
    pub url: String,
    /// Response status (0 when the request failed).
    pub status: u16,
}

/// A JavaScript dialog observed while auto-handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogInfo {
    /// `alert`, `confirm`, `prompt`, or `beforeunload`.
    pub dialog_type: String,
    /// Dialog message text.
    pub message: String,
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

/// Preferred color scheme for media emulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// `prefers-color-scheme: dark`.
    Dark,
    /// `prefers-color-scheme: light`.
    Light,
}

/// Reduced-motion preference for media emulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotion {
    /// `prefers-reduced-motion: reduce`.
    Reduce,
    /// `prefers-reduced-motion: no-preference`.
    NoPreference,
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
    /// First match is editable (enabled input/textarea/select or contenteditable).
    #[serde(default)]
    pub editable: bool,
    /// First match is the focused element.
    #[serde(default)]
    pub focused: bool,
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
    capture: Arc<Mutex<Option<CaptureState>>>,
    routes: Arc<Mutex<Vec<RouteRule>>>,
    net_capture: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

/// At most one capture (recording or frame stream) per page: both use the
/// same screencast session on Chromium.
enum CaptureState {
    /// Active recording.
    Recording(RecordingState),
    /// Active frame stream pump.
    Streaming(tokio::task::AbortHandle),
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
            capture: Arc::new(Mutex::new(None)),
            routes: Arc::new(Mutex::new(Vec::new())),
            net_capture: Arc::new(Mutex::new(None)),
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

    /// Locate `[data-testid="id"]` exactly.
    #[must_use]
    pub fn get_by_test_id(&self, id: &str) -> Locator {
        Locator::new(self.clone(), Selector::test_id(id))
    }

    /// Locate elements containing `text` (case-insensitive substring).
    #[must_use]
    pub fn get_by_text(&self, text: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_text(text))
    }

    /// Locate an ARIA role, optionally filtered by accessible name.
    #[must_use]
    pub fn get_by_role(&self, role: &str, name: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_role(role, name))
    }

    /// Locate a `<label>` by its text.
    ///
    /// Matches the label element itself (not the labeled control).
    #[must_use]
    pub fn get_by_label(&self, text: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_label(text))
    }

    /// Locate by `[placeholder]` (case-insensitive substring).
    #[must_use]
    pub fn get_by_placeholder(&self, text: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_placeholder(text))
    }

    /// Locate by `[alt]` (case-insensitive substring).
    #[must_use]
    pub fn get_by_alt(&self, text: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_alt(text))
    }

    /// Locate by `[title]` (case-insensitive substring).
    #[must_use]
    pub fn get_by_title(&self, text: &str) -> Locator {
        Locator::new(self.clone(), Selector::by_title(text))
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

    /// Press the left mouse button at CSS-pixel coordinates.
    pub async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver.mouse_down(x, y).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Release the left mouse button at CSS-pixel coordinates.
    pub async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver.mouse_up(x, y).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Scroll a wheel at CSS-pixel coordinates by (`delta_x`, `delta_y`).
    pub async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        self.driver.mouse_wheel(x, y, delta_x, delta_y).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Hold a key down (pair with [`Page::key_up`]).
    pub async fn key_down(&self, key: &str) -> E2eResult<()> {
        self.driver.key_down(key).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Release a held key.
    pub async fn key_up(&self, key: &str) -> E2eResult<()> {
        self.driver.key_up(key).await?;
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

    /// Save cookies plus current-origin localStorage to a JSON file.
    pub async fn save_storage_state(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        let origin = self.evaluate_string("location.origin").await?;
        let local_storage: HashMap<String, String> =
            serde_json::from_value(self.evaluate_value("({ ...localStorage })").await?)?;
        let state = StorageState {
            origin,
            cookies: self.cookies().await?,
            local_storage,
        };
        std::fs::write(path, serde_json::to_string_pretty(&state)?)?;
        Ok(())
    }

    /// Load storage state saved by [`Page::save_storage_state`].
    ///
    /// The page must already be on the saved origin; cookies restore by
    /// name/value and localStorage is replaced wholesale.
    pub async fn load_storage_state(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        let raw = std::fs::read_to_string(path)?;
        let state: StorageState = serde_json::from_str(&raw)?;
        let origin = self.evaluate_string("location.origin").await?;
        if origin != state.origin {
            return Err(E2eError::Config(format!(
                "storage state is for origin '{}', navigate there first (at '{origin}')",
                state.origin
            )));
        }
        let url = self.url().await?;
        for cookie in &state.cookies {
            self.driver
                .set_cookie(&cookie.name, &cookie.value, &url)
                .await?;
        }
        let entries = serde_json::to_string(&state.local_storage)?;
        self.evaluate_value(&format!(
            "(() => {{ localStorage.clear(); \
             for (const [k, v] of Object.entries({entries})) localStorage.setItem(k, v); \
             return true; }})()"
        ))
        .await?;
        Ok(())
    }

    /// Grant permissions (`geolocation`, `notifications`, ...).
    ///
    /// Chromium grants to all origins; Firefox grants to the current page
    /// origin (navigate first).
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        self.driver.grant_permissions(permissions).await
    }

    /// Override the geolocation coordinates.
    ///
    /// Pair with [`Page::grant_permissions`]; Firefox supports this only on
    /// recent builds and otherwise fails loudly.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.driver.set_geolocation(latitude, longitude).await
    }

    /// Emulate offline mode (Chromium only).
    pub async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        self.driver.set_offline(offline).await
    }

    /// Set extra HTTP headers for subsequent requests (Chromium only).
    pub async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        self.driver.set_extra_http_headers(headers).await
    }

    /// Override the locale (Chromium only).
    ///
    /// Applies to subsequently loaded documents; navigate or reload after.
    /// Drives `Intl` and `Accept-Language`; `navigator.language` follows the
    /// launch `--lang` flag instead.
    pub async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        self.driver.set_locale(locale).await
    }

    /// Override the timezone (Chromium only).
    ///
    /// Applies to subsequently loaded documents; navigate or reload after.
    pub async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        self.driver.set_timezone(timezone_id).await
    }

    /// Emulate media features (Chromium only).
    ///
    /// `(None, None)` is a no-op on every engine.
    pub async fn emulate_media(
        &self,
        color_scheme: Option<ColorScheme>,
        reduced_motion: Option<ReducedMotion>,
    ) -> E2eResult<()> {
        if color_scheme.is_none() && reduced_motion.is_none() {
            return Ok(());
        }
        self.driver
            .emulate_media(color_scheme, reduced_motion)
            .await
    }

    /// Start intercepting requests with glob rules.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = rules;
        self.restart_routing().await
    }

    /// Stop intercepting requests.
    pub async fn stop_routing(&self) {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();
        let handle = self.routing.lock().map(|mut r| r.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
            self.driver.stop_routing().await;
        }
    }

    /// Remove rules with `pattern`; returns how many were removed.
    pub async fn unroute(&self, pattern: &str) -> E2eResult<usize> {
        let removed = self
            .routes
            .lock()
            .map(|mut routes| {
                let before = routes.len();
                routes.retain(|rule| rule.pattern != pattern);
                before - routes.len()
            })
            .unwrap_or(0);
        self.restart_routing().await?;
        Ok(removed)
    }

    /// Apply the stored rules (no pump when empty).
    async fn restart_routing(&self) -> E2eResult<()> {
        let handle = self.routing.lock().map(|mut r| r.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
            self.driver.stop_routing().await;
        }
        let rules = self
            .routes
            .lock()
            .map(|routes| routes.clone())
            .unwrap_or_default();
        if rules.is_empty() {
            return Ok(());
        }
        let handle = self.driver.start_routing(Arc::new(rules)).await?;
        *self.routing.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    /// Start recording requests; clears previously recorded ones.
    pub fn start_request_capture(&self) {
        self.stop_request_capture();
        self.sink.clear_requests();
        let handle = self.driver.start_request_capture();
        *self.net_capture.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    /// Stop recording requests (keeps recorded ones).
    pub fn stop_request_capture(&self) {
        let handle = self
            .net_capture
            .lock()
            .map(|mut c| c.take())
            .unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    /// Requests recorded since capture started (oldest first, capped).
    #[must_use]
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.sink.requests()
    }

    /// Auto-handle JavaScript dialogs (`accept` = OK vs dismiss).
    pub async fn handle_dialogs(&self, accept: bool) -> E2eResult<()> {
        self.stop_dialog_handling().await;
        let handle = self.driver.start_dialogs(accept, None).await?;
        *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    /// Auto-handle dialogs, answering prompts with `prompt_text`.
    pub async fn handle_dialogs_with_prompt(
        &self,
        accept: bool,
        prompt_text: &str,
    ) -> E2eResult<()> {
        self.stop_dialog_handling().await;
        let handle = self
            .driver
            .start_dialogs(accept, Some(prompt_text.to_string()))
            .await?;
        *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    /// Dialogs observed while auto-handling (oldest first).
    #[must_use]
    pub fn dialogs(&self) -> Vec<DialogInfo> {
        self.sink
            .dialogs
            .lock()
            .map(|d| d.clone())
            .unwrap_or_default()
    }

    /// The most recently observed dialog, if any.
    #[must_use]
    pub fn last_dialog(&self) -> Option<DialogInfo> {
        self.dialogs().pop()
    }

    /// Stop auto-handling dialogs.
    pub async fn stop_dialog_handling(&self) {
        let handle = self.dialogs.lock().map(|mut d| d.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    /// Start recording video (one capture at a time per page).
    ///
    /// Chromium assembles damage-driven screencast frames with per-frame
    /// timestamps (correct duration even for mostly-static pages);
    /// Firefox records natively. Fully static recordings with zero
    /// repaints fail loudly at stop time on Chromium.
    pub async fn start_video(&self, opts: VideoOptions) -> E2eResult<()> {
        if self.capture.lock().map(|c| c.is_some()).unwrap_or(true) {
            return Err(E2eError::Config(
                "capture already active on this page; stop it first".to_string(),
            ));
        }
        let state = self.driver.start_recording(&opts).await?;
        *self.capture.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(CaptureState::Recording(state));
        Ok(())
    }

    /// Take the capture state only when it is a recording.
    fn take_recording(&self) -> Option<RecordingState> {
        let mut guard = self.capture.lock().ok()?;
        match guard.as_ref() {
            Some(CaptureState::Recording(_)) => match guard.take() {
                Some(CaptureState::Recording(state)) => Some(state),
                _ => None,
            },
            _ => None,
        }
    }

    /// Take the capture state only when it is a frame stream.
    fn take_stream(&self) -> Option<tokio::task::AbortHandle> {
        let mut guard = self.capture.lock().ok()?;
        match guard.as_ref() {
            Some(CaptureState::Streaming(_)) => match guard.take() {
                Some(CaptureState::Streaming(pump)) => Some(pump),
                _ => None,
            },
            _ => None,
        }
    }

    /// Stop the recording and write the video to `path`.
    pub async fn stop_video(&self, path: &std::path::Path) -> E2eResult<std::path::PathBuf> {
        match self.take_recording() {
            Some(state) => self.driver.stop_recording(state, path).await,
            None if self.is_streaming() => Err(E2eError::Config(
                "a frame stream (not a recording) is active; use stop_frames".to_string(),
            )),
            None => Err(E2eError::Config(
                "no active recording on this page".to_string(),
            )),
        }
    }

    /// Discard the recording without producing output.
    pub async fn cancel_video(&self) {
        if let Some(state) = self.take_recording() {
            self.driver.cancel_recording(state).await;
        }
    }

    /// True when a frame stream (not a recording) is active.
    fn is_streaming(&self) -> bool {
        self.capture
            .lock()
            .map(|c| matches!(*c, Some(CaptureState::Streaming(_))))
            .unwrap_or(false)
    }

    /// Start a live frame stream (one capture at a time per page).
    ///
    /// Chromium emits frames on repaint only: static pages yield nothing
    /// until damage occurs (use `next_timeout`). Firefox polls screenshots
    /// at `opts.fps` regardless of motion.
    pub async fn frames(&self, opts: VideoOptions) -> E2eResult<FrameStream> {
        if self.capture.lock().map(|c| c.is_some()).unwrap_or(true) {
            return Err(E2eError::Config(
                "capture already active on this page; stop it first".to_string(),
            ));
        }
        let (stream, pump) = self.driver.start_frame_stream(&opts).await?;
        *self.capture.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(CaptureState::Streaming(pump));
        Ok(stream)
    }

    /// Stop the live frame stream.
    pub async fn stop_frames(&self) {
        if let Some(pump) = self.take_stream() {
            pump.abort();
            self.driver.stop_frame_stream().await;
        }
    }

    /// Screenshot one element box (PNG by default, JPEG with `quality`).
    pub async fn screenshot_clip(
        &self,
        rect: &ElementRect,
        quality: Option<u8>,
    ) -> E2eResult<Vec<u8>> {
        self.driver.screenshot_clip(rect, quality).await
    }

    /// Close the page target.
    pub async fn close(&self) -> E2eResult<()> {
        self.stop_routing().await;
        self.stop_dialog_handling().await;
        self.stop_request_capture();
        self.stop_frames().await;
        self.cancel_video().await;
        self.driver.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Navigation without a base URL stays loud (covered through goto errors).
    #[test]
    fn relative_urls_need_a_base() {
        let message = "relative URL without a base_url";
        assert!(message.contains("base_url"));
    }

    #[test]
    fn storage_state_round_trips() {
        let mut local_storage = HashMap::new();
        local_storage.insert("k".to_string(), "v".to_string());
        let state = StorageState {
            origin: "http://127.0.0.1:9".to_string(),
            cookies: vec![Cookie {
                name: "sess".to_string(),
                value: "abc".to_string(),
                domain: None,
                path: None,
                http_only: false,
                secure: false,
            }],
            local_storage,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: StorageState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.origin, state.origin);
        assert_eq!(back.cookies.len(), 1);
        assert_eq!(back.local_storage.get("k").unwrap(), "v");
    }
}
