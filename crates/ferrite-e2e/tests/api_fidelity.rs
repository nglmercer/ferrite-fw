//! API transport semantics, native cookie linkage and lifecycle budgets.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[path = "common/tls.rs"]
mod tls;
use tls::tls_fixture;

// Aborting a runner body must also release its fixture listener.
struct AbortOnDrop(tokio::task::AbortHandle);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[tokio::test]
async fn https_validation_redirect_errors_cookie_sync_and_response_disposal() {
    let mut tls = tls_fixture().await;
    let target = tls.url.clone();
    let app = axum::Router::new().route(
        "/tls",
        axum::routing::get(move || {
            let target = target.clone();
            async move {
                (
                    axum::http::StatusCode::FOUND,
                    [
                        ("location", target),
                        ("set-cookie", "tls_hop=1; Path=/".into()),
                    ],
                    "redirect",
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let _stop = AbortOnDrop(task.abort_handle());
    let checked = client(&base);
    assert!(matches!(
        checked
            .fetch_with(
                "GET",
                "/tls",
                ApiRequestOptions {
                    max_retries: 3,
                    timeout: Some(Duration::from_secs(1)),
                    ..Default::default()
                }
            )
            .await,
        Err(E2eError::Http(_))
    ));
    assert!(checked
        .cookie_header("/")
        .unwrap()
        .unwrap()
        .contains("tls_hop=1"));
    let ignored = ApiClient::with_options(ApiClientOptions {
        ignore_https_errors: true,
        base_url: Some(base.clone()),
        ..Default::default()
    })
    .unwrap();
    let mut response = ignored.get("/tls").await.unwrap();
    assert_eq!(response.status(), 200);
    assert!(response.url().starts_with(&tls.url));
    assert!(response.text().to_lowercase().contains("<html>"));
    ignored.dispose();
    assert!(
        !response.bytes().is_empty(),
        "returned responses own their bytes"
    );
    response.dispose();
    assert!(response.bytes().is_empty());
    assert!(matches!(
        ignored.get("/tls").await,
        Err(E2eError::Cancelled(_))
    ));
    for browser in browsers(&base).await {
        for ignore_https_errors in [false, true] {
            let context = browser
                .new_context(ContextOptions {
                    ignore_https_errors,
                    ..Default::default()
                })
                .await
                .unwrap();
            let result = context.request().get("/tls").await;
            if ignore_https_errors {
                assert_eq!(result.unwrap().status(), 200);
            } else {
                assert!(matches!(result, Err(E2eError::Http(_))));
            }
            assert!(context
                .cookies()
                .await
                .unwrap()
                .iter()
                .any(|cookie| cookie.name == "tls_hop" && cookie.value == "1"));
            context.close().await.unwrap();
        }
        browser.close().await.unwrap();
    }
    tls.child.kill().await.unwrap();
}

fn reference(name: &str) -> Value {
    let data: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/api-reference.json"
    ))
    .unwrap();
    assert_eq!(data["playwright"], "1.63.0");
    data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()
        .clone()
}
#[derive(Clone)]
struct State {
    cross: String,
    status_calls: Arc<AtomicUsize>,
    auth_calls: Arc<AtomicUsize>,
}
async fn echo(
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::Json<Value> {
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    axum::Json(
        json!({"method":method.as_str(), "body":String::from_utf8_lossy(&body), "authorization":header("authorization"), "cookie":header("cookie"), "content_type":header("content-type"), "payload":body.to_vec(), "x_extra":header("x-extra")}),
    )
}
async fn fixture() -> (String, State, Vec<tokio::task::AbortHandle>) {
    let other = axum::Router::new().route("/echo", axum::routing::any(echo));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let cross = format!("http://{}", listener.local_addr().unwrap());
    let other = tokio::spawn(async move {
        axum::serve(listener, other).await.unwrap();
    });
    let state = State {
        cross,
        status_calls: Arc::new(AtomicUsize::new(0)),
        auth_calls: Arc::new(AtomicUsize::new(0)),
    };
    let app = axum::Router::new()
        .route("/", axum::routing::get(|| async { "fixture" }))
        .route("/echo", axum::routing::any(echo))
        .route(
            "/redirect/{status}",
            axum::routing::any(
                |axum::extract::Path(status): axum::extract::Path<u16>| async move {
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        [("location", "/echo"), ("set-cookie", "hop=1; Path=/")],
                        "redirect",
                    )
                },
            ),
        )
        .route(
            "/cross",
            axum::routing::any(
                |axum::extract::State(state): axum::extract::State<State>| async move {
                    (
                        axum::http::StatusCode::FOUND,
                        [
                            ("location", format!("{}/echo", state.cross)),
                            ("set-cookie", "hop=1; Path=/".into()),
                        ],
                        "redirect",
                    )
                },
            ),
        )
        .route(
            "/loop",
            axum::routing::get(|| async {
                (
                    axum::http::StatusCode::FOUND,
                    [("location", "/loop"), ("set-cookie", "loop=1; Path=/")],
                    "loop",
                )
            }),
        )
        .route(
            "/status",
            axum::routing::get(
                |axum::extract::State(state): axum::extract::State<State>| async move {
                    state.status_calls.fetch_add(1, Ordering::SeqCst);
                    (axum::http::StatusCode::SERVICE_UNAVAILABLE, "unavailable")
                },
            ),
        )
        .route(
            "/auth",
            axum::routing::any(
                |axum::extract::State(state): axum::extract::State<State>,
                 method: axum::http::Method,
                 headers: axum::http::HeaderMap,
                 body: axum::body::Bytes| async move {
                    state.auth_calls.fetch_add(1, Ordering::SeqCst);
                    if headers
                        .get("authorization")
                        .is_some_and(|value| value == "Basic dXNlcjpwYXNz")
                    {
                        axum::response::IntoResponse::into_response(
                            echo(method, headers, body).await,
                        )
                    } else {
                        axum::response::IntoResponse::into_response((
                            axum::http::StatusCode::UNAUTHORIZED,
                            [("www-authenticate", "Basic realm=\"fixture\"")],
                            "challenge",
                        ))
                    }
                },
            ),
        )
        .route(
            "/slow",
            axum::routing::get(|| async {
                tokio::time::sleep(Duration::from_millis(250)).await;
                "slow"
            }),
        )
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, state, vec![other.abort_handle(), task.abort_handle()])
}
fn stop(tasks: Vec<tokio::task::AbortHandle>) {
    for task in tasks {
        task.abort();
    }
}
fn client(base: &str) -> ApiClient {
    ApiClient::with_base_url(base)
}
fn auth(base: &str, send: ApiCredentialsSend, origin: Option<String>) -> ApiClient {
    ApiClient::with_options(ApiClientOptions {
        base_url: Some(base.into()),
        credentials: Some(HttpCredentials::new("user", "pass")),
        credential_send: send,
        credential_origin: origin,
        ..Default::default()
    })
    .unwrap()
}
fn comparable(value: &Value) -> Value {
    json!({"method":value["method"], "body":value["body"], "authorization":value["authorization"]})
}

#[tokio::test]
async fn pinned_redirect_methods_payloads_limits_headers_and_status_failures() {
    let (base, state, tasks) = fixture().await;
    let api = client(&base);
    for method in ["POST", "PUT"] {
        for status in [301, 302, 303, 307, 308] {
            let value: Value = api
                .fetch_with(
                    method,
                    &format!("/redirect/{status}"),
                    ApiRequestOptions {
                        body: Some(b"abc".to_vec()),
                        headers: vec![("content-type".into(), "application/x-fixture".into())],
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
                .json()
                .unwrap();
            assert_eq!(
                comparable(&value),
                reference(&format!("{method}-{status}"))["result"]
            );
            if value["method"] == "GET" {
                assert!(value["content_type"].is_null());
            } else {
                assert_eq!(value["content_type"], "application/x-fixture");
            }
            assert!(value["cookie"].as_str().unwrap().contains("hop=1"));
        }
    }
    let no = api
        .fetch_with(
            "GET",
            "/redirect/302",
            ApiRequestOptions {
                max_redirects: Some(0),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        no.status() as u64,
        reference("no-redirect")["status"].as_u64().unwrap()
    );
    assert_eq!(
        no.text(),
        reference("no-redirect")["body"].as_str().unwrap()
    );
    assert!(
        matches!(api.fetch_with("GET", "/loop", ApiRequestOptions { max_redirects: Some(2), ..Default::default() }).await, Err(E2eError::Config(message)) if message.contains("redirect count"))
    );
    let value: Value = api
        .fetch_with(
            "POST",
            "/cross",
            ApiRequestOptions {
                body: Some(b"abc".to_vec()),
                headers: vec![
                    ("authorization".into(), "Bearer private".into()),
                    ("cookie".into(), "manual=first".into()),
                    ("x-extra".into(), "retained".into()),
                ],
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .json()
        .unwrap();
    assert!(value["authorization"].is_null());
    assert_eq!(value["x_extra"], "retained");
    assert!(!value["cookie"].as_str().unwrap().contains("manual="));
    let response = api
        .fetch_with(
            "GET",
            "/status",
            ApiRequestOptions {
                max_retries: 3,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        response.status() as u64,
        reference("no-http-status-retry")["status"]
            .as_u64()
            .unwrap()
    );
    assert_eq!(state.status_calls.load(Ordering::SeqCst), 1);
    assert!(
        matches!(api.fetch_with("GET", "/status", ApiRequestOptions { fail_on_status_code: true, ..Default::default() }).await, Err(E2eError::Config(message)) if message.contains("503") && message.contains("unavailable"))
    );
    // All conflicting payload pairs are rejected before sending.
    for (a, b) in [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)] {
        let mut options = ApiRequestOptions::default();
        for kind in [a, b] {
            match kind {
                0 => options.body = Some(vec![]),
                1 => options.json = Some(json!({})),
                2 => options.form = Some(vec![]),
                _ => options.multipart = Some(vec![]),
            }
        }
        assert!(
            matches!(api.fetch_with("POST", "/echo", options).await, Err(E2eError::Config(message)) if message.contains("mutually exclusive"))
        );
    }
    stop(tasks);
}

#[tokio::test]
async fn credentials_challenges_origin_filters_and_multipart_replay() {
    let (base, state, tasks) = fixture().await;
    for (label, send, origin) in [
        ("default", ApiCredentialsSend::Unauthorized, None),
        ("always", ApiCredentialsSend::Always, None),
        (
            "origin-mismatch",
            ApiCredentialsSend::Always,
            Some("http://127.0.0.1:1".into()),
        ),
    ] {
        let api = auth(&base, send, origin);
        let value: Value = api.get("/echo").await.unwrap().json().unwrap();
        assert_eq!(
            comparable(&value),
            reference(&format!("auth-{label}-echo"))["result"]
        );
        let response = api.get("/auth").await.unwrap();
        assert_eq!(
            response.status() as u64,
            reference(&format!("auth-{label}-challenge"))["status"]
                .as_u64()
                .unwrap()
        );
        if response.ok() {
            assert_eq!(
                comparable(&response.json::<Value>().unwrap()),
                serde_json::from_str::<Value>(
                    reference(&format!("auth-{label}-challenge"))["body"]
                        .as_str()
                        .unwrap()
                )
                .unwrap()
            );
        }
    }
    let api = auth(
        &base,
        ApiCredentialsSend::Unauthorized,
        Some(base.to_uppercase()),
    );
    let binary: Vec<u8> = (0..4097).map(|n| (n % 256) as u8).collect();
    let before = state.auth_calls.load(Ordering::SeqCst);
    let value: Value = api
        .fetch_with(
            "POST",
            "/auth",
            ApiRequestOptions {
                body: Some(binary.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(value["payload"], json!(binary));
    assert_eq!(state.auth_calls.load(Ordering::SeqCst) - before, 2);
    for path in ["/redirect/307", "/redirect/308", "/auth"] {
        let value: Value = api
            .fetch_with(
                "POST",
                path,
                ApiRequestOptions {
                    multipart: Some(vec![MultipartField {
                        name: "file".into(),
                        bytes: b"multipart bytes".to_vec(),
                        filename: Some("test.bin".into()),
                        content_type: Some("application/octet-stream".into()),
                    }]),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .json()
            .unwrap();
        assert!(value["body"].as_str().unwrap().contains("multipart bytes"));
        assert!(value["body"]
            .as_str()
            .unwrap()
            .contains("filename=\"test.bin\""));
        assert!(value["content_type"]
            .as_str()
            .unwrap()
            .starts_with("multipart/form-data; boundary="));
    }
    let scoped = auth(&base, ApiCredentialsSend::Always, Some(base.clone()));
    let value: Value = scoped.get("/cross").await.unwrap().json().unwrap();
    assert!(value["authorization"].is_null());
    let value: Value = api
        .fetch_with(
            "GET",
            "/echo",
            ApiRequestOptions {
                headers: vec![("authorization".into(), "Bearer explicit".into())],
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(value["authorization"], "Bearer explicit");
    for origin in [
        "not a URL",
        "file:///x",
        "http://127.0.0.1/private",
        "http://user:pass@127.0.0.1",
    ] {
        assert!(matches!(
            ApiClient::with_options(ApiClientOptions {
                credential_origin: Some(origin.into()),
                ..Default::default()
            }),
            Err(E2eError::Config(_))
        ));
    }
    stop(tasks);
}

async fn raw(
    resets: usize,
    truncated_body: bool,
) -> (String, Arc<AtomicUsize>, tokio::task::AbortHandle) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let count = counter.fetch_add(1, Ordering::SeqCst);
            let mut buffer = [0; 8192];
            let _ = socket.read(&mut buffer).await;
            if count < resets {
                socket.shutdown().await.ok();
                continue;
            }
            let reply = if truncated_body {
                b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nSet-Cookie: body=1; Path=/\r\nConnection: close\r\n\r\npartial".as_slice()
            } else {
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".as_slice()
            };
            socket.write_all(reply).await.ok();
            socket.shutdown().await.ok();
        }
    });
    (base, calls, task.abort_handle())
}
#[tokio::test]
async fn reset_retries_share_deadline_and_body_failures_are_not_retried() {
    let (base, calls, stop) = raw(1, false).await;
    assert_eq!(
        client(&base)
            .fetch_with(
                "GET",
                "/",
                ApiRequestOptions {
                    max_retries: 1,
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .text(),
        "ok"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst) as u64,
        reference("retry-reset")["requests"].as_u64().unwrap()
    );
    stop.abort();
    let (base, calls, stop) = raw(usize::MAX, false).await;
    assert!(matches!(
        client(&base)
            .fetch_with(
                "GET",
                "/",
                ApiRequestOptions {
                    max_retries: 100,
                    timeout: Some(Duration::from_millis(70)),
                    ..Default::default()
                }
            )
            .await,
        Err(E2eError::Timeout(_, _))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    stop.abort();
    let (base, calls, stop) = raw(usize::MAX, false).await;
    let api = client(&base);
    let (result, ()) = tokio::join!(
        api.fetch_with(
            "GET",
            "/",
            ApiRequestOptions {
                max_retries: 100,
                timeout: Some(Duration::ZERO),
                ..Default::default()
            }
        ),
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            api.dispose();
        }
    );
    assert!(matches!(result, Err(E2eError::Cancelled(_))));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    stop.abort();
    let (base, calls, stop) = raw(usize::MAX, false).await;
    let token = CancellationToken::new();
    let api = client(&base);
    let cancel = token.clone();
    let (result, ()) = tokio::join!(
        api.fetch_with(
            "GET",
            "/",
            ApiRequestOptions {
                max_retries: 100,
                timeout: Some(Duration::ZERO),
                cancellation: Some(token),
                ..Default::default()
            }
        ),
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel.cancel();
        }
    );
    assert!(matches!(result, Err(E2eError::Cancelled(_))));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    stop.abort();
    let (base, calls, stop) = raw(0, true).await;
    let api = client(&base);
    assert!(matches!(
        api.fetch_with(
            "GET",
            "/",
            ApiRequestOptions {
                max_retries: 3,
                ..Default::default()
            }
        )
        .await,
        Err(E2eError::Http(_))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(api.cookie_header("/").unwrap().unwrap().contains("body=1"));
    stop.abort();
}

async fn browsers(base: &str) -> Vec<Browser> {
    let mut all = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let mut browser =
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap();
            browser.set_base_url(Some(base.into()));
            eprintln!(
                "API fidelity {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            all.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(all.len(), 2);
    }
    all
}
#[tokio::test]
async fn native_redirect_cookie_sync_errors_disposal_and_runner_budgets() {
    let (base, _state, tasks) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let api = context.request();
        assert!(api
            .fetch_with(
                "GET",
                "/loop",
                ApiRequestOptions {
                    max_redirects: Some(1),
                    ..Default::default()
                }
            )
            .await
            .is_err());
        assert!(context
            .cookies()
            .await
            .unwrap()
            .iter()
            .any(|cookie| cookie.name == "loop" && cookie.value == "1"));
        let value: Value = api
            .fetch_with(
                "POST",
                "/redirect/307",
                ApiRequestOptions {
                    body: Some(b"abc".to_vec()),
                    ..Default::default()
                },
            )
            .await
            .unwrap()
            .json()
            .unwrap();
        assert!(value["cookie"].as_str().unwrap().contains("hop=1"));
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        assert!(page
            .evaluate::<String>("document.cookie")
            .await
            .unwrap()
            .contains("hop=1"));
        let (body_url, calls, stop_body) = raw(0, true).await;
        let _stop_body = AbortOnDrop(stop_body);
        assert!(matches!(
            api.fetch_with(
                "GET",
                &body_url,
                ApiRequestOptions {
                    max_retries: 3,
                    ..Default::default()
                }
            )
            .await,
            Err(E2eError::Http(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(context
            .cookies()
            .await
            .unwrap()
            .iter()
            .any(|cookie| cookie.name == "body" && cookie.value == "1"));
        let future = api.fetch_with(
            "GET",
            "/slow",
            ApiRequestOptions {
                timeout: Some(Duration::ZERO),
                ..Default::default()
            },
        );
        let (result, _) = tokio::join!(future, async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            context.clone().close().await.unwrap();
        });
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let output = tempfile::tempdir().unwrap();
        let witnesses = Arc::new(Mutex::new(Vec::new()));
        let observed = witnesses.clone();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(850))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test_with_context(
                    "API retry enclosing deadline",
                    move |ctx| {
                        let observed = observed.clone();
                        async move {
                            let (base, calls, stop) = raw(usize::MAX, false).await;
                            observed.lock().unwrap().push(Arc::downgrade(&calls));
                            let _stop = AbortOnDrop(stop);
                            let result = ctx
                                .context
                                .request()
                                .fetch_with(
                                    "GET",
                                    &base,
                                    ApiRequestOptions {
                                        max_retries: 100,
                                        timeout: Some(Duration::ZERO),
                                        ..Default::default()
                                    },
                                )
                                .await;
                            result.map(|_| ())
                        }
                    },
                )],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(witnesses.lock().unwrap().len(), 2);
        tokio::time::timeout(Duration::from_secs(1), async {
            while witnesses
                .lock()
                .unwrap()
                .iter()
                .any(|weak| weak.upgrade().is_some())
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("retry fixtures release counters/listeners after body cancellation");
        browser.close().await.unwrap();
    }
    stop(tasks);
}
