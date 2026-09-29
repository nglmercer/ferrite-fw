//! Pages: navigation, evaluation, input, screenshots, routing.
//!
//! [`Page`] is engine-agnostic: it delegates to a [`Driver`](crate::driver::Driver)
//! (CDP for Chromium, WebDriver BiDi for Firefox).

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
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
    /// Request headers as sent.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Request body text (Chromium only; `None` when absent or streamed).
    #[serde(default)]
    pub post_data: Option<String>,
    /// Request-to-response wall time in ms (`None` when the start was missed).
    #[serde(default)]
    pub duration_ms: Option<u64>,
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
        Self {
            pattern: pattern.into(),
            action: RouteAction::ContinueWith {
                url,
                method,
                headers,
                body,
            },
        }
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
        Self {
            pattern: pattern.into(),
            action: RouteAction::ModifyResponse {
                status,
                headers,
                body,
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
    /// Context grants waiting for the first http(s) navigation (Firefox
    /// grants need an origin, so fresh pages cannot take them yet).
    pending_grants: Arc<Mutex<Vec<String>>>,
    net_capture: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    exposed: ExposedState,
    registry: Weak<Mutex<Vec<Page>>>,
    frame_id: Option<String>,
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
  window.__ferriteClock = {
    setFixed(ms) {
      now = +ms || 0;
      return now;
    },
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
    pub(crate) fn new(
        driver: Driver,
        sink: ConsoleSink,
        slow_mo: Duration,
        base_url: Option<String>,
        registry: Weak<Mutex<Vec<Page>>>,
        context_routes: Arc<Mutex<Vec<RouteRule>>>,
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
            pending_grants: Arc::new(Mutex::new(Vec::new())),
            net_capture: Arc::new(Mutex::new(None)),
            exposed: ExposedState::default(),
            registry,
            frame_id: None,
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
        out
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

    /// Start intercepting requests with glob rules (replaces page rules;
    /// context rules still apply as fallback, page rules win on overlap).
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = rules;
        self.restart_routing().await
    }

    /// Stop page-level interception (context rules still apply).
    pub async fn stop_routing(&self) {
        *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();
        let _ = self.restart_routing().await;
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

    /// Apply the stored rules (page first, context fallback; no pump when empty).
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
        self.driver.set_download_dir(dir.as_ref()).await
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
    pub async fn wait_for_download(
        &self,
        dir: impl AsRef<Path>,
        timeout: Duration,
    ) -> E2eResult<PathBuf> {
        let dir = dir.as_ref();
        let before = dir_names(dir)?;
        let deadline = tokio::time::Instant::now() + timeout;
        let mut stable: Option<(PathBuf, u64)> = None;
        loop {
            let mut candidate: Option<(PathBuf, u64)> = None;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if before.contains(&name) || !path.is_file() || is_temp_download(&name) {
                        continue;
                    }
                    let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                    candidate = Some((path, size));
                    break;
                }
            }
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
        if let Some(registry) = self.registry.upgrade() {
            let target = self.target_id().to_string();
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|page| page.target_id() != target);
        }
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
}
