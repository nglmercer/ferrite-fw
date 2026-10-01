//! Native route removal, concurrency and resource ownership regressions.
use ferrite_e2e::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

fn reference(name: &str) -> serde_json::Value {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/route-reference.json"
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
async fn fixture() -> (String, tokio::task::AbortHandle) {
    let app = axum::Router::new().fallback(axum::routing::get(|| async { "network" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, task.abort_handle())
}
async fn launch(kind: BrowserKind, base: &str) -> Option<Browser> {
    let path = match kind {
        BrowserKind::Chromium => find_chromium(None),
        BrowserKind::Firefox => find_firefox(None),
    };
    let Some(path) = path else {
        assert_ne!(
            std::env::var("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").as_deref(),
            Ok("1"),
            "missing {}",
            kind.name()
        );
        return None;
    };
    let mut browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
        .await
        .unwrap();
    browser.set_base_url(Some(base.into()));
    eprintln!(
        "route lifecycle {}: {}",
        kind.name(),
        browser.version().await.unwrap()
    );
    Some(browser)
}
async fn start(page: &Page, path: &str) {
    let expression = format!("window.routeResult = undefined; window.routeFetch = fetch({}).then(r => r.text()).then(v => window.routeResult = v, () => window.routeResult = 'failed'); null", serde_json::to_string(path).unwrap());
    page.evaluate::<serde_json::Value>(&expression)
        .await
        .unwrap();
}
async fn result(page: &Page) -> String {
    page.evaluate("window.routeFetch.then(() => window.routeResult)")
        .await
        .unwrap()
}
async fn reached(token: &CancellationToken) {
    tokio::time::timeout(Duration::from_secs(3), token.cancelled())
        .await
        .expect("handler started");
}
async fn gate(
    page: &Page,
    pattern: &str,
    error: bool,
) -> (CancellationToken, CancellationToken, std::sync::Weak<()>) {
    let started = CancellationToken::new();
    let release = CancellationToken::new();
    let witness = Arc::new(());
    let weak = Arc::downgrade(&witness);
    let first = started.clone();
    let second = release.clone();
    page.route_with_handler(pattern, move |_| {
        let started = first.clone();
        let release = second.clone();
        let witness = witness.clone();
        async move {
            let _witness = witness;
            started.cancel();
            release.cancelled().await;
            if error {
                Err(E2eError::Config("intentional route failure".into()))
            } else {
                Ok(RouteAction::fulfill(200, b"handled".to_vec(), "text/plain"))
            }
        }
    })
    .await
    .unwrap();
    (started, release, weak)
}

#[tokio::test]
async fn native_removal_preserves_active_calls_and_unrelated_dispatch() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let mut page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        page.set_timeout(Duration::from_secs(3));
        let (started, release, weak) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        // Registering another route cannot drop the suspended callback.
        page.route_with_handler("**/fast", |_| async {
            Ok(RouteAction::fulfill(200, b"fast".to_vec(), "text/plain"))
        })
        .await
        .unwrap();
        assert_eq!(
            page.evaluate::<String>("fetch('/fast').then(r => r.text())")
                .await
                .unwrap(),
            "fast"
        );
        assert_eq!(page.unroute("**/slow").await.unwrap(), 1);
        assert!(
            weak.upgrade().is_some(),
            "active future owns its callback state"
        );
        assert_eq!(
            page.evaluate::<String>("fetch('/slow').then(r => r.text())")
                .await
                .unwrap(),
            "network"
        );
        assert_eq!(
            result(&page).await,
            reference("remove-default")["result"].as_str().unwrap()
        );
        assert!(
            weak.upgrade().is_some(),
            "removal releases the request before finishing the callback"
        );
        release.cancel();
        tokio::time::timeout(Duration::from_secs(2), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        page.unroute_all().await.unwrap();
        // Re-arming during idle cleanup must not disable the new registration.
        for _ in 0..16 {
            page.route_with_handler("**/fast", |_| async {
                Ok(RouteAction::fulfill(200, b"again".to_vec(), "text/plain"))
            })
            .await
            .unwrap();
            assert_eq!(
                page.evaluate::<String>("fetch('/fast').then(r => r.text())")
                    .await
                    .unwrap_or_else(|error| panic!("{error}: {:?}", page.trace())),
                "again"
            );
            page.unroute_all().await.unwrap();
        }
        assert_eq!(
            page.evaluate::<String>("fetch('/fast').then(r => r.text())")
                .await
                .unwrap(),
            "network"
        );
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_wait_ignore_errors_cancel_and_panics() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let (started, release, _) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        let waiting_page = page.clone();
        let wait = tokio::spawn(async move {
            waiting_page
                .unroute_all_with(
                    UnrouteOptions::default()
                        .behavior(UnrouteBehavior::Wait)
                        .timeout(Duration::ZERO),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!wait.is_finished());
        release.cancel();
        assert_eq!(wait.await.unwrap().unwrap(), 1);
        assert_eq!(result(&page).await, "handled");
        for behavior in [
            UnrouteBehavior::Default,
            UnrouteBehavior::IgnoreErrors,
            UnrouteBehavior::Cancel,
        ] {
            let (started, release, weak) = gate(&page, "**/slow", true).await;
            start(&page, "/slow").await;
            reached(&started).await;
            let before = page
                .trace()
                .iter()
                .filter(|entry| entry.kind == "route")
                .count();
            assert_eq!(
                page.unroute_all_with(UnrouteOptions::default().behavior(behavior))
                    .await
                    .unwrap(),
                1
            );
            if behavior != UnrouteBehavior::Cancel {
                release.cancel();
            }
            assert_eq!(
                result(&page).await,
                if behavior == UnrouteBehavior::Cancel {
                    "failed"
                } else {
                    "network"
                }
            );
            tokio::time::timeout(Duration::from_secs(2), async {
                while weak.upgrade().is_some() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let after = page
                .trace()
                .iter()
                .filter(|entry| entry.kind == "route")
                .count();
            assert_eq!(
                after - before,
                usize::from(behavior == UnrouteBehavior::Default)
            );
            tokio::time::timeout(Duration::from_secs(2), async {
                while weak.upgrade().is_some() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        // Creation and polling panics must abort their own request, not the pump.
        page.route_with_handler(
            "**/creation",
            |_| -> std::future::Ready<E2eResult<RouteAction>> { panic!("creation panic") },
        )
        .await
        .unwrap();
        page.route_with_handler("**/poll", |_| async {
            panic!("poll panic");
            #[allow(unreachable_code)]
            Ok(RouteAction::Continue)
        })
        .await
        .unwrap();
        for path in ["/creation", "/poll"] {
            start(&page, path).await;
            assert_eq!(result(&page).await, "failed");
        }
        assert_eq!(
            page.evaluate::<String>("fetch('/other').then(r => r.text())")
                .await
                .unwrap(),
            "network"
        );
        page.unroute_all().await.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_removal_budgets_cancellation_and_page_disposal() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let mut page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let (started, release, _) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        page.set_timeout(Duration::from_millis(50));
        assert!(matches!(
            page.unroute_all_with(UnrouteOptions::default().behavior(UnrouteBehavior::Wait))
                .await,
            Err(E2eError::Timeout(_, _))
        ));
        page.set_timeout(Duration::from_secs(3));
        release.cancel();
        assert_eq!(result(&page).await, "handled");
        let (started, release, _) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let waiting_page = page.clone();
        let wait = tokio::spawn(async move {
            waiting_page
                .unroute_all_with(
                    UnrouteOptions::default()
                        .behavior(UnrouteBehavior::Wait)
                        .timeout(Duration::ZERO)
                        .cancellation(token),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!wait.is_finished());
        cancel.cancel();
        assert!(matches!(wait.await.unwrap(), Err(E2eError::Cancelled(_))));
        release.cancel();
        assert_eq!(result(&page).await, "handled");
        let (started, _release, weak) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        let waiting_page = page.clone();
        let wait = tokio::spawn(async move {
            waiting_page
                .unroute_all_with(
                    UnrouteOptions::default()
                        .behavior(UnrouteBehavior::Wait)
                        .timeout(Duration::ZERO),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        page.close().await.unwrap();
        assert!(matches!(wait.await.unwrap(), Err(E2eError::Cancelled(_))));
        tokio::time::timeout(Duration::from_secs(2), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let remote = browser.new_page().await.unwrap();
        remote.goto("/").await.unwrap();
        let (started, _release, weak) = gate(&remote, "**/slow", false).await;
        start(&remote, "/slow").await;
        reached(&started).await;
        if let Some(connection) = browser.cdp() {
            connection
                .call(
                    None,
                    "Target.closeTarget",
                    serde_json::json!({ "targetId": remote.target_id() }),
                    Duration::from_secs(3),
                )
                .await
                .unwrap();
        } else {
            browser
                .bidi()
                .unwrap()
                .call(
                    "browsingContext.close",
                    serde_json::json!({ "context": remote.target_id() }),
                    Duration::from_secs(3),
                )
                .await
                .unwrap();
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while !remote.is_closed() || weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_context_removal_and_concurrent_hit_limits() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        context
            .route_with_handler_times("**/once", 1, move |_| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(80)).await;
                    Ok(RouteAction::fulfill(200, b"once".to_vec(), "text/plain"))
                }
            })
            .await
            .unwrap();
        let a = context.new_page().await.unwrap();
        let b = context.new_page().await.unwrap();
        a.goto("/").await.unwrap();
        b.goto("/").await.unwrap();
        let (left, right) = tokio::join!(
            a.evaluate::<String>("fetch('/once').then(r => r.text())"),
            b.evaluate::<String>("fetch('/once').then(r => r.text())")
        );
        let mut results = vec![left.unwrap(), right.unwrap()];
        results.sort();
        assert_eq!(
            serde_json::json!(results),
            reference("concurrent-times")["results"]
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        context.unroute_all().await.unwrap();
        let fallbacks = Arc::new(AtomicUsize::new(0));
        let counter = fallbacks.clone();
        a.route_with_handler_times("**/fallback", 1, move |_| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(RouteAction::Fallback)
            }
        })
        .await
        .unwrap();
        a.route_with_handler("**/fallback", |_| async {
            Ok(RouteAction::fulfill(200, b"lower".to_vec(), "text/plain"))
        })
        .await
        .unwrap();
        let values: Vec<String> = a.evaluate("Promise.all([fetch('/fallback').then(r => r.text()), fetch('/fallback').then(r => r.text())])").await.unwrap();
        assert_eq!(
            serde_json::json!(values),
            reference("fallback-times")["results"]
        );
        assert_eq!(
            fallbacks.load(Ordering::SeqCst) as u64,
            reference("fallback-times")["count"].as_u64().unwrap()
        );
        a.unroute_all().await.unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let release = CancellationToken::new();
        let gate = release.clone();
        let witness = Arc::new(());
        let weak = Arc::downgrade(&witness);
        context
            .route_matching(&UrlMatcher::exact("/slow"), move |_| {
                let tx = tx.clone();
                let gate = gate.clone();
                let witness = witness.clone();
                async move {
                    let _witness = witness;
                    tx.send(()).unwrap();
                    gate.cancelled().await;
                    Ok(RouteAction::fulfill(200, b"context".to_vec(), "text/plain"))
                }
            })
            .await
            .unwrap();
        start(&a, "/slow").await;
        start(&b, "/slow").await;
        for _ in 0..2 {
            tokio::time::timeout(Duration::from_secs(3), rx.recv())
                .await
                .unwrap()
                .unwrap();
        }
        let ctx = context.clone();
        let wait = tokio::spawn(async move {
            ctx.unroute_matching_with(
                &UrlMatcher::exact("/slow"),
                UnrouteOptions::default().behavior(UnrouteBehavior::Wait),
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(!wait.is_finished());
        for page in [&a, &b] {
            assert_eq!(
                page.evaluate::<String>("fetch('/slow').then(r => r.text())")
                    .await
                    .unwrap(),
                "network"
            );
        }
        release.cancel();
        assert_eq!(wait.await.unwrap().unwrap(), 1);
        assert_eq!(result(&a).await, "context");
        assert_eq!(result(&b).await, "context");
        let future = context.new_page().await.unwrap();
        future.goto("/").await.unwrap();
        assert_eq!(
            future
                .evaluate::<String>("fetch('/slow').then(r => r.text())")
                .await
                .unwrap(),
            "network"
        );
        context.close().await.unwrap();
        assert!(weak.upgrade().is_none());
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_route_runner_retries_context_close_and_disconnect() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let witnesses = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = witnesses.clone();
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(900))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test("route removal enclosing budget", move |page| {
                    let captured = captured.clone();
                    async move {
                        page.goto("/").await?;
                        let (started, _release, weak) = gate(&page, "**/slow", false).await;
                        captured.lock().unwrap().push(weak);
                        start(&page, "/slow").await;
                        reached(&started).await;
                        page.unroute_all_with(
                            UnrouteOptions::default()
                                .behavior(UnrouteBehavior::Wait)
                                .timeout(Duration::ZERO),
                        )
                        .await?;
                        Ok(())
                    }
                })],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(witnesses.lock().unwrap().len(), 2);
        assert!(
            witnesses
                .lock()
                .unwrap()
                .iter()
                .all(|weak| weak.upgrade().is_none()),
            "each retry releases its suspended route future"
        );
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let witness = Arc::new(());
        let weak = Arc::downgrade(&witness);
        let owner = context.clone();
        let started = CancellationToken::new();
        let signal = started.clone();
        context
            .route_with_handler("**/slow", move |_| {
                let witness = witness.clone();
                let owner = owner.clone();
                let signal = signal.clone();
                async move {
                    let _keep = (witness, owner);
                    signal.cancel();
                    std::future::pending::<E2eResult<RouteAction>>().await
                }
            })
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        start(&page, "/slow").await;
        reached(&started).await;
        let scoped = context.clone();
        let wait = tokio::spawn(async move {
            scoped
                .unroute_all_with(
                    UnrouteOptions::default()
                        .behavior(UnrouteBehavior::Wait)
                        .timeout(Duration::ZERO),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        context.close().await.unwrap();
        assert!(matches!(wait.await.unwrap(), Err(E2eError::Cancelled(_))));
        assert!(weak.upgrade().is_none());
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let (started, _release, weak) = gate(&page, "**/slow", false).await;
        start(&page, "/slow").await;
        reached(&started).await;
        let waiting_page = page.clone();
        let wait = tokio::spawn(async move {
            waiting_page
                .unroute_all_with(
                    UnrouteOptions::default()
                        .behavior(UnrouteBehavior::Wait)
                        .timeout(Duration::ZERO),
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(40)).await;
        if let Some(connection) = browser.cdp() {
            connection.close();
        } else {
            browser.bidi().unwrap().close();
        }
        assert!(matches!(
            wait.await.unwrap(),
            Err(E2eError::Disconnected(_))
        ));
        tokio::time::timeout(Duration::from_secs(2), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_dropped_route_startup_releases_callbacks_requests_and_allows_retry() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        for context_registration in [false, true] {
            let context = browser
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            let mut page = context.new_page().await.unwrap();
            page.goto("/").await.unwrap();
            page.set_timeout(Duration::from_secs(3));
            let witness = Arc::new(());
            let weak = Arc::downgrade(&witness);
            let callback = move |_| {
                let owned = witness.clone();
                async move {
                    drop(owned);
                    Ok(RouteAction::fulfill(
                        200,
                        b"abandoned".to_vec(),
                        "text/plain",
                    ))
                }
            };
            let mut setup: std::pin::Pin<
                Box<dyn std::future::Future<Output = E2eResult<()>> + '_>,
            > = if context_registration {
                Box::pin(context.route_with_handler("**/abandoned", callback))
            } else {
                Box::pin(page.route_with_handler("**/abandoned", callback))
            };
            // The uncontended registration reaches its first native response await.
            // Drop it before polling that response, without cancelling the live page.
            assert!(futures::poll!(&mut setup).is_pending());
            drop(setup);
            assert!(
                weak.upgrade().is_none(),
                "abandoned callback must be released"
            );
            start(&page, "/abandoned").await;
            assert_eq!(
                result(&page).await,
                "network",
                "abandoned interception must not leave the request paused"
            );
            page.route_with_handler("**/installed", |_| async {
                Ok(RouteAction::fulfill(200, b"retried".to_vec(), "text/plain"))
            })
            .await
            .unwrap();
            start(&page, "/installed").await;
            assert_eq!(result(&page).await, "retried");
            context.close().await.unwrap();
        }
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_rule_replacement_drop_validation_and_empty_context_lifecycle() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        for context_registration in [false, true] {
            let context = browser
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            let mut page = context.new_page().await.unwrap();
            page.goto("/").await.unwrap();
            page.set_timeout(Duration::from_secs(3));
            let rules = vec![RouteRule::fulfill(
                "**/abandoned-rule",
                200,
                b"abandoned".to_vec(),
                "text/plain",
            )];
            let mut setup: std::pin::Pin<
                Box<dyn std::future::Future<Output = E2eResult<()>> + '_>,
            > = if context_registration {
                Box::pin(context.route(rules))
            } else {
                Box::pin(page.route(rules))
            };
            assert!(futures::poll!(&mut setup).is_pending());
            drop(setup);
            start(&page, "/abandoned-rule").await;
            assert_eq!(result(&page).await, "network");
            let rules = vec![RouteRule::fulfill(
                "**/kept-rule",
                200,
                b"kept".to_vec(),
                "text/plain",
            )];
            if context_registration {
                context.route(rules).await.unwrap();
            } else {
                page.route(rules).await.unwrap();
            }
            // Rejected replacement must preserve the accepted profile on live
            // pages, and must not affect later pages created in a context.
            let invalid = vec![RouteRule::abort("[")];
            let outcome = if context_registration {
                context.route(invalid).await
            } else {
                page.route(invalid).await
            };
            assert!(matches!(outcome, Err(E2eError::Config(_))));
            start(&page, "/kept-rule").await;
            assert_eq!(result(&page).await, "kept");
            if context_registration {
                let future = context.new_page().await.unwrap();
                future.goto("/").await.unwrap();
                start(&future, "/kept-rule").await;
                assert_eq!(result(&future).await, "kept");
            }
            let retained = context.clone();
            context.close().await.unwrap();
            assert!(matches!(
                retained.route(Vec::new()).await,
                Err(E2eError::Cancelled(_))
            ));
        }
        let empty = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        if kind == BrowserKind::Firefox {
            // Engine capability validation also applies before a context has pages.
            let rule =
                RouteRule::continue_with("**", Some(format!("{base}override")), None, None, None);
            assert!(matches!(
                empty.route(vec![rule]).await,
                Err(E2eError::Config(_))
            ));
        }
        if let Some(connection) = browser.cdp() {
            connection.close();
        } else {
            browser.bidi().unwrap().close();
        }
        // Wait for actual transport loss, rather than treating close enqueue as EOF.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        loop {
            match empty.route(Vec::new()).await {
                Err(E2eError::Disconnected(_)) => break,
                Ok(()) if tokio::time::Instant::now() < deadline => tokio::task::yield_now().await,
                outcome => panic!("empty context must observe disconnect: {outcome:?}"),
            }
        }
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn native_page_routes_cover_new_descendants_without_intercepting_sibling_pages() {
    let (base, stop) = fixture().await;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let Some(browser) = launch(kind, &base).await else {
            continue;
        };
        let parent = browser.new_page().await.unwrap();
        parent.goto("/").await.unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        parent
            .route_matching(&UrlMatcher::exact("/descendant-fetch"), move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                async { Ok(RouteAction::fulfill(200, b"owned".to_vec(), "text/plain")) }
            })
            .await
            .unwrap();
        parent.evaluate::<serde_json::Value>("new Promise(resolve => {const child=document.createElement('iframe'); child.src='/child'; child.onload=()=>resolve(true); document.body.append(child);})").await.unwrap();
        let child = parent
            .document_frames()
            .await
            .unwrap()
            .into_iter()
            .find(|frame| frame.parent_id().is_some())
            .unwrap();
        let body = tokio::time::timeout(
            Duration::from_secs(4),
            child.evaluate_value("fetch('/descendant-fetch').then(response=>response.text())"),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(body, serde_json::json!("owned"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let sibling = parent.context().unwrap().new_page().await.unwrap();
        sibling.goto("/").await.unwrap();
        let body: String = sibling
            .evaluate("fetch('/descendant-fetch').then(response=>response.text())")
            .await
            .unwrap();
        assert_eq!(body, "network");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        parent.unroute_all().await.unwrap();
        let body = tokio::time::timeout(
            Duration::from_secs(4),
            child.evaluate_value("fetch('/descendant-fetch').then(response=>response.text())"),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(body, serde_json::json!("network"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        browser.close().await.unwrap();
    }
    stop.abort();
}
