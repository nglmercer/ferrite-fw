//! SSR adapters.

use crate::request::*;
use ferrite_config::ResolvedConfig;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use ferrite_runtime::runtime_for_backend;
use ferrite_runtime::JsRuntime;
use ferrite_runtime::JsValue;
use ferrite_runtime::RuntimeEnvironment;
use ferrite_runtime::{CompiledModule, CompiledModuleGraph};
#[cfg(feature = "napi-vm")]
use std::path::PathBuf;
use std::sync::Arc;

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
pub(crate) fn inject_shell(shell: &str, body: &str, preload_files: &[String]) -> String {
    let mut preload = String::new();
    for file in preload_files {
        let path = file.split(['?', '#']).next().unwrap_or(file);
        let escaped = file
            .replace('&', "&amp;")
            .replace('"', "&quot;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\'', "&#39;");
        if path.ends_with(".css") {
            preload.push_str(&format!("<link rel=\"stylesheet\" href=\"{escaped}\">\n"));
        } else if path.ends_with(".js") {
            preload.push_str(&format!(
                "<link rel=\"modulepreload\" href=\"{escaped}\">\n"
            ));
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
    render_lock: tokio::sync::Mutex<()>,
    module: CompiledModule,
    graph: Option<CompiledModuleGraph>,
    export: String,
    shell: Option<String>,
}

impl JsSsrAdapter {
    /// Wrap an explicit runtime and entry module.
    #[must_use]
    pub fn new(runtime: Arc<dyn JsRuntime>, module: CompiledModule) -> Self {
        Self {
            runtime,
            render_lock: tokio::sync::Mutex::new(()),
            module,
            graph: None,
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
                queue_capacity: resolved.runtime.queue_capacity,
                max_request_bytes: resolved.runtime.max_request_bytes,
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
                render_lock: tokio::sync::Mutex::new(()),
                module,
                graph: None,
                export: "render".to_string(),
                shell: None,
            };
        }
        Self::new(runtime_for_backend(&resolved.runtime.backend), module)
    }

    /// Build from an explicitly compiled graph and the configured runtime.
    /// No dependency is compiled or substituted by this adapter.
    pub fn from_resolved_graph(
        resolved: &ResolvedConfig,
        graph: CompiledModuleGraph,
    ) -> Result<Self> {
        graph.validate()?;
        let entry = graph
            .modules
            .iter()
            .find(|module| module.id == graph.entry)
            .expect("validated graph entry")
            .clone();
        let mut adapter = Self::from_resolved(resolved, entry);
        adapter.graph = Some(graph);
        Ok(adapter)
    }

    /// Replace the compiled graph while retaining the explicitly selected runtime.
    /// Callers must serialize replacement and rendering for a shared adapter.
    pub fn replace_graph(&mut self, graph: CompiledModuleGraph) -> Result<()> {
        graph.validate()?;
        self.graph = Some(graph);
        Ok(())
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
        let _render = self.render_lock.lock().await;
        let url = if context.url.is_empty() {
            request.uri.clone()
        } else {
            context.url.clone()
        };
        let environment = RuntimeEnvironment {
            ssr: true,
            request_id: None,
        };
        let graph = self.graph.clone().unwrap_or_else(|| CompiledModuleGraph {
            entry: self.module.id.clone(),
            modules: vec![self.module.clone()],
        });
        // Keep the legacy URL argument, adding a lossless request payload.
        // Bytes are an array rather than a lossy UTF-8 body conversion.
        let request_value = JsValue::from(serde_json::json!({
            "method": request.method,
            "uri": request.uri,
            "headers": request.headers,
            "body": request.body,
        }));
        let result = self
            .runtime
            .invoke_module_graph(
                graph,
                &self.export,
                vec![JsValue::String(url), request_value],
                environment,
            )
            .await?;
        let mut response = js_render_response(result, &self.export)?;
        if let (Some(shell), crate::RenderBody::Full(html)) = (&self.shell, &mut response.body) {
            *html = inject_shell(shell, html, &context.preload);
        }
        Ok(response)
    }
}

/// Structured JavaScript rendering result. Streaming requires a separate
/// runtime contract and is deliberately not accepted as a JSON result.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct JsRenderResponse {
    html: String,
    #[serde(default = "default_render_status")]
    status: f64,
    #[serde(default)]
    headers: Vec<(String, String)>,
}

fn default_render_status() -> f64 {
    200.0
}

fn js_render_response(value: JsValue, export: &str) -> Result<SsrResponse> {
    if let JsValue::String(html) = value {
        return Ok(SsrResponse::html(html));
    }
    let json = ferrite_runtime::js_value_to_json(&value)?;
    let rendered: JsRenderResponse = serde_json::from_value(json).map_err(|error| FerriteError::Ssr(format!(
        "`{export}` must return an HTML string or {{ html, status?, headers?: [[name, value], ...] }}: {error}"
    )))?;
    if !rendered.status.is_finite()
        || rendered.status.fract() != 0.0
        || !(200.0..=599.0).contains(&rendered.status)
    {
        return Err(FerriteError::Ssr(format!(
            "`{export}` returned invalid final HTTP status {} (expected 200..599)",
            rendered.status
        )));
    }
    let mut response = SsrResponse::html(rendered.html);
    response.status = rendered.status as u16;
    // Explicit Content-Type takes precedence over the HTML default.
    if rendered
        .headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        response.headers.clear();
    }
    response.headers.extend(rendered.headers);
    Ok(response)
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
