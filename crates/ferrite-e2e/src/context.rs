//! Isolated browser contexts (pages, cookies, viewport defaults).

use std::time::Duration;

use serde_json::Value;

use crate::browser::Backend;
use crate::driver::{BidiDriver, CdpDriver, ConsoleSink, Driver};
use crate::error::{E2eError, E2eResult};
use crate::page::{Cookie, Page, Viewport};

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
}

impl BrowserContext {
    pub(crate) fn new(
        backend: Backend,
        id: Option<String>,
        options: ContextOptions,
        slow_mo: Duration,
        timeout: Duration,
        base_url: Option<String>,
    ) -> Self {
        Self {
            backend,
            id,
            options,
            slow_mo,
            timeout,
            base_url,
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
                    CdpDriver::spawn(cdp.clone(), session, target_id, self.timeout, sink.clone())
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
                ))
            }
        };

        let page = Page::new(driver, sink, self.slow_mo, self.base_url.clone());
        if let Some(viewport) = self.options.viewport {
            page.set_viewport(viewport).await?;
        }
        if let Some(user_agent) = &self.options.user_agent {
            page.set_user_agent(user_agent).await?;
        }
        if self.options.ignore_https_errors {
            page.set_ignore_https_errors(true).await?;
        }
        Ok(page)
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

    /// Close the context and all its pages.
    pub async fn close(self) -> E2eResult<()> {
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
