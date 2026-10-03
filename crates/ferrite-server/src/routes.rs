//! Dev server HTTP handlers.

use crate::DevServer;
use crate::DevServerInner;
use crate::PipelineResponse;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::Uri;
use axum::response::IntoResponse;
use axum::response::Response;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use ferrite_hmr::client_source;
use ferrite_ssr::RpcEncoding;
use ferrite_ssr::SsrContext;
use ferrite_ssr::SsrHttpRequest;
use ferrite_ssr::RPC_ROUTE_PREFIX;
use std::sync::Arc;
use std::sync::Mutex;

// --- HTTP handlers -----------------------------------------------------------

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    State(inner): State<Arc<DevServerInner>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, inner))
}

async fn handle_socket(socket: WebSocket, inner: Arc<DevServerInner>) {
    use futures::{SinkExt as _, StreamExt as _};
    let (mut sender, mut receiver) = socket.split();
    // Subscribe before the handshake so diagnostics cannot fall into its gap.
    let mut rx = inner.hmr.subscribe();
    // Handshake.
    let hello = serde_json::to_string(&ferrite_hmr::HmrMessage::Connected {
        version: ferrite_core::VERSION.to_string(),
    })
    .unwrap_or_default();
    if sender.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    let previous_error = inner.hmr_error.lock().ok().and_then(|error| error.clone());
    if let Some(err) = previous_error {
        let text = serde_json::to_string(&ferrite_hmr::HmrMessage::Error { err })
            .expect("diagnostics are serializable");
        if sender.send(Message::Text(text.into())).await.is_err() {
            return;
        }
    }
    let send_task = tokio::spawn(async move {
        while let Ok(text) = rx.recv().await {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(message) = receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                tracing::debug!("hmr client message: {text:?}");
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
    send_task.abort();
}

pub(crate) async fn client_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        client_source(),
    )
}

pub(crate) async fn inspect_handler(State(inner): State<Arc<DevServerInner>>) -> impl IntoResponse {
    let modules = inner.graph.module_ids();
    let body = serde_json::json!({
        "version": ferrite_core::VERSION,
        "root": inner.config.root,
        "mode": inner.config.mode,
        "plugins": inner.plugins.names(),
        "modules": modules.len(),
        "moduleSample": modules.iter().take(50).map(|id| &id.0).collect::<Vec<_>>(),
        "hmrReceivers": inner.hmr.receivers(),
        "warnings": inner.warnings.lock().map(|w| w.clone()).unwrap_or_default(),
    });
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
}

pub(crate) async fn rpc_handler(
    State(inner): State<Arc<DevServerInner>>,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let hash = uri
        .path()
        .strip_prefix(RPC_ROUTE_PREFIX)
        .unwrap_or("")
        .to_string();
    let encoding = RpcEncoding::from_content_type(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
    );
    let args = match encoding.decode(&body) {
        Ok(args) => args,
        Err(error) => return error_response(&error),
    };
    let registry = inner.rpc.read().await;
    match registry
        .invoke(&ferrite_ssr::RpcRequest { hash, args })
        .await
    {
        Ok(value) => match encoding.encode(&value) {
            Ok(bytes) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, encoding.content_type())],
                bytes,
            )
                .into_response(),
            Err(error) => error_response(&error),
        },
        Err(error) => error_response(&error),
    }
}

pub(crate) async fn fallback_handler(
    State(inner): State<Arc<DevServerInner>>,
    method: axum::http::Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let path = uri.path().to_string();
    let query = uri
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let url = format!("{path}{query}");
    // Proxy rules run before the pipeline (Vite middleware order).
    if !inner.config.server.proxy.is_empty() {
        let rules = crate::proxy::rules_from_config(&inner.config.server.proxy);
        if let Some(rule) = crate::proxy::match_proxy(&rules, &path) {
            let target = rule.forward_url(&url);
            match crate::proxy::forward(&inner.http_client, &method, &target, &headers, body).await
            {
                Ok(response) => return response,
                Err(error) => {
                    let body = serde_json::json!({
                        "error": format!("proxy to `{target}` failed: {error}"),
                    });
                    return (
                        StatusCode::BAD_GATEWAY,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        body.to_string(),
                    )
                        .into_response();
                }
            }
        }
    }
    // Assemble a lightweight server handle for the pipeline.
    let server = DevServerRef {
        inner: inner.clone(),
    };
    match server.transform_request_owned(&url).await {
        Ok(response) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                response.content_type.clone(),
            )],
            response.body,
        )
            .into_response(),
        Err(error) => {
            // SSR adapter fallback for app routes (no extension, not a file).
            if !path.contains('.') {
                let adapter = inner.ssr_adapter.read().await.clone();
                if let Some(adapter) = adapter {
                    let request = SsrHttpRequest {
                        method: "GET".to_string(),
                        uri: url.clone(),
                        headers: Vec::new(),
                        body: Vec::new(),
                    };
                    let context = SsrContext {
                        url,
                        ..Default::default()
                    };
                    match adapter.render(request, context).await {
                        Ok(ssr) => {
                            let status = StatusCode::from_u16(ssr.status).unwrap_or(StatusCode::OK);
                            match ssr.body {
                                ferrite_ssr::RenderBody::Full(html) => {
                                    return (
                                        status,
                                        [(
                                            axum::http::header::CONTENT_TYPE,
                                            "text/html; charset=utf-8",
                                        )],
                                        html,
                                    )
                                        .into_response();
                                }
                                ferrite_ssr::RenderBody::Stream(stream) => {
                                    use futures::StreamExt as _;
                                    let mapped = stream.map(|item| {
                                        item.map_err(|error| {
                                            std::io::Error::other(error.to_string())
                                        })
                                    });
                                    let body = axum::body::Body::from_stream(mapped);
                                    return (status, body).into_response();
                                }
                            }
                        }
                        Err(error) => return error_response(&error),
                    }
                }
            }
            if !inner.config.is_production && path.contains('.') && !path.starts_with("/@") {
                // Failed first loads still need a source node so a correction
                // or creation produces a reload instead of being untracked.
                let id = crate::util::strip_hmr_timestamp(&ferrite_core::ModuleId::new(&url));
                inner
                    .graph
                    .ensure(&id, ferrite_core::ModuleType::from_path(&path));
                let handle = DevServer {
                    inner: inner.clone(),
                    watcher: Arc::new(Mutex::new(None)),
                };
                handle.report_hmr_error(&id, &error);
            }
            error_response(&error)
        }
    }
}

/// Pipeline access without watcher ownership (HTTP handlers).
struct DevServerRef {
    inner: Arc<DevServerInner>,
}

impl DevServerRef {
    /// Mirror of [`DevServer::transform_request`] for borrowed state.
    async fn transform_request_owned(&self, url: &str) -> Result<PipelineResponse> {
        // Reuse the same logic by constructing a temporary handle. The
        // watcher field is unused on this path.
        let server = DevServer {
            inner: self.inner.clone(),
            watcher: Arc::new(Mutex::new(None)),
        };
        // Avoid re-running watcher setup: call the pipeline directly.
        server.transform_request_inner(url).await
    }
}

impl DevServer {
    /// Inner request transform (shared by owned + borrowed handles).
    async fn transform_request_inner(&self, url: &str) -> Result<PipelineResponse> {
        self.transform_request(url).await
    }
}

fn error_response(error: &FerriteError) -> Response {
    let diagnostic = error.diagnostic();
    let body = serde_json::json!({
        "error": diagnostic.message,
        "code": diagnostic.code,
        "id": diagnostic.id,
        "frame": diagnostic.frame,
    });
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

pub(crate) async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
