//! Captured response helpers use the original native exchange.
use ferrite_e2e::*;
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

#[tokio::test]
async fn captured_native_bytes_text_json_and_explicit_unavailability() {
    use axum::{routing::get, Router};
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    fn streamed(fail: bool) -> axum::body::Body {
        use futures::StreamExt;
        let first = futures::stream::once(async { Ok::<_, std::io::Error>(b"first".to_vec()) });
        let last = futures::stream::once(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            if fail {
                Err(std::io::Error::other("fixture interrupted"))
            } else {
                Ok::<_, std::io::Error>(b"last".to_vec())
            }
        });
        axum::body::Body::from_stream(first.chain(last))
    }
    let app = Router::new()
        .route("/", get(|| async { "fixture" }))
        .route("/stream", get(|| async { streamed(false) }))
        .route("/fail", get(|| async { streamed(true) }))
        .route("/binary", get(|| async { vec![0_u8, 255, 128, 65] }))
        .route(
            "/error",
            get(|| async {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "error response",
                )
            }),
        )
        .route(
            "/json",
            get(move || {
                let counted = counted.clone();
                async move {
                    counted.fetch_add(1, Ordering::SeqCst);
                    "{\"answer\":42,\"text\":\"ñ🦀\"}"
                }
            }),
        )
        .route("/empty", get(|| async { "" }))
        .route("/bad", get(|| async { "{invalid" }))
        .route("/large", get(|| async { "x".repeat(1024 * 1024 + 1) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut engines = 0;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(path) = path else { continue };
        engines += 1;
        let mut browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
            .await
            .unwrap();
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let options = || OperationOptions {
            timeout: Some(Duration::from_secs(5)),
            cancellation: None,
        };
        let matcher = UrlMatcher::exact("/json");
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&matcher, options()),
            page.evaluate_value("fetch('/json').then(r=>r.text())")
        );
        trigger.unwrap();
        let response = response.unwrap();
        assert_eq!(response.body_capture_state(), BodyCaptureState::NotCaptured);
        assert!(response.body().await.is_err());
        page.start_request_capture();
        let before = count.load(Ordering::SeqCst);
        let matcher = UrlMatcher::exact("/json");
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&matcher, options()),
            page.evaluate_value("fetch('/json').then(r=>r.text())")
        );
        trigger.unwrap();
        let response = response.unwrap();
        if kind == BrowserKind::Firefox {
            assert!(matches!(response.body().await, Err(E2eError::Config(_))));
            assert!(matches!(response.text().await, Err(E2eError::Config(_))));
            assert!(matches!(
                response.json::<serde_json::Value>().await,
                Err(E2eError::Config(_))
            ));
        } else {
            assert_eq!(
                response.json::<serde_json::Value>().await.unwrap(),
                json!({"answer":42,"text":"ñ🦀"})
            );
            assert_eq!(
                response.text().await.unwrap(),
                "{\"answer\":42,\"text\":\"ñ🦀\"}"
            );
            assert_eq!(
                response.body().await.unwrap(),
                response.text().await.unwrap().into_bytes()
            );
            assert_eq!(
                count.load(Ordering::SeqCst),
                before + 1,
                "helpers must not refetch"
            );
            for endpoint in ["/empty", "/bad", "/large"] {
                let matcher = UrlMatcher::exact(endpoint);
                let script = format!("fetch('{endpoint}').then(r=>r.text())");
                let (result, trigger) = tokio::join!(
                    page.wait_for_response_handle(&matcher, options()),
                    page.evaluate_value(&script)
                );
                trigger.unwrap();
                let result = result.unwrap();
                match endpoint {
                    "/empty" => assert!(result.body().await.unwrap().is_empty()),
                    "/bad" => assert!(matches!(
                        result.json::<serde_json::Value>().await,
                        Err(E2eError::Diagnostic { .. })
                    )),
                    _ => {
                        assert!(result.body().await.is_err());
                        assert_eq!(
                            result.body_capture_state(),
                            BodyCaptureState::Truncated { limit: 1024 * 1024 }
                        );
                    }
                }
            }
            for endpoint in ["/binary", "/error", "/fail"] {
                let matcher = UrlMatcher::exact(endpoint);
                let script =
                    format!("fetch('{endpoint}').then(r=>r.arrayBuffer()).catch(()=>{{}}); true");
                let (result, trigger) = tokio::join!(
                    page.wait_for_response_handle(&matcher, options()),
                    page.evaluate_value(&script)
                );
                trigger.unwrap();
                let result = result.unwrap();
                if endpoint == "/fail" {
                    assert!(result.finished().await.is_err());
                    assert!(matches!(
                        result.body_capture_state(),
                        BodyCaptureState::Failed(_)
                    ));
                    assert!(result.body().await.is_err());
                } else if endpoint == "/binary" {
                    assert_eq!(result.body().await.unwrap(), vec![0, 255, 128, 65]);
                    assert_eq!(result.text().await.unwrap(), "\0��A");
                } else {
                    assert_eq!(result.status(), 500);
                    assert_eq!(result.text().await.unwrap(), "error response");
                }
            }
            let matcher = UrlMatcher::exact("/stream");
            let (pending, trigger) = tokio::join!(
                page.wait_for_response_handle(&matcher, options()),
                page.evaluate_value("fetch('/stream').then(r=>r.text()); true")
            );
            trigger.unwrap();
            let pending = pending.unwrap();
            assert_eq!(pending.body_capture_state(), BodyCaptureState::Pending);
            assert!(matches!(
                pending
                    .body_with_options(OperationOptions {
                        timeout: Some(Duration::from_millis(30)),
                        cancellation: None
                    })
                    .await,
                Err(E2eError::Timeout(30, _))
            ));
            let token = CancellationToken::new();
            let (wait, ()) = tokio::join!(
                pending.body_with_options(OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: Some(token.clone())
                }),
                async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    token.cancel();
                }
            );
            assert!(matches!(wait, Err(E2eError::Cancelled(_))));
            page.stop_request_capture();
            assert!(matches!(
                pending.body_capture_state(),
                BodyCaptureState::Unavailable(_)
            ));
            assert!(pending.body().await.is_err());
            page.start_request_capture();
            tokio::time::sleep(Duration::from_millis(650)).await;
            assert!(matches!(
                pending.body_capture_state(),
                BodyCaptureState::Unavailable(_)
            ));
            page.stop_request_capture();
            page.close().await.unwrap();
            assert_eq!(
                response.json::<serde_json::Value>().await.unwrap()["answer"],
                42
            );
        }
        browser.close().await.unwrap();
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(engines, 2);
    }
    server.abort();
}
