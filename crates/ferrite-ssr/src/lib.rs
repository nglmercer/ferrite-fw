//! Server-side rendering (spec §21–§23, §46–§50).
//!
//! [`SsrAdapter`] is the framework boundary: pure-Rust SSR (Leptos/Dioxus/
//! custom), embedded-JS SSR via [`ferrite_runtime::JsRuntime`], or hybrid
//! Rust routes + JS islands. Also: externals, streaming, islands, and the
//! `/_ferrite/rpc/` server-function transport.

use std::collections::HashMap;
#[cfg(feature = "napi-vm")]
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use ferrite_config::{ResolvedConfig, SsrConfig};
use ferrite_core::{FerriteError, Result};
use ferrite_runtime::{
    runtime_for_backend, CompiledModule, JsRuntime, JsValue, RuntimeEnvironment,
};
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
        Ok(SsrResponse::html(inject_shell(
            &self.shell,
            "",
            &context.preload,
        )))
    }
}

/// Inject a rendered body plus preload tags into an HTML shell.
fn inject_shell(shell: &str, body: &str, preload_files: &[String]) -> String {
    let mut preload = String::new();
    for file in preload_files {
        if file.ends_with(".css") {
            preload.push_str(&format!("<link rel=\"stylesheet\" href=\"{file}\">\n"));
        } else if file.ends_with(".js") {
            preload.push_str(&format!("<link rel=\"modulepreload\" href=\"{file}\">\n"));
        }
    }
    shell
        .replace("<!--ssr-outlet-->", body)
        .replace("</head>", &format!("{preload}</head>"))
}

/// SSR adapter backed by an embedded JS runtime (§20, §46).
///
/// Evaluates the entry module, calls its `render` export with the request
/// URL, and serves the returned HTML string — injected into `shell` when
/// set. Evaluation runs per request (dev usage); production SSR should
/// cache or pre-render. Build with [`JsSsrAdapter::from_resolved`] so the
/// `[runtime]` backend, budgets, and native allowlist are honored.
pub struct JsSsrAdapter {
    runtime: Arc<dyn JsRuntime>,
    module: CompiledModule,
    export: String,
    shell: Option<String>,
}

impl JsSsrAdapter {
    /// Wrap an explicit runtime and entry module.
    #[must_use]
    pub fn new(runtime: Arc<dyn JsRuntime>, module: CompiledModule) -> Self {
        Self {
            runtime,
            module,
            export: "render".to_string(),
            shell: None,
        }
    }

    /// Build from resolved config.
    ///
    /// The `napi-vm` backend maps fuel/loop budgets plus the native
    /// allowlist/integrity pins onto the worker; every other name goes
    /// through [`runtime_for_backend`] (unknown or unavailable backends
    /// fail loudly at render, never silently).
    #[must_use]
    pub fn from_resolved(resolved: &ResolvedConfig, module: CompiledModule) -> Self {
        #[cfg(feature = "napi-vm")]
        if resolved.runtime.backend == "napi-vm" {
            let mut options = ferrite_runtime::napi_vm::NapiVmOptions {
                roots: vec![resolved.root.clone()],
                fuel_budget: resolved.runtime.fuel_budget,
                loop_budget: resolved.runtime.loop_budget,
                ..Default::default()
            };
            for entry in &resolved.runtime.native_allow {
                options
                    .native_allow
                    .push(ferrite_runtime::napi_vm::NativeAddonAllow {
                        path: PathBuf::from(entry),
                        sha256_hex: resolved.runtime.native_integrity.get(entry).cloned(),
                    });
            }
            let runtime: Arc<dyn JsRuntime> =
                Arc::new(ferrite_runtime::napi_vm::NapiVmRuntime::new(options));
            return Self {
                runtime,
                module,
                export: "render".to_string(),
                shell: None,
            };
        }
        Self::new(runtime_for_backend(&resolved.runtime.backend), module)
    }

    /// Call a different export instead of `render`.
    #[must_use]
    pub fn with_export(mut self, export: impl Into<String>) -> Self {
        self.export = export.into();
        self
    }

    /// Inject rendered HTML into a shell (`<!--ssr-outlet-->` outlet).
    #[must_use]
    pub fn with_shell(mut self, shell: impl Into<String>) -> Self {
        self.shell = Some(shell.into());
        self
    }
}

#[async_trait::async_trait]
impl SsrAdapter for JsSsrAdapter {
    async fn render(&self, request: SsrHttpRequest, context: SsrContext) -> Result<SsrResponse> {
        let url = if context.url.is_empty() {
            request.uri.clone()
        } else {
            context.url.clone()
        };
        let namespace = self
            .runtime
            .evaluate_module(
                self.module.clone(),
                RuntimeEnvironment {
                    ssr: true,
                    request_id: None,
                },
            )
            .await?;
        let handle = namespace.get_function(&self.export)?.clone();
        let result = self
            .runtime
            .call(&handle, vec![JsValue::String(url)])
            .await?;
        let JsValue::String(html) = result else {
            return Err(FerriteError::Ssr(format!(
                "`{}` export must return an HTML string",
                self.export
            )));
        };
        let body = match &self.shell {
            Some(shell) => inject_shell(shell, &html, &context.preload),
            None => html,
        };
        Ok(SsrResponse::html(body))
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

/// True when `specifier` names a native `.node` binary (query stripped).
#[must_use]
pub fn is_native_specifier(specifier: &str) -> bool {
    specifier
        .split('?')
        .next()
        .unwrap_or(specifier)
        .ends_with(".node")
}

/// True when `id` (bare specifier or path) is SSR-external (§23).
///
/// Native `.node` binaries are always external: they cannot be bundled.
/// Load them at runtime through an embedded backend (see
/// [`native_shim_module`]).
#[must_use]
pub fn is_external(specifier: &str, config: &SsrConfig) -> bool {
    if is_native_specifier(specifier) {
        return true;
    }
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

/// SSR placeholder for a native `.node` binary (§22–§23).
///
/// Bundlers and `ssrLoadModule` implementations must substitute this for
/// any [`is_native_specifier`] import instead of reading the binary: the
/// stub throws on evaluation with an actionable message telling the user
/// to allowlist the file under `[runtime]` and select the `napi-vm`
/// backend (feature `napi-vm`, disabled by default).
#[must_use]
pub fn native_shim_module(specifier: &str) -> SsrModule {
    let literal = js_single_quoted(specifier);
    SsrModule {
        id: specifier.to_string(),
        code: format!(
            "throw new Error(\"[ferrite] cannot bundle native module {literal}: \
             .node binaries stay external in SSR. Allowlist the file under [runtime] \
             native_allow (+ native_integrity) and run with `--runtime napi-vm` \
             (build with `--features napi-vm`).\");\n"
        ),
        dependencies: Vec::new(),
    }
}

/// Render a Rust string as a JS single-quoted literal.
fn js_single_quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        match ch {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
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

/// RPC encodings (§50).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcEncoding {
    /// JSON (default).
    Json,
    /// MessagePack.
    MessagePack,
    /// CBOR.
    Cbor,
}

impl RpcEncoding {
    /// Wire content type.
    #[must_use]
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::MessagePack => "application/msgpack",
            Self::Cbor => "application/cbor",
        }
    }

    /// Detect the encoding from a `Content-Type` header value (parameters
    /// ignored). Missing or unknown types fall back to JSON.
    #[must_use]
    pub fn from_content_type(content_type: Option<&str>) -> Self {
        match content_type.map(|raw| raw.split(';').next().unwrap_or("").trim()) {
            Some("application/msgpack" | "application/x-msgpack") => Self::MessagePack,
            Some("application/cbor") => Self::Cbor,
            _ => Self::Json,
        }
    }

    /// Decode RPC arguments from wire bytes.
    pub fn decode(self, bytes: &[u8]) -> Result<serde_json::Value> {
        match self {
            Self::Json => serde_json::from_slice(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad JSON rpc args: {error}"))),
            Self::MessagePack => rmp_serde::from_slice(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad MessagePack rpc args: {error}"))),
            Self::Cbor => ciborium::from_reader(bytes)
                .map_err(|error| FerriteError::Ssr(format!("bad CBOR rpc args: {error}"))),
        }
    }

    /// Encode an RPC result to wire bytes.
    pub fn encode(self, value: &serde_json::Value) -> Result<Vec<u8>> {
        match self {
            Self::Json => serde_json::to_vec(value)
                .map_err(|error| FerriteError::Ssr(format!("cannot encode JSON rpc: {error}"))),
            Self::MessagePack => rmp_serde::to_vec(value).map_err(|error| {
                FerriteError::Ssr(format!("cannot encode MessagePack rpc: {error}"))
            }),
            Self::Cbor => {
                let mut bytes = Vec::new();
                ciborium::into_writer(value, &mut bytes).map_err(|error| {
                    FerriteError::Ssr(format!("cannot encode CBOR rpc: {error}"))
                })?;
                Ok(bytes)
            }
        }
    }
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

    fn test_resolved() -> ResolvedConfig {
        ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(std::env::temp_dir()),
            ferrite_config::CliOverrides::default(),
        )
        .expect("resolve test config")
    }

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

    #[test]
    fn native_binaries_are_always_external() {
        let config = SsrConfig {
            bundle_all: true,
            no_external: vec!["./addon.node".to_string()],
            ..Default::default()
        };
        assert!(is_native_specifier("./addon.node"));
        assert!(is_native_specifier("./addon.node?v=1"));
        assert!(is_native_specifier("pkg/prebuilds/a.node"));
        assert!(!is_native_specifier("./addon.js"));
        assert!(is_external("./addon.node", &config));
        assert!(is_external("./addon.node?v=1", &SsrConfig::default()));
    }

    #[test]
    fn native_shim_throws_actionable_error() {
        let module = native_shim_module("./na'tive\\addon.node");
        assert_eq!(module.id, "./na'tive\\addon.node");
        assert!(module.dependencies.is_empty());
        assert!(
            module.code.starts_with("throw new Error("),
            "{}",
            module.code
        );
        assert!(module.code.contains("native_allow"), "{}", module.code);
        assert!(
            module.code.contains("--features napi-vm"),
            "{}",
            module.code
        );
        // Escaped literal: no raw quote or backslash breaks the JS string.
        assert!(
            module.code.contains("./na\\'tive\\\\addon.node"),
            "{}",
            module.code
        );
    }

    #[test]
    fn inject_shell_combines_body_and_preloads() {
        let shell = "<html><head></head><body><!--ssr-outlet--></body></html>";
        let html = inject_shell(
            shell,
            "<h1>hi</h1>",
            &[
                "/a.css".to_string(),
                "/b.js".to_string(),
                "/c.png".to_string(),
            ],
        );
        assert!(html.contains("<h1>hi</h1>"), "{html}");
        assert!(
            html.contains("<link rel=\"stylesheet\" href=\"/a.css\">"),
            "{html}"
        );
        assert!(
            html.contains("<link rel=\"modulepreload\" href=\"/b.js\">"),
            "{html}"
        );
        assert!(!html.contains("c.png"), "{html}");
        assert!(!html.contains("<!--ssr-outlet-->"), "{html}");
    }

    #[tokio::test]
    async fn unknown_backend_errors_loudly() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "does-not-exist".to_string();
        let adapter = JsSsrAdapter::from_resolved(
            &resolved,
            CompiledModule {
                id: "entry".to_string(),
                code: "export function render() { return \"x\"; }".to_string(),
                url: None,
            },
        );
        let error = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("does-not-exist"), "{error}");
    }

    #[cfg(feature = "napi-vm")]
    #[tokio::test]
    async fn renders_through_napi_vm() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "napi-vm".to_string();
        let adapter = JsSsrAdapter::from_resolved(
            &resolved,
            CompiledModule {
                id: "entry".to_string(),
                code: "export function render(url) { return `<h1>hello from ${url}</h1>`; }\n"
                    .to_string(),
                url: None,
            },
        )
        .with_shell("<html><head></head><body><!--ssr-outlet--></body></html>");
        let response = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/about".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext {
                    preload: vec!["/app.js".to_string()],
                    ..Default::default()
                },
            )
            .await
            .expect("render");
        let html = response.into_string().await.expect("body");
        assert!(html.contains("<h1>hello from /about</h1>"), "{html}");
        assert!(
            html.contains("<link rel=\"modulepreload\" href=\"/app.js\">"),
            "{html}"
        );
    }

    #[cfg(feature = "napi-vm")]
    #[tokio::test]
    async fn bad_allowlist_mapping_fails_loudly() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "napi-vm".to_string();
        resolved.runtime.native_allow = vec!["/nonexistent-ferrite/missing.node".to_string()];
        let adapter = JsSsrAdapter::from_resolved(
            &resolved,
            CompiledModule {
                id: "entry".to_string(),
                code: "export function render() { return \"x\"; }".to_string(),
                url: None,
            },
        );
        let error = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("missing.node"), "{error}");
    }

    #[test]
    fn rpc_encodings_roundtrip() {
        let value = serde_json::json!({"id": 1, "tags": ["a", "b"], "nested": {"ok": true}});
        for encoding in [
            RpcEncoding::Json,
            RpcEncoding::MessagePack,
            RpcEncoding::Cbor,
        ] {
            let bytes = encoding.encode(&value).expect("encode");
            assert!(!bytes.is_empty());
            let back = encoding.decode(&bytes).expect("decode");
            assert_eq!(back, value, "{encoding:?}");
        }
        // Binary encodings are more compact than JSON here.
        let json_len = RpcEncoding::Json.encode(&value).unwrap().len();
        assert!(RpcEncoding::MessagePack.encode(&value).unwrap().len() < json_len);
        assert!(RpcEncoding::Cbor.encode(&value).unwrap().len() < json_len);
    }

    #[test]
    fn rpc_content_type_detection() {
        assert_eq!(RpcEncoding::from_content_type(None), RpcEncoding::Json);
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/json")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/json; charset=utf-8")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/msgpack")),
            RpcEncoding::MessagePack
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/x-msgpack")),
            RpcEncoding::MessagePack
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/cbor")),
            RpcEncoding::Cbor
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("text/plain")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::MessagePack.content_type(),
            "application/msgpack"
        );
        assert_eq!(RpcEncoding::Cbor.content_type(), "application/cbor");
    }

    #[test]
    fn rpc_decode_rejects_garbage() {
        // Invalid UTF-8 (JSON), 0xc1 never-used byte (msgpack), tag(1)
        // followed by break (CBOR): all three must fail.
        for encoding in [
            RpcEncoding::Json,
            RpcEncoding::MessagePack,
            RpcEncoding::Cbor,
        ] {
            assert!(encoding.decode(b"\xc1\xff").is_err(), "{encoding:?}");
            assert!(encoding.decode(b"").is_err(), "{encoding:?} empty");
        }
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
