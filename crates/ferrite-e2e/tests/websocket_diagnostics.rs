use ferrite_e2e::*;
use serde_json::json;
use std::time::Duration;

fn options() -> OperationOptions {
    OperationOptions {
        timeout: Some(Duration::from_secs(5)),
        cancellation: None,
    }
}
async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            result.push(
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap(),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
async fn server() -> (
    String,
    tokio::task::AbortHandle,
    std::sync::Arc<tokio::sync::Notify>,
) {
    use axum::{
        extract::{State, WebSocketUpgrade},
        response::Html,
        routing::get,
        Router,
    };
    use std::sync::Arc;
    let reject = Arc::new(tokio::sync::Notify::new());
    let app = Router::new()
        .route("/", get(|| async { Html("<h1>sockets</h1>") }))
        .route(
            "/socket",
            get(|upgrade: WebSocketUpgrade| async {
                upgrade.on_upgrade(|mut socket| async move {
                    while let Some(Ok(message)) = socket.recv().await {
                        if socket.send(message).await.is_err() {
                            break;
                        }
                    }
                })
            }),
        )
        .route(
            "/reject",
            get(
                |State(reject): State<Arc<tokio::sync::Notify>>| async move {
                    reject.notified().await;
                    axum::http::StatusCode::BAD_REQUEST
                },
            ),
        )
        .with_state(reject.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, task.abort_handle(), reject)
}

async fn connect(page: &Page, url: &str, count: usize) -> Vec<WebSocketSnapshot> {
    let script=format!("(async()=>{{window.sockets=Array.from({{length:{count}}},()=>new WebSocket({}));await Promise.all(sockets.map(s=>new Promise((r,j)=>{{s.onopen=r;s.onerror=j}})));}})()",json!(url));
    let matcher = UrlMatcher::exact(url);
    let (created, connected) = tokio::join!(
        page.wait_for_websocket(&matcher, options()),
        page.evaluate_value(&script)
    );
    connected.unwrap();
    let created = created.unwrap();
    assert!(!created.socket_id.is_empty());
    assert_eq!(created.direction, WebSocketDirection::Created);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let sockets = page.websocket_diagnostics().unwrap().sockets;
            if sockets.len() == count {
                return sockets;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn native_identity_text_binary_caps_and_real_close() {
    for browser in browsers().await {
        let (base, shutdown, _reject) = server().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let url = format!("{}socket", base.replace("http:", "ws:"));
        if browser.kind() == BrowserKind::Firefox {
            assert!(page
                .websocket_diagnostics()
                .unwrap_err()
                .to_string()
                .contains("not supported on Firefox"));
            assert!(page.websocket_snapshot("missing").is_err());
            assert!(page
                .wait_for_websocket(&UrlMatcher::exact(&url), options())
                .await
                .is_err());
            assert!(page
                .wait_for_websocket_event("missing", WebSocketDirection::Received, options())
                .await
                .is_err());
            page.close().await.unwrap();
            browser.close().await.unwrap();
            shutdown.abort();
            continue;
        }
        let sockets = connect(&page, &url, 2).await;
        assert_ne!(sockets[0].socket_id, sockets[1].socket_id);
        assert_eq!(sockets[0].url, sockets[1].url);
        let id = &sockets[0].socket_id;
        let (sent, received, send) = tokio::join!(
            page.wait_for_websocket_event(id, WebSocketDirection::Sent, options()),
            page.wait_for_websocket_event(id, WebSocketDirection::Received, options()),
            page.evaluate_value("sockets[0].send('雪<&')")
        );
        send.unwrap();
        let sent = sent.unwrap();
        let received = received.unwrap();
        assert_eq!(sent.opcode, Some(1));
        assert_eq!(received.opcode, Some(1));
        assert_eq!(received.payload_bytes().unwrap(), "雪<&".as_bytes());
        assert_eq!(sent.socket_id, received.socket_id);
        let (binary, send) = tokio::join!(
            page.wait_for_websocket_event(id, WebSocketDirection::Received, options()),
            page.evaluate_value("sockets[0].send(new Uint8Array([0,255,240,128,1]))")
        );
        send.unwrap();
        let binary = binary.unwrap();
        assert_eq!(binary.opcode, Some(2));
        assert_eq!(binary.payload_bytes().unwrap(), vec![0, 255, 240, 128, 1]);
        let (isolated, send) = tokio::join!(
            page.wait_for_websocket_event(
                id,
                WebSocketDirection::Received,
                OperationOptions {
                    timeout: Some(Duration::from_millis(80)),
                    cancellation: None
                }
            ),
            page.evaluate_value("sockets[1].send('other socket')")
        );
        send.unwrap();
        assert!(matches!(isolated, Err(E2eError::Timeout(..))));
        let (large, send) = tokio::join!(
            page.wait_for_websocket_event(id, WebSocketDirection::Received, options()),
            page.evaluate_value("sockets[0].send('x'.repeat(32768))")
        );
        send.unwrap();
        let large = large.unwrap();
        assert!(large.payload_truncated);
        assert!(large.payload_bytes().is_err());
        assert!(
            page.websocket_snapshot(id)
                .unwrap()
                .unwrap()
                .history_truncated
        );
        let (closed, close) = tokio::join!(
            page.wait_for_websocket_event(id, WebSocketDirection::Closed, options()),
            page.evaluate_value("new Promise(r=>{sockets[0].onclose=r;sockets[0].close()})")
        );
        close.unwrap();
        assert_eq!(closed.unwrap().direction, WebSocketDirection::Closed);
        assert_eq!(
            page.websocket_snapshot(id).unwrap().unwrap().is_closed(),
            Some(true)
        );
        assert!(matches!(
            page.wait_for_websocket_event(id, WebSocketDirection::Received, options())
                .await,
            Err(E2eError::Network { .. })
        ));
        assert_eq!(
            page.wait_for_websocket_event(id, WebSocketDirection::Closed, options())
                .await
                .unwrap()
                .socket_id,
            *id
        );
        let long_url = format!("{url}?query={}", "x".repeat(5000));
        let script = format!("window.longSocket=new WebSocket({}); true", json!(long_url));
        let any_url = UrlMatcher::glob("**").unwrap();
        let (truncated, trigger) = tokio::join!(
            page.wait_for_websocket(&any_url, options()),
            page.evaluate_value(&script)
        );
        trigger.unwrap();
        assert!(
            matches!(truncated, Err(E2eError::Network { message, .. }) if message.contains("URL truncated"))
        );
        assert!(page
            .websocket_diagnostics()
            .unwrap()
            .sockets
            .iter()
            .any(|socket| socket.url_truncated));
        let copied = page.websocket_diagnostics().unwrap();
        page.close().await.unwrap();
        assert_eq!(
            page.websocket_snapshot(id).unwrap().unwrap().is_closed(),
            Some(true)
        );
        assert_eq!(
            page.websocket_snapshot(&sockets[1].socket_id)
                .unwrap()
                .unwrap()
                .is_closed(),
            None
        );
        assert!(!copied.events.is_empty());
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn native_error_timeout_cancel_disposal_and_enclosing_step() {
    for browser in browsers().await {
        if browser.kind() == BrowserKind::Firefox {
            browser.close().await.unwrap();
            continue;
        }
        let (base, shutdown, reject) = server().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let url = format!("{}reject", base.replace("http:", "ws:"));
        let matcher = UrlMatcher::exact(&url);
        let script = format!("window.bad=new WebSocket({})", json!(url));
        let (created, trigger) = tokio::join!(
            page.wait_for_websocket(&matcher, options()),
            page.evaluate_value(&script)
        );
        trigger.unwrap();
        let id = created.unwrap().socket_id;
        let (error, ()) = tokio::join!(
            page.wait_for_websocket_event(&id, WebSocketDirection::Error, options()),
            async {
                reject.notify_one();
            }
        );
        let error = error.unwrap();
        assert!(!error.error.as_ref().unwrap().is_empty());
        let snapshot = page.websocket_snapshot(&id).unwrap().unwrap();
        assert!(snapshot.last_error.is_some());
        assert!(page
            .wait_for_websocket_event(&id, WebSocketDirection::Received, options())
            .await
            .is_err());
        let missing = UrlMatcher::exact("ws://never/created");
        let error = page
            .wait_for_websocket(
                &missing,
                OperationOptions {
                    timeout: Some(Duration::from_millis(20)),
                    cancellation: None,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Timeout(..)));
        let token = CancellationToken::new();
        let (result, ()) = tokio::join!(
            page.wait_for_websocket(
                &missing,
                OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: Some(token.clone())
                }
            ),
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                token.cancel_with_reason("socket caller canceled");
            }
        );
        assert!(
            matches!(result,Err(E2eError::Cancelled(reason)) if reason=="socket caller canceled")
        );
        let error = page
            .step_with(
                "socket wait",
                StepOptions::default().timeout(Duration::from_millis(20)),
                |_| async {
                    page.wait_for_websocket(
                        &missing,
                        OperationOptions {
                            timeout: Some(Duration::ZERO),
                            cancellation: None,
                        },
                    )
                    .await
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Timeout(..)));
        let (result, close) = tokio::join!(page.wait_for_websocket(&missing, options()), async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            page.close().await
        });
        close.unwrap();
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn transport_disconnect_and_retry_scopes_do_not_fabricate_close() {
    for browser in browsers().await {
        if browser.kind() == BrowserKind::Firefox {
            browser.close().await.unwrap();
            continue;
        }
        let (base, shutdown, _reject) = server().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let url = format!("{}socket", base.replace("http:", "ws:"));
        connect(&page, &url, 1).await;
        let root = tempfile::tempdir().unwrap();
        let target = url.clone();
        let navigation = base.clone();
        let report = Runner::default()
            .output_dir(root.path().display().to_string())
            .retries(1)
            .list_progress(false)
            .run(
                &browser,
                vec![test_with_context("socket retry", move |ctx| {
                    let target = target.clone();
                    let navigation = navigation.clone();
                    async move {
                        assert!(ctx.page.websocket_diagnostics()?.sockets.is_empty());
                        ctx.page.goto(&navigation).await?;
                        connect(&ctx.page, &target, 1).await;
                        if ctx.info.retry == 0 {
                            return Err(E2eError::Expect("retry".into()));
                        }
                        Ok(())
                    }
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_json());
        assert_eq!(report.results[0].attempts, 2);
        let missing = UrlMatcher::exact("ws://never/created");
        let (result, ()) = tokio::join!(page.wait_for_websocket(&missing, options()), async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            browser.cdp().unwrap().close();
        });
        assert!(
            matches!(result, Err(E2eError::Disconnected(_))),
            "{result:?}"
        );
        let snapshot = page.websocket_diagnostics().unwrap();
        assert!(snapshot.observation_lost.is_some());
        assert!(snapshot
            .sockets
            .iter()
            .all(|s| s.is_closed() != Some(false)));
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn adopted_popup_keeps_its_native_socket_scope_after_opener_closes() {
    for browser in browsers().await {
        if browser.kind() == BrowserKind::Firefox {
            browser.close().await.unwrap();
            continue;
        }
        let (base, shutdown, _reject) = server().await;
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let opener = context.new_page().await.unwrap();
        opener.goto(&base).await.unwrap();
        let script = format!("window.open({}); true", json!(base));
        let (popup, trigger) = tokio::join!(
            opener.wait_for_popup(Duration::from_secs(5)),
            opener.evaluate_value(&script)
        );
        trigger.unwrap();
        let popup = popup.unwrap();
        let url = format!("{}socket", base.replace("http:", "ws:"));
        let sockets = connect(&popup, &url, 1).await;
        assert!(opener.websocket_diagnostics().unwrap().sockets.is_empty());
        opener.close().await.unwrap();
        let (received, send) = tokio::join!(
            popup.wait_for_websocket_event(
                &sockets[0].socket_id,
                WebSocketDirection::Received,
                options()
            ),
            popup.evaluate_value("sockets[0].send('popup')")
        );
        send.unwrap();
        assert_eq!(received.unwrap().payload, "popup");
        let (wait, close) = tokio::join!(
            popup.wait_for_websocket_event(
                &sockets[0].socket_id,
                WebSocketDirection::Received,
                options()
            ),
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                context.close().await
            }
        );
        close.unwrap();
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        assert_eq!(
            popup
                .websocket_snapshot(&sockets[0].socket_id)
                .unwrap()
                .unwrap()
                .is_closed(),
            None
        );
        browser.close().await.unwrap();
        shutdown.abort();
    }
}
