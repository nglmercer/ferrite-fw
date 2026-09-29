//! Isolated browser contexts (pages, cookies, viewport defaults).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::browser::Backend;
use crate::driver::{base64_encode, now_ms, BidiDriver, CdpDriver, ConsoleSink, Driver};
use crate::error::{E2eError, E2eResult};
use crate::page::{
    Cookie, DeviceDescriptor, HttpCredentials, Page, RouteAction, RouteHandler, RouteHandlerEntry,
    RouteInfo, RouteRule, StorageState, Viewport,
};
use std::future::Future;

/// Service-worker policy for a context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceWorkerMode {
    /// Workers run normally.
    #[default]
    Allow,
    /// Workers are bypassed (Chromium; approximated — registration still
    /// succeeds but fetches skip workers entirely).
    Block,
}

/// Options for a new browser context.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextOptions {
    /// Default viewport applied to every page in the context.
    pub viewport: Option<Viewport>,
    /// Default user agent override (Chromium per-page; Firefox is
    /// launch-wide, so this errors on Firefox pages).
    pub user_agent: Option<String>,
    /// Proxy server (`host:port`, `http(s)://…`, `socks5://…`); must match
    /// the launch proxy (stock engines apply proxies browser-wide).
    pub proxy_server: Option<String>,
    /// Ignore HTTPS certificate errors.
    pub ignore_https_errors: bool,
    /// Locale override (`fr-FR`; Chromium only, fails loudly on Firefox).
    pub locale: Option<String>,
    /// Timezone override (`America/New_York`; Chromium only).
    pub timezone_id: Option<String>,
    /// Geolocation override (latitude, longitude).
    pub geolocation: Option<(f64, f64)>,
    /// Permissions granted to every page (`geolocation`, ...).
    pub permissions: Vec<String>,
    /// Start offline (Chromium only).
    pub offline: bool,
    /// HTTP credentials for basic/digest challenges (Chromium only).
    pub http_credentials: Option<HttpCredentials>,
    /// Extra HTTP headers for every request (Chromium only).
    pub extra_http_headers: Vec<(String, String)>,
    /// Device pixel ratio (Chromium only; needs `viewport` for full effect).
    pub device_scale_factor: Option<f64>,
    /// Mobile viewport behavior (Chromium only).
    pub is_mobile: bool,
    /// Touch event support (Chromium only).
    pub has_touch: bool,
    /// Enable JavaScript (`None` = on; `Some(false)` is Chromium only).
    pub java_script_enabled: Option<bool>,
    /// Bypass Content-Security-Policy checks (Chromium only).
    pub bypass_csp: bool,
    /// Accept downloads (`false` denies browser-wide; Chromium only).
    pub accept_downloads: bool,
    /// Directory downloads land in (Chromium per-context; Firefox is
    /// launch-wide, so this errors on Firefox pages).
    pub downloads_path: Option<PathBuf>,
    /// Storage state file applied to the context (cookies at once,
    /// localStorage when its origin loads).
    pub storage_state: Option<PathBuf>,
    /// Service-worker policy (blocking is Chromium only).
    pub service_workers: ServiceWorkerMode,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            viewport: None,
            user_agent: None,
            proxy_server: None,
            ignore_https_errors: false,
            locale: None,
            timezone_id: None,
            geolocation: None,
            permissions: Vec::new(),
            offline: false,
            http_credentials: None,
            extra_http_headers: Vec::new(),
            device_scale_factor: None,
            is_mobile: false,
            has_touch: false,
            java_script_enabled: None,
            bypass_csp: false,
            accept_downloads: true,
            downloads_path: None,
            storage_state: None,
            service_workers: ServiceWorkerMode::Allow,
        }
    }
}

impl ContextOptions {
    /// Set the default viewport.
    #[must_use]
    pub fn viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport = Some(Viewport { width, height });
        self
    }

    /// Set the default user agent.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set the locale override.
    #[must_use]
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Set the timezone override.
    #[must_use]
    pub fn timezone_id(mut self, timezone_id: impl Into<String>) -> Self {
        self.timezone_id = Some(timezone_id.into());
        self
    }

    /// Set the geolocation override.
    #[must_use]
    pub fn geolocation(mut self, latitude: f64, longitude: f64) -> Self {
        self.geolocation = Some((latitude, longitude));
        self
    }

    /// Grant permissions to every page.
    #[must_use]
    pub fn permissions(mut self, permissions: &[&str]) -> Self {
        self.permissions = permissions.iter().map(ToString::to_string).collect();
        self
    }

    /// Start offline.
    #[must_use]
    pub fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// Set HTTP credentials for basic/digest challenges.
    #[must_use]
    pub fn http_credentials(mut self, credentials: HttpCredentials) -> Self {
        self.http_credentials = Some(credentials);
        self
    }

    /// Set extra HTTP headers for every request.
    #[must_use]
    pub fn extra_http_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.extra_http_headers = headers;
        self
    }

    /// Set the device pixel ratio.
    #[must_use]
    pub fn device_scale_factor(mut self, factor: f64) -> Self {
        self.device_scale_factor = Some(factor);
        self
    }

    /// Enable mobile viewport behavior.
    #[must_use]
    pub fn is_mobile(mut self, mobile: bool) -> Self {
        self.is_mobile = mobile;
        self
    }

    /// Enable touch event support.
    #[must_use]
    pub fn has_touch(mut self, touch: bool) -> Self {
        self.has_touch = touch;
        self
    }

    /// Enable or disable JavaScript.
    #[must_use]
    pub fn java_script_enabled(mut self, enabled: bool) -> Self {
        self.java_script_enabled = Some(enabled);
        self
    }

    /// Bypass Content-Security-Policy checks.
    #[must_use]
    pub fn bypass_csp(mut self, bypass: bool) -> Self {
        self.bypass_csp = bypass;
        self
    }

    /// Accept or deny downloads.
    #[must_use]
    pub fn accept_downloads(mut self, accept: bool) -> Self {
        self.accept_downloads = accept;
        self
    }

    /// Set the directory downloads land in.
    #[must_use]
    pub fn downloads_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.downloads_path = Some(path.into());
        self
    }

    /// Apply a storage state file to the context.
    #[must_use]
    pub fn storage_state(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage_state = Some(path.into());
        self
    }

    /// Set the service-worker policy.
    #[must_use]
    pub fn service_workers(mut self, mode: ServiceWorkerMode) -> Self {
        self.service_workers = mode;
        self
    }
}

/// Live context settings: option seeds plus later setter calls, applied to
/// current and future pages.
#[derive(Debug, Clone, Default)]
struct ContextLiveState {
    offline: bool,
    http_credentials: Option<HttpCredentials>,
    extra_http_headers: Vec<(String, String)>,
    locale: Option<String>,
    timezone_id: Option<String>,
    java_script_enabled: Option<bool>,
    bypass_csp: bool,
    downloads_allowed: bool,
    downloads_path: Option<PathBuf>,
    service_workers: ServiceWorkerMode,
    action_timeout: Option<Duration>,
    navigation_timeout: Option<Duration>,
    expect_timeout: Option<Duration>,
    init_scripts: Vec<String>,
    storage: Option<StorageState>,
}

impl ContextLiveState {
    fn seed(options: &ContextOptions) -> Self {
        Self {
            offline: options.offline,
            http_credentials: options.http_credentials.clone(),
            extra_http_headers: options.extra_http_headers.clone(),
            locale: options.locale.clone(),
            timezone_id: options.timezone_id.clone(),
            java_script_enabled: options.java_script_enabled,
            bypass_csp: options.bypass_csp,
            downloads_allowed: options.accept_downloads,
            downloads_path: options.downloads_path.clone(),
            service_workers: options.service_workers,
            action_timeout: None,
            navigation_timeout: None,
            expect_timeout: None,
            init_scripts: Vec::new(),
            storage: None,
        }
    }
}

/// Options for [`BrowserContext::start_tracing`].
#[derive(Debug, Clone, Default)]
pub struct TracingOptions {
    /// Capture a PNG screenshot after every [`Page::step`].
    pub screenshots: bool,
}

impl TracingOptions {
    /// Capture a PNG screenshot after every [`Page::step`].
    #[must_use]
    pub fn screenshots(mut self, enabled: bool) -> Self {
        self.screenshots = enabled;
        self
    }
}

/// One tracing screenshot (PNG bytes; base64-encoded at export).
#[derive(Debug, Clone)]
pub(crate) struct TraceScreenshot {
    /// Milliseconds since the Unix epoch.
    pub(crate) ts_ms: u64,
    /// Step name the screenshot was taken after.
    pub(crate) step: String,
    /// PNG bytes.
    pub(crate) png: Vec<u8>,
}

/// Active tracing session, shared with every page of the context.
#[derive(Debug)]
pub(crate) struct TracingState {
    /// Capture screenshots after steps.
    screenshots: bool,
    /// Session start, milliseconds since the Unix epoch.
    started_ms: u64,
    /// Captured screenshots (oldest first).
    shots: Vec<TraceScreenshot>,
}

impl TracingState {
    pub(crate) fn screenshots(&self) -> bool {
        self.screenshots
    }

    pub(crate) fn push(&mut self, shot: TraceScreenshot) {
        self.shots.push(shot);
    }
}

/// Kinds of events emitted by a context across all its pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextEventKind {
    Page,
    Closed,
    Console,
    PageError,
    Dialog,
    Request,
    Response,
    Download,
    Popup,
    PageClose,
    WebSocket,
}
/// A context event with its source page identity. New pages and popups are
/// adopted into the context before their events are emitted.
#[derive(Debug, Clone)]
pub enum ContextEvent {
    Page(Box<Page>),
    Closed,
    PageEvent {
        page_id: String,
        event: crate::PageEvent,
    },
}
impl ContextEvent {
    pub fn page_id(&self) -> Option<&str> {
        match self {
            Self::Page(page) => Some(page.target_id()),
            Self::PageEvent { page_id, .. } => Some(page_id),
            Self::Closed => None,
        }
    }
    pub fn kind(&self) -> ContextEventKind {
        use crate::{PageEvent, PageEventKind};
        match self {
            Self::Page(_) => ContextEventKind::Page,
            Self::Closed => ContextEventKind::Closed,
            Self::PageEvent {
                event: PageEvent::Console(message),
                ..
            } if message.kind == "exception" => ContextEventKind::PageError,
            Self::PageEvent { event, .. } => match event.kind() {
                PageEventKind::Console => ContextEventKind::Console,
                PageEventKind::Dialog => ContextEventKind::Dialog,
                PageEventKind::Request => ContextEventKind::Request,
                PageEventKind::Response => ContextEventKind::Response,
                PageEventKind::Download => ContextEventKind::Download,
                PageEventKind::Popup => ContextEventKind::Popup,
                PageEventKind::Closed => ContextEventKind::PageClose,
                PageEventKind::WebSocket => ContextEventKind::WebSocket,
            },
        }
    }
}

/// An isolated browser context; pages inside it share cookies and storage.
#[derive(Clone)]
pub struct BrowserContext {
    closed: Arc<std::sync::atomic::AtomicBool>,
    cancellation: crate::CancellationToken,
    events: tokio::sync::broadcast::Sender<ContextEvent>,
    backend: Backend,
    id: Option<String>,
    options: ContextOptions,
    slow_mo: Duration,
    timeout: Duration,
    base_url: Option<String>,
    pages: Arc<Mutex<Vec<Page>>>,
    registry: Weak<Mutex<Vec<BrowserContext>>>,
    /// Routing rules shared with every page (page rules win on overlap).
    routes: Arc<Mutex<Vec<RouteRule>>>,
    /// Route handlers shared with every page.
    handlers: Arc<Mutex<Vec<RouteHandlerEntry>>>,
    /// Granted permissions, applied to current and future pages.
    permissions: Arc<Mutex<Vec<String>>>,
    /// Geolocation override, applied to current and future pages.
    geolocation: Arc<Mutex<Option<(f64, f64)>>>,
    /// Live settings (option seeds + setter calls), applied to current
    /// and future pages.
    live: Arc<Mutex<ContextLiveState>>,
    /// Launch-wide download dir (Firefox downloads land here).
    download_dir: Option<PathBuf>,
    /// Active tracing session (shared with every page).
    tracing: Arc<Mutex<Option<TracingState>>>,
}

impl BrowserContext {
    #[allow(clippy::too_many_arguments)] // Internal constructor; called from two launch sites.
    pub(crate) fn new(
        backend: Backend,
        id: Option<String>,
        options: ContextOptions,
        slow_mo: Duration,
        timeout: Duration,
        base_url: Option<String>,
        registry: Weak<Mutex<Vec<BrowserContext>>>,
        download_dir: Option<PathBuf>,
    ) -> Self {
        let live = ContextLiveState::seed(&options);
        let permissions = options.permissions.clone();
        let geolocation = options.geolocation;
        Self {
            closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cancellation: crate::CancellationToken::new(),
            events: tokio::sync::broadcast::channel(512).0,
            backend,
            id,
            options,
            slow_mo,
            timeout,
            base_url,
            pages: Arc::new(Mutex::new(Vec::new())),
            registry,
            routes: Arc::new(Mutex::new(Vec::new())),
            handlers: Arc::new(Mutex::new(Vec::new())),
            permissions: Arc::new(Mutex::new(permissions)),
            geolocation: Arc::new(Mutex::new(geolocation)),
            live: Arc::new(Mutex::new(live)),
            download_dir,
            tracing: Arc::new(Mutex::new(None)),
        }
    }

    /// Subscribe before triggering an action to observe context and page events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ContextEvent> {
        self.events.subscribe()
    }
    pub async fn wait_for_event(
        &self,
        kind: ContextEventKind,
        timeout: Duration,
    ) -> E2eResult<ContextEvent> {
        self.wait_for_event_with_options(
            kind,
            crate::OperationOptions {
                timeout: Some(timeout),
                ..Default::default()
            },
        )
        .await
    }
    pub async fn wait_for_event_with_options(
        &self,
        kind: ContextEventKind,
        options: crate::OperationOptions,
    ) -> E2eResult<ContextEvent> {
        if kind == ContextEventKind::Closed && self.is_closed() {
            return Ok(ContextEvent::Closed);
        }
        if kind == ContextEventKind::WebSocket && matches!(self.backend, Backend::Bidi { .. }) {
            return Err(E2eError::Config(
                "websocket events are not supported on Firefox".into(),
            ));
        }
        let mut events = self.subscribe();
        let wait = crate::operation::Deadline::new(options.timeout.unwrap_or(self.timeout)).run(
            format!("wait for context {kind:?}"),
            async {
                loop {
                    match events.recv().await {
                        Ok(event) if event.kind() == kind => return Ok(event),
                        Ok(ContextEvent::Closed) => {
                            return Err(E2eError::Cancelled("browser context closed".into()))
                        }
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return Err(E2eError::Disconnected(
                                "context event stream closed".into(),
                            ))
                        }
                    }
                }
            },
        );
        let wait = async {
            if kind == ContextEventKind::Closed {
                wait.await
            } else {
                self.cancellation.run(wait).await
            }
        };
        match options.cancellation {
            Some(token) => token.run(wait).await,
            None => wait.await,
        }
    }
    /// Context id (`None` for the default context).
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Open a new page in this context.
    pub async fn new_page(&self) -> E2eResult<Page> {
        if self.is_closed() {
            return Err(E2eError::Config("browser context is closed".into()));
        }
        let sink = ConsoleSink::new();
        let driver = match &self.backend {
            Backend::Cdp(cdp) => {
                let mut params = serde_json::json!({ "url": "about:blank" });
                if let Some(id) = &self.id {
                    params["browserContextId"] = Value::String(id.clone());
                }
                let target = cdp
                    .call(None, "Target.createTarget", params, self.timeout)
                    .await?;
                let target_id = target
                    .get("targetId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| E2eError::Launch("no targetId".to_string()))?
                    .to_string();
                let attached = cdp
                    .call(
                        None,
                        "Target.attachToTarget",
                        serde_json::json!({ "targetId": target_id, "flatten": true }),
                        self.timeout,
                    )
                    .await?;
                let session = attached
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .ok_or_else(|| E2eError::Launch("no sessionId".to_string()))?
                    .to_string();
                Driver::Cdp(
                    CdpDriver::spawn(
                        cdp.clone(),
                        session,
                        target_id,
                        self.timeout,
                        sink.clone(),
                        self.id.clone(),
                    )
                    .await?,
                )
            }
            Backend::Bidi {
                conn,
                insecure_certs,
                ..
            } => {
                let mut params = serde_json::json!({ "type": "tab" });
                if let Some(id) = &self.id {
                    params["userContext"] = Value::String(id.clone());
                }
                let created = conn
                    .call("browsingContext.create", params, self.timeout)
                    .await?;
                let context = created
                    .get("context")
                    .and_then(Value::as_str)
                    .ok_or_else(|| E2eError::Launch("no BiDi context".to_string()))?
                    .to_string();
                Driver::Bidi(BidiDriver::spawn(
                    conn.clone(),
                    context,
                    self.timeout,
                    *insecure_certs,
                    sink.clone(),
                    self.id.clone(),
                ))
            }
        };

        self.finish_page(driver, sink).await
    }

    /// Wrap a live driver as a context page (shared by `new_page` and popup
    /// adoption): options, stored rules/grants, registration.
    pub(crate) async fn finish_page(
        &self,
        mut driver: Driver,
        sink: ConsoleSink,
    ) -> E2eResult<Page> {
        driver.bind_context_cancellation(self.cancellation.clone());
        let target_id = driver.target_id().to_owned();
        sink.forward_context(&self.events, &target_id);
        let mut page = Page::new(
            driver,
            sink,
            self.slow_mo,
            self.base_url.clone(),
            Arc::downgrade(&self.pages),
            Arc::clone(&self.routes),
            Arc::clone(&self.handlers),
            Arc::clone(&self.tracing),
        );
        page.context_registry = self.registry.clone();
        page.context_id = self.id.clone();
        if let Some(viewport) = self.options.viewport {
            page.set_viewport(viewport).await?;
        }
        if self.options.device_scale_factor.is_some()
            || self.options.is_mobile
            || self.options.has_touch
        {
            // Playwright combines device flags with the viewport (default
            // 1280x720 when unset).
            let viewport = self.options.viewport.unwrap_or(Viewport {
                width: 1280,
                height: 720,
            });
            page.emulate_device(DeviceDescriptor {
                viewport,
                device_scale_factor: self.options.device_scale_factor.unwrap_or(1.0),
                mobile: self.options.is_mobile,
                has_touch: self.options.has_touch,
            })
            .await?;
        }
        let permissions = self
            .permissions
            .lock()
            .map(|grants| grants.clone())
            .unwrap_or_default();
        if !permissions.is_empty() {
            if matches!(self.backend, Backend::Cdp(_)) {
                let names: Vec<&str> = permissions.iter().map(String::as_str).collect();
                page.grant_permissions(&names).await?;
            } else {
                // Fresh pages sit on about:blank, which Firefox cannot grant
                // to; the grants land on the first http(s) navigation.
                page.defer_grants(permissions);
            }
        }
        if let Some((latitude, longitude)) = self.geolocation.lock().map(|geo| *geo).unwrap_or(None)
        {
            page.set_geolocation(latitude, longitude).await?;
        }
        if let Some(user_agent) = &self.options.user_agent {
            page.set_user_agent(user_agent).await?;
        }
        if self.options.ignore_https_errors {
            page.set_ignore_https_errors(true).await?;
        }
        self.apply_live_state(&page).await?;
        page.sync_protocol_timeout();
        if !self
            .routes
            .lock()
            .map(|routes| routes.is_empty())
            .unwrap_or(true)
        {
            page.restart_routing().await?;
        }
        let (downloads_path, downloads_allowed) = {
            let live = self.live.lock().unwrap_or_else(|e| e.into_inner());
            (live.downloads_path.clone(), live.downloads_allowed)
        };
        if let Some(dir) = downloads_path.as_ref().or(self.download_dir.as_ref()) {
            if downloads_path.is_none()
                && downloads_allowed
                && matches!(self.backend, Backend::Cdp(_))
            {
                page.set_download_dir(dir).await?;
            } else {
                page.remember_download_dir(dir);
            }
        }
        self.pages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(page.clone());
        let _ = self.events.send(ContextEvent::Page(Box::new(page.clone())));
        Ok(page)
    }

    /// Open pages in this context.
    #[must_use]
    pub fn pages(&self) -> Vec<Page> {
        self.pages.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Set default action timeouts on existing and future pages.
    pub fn set_default_timeout(&self, timeout: Duration) {
        for mut page in self.pages() {
            page.set_timeout(timeout);
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .action_timeout = Some(timeout);
    }

    /// Set default navigation timeouts on existing and future pages.
    pub fn set_default_navigation_timeout(&self, timeout: Duration) {
        for page in self.pages() {
            page.set_navigation_timeout(timeout);
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .navigation_timeout = Some(timeout);
    }

    /// Set default assertion timeouts on existing and future pages.
    pub fn set_expect_timeout(&self, timeout: Duration) {
        for page in self.pages() {
            page.set_expect_timeout(timeout);
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .expect_timeout = Some(timeout);
    }

    /// HTTP request client sharing this context's cookies.
    pub fn request(&self) -> crate::ApiClient {
        crate::ApiClient::with_options(self.api_options())
            .expect("validated browser context HTTP settings")
            .with_context(self.clone())
    }

    pub fn api_options(&self) -> crate::ApiClientOptions {
        let live = self.live.lock().unwrap_or_else(|e| e.into_inner());
        crate::ApiClientOptions {
            base_url: self.base_url.clone(),
            headers: live.extra_http_headers.clone(),
            credentials: live.http_credentials.clone(),
            timeout: live.action_timeout.unwrap_or(self.timeout),
            ignore_https_errors: self.options.ignore_https_errors,
            proxy: self.options.proxy_server.clone().map(|proxy| {
                if proxy.contains("://") {
                    proxy
                } else {
                    format!("http://{proxy}")
                }
            }),
            ..crate::ApiClientOptions::default()
        }
    }

    pub fn cancellation_token(&self) -> crate::CancellationToken {
        self.cancellation.clone()
    }

    /// Read cookies visible to this context.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        self.cancellation
            .run(crate::context_cookies::cookies(
                &self.backend,
                self.id.as_deref(),
                self.timeout,
            ))
            .await
    }

    /// Clear all cookies in this context.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        self.cancellation
            .run(crate::context_cookies::clear_cookies(
                &self.backend,
                self.id.as_deref(),
                self.timeout,
            ))
            .await
    }

    /// Set cookies for `url` without opening or navigating a page.
    pub async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        self.cancellation
            .run(crate::context_cookies::add_cookies(
                &self.backend,
                self.id.as_deref(),
                self.timeout,
                cookies,
                url,
            ))
            .await
    }

    /// Route matching requests on every current and future page.
    ///
    /// Page-level [`Page::route`] rules win on overlap. When a current page
    /// rejects the rules (e.g. a Firefox-unsupported override), the stored
    /// set is left unchanged and the error surfaces.
    pub async fn route(&self, rules: Vec<RouteRule>) -> E2eResult<()> {
        let old = self
            .routes
            .lock()
            .map(|mut stored| std::mem::replace(&mut *stored, rules))
            .unwrap_or_default();
        let mut failed = None;
        for page in self.pages() {
            if let Err(error) = page.restart_routing().await {
                failed = Some(error);
                break;
            }
        }
        if let Some(error) = failed {
            *self.routes.lock().unwrap_or_else(|e| e.into_inner()) = old;
            for page in self.pages() {
                let _ = page.restart_routing().await;
            }
            return Err(error);
        }
        Ok(())
    }

    /// Register a context-level route handler. Handlers run before
    /// declarative rules and page handlers; the first matching handler (or
    /// rule) on the page wins (pattern match on `pattern`), unless it
    /// returns [`RouteAction::Fallback`].
    pub async fn route_with_handler<F, Fut>(&self, pattern: &str, handler: F) -> E2eResult<()>
    where
        F: Fn(RouteInfo) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<RouteAction>> + Send + 'static,
    {
        self.route_entry(pattern, handler, None).await
    }

    /// [`BrowserContext::route_with_handler`] limited to `n` matches.
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

    /// Register a context handler entry with an optional match limit.
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
                pattern: pattern.to_owned(),
                handler,
                times,
                hits: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            });
        let mut failed = None;
        for page in self.pages() {
            if let Err(error) = page.restart_routing().await {
                failed = Some(error);
                break;
            }
        }
        // Roll back the stored handler on failure to keep pages in sync.
        if let Some(error) = failed {
            self.handlers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop();
            return Err(error);
        }
        Ok(())
    }

    /// Start tracing this context (Playwright `context.tracing.start()`).
    ///
    /// Actions, console messages and captured requests accumulate per page;
    /// with `screenshots` every [`Page::step`] also captures a PNG.
    /// Restarting discards the previous session.
    pub fn start_tracing(&self, options: TracingOptions) {
        *self.tracing.lock().unwrap_or_else(|e| e.into_inner()) = Some(TracingState {
            screenshots: options.screenshots,
            started_ms: now_ms(),
            shots: Vec::new(),
        });
    }

    /// Stop tracing and write the self-contained JSON trace to `path`
    /// (Playwright `context.tracing.stop({ path })`).
    ///
    /// Errors when tracing was never started. Screenshots embed as base64
    /// PNGs; request bodies are never included.
    pub async fn stop_tracing(&self, path: impl AsRef<std::path::Path>) -> E2eResult<()> {
        let state = self
            .tracing
            .lock()
            .map(|mut tracing| tracing.take())
            .unwrap_or(None)
            .ok_or_else(|| E2eError::Config("tracing is not started".to_string()))?;
        let mut pages = Vec::new();
        for page in self.pages() {
            pages.push(serde_json::json!({
                "target_id": page.target_id(),
                "url": page.url().await.unwrap_or_default(),
                "actions": page.trace(),
                "console": page.console_messages(),
                "requests": page.requests(),
            }));
        }
        let shots: Vec<serde_json::Value> = state
            .shots
            .iter()
            .map(|shot| {
                serde_json::json!({
                    "ts_ms": shot.ts_ms,
                    "step": shot.step,
                    "png_base64": base64_encode(&shot.png),
                })
            })
            .collect();
        let trace = serde_json::json!({
            "tool": "ferrite",
            "version": env!("CARGO_PKG_VERSION"),
            "started_ms": state.started_ms,
            "screenshots_enabled": state.screenshots,
            "pages": pages,
            "screenshots": shots,
        });
        std::fs::write(path.as_ref(), serde_json::to_string_pretty(&trace)?)?;
        Ok(())
    }

    /// Remove context rules/handlers with `pattern`; returns how many were removed.
    pub async fn unroute(&self, pattern: &str) -> E2eResult<usize> {
        let removed_rules = self
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
        for page in self.pages() {
            page.restart_routing().await?;
        }
        Ok(removed_rules + removed_handlers)
    }

    /// Remove all context rules and handlers; returns how many were removed.
    pub async fn unroute_all(&self) -> E2eResult<usize> {
        let removed_rules = self
            .routes
            .lock()
            .map(|mut routes| std::mem::take(&mut *routes).len())
            .unwrap_or(0);
        let removed_handlers = self
            .handlers
            .lock()
            .map(|mut handlers| std::mem::take(&mut *handlers).len())
            .unwrap_or(0);
        for page in self.pages() {
            page.restart_routing().await?;
        }
        Ok(removed_rules + removed_handlers)
    }

    /// Replay responses from a HAR 1.2 file on every current and future page.
    ///
    /// Entries match on exact method + URL; misses fall through. Returns how
    /// many entries were loaded. Remove with `unroute("**")`.
    pub async fn route_from_har(
        &self,
        path: impl AsRef<std::path::Path>,
        options: crate::page::RouteFromHarOptions,
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

    /// Grant permissions on every current and future page (replaces the set).
    ///
    /// On Firefox, pages that have not reached an http(s) URL yet take the
    /// grants on their first navigation (BiDi grants need an origin).
    pub async fn grant_permissions(&self, permissions: &[&str]) -> E2eResult<()> {
        let cdp = matches!(self.backend, Backend::Cdp(_));
        for page in self.pages() {
            let http = page
                .url()
                .await
                .map(|url| url.starts_with("http"))
                .unwrap_or(false);
            if cdp || http {
                page.grant_permissions(permissions).await?;
            } else {
                page.defer_grants(permissions.iter().map(|name| name.to_string()).collect());
            }
        }
        *self.permissions.lock().unwrap_or_else(|e| e.into_inner()) =
            permissions.iter().map(|name| name.to_string()).collect();
        Ok(())
    }

    /// Override geolocation on every current and future page.
    pub async fn set_geolocation(&self, latitude: f64, longitude: f64) -> E2eResult<()> {
        for page in self.pages() {
            page.set_geolocation(latitude, longitude).await?;
        }
        *self.geolocation.lock().unwrap_or_else(|e| e.into_inner()) = Some((latitude, longitude));
        Ok(())
    }

    /// Reset permissions on every current page and forget stored grants.
    pub async fn clear_permissions(&self) -> E2eResult<()> {
        for page in self.pages() {
            page.clear_permissions().await?;
        }
        *self.permissions.lock().unwrap_or_else(|e| e.into_inner()) = Vec::new();
        Ok(())
    }

    /// Clear the geolocation override on every current page and forget it.
    pub async fn clear_geolocation(&self) -> E2eResult<()> {
        for page in self.pages() {
            page.clear_geolocation().await?;
        }
        *self.geolocation.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }

    /// Apply the live settings snapshot to one page (new pages).
    async fn apply_live_state(&self, page: &Page) -> E2eResult<()> {
        let live = self
            .live
            .lock()
            .map(|live| live.clone())
            .unwrap_or_default();
        let mut page = page.clone();
        if let Some(timeout) = live.action_timeout {
            page.set_timeout(timeout);
        }
        if let Some(timeout) = live.navigation_timeout {
            page.set_navigation_timeout(timeout);
        }
        if let Some(timeout) = live.expect_timeout {
            page.set_expect_timeout(timeout);
        }
        if live.offline {
            page.set_offline(true).await?;
        }
        if let Some(credentials) = &live.http_credentials {
            page.set_http_credentials(Some(&credentials.username), Some(&credentials.password))
                .await?;
        }
        if !live.extra_http_headers.is_empty() {
            let headers: Vec<(&str, &str)> = live
                .extra_http_headers
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            page.set_extra_http_headers(&headers).await?;
        }
        if let Some(locale) = &live.locale {
            page.set_locale(locale).await?;
        }
        if let Some(timezone_id) = &live.timezone_id {
            page.set_timezone(timezone_id).await?;
        }
        if let Some(enabled) = live.java_script_enabled {
            page.set_java_script_enabled(enabled).await?;
        }
        if live.bypass_csp {
            page.set_bypass_csp(true).await?;
        }
        if !live.downloads_allowed {
            page.set_downloads_allowed(false).await?;
        }
        if let Some(dir) = &live.downloads_path {
            page.set_download_dir(dir).await?;
        }
        if live.service_workers == ServiceWorkerMode::Block {
            page.set_service_workers_blocked(true).await?;
        }
        for source in &live.init_scripts {
            page.add_init_script(source).await?;
        }
        if let Some(state) = &live.storage {
            page.apply_storage_state(state).await?;
        }
        Ok(())
    }

    /// Emulate offline mode on every current and future page (Chromium only).
    pub async fn set_offline(&self, offline: bool) -> E2eResult<()> {
        for page in self.pages() {
            page.set_offline(offline).await?;
        }
        self.live.lock().unwrap_or_else(|e| e.into_inner()).offline = offline;
        Ok(())
    }

    /// Send HTTP credentials with subsequent requests on every current and
    /// future page (Chromium only; basic preempted, digest challenged).
    pub async fn set_http_credentials(
        &self,
        username: Option<&str>,
        password: Option<&str>,
    ) -> E2eResult<()> {
        let stored = match (username, password) {
            (Some(user), Some(pass)) => Some(HttpCredentials::new(user, pass)),
            (None, None) => None,
            _ => {
                return Err(E2eError::Config(
                    "set_http_credentials needs both username and password (or neither to clear)"
                        .to_string(),
                ));
            }
        };
        for page in self.pages() {
            page.set_http_credentials(username, password).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .http_credentials = stored;
        Ok(())
    }

    /// Set extra HTTP headers on every current and future page (Chromium only).
    pub async fn set_extra_http_headers(&self, headers: &[(&str, &str)]) -> E2eResult<()> {
        for page in self.pages() {
            page.set_extra_http_headers(headers).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extra_http_headers = headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        Ok(())
    }

    /// Override the locale on every current and future page (Chromium only).
    pub async fn set_locale(&self, locale: &str) -> E2eResult<()> {
        for page in self.pages() {
            page.set_locale(locale).await?;
        }
        self.live.lock().unwrap_or_else(|e| e.into_inner()).locale = Some(locale.to_string());
        Ok(())
    }

    /// Override the timezone on every current and future page (Chromium only).
    pub async fn set_timezone(&self, timezone_id: &str) -> E2eResult<()> {
        for page in self.pages() {
            page.set_timezone(timezone_id).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .timezone_id = Some(timezone_id.to_string());
        Ok(())
    }

    /// Enable or disable JavaScript on every current and future page
    /// (disabling is Chromium only).
    pub async fn set_java_script_enabled(&self, enabled: bool) -> E2eResult<()> {
        for page in self.pages() {
            page.set_java_script_enabled(enabled).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .java_script_enabled = Some(enabled);
        Ok(())
    }

    /// Bypass Content-Security-Policy checks on every current and future
    /// page (Chromium only).
    pub async fn set_bypass_csp(&self, bypass: bool) -> E2eResult<()> {
        for page in self.pages() {
            page.set_bypass_csp(bypass).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bypass_csp = bypass;
        Ok(())
    }

    /// Allow or deny downloads on every current and future page
    /// (Chromium only; denying is browser-wide).
    pub async fn set_downloads_allowed(&self, allowed: bool) -> E2eResult<()> {
        for page in self.pages() {
            page.set_downloads_allowed(allowed).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .downloads_allowed = allowed;
        Ok(())
    }

    /// Set the service-worker policy on every current and future page
    /// (blocking is Chromium only).
    pub async fn set_service_workers(&self, mode: ServiceWorkerMode) -> E2eResult<()> {
        for page in self.pages() {
            page.set_service_workers_blocked(mode == ServiceWorkerMode::Block)
                .await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .service_workers = mode;
        Ok(())
    }

    /// Run `source` before page scripts in every future document, on every
    /// current and future page.
    pub async fn add_init_script(&self, source: &str) -> E2eResult<()> {
        for page in self.pages() {
            page.add_init_script(source).await?;
        }
        self.live
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .init_scripts
            .push(source.to_string());
        Ok(())
    }

    /// Apply a storage state file to the context: cookies land at once
    /// (context-wide), localStorage lands on pages already on the saved
    /// origin and follows future navigations there.
    pub async fn load_storage_state(&self, path: impl AsRef<std::path::Path>) -> E2eResult<()> {
        let raw = std::fs::read_to_string(path.as_ref()).map_err(|error| {
            E2eError::Config(format!(
                "cannot read storage state {}: {error}",
                path.as_ref().display()
            ))
        })?;
        let state: StorageState = serde_json::from_str(&raw).map_err(|error| {
            E2eError::Config(format!(
                "cannot parse storage state {}: {error}",
                path.as_ref().display()
            ))
        })?;
        self.apply_storage_state(&state).await
    }

    /// Restore cookies immediately and localStorage before future application scripts.
    pub async fn apply_storage_state(&self, state: &StorageState) -> E2eResult<()> {
        self.cancellation.check()?;
        let state = state.clone();
        for page in self.pages() {
            page.apply_storage_state(&state).await?;
        }
        self.live.lock().unwrap_or_else(|e| e.into_inner()).storage = Some(state.clone());
        // Cookie import is context-wide and does not create observable scratch pages.
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
                .or_else(|| {
                    state
                        .all_origins()
                        .first()
                        .map(|origin| origin.origin.clone())
                })
                .ok_or_else(|| {
                    E2eError::Config("storage-state cookie needs a domain or origin".into())
                })?;
            self.add_cookies(std::slice::from_ref(cookie), &url).await?;
        }
        Ok(())
    }

    /// Capture cookies and localStorage for all origins with live pages.
    /// Origins visited and subsequently closed are not inventoried; IndexedDB is deferred.
    pub async fn storage_state(&self) -> E2eResult<StorageState> {
        let mut origins = std::collections::BTreeMap::new();
        for page in self.pages() {
            let state = page.storage_state().await?;
            for origin in state.all_origins() {
                if origin.origin != "null" {
                    origins.insert(origin.origin.clone(), origin);
                }
            }
        }
        Ok(StorageState {
            origin: String::new(),
            local_storage: Default::default(),
            origins: origins.into_values().collect(),
            cookies: self.cookies().await?,
        })
    }

    /// Save context-wide state in Playwright's cookies/origins JSON format.
    pub async fn save_storage_state(&self, path: impl AsRef<std::path::Path>) -> E2eResult<()> {
        std::fs::write(
            path,
            serde_json::to_string_pretty(&self.storage_state().await?)?,
        )?;
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Close the context and all its pages.
    pub async fn close(self) -> E2eResult<()> {
        if self.closed.swap(true, std::sync::atomic::Ordering::AcqRel) {
            return Ok(());
        }
        let _ = self.events.send(ContextEvent::Closed);
        self.cancellation
            .cancel_with_reason("browser context closed");
        if let Some(registry) = self.registry.upgrade() {
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|context| context.id != self.id);
        }
        for page in self.pages() {
            page.close_target().await.ok();
        }
        self.pages.lock().unwrap_or_else(|e| e.into_inner()).clear();
        match (&self.backend, self.id) {
            (Backend::Cdp(cdp), Some(id)) => {
                let _ = cdp
                    .call(
                        None,
                        "Target.disposeBrowserContext",
                        serde_json::json!({ "browserContextId": id }),
                        self.timeout,
                    )
                    .await;
            }
            (Backend::Bidi { conn, .. }, Some(id)) => {
                let _ = conn
                    .call(
                        "browser.removeUserContext",
                        serde_json::json!({ "userContext": id }),
                        self.timeout,
                    )
                    .await;
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_accepts_downloads() {
        let options = ContextOptions::default();
        assert!(options.accept_downloads);
        assert!(!options.offline);
        assert!(!options.is_mobile);
        assert!(!options.has_touch);
        assert!(!options.bypass_csp);
        assert_eq!(options.service_workers, ServiceWorkerMode::Allow);
        assert!(options.storage_state.is_none());
        assert!(options.java_script_enabled.is_none());
    }

    #[test]
    fn options_builders_chain() {
        let options = ContextOptions::default()
            .viewport(800, 600)
            .locale("fr-FR")
            .timezone_id("America/New_York")
            .geolocation(48.8, 2.3)
            .permissions(&["geolocation"])
            .offline(true)
            .http_credentials(HttpCredentials::new("ada", "s3cret"))
            .extra_http_headers(vec![("X-A".to_string(), "1".to_string())])
            .device_scale_factor(2.0)
            .is_mobile(true)
            .has_touch(true)
            .java_script_enabled(false)
            .bypass_csp(true)
            .accept_downloads(false)
            .downloads_path("/tmp/dl")
            .storage_state("/tmp/state.json")
            .service_workers(ServiceWorkerMode::Block);
        assert_eq!(
            options.viewport,
            Some(Viewport {
                width: 800,
                height: 600
            })
        );
        assert_eq!(options.locale.as_deref(), Some("fr-FR"));
        assert_eq!(options.timezone_id.as_deref(), Some("America/New_York"));
        assert_eq!(options.geolocation, Some((48.8, 2.3)));
        assert_eq!(options.permissions, vec!["geolocation".to_string()]);
        assert!(options.offline);
        assert_eq!(
            options.http_credentials,
            Some(HttpCredentials::new("ada", "s3cret"))
        );
        assert_eq!(
            options.extra_http_headers,
            vec![("X-A".to_string(), "1".to_string())]
        );
        assert_eq!(options.device_scale_factor, Some(2.0));
        assert!(options.is_mobile);
        assert!(options.has_touch);
        assert_eq!(options.java_script_enabled, Some(false));
        assert!(options.bypass_csp);
        assert!(!options.accept_downloads);
        assert_eq!(options.downloads_path, Some(PathBuf::from("/tmp/dl")));
        assert_eq!(
            options.storage_state,
            Some(PathBuf::from("/tmp/state.json"))
        );
        assert_eq!(options.service_workers, ServiceWorkerMode::Block);
    }

    #[test]
    fn live_state_seeds_from_options() {
        let options = ContextOptions::default()
            .offline(true)
            .locale("de-DE")
            .accept_downloads(false);
        let live = ContextLiveState::seed(&options);
        assert!(live.offline);
        assert_eq!(live.locale.as_deref(), Some("de-DE"));
        assert!(!live.downloads_allowed);
        assert!(live.init_scripts.is_empty());
        assert!(live.storage.is_none());
    }
}
