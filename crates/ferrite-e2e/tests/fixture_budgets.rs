//! Explicit fixture limits, shared cleanup clocks and disposal after dropped waits.
use ferrite_e2e::*;
use futures::FutureExt;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

const BUDGET: Duration = Duration::from_secs(15);

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
                .unwrap();
            eprintln!(
                "fixture budgets {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}

fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::from_config(&E2eConfig {
        screenshot: "off".into(),
        ..Default::default()
    })
    .workers(1)
    .test_timeout(BUDGET)
    .cleanup_timeout(Duration::from_secs(3))
    .output_dir(dir.path().display().to_string())
    .list_progress(false)
}

async fn native_context_ids(browser: &Browser) -> Vec<String> {
    let ids = if let Some(cdp) = browser.cdp() {
        cdp.call(None, "Target.getBrowserContexts", json!({}), BUDGET)
            .await
            .unwrap()["browserContextIds"]
            .clone()
    } else {
        let value = browser
            .bidi()
            .unwrap()
            .call("browser.getUserContexts", json!({}), BUDGET)
            .await
            .unwrap();
        json!(value["userContexts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|context| context["userContext"].as_str().unwrap())
            .collect::<Vec<_>>())
    };
    let mut ids = ids
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

async fn native_target_ids(browser: &Browser) -> Vec<String> {
    if let Some(cdp) = browser.cdp() {
        let value = cdp
            .call(None, "Target.getTargets", json!({}), BUDGET)
            .await
            .unwrap();
        return value["targetInfos"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| target["targetId"].as_str().unwrap().to_owned())
            .collect();
    }
    fn collect(context: &Value, ids: &mut Vec<String>) {
        ids.push(context["context"].as_str().unwrap().to_owned());
        for child in context["children"].as_array().into_iter().flatten() {
            collect(child, ids);
        }
    }
    let value = browser
        .bidi()
        .unwrap()
        .call("browsingContext.getTree", json!({}), BUDGET)
        .await
        .unwrap();
    let mut ids = Vec::new();
    for context in value["contexts"].as_array().unwrap() {
        collect(context, &mut ids);
    }
    ids
}

async fn released(browser: &Browser, expected: &[String]) {
    tokio::time::timeout(BUDGET, async {
        loop {
            if native_context_ids(browser).await == expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("native user contexts were not actually released");
}

#[tokio::test]
async fn native_dropped_context_page_and_convenience_close_waits_complete_once() {
    for browser in browsers().await {
        let before = native_context_ids(&browser).await;
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        let sibling = context.new_page().await.unwrap();
        let mut events = page.subscribe();
        // On this current-thread executor, the first poll spawns disposal and
        // then drops its wait before the spawned operation has been polled.
        assert!(context.clone().close().now_or_never().is_none());
        let (first, second) = tokio::time::timeout(BUDGET, async {
            tokio::join!(context.clone().close(), context.clone().close())
        })
        .await
        .unwrap();
        first.unwrap();
        second.unwrap();
        assert!(page.is_closed() && sibling.is_closed() && context.is_closed());
        assert_eq!(native_context_ids(&browser).await, before);
        let targets = native_target_ids(&browser).await;
        assert!(!targets
            .iter()
            .any(|id| id == page.target_id() || id == sibling.target_id()));
        let mut closed = 0;
        while let Ok(event) = events.try_recv() {
            if matches!(event, PageEvent::Closed) {
                closed += 1;
            }
        }
        assert_eq!(closed, 1);

        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let explicit = context.new_page().await.unwrap();
        assert!(explicit.close().now_or_never().is_none());
        tokio::time::timeout(BUDGET, explicit.close())
            .await
            .unwrap()
            .unwrap();
        assert!(explicit.is_closed());
        assert!(!native_target_ids(&browser)
            .await
            .iter()
            .any(|id| id == explicit.target_id()));
        // Explicit page closure must leave its context available.
        let next = context.new_page().await.unwrap();
        assert_eq!(next.evaluate::<Value>("6*7").await.unwrap(), json!(42));
        context.close().await.unwrap();

        let convenience = browser.new_page().await.unwrap();
        let owned = convenience.context().unwrap();
        assert!(convenience.close().now_or_never().is_none());
        let (first, second) = tokio::join!(convenience.close(), convenience.close());
        first.unwrap();
        second.unwrap();
        assert!(owned.is_closed());
        assert_eq!(native_context_ids(&browser).await, before);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_explicit_setup_limit_releases_completed_dependencies_after_failure() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/fixture-budget-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    assert_eq!(reference["cases"].as_array().unwrap().len(), 8);
    for browser in browsers().await {
        let before = native_context_ids(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(AtomicUsize::new(0));
        let count = cleanup.clone();
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(42) }).teardown(move |_| {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<u64>::new(|map| async move {
                    map.require::<u32>()?;
                    std::future::pending::<E2eResult<u64>>().await
                })
                .dependency::<u32>()
                .setup_timeout(Duration::from_millis(30)),
            )
            .run(
                &browser,
                vec![test("setup limit", |_| async {
                    panic!("body after failed setup")
                })
                .fixture::<u64>()],
            )
            .await;
        assert_eq!(report.failed(), 1, "{}", report.to_list());
        assert_eq!(cleanup.load(Ordering::SeqCst), 1);
        let attempt = &report.results[0].attempt_results[0];
        assert!(attempt
            .errors
            .iter()
            .any(|error| error.code == "FERRITE_E2E_TIMEOUT"
                && error.message.contains("u64")
                && error.phase == "test setup"));
        assert!(report.results[0].error.as_deref().unwrap().contains("30ms"));
        released(&browser, &before).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_shared_cleanup_exhaustion_reports_pending_phases_and_releases_native_context() {
    for browser in browsers().await {
        let before = native_context_ids(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(AtomicUsize::new(0));
        let count = cleanup.clone();
        let hook_count = cleanup.clone();
        let report = runner(&dir)
            .cleanup_timeout(Duration::from_millis(60))
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(42) }).teardown(move |_| {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<u64>::new(|_| async { Ok(1) })
                    .dependency::<u32>()
                    .teardown_timeout(Duration::ZERO)
                    .teardown(|_| async { std::future::pending::<E2eResult<()>>().await }),
            )
            .after_each(|_| async { std::future::pending::<E2eResult<()>>().await })
            .after_each(move |_| {
                let count = hook_count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .run(
                &browser,
                vec![test("shared cleanup", |_| async { Ok(()) }).fixture::<u64>()],
            )
            .await;
        assert_eq!(report.failed(), 1, "{}", report.to_list());
        assert_eq!(
            cleanup.load(Ordering::SeqCst),
            2,
            "ready hook and dependency must both execute"
        );
        let attempt = &report.results[0].attempt_results[0];
        for phase in [
            "after_each",
            "fixture teardown",
            "page close",
            "context close",
        ] {
            assert!(
                attempt
                    .errors
                    .iter()
                    .any(|error| error.phase == phase && error.code == "FERRITE_E2E_TIMEOUT"),
                "{phase}: {:?}",
                attempt.errors
            );
        }
        assert!(
            attempt.duration_ms < 3000,
            "shared budget must not restart for each cleanup"
        );
        assert_eq!(attempt.settings.as_ref().unwrap().cleanup_timeout_ms, 60);
        released(&browser, &before).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_zero_outer_cleanup_and_shorter_fixture_teardown_override() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let cleanup = Arc::new(AtomicUsize::new(0));
        let count = cleanup.clone();
        let report = runner(&dir)
            .cleanup_timeout(Duration::ZERO)
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(42) })
                    .setup_timeout(Duration::ZERO)
                    .teardown_timeout(Duration::ZERO)
                    .teardown(move |_| {
                        let count = count.clone();
                        async move {
                            tokio::time::sleep(Duration::from_millis(40)).await;
                            count.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }
                    }),
            )
            .fixture_definition(
                Fixture::<u64>::new(|_| async { Ok(1) })
                    .dependency::<u32>()
                    .teardown_timeout(Duration::from_millis(20))
                    .teardown(|_| async { std::future::pending::<E2eResult<()>>().await }),
            )
            .after_each(|_| async {
                tokio::time::sleep(Duration::from_millis(40)).await;
                Ok(())
            })
            .run(
                &browser,
                vec![test("local cleanup limit", |_| async { Ok(()) }).fixture::<u64>()],
            )
            .await;
        assert_eq!(report.failed(), 1, "{}", report.to_list());
        assert_eq!(cleanup.load(Ordering::SeqCst), 1);
        let attempt = &report.results[0].attempt_results[0];
        assert_eq!(attempt.errors.len(), 1, "{:?}", attempt.errors);
        assert_eq!(attempt.errors[0].phase, "fixture teardown");
        assert!(attempt.errors[0].message.contains("20ms"));
        assert_eq!(attempt.settings.as_ref().unwrap().cleanup_timeout_ms, 0);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_retry_rebuilds_worker_fixtures_and_keeps_dynamic_zero_and_soft_errors() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let setups = Arc::new(AtomicUsize::new(0));
        let cleanups = Arc::new(AtomicUsize::new(0));
        let count = setups.clone();
        let cleaned = cleanups.clone();
        let report = runner(&dir)
            .retries(1)
            .cleanup_timeout(Duration::from_millis(250))
            .fixture_definition(
                Fixture::<u32>::new(move |_| {
                    let count = count.clone();
                    async move { Ok(count.fetch_add(1, Ordering::SeqCst) as u32 + 1) }
                })
                .scope(FixtureScope::Worker)
                .setup_timeout(Duration::from_millis(50))
                .teardown(move |_| {
                    let count = cleaned.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<u64>::new(|map| async move {
                    map.require::<TestInfo>()?.set_timeout(Duration::ZERO);
                    tokio::time::sleep(Duration::from_millis(15)).await;
                    Ok(u64::from(*map.require::<u32>()?))
                })
                .dependency::<u32>()
                .dependency::<TestInfo>()
                .setup_timeout(Duration::from_millis(50)),
            )
            .after_each_with_context(ContextHook::new(|ctx| async move {
                if ctx.info.retry == 0 {
                    std::future::pending::<E2eResult<()>>().await
                } else {
                    Ok(())
                }
            }))
            .run(
                &browser,
                vec![
                    test_with_context("retry fixture cleanup", |ctx| async move {
                        assert_eq!(*ctx.require::<u64>()?, u64::from(ctx.info.retry) + 1);
                        assert_eq!(ctx.info.settings().timeout_ms, 0);
                        if ctx.info.retry == 0 {
                            ctx.info
                                .soft_asserts()
                                .check(Err(E2eError::Expect("first body mismatch".into())))?;
                        }
                        Ok(())
                    })
                    .fixture::<u64>(),
                ],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert!(report.results[0].flaky);
        assert_eq!(setups.load(Ordering::SeqCst), 2);
        assert_eq!(cleanups.load(Ordering::SeqCst), 2);
        let attempts = &report.results[0].attempt_results;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].soft_assertions.len(), 1);
        assert!(attempts[0]
            .errors
            .iter()
            .any(|error| error.phase == "after_each"));
        assert!(attempts[1].soft_assertions.is_empty());
        assert_eq!(attempts[1].settings.as_ref().unwrap().timeout_ms, 0);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_cancelled_setup_releases_completed_dependencies_and_native_context() {
    for browser in browsers().await {
        let before = native_context_ids(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let ready = Arc::new(tokio::sync::Notify::new());
        let entered = ready.clone();
        let cleanup = Arc::new(AtomicUsize::new(0));
        let count = cleanup.clone();
        let cancellation = CancellationToken::new();
        let interrupt = cancellation.clone();
        let signal = tokio::spawn(async move {
            ready.notified().await;
            interrupt.cancel_with_reason("fixture setup cancelled");
        });
        let report = runner(&dir)
            .with_cancellation(cancellation)
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(42) }).teardown(move |_| {
                    let count = count.clone();
                    async move {
                        count.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<u64>::new(move |map| {
                    let entered = entered.clone();
                    async move {
                        map.require::<u32>()?;
                        entered.notify_one();
                        std::future::pending::<E2eResult<u64>>().await
                    }
                })
                .dependency::<u32>()
                .setup_timeout(Duration::ZERO),
            )
            .run(
                &browser,
                vec![test("cancelled fixture setup", |_| async {
                    panic!("cancelled setup entered body")
                })
                .fixture::<u64>()],
            )
            .await;
        tokio::time::timeout(BUDGET, signal).await.unwrap().unwrap();
        assert_eq!(cleanup.load(Ordering::SeqCst), 1);
        let attempt = &report
            .results
            .iter()
            .find(|result| result.name == "cancelled fixture setup")
            .unwrap()
            .attempt_results[0];
        assert_eq!(attempt.status, AttemptStatus::Interrupted);
        assert!(attempt
            .errors
            .iter()
            .any(|error| error.code == "FERRITE_E2E_CANCELLED" && error.phase == "test setup"));
        released(&browser, &before).await;
        browser.close().await.unwrap();
    }
}
