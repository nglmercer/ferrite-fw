use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

async fn browsers() -> Vec<Browser> {
    let mut browsers = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .expect("installed engine must launch");
            eprintln!(
                "callback native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            browsers.push(browser);
        } else {
            eprintln!("callback browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{response::Html, routing::get, Router};
    let app=Router::new().fallback(get(||async {Html("<script>window.startupType=typeof contextDouble; window.startupResult=contextDouble(21); console.log('callback startup');</script><iframe src='/child'></iframe>")}));
    // Child documents must not recursively create unlimited frames.
    let app = app.route("/child", get(|| async { Html("<p>child</p>") }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}
struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
async fn wait_count(counter: &AtomicUsize, minimum: usize) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while counter.load(Ordering::SeqCst) < minimum {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("callback task must be reclaimed");
}

#[tokio::test]
async fn async_callbacks_errors_concurrency_navigation_and_named_removal() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.expose_function_async("double", |args| async move {
            Ok(json!(args[0].as_i64().unwrap() * 2))
        })
        .await
        .unwrap();
        assert_eq!(page.evaluate_value("double(21)").await.unwrap(), 42);
        assert!(matches!(
            page.expose_function("double", |_| json!(0)).await,
            Err(E2eError::Config(_))
        ));
        assert!(matches!(
            page.expose_function("__ferriteExpose", |_| json!(0)).await,
            Err(E2eError::Config(_))
        ));
        page.expose_function_async("failure", |_| async {
            Err(E2eError::Config("Rust says no".into()))
        })
        .await
        .unwrap();
        assert!(page
            .evaluate::<String>("failure().catch(e=>e.message)")
            .await
            .unwrap()
            .contains("Rust says no"));
        page.expose_function_async("panicFuture", |_| async {
            panic!("future panic");
            #[allow(unreachable_code)]
            Ok(Value::Null)
        })
        .await
        .unwrap();
        assert!(page
            .evaluate::<String>("panicFuture().catch(e=>e.message)")
            .await
            .unwrap()
            .contains("panicked"));
        page.expose_function("panicCreation", |_| panic!("creation panic"))
            .await
            .unwrap();
        assert!(page
            .evaluate::<String>("panicCreation().catch(e=>e.message)")
            .await
            .unwrap()
            .contains("panicked"));
        let dropped = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let (d, s) = (dropped.clone(), started.clone());
        page.expose_function_async("independent", move |args| {
            let (d, s) = (d.clone(), s.clone());
            async move {
                if args[0] == "slow" {
                    let _guard = Dropped(d);
                    s.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                }
                Ok(json!(42))
            }
        })
        .await
        .unwrap();
        page.evaluate_value("window.oldCall=independent('slow').catch(e=>e.message); true")
            .await
            .unwrap();
        wait_count(&started, 1).await;
        assert_eq!(
            page.evaluate_value("independent('fast')").await.unwrap(),
            42
        );
        page.remove_exposed_function("independent").await.unwrap();
        assert!(page
            .evaluate::<String>("oldCall")
            .await
            .unwrap()
            .contains("removed"));
        wait_count(&dropped, 1).await;
        page.remove_exposed_function("independent").await.unwrap();
        assert_eq!(page.evaluate_value("double(7)").await.unwrap(), 14);
        let (d, s) = (dropped.clone(), started.clone());
        page.expose_function_async("navigation", move |args| {
            let (d, s) = (d.clone(), s.clone());
            async move {
                if args[0] == 0 {
                    let _guard = Dropped(d);
                    s.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                }
                Ok(json!(args[0]))
            }
        })
        .await
        .unwrap();
        page.evaluate_value("navigation(0); true").await.unwrap();
        wait_count(&started, 2).await;
        page.goto("data:text/html,<p>new</p>").await.unwrap();
        wait_count(&dropped, 2).await;
        assert_eq!(page.evaluate_value("navigation(99)").await.unwrap(), 99);
        assert_eq!(
            page.evaluate_value("typeof independent").await.unwrap(),
            "undefined"
        );
        page.remove_exposed_function("double").await.unwrap();
        page.expose_function("double", |_| json!(55)).await.unwrap();
        page.goto("data:text/html,<p>again</p>").await.unwrap();
        assert_eq!(page.evaluate_value("double()").await.unwrap(), 55);
        let cancellation = CancellationToken::new();
        let (d, s) = (dropped.clone(), started.clone());
        page.with_cancellation(cancellation.clone())
            .expose_function_async("cancellable", move |_| {
                let (d, s) = (d.clone(), s.clone());
                async move {
                    let _guard = Dropped(d);
                    s.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    Ok(Value::Null)
                }
            })
            .await
            .unwrap();
        page.evaluate_value("window.cancelledCall=cancellable().catch(e=>e.message); true")
            .await
            .unwrap();
        wait_count(&started, 3).await;
        cancellation.cancel();
        assert!(page
            .evaluate::<String>("cancelledCall")
            .await
            .unwrap()
            .contains("canceled"));
        wait_count(&dropped, 3).await;
        assert_eq!(page.evaluate_value("double()").await.unwrap(), 55);
        page.remove_exposed_function("cancellable").await.unwrap();
        page.clear_exposed_functions().await;
        page.goto("data:text/html,<p>clear</p>").await.unwrap();
        assert_eq!(
            page.evaluate_value("typeof double").await.unwrap(),
            "undefined"
        );
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(matches!(
            page.with_cancellation(cancelled)
                .expose_function("cancelled", |_| json!(1))
                .await,
            Err(E2eError::Cancelled(_))
        ));
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn context_callbacks_startup_popups_isolation_and_binding_frames() {
    for browser in browsers().await {
        let (base, stop) = server().await;
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        context.set_default_timeout(Duration::ZERO);
        context
            .expose_function_async("contextDouble", |args| async move {
                Ok(json!(args[0].as_i64().unwrap() * 2))
            })
            .await
            .unwrap();
        assert_eq!(page.evaluate_value("contextDouble(21)").await.unwrap(), 42);
        let other = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let other_page = other.new_page().await.unwrap();
        assert_eq!(
            other_page
                .evaluate_value("typeof contextDouble")
                .await
                .unwrap(),
            "undefined"
        );
        context
            .add_init_script("window.initCallback = contextDouble(20);")
            .await
            .unwrap();
        let second = context.new_page().await.unwrap();
        let (page_registration, context_registration) = tokio::join!(
            page.expose_function("racingName", |_| json!("page")),
            context.expose_function("racingName", |_| json!("context"))
        );
        assert_ne!(
            page_registration.is_ok(),
            context_registration.is_ok(),
            "exactly one competing registration owns the name"
        );
        if context_registration.is_ok() {
            context.remove_exposed_function("racingName").await.unwrap();
        } else {
            page.remove_exposed_function("racingName").await.unwrap();
        }
        for page in [&page, &second] {
            page.goto(&base).await.unwrap();
            assert_eq!(
                page.evaluate_value("startupType").await.unwrap(),
                "function"
            );
            assert_eq!(page.evaluate_value("startupResult").await.unwrap(), 42);
        }
        assert_eq!(second.evaluate_value("initCallback").await.unwrap(), 40);
        assert!(matches!(
            page.expose_function("contextDouble", |_| json!(1)).await,
            Err(E2eError::Config(_))
        ));
        page.expose_function("pageOnly", |_| json!(1))
            .await
            .unwrap();
        assert!(matches!(
            context.expose_function("pageOnly", |_| json!(1)).await,
            Err(E2eError::Config(_))
        ));
        let popup_wait = page.wait_for_popup(Duration::from_secs(5));
        let popup_script = format!("window.open({}); true", json!(base));
        let popup_open = page.evaluate_value(&popup_script);
        let (popup, opened) = tokio::join!(popup_wait, popup_open);
        opened.unwrap();
        let popup = popup.unwrap();
        popup
            .wait_for_function("window.startupType !== undefined", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(
            popup.evaluate_value("startupType").await.unwrap(),
            "function"
        );
        assert_eq!(popup.evaluate_value("startupResult").await.unwrap(), 42);
        context.expose_binding("caller",|source,args|async move {
            Ok(json!({"context":source.context.id(),"page":source.page.target_id(),"frame":source.frame.id(),"url":source.frame.current_url().await?,"args":args}))
        }).await.unwrap();
        for page in [&page, &second, &popup] {
            let main = page.evaluate_value("caller('main')").await.unwrap();
            assert_eq!(main["page"], page.target_id());
            assert_eq!(main["context"], context.id().unwrap());
            assert_eq!(main["url"], base);
            let child = page
                .evaluate_value("frames[0].caller('child')")
                .await
                .unwrap();
            assert_eq!(child["page"], page.target_id());
            assert_ne!(child["frame"], main["frame"]);
            assert_eq!(child["url"], format!("{base}child"));
        }
        page.expose_binding("pageCaller", |source, _| async move {
            Ok(json!({"page":source.page.target_id(),"frame":source.frame.id()}))
        })
        .await
        .unwrap();
        assert_eq!(
            second.evaluate_value("typeof pageCaller").await.unwrap(),
            "undefined"
        );
        let main = page.evaluate_value("pageCaller()").await.unwrap();
        let child = page.evaluate_value("frames[0].pageCaller()").await.unwrap();
        assert_eq!(child["page"], page.target_id());
        assert_ne!(main["frame"], child["frame"]);
        page.remove_exposed_function("pageCaller").await.unwrap();
        context
            .remove_exposed_function("contextDouble")
            .await
            .unwrap();
        for page in [&page, &second, &popup] {
            assert_eq!(
                page.evaluate_value("typeof contextDouble").await.unwrap(),
                "undefined"
            );
            page.goto(&format!("{base}child")).await.unwrap();
            assert_eq!(
                page.evaluate_value("typeof contextDouble").await.unwrap(),
                "undefined"
            );
            assert_eq!(
                page.evaluate_value("caller(8)").await.unwrap()["args"],
                json!([8])
            );
        }
        let third = context.new_page().await.unwrap();
        third.goto(&format!("{base}child")).await.unwrap();
        assert_eq!(
            third.evaluate_value("typeof contextDouble").await.unwrap(),
            "undefined"
        );
        context.remove_exposed_function("caller").await.unwrap();
        context.close().await.unwrap();
        other.close().await.unwrap();
        stop.abort();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn callback_runner_budget_and_retries_cancel_each_attempt() {
    for browser in browsers().await {
        let started = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let (s, d) = (started.clone(), dropped.clone());
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .workers(1)
            .list_progress(false)
            .test_timeout(Duration::from_millis(650))
            .retries(1)
            .output_dir(output.path().to_str().unwrap())
            .run(
                &browser,
                vec![test(
                    "async callback follows enclosing budget",
                    move |page| {
                        let (s, d) = (s.clone(), d.clone());
                        async move {
                            // A zero protocol timeout cannot outlive the attempt.
                            let page = page.with_timeout(Duration::ZERO);
                            page.expose_function_async("never", move |_| {
                                let (s, d) = (s.clone(), d.clone());
                                async move {
                                    let _guard = Dropped(d);
                                    s.fetch_add(1, Ordering::SeqCst);
                                    std::future::pending::<()>().await;
                                    Ok(Value::Null)
                                }
                            })
                            .await?;
                            page.evaluate_value("never()").await?;
                            Ok(())
                        }
                    },
                )],
            )
            .await;
        assert!(!report.ok());
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(started.load(Ordering::SeqCst), 2);
        wait_count(&dropped, 2).await;
        assert!(
            browser.contexts().is_empty(),
            "retry cleanup disposes callback contexts"
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn callback_closure_releases_handlers_and_pending_work() {
    for browser in browsers().await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        let held = Arc::new(());
        let weak = Arc::downgrade(&held);
        let dropped = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(AtomicUsize::new(0));
        let (d, s) = (dropped.clone(), started.clone());
        context
            .expose_function_async("forever", move |_| {
                let (held, d, s) = (held.clone(), d.clone(), s.clone());
                async move {
                    let _held = held;
                    let _guard = Dropped(d);
                    s.fetch_add(1, Ordering::SeqCst);
                    std::future::pending::<()>().await;
                    Ok(Value::Null)
                }
            })
            .await
            .unwrap();
        page.evaluate_value("forever(); true").await.unwrap();
        wait_count(&started, 1).await;
        context.clone().close().await.unwrap();
        wait_count(&dropped, 1).await;
        tokio::time::timeout(Duration::from_secs(3), async {
            while weak.upgrade().is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("closed context releases callback captures");
        assert!(page.is_closed());
        assert!(matches!(
            context.expose_function("afterClose", |_| json!(1)).await,
            Err(E2eError::Cancelled(_))
        ));
        browser.close().await.unwrap();
    }
}
