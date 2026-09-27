//! SSR router.

use crate::request::*;
use ferrite_core::Result;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

/// Minimal route table with `:param` segments (§47 framework API).
///
/// Higher-level file-system routing generates entries for this table
/// (`src/routes/users/[id].rs` → `/users/:id`).
#[derive(Clone, Default)]
pub struct SsrRouter {
    /// Routes in registration order.
    routes: Vec<SsrRoute>,
}

/// Route handler: request + context + params.
pub(crate) type SsrHandler = Arc<
    dyn Fn(SsrHttpRequest, SsrContext, HashMap<String, String>) -> RpcBoxFutureSsr + Send + Sync,
>;

/// One route: pattern + handler.
#[derive(Clone)]
pub(crate) struct SsrRoute {
    pattern: String,
    handler: SsrHandler,
}

pub(crate) type RpcBoxFutureSsr =
    Pin<Box<dyn std::future::Future<Output = Result<SsrResponse>> + Send>>;

impl SsrRouter {
    /// Empty router.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `pattern` (`/users/:id`) with a handler.
    pub fn route<F, Fut>(&mut self, pattern: impl Into<String>, handler: F) -> &mut Self
    where
        F: Fn(SsrHttpRequest, SsrContext, HashMap<String, String>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<SsrResponse>> + Send + 'static,
    {
        self.routes.push(SsrRoute {
            pattern: pattern.into(),
            handler: Arc::new(move |request, context, params| {
                Box::pin(handler(request, context, params)) as RpcBoxFutureSsr
            }),
        });
        self
    }

    /// Match `path` against the table (first match wins).
    #[must_use]
    pub fn find(&self, path: &str) -> Option<(usize, HashMap<String, String>)> {
        self.routes.iter().enumerate().find_map(|(index, route)| {
            match_route(&route.pattern, path).map(|params| (index, params))
        })
    }

    /// Dispatch a request (404 response when nothing matches).
    pub async fn handle(
        &self,
        request: SsrHttpRequest,
        context: SsrContext,
    ) -> Result<SsrResponse> {
        let path = request.uri.split('?').next().unwrap_or("/").to_string();
        if let Some((index, params)) = self.find(&path) {
            let route = &self.routes[index];
            return (route.handler)(request, context, params).await;
        }
        Ok(SsrResponse {
            status: 404,
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
            body: RenderBody::Full("not found".to_string()),
        })
    }
}

impl std::fmt::Debug for SsrRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SsrRouter")
            .field(
                "routes",
                &self
                    .routes
                    .iter()
                    .map(|route| &route.pattern)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Match `/users/:id` against `/users/42`.
#[must_use]
pub fn match_route(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    if pattern_parts.len() != path_parts.len() {
        return None;
    }
    let mut params = HashMap::new();
    for (expected, actual) in pattern_parts.iter().zip(path_parts.iter()) {
        if let Some(name) = expected.strip_prefix(':') {
            if actual.is_empty() {
                return None;
            }
            params.insert(name.to_string(), (*actual).to_string());
        } else if expected != actual {
            return None;
        }
    }
    Some(params)
}
