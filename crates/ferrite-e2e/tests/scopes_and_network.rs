//! Fixture, nested suite and network lifecycle behavior on both native engines.
use ferrite_e2e::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
const WAIT: Duration = Duration::from_secs(10);
async fn browsers() -> Vec<Browser> {
    let mut browsers = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            eprintln!("validating {kind:?}: {}", path.display());
            browsers.push(
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .expect("installed browser launches"),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(
            browsers.len(),
            2,
            "this parity check requires Chromium and Firefox"
        );
    }
    browsers
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::default()
        .workers(1)
        .test_timeout(WAIT)
        .output_dir(dir.path().display().to_string())
        .list_progress(false)
}
#[derive(Debug)]
struct Account(usize);
#[derive(Debug)]
struct Session(usize);
#[derive(Debug)]
struct Data(usize);
struct Unused;

#[tokio::test]
async fn lazy_dependencies_worker_project_isolation_and_reverse_teardown() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let next = Arc::new(AtomicUsize::new(0));
        let a = order.clone();
        let counter = next.clone();
        let a_down = order.clone();
        let b = order.clone();
        let b_down = order.clone();
        let c = order.clone();
        let c_down = order.clone();
        let seen = Arc::new(Mutex::new(HashMap::new()));
        let mut tests = Vec::new();
        for name in ["first", "second"] {
            let seen = seen.clone();
            tests.push(
                test_with_context(name, move |ctx| {
                    let seen = seen.clone();
                    async move {
                        let account = ctx.get::<Account>().unwrap();
                        let session = ctx.get::<Session>().unwrap();
                        let data = ctx.get::<Data>().unwrap();
                        assert_eq!(account.0, session.0);
                        assert_eq!(session.0, data.0);
                        let key = (ctx.info.worker_index, ctx.info.project.clone());
                        let mut seen = seen.lock().unwrap();
                        let old = seen.entry(key).or_insert_with(|| account.clone());
                        assert!(Arc::ptr_eq(old, &account));
                        Ok(())
                    }
                })
                .fixture::<Data>(),
            );
        }
        // Intentionally register dependents before dependencies.
        let report = runner(&dir)
            .project(Project::new("alpha"))
            .project(Project::new("beta"))
            .repeat_each(2)
            .fixture_definition(
                Fixture::<Data>::new(move |map| {
                    let c = c.clone();
                    async move {
                        let value = map.require::<Session>()?.0;
                        c.lock().unwrap().push(("data+", value));
                        Ok(Data(value))
                    }
                })
                .dependency::<Session>()
                .teardown(move |value| {
                    let c = c_down.clone();
                    async move {
                        c.lock().unwrap().push(("data-", value.0));
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<Session>::new(move |map| {
                    let b = b.clone();
                    async move {
                        let value = map.require::<Account>()?.0;
                        b.lock().unwrap().push(("session+", value));
                        Ok(Session(value))
                    }
                })
                .dependency::<Account>()
                .scope(FixtureScope::Worker)
                .teardown(move |value| {
                    let b = b_down.clone();
                    async move {
                        b.lock().unwrap().push(("session-", value.0));
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<Account>::new(move |_| {
                    let a = a.clone();
                    let counter = counter.clone();
                    async move {
                        let value = counter.fetch_add(1, Ordering::SeqCst);
                        a.lock().unwrap().push(("account+", value));
                        Ok(Account(value))
                    }
                })
                .scope(FixtureScope::Worker)
                .teardown(move |value| {
                    let a = a_down.clone();
                    async move {
                        a.lock().unwrap().push(("account-", value.0));
                        Ok(())
                    }
                }),
            )
            .fixture_definition(Fixture::<Unused>::new(|_| async {
                panic!("unused fixture must remain lazy")
            }))
            .run(&browser, tests)
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(report.results.len(), 8);
        assert_eq!(next.load(Ordering::SeqCst), 2);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert!(!Arc::ptr_eq(
            seen.values().next().unwrap(),
            seen.values().nth(1).unwrap()
        ));
        let order = order.lock().unwrap().clone();
        for id in 0..2 {
            let lifecycle: Vec<_> = order
                .iter()
                .filter(|(_, value)| *value == id)
                .map(|(name, _)| *name)
                .collect();
            assert_eq!(
                lifecycle,
                [
                    "account+", "session+", "data+", "data-", "data+", "data-", "data+", "data-",
                    "data+", "data-", "session-", "account-"
                ]
            );
        }
        drop(order);
        drop(seen);
        browser.close().await.unwrap();
    }
}
fn suite(name: &str, order: Arc<Mutex<Vec<String>>>) -> Suite {
    let before_all = order.clone();
    let before_each = order.clone();
    let after_each = order.clone();
    let a = format!("{name} all+");
    let b = format!("{name} each+");
    let c = format!("{name} each-");
    let d = format!("{name} all-");
    Suite::new(name)
        .before_all(move || {
            let log = before_all.clone();
            let a = a.clone();
            async move {
                log.lock().unwrap().push(a);
                Ok(())
            }
        })
        .before_each(move |_| {
            let log = before_each.clone();
            let b = b.clone();
            async move {
                log.lock().unwrap().push(b);
                Ok(())
            }
        })
        .after_each(move |_| {
            let log = after_each.clone();
            let c = c.clone();
            async move {
                log.lock().unwrap().push(c);
                Ok(())
            }
        })
        .after_all(move || {
            let log = order.clone();
            let d = d.clone();
            async move {
                log.lock().unwrap().push(d);
                Ok(())
            }
        })
}
#[tokio::test]
async fn nested_suite_hooks_settings_and_unrelated_tests() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(Mutex::new(Vec::new()));
        let body_log = log.clone();
        let nested = suite("inner", log.clone())
            .timeout(Duration::from_secs(8))
            .tests(vec![test_with_context("one", move |ctx| {
                let log = body_log.clone();
                async move {
                    assert_eq!(ctx.info.timeout, Duration::from_secs(8));
                    assert!(ctx.info.tags.contains(&"outer-tag".into()));
                    assert_eq!(ctx.evaluate::<u32>("innerWidth").await?, 640);
                    log.lock().unwrap().push("body".into());
                    Ok(())
                }
            })]);
        let mut tests = suite("outer", log.clone())
            .timeout(Duration::from_secs(9))
            .tag("outer-tag")
            .context_options(ContextOptions::default().viewport(640, 480))
            .tests(nested);
        let unrelated = log.clone();
        tests.push(test("unrelated", move |_| {
            let log = unrelated.clone();
            async move {
                log.lock().unwrap().push("unrelated".into());
                Ok(())
            }
        }));
        tests.extend(
            Suite::new("skipped")
                .before_all(|| async { panic!("skipped suite setup") })
                .skip()
                .tests(vec![test("never", |_| async { panic!("skipped body") })]),
        );
        let report = runner(&dir).run(&browser, tests).await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(
            *log.lock().unwrap(),
            [
                "outer all+",
                "inner all+",
                "outer each+",
                "inner each+",
                "body",
                "inner each-",
                "outer each-",
                "inner all-",
                "outer all-",
                "unrelated"
            ]
        );
        assert_eq!(
            report
                .results
                .iter()
                .filter(|r| r.status == TestStatus::Skipped)
                .count(),
            1
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn suite_failure_cleanup_and_fixture_failures_are_reported() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let before = calls.clone();
        let after = calls.clone();
        let unrelated = calls.clone();
        let mut tests = Suite::new("broken")
            .before_all(move || {
                let calls = before.clone();
                async move {
                    calls.lock().unwrap().push("setup");
                    Err(E2eError::Expect("suite setup failed".into()))
                }
            })
            .after_all(move || {
                let calls = after.clone();
                async move {
                    calls.lock().unwrap().push("cleanup");
                    Ok(())
                }
            })
            .tests(vec![
                test("one", |_| async { panic!("body after failed setup") }),
                test("two", |_| async { panic!("body after failed setup") }),
            ]);
        tests.push(test("unrelated", move |_| {
            let calls = unrelated.clone();
            async move {
                calls.lock().unwrap().push("unrelated");
                Ok(())
            }
        }));
        let report = runner(&dir).run(&browser, tests).await;
        assert_eq!(
            report
                .results
                .iter()
                .filter(|r| r.status == TestStatus::Failed)
                .count(),
            2
        );
        assert_eq!(
            *calls.lock().unwrap(),
            ["setup", "cleanup", "setup", "cleanup", "unrelated"]
        );
        let counter = Arc::new(AtomicUsize::new(0));
        let count = counter.clone();
        let report = runner(&dir)
            .cleanup_timeout(Duration::from_secs(2))
            .fixture_definition(
                Fixture::<Account>::new(|_| async { Ok(Account(1)) })
                    .scope(FixtureScope::Worker)
                    .automatic(true)
                    .teardown(move |_| {
                        let count = count.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            Err(E2eError::Expect("worker teardown failed".into()))
                        }
                    }),
            )
            .run(&browser, vec![test("passes", |_| async { Ok(()) })])
            .await;
        assert_eq!(report.exit_code(), 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(report
            .results
            .iter()
            .any(|r| r.name.contains("worker fixtures")
                && r.error
                    .as_deref()
                    .unwrap()
                    .contains("worker teardown failed")));
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<Account>::new(|_| async { Ok(Account(0)) }).dependency::<Session>(),
            )
            .run(&browser, vec![])
            .await;
        assert!(report.results[0]
            .error
            .as_deref()
            .unwrap()
            .contains("missing dependency"));
        browser.close().await.unwrap();
    }
}

async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{http::StatusCode, response::Html, routing::get, Router};
    let app = Router::new()
        .route("/", get(|| async { Html("<title>network</title>") }))
        .route(
            "/error",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "HTTP error") }),
        )
        .route(
            "/redirect",
            get(|| async { (StatusCode::FOUND, [("location", "/error")], "") }),
        )
        .route(
            "/stream",
            get(|| async {
                axum::body::Body::from_stream(futures::stream::once(async {
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    Ok::<_, std::io::Error>("stream body")
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, task.abort_handle())
}
#[tokio::test]
async fn network_lifecycle_http_errors_redirects_transport_failure_and_context_forwarding() {
    let (base, stop) = server().await;
    let unavailable = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bad_url = format!("http://{}/missing", unavailable.local_addr().unwrap());
    drop(unavailable);
    for browser in browsers().await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        for path in ["error", "redirect", "stream"] {
            let mut page_events = page.subscribe();
            let mut context_events = context.subscribe();
            let url = format!("{base}{path}");
            let script = format!("fetch({url:?}).then(r=>r.text())");
            page.evaluate::<String>(&script).await.unwrap();
            let mut events = Vec::new();
            loop {
                let event = tokio::time::timeout(WAIT, page_events.recv())
                    .await
                    .unwrap()
                    .unwrap();
                let done = matches!(&event, PageEvent::RequestFinished(request) if request.url.ends_with(if path == "redirect" { "error" } else { path }));
                assert!(
                    !matches!(event, PageEvent::RequestFailed { .. }),
                    "HTTP errors are completed requests"
                );
                events.push(event);
                if done {
                    break;
                }
            }
            let end = events.last().unwrap();
            let PageEvent::RequestFinished(end) = end else {
                unreachable!()
            };
            assert_eq!(end.method, "GET");
            assert!(!end.request_id.is_empty());
            let response_pos = events.iter().position(|event| matches!(event, PageEvent::Response {request_id, url, status, ..} if request_id == &end.request_id && url == &end.url && *status == if path == "stream" {200} else {500})).expect("response before completion");
            assert!(response_pos < events.len() - 1);
            if path == "redirect" {
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(event, PageEvent::RequestFinished(_)))
                        .count(),
                    2
                );
            }
            loop {
                let event = tokio::time::timeout(WAIT, context_events.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if let ContextEvent::PageEvent {
                    page_id,
                    event: PageEvent::RequestFinished(request),
                } = event
                {
                    if request.url == end.url {
                        assert_eq!(page_id, page.target_id());
                        assert_eq!(request.request_id, end.request_id);
                        break;
                    }
                }
            }
        }
        let mut events = page.subscribe();
        let mut context_events = context.subscribe();
        page.evaluate::<bool>(&format!("fetch({bad_url:?}).catch(()=>{{}});true"))
            .await
            .unwrap();
        let failed = loop {
            let event = tokio::time::timeout(WAIT, events.recv())
                .await
                .unwrap()
                .unwrap();
            match event {
                PageEvent::RequestFailed {
                    request,
                    error_text,
                    ..
                } if request.url == bad_url => {
                    assert!(!error_text.is_empty());
                    assert_eq!(request.method, "GET");
                    assert!(!request.request_id.is_empty());
                    break request;
                }
                PageEvent::Response { url, .. }
                | PageEvent::RequestFinished(NetworkRequest { url, .. })
                    if url == bad_url =>
                {
                    panic!("transport failure is not a response or success")
                }
                _ => {}
            }
        };
        loop {
            let event = tokio::time::timeout(WAIT, context_events.recv())
                .await
                .unwrap()
                .unwrap();
            if let ContextEvent::PageEvent {
                page_id,
                event: PageEvent::RequestFailed { request, .. },
            } = event
            {
                if request.url == bad_url {
                    assert_eq!(page_id, page.target_id());
                    assert_eq!(request.request_id, failed.request_id);
                    break;
                }
            }
        }
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn parallel_workers_and_retries_keep_fixture_lifetimes_isolated() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let next = Arc::new(AtomicUsize::new(0));
        let setup = next.clone();
        let observed = Arc::new(Mutex::new(HashMap::<usize, Arc<Account>>::new()));
        let mut tests = Vec::new();
        for i in 0..6 {
            let observed = observed.clone();
            tests.push(test_with_context(format!("test {i}"), move |ctx| {
                let observed = observed.clone();
                async move {
                    let value = ctx.get::<Account>().unwrap();
                    {
                        let mut observed = observed.lock().unwrap();
                        let old = observed
                            .entry(ctx.info.worker_index)
                            .or_insert_with(|| value.clone());
                        assert!(Arc::ptr_eq(old, &value));
                    }
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    Ok(())
                }
            }));
        }
        let report = runner(&dir)
            .workers(2)
            .fixture_definition(
                Fixture::<Account>::new(move |_| {
                    let setup = setup.clone();
                    async move { Ok(Account(setup.fetch_add(1, Ordering::SeqCst))) }
                })
                .scope(FixtureScope::Worker)
                .automatic(true),
            )
            .run(&browser, tests)
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(next.load(Ordering::SeqCst), 2);
        let seen = observed.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert!(!Arc::ptr_eq(&seen[&0], &seen[&1]));
        drop(seen);

        let order = Arc::new(Mutex::new(Vec::new()));
        let setup = order.clone();
        let teardown = order.clone();
        let attempt_setup = order.clone();
        let attempt_down = order.clone();
        let body = order.clone();
        let log = order.clone();
        let tests = suite("retry suite", log)
            .retries(1)
            .tests(vec![test_with_context("retry", move |ctx| {
                let log = body.clone();
                async move {
                    assert!(ctx.get::<Account>().is_some());
                    assert!(ctx.get::<Data>().is_some());
                    log.lock().unwrap().push(format!("body {}", ctx.info.retry));
                    if ctx.info.retry == 0 {
                        Err(E2eError::Expect("retry me".into()))
                    } else {
                        Ok(())
                    }
                }
            })
            .fixture::<Data>()]);
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<Account>::new(move |_| {
                    let log = setup.clone();
                    async move {
                        log.lock().unwrap().push("worker+".into());
                        Ok(Account(1))
                    }
                })
                .scope(FixtureScope::Worker)
                .teardown(move |_| {
                    let log = teardown.clone();
                    async move {
                        log.lock().unwrap().push("worker-".into());
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<Data>::new(move |map| {
                    let log = attempt_setup.clone();
                    async move {
                        map.require::<Account>()?;
                        log.lock().unwrap().push("attempt+".into());
                        Ok(Data(1))
                    }
                })
                .dependency::<Account>()
                .teardown(move |_| {
                    let log = attempt_down.clone();
                    async move {
                        log.lock().unwrap().push("attempt-".into());
                        Ok(())
                    }
                }),
            )
            .run(&browser, tests)
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(report.results[0].attempts, 2);
        let lifecycle = order.lock().unwrap().clone();
        assert_eq!(
            lifecycle
                .iter()
                .filter(|item| item.as_str() == "worker+")
                .count(),
            2
        );
        assert_eq!(
            lifecycle
                .iter()
                .filter(|item| item.as_str() == "worker-")
                .count(),
            2
        );
        assert_eq!(
            lifecycle
                .iter()
                .filter(|item| item.as_str() == "retry suite all+")
                .count(),
            2
        );
        assert_eq!(
            lifecycle
                .iter()
                .filter(|item| item.as_str() == "retry suite all-")
                .count(),
            2
        );
        for item in ["attempt+", "attempt-"] {
            assert_eq!(
                lifecycle
                    .iter()
                    .filter(|seen| seen.as_str() == item)
                    .count(),
                2
            );
        }
        drop(lifecycle);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn automatic_worker_fixtures_precede_suite_setup_and_hook_failures_retry() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let order = Arc::new(Mutex::new(Vec::new()));
        let setup = order.clone();
        let teardown = order.clone();
        let before_all = order.clone();
        let after_all = order.clone();
        let body = order.clone();
        let tries = Arc::new(AtomicUsize::new(0));
        let tries_for_hook = tries.clone();
        let tests = Suite::new("hook retry")
            .retries(1)
            .before_all(move || {
                let order = before_all.clone();
                let tries = tries_for_hook.clone();
                async move {
                    order.lock().unwrap().push("all+");
                    if tries.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(E2eError::Expect("retry suite setup".into()))
                    } else {
                        Ok(())
                    }
                }
            })
            .after_all(move || {
                let order = after_all.clone();
                async move {
                    order.lock().unwrap().push("all-");
                    Ok(())
                }
            })
            .tests(vec![test_with_context("body", move |ctx| {
                let order = body.clone();
                async move {
                    assert_eq!(ctx.info.retry, 1);
                    assert!(ctx.get::<Account>().is_some());
                    order.lock().unwrap().push("body");
                    Ok(())
                }
            })]);
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<Account>::new(move |_| {
                    let order = setup.clone();
                    async move {
                        order.lock().unwrap().push("worker+");
                        Ok(Account(1))
                    }
                })
                .scope(FixtureScope::Worker)
                .automatic(true)
                .teardown(move |_| {
                    let order = teardown.clone();
                    async move {
                        order.lock().unwrap().push("worker-");
                        Ok(())
                    }
                }),
            )
            .run(&browser, tests)
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(
            *order.lock().unwrap(),
            ["worker+", "all+", "all-", "worker-", "worker+", "all+", "body", "all-", "worker-"]
        );
        browser.close().await.unwrap();
    }
}
