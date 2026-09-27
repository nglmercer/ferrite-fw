//! Server-side rendering (spec §21–§23, §46–§50).
//!
//! [`SsrAdapter`] is the framework boundary: pure-Rust SSR (Leptos/Dioxus/
//! custom), embedded-JS SSR via [`ferrite_runtime::JsRuntime`], or hybrid
//! Rust routes + JS islands. Also: externals, streaming, islands, and the
//! `/_ferrite/rpc/` server-function transport.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use ferrite_config::SsrConfig;
use ferrite_core::{FerriteError, Result};
use futures::Stream;

/// An SSR HTTP request (framework-agnostic).
#[derive(Debug, Clone)]
pub struct SsrHttpRequest {
    /// Method (`GET`, ...).
    pub method: String,
    /// Full URI.
    pub uri: String,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

/// SSR render context.
#[derive(Debug, Clone, Default)]
pub struct SsrContext {
    /// Request URL.
    pub url: String,
    /// Request headers.
    pub headers: HashMap<String, String>,
    /// CSP nonce, when configured.
    pub nonce: Option<String>,
    /// Preload files for this route (from the manifest).
    pub preload: Vec<String>,
}

/// Rendered body: full string or byte stream (§48).
pub enum RenderBody {
    /// Complete HTML.
    Full(String),
    /// Streamed HTML.
    Stream(Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>),
}

impl std::fmt::Debug for RenderBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(html) => f.debug_tuple("Full").field(&html.len()).finish(),
            Self::Stream(_) => f.debug_tuple("Stream").finish(),
        }
    }
}

/// SSR response.
#[derive(Debug)]
pub struct SsrResponse {
    /// Status code.
    pub status: u16,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: RenderBody,
}

impl SsrResponse {
    /// HTML response.
    #[must_use]
    pub fn html(html: impl Into<String>) -> Self {
        Self {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: RenderBody::Full(html.into()),
        }
    }

    /// Streaming HTML response from chunks.
    #[must_use]
    pub fn stream(chunks: Vec<String>) -> Self {
        let stream = futures::stream::iter(chunks.into_iter().map(|chunk| Ok(Bytes::from(chunk))));
        Self {
            status: 200,
            headers: vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            body: RenderBody::Stream(Box::pin(stream)),
        }
    }

    /// Collect the body into a string (buffers streams).
    pub async fn into_string(self) -> Result<String> {
        match self.body {
            RenderBody::Full(html) => Ok(html),
            RenderBody::Stream(mut stream) => {
                use futures::StreamExt as _;
                let mut output = Vec::new();
                while let Some(chunk) = stream.next().await {
                    output.extend_from_slice(&chunk?);
                }
                String::from_utf8(output)
                    .map_err(|error| FerriteError::Ssr(format!("non-utf8 stream: {error}")))
            }
        }
    }
}

/// Framework SSR adapter (§46).
#[async_trait::async_trait]
pub trait SsrAdapter: Send + Sync {
    /// Render a request to a response.
    async fn render(&self, request: SsrHttpRequest, context: SsrContext) -> Result<SsrResponse>;
}

/// Closure-based adapter for custom renderers.
pub struct FnAdapter<F> {
    /// Render function.
    handler: F,
}

impl<F> FnAdapter<F> {
    /// Wrap a render closure.
    #[must_use]
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

#[async_trait::async_trait]
impl<F, Fut> SsrAdapter for FnAdapter<F>
where
    F: Fn(SsrHttpRequest, SsrContext) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<SsrResponse>> + Send,
{
    async fn render(&self, request: SsrHttpRequest, context: SsrContext) -> Result<SsrResponse> {
        (self.handler)(request, context).await
    }
}

/// Static-shell adapter: serves an HTML shell with preload injection.
#[derive(Debug, Clone)]
pub struct StaticShellAdapter {
    /// HTML shell (`<!--ssr-outlet-->` marks the app outlet).
    pub shell: String,
}

#[async_trait::async_trait]
impl SsrAdapter for StaticShellAdapter {
    async fn render(&self, _request: SsrHttpRequest, context: SsrContext) -> Result<SsrResponse> {
        let mut preload = String::new();
        for file in &context.preload {
            if file.ends_with(".css") {
                preload.push_str(&format!("<link rel=\"stylesheet\" href=\"{file}\">\n"));
            } else if file.ends_with(".js") {
                preload.push_str(&format!("<link rel=\"modulepreload\" href=\"{file}\">\n"));
            }
        }
        let html = self
            .shell
            .replace("<!--ssr-outlet-->", "")
            .replace("</head>", &format!("{preload}</head>"));
        Ok(SsrResponse::html(html))
    }
}

/// A loaded SSR module (`ssrLoadModule`, §22).
#[derive(Debug, Clone)]
pub struct SsrModule {
    /// Module id.
    pub id: String,
    /// Transformed SSR code.
    pub code: String,
    /// Transitive dependency ids.
    pub dependencies: Vec<String>,
}

/// True when `id` (bare specifier or path) is SSR-external (§23).
#[must_use]
pub fn is_external(specifier: &str, config: &SsrConfig) -> bool {
    if config.bundle_all {
        return false;
    }
    let matches = |list: &[String]| {
        list.iter().any(|pattern| {
            pattern == specifier
                || specifier.starts_with(&format!("{pattern}/"))
                || (pattern.contains('*') && glob_match(pattern, specifier))
        })
    };
    if matches(&config.no_external) {
        return false;
    }
    if matches(&config.external) {
        return true;
    }
    // Default: bare imports of known server-only packages stay external.
    !specifier.starts_with('.')
        && !specifier.starts_with('/')
        && !specifier.starts_with("virtual:")
        && is_server_only(specifier)
}

/// Known server-only packages (kept external unless `noExternal`).
fn is_server_only(specifier: &str) -> bool {
    let name = specifier.split('/').next().unwrap_or(specifier);
    matches!(
        name,
        "pg" | "sharp" | "fsevents" | "node-gyp" | "esbuild" | "workerd"
    )
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // Single-`*` glob.
    match pattern.split_once('*') {
        Some((prefix, suffix)) => text.starts_with(prefix) && text.ends_with(suffix),
        None => pattern == text,
    }
}

/// An SSR island (§49): server-rendered HTML + selective hydration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Island {
    /// Component name.
    pub name: String,
    /// Serialized props.
    pub props: serde_json::Value,
    /// Server-rendered HTML.
    pub html: String,
}

/// Render an island placeholder div.
#[must_use]
pub fn island_tag(island: &Island) -> String {
    let props = serde_json::to_string(&island.props).unwrap_or_else(|_| "{}".to_string());
    let escaped = props.replace('"', "&quot;");
    format!(
        "<div data-ferrite-island=\"{}\" data-props=\"{}\">{}</div>",
        island.name, escaped, island.html
    )
}

/// Client hydration scanner for islands (§49).
#[must_use]
pub fn island_hydration_script() -> &'static str {
    r#"for (const el of document.querySelectorAll("[data-ferrite-island]")) {
  const name = el.getAttribute("data-ferrite-island");
  const props = JSON.parse(el.getAttribute("data-props") || "{}");
  import(`/islands/${name}.js`).then((mod) => mod.hydrate?.(el, props));
}
"#
}

/// Server-function RPC transport (§50).
pub const RPC_ROUTE_PREFIX: &str = "/_ferrite/rpc/";

/// RPC encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcEncoding {
    /// JSON (v0.1).
    Json,
    /// MessagePack (roadmap).
    MessagePack,
    /// CBOR (roadmap).
    Cbor,
}

/// An RPC invocation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcRequest {
    /// Function hash.
    pub hash: String,
    /// JSON-encoded arguments.
    pub args: serde_json::Value,
}

/// Registry of server functions (`hash` → handler).
#[derive(Clone, Default)]
pub struct RpcRegistry {
    /// Handlers.
    handlers: HashMap<String, Arc<dyn Fn(serde_json::Value) -> RpcBoxFuture + Send + Sync>>,
}

type RpcBoxFuture = Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send>>;

impl RpcRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for `hash`.
    pub fn register<F, Fut>(&mut self, hash: impl Into<String>, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        self.handlers.insert(
            hash.into(),
            Arc::new(move |args| Box::pin(handler(args)) as RpcBoxFuture),
        );
    }

    /// Invoke a handler.
    pub async fn invoke(&self, request: &RpcRequest) -> Result<serde_json::Value> {
        let handler = self.handlers.get(&request.hash).ok_or_else(|| {
            FerriteError::Ssr(format!("unknown server function `{}`", request.hash))
        })?;
        handler(request.args.clone()).await
    }
}

impl std::fmt::Debug for RpcRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcRegistry")
            .field("hashes", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

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
type SsrHandler = Arc<
    dyn Fn(SsrHttpRequest, SsrContext, HashMap<String, String>) -> RpcBoxFutureSsr + Send + Sync,
>;

/// One route: pattern + handler.
#[derive(Clone)]
struct SsrRoute {
    pattern: String,
    handler: SsrHandler,
}

type RpcBoxFutureSsr = Pin<Box<dyn std::future::Future<Output = Result<SsrResponse>> + Send>>;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_matching() {
        let config = SsrConfig {
            external: vec!["pg".to_string()],
            no_external: vec!["my-esm-package".to_string()],
            ..Default::default()
        };
        assert!(is_external("pg", &config));
        assert!(is_external("pg/lib", &config));
        assert!(!is_external("my-esm-package", &config));
        assert!(!is_external("./local.ts", &config));
    }

    #[tokio::test]
    async fn rpc_roundtrip() {
        let mut registry = RpcRegistry::new();
        registry.register("abc123", |args| async move { Ok(args) });
        let value = registry
            .invoke(&RpcRequest {
                hash: "abc123".to_string(),
                args: serde_json::json!({"id": 1}),
            })
            .await
            .unwrap();
        assert_eq!(value, serde_json::json!({"id": 1}));
    }

    #[tokio::test]
    async fn stream_collects() {
        let response = SsrResponse::stream(vec!["<a>".to_string(), "</a>".to_string()]);
        assert_eq!(response.into_string().await.unwrap(), "<a></a>");
    }

    #[test]
    fn route_params_match() {
        let params = super::match_route("/users/:id", "/users/42").unwrap();
        assert_eq!(params.get("id").unwrap(), "42");
        assert!(super::match_route("/users/:id", "/users").is_none());
        assert!(super::match_route("/a", "/b").is_none());
    }

    #[tokio::test]
    async fn router_dispatches_and_404s() {
        let mut router = super::SsrRouter::new();
        router.route("/users/:id", |_request, _context, params| async move {
            Ok(super::SsrResponse::html(format!("user {}", params["id"])))
        });
        let found = router
            .handle(
                super::SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/users/7".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                super::SsrContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(found.status, 200);
        let missing = router
            .handle(
                super::SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/nope".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                super::SsrContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status, 404);
    }
}
