//! Reliability, authentication, frames, runner limits and context-event regressions.
use ferrite_e2e::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
const SECOND: Duration = Duration::from_secs(5);
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{
        http::{HeaderMap, StatusCode},
        response::Html,
        routing::get,
        Router,
    };
    let app=Router::new()
        .route("/",get(||async {Html("<title>host</title><iframe src='/child'></iframe><a id='file' href='/download'>file</a>")}))
        .route("/child",get(||async {Html("<title>child</title><p id='value'>child</p>")}))
        .route("/delay",get(||async {tokio::time::sleep(Duration::from_millis(180)).await;"done"}))
        .route("/body",get(||async {axum::body::Body::from_stream(futures::stream::once(async {tokio::time::sleep(Duration::from_millis(180)).await;Ok::<_,std::io::Error>("body")}))}))
        .route("/login",get(||async {(StatusCode::FOUND,[("location","/inspect"),("set-cookie","session=secret; Path=/; HttpOnly; SameSite=Lax")],"")}))
        .route("/clear",get(||async {([("set-cookie","session=; Path=/; Max-Age=0")],"cleared")}))
        .route("/inspect",get(|headers:HeaderMap|async move {axum::Json(serde_json::json!({"cookie":headers.get("cookie").and_then(|v|v.to_str().ok()).unwrap_or(""),"auth":headers.get("authorization").and_then(|v|v.to_str().ok()).unwrap_or(""),"key":headers.get("x-key").and_then(|v|v.to_str().ok()).unwrap_or("")}))}))
        .route("/download",get(||async {([("content-disposition","attachment; filename=report.txt"),("content-type","application/octet-stream")],"download content")}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}
async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(executable) = executable {
            result.push(
                Browser::launch(
                    LaunchOptions::default()
                        .browser(kind)
                        .executable(executable),
                )
                .await
                .expect("installed browser must launch"),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(
            result.len(),
            2,
            "both Chromium and Firefox must launch successfully"
        );
    }
    result
}
#[tokio::test]
async fn api_deadlines_cancellation_disposal_and_authentication_state() {
    let (base, stop) = server().await;
    let client = ApiClient::with_options(ApiClientOptions {
        base_url: Some(base),
        timeout: Duration::from_millis(70),
        ..Default::default()
    })
    .unwrap();
    for path in ["delay", "body"] {
        assert!(
            matches!(client.get(path).await, Err(E2eError::Timeout(..))),
            "{path}"
        );
    }
    let unavailable = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable_url = format!("http://{}/", unavailable.local_addr().unwrap());
    drop(unavailable);
    let started = std::time::Instant::now();
    let retried = client
        .fetch_with(
            "GET",
            &unavailable_url,
            ApiRequestOptions {
                timeout: Some(Duration::from_millis(30)),
                max_retries: u32::MAX,
                ..Default::default()
            },
        )
        .await;
    assert!(matches!(retried, Err(E2eError::Timeout(..))));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        client
            .fetch_with(
                "GET",
                "delay",
                ApiRequestOptions {
                    timeout: Some(Duration::ZERO),
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .text(),
        "done"
    );
    let token = CancellationToken::new();
    let pending = client.with_cancellation(token.clone());
    let result = tokio::join!(
        pending.fetch_with(
            "GET",
            "delay",
            ApiRequestOptions {
                timeout: Some(Duration::ZERO),
                ..Default::default()
            }
        ),
        async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            token.cancel();
        }
    );
    assert!(matches!(result.0, Err(E2eError::Cancelled(_))));
    assert!(client
        .get("login")
        .await
        .unwrap()
        .text()
        .contains("session=secret"));
    let saved = client.storage_state().await.unwrap();
    assert_eq!(saved.cookies.len(), 1);
    assert!(saved.cookies[0].http_only);
    assert_eq!(saved.cookies[0].same_site.as_deref(), Some("Lax"));
    assert_eq!(saved.cookies[0].path.as_deref(), Some("/"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.json");
    client.save_storage_state(&path).await.unwrap();
    let restored = ApiClient::with_options(ApiClientOptions {
        base_url: client
            .get("inspect")
            .await
            .unwrap()
            .url()
            .rsplit_once('/')
            .map(|(base, _)| format!("{base}/")),
        storage_state: Some(saved.clone()),
        ..Default::default()
    })
    .unwrap();
    assert!(restored
        .get("inspect")
        .await
        .unwrap()
        .text()
        .contains("session=secret"));
    restored.get("clear").await.unwrap();
    assert!(restored.storage_state().await.unwrap().cookies.is_empty());
    restored.load_storage_state(&path).await.unwrap();
    assert_eq!(restored.storage_state().await.unwrap().cookies.len(), 1);
    let clone = client.clone();
    let (result, ()) = tokio::join!(
        clone.fetch_with(
            "GET",
            "delay",
            ApiRequestOptions {
                timeout: Some(Duration::ZERO),
                ..Default::default()
            }
        ),
        async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            client.dispose();
        }
    );
    assert!(matches!(result, Err(E2eError::Cancelled(_))));
    assert!(matches!(
        clone.storage_state().await,
        Err(E2eError::Cancelled(_))
    ));
    stop.abort();
}
#[tokio::test]
async fn browser_timeouts_cancellation_frames_and_context_auth() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        // API options are inherited even when Firefox cannot apply them to pages.
        let api_context = browser
            .new_context(ContextOptions {
                extra_http_headers: vec![("x-key".into(), "inherited".into())],
                http_credentials: Some(HttpCredentials {
                    username: "ada".into(),
                    password: "secret".into(),
                }),
                ..Default::default()
            })
            .await
            .unwrap();
        let api = api_context.request();
        let data: serde_json::Value = api.get("inspect").await.unwrap().json().unwrap();
        assert_eq!(data["key"], "inherited");
        assert_eq!(data["auth"], "Basic YWRhOnNlY3JldA==");
        assert!(api_context.pages().is_empty());
        api.get("login").await.unwrap();
        assert!(api_context
            .cookies()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "session"));
        let saved = tempfile::tempdir().unwrap();
        let saved_path = saved.path().join("auth.json");
        api.save_storage_state(&saved_path).await.unwrap();
        let restored = browser
            .new_context(ContextOptions {
                storage_state: Some(saved_path),
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(restored.pages().is_empty());
        let restored_page = restored.new_page().await.unwrap();
        restored_page.goto("/inspect").await.unwrap();
        assert!(restored_page
            .content()
            .await
            .unwrap()
            .contains("session=secret"));
        restored.close().await.unwrap();
        api.get("clear").await.unwrap();
        assert!(api_context.cookies().await.unwrap().is_empty());
        api_context.close().await.unwrap();
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let mut page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let sibling = page.clone();
        context.set_default_timeout(Duration::from_millis(60));
        assert_eq!(sibling.timeout(), Duration::from_millis(60));
        assert!(matches!(
            page.evaluate::<bool>("new Promise(()=>{})").await,
            Err(E2eError::Timeout(..))
        ));
        assert!(page
            .evaluate_with_options::<bool>(
                "new Promise(r=>setTimeout(()=>r(true),150))",
                OperationOptions {
                    timeout: Some(Duration::ZERO),
                    ..Default::default()
                }
            )
            .await
            .unwrap());
        page.wait_for_function(
            "new Promise(r=>setTimeout(()=>r(true),150))",
            Duration::ZERO,
        )
        .await
        .unwrap();
        assert!(matches!(
            page.wait_for_function("new Promise(()=>{})", Duration::from_millis(25))
                .await,
            Err(E2eError::Timeout(..))
        ));
        let token = CancellationToken::new();
        let canceled = page.with_cancellation(token.clone());
        let locator = canceled.locator("#missing");
        let (result, ()) = tokio::join!(locator.wait_for(Duration::ZERO), async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            token.cancel();
        });
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        page.set_timeout(SECOND);
        let frame = page
            .document_frames()
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.url().ends_with("/child"))
            .unwrap();
        assert_eq!(frame.page().target_id(), page.target_id());
        frame
            .set_content("<title>replaced</title><p id='new'>new</p>")
            .await
            .unwrap();
        frame.wait_for_selector("#new", SECOND).await.unwrap();
        frame
            .wait_for_function("document.title==='replaced'", SECOND)
            .await
            .unwrap();
        frame.wait_for_url("/child", SECOND).await.unwrap();
        frame.wait_for_load_state(LoadState::Load).await.unwrap();
        assert_eq!(page.title().await.unwrap(), "host");
        let cancel = CancellationToken::new();
        let waiter = page.with_cancellation(cancel.clone());
        let (result, ()) = tokio::join!(
            waiter.wait_for_event(PageEventKind::Closed, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                cancel.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let waiting = page.with_cancellation(CancellationToken::new());
        let (result, ()) =
            tokio::join!(waiting.wait_for_function("false", Duration::ZERO), async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                context.clone().close().await.unwrap();
            });
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        browser.close().await.unwrap();
    }
    stop.abort();
}
#[tokio::test]
async fn context_events_cover_pages_network_errors_popups_downloads_and_close() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let custom_downloads = tempfile::tempdir().unwrap();
        let context = browser
            .new_context(ContextOptions {
                downloads_path: if browser.kind() == BrowserKind::Chromium {
                    Some(custom_downloads.path().to_path_buf())
                } else {
                    None
                },
                ..Default::default()
            })
            .await
            .unwrap();
        let mut events = context.subscribe();
        let page = context.new_page().await.unwrap();
        assert!(matches!(
            events.recv().await.unwrap(),
            ContextEvent::Page(_)
        ));
        page.goto("/").await.unwrap();
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::Console, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                page.evaluate::<bool>("console.log('context console');true")
                    .await
                    .unwrap();
            }
        );
        assert_eq!(event.unwrap().page_id(), Some(page.target_id()));
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::PageError, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                page.evaluate::<bool>("setTimeout(()=>{throw new Error('context error')},20);true")
                    .await
                    .unwrap();
            }
        );
        assert_eq!(event.unwrap().page_id(), Some(page.target_id()));
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::Response, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                page.evaluate::<String>("fetch('/inspect').then(r=>r.text())")
                    .await
                    .unwrap();
            }
        );
        assert_eq!(event.unwrap().page_id(), Some(page.target_id()));
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::Page, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                page.evaluate::<bool>("window.open('/child');true")
                    .await
                    .unwrap();
            }
        );
        let ContextEvent::Page(popup) = event.unwrap() else {
            panic!("popup page")
        };
        assert_ne!(popup.target_id(), page.target_id());
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::Download, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                page.locator("#file").click().await.unwrap();
            }
        );
        let event = event.unwrap();
        assert_eq!(event.page_id(), Some(page.target_id()));
        let ContextEvent::PageEvent {
            event: PageEvent::Download(path),
            ..
        } = event
        else {
            panic!("download")
        };
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(path.parent(), Some(custom_downloads.path()));
        }
        assert_eq!(std::fs::read_to_string(path).unwrap(), "download content");
        let (event, ()) = tokio::join!(page.wait_for_event(PageEventKind::Closed, SECOND), async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            page.close().await.unwrap();
        });
        assert!(matches!(event.unwrap(), PageEvent::Closed));
        let (event, ()) = tokio::join!(
            context.wait_for_event(ContextEventKind::Closed, SECOND),
            async {
                tokio::time::sleep(Duration::from_millis(25)).await;
                context.clone().close().await.unwrap();
            }
        );
        assert!(matches!(event.unwrap(), ContextEvent::Closed));
        browser.close().await.unwrap();
    }
    stop.abort();
}
#[tokio::test]
async fn runner_limits_bound_setup_cleanup_and_keep_final_failure_counts() {
    let dir = tempfile::tempdir().unwrap();
    let config = E2eConfig {
        workers: 1,
        screenshot: "off".into(),
        ..Default::default()
    };
    for browser in browsers().await {
        let runner = Runner::from_config(&config)
            .output_dir(dir.path().display().to_string())
            .list_progress(false)
            .cleanup_timeout(Duration::from_millis(80));
        let bodies = Arc::new(AtomicUsize::new(0));
        let count = bodies.clone();
        let report = runner
            .clone()
            .max_failures(1)
            .run(
                &browser,
                vec![
                    test("a fails", |_| async {
                        Err(E2eError::Expect("failure".into()))
                    }),
                    test("b pending", move |_| {
                        let count = count.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }
                    }),
                ],
            )
            .await;
        assert_eq!(bodies.load(Ordering::SeqCst), 0);
        assert_eq!(
            report
                .results
                .iter()
                .filter(|r| r.status == TestStatus::Failed)
                .count(),
            1
        );
        assert_eq!(
            report
                .results
                .iter()
                .filter(|r| r.status == TestStatus::Skipped)
                .count(),
            1
        );
        let attempts = Arc::new(AtomicUsize::new(0));
        let count = attempts.clone();
        let report = runner
            .clone()
            .max_failures(1)
            .retries(1)
            .run(
                &browser,
                vec![
                    test("retry", move |_| {
                        let count = count.clone();
                        async move {
                            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                                Err(E2eError::Expect("first".into()))
                            } else {
                                Ok(())
                            }
                        }
                    }),
                    test("next", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.exit_code(), 0);
        let cleaned = Arc::new(AtomicUsize::new(0));
        let cleanup = cleaned.clone();
        let start = std::time::Instant::now();
        let report = runner
            .clone()
            .test_timeout(Duration::from_millis(1000))
            .before_each(|_| async { std::future::pending::<E2eResult<()>>().await })
            .after_each(move |_| {
                let cleanup = cleanup.clone();
                async move {
                    cleanup.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .run(&browser, vec![test("hung hook", |_| async { Ok(()) })])
            .await;
        assert_eq!(report.exit_code(), 1);
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(browser.contexts().is_empty());
        let partial = Arc::new(AtomicUsize::new(0));
        let counter = partial.clone();
        let report = runner
            .clone()
            .test_timeout(Duration::from_millis(1000))
            .fixture_with_teardown(
                || async { Ok(1_u16) },
                move |_| {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                },
            )
            .fixture(|| async { std::future::pending::<E2eResult<String>>().await })
            .run(
                &browser,
                vec![test("partial fixture setup", |_| async { Ok(()) })],
            )
            .await;
        assert_eq!(report.exit_code(), 1);
        assert_eq!(partial.load(Ordering::SeqCst), 1);
        assert!(browser.contexts().is_empty());
        // Expected failures do not consume maxFailures; active workers may finish.
        let report = runner
            .clone()
            .max_failures(1)
            .run(
                &browser,
                vec![
                    test("a expected", |_| async {
                        Err(E2eError::Expect("expected".into()))
                    })
                    .fail(),
                    test("b runs", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.exit_code(), 0);
        assert_eq!(
            report
                .results
                .iter()
                .filter(|r| r.status == TestStatus::Passed)
                .count(),
            1
        );
        let order = Arc::new(Mutex::new(Vec::new()));
        let first = order.clone();
        let second = order.clone();
        let report = runner
            .clone()
            .fixture_with_teardown(
                || async { Ok(42_u32) },
                move |_| {
                    let first = first.clone();
                    async move {
                        first.lock().unwrap().push("first");
                        Ok(())
                    }
                },
            )
            .fixture_with_teardown(
                || async { Ok(43_u64) },
                move |_| {
                    let second = second.clone();
                    async move {
                        second.lock().unwrap().push("second");
                        std::future::pending::<E2eResult<()>>().await
                    }
                },
            )
            .run(&browser, vec![test("cleanup", |_| async { Ok(()) })])
            .await;
        assert_eq!(report.exit_code(), 1);
        assert_eq!(*order.lock().unwrap(), vec!["second", "first"]);
        let teardown = Arc::new(AtomicUsize::new(0));
        let counter = teardown.clone();
        let start = std::time::Instant::now();
        let report = runner
            .clone()
            .global_timeout(Duration::from_millis(1000))
            .global_teardown(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .run(
                &browser,
                vec![
                    test("global hung", |_| async {
                        std::future::pending::<E2eResult<()>>().await
                    }),
                    test("pending", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.exit_code(), 1);
        assert!(report.results.iter().any(|r| r.name == "<run interrupted>"));
        assert_eq!(teardown.load(Ordering::SeqCst), 1);
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(browser.contexts().is_empty());
        let counter = teardown.clone();
        let report = runner
            .clone()
            .global_setup(|| async { panic!("setup panic") })
            .global_teardown(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .run(&browser, vec![])
            .await;
        assert_eq!(report.exit_code(), 1);
        assert_eq!(teardown.load(Ordering::SeqCst), 2);
        browser.close().await.unwrap();
    }
}
