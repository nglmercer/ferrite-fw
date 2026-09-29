//! Isolated browser contexts (pages, cookies, viewport defaults).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::browser::Backend;
use crate::driver::{base64_encode, now_ms, BidiDriver, CdpDriver, ConsoleSink, Driver};
use crate::error::{E2eError, E2eResult};
use crate::page::{
    Cookie, Page, RouteAction, RouteHandler, RouteHandlerEntry, RouteInfo, RouteRule, Viewport,
};
use std::future::Future;

/// Options for a new browser context.
#[derive(Debug, Clone, Default)]
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

/// An isolated browser context; pages inside it share cookies and storage.
#[derive(Clone)]
pub struct BrowserContext {
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
        Self {
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
            permissions: Arc::new(Mutex::new(Vec::new())),
            geolocation: Arc::new(Mutex::new(None)),
            download_dir,
            tracing: Arc::new(Mutex::new(None)),
        }
    }

    /// Context id (`None` for the default context).
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Open a new page in this context.
    pub async fn new_page(&self) -> E2eResult<Page> {
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
    pub(crate) async fn finish_page(&self, driver: Driver, sink: ConsoleSink) -> E2eResult<Page> {
        let page = Page::new(
            driver,
            sink,
            self.slow_mo,
            self.base_url.clone(),
            Arc::downgrade(&self.pages),
            Arc::clone(&self.routes),
            Arc::clone(&self.handlers),
            Arc::clone(&self.tracing),
        );
        if let Some(viewport) = self.options.viewport {
            page.set_viewport(viewport).await?;
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
        if !self
            .routes
            .lock()
            .map(|routes| routes.is_empty())
            .unwrap_or(true)
        {
            page.restart_routing().await?;
        }
        if let Some(dir) = &self.download_dir {
            page.remember_download_dir(dir);
        }
        self.pages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(page.clone());
        Ok(page)
    }

    /// Open pages in this context.
    #[must_use]
    pub fn pages(&self) -> Vec<Page> {
        self.pages.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Read cookies visible to this context.
    pub async fn cookies(&self) -> E2eResult<Vec<Cookie>> {
        let page = self.new_page().await?;
        let cookies = page.cookies().await?;
        page.close().await?;
        Ok(cookies)
    }

    /// Clear all cookies in this context.
    pub async fn clear_cookies(&self) -> E2eResult<()> {
        let page = self.new_page().await?;
        page.clear_cookies().await?;
        page.close().await?;
        Ok(())
    }

    /// Set cookies for `url` (navigates a scratch page there; context-wide).
    pub async fn add_cookies(&self, cookies: &[Cookie], url: &str) -> E2eResult<()> {
        let page = self.new_page().await?;
        page.goto(url).await?;
        let result = page.add_cookies(cookies).await;
        page.close().await?;
        result
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
    /// rule) on the page wins (pattern match on `pattern`).
    pub async fn route_with_handler<F, Fut>(&self, pattern: &str, handler: F) -> E2eResult<()>
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

    /// Close the context and all its pages.
    pub async fn close(self) -> E2eResult<()> {
        if let Some(registry) = self.registry.upgrade() {
            registry
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .retain(|context| context.id != self.id);
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
