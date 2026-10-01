//! Typed per-hop identity, native frame metadata and body completion.
use ferrite_e2e::*;
use serde_json::json;
use std::time::{Duration, Instant};
const WAIT: Duration = Duration::from_secs(4);
fn options() -> OperationOptions {
    OperationOptions {
        timeout: Some(WAIT),
        cancellation: None,
    }
}
async fn wait_for_disconnection(events: &mut NetworkEvents) -> E2eResult<()> {
    loop {
        events.recv().await?;
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
            let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .expect("installed browser must launch");
            eprintln!(
                "network metadata native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("network metadata browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{
        body::Body,
        http::{Response, StatusCode},
        response::Html,
        routing::{get, post},
        Router,
    };
    fn streamed(stall: bool, fail: bool) -> Response<Body> {
        use futures::StreamExt;
        let first = futures::stream::once(async { Ok::<_, std::io::Error>(b"first".to_vec()) });
        let last = futures::stream::once(async move {
            if stall {
                std::future::pending::<()>().await;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            if fail {
                Err(std::io::Error::other("stream transport failure"))
            } else {
                Ok(b"-last".to_vec())
            }
        });
        Response::builder()
            .status(200)
            .header("content-type", "application/octet-stream")
            .header("content-length", if fail || stall { "1000" } else { "10" })
            .body(Body::from_stream(first.chain(last)))
            .unwrap()
    }
    let app = Router::new()
        .route(
            "/",
            get(|| async { Html("<p>root</p><iframe src='/frame'></iframe>") }),
        )
        .route("/frame", get(|| async { Html("<p>child</p>") }))
        .route(
            "/json",
            post(|| async {
                let mut response = Response::new(Body::from("{\"accepted\":true}"));
                for (name, value) in [
                    ("content-type", "application/json"),
                    ("x-repeat", "first"),
                    ("x-repeat", "second"),
                    ("set-cookie", "first=1; Path=/"),
                    ("set-cookie", "second=2; Path=/"),
                ] {
                    response
                        .headers_mut()
                        .append(name, axum::http::HeaderValue::from_static(value));
                }
                response
            }),
        )
        .route("/form", post(|| async { "form accepted" }))
        .route("/frame-data", get(|| async { "frame data" }))
        .route(
            "/redirect-one",
            get(|| async { (StatusCode::FOUND, [("location", "/redirect-two")], "one") }),
        )
        .route(
            "/redirect-two",
            get(|| async {
                (
                    StatusCode::TEMPORARY_REDIRECT,
                    [("location", "/final")],
                    "two",
                )
            }),
        )
        .route(
            "/final",
            get(|| async { (StatusCode::IM_A_TEAPOT, "final") }),
        )
        .route("/stream", get(|| async { streamed(false, false) }))
        .route("/fail", get(|| async { streamed(false, true) }))
        .route("/stall", get(|| async { streamed(true, false) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}

#[tokio::test]
async fn typed_native_metadata_post_data_frames_headers_and_legacy_waits() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let target = UrlMatcher::exact("/json");
        let (typed,legacy,trigger)=tokio::join!(
            page.wait_for_response_handle(&target,options()),
            page.wait_for_response("/json",WAIT),
            page.evaluate_value("fetch('/json',{method:'POST',headers:{'Content-Type':'application/json','X-Request':'native'},body:JSON.stringify({answer:42,text:'unicode ñ'})}).then(r=>r.text())")
        );
        trigger.unwrap();
        let response = typed.unwrap();
        let legacy = legacy.unwrap();
        assert_eq!(response.status(), 200);
        assert!(response.ok());
        response.finished().await.unwrap();
        let request = response.request();
        assert_eq!(request.native_id(), legacy.request_id.as_deref().unwrap());
        assert_eq!(request.method(), "POST");
        assert_eq!(request.url(), legacy.url);
        assert_eq!(request.page_id(), Some(page.target_id()));
        assert_eq!(
            request.page().timeout(),
            page.timeout(),
            "wait overrides do not replace the owning page defaults"
        );
        assert_eq!(request.page().target_id(), page.target_id());
        assert_eq!(request.header_value("x-ReQuest"), Some("native".into()));
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(request.resource_type(), Some("fetch"));
            assert_eq!(request.post_data_json().unwrap().unwrap()["answer"], 42);
        } else {
            assert!(
                request.post_data().is_none(),
                "Firefox absent post data stays unavailable"
            );
        }
        assert_eq!(request.is_navigation_request(), Some(false));
        assert_eq!(
            request.frame().await.unwrap().unwrap().id(),
            page.main_frame().await.unwrap().id()
        );

        assert_eq!(response.header_values("SET-cookie").len(), 2);
        assert_eq!(
            response.header_value("x-repeat"),
            Some("first, second".into())
        );
        assert!(response.header_value("set-cookie").unwrap().contains('\n'));
        assert!(response
            .headers_array()
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("content-type")
                && header.value == "application/json"));
        let snapshot = request.snapshot();
        assert!(snapshot.response_received);
        assert_eq!(snapshot.completion, RequestCompletion::Finished);
        let roundtrip: RequestSnapshot =
            serde_json::from_value(serde_json::to_value(snapshot).unwrap()).unwrap();
        assert_eq!(roundtrip.id, request.id());
        let form_target = UrlMatcher::exact("/form");
        let (form,trigger)=tokio::join!(page.wait_for_request_handle(&form_target,options()),page.evaluate_value("fetch('/form',{method:'POST',headers:{'content-type':'application/x-www-form-urlencoded'},body:'word=a+b&repeat=first&repeat=last&hash=%23'}).then(r=>r.text())"));
        trigger.unwrap();
        let form = form.unwrap();
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(
                form.post_data_json().unwrap(),
                Some(json!({"word":"a b","repeat":"last","hash":"#"}))
            );
        }
        let child = page
            .document_frames()
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.parent_id().is_some())
            .unwrap();
        let (child_request, trigger) = tokio::join!(
            page.wait_for_request_handle_where(
                |request| request.url().ends_with("/frame-data"),
                options()
            ),
            child.evaluate_value("fetch('/frame-data').then(r=>r.text())")
        );
        trigger.unwrap();
        let child_request = child_request.unwrap();
        assert_eq!(child_request.frame_id(), Some(child.id()));
        assert_eq!(
            child_request.frame().await.unwrap().unwrap().id(),
            child.id()
        );
        assert_eq!(child_request.page_id(), Some(page.target_id()));
        // Concurrent native requests keep their individual identities and URLs.
        let mut events = page.subscribe_network();
        page.evaluate_value(
            "Promise.all([0,1,2,3].map(i=>fetch('/frame-data?id='+i).then(r=>r.text()))); true",
        )
        .await
        .unwrap();
        let mut ids = std::collections::HashMap::new();
        let mut completed = std::collections::HashSet::new();
        while completed.len() < 4 {
            match events.recv_with_options(options()).await.unwrap() {
                NetworkEvent::Request(r) if r.url().contains("/frame-data?id=") => {
                    assert!(ids.insert(r.id().to_owned(), r.url().to_owned()).is_none());
                }
                NetworkEvent::Response(r) if r.url().contains("/frame-data?id=") => {
                    assert_eq!(ids.get(r.request().id()).unwrap(), r.url());
                }
                NetworkEvent::Finished(r) if r.url().contains("/frame-data?id=") => {
                    assert!(completed.insert(r.id().to_owned()));
                }
                _ => {}
            }
        }
        assert_eq!(ids.len(), 4);
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn redirect_hops_have_links_native_order_and_distinct_ids() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let mut events = page.subscribe_network();
        page.evaluate_value("fetch('/redirect-one').then(r=>r.text()); true")
            .await
            .unwrap();
        let mut ordered = Vec::new();
        loop {
            let event = events.recv_with_options(options()).await.unwrap();
            let (phase, request) = match event {
                NetworkEvent::Request(r) => ("request", r),
                NetworkEvent::Response(r) => ("response", r.request()),
                NetworkEvent::Finished(r) => ("finished", r),
                NetworkEvent::Failed(r) => panic!("unexpected failure: {r:?}"),
            };
            if !["redirect-one", "redirect-two", "final"]
                .iter()
                .any(|path| request.url().ends_with(path))
            {
                continue;
            }
            ordered.push((phase, request.url().rsplit('/').next().unwrap().to_owned()));
            if phase == "finished" && request.url().ends_with("/final") {
                break;
            }
        }
        assert_eq!(
            ordered,
            vec![
                ("request", "redirect-one".into()),
                ("response", "redirect-one".into()),
                ("finished", "redirect-one".into()),
                ("request", "redirect-two".into()),
                ("response", "redirect-two".into()),
                ("finished", "redirect-two".into()),
                ("request", "final".into()),
                ("response", "final".into()),
                ("finished", "final".into())
            ]
        );
        let hops: Vec<_> = page
            .network_requests()
            .into_iter()
            .filter(|r| {
                ["redirect-one", "redirect-two", "final"]
                    .iter()
                    .any(|path| r.url().ends_with(path))
            })
            .collect();
        assert_eq!(hops.len(), 3);
        assert_ne!(hops[0].id(), hops[1].id());
        assert_ne!(hops[1].id(), hops[2].id());
        assert_eq!(hops[0].redirected_to().unwrap().id(), hops[1].id());
        assert_eq!(hops[2].redirected_from().unwrap().id(), hops[1].id());
        assert_eq!(hops[1].redirected_from().unwrap().id(), hops[0].id());
        for (hop, status) in hops.iter().zip([302, 307, 418]) {
            let response = hop.response().unwrap();
            assert_eq!(response.status(), status);
            response.finished().await.unwrap();
        }
        assert!(!hops[2].response().unwrap().ok());
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn response_headers_completion_transport_failure_and_lifecycle_budgets() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let mut page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let target = UrlMatcher::exact("/stream");
        let started = Instant::now();
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&target, options()),
            page.evaluate_value("fetch('/stream').then(r=>r.text()).catch(()=>{}); true")
        );
        trigger.unwrap();
        let response = response.unwrap();
        assert!(started.elapsed() < Duration::from_millis(400));
        assert_eq!(response.request().completion(), RequestCompletion::Pending);
        assert!(matches!(
            response
                .finished_with_options(OperationOptions {
                    timeout: Some(Duration::from_millis(75)),
                    cancellation: None
                })
                .await,
            Err(E2eError::Timeout(75, _))
        ));
        let token = CancellationToken::new();
        let (wait, ()) = tokio::join!(
            response.finished_with_options(OperationOptions {
                timeout: Some(Duration::ZERO),
                cancellation: Some(token.clone())
            }),
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                token.cancel();
            }
        );
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        response
            .finished_with_options(OperationOptions {
                timeout: Some(Duration::ZERO),
                cancellation: None,
            })
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(450));
        let failure = UrlMatcher::exact("/fail");
        let (failed, trigger) = tokio::join!(
            page.wait_for_response_handle(&failure, options()),
            page.evaluate_value("fetch('/fail').then(r=>r.text()).catch(()=>{}); true")
        );
        trigger.unwrap();
        let failed = failed.unwrap();
        assert_eq!(failed.status(), 200);
        assert!(matches!(
            failed.finished().await,
            Err(E2eError::Network { .. })
        ));
        assert!(failed.request().failure().is_some());
        // A failed connection has a request identity but no fabricated Response.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let offline = format!("http://{}/offline", listener.local_addr().unwrap());
        drop(listener);
        let mut events = page.subscribe_network();
        page.evaluate_value(&format!(
            "fetch({}).catch(()=>{{}}); true",
            serde_json::to_string(&offline).unwrap()
        ))
        .await
        .unwrap();
        loop {
            if let NetworkEvent::Failed(request) =
                events.recv_with_options(options()).await.unwrap()
            {
                if request.url() == offline {
                    assert!(request.response().is_none());
                    assert!(request.failure().is_some());
                    break;
                }
            }
        }
        let before = page.timeout();
        page.set_timeout(Duration::from_millis(60));
        assert!(matches!(
            page.wait_for_response_handle(
                &UrlMatcher::contains("never"),
                OperationOptions::default()
            )
            .await,
            Err(E2eError::Timeout(60, _))
        ));
        page.set_timeout(before);
        let stall = UrlMatcher::exact("/stall");
        let (pending, trigger) = tokio::join!(
            page.wait_for_response_handle(&stall, options()),
            page.evaluate_value("fetch('/stall').then(r=>r.text()).catch(()=>{}); true")
        );
        trigger.unwrap();
        let pending = pending.unwrap();
        let mut subscription = page.subscribe_network();
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            subscription
                .recv_with_options(OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: Some(token)
                })
                .await,
            Err(E2eError::Cancelled(_))
        ));
        let (wait, event, closed) = tokio::join!(
            pending.finished_with_options(OperationOptions {
                timeout: Some(Duration::ZERO),
                cancellation: None
            }),
            subscription.recv(),
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                page.close().await
            }
        );
        closed.unwrap();
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        assert!(matches!(event, Err(E2eError::Cancelled(_))));
        response.finished().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn completion_runner_retries_and_native_transport_disconnection() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(700))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test(
                    "native response completion enclosing budget",
                    |page| async move {
                        page.goto("/").await?;
                        let target = UrlMatcher::exact("/stall");
                        let (response, trigger) = tokio::join!(
                            page.wait_for_response_handle(&target, options()),
                            page.evaluate_value(
                                "fetch('/stall').then(r=>r.text()).catch(()=>{}); true"
                            )
                        );
                        trigger?;
                        response?
                            .finished_with_options(OperationOptions {
                                timeout: Some(Duration::ZERO),
                                cancellation: None,
                            })
                            .await
                    },
                )],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempts, 2);
        assert!(report.results[0]
            .attempt_results
            .iter()
            .all(|attempt| !attempt.errors.is_empty()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let target = UrlMatcher::exact("/stall");
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&target, options()),
            page.evaluate_value("fetch('/stall').then(r=>r.text()).catch(()=>{}); true")
        );
        trigger.unwrap();
        let response = response.unwrap();
        let mut events = page.subscribe_network();
        let event_wait = wait_for_disconnection(&mut events);
        let (wait, event, ()) = tokio::join!(
            response.finished_with_options(OperationOptions {
                timeout: Some(Duration::ZERO),
                cancellation: None
            }),
            event_wait,
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if let Some(connection) = browser.cdp() {
                    connection.close();
                } else {
                    browser.bidi().unwrap().close();
                }
            }
        );
        assert!(matches!(wait, Err(E2eError::Disconnected(_))));
        assert!(matches!(event, Err(E2eError::Disconnected(_))));
        assert!(!browser
            .cdp()
            .map_or_else(|| browser.bidi().unwrap().is_open(), CdpConnection::is_open));
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn detached_frame_inflight_response_settles_and_keeps_original_identity() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let child = page
            .document_frames()
            .await
            .unwrap()
            .into_iter()
            .find(|frame| frame.parent_id().is_some())
            .unwrap();
        let target = UrlMatcher::exact("/stall");
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&target, options()),
            child.evaluate_value("fetch('/stall').then(r=>r.text()).catch(()=>{}); true")
        );
        trigger.unwrap();
        let response = response.unwrap();
        let request = response.request();
        let id = request.id().to_owned();
        assert_eq!(request.frame_id(), Some(child.id()));
        assert_eq!(request.completion(), RequestCompletion::Pending);
        let (finished, detached, removed) = tokio::join!(
            tokio::time::timeout(
                WAIT,
                response.finished_with_options(OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: None,
                })
            ),
            page.wait_for_event(PageEventKind::FrameDetached, WAIT),
            tokio::time::timeout(
                WAIT,
                page.evaluate_value("document.querySelector('iframe').remove(); true")
            )
        );
        eprintln!("detached request {}: finished={finished:?}, detached={detached:?}, removed={removed:?}, completion={:?}", browser.kind().name(), request.completion());
        let finished = finished
            .expect("detached-frame in-flight completion must settle with disabled timeout");
        let removed = removed.expect("native iframe removal must settle");
        removed.unwrap();
        assert!(
            matches!(detached.unwrap(), PageEvent::FrameDetached(frame) if frame.frame_id == child.id())
        );
        assert!(
            matches!(&finished, Err(E2eError::Network { .. }))
                || matches!(&finished, Err(E2eError::Config(reason)) if reason.contains("detached before request completion")),
            "native detached request must report failure or explicit loss of frame observation: {finished:?}"
        );
        assert!(matches!(
            request.completion(),
            RequestCompletion::Failed(_) | RequestCompletion::Unavailable(_)
        ));
        assert_eq!(request.id(), id);
        assert_eq!(request.frame_id(), Some(child.id()));
        assert!(request.frame().await.unwrap().is_none());
        assert!(response.finished().await.is_err());
        assert_eq!(page.evaluate_value("6 * 7").await.unwrap(), json!(42));
        browser.close().await.unwrap();
    }
    stop.abort();
}
