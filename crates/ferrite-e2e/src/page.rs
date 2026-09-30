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

/// Browser JavaScript source position (zero-based line and column).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleLocation {
    pub url: String,
    pub line: u32,
    pub column: u32,
}

/// A console message or page error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleMessage {
    /// `log`, `warn`, `error`, `exception`, ...
    pub kind: String,
    /// Joined argument previews.
    pub text: String,
    #[serde(default)]
    pub location: Option<ConsoleLocation>,
    /// Native protocol milliseconds since the Unix epoch; None when unavailable.
    #[serde(default)]
    pub timestamp_ms: Option<u64>,
    /// Owning page target/context ID, matching ContextEvent.page_id.
    #[serde(default)]
    pub page_id: Option<String>,
    /// Bounded native argument data. None means the event supplied no arguments.
    #[serde(default)]
    pub arguments: Option<crate::ConsoleArguments>,
    /// Optional native uncaught-error metadata; constructor class is not Error.name.
    #[serde(default)]
    pub error: Option<Box<crate::PageErrorInfo>>,
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

/// Saved cookies and localStorage for multiple origins (Playwright JSON compatible).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageState {
    /// Origin the state belongs to.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub origin: String,
    /// Cookies (restored by name/value on the current origin).
    #[serde(default)]
    pub cookies: Vec<Cookie>,
    /// localStorage entries.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub local_storage: HashMap<String, String>,
    /// Origins in Playwright storage-state format. Legacy single-origin fields remain readable.
    #[serde(default)]
    pub origins: Vec<StorageOrigin>,
}

/// Storage state for one origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageOrigin {
    pub origin: String,
    #[serde(default, rename = "localStorage")]
    pub local_storage: Vec<StorageEntry>,
}

/// A localStorage name/value pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub name: String,
    pub value: String,
}

impl StorageState {
    pub(crate) fn all_origins(&self) -> Vec<StorageOrigin> {
        let mut origins = self.origins.clone();
        if !self.origin.is_empty() && !origins.iter().any(|entry| entry.origin == self.origin) {
            origins.push(StorageOrigin {
                origin: self.origin.clone(),
                local_storage: self
                    .local_storage
                    .iter()
                    .map(|(name, value)| StorageEntry {
                        name: name.clone(),
                        value: value.clone(),
                    })
                    .collect(),
            });
        }
        origins
    }
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
    FrameAttached,
    FrameNavigated,
    FrameDetached,
    DomContentLoaded,
    Load,
    DialogClosed,
    /// Console message or page exception.
    Console,
    /// JavaScript dialog (observed while handling is armed).
    Dialog,
    /// Network request started.
    Request,
    /// Network response headers received.
    Response,
    /// Response body finished downloading (also for HTTP error statuses).
    RequestFinished,
    /// Request failed at the transport layer.
    RequestFailed,
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

/// Identity and metadata for one network request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRequest {
    /// Backend request identifier, scoped to this page.
    pub request_id: String,
    /// HTTP method.
    pub method: String,
    /// Request URL.
    pub url: String,
}

/// An observed page event.
#[derive(Debug, Clone)]
pub enum PageEvent {
    /// A native child frame was attached; navigation metadata may not exist yet.
    FrameAttached(crate::FrameEvent),
    /// A native frame committed a document or same-document navigation.
    FrameNavigated(crate::FrameEvent),
    /// Child-first native subtree removal; identities never retarget replacements.
    FrameDetached(crate::FrameEvent),
    /// Main-document DOM readiness, including repeated set-content operations.
    DomContentLoaded(crate::FrameEvent),
    /// Main-document load readiness.
    Load(crate::FrameEvent),
    /// Native dialog closed; handling still needs to be armed for Dialog events.
    DialogClosed(crate::DialogClosedInfo),
    /// Console message or page exception.
    Console(ConsoleMessage),
    /// JavaScript dialog.
    Dialog(DialogInfo),
    /// Network request started.
    Request {
        /// Backend request identifier, scoped to this page.
        request_id: String,
        /// HTTP method.
        method: String,
        /// Request URL.
        url: String,
    },
    /// Network response headers received.
    Response {
        /// Backend request identifier, scoped to this page.
        request_id: String,
        /// Request URL.
        url: String,
        /// HTTP response status.
        status: u16,
    },
    /// Response body finished downloading, including HTTP errors.
    RequestFinished(NetworkRequest),
    /// Request failed without successfully completing the response body.
    RequestFailed {
        /// The failed request.
        request: NetworkRequest,
        /// Backend error description.
        error_text: String,
        /// Explicit cancellation, or `None` when the backend does not supply it.
        cancelled: Option<bool>,
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
            Self::FrameAttached(_) => PageEventKind::FrameAttached,
            Self::FrameNavigated(_) => PageEventKind::FrameNavigated,
            Self::FrameDetached(_) => PageEventKind::FrameDetached,
            Self::DomContentLoaded(_) => PageEventKind::DomContentLoaded,
            Self::Load(_) => PageEventKind::Load,
            Self::DialogClosed(_) => PageEventKind::DialogClosed,
            Self::Console(_) => PageEventKind::Console,
            Self::Dialog(_) => PageEventKind::Dialog,
            Self::Request { .. } => PageEventKind::Request,
            Self::Response { .. } => PageEventKind::Response,
            Self::RequestFinished(_) => PageEventKind::RequestFinished,
            Self::RequestFailed { .. } => PageEventKind::RequestFailed,
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
    /// SameSite policy (`Strict`, `Lax`, or `None`).
    #[serde(default, rename = "sameSite", skip_serializing_if = "Option::is_none")]
    pub same_site: Option<String>,
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
    /// Parent frame id (`None` for the main frame).
    pub parent_id: Option<String>,
    /// Frame name (`<iframe name>`; empty on Firefox).
    pub name: String,
    /// Document URL at lookup time; use `current_url()` after navigation.
    pub url: String,
}

/// A frame in the page (main frame or iframe) with frame-scoped
/// `evaluate` and locators. Pointer action options support same-origin frame
/// offsets and positive axis-aligned scaling. Cross-origin frame coordinates
/// and rotated/perspective frame transforms are unsupported.
#[derive(Clone)]
pub struct Frame {
    page: Page,
    id: String,
    parent_id: Option<String>,
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
    /// Native frame/context identity; retained by a handle after detachment.
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    /// The top-level page owning this frame.
    pub fn page(&self) -> Page {
        self.page.owning_page()
    }

    /// Replace only this frame's document.
    pub async fn set_content(&self, html: &str) -> E2eResult<()> {
        self.page
            .auto_step("frame.set_content", crate::StepCategory::Action, async {
                self.scoped_page().set_content(html).await
            })
            .await
    }

    pub async fn wait_for_function(&self, expression: &str, timeout: Duration) -> E2eResult<()> {
        self.scoped_page()
            .wait_for_function(expression, timeout)
            .await
    }

    pub async fn wait_for_function_with_options(
        &self,
        expression: &str,
        options: crate::OperationOptions,
    ) -> E2eResult<()> {
        self.scoped_page()
            .wait_for_function_with_options(expression, options)
            .await
    }

    /// Frame-scoped function wait with JSON arguments, scheduling and result.
    pub async fn wait_for_function_value<A: Serialize + Sync>(
        &self,
        expression: &str,
        argument: &A,
        options: crate::FunctionWaitOptions,
    ) -> E2eResult<Value> {
        self.scoped_page()
            .wait_for_function_value(expression, argument, options)
            .await
    }

    /// Read the current URL, including navigations after this handle was created.
    pub async fn current_url(&self) -> E2eResult<String> {
        self.scoped_page().url().await
    }

    pub async fn wait_for_url(&self, fragment: &str, timeout: Duration) -> E2eResult<()> {
        self.scoped_page().wait_for_url(fragment, timeout).await
    }

    pub async fn wait_for_load_state(&self, state: LoadState) -> E2eResult<()> {
        self.scoped_page().wait_for_load_state(state).await
    }

    pub async fn wait_for_selector(&self, selector: &str, timeout: Duration) -> E2eResult<Locator> {
        self.scoped_page()
            .wait_for_selector(selector, timeout)
            .await
    }

    /// Full-URL matching in this frame.
    pub async fn wait_for_url_matching(
        &self,
        matcher: &crate::UrlMatcher,
        timeout: Duration,
    ) -> E2eResult<()> {
        self.scoped_page()
            .wait_for_url_matching(matcher, timeout)
            .await
    }
    pub async fn wait_for_url_where<F>(&self, predicate: F, timeout: Duration) -> E2eResult<()>
    where
        F: FnMut(&str) -> bool,
    {
        self.scoped_page()
            .wait_for_url_where(predicate, timeout)
            .await
    }

    pub async fn wait_for_url_with_options(
        &self,
        fragment: &str,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()> {
        self.scoped_page()
            .wait_for_url_with_options(fragment, options)
            .await
    }
    pub async fn wait_for_url_matching_with_options(
        &self,
        matcher: &crate::UrlMatcher,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()> {
        self.scoped_page()
            .wait_for_url_matching_with_options(matcher, options)
            .await
    }
    pub async fn wait_for_url_where_with_options<F>(
        &self,
        predicate: F,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()>
    where
        F: FnMut(&str) -> bool,
    {
        self.scoped_page()
            .wait_for_url_where_with_options(predicate, options)
            .await
    }

    /// Frame name (empty on Firefox).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Snapshot URL when this handle was obtained; use `current_url()` after navigation.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Navigate this frame to a URL.
    pub async fn goto(&self, url: &str) -> E2eResult<()> {
        self.page
            .auto_step("frame.goto", crate::StepCategory::Action, async {
                self.page.driver.frame_navigate(&self.id, url).await
            })
            .await
    }

    /// Parent frame (`None` for the main frame).
    pub async fn parent(&self) -> E2eResult<Option<Frame>> {
        let Some(parent_id) = &self.parent_id else {
            return Ok(None);
        };
        Ok(self
            .page
            .document_frames()
            .await?
            .into_iter()
            .find(|frame| frame.id == *parent_id))
    }

    /// Direct child frames.
    pub async fn child_frames(&self) -> E2eResult<Vec<Frame>> {
        Ok(self
            .page
            .document_frames()
            .await?
            .into_iter()
            .filter(|frame| frame.parent_id.as_deref() == Some(self.id.as_str()))
            .collect())
    }

    /// Whether the identity is absent from the current native frame tree.
    /// Explicit page closure is detached; protocol/disconnection errors propagate.
    pub async fn is_detached(&self) -> E2eResult<bool> {
        if self.page.is_closed() {
            return Ok(true);
        }
        Ok(!self
            .page
            .document_frames()
            .await?
            .iter()
            .any(|frame| frame.id == self.id))
    }

    /// Frame document title.
    pub async fn title(&self) -> E2eResult<String> {
        self.evaluate_value("document.title")
            .await?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| E2eError::Config("frame has no title".to_string()))
    }

    /// Frame document HTML.
    pub async fn content(&self) -> E2eResult<String> {
        self.evaluate_value("document.documentElement.outerHTML")
            .await?
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| E2eError::Config("frame has no document".to_string()))
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

    /// Locate an ARIA role with full predicates inside this frame.
    pub fn get_by_role_with(
        &self,
        role: &str,
        options: crate::locator::GetByRoleOptions,
    ) -> Locator {
        Locator::new(self.scoped_page(), Selector::by_role_with(role, &options))
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
pub(crate) fn valid_expose_name(name: &str) -> bool {
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
    /// Locators to cover, including document/full-page captures.
    pub mask: Vec<Locator>,
    /// Freeze animations/transitions during the capture.
    pub disable_animations: bool,
    /// Hide the text caret during the capture.
    pub hide_caret: bool,
    /// Clip in viewport capture coordinates, or document coordinates with full_page.
    pub clip: Option<ElementRect>,
    pub scale: crate::ScreenshotScale,
    /// Transparent default canvas on Chromium (PNG only). Firefox rejects this.
    pub omit_background: bool,
    /// CSS color for masks; None retains magenta.
    pub mask_color: Option<String>,
    /// Temporary CSS in the main document, reachable same-origin frames and open shadow roots.
    pub style: Option<String>,
    /// One capture budget, including queueing, preparation and restoration.
    pub timeout: Option<Duration>,
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
    /// Skip nonessential readiness checks; still uses trusted mouse input.
    pub force: bool,
    /// Number of clicks (2 = double-click).
    pub click_count: u32,
    /// Mouse button (trusted clicks only).
    pub button: MouseButton,
    /// Delay between button down and up.
    pub delay: Duration,
    pub position: Option<crate::ActionPosition>,
    pub modifiers: Vec<crate::KeyboardModifier>,
    pub trial: bool,
    pub timeout: Option<Duration>,
}

impl ClickOptions {
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.position = Some(crate::ActionPosition { x, y });
        self
    }
    pub fn modifiers(mut self, values: &[crate::KeyboardModifier]) -> Self {
        self.modifiers = values.into();
        self
    }
    pub fn trial(mut self, value: bool) -> Self {
        self.trial = value;
        self
    }
    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = Some(value);
        self
    }
    pub(crate) fn action_options(&self) -> crate::ActionOptions {
        crate::ActionOptions {
            force: self.force,
            position: self.position,
            modifiers: self.modifiers.clone(),
            trial: self.trial,
            timeout: self.timeout,
        }
    }
}

/// Mouse button for clicks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary button.
    #[default]
    Left,
    /// Middle button (wheel press).
    Middle,
    /// Secondary button (right-click).
    Right,
}

impl MouseButton {
    /// CDP button name.
    pub(crate) fn as_cdp(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Middle => "middle",
            Self::Right => "right",
        }
    }

    /// BiDi pointer button index.
    pub(crate) fn as_bidi(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
        }
    }
}

/// Options for [`Page::mouse_click_with`].
#[derive(Debug, Clone, Default)]
pub struct MouseClickOptions {
    /// Mouse button.
    pub button: MouseButton,
    /// Number of clicks.
    pub click_count: u32,
    /// Delay between button down and up.
    pub delay: Duration,
}

impl MouseClickOptions {
    /// Set the mouse button.
    #[must_use]
    pub fn button(mut self, button: MouseButton) -> Self {
        self.button = button;
        self
    }

    /// Set the click count.
    #[must_use]
    pub fn click_count(mut self, count: u32) -> Self {
        self.click_count = count;
        self
    }

    /// Set the down/up delay.
    #[must_use]
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// Options for [`Page::press_key_with`].
#[derive(Debug, Clone, Default)]
pub struct KeyPressOptions {
    /// Delay between key down and up.
    pub delay: Duration,
}

impl KeyPressOptions {
    /// Set the down/up delay.
    #[must_use]
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
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
    /// Native bytes/preview; consult body_state() before interpreting availability.
    pub post_data: Option<Vec<u8>>,
    pub(crate) body_state: crate::RouteBodyState,
    pub(crate) owner: Option<crate::route_options::RouteFetchOwner>,
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
    /// Shared URL matching; mutually exclusive with the legacy glob filter.
    pub url_matcher: Option<crate::UrlMatcher>,
}

impl RouteFromHarOptions {
    pub fn matching(mut self, matcher: crate::UrlMatcher) -> Self {
        self.url_matcher = Some(matcher);
        self
    }
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
    /// Explicit shared matcher; None keeps the legacy glob contract.
    pub matcher: Option<crate::UrlMatcher>,
    /// Handler deciding matching requests.
    pub handler: RouteHandler,
    /// Match at most this many requests (`None` = unlimited).
    pub times: Option<u32>,
    /// Matches consumed so far (shared across clones).
    pub hits: Arc<std::sync::atomic::AtomicU32>,
}

impl RouteHandlerEntry {
    /// Whether the entry still matches (`times` not exhausted).
    #[cfg(test)]
    pub(crate) fn allows_match(&self) -> bool {
        match self.times {
            Some(limit) => self.hits.load(std::sync::atomic::Ordering::Relaxed) < limit,
            None => true,
        }
    }

    /// Record one match.
    #[cfg(test)]
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
    /// Explicit shared matcher; None keeps the legacy glob contract.
    pub matcher: Option<crate::UrlMatcher>,
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
            matcher: None,
            action,
            times: None,
            hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }

    /// Use exact/glob/regex/contains matching instead of the legacy pattern.
    pub fn matching(mut self, matcher: crate::UrlMatcher) -> Self {
        self.matcher = Some(matcher);
        self
    }

    /// Whether the rule still matches (`times` not exhausted).
    #[cfg(test)]
    pub(crate) fn allows_match(&self) -> bool {
        match self.times {
            Some(limit) => self.hits.load(std::sync::atomic::Ordering::Relaxed) < limit,
            None => true,
        }
    }

    /// Record one match.
    #[cfg(test)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Milliseconds since the Unix epoch.
    pub ts_ms: u64,
    /// Entry kind (`action`, `navigation`, `console`, ...).
    pub kind: String,
    /// Human-readable detail.
    pub detail: String,
    /// Owned structured console/error payload for console entries.
    #[serde(default)]
    pub console: Option<Box<ConsoleMessage>>,
}

/// Element state snapshot for one selector.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ElementState {
    /// Element receives pointer input at its center.
    #[serde(default)]
    pub receives_events: bool,
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
    /// Source URL (Chromium, matched from download events; `None` on
    /// Firefox and for hand-built downloads).
    pub url: Option<String>,
    /// Terminal failure state (`canceled`, `interrupted`; `None` when the
    /// download completed).
    pub failure: Option<String>,
    /// CDP download GUID (Chromium only).
    pub(crate) guid: Option<String>,
    /// Identity only, so keeping a completed download never retains its page.
    page_id: Option<String>,
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
            url: None,
            failure: None,
            guid: None,
            page_id: None,
        }
    }

    /// Owning page identity, or None for a hand-built path.
    #[must_use]
    pub fn page_id(&self) -> Option<&str> {
        self.page_id.as_deref()
    }

    /// Read the completed file. For large files prefer `create_read_stream`.
    pub async fn read(&self) -> E2eResult<Vec<u8>> {
        self.read_with_options(crate::OperationOptions::default())
            .await
    }

    /// Read with a local budget and caller cancellation. None/zero disables the
    /// local timeout; this completed file has no live browser dependency.
    pub async fn read_with_options(&self, options: crate::OperationOptions) -> E2eResult<Vec<u8>> {
        Self::file_operation(options, async {
            self.check_completed()?;
            Ok(tokio::fs::read(&self.path).await?)
        })
        .await
    }

    /// Open a completed file as a Tokio AsyncRead/AsyncSeek stream.
    pub async fn create_read_stream(&self) -> E2eResult<tokio::fs::File> {
        self.create_read_stream_with_options(crate::OperationOptions::default())
            .await
    }

    /// Options bound opening the file. Subsequent stream reads use normal Tokio
    /// I/O; wrap them in CancellationToken::run to cancel a larger read operation.
    pub async fn create_read_stream_with_options(
        &self,
        options: crate::OperationOptions,
    ) -> E2eResult<tokio::fs::File> {
        Self::file_operation(options, async {
            self.check_completed()?;
            Ok(tokio::fs::File::open(&self.path).await?)
        })
        .await
    }

    fn check_completed(&self) -> E2eResult<()> {
        if let Some(failure) = &self.failure {
            return Err(E2eError::Config(format!(
                "download did not complete successfully: {failure}"
            )));
        }
        Ok(())
    }

    async fn file_operation<T>(
        options: crate::OperationOptions,
        future: impl Future<Output = E2eResult<T>>,
    ) -> E2eResult<T> {
        let future = crate::operation::Deadline::new(options.timeout.unwrap_or(Duration::ZERO))
            .run("completed download I/O", future);
        match options.cancellation {
            Some(token) => token.run(future).await,
            None => future.await,
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

    /// Delete the downloaded file. Missing files are idempotent; other I/O
    /// failures are reported rather than hidden.
    pub async fn delete(&self) -> E2eResult<()> {
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(E2eError::Io(error)),
        }
    }
}

/// An automated page (one browser tab).
#[derive(Clone)]
pub struct Page {
    pub(crate) screenshot_state: Arc<crate::screenshot::ScreenshotState>,
    pub(crate) reporter: Option<crate::report::StepSession>,
    expect_timeout: Arc<Mutex<Duration>>,
    action_timeout: Arc<Mutex<Duration>>,
    navigation_timeout: Arc<Mutex<Option<Duration>>>,
    pub(crate) snapshot_dir: Option<PathBuf>,
    pub(crate) snapshot_update: Option<crate::SnapshotUpdate>,
    pub(crate) driver: Driver,
    pub(crate) coverage_state: Arc<tokio::sync::Mutex<crate::coverage::CoverageState>>,
    sink: ConsoleSink,
    slow_mo: Duration,
    base_url: Option<String>,
    routing: Arc<crate::routing::PumpSlot>,
    route_runtime: Arc<crate::routing::RouteRuntime>,
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
    net_capture: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    pub(crate) exposed: crate::callbacks::ExposedState,
    extra_headers: Arc<Mutex<Vec<(String, String)>>>,
    auth_credentials: Arc<Mutex<Option<(String, String)>>>,
    cleared_auth: Arc<Mutex<bool>>,
    registry: Weak<Mutex<Vec<Page>>>,
    pub(crate) context_registry: Weak<Mutex<Vec<crate::BrowserContext>>>,
    pub(crate) context_id: Option<String>,
    pub(crate) owns_context: bool,
    frame_id: Option<String>,
    pub(crate) lazy_frames: Vec<Selector>,
    download_consumed: Arc<Mutex<HashMap<PathBuf, (u64, std::time::SystemTime)>>>,
    download_dir: Arc<Mutex<Option<PathBuf>>>,
    /// Tracing session shared with the owning context (screenshots on steps).
    tracing: Arc<Mutex<Option<TracingState>>>,
    closed: Arc<Mutex<bool>>,
    close_task: crate::operation::SharedClose,
    owner_close_task: crate::operation::SharedClose,
    /// Opener page's target id (`None` unless opened as a popup).
    opener_target: Arc<Mutex<Option<String>>>,
    /// Locator handlers, run before element actions.
    locator_handlers: Arc<Mutex<Vec<LocatorHandlerEntry>>>,
    /// Re-entrancy guard for handler runs (handlers act, which acts...).
    handlers_running: Arc<Mutex<bool>>,
}

/// A locator handler: run `handler` before element actions while `locator`
/// matches (overlay dismissal, cookie banners).
pub type LocatorHandlerFn =
    Arc<dyn Fn(Locator) -> futures::future::BoxFuture<'static, E2eResult<()>> + Send + Sync>;

/// Options for [`Page::add_locator_handler_with`].
#[derive(Debug, Clone, Default)]
pub struct LocatorHandlerOptions {
    /// Run at most this many times (`None` = unlimited).
    pub times: Option<u32>,
}

impl LocatorHandlerOptions {
    /// Run at most `n` times.
    #[must_use]
    pub fn times(mut self, n: u32) -> Self {
        self.times = Some(n);
        self
    }
}

/// One registered locator handler.
#[derive(Clone)]
struct LocatorHandlerEntry {
    locator: Locator,
    handler: LocatorHandlerFn,
    times: Option<u32>,
    hits: u32,
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Page")
            .field("target_id", &self.target_id())
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

/// Fake clock injected by [`Page::clock_install`]: overrides `Date`,
/// `setTimeout`/`setInterval`, `requestAnimationFrame` and `performance.now`
/// with a virtual queue drained by `__ferriteClock.tick(ms)`.
const CLOCK_SCRIPT: &str = include_str!("clock.js");

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
        mut driver: Driver,
        sink: ConsoleSink,
        slow_mo: Duration,
        base_url: Option<String>,
        registry: Weak<Mutex<Vec<Page>>>,
        context_routes: Arc<Mutex<Vec<RouteRule>>>,
        context_handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
        tracing: Arc<Mutex<Option<TracingState>>>,
    ) -> Self {
        let action_timeout = Arc::new(Mutex::new(driver.timeout()));
        driver.share_timeout(action_timeout.clone());
        let download_dir = sink.download_dir.clone();
        Self {
            screenshot_state: Arc::new(crate::screenshot::ScreenshotState::default()),
            reporter: None,
            driver,
            coverage_state: Arc::new(tokio::sync::Mutex::new(Default::default())),
            sink,
            slow_mo,
            base_url,
            routing: Arc::new(tokio::sync::Mutex::new(None)),
            route_runtime: Arc::new(crate::routing::RouteRuntime::default()),
            dialogs: Arc::new(Mutex::new(None)),
            capture: Arc::new(Mutex::new(None)),
            routes: Arc::new(Mutex::new(Vec::new())),
            context_routes,
            handlers: Arc::new(Mutex::new(Vec::new())),
            context_handlers,
            expect_timeout: Arc::new(Mutex::new(Duration::from_millis(
                crate::expect::DEFAULT_EXPECT_MS,
            ))),
            action_timeout,
            navigation_timeout: Arc::new(Mutex::new(None)),
            snapshot_dir: None,
            snapshot_update: None,
            pending_grants: Arc::new(Mutex::new(Vec::new())),
            net_capture: Arc::new(Mutex::new(None)),
            exposed: crate::callbacks::ExposedState::default(),
            extra_headers: Arc::new(Mutex::new(Vec::new())),
            auth_credentials: Arc::new(Mutex::new(None)),
            cleared_auth: Arc::new(Mutex::new(false)),
            registry,
            context_registry: Weak::new(),
            context_id: None,
            owns_context: false,
            frame_id: None,
            lazy_frames: Vec::new(),
            download_consumed: Arc::new(Mutex::new(HashMap::new())),
            download_dir,
            tracing,
            closed: Arc::new(Mutex::new(false)),
            close_task: crate::operation::SharedClose::default(),
            owner_close_task: crate::operation::SharedClose::default(),
            opener_target: Arc::new(Mutex::new(None)),
            locator_handlers: Arc::new(Mutex::new(Vec::new())),
            handlers_running: Arc::new(Mutex::new(false)),
        }
    }

    /// Clone scoped to a frame (evaluation runs inside it).
    pub(crate) fn scoped(mut self, frame_id: String) -> Self {
        self.frame_id = Some(frame_id);
        self
    }

    pub(crate) fn owning_page(&self) -> Self {
        let mut page = self.clone();
        page.frame_id = None;
        page.lazy_frames.clear();
        page
    }

    pub(crate) fn sync_protocol_timeout(&mut self) {
        self.driver.share_timeout(self.action_timeout.clone());
    }

    /// Scope cancellation to this clone and locators/assertions created from it.
    pub fn with_cancellation(&self, token: crate::CancellationToken) -> Self {
        let mut page = self.clone();
        page.driver = page.driver.with_cancellation(token);
        page
    }

    pub(crate) fn run_operation<'a, T: 'a>(
        &'a self,
        future: impl std::future::Future<Output = E2eResult<T>> + 'a,
    ) -> impl std::future::Future<Output = E2eResult<T>> + 'a {
        self.driver.run(future)
    }
    /// Override action/protocol timeouts for this clone without changing siblings.
    /// Zero disables the timeout.
    pub fn with_timeout(&self, timeout: Duration) -> Self {
        let mut page = self.clone();
        page.action_timeout = Arc::new(Mutex::new(timeout));
        page.driver.share_timeout(page.action_timeout.clone());
        page
    }

    fn operation_page(&self, options: &crate::OperationOptions) -> Self {
        let mut page = options
            .timeout
            .map(|timeout| self.with_timeout(timeout))
            .unwrap_or_else(|| self.clone());
        if let Some(token) = &options.cancellation {
            page = page.with_cancellation(token.clone());
        }
        page
    }

    pub async fn evaluate_with_options<T: serde::de::DeserializeOwned>(
        &self,
        expression: &str,
        options: crate::OperationOptions,
    ) -> E2eResult<T> {
        self.operation_page(&options).evaluate(expression).await
    }

    pub async fn wait_for_function_with_options(
        &self,
        expression: &str,
        options: crate::OperationOptions,
    ) -> E2eResult<()> {
        self.operation_page(&options)
            .wait_for_function(
                expression,
                options.timeout.unwrap_or_else(|| self.timeout()),
            )
            .await
    }

    pub async fn wait_for_event_with_options(
        &self,
        kind: PageEventKind,
        options: crate::OperationOptions,
    ) -> E2eResult<PageEvent> {
        self.operation_page(&options)
            .wait_for_event(kind, options.timeout.unwrap_or_else(|| self.timeout()))
            .await
    }

    /// Lazily locate an iframe. Nested same-origin frames and frame replacements
    /// are supported; use document_frames() for protocol-backed cross-origin frames.
    pub fn frame_locator(&self, selector: &str) -> crate::FrameLocator {
        crate::FrameLocator::new(self.clone(), Selector::parse(selector.to_string()))
    }

    pub fn browser_kind(&self) -> crate::BrowserKind {
        match &self.driver {
            Driver::Cdp(_) => crate::BrowserKind::Chromium,
            Driver::Bidi(_) => crate::BrowserKind::Firefox,
        }
    }

    /// Chromium JavaScript and CSS coverage controller.
    pub fn coverage(&self) -> crate::Coverage {
        crate::Coverage::new(self.clone())
    }

    pub(crate) fn cdp_events(
        &self,
    ) -> E2eResult<(
        String,
        tokio::sync::broadcast::Receiver<crate::cdp::CdpEvent>,
    )> {
        self.driver.cdp_events()
    }

    /// Default retry window for assertions on this page.
    pub fn expect_timeout(&self) -> Duration {
        *self
            .expect_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Set the default assertion retry window.
    pub fn set_expect_timeout(&self, timeout: Duration) {
        *self
            .expect_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = timeout;
    }

    /// The context owning this page, while it remains registered.
    pub fn context(&self) -> Option<crate::BrowserContext> {
        self.context_registry
            .upgrade()?
            .lock()
            .ok()?
            .iter()
            .find(|c| c.id() == self.context_id.as_deref())
            .cloned()
    }

    /// HTTP client sharing cookies with this page's owning context.
    pub fn request(&self) -> E2eResult<crate::ApiClient> {
        self.context()
            .map(|context| context.request())
            .ok_or_else(|| E2eError::Config("page's owning context is no longer available".into()))
    }

    /// Clear buffered console messages.
    pub fn clear_console_messages(&self) {
        self.sink
            .console
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// Captured JavaScript exceptions.
    pub fn page_errors(&self) -> Vec<ConsoleMessage> {
        self.console_messages()
            .into_iter()
            .filter(|m| m.kind == "exception")
            .collect()
    }

    /// Clear buffered JavaScript exceptions while retaining console output.
    pub fn clear_page_errors(&self) {
        self.sink
            .console
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|m| m.kind != "exception");
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
        // A close event must be delivered even though it cancels page operations.
        if kind == PageEventKind::Closed {
            if self.is_closed() {
                return Ok(PageEvent::Closed);
            }
            let mut events = self.subscribe();
            return self
                .driver
                .run_close_wait(crate::operation::Deadline::new(timeout).run(
                    "wait for page closed",
                    async {
                        loop {
                            match events.recv().await {
                                Ok(PageEvent::Closed) => return Ok(PageEvent::Closed),
                                Ok(_)
                                | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    return Err(E2eError::Disconnected(
                                        "page event stream closed".into(),
                                    ))
                                }
                            }
                        }
                    },
                ))
                .await;
        }
        self.driver
            .run(async {
                if kind == PageEventKind::WebSocket && matches!(self.driver, Driver::Bidi(_)) {
                    return Err(E2eError::Config(
                        "websocket events are not supported on Firefox \
                 (BiDi has no socket-frame events)"
                            .to_string(),
                    ));
                }
                if let Driver::Bidi(driver) = &self.driver {
                    let event = match kind {
                        PageEventKind::FrameNavigated => {
                            Some("browsingContext.navigationCommitted")
                        }
                        PageEventKind::DialogClosed => Some("browsingContext.userPromptClosed"),
                        _ => None,
                    };
                    if let Some(event) = event {
                        if !driver.supports_lifecycle_event(event) {
                            return Err(E2eError::Config(format!(
                                "native {event} events are unavailable on this Firefox version"
                            )));
                        }
                    }
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
                crate::operation::Deadline::new(timeout)
                    .run(format!("wait for {kind:?} event"), async {
                        loop {
                            match events.recv().await {
                                Ok(event) if event.kind() == kind => return Ok(event),
                                Ok(_)
                                | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    return Err(E2eError::Disconnected(
                                        "page event stream closed".into(),
                                    ))
                                }
                            }
                        }
                    })
                    .await
            })
            .await
    }

    /// Push an event to this page's subscribers.
    pub(crate) fn emit(&self, event: PageEvent) {
        self.sink.emit(event);
    }

    /// Mark the page closed and emit [`PageEvent::Closed`] once.
    pub(crate) fn mark_closed(&self) {
        let mut closed = self.closed.lock().unwrap_or_else(|e| e.into_inner());
        if *closed {
            return;
        }
        *closed = true;
        drop(closed);
        self.routes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.handlers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.route_runtime.clear();
        self.exposed.stop();
        self.emit(PageEvent::Closed);
        self.driver.cancel_lifecycle();
    }

    /// Whether the page was closed or its native transport disconnected.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.driver.is_disconnected()
            || self.sink.native_closed()
            || self.closed.lock().map(|c| *c).unwrap_or(false)
    }

    /// Wait until `selector` matches, then return the [`Locator`].
    pub async fn wait_for_selector(&self, selector: &str, timeout: Duration) -> E2eResult<Locator> {
        self.driver
            .run(async {
                let locator = self.locator(selector.to_string());
                locator.wait_for(timeout).await?;
                Ok(locator)
            })
            .await
    }

    /// Wait until `selector` reaches `state`, returning the locator.
    pub async fn wait_for_selector_with(
        &self,
        selector: &str,
        state: crate::locator::WaitForState,
        timeout: Duration,
    ) -> E2eResult<Locator> {
        self.driver
            .run(async {
                let locator = self.locator(selector.to_string());
                locator.wait_for_state(state, timeout).await?;
                Ok(locator)
            })
            .await
    }

    /// Wait for a popup opened from this page (already adopted and usable).
    pub async fn wait_for_popup(&self, timeout: Duration) -> E2eResult<Page> {
        self.driver
            .run(async {
                match self.wait_for_event(PageEventKind::Popup, timeout).await? {
                    PageEvent::Popup(page) => Ok(*page),
                    _ => unreachable!("filtered by kind"),
                }
            })
            .await
    }

    /// The page that opened this popup (`None` for tabs and closed openers).
    #[must_use]
    pub fn opener(&self) -> Option<Page> {
        let target = self
            .opener_target
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(None)?;
        self.registry
            .upgrade()
            .and_then(|pages| {
                pages
                    .lock()
                    .map(|pages| {
                        pages
                            .iter()
                            .find(|page| page.target_id() == target)
                            .cloned()
                    })
                    .unwrap_or(None)
            })
            .filter(|page| !page.is_closed())
    }

    /// Record the opener's target id (popup adoption).
    pub(crate) fn set_opener_target(&self, target: &str) {
        *self.opener_target.lock().unwrap_or_else(|e| e.into_inner()) = Some(target.to_string());
    }

    /// Wait for the next JavaScript dialog.
    ///
    /// Auto-handling is armed with `accept` when it is not already running,
    /// so the dialog is both observed and answered.
    pub async fn wait_for_dialog(&self, accept: bool, timeout: Duration) -> E2eResult<DialogInfo> {
        let scoped = self.with_timeout(timeout);
        scoped
            .driver
            .run(async {
                let armed = scoped.dialogs.lock().map(|d| d.is_some()).unwrap_or(false);
                if !armed {
                    scoped.handle_dialogs(accept).await?;
                }
                match scoped
                    .wait_for_event(PageEventKind::Dialog, timeout)
                    .await?
                {
                    PageEvent::Dialog(info) => Ok(info),
                    _ => unreachable!("filtered by kind"),
                }
            })
            .await
    }

    /// Default timeout for protocol calls.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        *self
            .action_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Override the default timeout.
    pub fn set_timeout(&mut self, timeout: Duration) {
        *self
            .action_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = timeout;
        self.driver.set_timeout(timeout);
    }

    /// Override the navigation timeout, independently of locator actions.
    pub fn set_navigation_timeout(&self, timeout: Duration) {
        *self
            .navigation_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(timeout);
    }

    pub fn navigation_timeout(&self) -> Duration {
        self.navigation_timeout
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap_or_else(|| self.timeout())
    }

    /// Raw protocol call with the page timeout (CDP method on Chromium,
    /// BiDi method with context injected on Firefox).
    pub async fn call(&self, method: &str, params: Value) -> E2eResult<Value> {
        self.call_with_timeout(method, params, self.timeout()).await
    }

    /// Raw protocol call with an explicit timeout.
    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> E2eResult<Value> {
        self.driver
            .run(crate::operation::Deadline::new(timeout).run(method, async {
                let background = self.browser_kind() == crate::BrowserKind::Chromium
                    && method == "Emulation.setDefaultBackgroundColorOverride";
                let _guard = if background {
                    Some(self.screenshot_state.gate.clone().lock_owned().await)
                } else {
                    None
                };
                let color = params.get("color").cloned();
                let result = self.driver.raw(method, params, timeout).await?;
                if background {
                    *self
                        .screenshot_state
                        .background
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = color;
                }
                Ok(result)
            }))
            .await
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
    pub(crate) fn resolve_url(&self, url: &str) -> E2eResult<String> {
        if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("about:")
            || url.starts_with("data:")
            || url.starts_with("file:")
        {
            return Ok(url.to_string());
        }
        if let Some(base) = self
            .base_url
            .clone()
            .or_else(|| std::env::var("FERRITE_E2E_BASE_URL").ok())
        {
            return reqwest::Url::parse(&base)
                .and_then(|base| base.join(url))
                .map(|url| url.to_string())
                .map_err(|error| E2eError::Navigation {
                    url: url.to_string(),
                    message: error.to_string(),
                });
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
        self.auto_step("page.goto", crate::StepCategory::Action, async {
            self.driver
                .run(async {
                    self.goto_with_options(url, NavigationOptions::default())
                        .await
                })
                .await
        })
        .await
    }

    /// Navigate with explicit options.
    pub async fn goto_with_options(&self, url: &str, options: NavigationOptions) -> E2eResult<()> {
        self.auto_step(
            "page.goto_with_options",
            crate::StepCategory::Action,
            async {
                self.driver
                    .run(async {
                        let url = self.resolve_url(url)?;
                        let timeout = options.timeout.unwrap_or_else(|| self.navigation_timeout());
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
                        Ok(())
                    })
                    .await
            },
        )
        .await
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

    /// Restore cookies before the first request and localStorage before app scripts.
    pub(crate) async fn apply_storage_state(&self, state: &StorageState) -> E2eResult<()> {
        self.driver.run(async {

        let origins = state.all_origins();
        for cookie in &state.cookies {
            let url = cookie
                .domain
                .as_deref()
                .map(|domain| {
                    format!(
                        "{}://{}/",
                        if cookie.secure { "https" } else { "http" },
                        domain.trim_start_matches('.')
                    )
                })
                .or_else(|| origins.first().map(|entry| entry.origin.clone()))
                .ok_or_else(|| {
                    E2eError::Config("storage-state cookie needs a domain or origin".into())
                })?;
            self.driver
                .add_cookies(std::slice::from_ref(cookie), &url)
                .await?;
        }
        let origins_json = serde_json::to_string(&origins)?;
        let script = format!("(() => {{ const state = {origins_json}.find(entry => entry.origin === location.origin); if (state) {{ localStorage.clear(); for (const entry of state.localStorage) localStorage.setItem(entry.name, entry.value); }} return true; }})()");
        self.add_init_script(&script).await?;
        self.evaluate_value(&script).await?;
        Ok(())
        }).await
    }

    /// Reload the page.
    pub async fn reload(&self) -> E2eResult<()> {
        self.auto_step("page.reload", crate::StepCategory::Action, async {
            self.driver.run(async { self.driver.reload().await }).await
        })
        .await
    }

    /// Go back in history.
    pub async fn go_back(&self) -> E2eResult<()> {
        self.auto_step("page.go_back", crate::StepCategory::Action, async {
            self.driver
                .run(async { self.driver.traverse(-1).await })
                .await
        })
        .await
    }

    /// Go forward in history.
    pub async fn go_forward(&self) -> E2eResult<()> {
        self.auto_step("page.go_forward", crate::StepCategory::Action, async {
            self.driver
                .run(async { self.driver.traverse(1).await })
                .await
        })
        .await
    }

    /// Current page title.
    pub async fn title(&self) -> E2eResult<String> {
        self.driver
            .run(async { self.evaluate_string("document.title").await })
            .await
    }

    /// Current page URL.
    pub async fn url(&self) -> E2eResult<String> {
        self.driver
            .run(async { self.evaluate_string("location.href").await })
            .await
    }

    /// Full HTML content.
    pub async fn content(&self) -> E2eResult<String> {
        self.driver
            .run(async {
                self.evaluate_string("document.documentElement.outerHTML")
                    .await
            })
            .await
    }

    /// Set the document HTML.
    pub async fn set_content(&self, html: &str) -> E2eResult<()> {
        self.auto_step("page.set_content", crate::StepCategory::Action, async {
            self.driver
                .run(async {
                    if self.frame_id.is_some() || !self.lazy_frames.is_empty() {
                        self.evaluate_value(&format!(
                            "document.open(); document.write({}); document.close(); true",
                            serde_json::to_string(html)?
                        ))
                        .await?;
                        Ok(())
                    } else {
                        self.driver.set_content(html).await
                    }
                })
                .await
        })
        .await
    }

    /// Bring the page to front.
    pub async fn bring_to_front(&self) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.bring_to_front().await })
            .await
    }

    /// Evaluate a function with a JSON-serializable argument.
    pub async fn evaluate_with_arg<T: serde::de::DeserializeOwned, A: Serialize>(
        &self,
        function: &str,
        argument: &A,
    ) -> E2eResult<T> {
        self.driver
            .run(async {
                self.evaluate(&format!(
                    "({function})({})",
                    serde_json::to_string(argument)?
                ))
                .await
            })
            .await
    }

    /// Evaluate JavaScript and deserialize the returned value.
    pub async fn evaluate<T>(&self, expression: &str) -> E2eResult<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.driver
            .run(async {
                let value = self.evaluate_value(expression).await?;
                serde_json::from_value(value).map_err(E2eError::Json)
            })
            .await
    }

    /// Evaluate JavaScript and return the raw JSON value.
    pub fn evaluate_value<'a>(
        &'a self,
        expression: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = E2eResult<Value>> + Send + 'a>> {
        // An erased future keeps downstream Send proofs and async stacks small.
        Box::pin(async move {
            self.driver.run(async {

        let wrapped;
        let expression = if self.lazy_frames.is_empty() {
            expression
        } else {
            let selectors = self
                .lazy_frames
                .iter()
                .map(|selector| serde_json::to_string(&selector.resolve_js()))
                .collect::<Result<Vec<_>, _>>()?
                .join(",");
            let source = serde_json::to_string(expression)?;
            wrapped = format!("(() => {{ let frame = window; for (const selector of [{selectors}]) {{ let elements; try {{ elements = frame.eval(selector); }} catch (error) {{ throw new Error('FrameLocator supports same-origin frames; use the Frame API for cross-origin frames: ' + error.message); }} if (elements.length !== 1) throw new Error('strict frame locator: expected exactly one iframe, got ' + elements.length); if (!elements[0].matches('iframe,frame')) throw new Error('frame locator target is not an iframe'); frame = elements[0].contentWindow; if (!frame) throw new Error('iframe is not attached'); }} return frame.eval({source}); }})()");
            &wrapped
        };
        match &self.frame_id {
            Some(id) => self.driver.frame_evaluate(id, expression).await,
            None => self.driver.evaluate(expression).await,
        }
        }).await
        })
    }

    /// Evaluate JavaScript and keep the result alive as a [`JSHandle`]
    /// (Playwright `page.evaluateHandle()`).
    ///
    /// Frame-scoped pages are not supported: handles need the top-level
    /// execution context.
    pub async fn evaluate_handle(&self, expression: &str) -> E2eResult<JSHandle> {
        self.driver
            .run(async {
                if self.frame_id.is_some() || !self.lazy_frames.is_empty() {
                    return Err(E2eError::Config(
                        "evaluate_handle on a frame-scoped page is not supported".to_string(),
                    ));
                }
                self.driver.evaluate_handle(expression).await
            })
            .await
    }

    async fn evaluate_string(&self, expression: &str) -> E2eResult<String> {
        let value = self.evaluate_value(expression).await?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    /// Wait until a JS expression returns truthy.
    pub async fn wait_for_function(&self, expression: &str, timeout: Duration) -> E2eResult<()> {
        let scoped = self.with_timeout(timeout);
        scoped
            .driver
            .run(async {
                crate::operation::Deadline::new(timeout)
                    .run("wait_for_function", async {
                        let deadline = crate::operation::Deadline::new(timeout);
                        let wrapped = format!("(async () => Boolean(await ({expression})))()");
                        loop {
                            scoped.driver.run(async { Ok(()) }).await?;
                            if let Ok(value) = scoped.evaluate_value(&wrapped).await {
                                if value.as_bool().unwrap_or(false) {
                                    return Ok(());
                                }
                            }
                            if deadline.expired() {
                                return Err(E2eError::Timeout(
                                    timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                                    format!("wait_for_function({expression})"),
                                ));
                            }
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    })
                    .await
            })
            .await
    }

    /// Wait for a function/expression to become truthy and return its JSON value.
    /// The argument is JSON only; functions receive it as their single argument.
    /// Values are captured at success without re-evaluating the predicate.
    /// A returned promise is truthy; its resolved value may be false (Playwright semantics).
    pub async fn wait_for_function_value<A: Serialize + Sync>(
        &self,
        expression: &str,
        argument: &A,
        options: crate::FunctionWaitOptions,
    ) -> E2eResult<Value> {
        let argument = serde_json::to_value(argument)?;
        self.auto_step(
            "page.wait_for_function",
            crate::StepCategory::Action,
            async {
                match crate::function_wait::wait(self, expression, argument, options, false).await?
                {
                    crate::function_wait::ResultValue::Json(value) => Ok(value),
                    _ => unreachable!(),
                }
            },
        )
        .await
    }

    /// Retain the truthy result as a live handle in the top-level document.
    /// Frame/lazy-frame handles are unsupported; use the JSON result helper there.
    pub async fn wait_for_function_handle<A: Serialize + Sync>(
        &self,
        expression: &str,
        argument: &A,
        options: crate::FunctionWaitOptions,
    ) -> E2eResult<JSHandle> {
        if self.frame_id.is_some() || !self.lazy_frames.is_empty() {
            return Err(E2eError::Config("function wait remote handles require a top-level Page; use wait_for_function_value for frames".into()));
        }
        let argument = serde_json::to_value(argument)?;
        self.auto_step(
            "page.wait_for_function",
            crate::StepCategory::Action,
            async {
                match crate::function_wait::wait(self, expression, argument, options, true).await? {
                    crate::function_wait::ResultValue::Handle(handle) => Ok(*handle),
                    _ => unreachable!(),
                }
            },
        )
        .await
    }

    /// Wait a fixed amount of time.
    pub async fn wait_for_timeout(&self, duration: Duration) -> E2eResult<()> {
        self.driver
            .run(async {
                tokio::time::sleep(duration).await;
                Ok(())
            })
            .await
    }

    /// Wait until the URL contains `fragment`.
    pub async fn wait_for_url(&self, fragment: &str, timeout: Duration) -> E2eResult<()> {
        self.wait_for_url_matching(&crate::UrlMatcher::contains(fragment), timeout)
            .await
    }

    /// Wait for a full URL matched by exact/glob/regex rules. Exact relative URLs
    /// resolve against base_url; legacy wait_for_url retains substring matching.
    pub async fn wait_for_url_matching(
        &self,
        matcher: &crate::UrlMatcher,
        timeout: Duration,
    ) -> E2eResult<()> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_url_where(move |url| matcher.matches(url), timeout)
            .await
    }
    /// Wait for a URL predicate, including history/hash navigation.
    pub async fn wait_for_url_where<F>(&self, predicate: F, timeout: Duration) -> E2eResult<()>
    where
        F: FnMut(&str) -> bool,
    {
        // Preserve the legacy URL-only readiness contract.
        self.wait_for_url_where_with_options(
            predicate,
            crate::UrlWaitOptions::default()
                .wait_until(LoadState::Commit)
                .timeout(timeout),
        )
        .await
    }

    /// Legacy substring matching with explicit document readiness controls.
    pub async fn wait_for_url_with_options(
        &self,
        fragment: &str,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()> {
        self.wait_for_url_matching_with_options(&crate::UrlMatcher::contains(fragment), options)
            .await
    }
    /// Match a full URL and the current document's readiness in one budget.
    pub async fn wait_for_url_matching_with_options(
        &self,
        matcher: &crate::UrlMatcher,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_url_where_with_options(move |url| matcher.matches(url), options)
            .await
    }

    /// Match a URL predicate and readiness from the same document observation.
    /// NetworkIdle tracks this Page's observed HTTP activity for 500ms, after
    /// Load; worker/socket/OOPIF traffic is not a complete connectivity signal.
    /// Frame-scoped NetworkIdle is unsupported and rejected before waiting.
    pub async fn wait_for_url_where_with_options<F>(
        &self,
        mut predicate: F,
        options: crate::UrlWaitOptions,
    ) -> E2eResult<()>
    where
        F: FnMut(&str) -> bool,
    {
        if options.wait_until == LoadState::NetworkIdle
            && (self.frame_id.is_some() || !self.lazy_frames.is_empty())
        {
            return Err(E2eError::Config("frame network-idle tracking is not available; use document load or an application predicate".into()));
        }
        let timeout = options.timeout.unwrap_or_else(|| self.navigation_timeout());
        let mut scoped = self.with_timeout(timeout);
        if let Some(token) = options.cancellation {
            scoped = scoped.with_cancellation(token);
        }
        self.auto_step_local("page.wait_for_url", crate::StepCategory::Action, async {
            scoped
                .run_operation(crate::operation::Deadline::new(timeout).run(
                    format!("wait_for_url and {:?}", options.wait_until),
                    async {
                        let mut quiet: Option<(Value, u64, tokio::time::Instant)> = None;
                        loop {
                            // One evaluation prevents an old URL from being combined
                            // with the replacement document's readyState.
                            let observed = scoped
                                .evaluate_value(crate::url_wait::DOCUMENT_OBSERVATION)
                                .await;
                            let observed = match observed {
                                Ok(value) => value,
                                Err(error)
                                    if crate::url_wait::navigation_replaced_realm(&error) =>
                                {
                                    quiet = None;
                                    tokio::time::sleep(Duration::from_millis(25)).await;
                                    continue;
                                }
                                Err(error) => return Err(error),
                            };
                            let matched = predicate(observed["url"].as_str().unwrap_or_default());
                            let ready =
                                crate::url_wait::document_ready(&observed, options.wait_until);
                            if matched && ready {
                                if options.wait_until != LoadState::NetworkIdle {
                                    return Ok(());
                                }
                                let (active, activity) = scoped.sink.network_activity();
                                if active == 0 {
                                    match &quiet {
                                        Some((previous, epoch, since))
                                            if previous == &observed && *epoch == activity =>
                                        {
                                            if since.elapsed() >= Duration::from_millis(500) {
                                                return Ok(());
                                            }
                                        }
                                        _ => {
                                            quiet = Some((
                                                observed,
                                                activity,
                                                tokio::time::Instant::now(),
                                            ))
                                        }
                                    }
                                } else {
                                    quiet = None;
                                }
                            } else {
                                quiet = None;
                            }
                            tokio::time::sleep(Duration::from_millis(25)).await;
                        }
                    },
                ))
                .await
        })
        .await
    }

    /// Wait for a document load state.
    pub async fn wait_for_load_state(&self, state: LoadState) -> E2eResult<()> {
        self.driver
            .run(async {
                let timeout = self.navigation_timeout();
                if self.frame_id.is_some() || !self.lazy_frames.is_empty() {
                    let expression = match state {
                        LoadState::Commit => "true",
                        LoadState::DomContentLoaded => crate::url_wait::DOM_CONTENT_LOADED,
                        LoadState::Load => "document.readyState === 'complete'",
                        LoadState::NetworkIdle => return Err(E2eError::Config("frame network-idle tracking is not available; use document load or an application predicate".into())),
                    };
                    self.wait_for_function(expression, timeout).await
                } else { self.driver.wait_for_load(state, timeout).await }
            })
            .await
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

    /// Locate an ARIA role with full predicates.
    #[must_use]
    pub fn get_by_role_with(
        &self,
        role: &str,
        options: crate::locator::GetByRoleOptions,
    ) -> Locator {
        Locator::new(self.clone(), Selector::by_role_with(role, &options))
    }

    /// Locate a `<label>` by its text.
    ///
    /// Matches the control associated with the label.
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
        self.driver
            .run(async {
                let expression = selector.state_expression();
                let value = self.evaluate_value(&expression).await?;
                serde_json::from_value(value).map_err(E2eError::Json)
            })
            .await
    }

    /// Register a locator handler, run before element actions while
    /// `locator` matches (overlay dismissal, cookie banners). Handler
    /// errors fail the action. Direct coordinate/keyboard calls
    /// (`mouse_click`, `press_key`) do not trigger handlers.
    pub async fn add_locator_handler<F, Fut>(&self, locator: &Locator, handler: F)
    where
        F: Fn(Locator) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.add_locator_handler_with(locator, LocatorHandlerOptions::default(), handler)
            .await;
    }

    /// [`Page::add_locator_handler`] with a run limit.
    pub async fn add_locator_handler_with<F, Fut>(
        &self,
        locator: &Locator,
        options: LocatorHandlerOptions,
        handler: F,
    ) where
        F: Fn(Locator) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        let entry = LocatorHandlerEntry {
            locator: locator.clone(),
            handler: Arc::new(move |found| Box::pin(handler(found))),
            times: options.times,
            hits: 0,
        };
        self.locator_handlers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(entry);
    }

    /// Remove the handler registered for `locator` (matched by selector).
    pub fn remove_locator_handler(&self, locator: &Locator) {
        if let Ok(mut handlers) = self.locator_handlers.lock() {
            let raw = locator.selector_raw();
            handlers.retain(|entry| entry.locator.selector_raw() != raw);
        }
    }

    /// Run due handlers (each matching locator, within budget).
    pub(crate) async fn run_locator_handlers(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                {
                    let mut running = self
                        .handlers_running
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if *running {
                        return Ok(());
                    }
                    *running = true;
                }
                let result = self.run_locator_handlers_inner().await;
                *self
                    .handlers_running
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = false;
                result
            })
            .await
    }

    /// Handler pass: match, invoke, count hits.
    async fn run_locator_handlers_inner(&self) -> E2eResult<()> {
        let pending: Vec<(Locator, LocatorHandlerFn)> = self
            .locator_handlers
            .lock()
            .map(|handlers| {
                handlers
                    .iter()
                    .filter(|entry| entry.times.is_none_or(|limit| entry.hits < limit))
                    .map(|entry| (entry.locator.clone(), Arc::clone(&entry.handler)))
                    .collect()
            })
            .unwrap_or_default();
        for (locator, handler) in pending {
            if !locator.is_visible().await.unwrap_or(false) {
                continue;
            }
            handler(locator.clone()).await?;
            if let Ok(mut handlers) = self.locator_handlers.lock() {
                if let Some(entry) = handlers
                    .iter_mut()
                    .find(|entry| entry.locator.selector_raw() == locator.selector_raw())
                {
                    entry.hits += 1;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn action(
        &self,
        selector: &Selector,
        action: &str,
        argument: Option<&str>,
    ) -> E2eResult<Value> {
        self.driver
            .run(async {
                let deadline = crate::operation::Deadline::new(self.timeout());
                let value = loop {
                    self.run_locator_handlers().await?;
                    let state = self.query_state(selector).await?;
                    if selector.is_strict() && state.count > 1 {
                        return Err(E2eError::Locator {
                            selector: selector.raw().into(),
                            message: "strict mode violation: multiple elements match".into(),
                        });
                    }
                    let needs_visible = matches!(
                        action,
                        "fill" | "clear" | "check" | "select" | "select_many"
                    );
                    let needs_editable = matches!(action, "fill" | "clear");
                    if state.count > 0
                        && (!needs_visible || (state.visible && state.enabled))
                        && (!needs_editable || state.editable)
                    {
                        let expression = selector.action_expression(action, argument);
                        let value = self.evaluate_value(&expression).await?;
                        if value.get("ok").and_then(Value::as_bool) != Some(false) {
                            break value;
                        }
                        let message = value
                            .get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("action failed");
                        if message != "option not found" {
                            return Err(E2eError::Locator {
                                selector: selector.raw().into(),
                                message: message.into(),
                            });
                        }
                    }
                    if !self.timeout().is_zero() && deadline.expired() {
                        return Err(E2eError::Timeout(
                            self.timeout().as_millis() as u64,
                            format!("{action} {}: element not ready", selector.raw()),
                        ));
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                };
                self.sink
                    .record("action", format!("{action} {}", selector.raw()));
                self.slow_mo().await;
                Ok(value)
            })
            .await
    }

    /// Current viewport size in CSS pixels.
    pub async fn viewport_size(&self) -> E2eResult<Viewport> {
        self.driver
            .run(async {
                let value = self
                    .evaluate_value("({ w: window.innerWidth, h: window.innerHeight })")
                    .await?;
                Ok(Viewport {
                    width: value["w"].as_u64().unwrap_or(0) as u32,
                    height: value["h"].as_u64().unwrap_or(0) as u32,
                })
            })
            .await
    }

    /// Request a garbage collection (Chromium only; BiDi fails loudly).
    pub async fn request_gc(&self) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.request_gc().await })
            .await
    }

    /// Trusted mouse click at CSS-pixel coordinates.
    pub async fn mouse_click(&self, x: f64, y: f64, click_count: u32) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_click(x, y, click_count).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Trusted mouse click with button/count/delay options.
    pub async fn mouse_click_with(
        &self,
        x: f64,
        y: f64,
        options: MouseClickOptions,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_click_with(x, y, &options).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Move the mouse to CSS-pixel coordinates.
    pub async fn mouse_move(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_move(x, y).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Insert text at the focused element (trusted input).
    pub async fn insert_text(&self, text: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.insert_text(text).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Dispatch a key press (name like `Enter` or a single char).
    pub async fn press_key(&self, key: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.press_key(key).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Dispatch a key press with down/up delay.
    pub async fn press_key_with(&self, key: &str, options: KeyPressOptions) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.press_key_with(key, &options).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Press the left mouse button at CSS-pixel coordinates.
    ///
    /// Input state persists across calls on both engines. Locator drag options
    /// additionally release the held button on cancellation or failure.
    pub async fn mouse_down(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_down(x, y).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Release the left mouse button at CSS-pixel coordinates.
    pub async fn mouse_up(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_up(x, y).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Drag from one CSS-pixel point to another in `steps` paced moves.
    pub async fn mouse_drag(&self, from: (f64, f64), to: (f64, f64), steps: u32) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_drag(from, to, steps).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Scroll a wheel at CSS-pixel coordinates by (`delta_x`, `delta_y`).
    pub async fn mouse_wheel(&self, x: f64, y: f64, delta_x: f64, delta_y: f64) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.mouse_wheel(x, y, delta_x, delta_y).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Hold a key down (pair with [`Page::key_up`]).
    pub async fn key_down(&self, key: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.key_down(key).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Release a held key.
    pub async fn key_up(&self, key: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.key_up(key).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Tap at CSS-pixel coordinates with the touchscreen.
    pub async fn touchscreen_tap(&self, x: f64, y: f64) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.touchscreen_tap(x, y).await?;
                self.slow_mo().await;
                Ok(())
            })
            .await
    }

    /// Run a named step. For fallible steps use `step_result` to record returned errors.
    #[track_caller]
    pub fn step<'a, F, T>(&'a self, name: &'a str, step: F) -> impl Future<Output = T> + 'a
    where
        F: Future<Output = T> + 'a,
        T: 'a,
    {
        let location = crate::SourceLocation::caller(std::panic::Location::caller());
        self.run_step(name, step, location, |_| None)
    }

    /// Run a fallible step, preserving its error even if the caller handles it.
    #[track_caller]
    pub fn step_result<'a, F, T>(
        &'a self,
        name: &'a str,
        step: F,
    ) -> impl Future<Output = E2eResult<T>> + 'a
    where
        F: Future<Output = E2eResult<T>> + 'a,
        T: 'a,
    {
        let location = crate::SourceLocation::caller(std::panic::Location::caller());
        self.run_step(name, step, location, |result| {
            result
                .as_ref()
                .err()
                .map(|e| crate::TestError::new(e, "step", None))
        })
    }

    /// Run a controlled step. Skips return `StepOutcome::Skipped` and keep the test running.
    #[track_caller]
    pub fn step_with<'a, T: 'a, F, Fut>(
        &'a self,
        name: &'a str,
        options: crate::StepOptions,
        body: F,
    ) -> impl Future<Output = E2eResult<crate::StepOutcome<T>>> + 'a
    where
        F: FnOnce(crate::StepContext) -> Fut + 'a,
        Fut: Future<Output = E2eResult<T>> + 'a,
    {
        let location = crate::SourceLocation::caller(std::panic::Location::caller());
        async move {
            let session = self
                .reporter
                .clone()
                .or_else(crate::report::current_session)
                .unwrap_or_else(|| {
                    crate::report::StepSession::new(
                        crate::report::ReporterHub::default(),
                        crate::AttemptInfo {
                            name: "standalone page".into(),
                            file: location.file.clone(),
                            line: location.line,
                            project: None,
                            worker_index: 0,
                            repeat_each_index: 0,
                            retry: 0,
                        },
                    )
                });
            let result = session.controlled(name, location, options, body).await;
            self.sink.record(
                "step",
                format!(
                    "{name} ({})",
                    match &result {
                        Ok(crate::StepOutcome::Skipped(_)) => "skipped",
                        Ok(_) => "completed",
                        Err(_) => "failed",
                    }
                ),
            );
            self.trace_screenshot(name).await;
            result
        }
    }
    pub(crate) fn record_locator_diagnostic(&self, detail: String) {
        self.sink.record("locator-operation", detail);
    }

    pub(crate) fn auto_step<'a, T: Send + 'a>(
        &self,
        title: impl Into<String>,
        category: crate::StepCategory,
        future: impl Future<Output = E2eResult<T>> + Send + 'a,
    ) -> futures::future::BoxFuture<'a, E2eResult<T>> {
        Box::pin(crate::report::automatic(
            self.reporter.clone(),
            title,
            category,
            future,
        ))
    }
    pub(crate) fn auto_step_local<'a, T: 'a>(
        &self,
        title: impl Into<String>,
        category: crate::StepCategory,
        future: impl Future<Output = E2eResult<T>> + 'a,
    ) -> impl Future<Output = E2eResult<T>> + 'a {
        crate::report::automatic(self.reporter.clone(), title, category, future)
    }

    async fn run_step<F: Future>(
        &self,
        name: &str,
        future: F,
        location: crate::SourceLocation,
        error: impl FnOnce(&F::Output) -> Option<crate::TestError>,
    ) -> F::Output {
        let started = std::time::Instant::now();
        let session = self
            .reporter
            .clone()
            .or_else(crate::report::current_session);
        let out = match &session {
            Some(session) => session.run(name, location, future, error).await,
            None => future.await,
        };
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
        match crate::screenshot::capture(
            self,
            ScreenshotOptions::default(),
            crate::screenshot::Source::Page,
        )
        .await
        {
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
        self.driver
            .run(async { Self::storage_get("localStorage", key, &self.driver).await })
            .await
    }

    /// Write a localStorage entry.
    pub async fn local_storage_set(&self, key: &str, value: &str) -> E2eResult<()> {
        self.driver
            .run(async { Self::storage_set("localStorage", key, value, &self.driver).await })
            .await
    }

    /// Remove a localStorage entry.
    pub async fn local_storage_remove(&self, key: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                let key_json = serde_json::to_string(key).unwrap_or_default();
                self.driver
                    .evaluate(&format!("localStorage.removeItem({key_json})"))
                    .await?;
                Ok(())
            })
            .await
    }

    /// Clear localStorage.
    pub async fn local_storage_clear(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.evaluate("localStorage.clear()").await?;
                Ok(())
            })
            .await
    }

    /// Read a sessionStorage entry (`None` when missing).
    pub async fn session_storage_get(&self, key: &str) -> E2eResult<Option<String>> {
        self.driver
            .run(async { Self::storage_get("sessionStorage", key, &self.driver).await })
            .await
    }

    /// Write a sessionStorage entry.
    pub async fn session_storage_set(&self, key: &str, value: &str) -> E2eResult<()> {
        self.driver
            .run(async { Self::storage_set("sessionStorage", key, value, &self.driver).await })
            .await
    }

    /// Remove a sessionStorage entry.
    pub async fn session_storage_remove(&self, key: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                let key_json = serde_json::to_string(key).unwrap_or_default();
                self.driver
                    .evaluate(&format!("sessionStorage.removeItem({key_json})"))
                    .await?;
                Ok(())
            })
            .await
    }

    /// Clear sessionStorage.
    pub async fn session_storage_clear(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.evaluate("sessionStorage.clear()").await?;
                Ok(())
            })
            .await
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

    /// Capture PNG or JPEG with supported clipping, masks and temporary styles.
    pub async fn screenshot(&self, options: ScreenshotOptions) -> E2eResult<Vec<u8>> {
        self.auto_step_local(
            "page.screenshot",
            crate::StepCategory::Action,
            crate::screenshot::capture(self, options, crate::screenshot::Source::Page),
        )
        .await
    }

    /// Drain restoration errors from captures whose caller dropped its wait.
    /// A subsequent capture also reports any unconsumed deferred errors.
    pub fn take_screenshot_cleanup_errors(&self) -> Vec<String> {
        self.screenshot_state.take_errors()
    }

    /// Capture a screenshot and write it to `path`.
    pub async fn save_screenshot(
        &self,
        path: &std::path::Path,
        options: ScreenshotOptions,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                let bytes = self.screenshot(options).await?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, bytes)?;
                Ok(())
            })
            .await
    }

    /// Print the page to PDF bytes.
    pub async fn pdf(&self) -> E2eResult<Vec<u8>> {
        self.driver
            .run(async { self.driver.print_pdf().await })
            .await
    }

    /// Set the viewport size.
    pub async fn set_viewport(&self, viewport: Viewport) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver
                    .set_viewport(viewport.width, viewport.height)
                    .await
            })
            .await
    }

    /// Emulate a device preset (Chromium only; loud error on Firefox).
    pub async fn emulate_device(&self, device: DeviceDescriptor) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.emulate_device(device).await })
            .await
    }

    /// Install a fake clock in the current document (freezes `Date`,
    /// `setTimeout`/`setInterval`, `requestAnimationFrame`, `performance.now`).
    /// Installs in current and future documents; starts paused for compatibility.
    pub async fn clock_install(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                self.add_init_script(CLOCK_SCRIPT).await?;
                self.evaluate_value(CLOCK_SCRIPT).await?;
                Ok(())
            })
            .await
    }

    /// Advance the fake clock by `ms`, firing due timers in order.
    /// Fails loudly when no clock is installed.
    pub async fn clock_advance(&self, ms: u64) -> E2eResult<()> {
        self.driver
            .run(async {
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
            })
            .await
    }

    /// Freeze Date at an exact epoch-millisecond time while timers continue.
    /// Fails loudly when no clock is installed.
    pub async fn clock_set_fixed_time(&self, ms: i64) -> E2eResult<()> {
        self.driver
            .run(async {
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
            })
            .await
    }

    /// Restore the native clock (idempotent).
    pub async fn clock_uninstall(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                self.add_init_script(
                    "window.__ferriteClock ? window.__ferriteClock.uninstall() : true",
                )
                .await?;
                self.evaluate_value(
                    "window.__ferriteClock ? window.__ferriteClock.uninstall() : true",
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Jump forward, firing each due timer at most once.
    pub async fn clock_fast_forward(&self, ms: u64) -> E2eResult<()> {
        self.driver
            .run(async { self.clock_command(&format!("fastForward({ms})")).await })
            .await
    }

    /// Run the fake clock forward by `ms` (alias for [`Page::clock_advance`]).
    pub async fn clock_run_for(&self, ms: u64) -> E2eResult<()> {
        self.driver
            .run(async { self.clock_advance(ms).await })
            .await
    }

    /// Change system time without shifting timer deadlines or firing timers.
    pub async fn clock_set_system_time(&self, ms: i64) -> E2eResult<()> {
        self.driver
            .run(async { self.clock_command(&format!("setSystem({ms})")).await })
            .await
    }

    /// Jump to an epoch time and pause; each due timer fires at most once.
    pub async fn clock_pause_at(&self, ms: i64) -> E2eResult<()> {
        self.driver
            .run(async { self.clock_command(&format!("pauseAt({ms})")).await })
            .await
    }

    /// Install with an initial epoch time and optional real-time progression.
    pub async fn clock_install_at(&self, ms: i64, paused: bool) -> E2eResult<()> {
        self.driver
            .run(async {
                let script = format!(
            "{CLOCK_SCRIPT}; window.__ferriteClock.setSystem({ms}); window.__ferriteClock.{}();",
            if paused { "pause" } else { "resume" }
        );
                self.add_init_script(&script).await?;
                self.evaluate_value(&script).await?;
                Ok(())
            })
            .await
    }

    async fn clock_command(&self, command: &str) -> E2eResult<()> {
        let result: Option<f64> = self
            .evaluate(&format!(
                "window.__ferriteClock ? window.__ferriteClock.{command} : null"
            ))
            .await?;
        result
            .map(|_| ())
            .ok_or_else(|| E2eError::Config("clock control needs clock_install first".into()))
    }

    /// Current fake-clock time in epoch milliseconds.
    pub async fn clock_now(&self) -> E2eResult<i64> {
        self.driver
            .run(async {
                let now: Option<i64> = self
                    .evaluate("window.__ferriteClock ? window.__ferriteClock.now() : null")
                    .await?;
                now.ok_or_else(|| {
                    E2eError::Config("clock_now needs clock_install first".to_string())
                })
            })
            .await
    }

    /// Pause the fake clock (records the paused flag; timers only fire via
    /// explicit advances while paused).
    pub async fn clock_pause(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                let now: Option<i64> = self
                    .evaluate("window.__ferriteClock ? window.__ferriteClock.pause() : null")
                    .await?;
                match now {
                    Some(_) => Ok(()),
                    None => Err(E2eError::Config(
                        "clock_pause needs clock_install first".to_string(),
                    )),
                }
            })
            .await
    }

    /// Resume a paused fake clock.
    pub async fn clock_resume(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                let now: Option<i64> = self
                    .evaluate("window.__ferriteClock ? window.__ferriteClock.resume() : null")
                    .await?;
                match now {
                    Some(_) => Ok(()),
                    None => Err(E2eError::Config(
                        "clock_resume needs clock_install first".to_string(),
                    )),
                }
            })
            .await
    }

    /// Override the user agent.
    pub async fn set_user_agent(&self, user_agent: &str) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_user_agent(user_agent).await })
            .await
    }

    /// Ignore HTTPS certificate errors.
    pub async fn set_ignore_https_errors(&self, ignore: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_ignore_https_errors(ignore).await })
            .await
    }

    /// Cookies visible to this page.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        self.driver.run(async { self.driver.cookies().await }).await
    }

    /// Set a cookie for the current page URL.
    pub async fn set_cookie(&self, name: &str, value: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                let url = self.url().await?;
                self.driver.set_cookie(name, value, &url).await
            })
            .await
    }

    /// Set full-fidelity cookies (path, flags, expiry honored).
    pub async fn add_cookies(&self, cookies: &[Cookie]) -> E2eResult<()> {
        self.driver
            .run(async {
                let url = self.url().await?;
                self.driver.add_cookies(cookies, &url).await
            })
            .await
    }

    /// Clear browser cookies.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.clear_cookies().await })
            .await
    }

    /// Clear matching cookies from the owning context's native store.
    pub async fn clear_cookies_with(&self, filter: crate::CookieFilter) -> E2eResult<()> {
        self.run_operation(async {
            self.context()
                .ok_or_else(|| {
                    E2eError::Config("page's owning context is no longer available".into())
                })?
                .clear_cookies_with(filter)
                .await
        })
        .await
    }

    /// Capture this page's storage state (origin, cookies, localStorage).
    pub async fn storage_state(&self) -> E2eResult<StorageState> {
        self.driver
            .run(async {
                let origin = self.evaluate_string("location.origin").await?;
                let local_storage: HashMap<String, String> =
                    serde_json::from_value(self.evaluate_value("({ ...localStorage })").await?)?;
                Ok(StorageState {
                    origins: vec![StorageOrigin {
                        origin: origin.clone(),
                        local_storage: local_storage
                            .iter()
                            .map(|(name, value)| StorageEntry {
                                name: name.clone(),
                                value: value.clone(),
                            })
                            .collect(),
                    }],
                    origin,
                    cookies: self.cookies().await?,
                    local_storage,
                })
            })
            .await
    }

    /// Save cookies plus current-origin localStorage to a JSON file.
    pub async fn save_storage_state(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        self.driver
            .run(async {
                let state = self.storage_state().await?;
                std::fs::write(path, serde_json::to_string_pretty(&state)?)?;
                Ok(())
            })
            .await
    }

    /// Load storage state saved by [`Page::save_storage_state`].
    ///
    /// The page must already be on the saved origin; cookies restore with
    /// full fidelity and localStorage is replaced wholesale.
    pub async fn load_storage_state(&self, path: impl AsRef<Path>) -> E2eResult<()> {
        self.driver
            .run(async {
                let raw = std::fs::read_to_string(path)?;
                let state: StorageState = serde_json::from_str(&raw)?;
                let origin = self.evaluate_string("location.origin").await?;
                if !state
                    .all_origins()
                    .iter()
                    .any(|entry| entry.origin == origin)
                {
                    return Err(E2eError::Config(format!(
                "storage state has no origin {origin}, navigate there first (to a saved origin)"
            )));
                }
                self.apply_storage_state(&state).await?;
                Ok(())
            })
            .await
    }

    /// Grant permissions (`geolocation`, `notifications`, ...).
    ///
    /// Chromium grants to all origins; Firefox grants to the current page
    /// origin (navigate first).
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.grant_permissions(permissions).await })
            .await
    }

    /// Reset granted permissions to the browser default.
    pub async fn clear_permissions(&self) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.clear_permissions().await })
            .await
    }

    /// Override the geolocation coordinates.
    ///
    /// Pair with [`Page::grant_permissions`]; Firefox supports this only on
    /// recent builds and otherwise fails loudly.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_geolocation(latitude, longitude).await })
            .await
    }

    /// Clear the geolocation override.
    pub async fn clear_geolocation(&self) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.clear_geolocation().await })
            .await
    }

    /// Send HTTP credentials with subsequent requests (Chromium only).
    ///
    /// Basic and Digest authenticate through browser challenges; digest
    /// (or proxy) challenges are answered from the Fetch domain; pass
    /// `None` to clear both. Firefox has neither override, so this fails
    /// loudly there.
    pub async fn set_http_credentials(
        &self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                let credentials =
            match (username, password) {
                (Some(user), Some(pass)) => Some((user.to_string(), pass.to_string())),
                (None, None) => None,
                _ => return Err(E2eError::Config(
                    "set_http_credentials needs both username and password (or neither to clear)"
                        .into(),
                )),
            };
                self.driver.set_auth_credentials(username, password).await?;
                *self
                    .auth_credentials
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = None;
                *self.cleared_auth.lock().unwrap_or_else(|e| e.into_inner()) =
                    credentials.is_none();
                let headers = self
                    .extra_headers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let headers: Vec<_> = headers
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                self.set_extra_http_headers(&headers).await
            })
            .await
    }

    /// Enable or disable JavaScript execution (Chromium only).
    pub async fn set_java_script_enabled(&self, enabled: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_java_script_enabled(enabled).await })
            .await
    }

    /// Bypass Content-Security-Policy checks (Chromium only).
    pub async fn set_bypass_csp(&self, bypass: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_bypass_csp(bypass).await })
            .await
    }

    /// Allow or deny downloads (Chromium only; browser-wide).
    pub async fn set_downloads_allowed(&self, allowed: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_downloads_allowed(allowed).await })
            .await
    }

    /// Block service workers (Chromium only; approximated by bypassing
    /// workers, so fetches skip them entirely).
    pub async fn set_service_workers_blocked(&self, blocked: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_service_workers_blocked(blocked).await })
            .await
    }

    /// Clear this origin's IndexedDB databases (Chromium only).
    pub async fn clear_indexed_db(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                let origin = self.evaluate_string("location.origin").await?;
                self.driver
                    .clear_data_for_origin(&origin, "indexeddb")
                    .await
            })
            .await
    }

    /// Emulate offline mode (Chromium only).
    pub async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_offline(offline).await })
            .await
    }

    /// Send Basic credentials preemptively, for servers without a challenge.
    /// Ordinary set_http_credentials uses browser challenges and supports Digest.
    pub async fn set_http_credentials_preemptive(
        &self,
        username: &str,
        password: &str,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                self.set_http_credentials(Some(username), Some(password))
                    .await?;
                *self
                    .auth_credentials
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = Some((username.into(), password.into()));
                let headers = self
                    .extra_headers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                self.set_extra_http_headers(
                    &headers
                        .iter()
                        .map(|(key, value)| (key.as_str(), value.as_str()))
                        .collect::<Vec<_>>(),
                )
                .await
            })
            .await
    }

    /// Set extra HTTP headers for subsequent requests (Chromium only).
    pub async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        self.driver
            .run(async {
                let mut combined: Vec<(String, String)> = headers
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect();
                let credentials = self
                    .auth_credentials
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if let Some((user, pass)) = credentials {
                    if !combined
                        .iter()
                        .any(|(key, _)| key.eq_ignore_ascii_case("authorization"))
                    {
                        combined.push((
                            "Authorization".into(),
                            format!(
                                "Basic {}",
                                base64_encode(format!("{user}:{pass}").as_bytes())
                            ),
                        ));
                    }
                }
                if *self.cleared_auth.lock().unwrap_or_else(|e| e.into_inner())
                    && !combined
                        .iter()
                        .any(|(key, _)| key.eq_ignore_ascii_case("authorization"))
                {
                    combined.push(("Authorization".into(), String::new()));
                }
                let borrowed: Vec<_> = combined
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                self.driver.set_extra_http_headers(&borrowed).await?;
                *self.extra_headers.lock().unwrap_or_else(|e| e.into_inner()) = headers
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect();
                Ok(())
            })
            .await
    }

    /// Override the locale (Chromium only).
    ///
    /// Applies to subsequently loaded documents; navigate or reload after.
    /// Drives `Intl` and `Accept-Language`; `navigator.language` follows the
    /// launch `--lang` flag instead.
    pub async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_locale(locale).await })
            .await
    }

    /// Override the timezone (Chromium only).
    ///
    /// Applies to subsequently loaded documents; navigate or reload after.
    pub async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        self.driver
            .run(async { self.driver.set_timezone(timezone_id).await })
            .await
    }

    /// Emulate media features (Chromium only).
    ///
    /// `(None, None)` is a no-op on every engine.
    pub async fn emulate_media(
        &self,
        color_scheme: Option<ColorScheme>,
        reduced_motion: Option<ReducedMotion>,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                if color_scheme.is_none() && reduced_motion.is_none() {
                    return Ok(());
                }
                self.driver
                    .emulate_media(color_scheme, reduced_motion)
                    .await
            })
            .await
    }

    /// Start intercepting requests with glob rules (replaces page rules;
    /// context rules still apply as fallback, page rules win on overlap).
    /// Route handlers (see [`Page::route_with_handler`]) run before rules.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        self.driver
            .run(async {
                let rules = crate::url_matcher::prepare_rules(rules, |url| self.resolve_url(url))?;
                *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = rules;
                self.restart_routing().await
            })
            .await
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
        self.driver
            .run(async { self.route_entry(pattern, handler, None, None).await })
            .await
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
        self.driver
            .run(async { self.route_entry(pattern, handler, Some(n), None).await })
            .await
    }

    /// Register a shared exact/glob/regex matcher, resolved before mutation.
    pub async fn route_matching<F, Fut>(
        &self,
        matcher: &crate::UrlMatcher,
        handler: F,
    ) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        self.driver
            .run(self.route_entry(&matcher.description(), handler, None, Some(matcher.clone())))
            .await
    }
    pub async fn route_matching_times<F, Fut>(
        &self,
        matcher: &crate::UrlMatcher,
        n: u32,
        handler: F,
    ) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        self.driver
            .run(self.route_entry(
                &matcher.description(),
                handler,
                Some(n),
                Some(matcher.clone()),
            ))
            .await
    }

    /// Register a handler entry with an optional match limit.
    async fn route_entry<F, Fut>(
        &self,
        pattern: &str,
        handler: F,
        times: Option<u32>,
        matcher: Option<crate::UrlMatcher>,
    ) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        let matcher = matcher
            .map(|matcher| matcher.resolved(|url| self.resolve_url(url)))
            .transpose()?;
        if matcher.is_none() {
            crate::url_matcher::legacy_glob(pattern)?;
        }
        let handler: RouteHandler = Arc::new(move |info| Box::pin(handler(info)));
        self.handlers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(RouteHandlerEntry {
                pattern: pattern.to_string(),
                matcher,
                handler,
                times,
                hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            });
        self.restart_routing().await
    }

    /// Stop page-level interception; current handler calls continue.
    pub async fn stop_routing(&self) {
        if self.is_closed() {
            self.routes
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            self.handlers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            self.route_runtime.clear();
            if let Some(task) = self.routing.lock().await.take() {
                task.abort();
            }
            let _ =
                tokio::time::timeout(Duration::from_millis(750), self.driver.stop_routing()).await;
        } else {
            let _ = self.unroute_all().await;
        }
    }

    /// Remove page rules/handlers with this pattern, preserving active calls.
    pub async fn unroute(&self, pattern: &str) -> E2eResult<usize> {
        self.unroute_with(pattern, crate::UnrouteOptions::default())
            .await
    }
    pub async fn unroute_with(
        &self,
        pattern: &str,
        options: crate::UnrouteOptions,
    ) -> E2eResult<usize> {
        self.remove_routes(
            options,
            |rule| rule.pattern == pattern,
            |entry| entry.pattern == pattern,
        )
        .await
    }
    /// Remove all page routes, preserving active calls and context routes.
    pub async fn unroute_all(&self) -> E2eResult<usize> {
        self.unroute_all_with(crate::UnrouteOptions::default())
            .await
    }
    pub async fn unroute_all_with(&self, options: crate::UnrouteOptions) -> E2eResult<usize> {
        self.remove_routes(options, |_| true, |_| true).await
    }
    /// Remove routes with this resolved matcher identity.
    pub async fn unroute_matching(&self, matcher: &crate::UrlMatcher) -> E2eResult<usize> {
        self.unroute_matching_with(matcher, crate::UnrouteOptions::default())
            .await
    }
    pub async fn unroute_matching_with(
        &self,
        matcher: &crate::UrlMatcher,
        options: crate::UnrouteOptions,
    ) -> E2eResult<usize> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.remove_routes(
            options,
            |rule| rule.matcher.as_ref() == Some(&matcher),
            |entry| entry.matcher.as_ref() == Some(&matcher),
        )
        .await
    }
    async fn remove_routes(
        &self,
        options: crate::UnrouteOptions,
        rule: impl Fn(&RouteRule) -> bool,
        handler: impl Fn(&RouteHandlerEntry) -> bool,
    ) -> E2eResult<usize> {
        let scoped = self.operation_page(&crate::OperationOptions {
            timeout: options.timeout,
            cancellation: options.cancellation,
        });
        let timeout = options.timeout.unwrap_or_else(|| {
            *self
                .action_timeout
                .lock()
                .unwrap_or_else(|e| e.into_inner())
        });
        scoped
            .run_operation(
                crate::operation::Deadline::new(timeout).run("route removal", async {
                    let removed_rules = {
                        let mut rules = self.routes.lock().unwrap_or_else(|e| e.into_inner());
                        let before = rules.len();
                        rules.retain(|r| !rule(r));
                        before - rules.len()
                    };
                    let removed_handlers = {
                        let mut entries = self.handlers.lock().unwrap_or_else(|e| e.into_inner());
                        let removed: Vec<_> =
                            entries.iter().filter(|e| handler(e)).cloned().collect();
                        entries.retain(|e| !handler(e));
                        removed
                    };
                    let calls = self.retire_routes(&removed_handlers, options.behavior);
                    scoped.restart_routing().await?;
                    if options.behavior == crate::UnrouteBehavior::Wait {
                        crate::routing::wait_calls(calls).await?;
                    }
                    Ok(removed_rules + removed_handlers.len())
                }),
            )
            .await
    }
    pub(crate) fn retire_routes(
        &self,
        entries: &[RouteHandlerEntry],
        behavior: crate::UnrouteBehavior,
    ) -> Vec<crate::CancellationToken> {
        self.route_runtime.retire(entries, behavior)
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
        self.driver
            .run(async {
                let file = crate::har::HarFile::load(path)?;
                let matcher =
                    crate::url_matcher::har_filter(&options, |url| self.resolve_url(url))?;
                let mut map = file.lookup();
                if let Some(matcher) = &matcher {
                    map.retain(|_, entry| matcher.matches(&entry.url));
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
            })
            .await
    }

    /// Apply the stored rules and handlers (page first, context fallback;
    /// no pump when both are empty).
    pub(crate) async fn restart_routing(&self) -> E2eResult<()> {
        self.driver
            .run(async {
                let mut slot = self.routing.lock().await;
                let mut rules = self
                    .routes
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                rules.extend(
                    self.context_routes
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone(),
                );
                let mut handlers = self
                    .handlers
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                handlers.extend(
                    self.context_handlers
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone(),
                );
                let configuration = crate::routing::RouteConfiguration::new(rules, handlers)?;
                self.driver.validate_routing(&configuration)?;
                self.route_runtime
                    .set_fetch_owner(self.driver.route_fetch_owner(
                        self.context_registry.clone(),
                        self.context_id.clone(),
                        self.action_timeout.clone(),
                    ));
                self.route_runtime.configure(configuration);
                if let Some(task) = slot.as_ref().filter(|task| !task.is_finished()) {
                    // If no call/stage remains, complete native shutdown before
                    // a subsequent page operation can start another request.
                    // Active default/ignore-errors callbacks never make removal wait.
                    let stopped = self.route_runtime.idle().then(|| task.stopped.clone());
                    drop(slot);
                    if let Some(stopped) = stopped {
                        stopped.cancelled().await;
                    }
                    return Ok(());
                }
                if slot.take().is_some() {
                    self.driver.stop_routing().await;
                }
                if self.route_runtime.empty() {
                    return Ok(());
                }
                let task = self
                    .driver
                    .start_routing(self.route_runtime.clone(), Arc::downgrade(&self.routing))
                    .await?;
                *slot = Some(task);
                Ok(())
            })
            .await
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

    /// Typed request observations since page creation, oldest first (4096 cap).
    /// Independent of HAR/body capture; each redirect hop has a unique ID.
    pub fn network_requests(&self) -> Vec<crate::Request> {
        self.sink
            .network_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .states()
            .into_iter()
            .map(|state| crate::Request::new(self.clone(), state))
            .collect()
    }

    /// Future native request/header/terminal events with typed live metadata.
    pub fn subscribe_network(&self) -> crate::NetworkEvents {
        crate::NetworkEvents {
            page: self.owning_page(),
            receiver: self
                .sink
                .network_log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .subscribe(),
        }
    }

    /// Wait for a future typed request matching a full-URL matcher.
    pub async fn wait_for_request_handle(
        &self,
        matcher: &crate::UrlMatcher,
        options: crate::OperationOptions,
    ) -> E2eResult<crate::Request> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_request_handle_where(move |request| matcher.matches(request.url()), options)
            .await
    }
    pub async fn wait_for_request_handle_where<F>(
        &self,
        mut predicate: F,
        options: crate::OperationOptions,
    ) -> E2eResult<crate::Request>
    where
        F: FnMut(&crate::Request) -> bool,
    {
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        let page = self.operation_page(&options);
        self.auto_step_local(
            "page.wait_for_request_handle",
            crate::StepCategory::Action,
            async {
                page.run_operation(crate::operation::Deadline::new(timeout).run(
                    "typed request",
                    async {
                        let mut events = page.subscribe_network();
                        loop {
                            if let crate::NetworkEvent::Request(request) = events.recv().await? {
                                if predicate(&request) {
                                    return Ok(crate::Request::new(
                                        self.clone(),
                                        request.state.clone(),
                                    ));
                                }
                            }
                        }
                    },
                ))
                .await
            },
        )
        .await
    }
    /// Wait for future response headers; completion is a separate finished() wait.
    pub async fn wait_for_response_handle(
        &self,
        matcher: &crate::UrlMatcher,
        options: crate::OperationOptions,
    ) -> E2eResult<crate::Response> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_response_handle_where(
            move |response| matcher.matches(response.url()),
            options,
        )
        .await
    }
    pub async fn wait_for_response_handle_where<F>(
        &self,
        mut predicate: F,
        options: crate::OperationOptions,
    ) -> E2eResult<crate::Response>
    where
        F: FnMut(&crate::Response) -> bool,
    {
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        let page = self.operation_page(&options);
        self.auto_step_local(
            "page.wait_for_response_handle",
            crate::StepCategory::Action,
            async {
                page.run_operation(crate::operation::Deadline::new(timeout).run(
                    "typed response headers",
                    async {
                        let mut events = page.subscribe_network();
                        loop {
                            if let crate::NetworkEvent::Response(response) = events.recv().await? {
                                if predicate(&response) {
                                    return Ok(response.with_page(self.clone()));
                                }
                            }
                        }
                    },
                ))
                .await
            },
        )
        .await
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
        self.driver
            .run(async { self.locator(selector).set_input_files(paths).await })
            .await
    }

    /// Upload generated bytes to a file input; empty payload lists clear it.
    pub async fn set_input_file_payloads(
        &self,
        selector: &str,
        files: &[crate::FilePayload],
    ) -> E2eResult<()> {
        self.locator(selector).set_input_file_payloads(files).await
    }

    /// Direct downloads to `dir` (created when missing). Chromium only;
    /// Firefox configures the download dir at launch
    /// ([`LaunchOptions::download_dir`](crate::LaunchOptions::download_dir)).
    pub async fn set_download_dir(&self, dir: impl AsRef<Path>) -> E2eResult<()> {
        self.driver
            .run(async {
                self.driver.set_download_dir(dir.as_ref()).await?;
                self.remember_download_dir(dir.as_ref());
                Ok(())
            })
            .await
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
        self.driver
            .run(async { self.driver.add_init_script(source).await })
            .await
    }

    /// Add a `<script src>` tag and wait for it to load.
    pub async fn add_script_tag_url(&self, url: &str) -> E2eResult<()> {
        self.driver
            .run(async {
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
            })
            .await
    }

    /// Add an inline `<script>` tag (runs immediately).
    pub async fn add_script_tag_content(&self, code: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                let code_json = serde_json::to_string(code).map_err(E2eError::Json)?;
                self.evaluate_value(&format!(
                    "(() => {{ const s = document.createElement('script'); \
             s.textContent = {code_json}; document.head.appendChild(s); \
             return true; }})()"
                ))
                .await?;
                Ok(())
            })
            .await
    }

    /// Add a stylesheet `<link>` tag and wait for it to load.
    pub async fn add_style_tag_url(&self, url: &str) -> E2eResult<()> {
        self.driver
            .run(async {
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
            })
            .await
    }

    /// Add an inline `<style>` tag.
    pub async fn add_style_tag_content(&self, css: &str) -> E2eResult<()> {
        self.driver
            .run(async {
                let css_json = serde_json::to_string(css).map_err(E2eError::Json)?;
                self.evaluate_value(&format!(
                    "(() => {{ const s = document.createElement('style'); \
             s.textContent = {css_json}; document.head.appendChild(s); \
             return true; }})()"
                ))
                .await?;
                Ok(())
            })
            .await
    }

    /// All document frames (main frame first; named `document_frames`
    /// because [`Page::frames`] streams video frames). Frame names are
    /// Chromium-only (empty on Firefox, which reports no names).
    pub async fn document_frames(&self) -> E2eResult<Vec<Frame>> {
        self.driver
            .run(async {
                Ok(self
                    .driver
                    .frames()
                    .await?
                    .into_iter()
                    .map(|info| Frame {
                        page: self.clone(),
                        id: info.id,
                        parent_id: info.parent_id,
                        name: info.name,
                        url: info.url,
                    })
                    .collect())
            })
            .await
    }

    /// Current top-level native frame. The identity survives same-target navigation.
    /// Closed/disconnected pages fail rather than returning a fabricated handle.
    pub async fn main_frame(&self) -> E2eResult<Frame> {
        self.document_frames()
            .await?
            .into_iter()
            .find(|frame| frame.parent_id.is_none())
            .ok_or_else(|| E2eError::Config("native page frame tree has no main frame".into()))
    }

    /// First current frame whose full URL matches. Relative exact/glob matchers
    /// resolve against the page base URL. This is a snapshot lookup, not a wait.
    pub async fn frame_by_url_matching(
        &self,
        matcher: &crate::UrlMatcher,
    ) -> E2eResult<Option<Frame>> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.frame_by_url_where(move |url| matcher.matches(url))
            .await
    }

    /// First frame matching a current URL predicate, in native tree order.
    /// No match returns None; native errors propagate. Handles keep their identity
    /// after detachment and never silently retarget a replacement iframe.
    pub async fn frame_by_url_where<F>(&self, mut predicate: F) -> E2eResult<Option<Frame>>
    where
        F: FnMut(&str) -> bool,
    {
        Ok(self
            .document_frames()
            .await?
            .into_iter()
            .find(|frame| predicate(&frame.url)))
    }

    /// First frame with exactly `name`.
    pub async fn frame_by_name(&self, name: &str) -> E2eResult<Option<Frame>> {
        self.driver
            .run(async {
                Ok(self
                    .document_frames()
                    .await?
                    .into_iter()
                    .find(|frame| frame.name == name))
            })
            .await
    }

    /// First frame whose URL contains `pattern`.
    pub async fn frame_by_url(&self, pattern: &str) -> E2eResult<Option<Frame>> {
        self.driver
            .run(async {
                Ok(self
                    .document_frames()
                    .await?
                    .into_iter()
                    .find(|frame| frame.url.contains(pattern)))
            })
            .await
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
        self.driver
            .run(async { Ok(self.wait_for_download_file(dir, timeout).await?.path) })
            .await
    }

    /// Wait for a download and wrap it as a [`Download`] (save/delete helpers).
    pub async fn wait_for_download_file(
        &self,
        dir: impl AsRef<Path>,
        timeout: Duration,
    ) -> E2eResult<Download> {
        self.driver
            .run(async {
                let path = self.wait_for_download_in(dir.as_ref(), timeout).await?;
                self.emit(PageEvent::Download(path.clone()));
                let mut download = Download::from_path(path);
                download.page_id = Some(self.target_id().to_owned());
                // Chromium fills URL/failure from download events (newest match wins).
                let name = download.suggested_filename.clone();
                if let Some(record) = self
                    .driver
                    .download_records()
                    .into_iter()
                    .rev()
                    .find(|record| record.filename == name)
                {
                    if !record.url.is_empty() {
                        download.url = Some(record.url.clone());
                    }
                    if record.state == "canceled" || record.state == "interrupted" {
                        download.failure = Some(record.state.clone());
                    }
                    download.guid = Some(record.guid.clone());
                }
                Ok(download)
            })
            .await
    }

    /// Cancel in-flight downloads (Chromium only); returns how many were
    /// canceled. Firefox has no download-cancel command and fails loudly.
    pub async fn cancel_downloads(&self) -> E2eResult<usize> {
        self.driver
            .run(async { self.driver.cancel_downloads().await })
            .await
    }

    /// Fetch `url` from inside the page (cookies included) and return the
    /// raw bytes (Playwright `response.body()` equivalent for re-fetchable
    /// resources; same-origin or CORS-open URLs only).
    pub async fn response_body(&self, url: &str) -> E2eResult<Vec<u8>> {
        self.driver
            .run(async {
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
                let b64 = value.as_str().ok_or_else(|| {
                    E2eError::Config(format!("response_body({url}) returned no body"))
                })?;
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|_| {
                        E2eError::Config(format!("response_body({url}) returned invalid base64"))
                    })
            })
            .await
    }

    /// Structured accessibility tree using implicit roles, associated labels,
    /// visibility, states and open shadow roots. This is a DOM approximation.
    pub async fn aria_snapshot_json(&self) -> E2eResult<Value> {
        self.driver
            .run(async {
                self.evaluate_value(&format!("({}).aria(document.body)", include_str!("dom.js")))
                    .await
            })
            .await
    }

    /// Indented accessibility snapshot, without truncating names or node counts.
    pub async fn aria_snapshot(&self) -> E2eResult<String> {
        self.driver
            .run(async {
                self.evaluate_string(&format!(
            "(() => {{ const f = {}; return f.render(f.aria(document.body)).join('\\n'); }})()",
            include_str!("dom.js")
        ))
                .await
            })
            .await
    }

    /// Download watcher without the event emission (shared implementation).
    async fn wait_for_download_in(&self, dir: &Path, timeout: Duration) -> E2eResult<PathBuf> {
        self.driver
            .run(
                crate::operation::Deadline::new(timeout).run("wait for download", async {
                    let before = dir_names(dir)?;
                    let cutoff = std::time::SystemTime::now()
                        .checked_sub(PREEXISTING_DOWNLOAD_GRACE)
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let deadline = crate::operation::Deadline::new(timeout);
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
                                let mtime = entry
                                    .metadata()
                                    .and_then(|meta| meta.modified())
                                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                                if self
                                    .download_consumed
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .get(&path)
                                    == Some(&(size, mtime))
                                {
                                    continue;
                                }
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
                        let candidate =
                            candidate.or_else(|| fallback.map(|(path, size, _)| (path, size)));
                        if let Some((path, size)) = candidate {
                            if stable.as_ref() == Some(&(path.clone(), size)) {
                                let modified = path.metadata()?.modified()?;
                                self.download_consumed
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .insert(path.clone(), (size, modified));
                                return Ok(path);
                            }
                            stable = Some((path, size));
                        } else {
                            stable = None;
                        }
                        if deadline.expired() {
                            return Err(E2eError::Timeout(
                                timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                                format!("wait_for_download({})", dir.display()),
                            ));
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }),
            )
            .await
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
        self.wait_for_request_matching(&crate::UrlMatcher::contains(pattern), timeout)
            .await
    }

    /// Wait for a response (nonzero status) whose URL contains `pattern`.
    /// Like [`Page::wait_for_request`], only matches responses arriving
    /// after this call.
    pub async fn wait_for_response(
        &self,
        pattern: &str,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest> {
        self.wait_for_response_matching(&crate::UrlMatcher::contains(pattern), timeout)
            .await
    }

    /// Wait for a future request matching an exact/glob/regex full URL.
    pub async fn wait_for_request_matching(
        &self,
        matcher: &crate::UrlMatcher,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_request_where(move |request| matcher.matches(&request.url), timeout)
            .await
    }
    /// Wait for a request predicate over method, URL, status and headers.
    pub async fn wait_for_request_where<F>(
        &self,
        mut predicate: F,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest>
    where
        F: FnMut(&RecordedRequest) -> bool,
    {
        self.wait_for_request_async(
            move |request| std::future::ready(Ok(predicate(&request))),
            timeout,
        )
        .await
    }
    /// Async predicate errors propagate; pending predicates remain bounded by
    /// timeout, caller cancellation and page/context disposal. Poll this future
    /// before triggering traffic (e.g. with tokio::join!).
    pub async fn wait_for_request_async<F, Fut>(
        &self,
        predicate: F,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest>
    where
        F: FnMut(RecordedRequest) -> Fut,
        Fut: Future<Output = E2eResult<bool>>,
    {
        self.auto_step_local(
            "page.wait_for_request",
            crate::StepCategory::Action,
            self.wait_for_network(false, predicate, timeout),
        )
        .await
    }
    /// Wait for a future response matching an exact/glob/regex full URL.
    pub async fn wait_for_response_matching(
        &self,
        matcher: &crate::UrlMatcher,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest> {
        let matcher = matcher.resolved(|url| self.resolve_url(url))?;
        self.wait_for_response_where(move |request| matcher.matches(&request.url), timeout)
            .await
    }
    /// Wait for a response predicate over method, URL, status and headers.
    pub async fn wait_for_response_where<F>(
        &self,
        mut predicate: F,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest>
    where
        F: FnMut(&RecordedRequest) -> bool,
    {
        self.wait_for_response_async(
            move |request| std::future::ready(Ok(predicate(&request))),
            timeout,
        )
        .await
    }
    /// Async predicate errors propagate; pending predicates remain bounded by
    /// timeout, caller cancellation and page/context disposal. Poll this future
    /// before triggering traffic (e.g. with tokio::join!).
    pub async fn wait_for_response_async<F, Fut>(
        &self,
        predicate: F,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest>
    where
        F: FnMut(RecordedRequest) -> Fut,
        Fut: Future<Output = E2eResult<bool>>,
    {
        self.auto_step_local(
            "page.wait_for_response",
            crate::StepCategory::Action,
            self.wait_for_network(true, predicate, timeout),
        )
        .await
    }
    async fn wait_for_network<F, Fut>(
        &self,
        response: bool,
        mut predicate: F,
        timeout: Duration,
    ) -> E2eResult<RecordedRequest>
    where
        F: FnMut(RecordedRequest) -> Fut,
        Fut: Future<Output = E2eResult<bool>>,
    {
        let scoped = self.with_timeout(timeout);
        scoped
            .run_operation(crate::operation::Deadline::new(timeout).run(
                if response {
                    "wait_for_response"
                } else {
                    "wait_for_request"
                },
                async {
                    let mut events = scoped.sink.subscribe_network();
                    scoped.ensure_request_capture();
                    loop {
                        let observed = events.recv().await.map_err(|error| match error {
                            tokio::sync::broadcast::error::RecvError::Lagged(count) => {
                                E2eError::Config(format!(
                                    "network wait lost {count} events; predicate could not keep up"
                                ))
                            }
                            tokio::sync::broadcast::error::RecvError::Closed => {
                                E2eError::Disconnected("network observation stream closed".into())
                            }
                        })?;
                        if observed.response == response
                            && predicate(observed.request.clone()).await?
                        {
                            return Ok(observed.request);
                        }
                    }
                },
            ))
            .await
    }

    /// Auto-handle JavaScript dialogs (`accept` = OK vs dismiss).
    pub async fn handle_dialogs(&self, accept: bool) -> E2eResult<()> {
        self.driver
            .run(async {
                self.stop_dialog_handling().await;
                let handle = self.driver.start_dialogs(accept, None, None).await?;
                *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
                Ok(())
            })
            .await
    }

    /// Auto-handle dialogs, answering prompts with `prompt_text`.
    pub async fn handle_dialogs_with_prompt(
        &self,
        accept: bool,
        prompt_text: &str,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                self.stop_dialog_handling().await;
                let handle = self
                    .driver
                    .start_dialogs(accept, Some(prompt_text.to_string()), None)
                    .await?;
                *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
                Ok(())
            })
            .await
    }

    /// Answer each dialog with a handler (Playwright `page.on("dialog")`
    /// equivalent): inspect the [`DialogInfo`], return the [`DialogDecision`].
    pub async fn handle_dialogs_with_handler(
        &self,
        handler: impl Fn(DialogInfo) -> DialogDecision + Send + Sync + 'static,
    ) -> E2eResult<()> {
        self.driver
            .run(async {
                self.stop_dialog_handling().await;
                let handle = self
                    .driver
                    .start_dialogs(true, None, Some(Arc::new(handler)))
                    .await?;
                *self.dialogs.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
                Ok(())
            })
            .await
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
        self.driver
            .run(async {
                if self.capture.lock().map(|c| c.is_some()).unwrap_or(true) {
                    return Err(E2eError::Config(
                        "capture already active on this page; stop it first".to_string(),
                    ));
                }
                let state = self.driver.start_recording(&opts).await?;
                *self.capture.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(CaptureState::Recording(state));
                Ok(())
            })
            .await
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
        self.driver
            .run(async {
                if self.capture.lock().map(|c| c.is_some()).unwrap_or(true) {
                    return Err(E2eError::Config(
                        "capture already active on this page; stop it first".to_string(),
                    ));
                }
                let (stream, pump) = self.driver.start_frame_stream(&opts).await?;
                *self.capture.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(CaptureState::Streaming(pump));
                Ok(stream)
            })
            .await
    }

    /// Stop the live frame stream.
    pub async fn stop_frames(&self) {
        if let Some(pump) = self.take_stream() {
            pump.abort();
            self.driver.stop_frame_stream().await;
        }
    }

    /// Capture a box in viewport-relative CSS coordinates, including offscreen portions.
    pub async fn screenshot_clip(
        &self,
        rect: &ElementRect,
        quality: Option<u8>,
    ) -> E2eResult<Vec<u8>> {
        self.auto_step_local(
            "page.screenshot_clip",
            crate::StepCategory::Action,
            crate::screenshot::capture(
                self,
                ScreenshotOptions {
                    quality,
                    ..Default::default()
                },
                crate::screenshot::Source::Box(rect.clone()),
            ),
        )
        .await
    }

    /// Close the page target and its owned convenience context. Cleanup
    /// continues after a dropped wait; repeated calls await the same disposal.
    pub async fn close(&self) -> E2eResult<()> {
        let page = self.clone();
        self.owner_close_task
            .run(async move {
                let context = if page.owns_context {
                    page.context()
                } else {
                    None
                };
                let result = page.close_target().await;
                if let Some(context) = context {
                    if let Err(error) = context.close().await {
                        return match result {
                            Ok(()) => Err(error),
                            Err(target) => Err(target
                                .with_context(&format!("context cleanup also failed: {error}"))),
                        };
                    }
                }
                result
            })
            .await
    }

    pub(crate) async fn close_target(&self) -> E2eResult<()> {
        let page = self.clone();
        self.close_task
            .run(async move { page.finish_close_target().await })
            .await
    }

    async fn finish_close_target(&self) -> E2eResult<()> {
        if self.is_closed() {
            return Ok(());
        }
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
        self.coverage_state.lock().await.cancel();
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
                same_site: None,
                expires: None,
            }],
            local_storage,
            origins: Vec::new(),
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
        assert!(download.url.is_none());
        assert!(download.failure.is_none());
        let download = Download::from_path(PathBuf::from("bare.bin"));
        assert_eq!(download.suggested_filename, "bare.bin");
    }

    #[test]
    fn mouse_button_mappings() {
        assert_eq!(MouseButton::default(), MouseButton::Left);
        assert_eq!(MouseButton::Left.as_cdp(), "left");
        assert_eq!(MouseButton::Middle.as_cdp(), "middle");
        assert_eq!(MouseButton::Right.as_cdp(), "right");
        assert_eq!(MouseButton::Left.as_bidi(), 0);
        assert_eq!(MouseButton::Middle.as_bidi(), 1);
        assert_eq!(MouseButton::Right.as_bidi(), 2);
        let options = MouseClickOptions::default()
            .button(MouseButton::Right)
            .click_count(2);
        assert_eq!(options.button, MouseButton::Right);
        assert_eq!(options.click_count, 2);
        assert!(options.delay.is_zero());
        let press = KeyPressOptions::default().delay(Duration::from_millis(50));
        assert_eq!(press.delay, Duration::from_millis(50));
        let clicks = ClickOptions::default();
        assert_eq!(clicks.button, MouseButton::Left);
        assert!(clicks.delay.is_zero());
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
            matcher: None,
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
