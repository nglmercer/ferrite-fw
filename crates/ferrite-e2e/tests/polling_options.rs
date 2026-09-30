//! Generic polling timing, native lifecycle, soft-probe scopes and retry reports.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::Instant;

async fn browsers() -> Vec<Browser> {
    let mut browsers = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let owner = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .unwrap();
            eprintln!(
                "polling options {} {}",
                kind.name(),
                owner.version().await.unwrap()
            );
            browsers.push(owner);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
fn options() -> PollingOptions {
    PollingOptions::default()
        .timeout(Timeout::ms(1000))
        .intervals([Duration::from_millis(20), Duration::from_millis(40)])
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::from_config(&E2eConfig {
        screenshot: "off".into(),
        workers: 2,
        timeout_ms: 10000,
        cleanup_timeout_ms: 3000,
        reporter: "json,junit,html".into(),
        output_dir: dir.path().display().to_string(),
        ..Default::default()
    })
    .list_progress(false)
}
fn flat(steps: &[StepInfo]) -> Vec<&StepInfo> {
    steps
        .iter()
        .flat_map(|step| std::iter::once(step).chain(flat(&step.steps)))
        .collect()
}
fn mismatch(message: impl Into<String>) -> E2eResult<()> {
    Err(E2eError::Expect(message.into()))
}

async fn context_ids(browser: &Browser) -> Vec<String> {
    let value = if let Some(cdp) = browser.cdp() {
        cdp.call(
            None,
            "Target.getBrowserContexts",
            json!({}),
            Duration::from_secs(5),
        )
        .await
        .unwrap()["browserContextIds"]
            .clone()
    } else {
        let contexts = browser
            .bidi()
            .unwrap()
            .call("browser.getUserContexts", json!({}), Duration::from_secs(5))
            .await
            .unwrap();
        json!(contexts["userContexts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["userContext"].as_str().unwrap())
            .collect::<Vec<_>>())
    };
    let mut ids = value
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[tokio::test]
async fn pinned_cadence_and_native_local_blocks_preserve_immediate_and_last_interval_behavior() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/polling-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<h1>ready</h1>").await.unwrap();
        let mut times = Vec::new();
        let start = Instant::now();
        let captured = page.clone();
        let value = expect_poll_with(
            "native counter",
            &options().message("API eventually ready"),
            || {
                times.push(start.elapsed().as_millis());
                let ready = times.len() >= 5;
                let page = captured.clone();
                async move {
                    let value = page.evaluate::<u32>("42").await?;
                    Ok(ready.then_some(value))
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(value, 42);
        assert_eq!(times.len(), 5);
        assert!(times[0] < 30);
        for (index, gap) in times.windows(2).map(|w| w[1] - w[0]).enumerate() {
            assert!(gap >= if index == 0 { 18 } else { 38 }, "{times:?}");
        }
        let pinned = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "poll-cadence")
            .unwrap();
        assert_eq!(
            pinned["probeTimesMs"].as_array().unwrap().len(),
            times.len()
        );
        assert_eq!(pinned["result"]["outcome"], "passed");
        // The Rc must survive an actual native await, proving local-future support.
        let local = std::rc::Rc::new(std::cell::Cell::new(0));
        expect_to_pass_with("local block", &options(), || {
            let local = local.clone();
            let page = page.clone();
            async move {
                assert_eq!(page.evaluate::<u32>("42").await?, 42);
                local.set(local.get() + 1);
                if local.get() < 3 {
                    mismatch("not ready")
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(local.get(), 3);
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn soft_probes_retry_without_collection_and_only_final_failure_survives_a_test_retry() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .retries(1)
            .run(
                &browser,
                vec![
                    test_with_context("nested soft recovery", |ctx| async move {
                        ctx.page.set_content("<h1>ready</h1>").await?;
                        let soft = ctx.info.soft_asserts();
                        let mut calls = 0;
                        // The factory itself performs a synchronous soft check.
                        expect_to_pass_with(
                            "sync probe",
                            &options().message("factory scope"),
                            || {
                                calls += 1;
                                let result = soft.check_with_message(
                                    if calls < 3 {
                                        mismatch("factory mismatch")
                                    } else {
                                        Ok(())
                                    },
                                    "inner soft",
                                );
                                std::future::ready(result)
                            },
                        )
                        .await?;
                        assert_eq!(calls, 3);
                        assert!(soft.failures()?.is_empty());
                        assert!(ctx.info.errors().is_empty());
                        let mut calls = 0;
                        expect_to_pass_with("async probe", &options(), || {
                            calls += 1;
                            let ready = calls >= 3;
                            let page = ctx.page.clone();
                            let soft = soft.clone();
                            async move {
                                soft.run("probe assertion", async {
                                    page.locator("h1").expect().text("ready").await?;
                                    if ready {
                                        Ok(())
                                    } else {
                                        mismatch("async mismatch")
                                    }
                                })
                                .await
                            }
                        })
                        .await?;
                        assert_eq!(calls, 3);
                        assert!(soft.failures()?.is_empty());
                        assert!(ctx.info.errors().is_empty());
                        Ok(())
                    }),
                    test_with_context("one final soft <&> then recover", |ctx| async move {
                        let soft = ctx.info.soft_asserts();
                        let options = options()
                            .timeout(Timeout::ms(130))
                            .message("custom deadline <&>");
                        let retry = ctx.info.retry;
                        let mut calls = 0;
                        soft.run(
                            "final polling",
                            expect_to_pass_with("last mismatch", &options, || {
                                calls += 1;
                                std::future::ready(if retry == 0 {
                                    mismatch(format!("probe {calls}"))
                                } else {
                                    Ok(())
                                })
                            }),
                        )
                        .await?;
                        assert!(calls >= if retry == 0 { 2 } else { 1 });
                        assert_eq!(soft.failures()?.len(), usize::from(retry == 0));
                        assert_eq!(ctx.info.errors().len(), usize::from(retry == 0));
                        ctx.info.attach("continued", b"yes", "text/plain")?;
                        Ok(())
                    }),
                    test_with_context(
                        "probe scope does not leak to joined work",
                        |ctx| async move {
                            let soft = ctx.info.soft_asserts();
                            let outside = soft.clone();
                            let mut calls = 0;
                            let opts = options();
                            let (poll, other) = futures::join!(
                                expect_to_pass_with("joined probe", &opts, || {
                                    calls += 1;
                                    std::future::ready(soft.check(if calls < 3 {
                                        mismatch("retry")
                                    } else {
                                        Ok(())
                                    }))
                                }),
                                async {
                                    tokio::task::yield_now().await;
                                    outside.check_with_message(
                                        mismatch("outside mismatch"),
                                        "outside probe",
                                    )
                                }
                            );
                            poll?;
                            other?;
                            assert_eq!(calls, 3);
                            assert_eq!(soft.failures()?.len(), 1);
                            assert_eq!(
                                soft.failures()?[0].message.as_deref(),
                                Some("outside probe")
                            );
                            Ok(())
                        },
                    )
                    .fail(),
                    test_with_context("typed operation is never softened", |ctx| async move {
                        let soft = ctx.info.soft_asserts();
                        let mut calls = 0;
                        let error = soft
                            .run(
                                "operation",
                                expect_to_pass_with("decode", &options(), || {
                                    calls += 1;
                                    let page = ctx.page.clone();
                                    async move {
                                        page.evaluate::<String>("42").await?;
                                        Ok(())
                                    }
                                }),
                            )
                            .await
                            .unwrap_err();
                        assert_eq!(error.code(), "FERRITE_E2E_JSON");
                        assert_eq!(calls, 1);
                        assert!(soft.failures()?.is_empty());
                        assert!(ctx.info.errors().is_empty());
                        Ok(())
                    }),
                ],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(report.flaky(), 1);
        let recovery = report
            .results
            .iter()
            .find(|r| r.name == "nested soft recovery")
            .unwrap();
        let assertions = flat(&recovery.attempt_results[0].steps)
            .into_iter()
            .filter(|s| s.category == StepCategory::Assertion)
            .collect::<Vec<_>>();
        assert_eq!(assertions.len(), 2);
        assert!(assertions
            .iter()
            .all(|s| s.steps.is_empty() && s.status == StepStatus::Passed));
        let result = report.results.iter().find(|r| r.flaky).unwrap();
        assert_eq!(
            result
                .attempt_results
                .iter()
                .map(|a| a.status)
                .collect::<Vec<_>>(),
            [AttemptStatus::Failed, AttemptStatus::Passed]
        );
        assert_eq!(result.attempt_results[0].soft_assertions.len(), 1);
        assert!(result.attempt_results[1].soft_assertions.is_empty());
        assert_eq!(result.attempt_results[0].errors.len(), 1);
        assert!(result.attempt_results[1].errors.is_empty());
        for (index, attempt) in result.attempt_results.iter().enumerate() {
            let assertions = flat(&attempt.steps)
                .into_iter()
                .filter(|s| s.category == StepCategory::Assertion)
                .collect::<Vec<_>>();
            assert_eq!(assertions.len(), 1);
            assert_eq!(assertions[0].title, "expect.soft final polling");
            assert!(assertions[0].steps.is_empty());
            assert_eq!(
                assertions[0].status,
                if index == 0 {
                    StepStatus::Failed
                } else {
                    StepStatus::Passed
                }
            );
            assert!(attempt.attachments.iter().any(|a| a.name == "continued"));
        }
        let failure = &result.attempt_results[0].soft_assertions[0];
        assert!(failure
            .error
            .message
            .contains("custom deadline <&>: last mismatch"));
        assert!(failure.error.message.contains("probe "));
        assert_eq!(failure.error.phase, "soft body");
        assert!(failure.step_id.is_some());
        let serialized: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(serialized.flaky(), 1);
        let html = report.to_html();
        assert!(html.contains("custom deadline &lt;&amp;&gt;"));
        assert!(!html.contains("custom deadline <&>"));
        if let Some(preview) = std::env::var_os("FERRITE_POLL_REPORT_PREVIEW") {
            let path = std::path::Path::new(&preview).join(browser.kind().name());
            let bundle = report.write_bundle(&path).unwrap();
            let page = browser.new_page().await.unwrap();
            page.goto(&format!("file://{}", bundle.html.display()))
                .await
                .unwrap();
            page.evaluate::<Value>("document.querySelectorAll('details').forEach(d=>{if(d.querySelector('summary')?.textContent.includes('Attempt'))d.open=true});null").await.unwrap();
            page.save_screenshot(
                &path.join("preview.png"),
                ScreenshotOptions {
                    full_page: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
            page.close().await.unwrap();
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn caller_cancellation_and_native_disposal_settle_without_waiting_for_poll_timeout() {
    for browser in browsers().await {
        for hung in [false, true] {
            let token = CancellationToken::new();
            let cancel = token.clone();
            let calls = Arc::new(AtomicUsize::new(0));
            let invoked = calls.clone();
            let task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(40)).await;
                cancel.cancel_with_reason("caller stop");
            });
            let start = Instant::now();
            let error = expect_poll_with::<(), _, _>(
                "cancel",
                &options()
                    .timeout(Duration::ZERO)
                    .intervals([Duration::from_secs(10)])
                    .cancellation(token),
                move || {
                    invoked.fetch_add(1, Ordering::SeqCst);
                    async move {
                        if hung {
                            std::future::pending::<()>().await;
                        }
                        Ok(None)
                    }
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CANCELLED");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(start.elapsed() < Duration::from_secs(2));
            task.await.unwrap();
        }
        for mode in 0..3 {
            let hung = mode == 1;
            let before = context_ids(&browser).await;
            let context = browser
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            let page = context.new_page().await.unwrap();
            let entered = Arc::new(tokio::sync::Notify::new());
            let close_entered = entered.clone();
            let close = context.clone();
            let task = tokio::spawn(async move {
                close_entered.notified().await;
                tokio::time::sleep(Duration::from_millis(30)).await;
                close.close().await
            });
            let calls = Arc::new(AtomicUsize::new(0));
            let invoked = calls.clone();
            let captured = page.clone();
            let start = Instant::now();
            let opts = if mode == 2 {
                options()
                    .timeout(Duration::ZERO)
                    .intervals([Duration::from_secs(10)])
                    .cancellation(context.cancellation_token())
            } else {
                options().timeout(Duration::ZERO)
            };
            let error = tokio::time::timeout(
                Duration::from_secs(5),
                expect_poll_with::<(), _, _>("disposed page", &opts, move || {
                    invoked.fetch_add(1, Ordering::SeqCst);
                    let page = captured.clone();
                    let entered = entered.clone();
                    async move {
                        entered.notify_one();
                        page.evaluate::<Value>(if hung { "new Promise(()=>{})" } else { "42" })
                            .await?;
                        Ok(None)
                    }
                }),
            )
            .await
            .unwrap()
            .unwrap_err();
            assert!(
                matches!(error, E2eError::Cancelled(_) | E2eError::Disconnected(_)),
                "{error}"
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            task.await.unwrap().unwrap();
            assert!(context.is_closed());
            assert!(page.is_closed());
            assert!(calls.load(Ordering::SeqCst) <= 3);
            if mode == 2 {
                assert_eq!(calls.load(Ordering::SeqCst), 1);
            }
            assert_eq!(context_ids(&browser).await, before);
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn enclosing_runner_timeout_preempts_zero_or_large_poll_windows_and_runs_cleanup() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let closed = Arc::new(AtomicUsize::new(0));
        let before = context_ids(&browser).await;
        let probes = Arc::new(AtomicUsize::new(0));
        for local_timeout in [Duration::ZERO, Duration::from_secs(10)] {
            let closed_hook = closed.clone();
            let entered = probes.clone();
            let report = runner(&dir)
                .workers(1)
                .after_each(move |_| {
                    let closed = closed_hook.clone();
                    async move {
                        closed.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })
                .run(
                    &browser,
                    vec![test_with_context("bounded poll", move |ctx| {
                        let entered = entered.clone();
                        async move {
                            ctx.info.set_timeout(Duration::from_millis(400));
                            expect_to_pass_with(
                                "hung native probe",
                                &options().timeout(local_timeout),
                                || {
                                    entered.fetch_add(1, Ordering::SeqCst);
                                    let page = ctx.page.clone();
                                    async move {
                                        page.evaluate::<Value>("new Promise(()=>{})").await?;
                                        Ok(())
                                    }
                                },
                            )
                            .await
                        }
                    })],
                )
                .await;
            assert!(!report.ok(), "{}", report.to_list());
            let attempt = &report.results[0].attempt_results[0];
            assert_eq!(attempt.status, AttemptStatus::TimedOut);
            assert!(attempt.soft_assertions.is_empty());
            assert!(attempt
                .errors
                .iter()
                .any(|e| e.code == "FERRITE_E2E_TIMEOUT"));
            assert_eq!(attempt.settings.as_ref().unwrap().timeout_ms, 400);
            assert!(attempt.duration_ms < 3000);
            assert_eq!(context_ids(&browser).await, before);
        }
        assert_eq!(closed.load(Ordering::SeqCst), 2);
        assert_eq!(probes.load(Ordering::SeqCst), 2);
        browser.close().await.unwrap();
    }
}
