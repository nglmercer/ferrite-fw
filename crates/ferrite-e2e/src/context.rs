//! Isolated browser contexts (pages, cookies, viewport defaults).

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use serde_json::Value;

use crate::browser::Backend;
use crate::driver::{BidiDriver, CdpDriver, ConsoleSink, Driver};
use crate::error::{E2eError, E2eResult};
use crate::page::{Cookie, Page, RouteRule, Viewport};

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
    /// Granted permissions, applied to current and future pages.
    permissions: Arc<Mutex<Vec<String>>>,
    /// Geolocation override, applied to current and future pages.
    geolocation: Arc<Mutex<Option<(f64, f64)>>>,
}

impl BrowserContext {
    pub(crate) fn new(
        backend: Backend,
        id: Option<String>,
        options: ContextOptions,
        slow_mo: Duration,
        timeout: Duration,
        base_url: Option<String>,
        registry: Weak<Mutex<Vec<BrowserContext>>>,
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
            permissions: Arc::new(Mutex::new(Vec::new())),
            geolocation: Arc::new(Mutex::new(None)),
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

        let page = Page::new(
            driver,
            sink,
            self.slow_mo,
            self.base_url.clone(),
            Arc::downgrade(&self.pages),
            Arc::clone(&self.routes),
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

    /// Remove context rules with `pattern`; returns how many were removed.
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
        for page in self.pages() {
            page.restart_routing().await?;
        }
        Ok(removed)
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
