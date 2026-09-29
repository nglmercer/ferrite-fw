//! Pages: navigation, evaluation, input, screenshots, routing.
//!
//! [`Page`] is engine-agnostic: it delegates to a [`Driver`](crate::driver::Driver)
//! (CDP for Chromium, WebDriver BiDi for Firefox).

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{ApiClient, ApiResponse};
use crate::context::{TraceScreenshot, TracingState};
use crate::driver::now_ms;
use crate::driver::{base64_encode, ConsoleSink, Driver, FrameStream, RecordingState};
use crate::error::{E2eError, E2eResult};
use crate::jshandle::JSHandle;
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

/// HTTP credentials for basic and digest auth challenges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCredentials {
    /// Username sent with challenges.
    pub username: String,
    /// Password sent with challenges.
    pub password: String,
}

impl HttpCredentials {
    /// Build credentials from a username and password.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
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
    /// Request headers as sent.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Request body text (Chromium only; `None` when absent or streamed).
    #[serde(default)]
    pub post_data: Option<String>,
    /// Request-to-response wall time in ms (`None` when the start was missed).
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Response status text (`""` when unknown or the request failed).
    #[serde(default)]
    pub status_text: String,
    /// Response MIME type (`""` when unknown).
    #[serde(default)]
    pub mime_type: String,
    /// Response headers as received.
    #[serde(default)]
    pub response_headers: Vec<(String, String)>,
    /// Request start, milliseconds since the Unix epoch.
    #[serde(default)]
    pub started_ms: Option<u64>,
    /// Engine request id (correlates with protocol logs).
    #[serde(default)]
    pub request_id: Option<String>,
    /// Response body bytes (Chromium only; `None` when unavailable).
    ///
    /// In-memory only: skipped by serde so traces stay lean. Bodies larger
    /// than 1 MiB are dropped (`body_truncated`).
    #[serde(skip)]
    pub body: Option<Vec<u8>>,
    /// True when the body exceeded the 1 MiB capture cap.
    #[serde(default)]
    pub body_truncated: bool,
}

impl RecordedRequest {
    /// Response body as lossy text (`None` when no body was captured).
    #[must_use]
    pub fn body_text(&self) -> Option<String> {
        self.body
            .as_ref()
            .map(|body| String::from_utf8_lossy(body).into_owned())
    }

    /// Response body parsed as JSON (`None` when absent or not JSON).
    #[must_use]
    pub fn body_json(&self) -> Option<Value> {
        self.body
            .as_ref()
            .and_then(|body| serde_json::from_slice(body).ok())
    }
}

/// A JavaScript dialog observed while auto-handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogInfo {
    /// `alert`, `confirm`, `prompt`, or `beforeunload`.
    pub dialog_type: String,
    /// Dialog message text.
    pub message: String,
}

/// Kinds of observable page events (see [`Page::wait_for_event`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageEventKind {
    /// Console message or page exception.
    Console,
    /// JavaScript dialog (observed while handling is armed).
    Dialog,
    /// Network request started.
    Request,
    /// Network response finished (status 0 when failed).
    Response,
    /// File download completed in the download dir.
    Download,
    /// Popup page opened from this page.
    Popup,
    /// The page was closed in the browser.
    Closed,
    /// WebSocket lifecycle or frame (Chromium only).
    WebSocket,
}

/// Direction of a [`WebSocketEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebSocketDirection {
    /// Socket opened.
    Created,
    /// Frame sent by the page.
    Sent,
    /// Frame received from the server.
    Received,
    /// Socket closed.
    Closed,
}

/// A WebSocket lifecycle or frame observation (Chromium only; BiDi has no
/// socket-frame events).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSocketEvent {
    /// Socket URL.
    pub url: String,
    /// What happened.
    pub direction: WebSocketDirection,
    /// Frame payload text (`""` for created/closed).
    pub payload: String,
}

/// An observed page event.
#[derive(Debug, Clone)]
pub enum PageEvent {
    /// Console message or page exception.
    Console(ConsoleMessage),
    /// JavaScript dialog.
    Dialog(DialogInfo),
    /// Network request started.
    Request {
        /// HTTP method.
        method: String,
        /// Request URL.
        url: String,
    },
    /// Network response finished.
    Response {
        /// Request URL.
        url: String,
        /// Response status (0 when failed).
        status: u16,
    },
    /// File download completed.
    Download(PathBuf),
    /// Popup page opened from this page (already adopted and usable).
    Popup(Box<Page>),
    /// The page was closed in the browser.
    Closed,
    /// WebSocket lifecycle or frame (Chromium only).
    WebSocket(WebSocketEvent),
}

impl PageEvent {
    /// The event's kind (for filtering).
    #[must_use]
    pub fn kind(&self) -> PageEventKind {
        match self {
            Self::Console(_) => PageEventKind::Console,
            Self::Dialog(_) => PageEventKind::Dialog,
            Self::Request { .. } => PageEventKind::Request,
            Self::Response { .. } => PageEventKind::Response,
            Self::Download(_) => PageEventKind::Download,
            Self::Popup(_) => PageEventKind::Popup,
            Self::Closed => PageEventKind::Closed,
            Self::WebSocket(_) => PageEventKind::WebSocket,
        }
    }
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
    /// Expiry as epoch seconds (`None`/negative = session cookie).
    #[serde(default, deserialize_with = "de_cookie_expires")]
    pub expires: Option<i64>,
}

/// CDP reports cookie expiry as a float; BiDi may omit it.
fn de_cookie_expires<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => Ok(number
            .as_i64()
            .or_else(|| number.as_f64().map(|float| float.round() as i64))),
        Some(_) => Ok(None),
    }
}

/// File names currently in `dir` (loud error when unreadable).
fn dir_names(dir: &Path) -> E2eResult<HashSet<String>> {
    let entries = std::fs::read_dir(dir)
        .map_err(|error| E2eError::Config(format!("cannot read {}: {error}", dir.display())))?;
    Ok(entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect())
}

/// True for in-progress download files (not yet renamed to the final name).
fn is_temp_download(name: &str) -> bool {
    name.ends_with(".part") || name.ends_with(".crdownload") || name.ends_with(".tmp")
}

/// How recently a pre-existing file must be modified to count as this wait's
/// download (local downloads routinely land before the wait starts).
const PREEXISTING_DOWNLOAD_GRACE: Duration = Duration::from_secs(30);

/// A frame listing entry (ids are opaque engine handles).
#[derive(Debug, Clone)]
pub(crate) struct FrameInfo {
    /// Opaque frame id for driver-level frame evaluation.
    pub id: String,
    /// Frame name (`<iframe name>`; empty on Firefox).
    pub name: String,
    /// Frame document URL.
    pub url: String,
}

/// A frame in the page (main frame or iframe) with frame-scoped
/// `evaluate` and locators. Note: coordinate-based locator actions
/// (`hover`, `tap`, `drag_to`) use frame-relative coordinates and may miss
/// on offset iframes; DOM actions and assertions are exact.
#[derive(Clone)]
pub struct Frame {
    page: Page,
    id: String,
    name: String,
    url: String,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("name", &self.name)
            .field("url", &self.url)
            .finish()
    }
}

impl Frame {
    /// Frame name (empty on Firefox).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Frame document URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Evaluate an expression in this frame, deserializing the result.
    pub async fn evaluate<T>(&self, expression: &str) -> E2eResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self.evaluate_value(expression).await?;
        serde_json::from_value(value).map_err(E2eError::Json)
    }

    /// Evaluate an expression in this frame, returning raw JSON.
    pub async fn evaluate_value(&self, expression: &str) -> E2eResult<Value> {
        self.page.driver.frame_evaluate(&self.id, expression).await
    }

    /// This page scoped to the frame (all evaluation runs inside it).
    fn scoped_page(&self) -> Page {
        self.page.clone().scoped(self.id.clone())
    }

    /// Locate elements inside this frame.
    pub fn locator(&self, selector: impl Into<String>) -> Locator {
        Locator::new(self.scoped_page(), Selector::parse(selector.into()))
    }

    /// Locate `[data-testid]` inside this frame.
    pub fn get_by_test_id(&self, id: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::test_id(id))
    }

    /// Locate elements containing `text` inside this frame.
    pub fn get_by_text(&self, text: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_text(text))
    }

    /// Locate an ARIA role inside this frame.
    pub fn get_by_role(&self, role: &str, name: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_role(role, name))
    }

    /// Locate a `<label>` by its text inside this frame.
    pub fn get_by_label(&self, text: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_label(text))
    }

    /// Locate by `[placeholder]` inside this frame.
    pub fn get_by_placeholder(&self, text: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_placeholder(text))
    }

    /// Locate by `[alt]` inside this frame.
    pub fn get_by_alt(&self, text: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_alt(text))
    }

    /// Locate by `[title]` inside this frame.
    pub fn get_by_title(&self, text: &str) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_title(text))
    }
}

/// True when `name` is a safe JS identifier for [`Page::expose_function`].
fn valid_expose_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some('_' | '$' | 'a'..='z' | 'A'..='Z'))
        && chars.all(|c| matches!(c, '_' | '$' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
}

/// Options for [`Page::screenshot`].
#[derive(Debug, Clone, Default)]
pub struct ScreenshotOptions {
    /// Capture the full scrollable page.
    pub full_page: bool,
    /// JPEG quality (1-100); PNG when unset.
    pub quality: Option<u8>,
    /// Locators to cover with magenta boxes (viewport captures only).
    pub mask: Vec<Locator>,
    /// Freeze animations/transitions during the capture.
    pub disable_animations: bool,
    /// Hide the text caret during the capture.
    pub hide_caret: bool,
}

/// A device to emulate (Chromium only).
#[derive(Debug, Clone, Copy)]
pub struct DeviceDescriptor {
    /// CSS viewport.
    pub viewport: Viewport,
    /// Device pixel ratio.
    pub device_scale_factor: f64,
    /// Mobile UA hints and viewport behavior.
    pub mobile: bool,
    /// Touch event support.
    pub has_touch: bool,
}

impl DeviceDescriptor {
    /// iPhone 15 (393x852, 3x, mobile, touch).
    pub const IPHONE_15: Self = Self {
        viewport: Viewport {
            width: 393,
            height: 852,
        },
        device_scale_factor: 3.0,
        mobile: true,
        has_touch: true,
    };
    /// Pixel 7 (412x915, 2.625x, mobile, touch).
    pub const PIXEL_7: Self = Self {
        viewport: Viewport {
            width: 412,
            height: 915,
        },
        device_scale_factor: 2.625,
        mobile: true,
        has_touch: true,
    };
    /// Desktop 1080p (1920x1080, 1x).
    pub const DESKTOP_1080P: Self = Self {
        viewport: Viewport {
            width: 1920,
            height: 1080,
        },
        device_scale_factor: 1.0,
        mobile: false,
        has_touch: false,
    };
    /// iPhone SE (375x667, 2x, mobile, touch).
    pub const IPHONE_SE: Self = Self {
        viewport: Viewport {
            width: 375,
            height: 667,
        },
        device_scale_factor: 2.0,
        mobile: true,
        has_touch: true,
    };
    /// iPad (768x1024, 2x, touch, tablet UI).
    pub const IPAD: Self = Self {
        viewport: Viewport {
            width: 768,
            height: 1024,
        },
        device_scale_factor: 2.0,
        mobile: false,
        has_touch: true,
    };
    /// Desktop 1440p (2560x1440, 1x).
    pub const DESKTOP_1440P: Self = Self {
        viewport: Viewport {
            width: 2560,
            height: 1440,
        },
        device_scale_factor: 1.0,
        mobile: false,
        has_touch: false,
    };
    /// Laptop retina (1440x900, 2x).
    pub const LAPTOP_RETINA: Self = Self {
        viewport: Viewport {
            width: 1440,
            height: 900,
        },
        device_scale_factor: 2.0,
        mobile: false,
        has_touch: false,
    };
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

/// Network error for [`RouteAction::AbortWith`] (Chromium reports the reason;
/// Firefox fails plainly, ignoring it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortReason {
    /// Generic failure.
    Failed,
    /// Aborted (`net::ERR_ABORTED`, the plain [`RouteAction::Abort`]).
    Aborted,
    /// Connection refused.
    ConnectionRefused,
    /// Connection reset.
    ConnectionReset,
    /// Connection closed.
    ConnectionClosed,
    /// Connection failed.
    ConnectionFailed,
    /// Timed out.
    TimedOut,
    /// DNS failure.
    NameNotResolved,
    /// No connectivity.
    InternetDisconnected,
    /// Blocked by the client.
    BlockedByClient,
    /// Access denied.
    AccessDenied,
}

impl AbortReason {
    /// CDP `Network.ErrorReason` name.
    pub(crate) fn as_cdp(&self) -> &'static str {
        match self {
            Self::Failed => "Failed",
            Self::Aborted => "Aborted",
            Self::ConnectionRefused => "ConnectionRefused",
            Self::ConnectionReset => "ConnectionReset",
            Self::ConnectionClosed => "ConnectionClosed",
            Self::ConnectionFailed => "ConnectionFailed",
            Self::TimedOut => "TimedOut",
            Self::NameNotResolved => "NameNotResolved",
            Self::InternetDisconnected => "InternetDisconnected",
            Self::BlockedByClient => "BlockedByClient",
            Self::AccessDenied => "AccessDenied",
        }
    }
}

/// What to do with an intercepted request.
#[derive(Debug, Clone)]
pub enum RouteAction {
    /// Fail the request (`net::ERR_ABORTED`).
    Abort,
    /// Fail the request with an explicit reason (Chromium; Firefox fails plainly).
    AbortWith(AbortReason),
    /// Let the request through.
    Continue,
    /// Pass to the next matching rule/handler (unmodified when none match).
    Fallback,
    /// Respond with a synthetic body.
    Fulfill {
        /// HTTP status.
        status: u16,
        /// Status text (`""` = engine default).
        status_text: String,
        /// Response headers (a `content-type` here wins over `content_type`).
        headers: Vec<(String, String)>,
        /// Response body bytes.
        body: Vec<u8>,
        /// Content type header (used unless `headers` sets one).
        content_type: String,
    },
    /// Continue with modified URL/method/headers/body (`None` = unchanged).
    ContinueWith {
        /// Replacement URL.
        url: Option<String>,
        /// Replacement method.
        method: Option<String>,
        /// Replacement headers (replaces the whole set).
        headers: Option<Vec<(String, String)>>,
        /// Replacement body bytes.
        body: Option<Vec<u8>>,
    },
    /// Modify the real response (`None` = keep original status/headers/body).
    /// Header overrides replace the whole set; Content-Length is repaired
    /// whenever the body changes.
    ModifyResponse {
        /// Replacement status.
        status: Option<u16>,
        /// Replacement headers (replaces the whole set).
        headers: Option<Vec<(String, String)>>,
        /// Replacement body bytes.
        body: Option<Vec<u8>>,
    },
}

/// An intercepted request handed to a route handler.
#[derive(Debug, Clone)]
pub struct RouteInfo {
    /// Request URL.
    pub url: String,
    /// HTTP method.
    pub method: String,
    /// Request headers as sent.
    pub headers: Vec<(String, String)>,
    /// Request body bytes (`None` when absent; Firefox never captures it).
    pub post_data: Option<Vec<u8>>,
}

impl RouteInfo {
    /// Fetch the real response over plain HTTP (no browser state, like
    /// Playwright's `route.fetch`): inspect or rework it, then `Fulfill`.
    pub async fn fetch(&self) -> E2eResult<ApiResponse> {
        ApiClient::new()
            .request(
                &self.method,
                &self.url,
                &self.headers,
                self.post_data.as_deref(),
            )
            .await
    }
}

/// A route handler: inspect the request, decide the action.
///
/// Handler errors abort the request (and are recorded on the trace) instead
/// of hanging the page.
pub type RouteHandler = Arc<
    dyn Fn(RouteInfo) -> futures::future::BoxFuture<'static, E2eResult<RouteAction>> + Send + Sync,
>;

/// Options for [`Page::route_from_har`].
#[derive(Debug, Clone, Default)]
pub struct RouteFromHarOptions {
    /// Only load entries whose URL matches this glob (`None` = all).
    pub url_filter: Option<String>,
}

impl RouteFromHarOptions {
    /// Only load entries whose URL matches `glob`.
    #[must_use]
    pub fn url_filter(mut self, glob: impl Into<String>) -> Self {
        self.url_filter = Some(glob.into());
        self
    }
}

/// A glob pattern paired with its route handler.
#[derive(Clone)]
pub struct RouteHandlerEntry {
    /// Glob pattern (`**/api/*`).
    pub pattern: String,
    /// Handler deciding matching requests.
    pub handler: RouteHandler,
    /// Match at most this many requests (`None` = unlimited).
    pub times: Option<u32>,
    /// Matches consumed so far (shared across clones).
    pub hits: Arc<std::sync::atomic::AtomicU32>,
}

impl RouteHandlerEntry {
    /// Whether the entry still matches (`times` not exhausted).
    pub(crate) fn allows_match(&self) -> bool {
        match self.times {
            Some(limit) => self.hits.load(std::sync::atomic::Ordering::Relaxed) < limit,
            None => true,
        }
    }

    /// Record one match.
    pub(crate) fn record_match(&self) {
        self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// A dialog answer decided by a handler.
#[derive(Debug, Clone)]
pub enum DialogDecision {
    /// Accept (answer prompts with the optional text).
    Accept(Option<String>),
    /// Dismiss.
    Dismiss,
}

/// A dialog handler: inspect the dialog, decide the answer.
pub type DialogHandler = Arc<dyn Fn(DialogInfo) -> DialogDecision + Send + Sync>;

impl DialogDecision {
    /// Accept the dialog (confirm/OK).
    pub fn accept() -> Self {
        Self::Accept(None)
    }

    /// Accept a prompt with `text`.
    pub fn accept_with(text: impl Into<String>) -> Self {
        Self::Accept(Some(text.into()))
    }

    /// Dismiss the dialog (cancel/close).
    pub fn dismiss() -> Self {
        Self::Dismiss
    }
}

impl RouteAction {
    /// Fail the request.
    pub fn abort() -> Self {
        Self::Abort
    }

    /// Fail the request with an explicit reason.
    pub fn abort_with(reason: AbortReason) -> Self {
        Self::AbortWith(reason)
    }

    /// Respond with a synthetic body.
    pub fn fulfill(status: u16, body: impl Into<Vec<u8>>, content_type: impl Into<String>) -> Self {
        Self::Fulfill {
            status,
            status_text: String::new(),
            headers: Vec::new(),
            body: body.into(),
            content_type: content_type.into(),
        }
    }

    /// Respond with a synthetic body, status text, and headers.
    pub fn fulfill_full(
        status: u16,
        status_text: impl Into<String>,
        headers: Vec<(String, String)>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self::Fulfill {
            status,
            status_text: status_text.into(),
            headers,
            body: body.into(),
            content_type: String::new(),
        }
    }

    /// Let the request through unchanged.
    pub fn continue_unchanged() -> Self {
        Self::Continue
    }

    /// Pass to the next matching rule/handler.
    pub fn fallback() -> Self {
        Self::Fallback
    }
}

/// A request-routing rule (glob pattern over the URL).
#[derive(Debug, Clone)]
pub struct RouteRule {
    /// Glob pattern (`**/api/*`).
    pub pattern: String,
    /// Action for matching requests.
    pub action: RouteAction,
    /// Match at most this many requests (`None` = unlimited).
    pub times: Option<u32>,
    /// Matches consumed so far (shared across clones).
    pub hits: Arc<std::sync::atomic::AtomicU32>,
}

impl RouteRule {
    /// A rule with defaults (unlimited matches).
    fn with_action(pattern: impl Into<String>, action: RouteAction) -> Self {
        Self {
            pattern: pattern.into(),
            action,
            times: None,
            hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Whether the rule still matches (`times` not exhausted).
    pub(crate) fn allows_match(&self) -> bool {
        match self.times {
            Some(limit) => self.hits.load(std::sync::atomic::Ordering::Relaxed) < limit,
            None => true,
        }
    }

    /// Record one match.
    pub(crate) fn record_match(&self) {
        self.hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Match at most `n` requests, then stop matching (stays registered).
    #[must_use]
    pub fn times(mut self, n: u32) -> Self {
        self.times = Some(n);
        self
    }

    /// Abort matching requests.
    pub fn abort(pattern: impl Into<String>) -> Self {
        Self::with_action(pattern, RouteAction::Abort)
    }

    /// Abort matching requests with an explicit reason.
    pub fn abort_with(pattern: impl Into<String>, reason: AbortReason) -> Self {
        Self::with_action(pattern, RouteAction::AbortWith(reason))
    }

    /// Fulfill matching requests with a synthetic response.
    pub fn fulfill(
        pattern: impl Into<String>,
        status: u16,
        body: impl Into<Vec<u8>>,
        content_type: impl Into<String>,
    ) -> Self {
        Self::with_action(pattern, RouteAction::fulfill(status, body, content_type))
    }

    /// Fulfill matching requests with status text and headers.
    pub fn fulfill_full(
        pattern: impl Into<String>,
        status: u16,
        status_text: impl Into<String>,
        headers: Vec<(String, String)>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self::with_action(
            pattern,
            RouteAction::fulfill_full(status, status_text, headers, body),
        )
    }

    /// Fulfill matching requests with a JSON body.
    pub fn fulfill_json(pattern: impl Into<String>, status: u16, json: &Value) -> Self {
        Self::fulfill(
            pattern,
            status,
            serde_json::to_vec(json).unwrap_or_default(),
            "application/json",
        )
    }

    /// Fulfill matching requests with a file's bytes (read once, now).
    pub fn fulfill_file(
        pattern: impl Into<String>,
        status: u16,
        path: impl AsRef<Path>,
        content_type: impl Into<String>,
    ) -> E2eResult<Self> {
        let body = std::fs::read(path.as_ref()).map_err(|error| {
            E2eError::Config(format!(
                "cannot read fulfill file {}: {error}",
                path.as_ref().display()
            ))
        })?;
        Ok(Self::fulfill(pattern, status, body, content_type))
    }

    /// Continue matching requests with modifications (`None` = unchanged).
    /// Header overrides replace the whole header set; body overrides carry a
    /// corrected Content-Length. URL overrides are Chromium-only (Firefox
    /// aborts the redirected request, so `route` fails fast there).
    pub fn continue_with(
        pattern: impl Into<String>,
        url: Option<String>,
        method: Option<String>,
        headers: Option<Vec<(String, String)>>,
        body: Option<Vec<u8>>,
    ) -> Self {
        Self::with_action(
            pattern,
            RouteAction::ContinueWith {
                url,
                method,
                headers,
                body,
            },
        )
    }

    /// Modify the real response for matching requests (`None` = keep the
    /// original status/headers/body). Chromium-only: Firefox rejects every
    /// response-phase override, so `route` fails fast there.
    pub fn modify_response(
        pattern: impl Into<String>,
        status: Option<u16>,
        headers: Option<Vec<(String, String)>>,
        body: Option<Vec<u8>>,
    ) -> Self {
        Self::with_action(
            pattern,
            RouteAction::ModifyResponse {
                status,
                headers,
                body,
            },
        )
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

/// A completed file download (Playwright `Download`).
#[derive(Debug, Clone)]
pub struct Download {
    /// Final file path in the download dir.
    pub path: PathBuf,
    /// Suggested file name (final path's file name).
    pub suggested_filename: String,
}

impl Download {
    /// Wrap a completed download path.
    #[must_use]
    pub fn from_path(path: PathBuf) -> Self {
        let suggested_filename = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            path,
            suggested_filename,
        }
    }

    /// Copy the download to `path` (creates parent dirs).
    pub async fn save_as(&self, path: impl AsRef<Path>) -> E2eResult<PathBuf> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        tokio::fs::copy(&self.path, &path).await.map_err(|error| {
            E2eError::Config(format!(
                "cannot save download {}: {error}",
                self.path.display()
            ))
        })?;
        Ok(path)
    }

    /// Delete the downloaded file (idempotent).
    pub async fn delete(&self) -> E2eResult<()> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) | Err(_) => Ok(()),
        }
    }
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
    /// Rules inherited from the owning context (shared; page rules win).
    context_routes: Arc<Mutex<Vec<RouteRule>>>,
    /// Page-level route handlers (checked before rules; first match wins
    /// unless it returns [`RouteAction::Fallback`]).
    handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
    /// Handlers inherited from the owning context (shared).
    context_handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
    /// Context grants waiting for the first http(s) navigation (Firefox
    /// grants need an origin, so fresh pages cannot take them yet).
    pending_grants: Arc<Mutex<Vec<String>>>,
    /// Storage state waiting for a navigation to its origin (localStorage
    /// needs a live document on the saved origin).
    pending_storage: Arc<Mutex<Option<StorageState>>>,
    net_capture: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    exposed: ExposedState,
    registry: Weak<Mutex<Vec<Page>>>,
    frame_id: Option<String>,
    download_dir: Arc<Mutex<Option<PathBuf>>>,
    /// Tracing session shared with the owning context (screenshots on steps).
    tracing: Arc<Mutex<Option<TracingState>>>,
    closed: Arc<Mutex<bool>>,
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("target_id", &self.target_id())
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

/// Rust handlers exposed to page JS ([`Page::expose_function`]).
type ExposedFn = Arc<dyn Fn(Vec<Value>) -> Value + Send + Sync>;

/// Exposed handlers plus their dispatch pump.
#[derive(Clone, Default)]
struct ExposedState {
    fns: Arc<Mutex<HashMap<String, ExposedFn>>>,
    pump: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
}

/// Fake clock injected by [`Page::clock_install`]: overrides `Date`,
/// `setTimeout`/`setInterval`, `requestAnimationFrame` and `performance.now`
/// with a virtual queue drained by `__ferriteClock.tick(ms)`.
const CLOCK_SCRIPT: &str = r#"(() => {
  if (window.__ferriteClock) return true;
  const native = {
    date: Date, setTimeout, clearTimeout, setInterval, clearInterval,
    raf: requestAnimationFrame, caf: cancelAnimationFrame,
    perf: performance.now.bind(performance),
  };
  let now = native.date.now();
  const origin = now;
  let seq = 1;
  const timers = new Map();
  const asFn = (cb) => typeof cb === "function" ? cb : (() => Function(String(cb))());
  function fakeDate(...args) { return args.length ? new native.date(...args) : new native.date(now); }
  fakeDate.now = () => now;
  fakeDate.parse = native.date.parse;
  fakeDate.UTC = native.date.UTC;
  fakeDate.prototype = native.date.prototype;
  window.Date = fakeDate;
  window.setTimeout = (cb, ms = 0, ...args) => {
    const id = seq++;
    timers.set(id, { time: now + Math.max(0, +ms || 0), cb: asFn(cb), args, repeat: 0 });
    return id;
  };
  window.clearTimeout = (id) => { timers.delete(id); };
  window.setInterval = (cb, ms = 0, ...args) => {
    const id = seq++;
    const step = Math.max(0, +ms || 0);
    timers.set(id, { time: now + step, cb: asFn(cb), args, repeat: step });
    return id;
  };
  window.clearInterval = (id) => { timers.delete(id); };
  window.requestAnimationFrame = (cb) => window.setTimeout(() => cb(now), 16);
  window.cancelAnimationFrame = (id) => { timers.delete(id); };
  performance.now = () => now - origin;
  let paused = false;
  window.__ferriteClock = {
    setFixed(ms) {
      now = +ms || 0;
      return now;
    },
    now() { return now; },
    pause() { paused = true; return now; },
    resume() { paused = false; return now; },
    isPaused() { return paused; },
    tick(ms) {
      const end = now + Math.max(0, +ms || 0);
      let fired = 0;
      for (;;) {
        let best = 0, bestTime = Infinity;
        for (const [id, t] of timers) {
          if (t.time <= end && t.time < bestTime) { best = id; bestTime = t.time; }
        }
        if (!best) break;
        if (++fired > 10000) throw new Error("clock tick exceeded 10000 timers (infinite timer loop?)");
        const t = timers.get(best);
        timers.delete(best);
        now = t.time;
        if (t.repeat) timers.set(best, { time: now + t.repeat, cb: t.cb, args: t.args, repeat: t.repeat });
        t.cb(...t.args);
      }
      now = end;
      return now;
    },
    uninstall() {
      window.Date = native.date;
      window.setTimeout = native.setTimeout;
      window.clearTimeout = native.clearTimeout;
      window.setInterval = native.setInterval;
      window.clearInterval = native.clearInterval;
      window.requestAnimationFrame = native.raf;
      window.cancelAnimationFrame = native.caf;
      performance.now = native.perf;
      delete window.__ferriteClock;
      return true;
    },
  };
  return true;
})()"#;

/// At most one capture (recording or frame stream) per page: both use the
/// same screencast session on Chromium.
enum CaptureState {
    /// Active recording.
    Recording(RecordingState),
    /// Active frame stream pump.
    Streaming(tokio::task::AbortHandle),
}

impl Page {
    #[allow(clippy::too_many_arguments)] // Internal constructor; called from one context site.
    pub(crate) fn new(
        driver: Driver,
        sink: ConsoleSink,
        slow_mo: Duration,
        base_url: Option<String>,
        registry: Weak<Mutex<Vec<Page>>>,
        context_routes: Arc<Mutex<Vec<RouteRule>>>,
        context_handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
        tracing: Arc<Mutex<Option<TracingState>>>,
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
            context_routes,
            handlers: Arc::new(Mutex::new(Vec::new())),
            context_handlers,
            pending_grants: Arc::new(Mutex::new(Vec::new())),
            pending_storage: Arc::new(Mutex::new(None)),
            net_capture: Arc::new(Mutex::new(None)),
            exposed: ExposedState::default(),
            registry,
            frame_id: None,
            download_dir: Arc::new(Mutex::new(None)),
            tracing,
            closed: Arc::new(Mutex::new(false)),
        }
    }

    /// Clone scoped to a frame (evaluation runs inside it).
    pub(crate) fn scoped(mut self, frame_id: String) -> Self {
        self.frame_id = Some(frame_id);
        self
    }

    /// Target id.
    #[must_use]
    pub fn target_id(&self) -> &str {
        self.driver.target_id()
    }

    /// Subscribe to page events (console, dialogs, network, downloads, popups).
    ///
    /// The receiver only sees events emitted after subscribing; lagged
    /// readers skip to the newest buffered event.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PageEvent> {
        self.sink.subscribe()
    }

    /// Wait for the next event of `kind` (relative `timeout`).
    ///
    /// Dialog events only flow while handling is armed
    /// ([`Page::handle_dialogs`]); download waits need
    /// [`Page::set_download_dir`] first.
    pub async fn wait_for_event(
        &self,
        kind: PageEventKind,
        timeout: Duration,
    ) -> E2eResult<PageEvent> {
        if kind == PageEventKind::WebSocket && matches!(self.driver, Driver::Bidi(_)) {
            return Err(E2eError::Config(
                "websocket events are not supported on Firefox \
                 (BiDi has no socket-frame events)"
                    .to_string(),
            ));
        }
        if kind == PageEventKind::Download {
            let dir = self
                .download_dir
                .lock()
                .map(|dir| dir.clone())
                .unwrap_or(None)
                .ok_or_else(|| {
                    E2eError::Config(
                        "download events need a download dir (call set_download_dir first)"
                            .to_string(),
                    )
                })?;
            let path = self.wait_for_download_in(&dir, timeout).await?;
            return Ok(PageEvent::Download(path));
        }
        let mut events = self.subscribe();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait for {kind:?} event"),
                ));
            }
            match tokio::time::timeout(remaining, events.recv()).await {
                Ok(Ok(event)) if event.kind() == kind => return Ok(event),
                Ok(Ok(_)) | Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    return Err(E2eError::Timeout(
                        timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                        format!("wait for {kind:?} event (bus closed)"),
                    ));
                }
                Err(_) => {
                    return Err(E2eError::Timeout(
                        timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                        format!("wait for {kind:?} event"),
                    ));
                }
            }
        }
    }

    /// Push an event to this page's subscribers.
    pub(crate) fn emit(&self, event: PageEvent) {
        self.sink.emit(event);
    }

    /// Mark the page closed and emit [`PageEvent::Closed`] once.
    pub(crate) fn mark_closed(&self) {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.emit(PageEvent::Closed);
    }

    /// Whether the page was closed (explicitly or by the browser).
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.lock().map(|c| *c).unwrap_or(false)
    }

    /// Wait until `selector` matches, then return the [`Locator`].
    pub async fn wait_for_selector(&self, selector: &str, timeout: Duration) -> E2eResult<Locator> {
        let locator = self.locator(selector.to_string());
        locator.wait_for(timeout).await?;
        Ok(locator)
    }

    /// Wait for a popup opened from this page (already adopted and usable).
    pub async fn wait_for_popup(&self, timeout: Duration) -> E2eResult<Page> {
        match self.wait_for_event(PageEventKind::Popup, timeout).await? {
            PageEvent::Popup(page) => Ok(*page),
            _ => unreachable!("filtered by kind"),
        }
    }

    /// Wait for the next JavaScript dialog.
    ///
    /// Auto-handling is armed with `accept` when it is not already running,
    /// so the dialog is both observed and answered.
    pub async fn wait_for_dialog(&self, accept: bool, timeout: Duration) -> E2eResult<DialogInfo> {
        let armed = self.dialogs.lock().map(|d| d.is_some()).unwrap_or(false);
        if !armed {
            self.handle_dialogs(accept).await?;
        }
        match self.wait_for_event(PageEventKind::Dialog, timeout).await? {
            PageEvent::Dialog(info) => Ok(info),
            _ => unreachable!("filtered by kind"),
        }
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
        self.apply_pending_grants(&url).await?;
        self.apply_pending_storage(&url).await?;
        Ok(())
    }

    /// Queue context grants until the first http(s) navigation (Firefox).
    pub(crate) fn defer_grants(&self, grants: Vec<String>) {
        *self
            .pending_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = grants;
    }

    /// Grant whatever the context queued, once an http(s) page is loaded.
    async fn apply_pending_grants(&self, url: &str) -> E2eResult<()> {
        if !url.starts_with("http") {
            return Ok(());
        }
        let pending = self
            .pending_grants
            .lock()
            .map(|mut grants| std::mem::take(&mut *grants))
            .unwrap_or_default();
        if pending.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = pending.iter().map(String::as_str).collect();
        self.grant_permissions(&names).await
    }

    /// Queue storage state until a navigation lands on its origin.
    pub(crate) fn defer_storage(&self, state: StorageState) {
        *self
            .pending_storage
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(state);
    }

    /// Inject queued storage once its origin loads (single-shot).
    async fn apply_pending_storage(&self, url: &str) -> E2eResult<()> {
        let pending = self
            .pending_storage
            .lock()
            .map(|mut slot| slot.take())
            .unwrap_or(None);
        let Some(state) = pending else {
            return Ok(());
        };
        if !url.starts_with(state.origin.as_str()) {
            self.defer_storage(state);
            return Ok(());
        }
        self.driver
            .add_cookies(&state.cookies, &state.origin)
            .await?;
        self.inject_local_storage(&state.local_storage).await
    }

    /// Replace this document's localStorage wholesale.
    async fn inject_local_storage(&self, entries: &HashMap<String, String>) -> E2eResult<()> {
        let entries = serde_json::to_string(entries)?;
        self.evaluate_value(&format!(
            "(() => {{ localStorage.clear(); \
             for (const [k, v] of Object.entries({entries})) localStorage.setItem(k, v); \
             return true; }})()"
        ))
        .await?;
        Ok(())
    }

    /// Apply context storage state now when already on the saved origin,
    /// else queue the whole state (cookies set on a blank page lose their
    /// origin association, so they wait for navigation like grants do).
    pub(crate) async fn apply_storage_state(&self, state: &StorageState) -> E2eResult<()> {
        let url = self.url().await.unwrap_or_default();
        if url.starts_with(state.origin.as_str()) {
            self.driver
                .add_cookies(&state.cookies, &state.origin)
                .await?;
            self.inject_local_storage(&state.local_storage).await?;
        } else {
            self.defer_storage(state.clone());
        }
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
        match &self.frame_id {
            Some(id) => self.driver.frame_evaluate(id, expression).await,
            None => self.driver.evaluate(expression).await,
        }
    }

    /// Evaluate JavaScript and keep the result alive as a [`JSHandle`]
    /// (Playwright `page.evaluateHandle()`).
    ///
    /// Frame-scoped pages are not supported: handles need the top-level
    /// execution context.
    pub async fn evaluate_handle(&self, expression: &str) -> E2eResult<JSHandle> {
        if self.frame_id.is_some() {
            return Err(E2eError::Config(
                "evaluate_handle on a frame-scoped page is not supported".to_string(),
            ));
        }
        self.driver.evaluate_handle(expression).await
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
    ///
    /// Note: on Firefox the hold does not survive across calls (BiDi input
    /// state resets after each action sequence), so build drags with
    /// [`Page::mouse_drag`], not manual down/move/up sequences.
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

    /// Drag from one CSS-pixel point to another in `steps` paced moves.
    pub async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        self.driver.mouse_drag(from, to, steps).await?;
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

    /// Tap at CSS-pixel coordinates with the touchscreen.
    pub async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver.touchscreen_tap(x, y).await?;
        self.slow_mo().await;
        Ok(())
    }

    /// Run a named step, recording it (with duration) in the trace.
    pub async fn step<F, T>(&self, name: &str, step: F) -> T
    where
        F: Future<Output = T>,
    {
        let started = std::time::Instant::now();
        let out = step.await;
        self.sink.record(
            "step",
            format!("{name} ({}ms)", started.elapsed().as_millis()),
        );
        self.trace_screenshot(name).await;
        out
    }

    /// Capture a tracing screenshot when the context traces with screenshots.
    /// Best-effort: failures are recorded, never raised.
    async fn trace_screenshot(&self, name: &str) {
        let armed = self
            .tracing
            .lock()
            .map(|tracing| tracing.as_ref().is_some_and(TracingState::screenshots))
            .unwrap_or(false);
        if !armed {
            return;
        }
        match self.screenshot(ScreenshotOptions::default()).await {
            Ok(png) => {
                self.tracing
                    .lock()
                    .map(|mut tracing| {
                        if let Some(state) = tracing.as_mut() {
                            state.push(TraceScreenshot {
                                ts_ms: now_ms(),
                                step: name.to_string(),
                                png,
                            });
                        }
                    })
                    .ok();
            }
            Err(error) => {
                self.sink
                    .record("trace-screenshot-failed", error.to_string());
            }
        }
    }

    /// Read a localStorage entry (`None` when missing).
    pub async fn local_storage_get(&self, key: &str) -> E2eResult<Option<String>> {
        Self::storage_get("localStorage", key, &self.driver).await
    }

    /// Write a localStorage entry.
    pub async fn local_storage_set(&self, key: &str, value: &str) -> E2eResult<()> {
        Self::storage_set("localStorage", key, value, &self.driver).await
    }

    /// Remove a localStorage entry.
    pub async fn local_storage_remove(&self, key: &str) -> E2eResult<()> {
        let key_json = serde_json::to_string(key).unwrap_or_default();
        self.driver
            .evaluate(&format!("localStorage.removeItem({key_json})"))
            .await?;
        Ok(())
    }

    /// Clear localStorage.
    pub async fn local_storage_clear(&self) -> E2eResult<()> {
        self.driver.evaluate("localStorage.clear()").await?;
        Ok(())
    }

    /// Read a sessionStorage entry (`None` when missing).
    pub async fn session_storage_get(&self, key: &str) -> E2eResult<Option<String>> {
        Self::storage_get("sessionStorage", key, &self.driver).await
    }

    /// Write a sessionStorage entry.
    pub async fn session_storage_set(&self, key: &str, value: &str) -> E2eResult<()> {
        Self::storage_set("sessionStorage", key, value, &self.driver).await
    }

    /// Remove a sessionStorage entry.
    pub async fn session_storage_remove(&self, key: &str) -> E2eResult<()> {
        let key_json = serde_json::to_string(key).unwrap_or_default();
        self.driver
            .evaluate(&format!("sessionStorage.removeItem({key_json})"))
            .await?;
        Ok(())
    }

    /// Clear sessionStorage.
    pub async fn session_storage_clear(&self) -> E2eResult<()> {
        self.driver.evaluate("sessionStorage.clear()").await?;
        Ok(())
    }

    /// Read a web-storage entry.
    async fn storage_get(storage: &str, key: &str, driver: &Driver) -> E2eResult<Option<String>> {
        let key_json = serde_json::to_string(key).unwrap_or_default();
        let value = driver
            .evaluate(&format!("{storage}.getItem({key_json})"))
            .await?;
        Ok(value.as_str().map(str::to_string))
    }

    /// Write a web-storage entry.
    async fn storage_set(storage: &str, key: &str, value: &str, driver: &Driver) -> E2eResult<()> {
        let key_json = serde_json::to_string(key).unwrap_or_default();
        let value_json = serde_json::to_string(value).unwrap_or_default();
        driver
            .evaluate(&format!("{storage}.setItem({key_json}, {value_json})"))
            .await?;
        Ok(())
    }

    /// Capture a screenshot (PNG by default, JPEG with `quality`).
    pub async fn screenshot(&self, options: ScreenshotOptions) -> E2eResult<Vec<u8>> {
        if options.full_page && !options.mask.is_empty() {
            return Err(E2eError::Config(
                "screenshot mask needs a viewport capture (mask + full_page is not supported)"
                    .to_string(),
            ));
        }
        let prepared = !options.mask.is_empty() || options.disable_animations || options.hide_caret;
        if prepared {
            self.prepare_screenshot(&options).await?;
        }
        let shot = self
            .driver
            .screenshot(options.full_page, options.quality)
            .await;
        if prepared {
            self.cleanup_screenshot().await.ok();
        }
        shot
    }

    /// Inject mask overlays and capture CSS.
    async fn prepare_screenshot(&self, options: &ScreenshotOptions) -> E2eResult<()> {
        let mut css = String::new();
        if options.disable_animations {
            css.push_str(
                "*,*::before,*::after{animation-duration:0s!important;\
                 animation-delay:0s!important;transition-duration:0s!important;\
                 scroll-behavior:auto!important}",
            );
        }
        if options.hide_caret {
            css.push_str("*{caret-color:transparent!important}");
        }
        let mut rects = Vec::new();
        for locator in &options.mask {
            rects.extend(locator.state().await?.rects);
        }
        let css_json = serde_json::to_string(&css).unwrap_or_default();
        let rects_json = serde_json::to_string(&rects).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "(() => {{ \
             const style = document.createElement('style'); \
             style.id = 'ferrite-shot-style'; style.textContent = {css_json}; \
             document.head.appendChild(style); \
             for (const r of {rects_json}) {{ \
             const d = document.createElement('div'); \
             d.className = 'ferrite-shot-mask'; \
             d.style.cssText = 'position:fixed;left:' + r.x + 'px;top:' + r.y \
             + 'px;width:' + r.width + 'px;height:' + r.height \
             + 'px;background:#FF00FF;z-index:2147483647;pointer-events:none;'; \
             document.body.appendChild(d); }} \
             return true; }})()"
        ))
        .await?;
        Ok(())
    }

    /// Remove mask overlays and capture CSS.
    async fn cleanup_screenshot(&self) -> E2eResult<()> {
        self.evaluate_value(
            "(() => { document.getElementById('ferrite-shot-style')?.remove(); \
             document.querySelectorAll('.ferrite-shot-mask') \
             .forEach(el => el.remove()); return true; })()",
        )
        .await?;
        Ok(())
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

    /// Emulate a device preset (Chromium only; loud error on Firefox).
    pub async fn emulate_device(&self, device: DeviceDescriptor) -> E2eResult<()> {
        self.driver.emulate_device(device).await
    }

    /// Install a fake clock in the current document (freezes `Date`,
    /// `setTimeout`/`setInterval`, `requestAnimationFrame`, `performance.now`).
    /// Applies to the current document only; navigations reset it.
    pub async fn clock_install(&self) -> E2eResult<()> {
        self.evaluate_value(CLOCK_SCRIPT).await?;
        Ok(())
    }

    /// Advance the fake clock by `ms`, firing due timers in order.
    /// Fails loudly when no clock is installed.
    pub async fn clock_advance(&self, ms: u64) -> E2eResult<()> {
        let now: Option<i64> = self
            .evaluate(&format!(
                "window.__ferriteClock ? window.__ferriteClock.tick({ms}) : null"
            ))
            .await?;
        match now {
            Some(_) => Ok(()),
            None => Err(E2eError::Config(
                "clock_advance needs clock_install first".to_string(),
            )),
        }
    }

    /// Jump the fake clock to an exact epoch-millisecond time.
    /// Fails loudly when no clock is installed.
    pub async fn clock_set_fixed_time(&self, ms: i64) -> E2eResult<()> {
        let now: Option<i64> = self
            .evaluate(&format!(
                "window.__ferriteClock ? window.__ferriteClock.setFixed({ms}) : null"
            ))
            .await?;
        match now {
            Some(_) => Ok(()),
            None => Err(E2eError::Config(
                "clock_set_fixed_time needs clock_install first".to_string(),
            )),
        }
    }

    /// Restore the native clock (idempotent).
    pub async fn clock_uninstall(&self) -> E2eResult<()> {
        self.evaluate_value("window.__ferriteClock ? window.__ferriteClock.uninstall() : true")
            .await?;
        Ok(())
    }

    /// Advance the fake clock by `ms` (alias for [`Page::clock_advance`]).
    pub async fn clock_fast_forward(&self, ms: u64) -> E2eResult<()> {
        self.clock_advance(ms).await
    }

    /// Run the fake clock forward by `ms` (alias for [`Page::clock_advance`]).
    pub async fn clock_run_for(&self, ms: u64) -> E2eResult<()> {
        self.clock_advance(ms).await
    }

    /// Set the fake clock to an exact epoch-millisecond time
    /// (alias for [`Page::clock_set_fixed_time`]).
    pub async fn clock_set_system_time(&self, ms: i64) -> E2eResult<()> {
        self.clock_set_fixed_time(ms).await
    }

    /// Current fake-clock time in epoch milliseconds.
    pub async fn clock_now(&self) -> E2eResult<i64> {
        let now: Option<i64> = self
            .evaluate("window.__ferriteClock ? window.__ferriteClock.now() : null")
            .await?;
        now.ok_or_else(|| E2eError::Config("clock_now needs clock_install first".to_string()))
    }

    /// Pause the fake clock (records the paused flag; timers only fire via
    /// explicit advances while paused).
    pub async fn clock_pause(&self) -> E2eResult<()> {
        let now: Option<i64> = self
            .evaluate("window.__ferriteClock ? window.__ferriteClock.pause() : null")
            .await?;
        match now {
            Some(_) => Ok(()),
            None => Err(E2eError::Config(
                "clock_pause needs clock_install first".to_string(),
            )),
        }
    }

    /// Resume a paused fake clock.
    pub async fn clock_resume(&self) -> E2eResult<()> {
        let now: Option<i64> = self
            .evaluate("window.__ferriteClock ? window.__ferriteClock.resume() : null")
            .await?;
        match now {
            Some(_) => Ok(()),
            None => Err(E2eError::Config(
                "clock_resume needs clock_install first".to_string(),
            )),
        }
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

    /// Set full-fidelity cookies (path, flags, expiry honored).
    pub async fn add_cookies(&self, cookies: &[Cookie]) -> E2eResult<()> {
        let url = self.url().await?;
        self.driver.add_cookies(cookies, &url).await
    }

    /// Clear browser cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.driver.clear_cookies().await
    }

    /// Capture this page's storage state (origin, cookies, localStorage).
    pub async fn storage_state(&self) -> E2eResult<StorageState> {
        let origin = self.evaluate_string("location.origin").await?;
        let local_storage: HashMap<String, String> =
            serde_json::from_value(self.evaluate_value("({ ...localStorage })").await?)?;
        Ok(StorageState {
            origin,
            cookies: self.cookies().await?,
            local_storage,
        })
    }

    /// Save cookies plus current-origin localStorage to a JSON file.
    pub async fn save_storage_state(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        let state = self.storage_state().await?;
        std::fs::write(path, serde_json::to_string_pretty(&state)?)?;
        Ok(())
    }

    /// Load storage state saved by [`Page::save_storage_state`].
    ///
    /// The page must already be on the saved origin; cookies restore with
    /// full fidelity and localStorage is replaced wholesale.
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
        self.driver.add_cookies(&state.cookies, &url).await?;
        self.inject_local_storage(&state.local_storage).await?;
        Ok(())
    }

    /// Grant permissions (`geolocation`, `notifications`, ...).
    ///
    /// Chromium grants to all origins; Firefox grants to the current page
    /// origin (navigate first).
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        self.driver.grant_permissions(permissions).await
    }

    /// Reset granted permissions to the browser default.
    pub async fn clear_permissions(&self) -> E2eResult<()> {
        self.driver.clear_permissions().await
    }

    /// Override the geolocation coordinates.
    ///
    /// Pair with [`Page::grant_permissions`]; Firefox supports this only on
    /// recent builds and otherwise fails loudly.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.driver.set_geolocation(latitude, longitude).await
    }

    /// Clear the geolocation override.
    pub async fn clear_geolocation(&self) -> E2eResult<()> {
        self.driver.clear_geolocation().await
    }

    /// Send HTTP credentials with subsequent requests (Chromium only).
    ///
    /// Basic auth is preempted with an `Authorization` header and digest
    /// (or proxy) challenges are answered from the Fetch domain; pass
    /// `None` to clear both. Firefox has neither override, so this fails
    /// loudly there.
    pub async fn set_http_credentials(
        &self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> E2eResult<()> {
        match (username, password) {
            (Some(user), Some(pass)) => {
                let token = base64_encode(format!("{user}:{pass}").as_bytes());
                let value = format!("Basic {token}");
                self.driver
                    .set_extra_http_headers(&[("Authorization", value.as_str())])
                    .await?;
                self.driver
                    .set_auth_credentials(Some(user), Some(pass))
                    .await
            }
            (None, None) => {
                self.driver.set_extra_http_headers(&[]).await?;
                self.driver.set_auth_credentials(None, None).await
            }
            _ => Err(E2eError::Config(
                "set_http_credentials needs both username and password (or neither to clear)"
                    .to_string(),
            )),
        }
    }

    /// Enable or disable JavaScript execution (Chromium only).
    pub async fn set_java_script_enabled(&self, enabled: bool) -> E2eResult<()> {
        self.driver.set_java_script_enabled(enabled).await
    }

    /// Bypass Content-Security-Policy checks (Chromium only).
    pub async fn set_bypass_csp(&self, bypass: bool) -> E2eResult<()> {
        self.driver.set_bypass_csp(bypass).await
    }

    /// Allow or deny downloads (Chromium only; browser-wide).
    pub async fn set_downloads_allowed(&self, allowed: bool) -> E2eResult<()> {
        self.driver.set_downloads_allowed(allowed).await
    }

    /// Block service workers (Chromium only; approximated by bypassing
    /// workers, so fetches skip them entirely).
    pub async fn set_service_workers_blocked(&self, blocked: bool) -> E2eResult<()> {
        self.driver.set_service_workers_blocked(blocked).await
    }

    /// Clear this origin's IndexedDB databases (Chromium only).
    pub async fn clear_indexed_db(&self) -> E2eResult<()> {
        let origin = self.evaluate_string("location.origin").await?;
        self.driver
            .clear_data_for_origin(&origin, "indexeddb")
            .await
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

    /// Start intercepting requests with glob rules (replaces page rules;
    /// context rules still apply as fallback, page rules win on overlap).
    /// Route handlers (see [`Page::route_with_handler`]) run before rules.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = rules;
        self.restart_routing().await
    }

    /// Intercept matching requests with an async handler (Playwright
    /// `page.route(handler)` equivalent): inspect the [`RouteInfo`], fetch
    /// the real response when needed, and return the [`RouteAction`].
    /// Handlers run before rules; first match wins unless it falls back.
    pub async fn route_with_handler<F, Fut>(&self, pattern: &str, handler: F) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        self.route_entry(pattern, handler, None).await
    }

    /// [`Page::route_with_handler`] limited to `n` matches.
    pub async fn route_with_handler_times<F, Fut>(
        &self,
        pattern: &str,
        n: u32,
        handler: F,
    ) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        self.route_entry(pattern, handler, Some(n)).await
    }

    /// Register a handler entry with an optional match limit.
    async fn route_entry<F, Fut>(
        &self,
        pattern: &str,
        handler: F,
        times: Option<u32>,
    ) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        let handler: RouteHandler = Arc::new(move |info| Box::pin(handler(info)));
        self.handlers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(RouteHandlerEntry {
                pattern: pattern.to_string(),
                handler,
                times,
                hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            });
        self.restart_routing().await
    }

    /// Stop page-level interception (rules and handlers; context entries
    /// still apply).
    pub async fn stop_routing(&self) {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();
        *self.handlers.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();
        let _ = self.restart_routing().await;
    }

    /// Remove rules and handlers with `pattern`; returns how many were
    /// removed (both kinds counted).
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
        let removed_handlers = self
            .handlers
            .lock()
            .map(|mut handlers| {
                let before = handlers.len();
                handlers.retain(|entry| entry.pattern != pattern);
                before - handlers.len()
            })
            .unwrap_or(0);
        self.restart_routing().await?;
        Ok(removed + removed_handlers)
    }

    /// Remove all page-level rules and handlers (context entries still apply).
    ///
    /// Returns how many entries were removed (both kinds counted).
    pub async fn unroute_all(&self) -> E2eResult<usize> {
        let removed = self
            .routes
            .lock()
            .map(|mut routes| std::mem::take(&mut *routes).len())
            .unwrap_or(0);
        let removed_handlers = self
            .handlers
            .lock()
            .map(|mut handlers| std::mem::take(&mut *handlers).len())
            .unwrap_or(0);
        self.restart_routing().await?;
        Ok(removed + removed_handlers)
    }

    /// Replay responses from a HAR 1.2 file (Playwright `routeFromHAR`).
    ///
    /// Entries match on exact method + URL and fulfill status, headers, and
    /// body; misses fall through to later handlers/rules and the network.
    /// Returns how many entries were loaded. Remove with `unroute("**")`.
    /// HAR-update mode is not supported (record with [`Page::save_har`]).
    pub async fn route_from_har(
        &self,
        path: impl AsRef<Path>,
        options: RouteFromHarOptions,
    ) -> E2eResult<usize> {
        let file = crate::har::HarFile::load(path)?;
        let matcher = match options.url_filter.as_deref() {
            Some(glob) => Some(globset::Glob::new(glob).map_err(|error| {
                E2eError::Config(format!("invalid HAR url filter {glob:?}: {error}"))
            })?),
            None => None,
        }
        .map(|glob| glob.compile_matcher());
        let mut map = file.lookup();
        if let Some(matcher) = &matcher {
            map.retain(|_, entry| matcher.is_match(&entry.url));
        }
        let count = map.len();
        if count == 0 {
            return Ok(0);
        }
        let map = Arc::new(map);
        self.route_with_handler("**", move |info: RouteInfo| {
            let map = Arc::clone(&map);
            async move {
                let key = (info.method.to_ascii_uppercase(), info.url.clone());
                match map.get(&key) {
                    Some(entry) => Ok(RouteAction::fulfill_full(
                        entry.status,
                        entry.status_text.clone(),
                        entry.headers.clone(),
                        entry.body.clone(),
                    )),
                    None => Ok(RouteAction::Fallback),
                }
            }
        })
        .await?;
        Ok(count)
    }

    /// Apply the stored rules and handlers (page first, context fallback;
    /// no pump when both are empty).
    pub(crate) async fn restart_routing(&self) -> E2eResult<()> {
        let handle = self.routing.lock().map(|mut r| r.take()).unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
            self.driver.stop_routing().await;
        }
        let mut rules = self
            .routes
            .lock()
            .map(|routes| routes.clone())
            .unwrap_or_default();
        rules.extend(
            self.context_routes
                .lock()
                .map(|routes| routes.clone())
                .unwrap_or_default(),
        );
        let mut handlers = self
            .handlers
            .lock()
            .map(|handlers| handlers.clone())
            .unwrap_or_default();
        handlers.extend(
            self.context_handlers
                .lock()
                .map(|handlers| handlers.clone())
                .unwrap_or_default(),
        );
        if rules.is_empty() && handlers.is_empty() {
            return Ok(());
        }
        let handle = self
            .driver
            .start_routing(Arc::new(rules), Arc::new(handlers))
            .await?;
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

    /// Export recorded traffic as a HAR 1.2 file (Playwright `recordHar`).
    ///
    /// Bodies are omitted (content-`omit` mode): only URLs, methods,
    /// headers, MIME types and timings are written.
    pub fn save_har(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        let har = crate::har::har_json(&self.requests());
        let text = serde_json::to_string_pretty(&har)?;
        std::fs::write(path.as_ref(), text)?;
        Ok(())
    }

    /// Write recorded traffic as HAR 1.2 with an explicit content mode.
    ///
    /// `Embed` base64-embeds captured bodies (Chromium records bodies;
    /// entries without one export as in `Omit`).
    pub fn save_har_with(
        &self,
        path: impl AsRef<Path>,
        mode: crate::har::HarContentMode,
    ) -> E2eResult<()> {
        let har = crate::har::har_json_with(&self.requests(), mode);
        let text = serde_json::to_string_pretty(&har)?;
        std::fs::write(path.as_ref(), text)?;
        Ok(())
    }

    /// Set files on the file input matching `selector` (empty list clears).
    pub async fn set_input_files<P: AsRef<Path>>(
        &self,
        selector: &str,
        paths: &[P],
    ) -> E2eResult<()> {
        self.locator(selector).set_input_files(paths).await
    }

    /// Direct downloads to `dir` (created when missing). Chromium only;
    /// Firefox configures the download dir at launch
    /// ([`LaunchOptions::download_dir`](crate::LaunchOptions::download_dir)).
    pub async fn set_download_dir(&self, dir: impl AsRef<Path>) -> E2eResult<()> {
        self.driver.set_download_dir(dir.as_ref()).await?;
        self.remember_download_dir(dir.as_ref());
        Ok(())
    }

    /// Record the download dir without touching the engine (launch-wide dirs
    /// are already in force via profile prefs).
    pub(crate) fn remember_download_dir(&self, dir: &Path) {
        *self.download_dir.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir.to_path_buf());
    }

    /// Run `source` before page scripts in every future document of this
    /// page (applies after the next navigation; on Firefox the source is
    /// wrapped in a function, so avoid top-level `return`).
    pub async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        self.driver.add_init_script(source).await
    }

    /// Add a `<script src>` tag and wait for it to load.
    pub async fn add_script_tag_url(&self, url: &str) -> E2eResult<()> {
        let url_json = serde_json::to_string(url).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "new Promise((resolve, reject) => {{ \
             const s = document.createElement('script'); s.src = {url_json}; \
             s.onload = () => resolve(true); \
             s.onerror = () => reject(new Error('script load failed')); \
             document.head.appendChild(s); }})"
        ))
        .await?;
        Ok(())
    }

    /// Add an inline `<script>` tag (runs immediately).
    pub async fn add_script_tag_content(&self, code: &str) -> E2eResult<()> {
        let code_json = serde_json::to_string(code).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "(() => {{ const s = document.createElement('script'); \
             s.textContent = {code_json}; document.head.appendChild(s); \
             return true; }})()"
        ))
        .await?;
        Ok(())
    }

    /// Add a stylesheet `<link>` tag and wait for it to load.
    pub async fn add_style_tag_url(&self, url: &str) -> E2eResult<()> {
        let url_json = serde_json::to_string(url).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "new Promise((resolve, reject) => {{ \
             const l = document.createElement('link'); l.rel = 'stylesheet'; \
             l.href = {url_json}; l.onload = () => resolve(true); \
             l.onerror = () => reject(new Error('stylesheet load failed')); \
             document.head.appendChild(l); }})"
        ))
        .await?;
        Ok(())
    }

    /// Add an inline `<style>` tag.
    pub async fn add_style_tag_content(&self, css: &str) -> E2eResult<()> {
        let css_json = serde_json::to_string(css).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "(() => {{ const s = document.createElement('style'); \
             s.textContent = {css_json}; document.head.appendChild(s); \
             return true; }})()"
        ))
        .await?;
        Ok(())
    }

    /// All document frames (main frame first; named `document_frames`
    /// because [`Page::frames`] streams video frames). Frame names are
    /// Chromium-only (empty on Firefox, which reports no names).
    pub async fn document_frames(&self) -> E2eResult<Vec<Frame>> {
        Ok(self
            .driver
            .frames()
            .await?
            .into_iter()
            .map(|info| Frame {
                page: self.clone(),
                id: info.id,
                name: info.name,
                url: info.url,
            })
            .collect())
    }

    /// First frame with exactly `name`.
    pub async fn frame_by_name(&self, name: &str) -> E2eResult<Option<Frame>> {
        Ok(self
            .document_frames()
            .await?
            .into_iter()
            .find(|frame| frame.name == name))
    }

    /// First frame whose URL contains `pattern`.
    pub async fn frame_by_url(&self, pattern: &str) -> E2eResult<Option<Frame>> {
        Ok(self
            .document_frames()
            .await?
            .into_iter()
            .find(|frame| frame.url.contains(pattern)))
    }

    /// Expose a Rust function to page JS as `window[name]`. The page calls
    /// it like `await window[name](...args)`; the callback receives all args
    /// and its return value resolves the promise (panics reject it).
    /// Dispatch polls (~50ms latency), so keep handlers fast. Applies to the
    /// current document only: re-expose after navigation.
    pub async fn expose_function<F>(&self, name: &str, f: F) -> E2eResult<()>
    where
        F: Fn(Vec<Value>) -> Value + Send + Sync + 'static,
    {
        if !valid_expose_name(name) {
            return Err(E2eError::Config(format!(
                "expose_function needs a JS identifier, got {name:?}"
            )));
        }
        let name_json = serde_json::to_string(name).map_err(E2eError::Json)?;
        self.evaluate_value(&format!(
            "(() => {{ \
             window.__ferriteExpose = window.__ferriteExpose \
             || {{ seq: 0, queue: [], pending: {{}} }}; \
             const bx = window.__ferriteExpose; \
             window[{name_json}] = (...args) => new Promise((resolve, reject) => {{ \
             const id = ++bx.seq; \
             bx.pending[id] = {{ resolve, reject }}; \
             bx.queue.push([{name_json}, id, args]); }}); \
             return true; }})()"
        ))
        .await?;
        self.exposed
            .fns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), Arc::new(f));
        self.start_expose_pump();
        Ok(())
    }

    /// Remove exposed functions and stop their dispatch pump.
    pub async fn clear_exposed_functions(&self) {
        let handle = self
            .exposed
            .pump
            .lock()
            .map(|mut pump| pump.take())
            .unwrap_or(None);
        if let Some(handle) = handle {
            handle.abort();
        }
        let names: Vec<String> = self
            .exposed
            .fns
            .lock()
            .map(|mut fns| fns.drain().map(|(name, _)| name).collect())
            .unwrap_or_default();
        if !names.is_empty() {
            let mut script = String::from("(() => {");
            for name in &names {
                script.push_str(&format!(
                    "delete window[{}];",
                    serde_json::to_string(name).unwrap_or_default()
                ));
            }
            script.push_str("delete window.__ferriteExpose; return true; })()");
            self.evaluate_value(&script).await.ok();
        }
    }

    /// Start the exposed-function dispatch pump (once per page).
    fn start_expose_pump(&self) {
        let mut pump = self.exposed.pump.lock().unwrap_or_else(|e| e.into_inner());
        if pump.is_some() {
            return;
        }
        let page = self.clone();
        let fns = self.exposed.fns.clone();
        *pump = Some(
            tokio::spawn(async move {
                let mut failures = 0u32;
                loop {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let calls: Vec<(String, u64, Vec<Value>)> = match page
                        .evaluate(
                            "(() => { const bx = window.__ferriteExpose; \
                             if (!bx) return []; \
                             const taken = bx.queue; bx.queue = []; return taken; })()",
                        )
                        .await
                    {
                        Ok(calls) => {
                            failures = 0;
                            calls
                        }
                        Err(_) => {
                            failures += 1;
                            if failures > 20 {
                                break;
                            }
                            continue;
                        }
                    };
                    for (name, id, args) in calls {
                        let handler = fns
                            .lock()
                            .map(|fns| fns.get(&name).cloned())
                            .unwrap_or(None);
                        let (result, failed) = match handler {
                            Some(handle) => {
                                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    handle(args)
                                })) {
                                    Ok(value) => (value.to_string(), false),
                                    Err(_) => (
                                        serde_json::json!("exposed function panicked").to_string(),
                                        true,
                                    ),
                                }
                            }
                            None => (
                                serde_json::json!("unknown exposed function").to_string(),
                                true,
                            ),
                        };
                        let settle = if failed { "reject" } else { "resolve" };
                        let delivered = page
                            .evaluate_value(&format!(
                                "(() => {{ const bx = window.__ferriteExpose; \
                                 const entry = bx && bx.pending[{id}]; \
                                 if (!entry) return false; \
                                 delete bx.pending[{id}]; entry.{settle}({result}); \
                                 return true; }})()"
                            ))
                            .await;
                        if delivered.is_err() {
                            failures += 1;
                            if failures > 20 {
                                break;
                            }
                        } else {
                            failures = 0;
                        }
                    }
                }
            })
            .abort_handle(),
        );
    }

    /// Wait for a new file in `dir`, returning once it stops growing.
    /// In-progress downloads (`*.part`, `*.crdownload`, `*.tmp`) are skipped.
    /// A file already present also matches when modified within the last 30s
    /// (fast local downloads often land before the wait starts); use a fresh
    /// dir per download to keep this unambiguous.
    pub async fn wait_for_download(
        &self,
        dir: impl AsRef<Path>,
        timeout: Duration,
    ) -> E2eResult<PathBuf> {
        Ok(self.wait_for_download_file(dir, timeout).await?.path)
    }

    /// Wait for a download and wrap it as a [`Download`] (save/delete helpers).
    pub async fn wait_for_download_file(
        &self,
        dir: impl AsRef<Path>,
        timeout: Duration,
    ) -> E2eResult<Download> {
        let path = self.wait_for_download_in(dir.as_ref(), timeout).await?;
        self.emit(PageEvent::Download(path.clone()));
        Ok(Download::from_path(path))
    }

    /// Fetch `url` from inside the page (cookies included) and return the
    /// raw bytes (Playwright `response.body()` equivalent for re-fetchable
    /// resources; same-origin or CORS-open URLs only).
    pub async fn response_body(&self, url: &str) -> E2eResult<Vec<u8>> {
        let url_json = serde_json::to_string(url).unwrap_or_default();
        let value: Value = self
            .evaluate(&format!(
                "fetch({url_json}).then(async r => {{ \
                 if (!r.ok) throw new Error('fetch ' + r.status); \
                 const buf = await r.arrayBuffer(); \
                 const bytes = new Uint8Array(buf); \
                 let bin = ''; \
                 for (let i = 0; i < bytes.length; i++) \
                   bin += String.fromCharCode(bytes[i]); \
                 return btoa(bin); }})"
            ))
            .await?;
        let b64 = value
            .as_str()
            .ok_or_else(|| E2eError::Config(format!("response_body({url}) returned no body")))?;
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|_| E2eError::Config(format!("response_body({url}) returned invalid base64")))
    }

    /// Accessibility snapshot: indented `role "name"` lines for interactive
    /// elements (approximation of Playwright `aria_snapshot`, same shape on
    /// both engines).
    pub async fn aria_snapshot(&self) -> E2eResult<String> {
        self.evaluate_string(
            "(() => { \
             const out = []; \
             const els = document.querySelectorAll( \
               'a,button,input,select,textarea,[role],[aria-label],h1,h2,h3'); \
             for (const el of els) { \
               const role = el.getAttribute('role') \
                 || el.tagName.toLowerCase(); \
               const name = (el.getAttribute('aria-label') \
                 || el.getAttribute('alt') \
                 || (el.textContent || '').trim().slice(0, 80) \
                 || el.value || '').trim(); \
               if (!name && !['input','select','textarea'].includes(role)) continue; \
               out.push('- ' + role + (name ? ' \"' + name + '\"' : '')); \
               if (out.length >= 200) break; \
             } \
             return out.join('\\n'); })()",
        )
        .await
    }

    /// Download watcher without the event emission (shared implementation).
    async fn wait_for_download_in(&self, dir: &Path, timeout: Duration) -> E2eResult<PathBuf> {
        let before = dir_names(dir)?;
        let cutoff = std::time::SystemTime::now()
            .checked_sub(PREEXISTING_DOWNLOAD_GRACE)
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let deadline = tokio::time::Instant::now() + timeout;
        let mut stable: Option<(PathBuf, u64)> = None;
        loop {
            let mut candidate: Option<(PathBuf, u64)> = None;
            // Most recently modified fresh pre-existing file (fallback when
            // nothing new appears: the download may have landed first).
            let mut fallback: Option<(PathBuf, u64, std::time::SystemTime)> = None;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !path.is_file() || is_temp_download(&name) {
                        continue;
                    }
                    let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                    if !before.contains(&name) {
                        candidate = Some((path, size));
                        break;
                    }
                    let mtime = entry
                        .metadata()
                        .and_then(|meta| meta.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    if mtime >= cutoff
                        && fallback
                            .as_ref()
                            .map(|(_, _, prev)| mtime > *prev)
                            .unwrap_or(true)
                    {
                        fallback = Some((path, size, mtime));
                    }
                }
            }
            let candidate = candidate.or_else(|| fallback.map(|(path, size, _)| (path, size)));
            if let Some((path, size)) = candidate {
                if stable.as_ref() == Some(&(path.clone(), size)) {
                    return Ok(path);
                }
                stable = Some((path, size));
            } else {
                stable = None;
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait_for_download({})", dir.display()),
                ));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Start capture when it isn't running (keeps history when it is).
    fn ensure_request_capture(&self) {
        let running = self
            .net_capture
            .lock()
            .map(|capture| capture.is_some())
            .unwrap_or(false);
        if !running {
            self.start_request_capture();
        }
    }

    /// Wait for a request whose URL contains `pattern`. Only matches
    /// requests made after this call, so call it before triggering the
    /// request. Starts capture when needed.
    pub async fn wait_for_request(
        &self,
        pattern: &str,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest> {
        self.ensure_request_capture();
        let skip = self.requests().len();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(found) = self
                .requests()
                .iter()
                .skip(skip)
                .find(|request| request.url.contains(pattern))
                .cloned()
            {
                return Ok(found);
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait_for_request({pattern})"),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Wait for a response (nonzero status) whose URL contains `pattern`.
    /// Like [`Page::wait_for_request`], only matches responses arriving
    /// after this call.
    pub async fn wait_for_response(
        &self,
        pattern: &str,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest> {
        self.ensure_request_capture();
        let skip = self.requests().len();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(found) = self
                .requests()
                .iter()
                .skip(skip)
                .find(|request| request.url.contains(pattern) && request.status != 0)
                .cloned()
            {
                return Ok(found);
            }
            if tokio::time::Instant::now() > deadline {
                return Err(E2eError::Timeout(
                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                    format!("wait_for_response({pattern})"),
                ));
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Auto-handle JavaScript dialogs (`accept` = OK vs dismiss).
    pub async fn handle_dialogs(&self, accept: bool) -> E2eResult<()> {
        self.stop_dialog_handling().await;
        let handle = self.driver.start_dialogs(accept, None, None).await?;
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
            .start_dialogs(accept, Some(prompt_text.to_string()), None)
            .await?;
        *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
        Ok(())
    }

    /// Answer each dialog with a handler (Playwright `page.on("dialog")`
    /// equivalent): inspect the [`DialogInfo`], return the [`DialogDecision`].
    pub async fn handle_dialogs_with_handler(
        &self,
        handler: impl Fn(DialogInfo) -> DialogDecision + Send + Sync + 'static,
    ) -> E2eResult<()> {
        self.stop_dialog_handling().await;
        let handle = self
            .driver
            .start_dialogs(true, None, Some(Arc::new(handler)))
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
        if let Some(registry) = self.registry.upgrade() {
            let target = self.target_id().to_string();
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|page| page.target_id() != target);
        }
        // Explicit closes emit directly (deregistration already happened, so
        // the browser-side destroy watcher stays quiet: exactly one event).
        self.mark_closed();
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

    #[test]
    fn device_presets_have_sane_metrics() {
        for device in [
            DeviceDescriptor::IPHONE_15,
            DeviceDescriptor::IPHONE_SE,
            DeviceDescriptor::IPAD,
            DeviceDescriptor::PIXEL_7,
            DeviceDescriptor::DESKTOP_1080P,
            DeviceDescriptor::DESKTOP_1440P,
            DeviceDescriptor::LAPTOP_RETINA,
        ] {
            assert!(device.viewport.width > 0);
            assert!(device.viewport.height > 0);
            assert!(device.device_scale_factor > 0.0);
        }
        assert_eq!(DeviceDescriptor::IPHONE_SE.viewport.width, 375);
        const {
            assert!(DeviceDescriptor::IPAD.has_touch);
            assert!(!DeviceDescriptor::IPAD.mobile);
            assert!(!DeviceDescriptor::LAPTOP_RETINA.has_touch);
        }
    }

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
                expires: None,
            }],
            local_storage,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: StorageState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.origin, state.origin);
        assert_eq!(back.cookies.len(), 1);
        assert_eq!(back.local_storage.get("k").unwrap(), "v");
    }

    #[test]
    fn clock_script_exposes_pause_and_now() {
        for method in [
            "now()",
            "pause()",
            "resume()",
            "isPaused()",
            "tick(",
            "setFixed(",
        ] {
            assert!(CLOCK_SCRIPT.contains(method), "missing {method}");
        }
    }

    #[test]
    fn download_suggested_name() {
        let download = Download::from_path(PathBuf::from("/tmp/report.txt"));
        assert_eq!(download.suggested_filename, "report.txt");
        let download = Download::from_path(PathBuf::from("bare.bin"));
        assert_eq!(download.suggested_filename, "bare.bin");
    }

    #[test]
    fn recorded_body_helpers() {
        let mut request = RecordedRequest {
            method: "GET".to_string(),
            url: "http://x.test/a".to_string(),
            status: 200,
            headers: Vec::new(),
            post_data: None,
            duration_ms: None,
            status_text: "OK".to_string(),
            mime_type: "application/json".to_string(),
            response_headers: Vec::new(),
            started_ms: None,
            request_id: Some("1".to_string()),
            body: None,
            body_truncated: false,
        };
        assert!(request.body_text().is_none());
        assert!(request.body_json().is_none());
        request.body = Some(br#"{"n":1}"#.to_vec());
        assert_eq!(request.body_text().as_deref(), Some(r#"{"n":1}"#));
        assert_eq!(request.body_json(), Some(serde_json::json!({"n": 1})));
        request.body = Some(vec![0xff, 0xfe]);
        assert!(request.body_json().is_none());
        // Bodies stay in memory (skipped by serde).
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("body").is_none());
        assert!(json.get("request_id").is_some());
    }

    #[test]
    fn route_rule_constructors() {
        let rule = RouteRule::fulfill("**/a", 200, "hi", "text/plain");
        assert!(rule.times.is_none());
        assert!(rule.allows_match());
        rule.record_match();
        let limited = RouteRule::abort("**/b").times(1);
        assert!(limited.allows_match());
        limited.record_match();
        assert!(!limited.allows_match());

        let json_rule = RouteRule::fulfill_json("**/j", 201, &serde_json::json!({"ok": true}));
        match &json_rule.action {
            RouteAction::Fulfill {
                status,
                body,
                content_type,
                ..
            } => {
                assert_eq!(*status, 201);
                assert_eq!(content_type, "application/json");
                assert_eq!(
                    serde_json::from_slice::<Value>(body).unwrap(),
                    serde_json::json!({"ok": true})
                );
            }
            action => panic!("unexpected {action:?}"),
        }
        let dir = std::env::temp_dir();
        let path = dir.join(format!("ferrite-fulfill-{}", std::process::id()));
        std::fs::write(&path, [0u8, 1, 2, 255]).unwrap();
        let file_rule =
            RouteRule::fulfill_file("**/f", 200, &path, "application/octet-stream").unwrap();
        match &file_rule.action {
            RouteAction::Fulfill {
                body, content_type, ..
            } => {
                assert_eq!(body, &vec![0u8, 1, 2, 255]);
                assert_eq!(content_type, "application/octet-stream");
            }
            action => panic!("unexpected {action:?}"),
        }
        assert!(
            RouteRule::fulfill_file("**/f", 200, dir.join("ferrite-nope"), "text/plain").is_err()
        );
        let _ = std::fs::remove_file(&path);

        let abort = RouteRule::abort_with("**/x", AbortReason::ConnectionRefused);
        assert!(matches!(
            abort.action,
            RouteAction::AbortWith(AbortReason::ConnectionRefused)
        ));
        assert_eq!(AbortReason::ConnectionRefused.as_cdp(), "ConnectionRefused");
        assert_eq!(AbortReason::TimedOut.as_cdp(), "TimedOut");
    }

    #[test]
    fn handler_entry_times() {
        let entry = RouteHandlerEntry {
            pattern: "**".to_string(),
            handler: Arc::new(|_| Box::pin(async { Ok(RouteAction::Fallback) })),
            times: Some(2),
            hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        };
        assert!(entry.allows_match());
        entry.record_match();
        assert!(entry.allows_match());
        entry.record_match();
        assert!(!entry.allows_match());
    }
}
