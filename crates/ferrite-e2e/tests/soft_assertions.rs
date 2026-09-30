//! Attempt-owned mismatch collection, native retries, cleanup and structured reports.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

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
                "soft assertions {} {}",
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
    .workers(2)
    .test_timeout(Duration::from_secs(5))
    .cleanup_timeout(Duration::from_secs(3))
    .output_dir(dir.path().display().to_string())
    .list_progress(false)
}
fn mismatch(message: &str) -> E2eResult<()> {
    Err(E2eError::Expect(message.into()))
}
fn flat(steps: &[StepInfo]) -> Vec<&StepInfo> {
    steps
        .iter()
        .flat_map(|step| std::iter::once(step).chain(flat(&step.steps)))
        .collect()
}
#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<AttemptResult>>>);
impl Reporter for Events {
    fn on_test_end(&self, _: &AttemptInfo, result: &TestResult) {
        self.0
            .lock()
            .unwrap()
            .push(result.attempt_results[0].clone());
    }
}

#[tokio::test]
async fn pinned_native_mismatches_continue_retry_expect_cleanup_and_parallel_isolation() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/soft-assertion-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let events = Events::default();
        let report = runner(&dir)
            .retries(1)
            .custom_reporter(events.clone())
            .after_each_with_context(ContextHook::new(|ctx| async move {
                if ctx.info.title == "cleanup mismatch" {
                    ctx.info
                        .soft_asserts()
                        .check_with_message(mismatch("cleanup"), "cleanup soft")?;
                }
                Ok(())
            }))
            .run(
                &browser,
                vec![
                    test_with_context("retry mismatches", |ctx| async move {
                        ctx.page.set_content("<h1>Actual</h1>").await?;
                        let soft = ctx.info.soft_asserts();
                        if ctx.info.retry == 0 {
                            soft.run(
                                "first message",
                                ctx.page
                                    .locator("h1")
                                    .describe("Heading")
                                    .expect()
                                    .timeout(Duration::from_millis(60))
                                    .text("wrong"),
                            )
                            .await?;
                            soft.check_with_message(mismatch("1 != 2"), "second message")?;
                            assert_eq!(ctx.info.errors().len(), 2);
                        }
                        ctx.info.attach("continued", b"yes", "text/plain")?;
                        Ok(())
                    }),
                    test_with_context("expected mismatch", |ctx| async move {
                        ctx.info.fail("expected");
                        SoftAsserts::for_attempt(&ctx.info)
                            .check_with_message(mismatch("1 != 2"), "expected soft")?;
                        Ok(())
                    }),
                    test_with_context("cleanup mismatch", |_| async { Ok(()) }),
                    test_with_context("parallel clean", |ctx| async move {
                        ctx.info.soft_asserts().check(Ok(()))?;
                        assert!(ctx.info.errors().is_empty());
                        Ok(())
                    }),
                    test_with_context("skip after mismatch", |ctx| async move {
                        if ctx.info.retry == 0 {
                            ctx.info
                                .soft_asserts()
                                .check_with_message(mismatch("1 != 2"), "before skip")?;
                        }
                        ctx.info.skip("per-attempt skip")
                    }),
                ],
            )
            .await;
        for case in reference["cases"].as_array().unwrap() {
            let result = report
                .results
                .iter()
                .find(|result| result.name == case["name"].as_str().unwrap())
                .unwrap();
            let actual = json!({"expected":format!("{:?}",result.attempt_results.last().unwrap().expected_status).to_ascii_lowercase(),
                "attempts":result.attempt_results.iter().map(|attempt|json!({"status":format!("{:?}",attempt.status).to_ascii_lowercase(),"errors":attempt.errors.len(),"continued":attempt.attachments.iter().any(|a|a.name=="continued")})).collect::<Vec<_>>()});
            assert_eq!(actual, case["result"], "{}", result.name);
        }
        let retry = report
            .results
            .iter()
            .find(|r| r.name == "retry mismatches")
            .unwrap();
        assert!(retry.flaky);
        assert!(retry.attempt_results[1].soft_assertions.is_empty());
        assert_eq!(retry.attempt_results[0].soft_assertions.len(), 2);
        let steps = flat(&retry.attempt_results[0].steps);
        let soft_step = steps
            .iter()
            .find(|s| s.title == "expect.soft first message")
            .unwrap();
        assert_eq!(soft_step.status, StepStatus::Failed);
        assert!(soft_step.steps.is_empty());
        assert!(soft_step
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("Heading"));
        assert_eq!(
            retry.attempt_results[0].soft_assertions[0].step_id,
            Some(soft_step.id)
        );
        assert_eq!(
            retry.attempt_results[0].soft_assertions[0]
                .error
                .location
                .as_ref()
                .unwrap()
                .line,
            soft_step.location.line
        );
        assert_eq!(
            events.0.lock().unwrap().len(),
            report
                .results
                .iter()
                .map(|r| r.attempt_results.len())
                .sum::<usize>()
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_sources_user_steps_context_json_html_and_retained_handles_are_owned() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let retained = Arc::new(Mutex::new(Vec::new()));
        let capture = retained.clone();
        let report = runner(&dir)
            .retries(1)
            .run(
                &browser,
                vec![test_with_context("owned <soft>", move |ctx| {
                    let capture = capture.clone();
                    async move {
                        let soft = ctx.info.soft_asserts();
                        capture
                            .lock()
                            .unwrap()
                            .push((ctx.info.clone(), soft.clone()));
                        ctx.page.set_content("<h1>Actual</h1>").await?;
                        ctx.page
                            .step_result("user step", async {
                                let line = line!() + 1;
                                soft.check_with_message(
                                    mismatch("first"),
                                    "<script>bad()</script> & 雪",
                                )?;
                                assert_eq!(
                                    ctx.info.soft_failures()[0]
                                        .error
                                        .location
                                        .as_ref()
                                        .unwrap()
                                        .line,
                                    line
                                );
                                soft.run(
                                    "second context",
                                    ctx.page
                                        .locator("h1")
                                        .expect()
                                        .timeout(Duration::from_millis(50))
                                        .text("wrong"),
                                )
                                .await?;
                                Ok(())
                            })
                            .await?;
                        assert_eq!(soft.failures()?.len(), 2);
                        Ok(())
                    }
                })],
            )
            .await;
        assert_eq!(report.failed(), 1);
        for attempt in &report.results[0].attempt_results {
            assert_eq!(attempt.soft_assertions.len(), 2);
            let steps = flat(&attempt.steps);
            let user = steps.iter().find(|s| s.title == "user step").unwrap();
            assert_eq!(attempt.soft_assertions[0].step_id, Some(user.id));
            assert!(attempt.soft_assertions[0]
                .title_path
                .last()
                .unwrap()
                .contains("user step"));
            assert_eq!(
                attempt.soft_assertions[1].title_path.last().unwrap(),
                "expect.soft second context"
            );
            assert!(attempt.errors.iter().all(|e| e.phase == "soft body"));
        }
        for (info, soft) in retained.lock().unwrap().iter() {
            assert!(matches!(
                soft.check(mismatch("late")),
                Err(E2eError::Config(_))
            ));
            assert_eq!(info.soft_failures().len(), 2);
            assert_eq!(soft.failures().unwrap().len(), 2);
        }
        let mut value = serde_json::to_value(&report).unwrap();
        let decoded: TestReport = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            decoded.results[0].attempt_results[0].soft_assertions.len(),
            2
        );
        for attempt in value["results"][0]["attempt_results"]
            .as_array_mut()
            .unwrap()
        {
            attempt.as_object_mut().unwrap().remove("soft_assertions");
        }
        let old: TestReport = serde_json::from_value(value).unwrap();
        assert!(old.results[0].attempt_results[0].soft_assertions.is_empty());
        let html = report.to_html();
        assert!(
            html.contains("Soft assertions") && html.contains("&lt;script&gt;bad()&lt;/script&gt;")
        );
        assert!(!html.contains("<script>bad()"));
        if let Some(preview) = std::env::var_os("FERRITE_SOFT_REPORT_PREVIEW") {
            report
                .write_bundle(std::path::Path::new(&preview).join(browser.kind().name()))
                .unwrap();
        }
        browser.close().await.unwrap();
    }
}

struct Resource(Arc<TestInfo>);
#[tokio::test]
async fn native_setup_cleanup_and_fixture_failures_reach_following_hooks_and_results() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let capture = seen.clone();
        let report = runner(&dir)
            .workers(1)
            .fixture_definition(
                Fixture::<Resource>::new(|values| async move {
                    let info = values.require::<TestInfo>()?;
                    if info.title == "setup mismatch" {
                        info.soft_asserts()
                            .check_with_message(mismatch("setup"), "setup context")?;
                    }
                    Ok(Resource(info))
                })
                .dependency::<TestInfo>()
                .automatic(true)
                .teardown(|value| async move {
                    if value.0.title == "fixture mismatch" {
                        value
                            .0
                            .soft_asserts()
                            .check_with_message(mismatch("fixture cleanup"), "fixture context")?;
                    }
                    assert!(value.0.status().is_some());
                    Ok(())
                }),
            )
            .after_each_with_context(ContextHook::new(|ctx| async move {
                if ctx.info.title == "hook mismatch" {
                    ctx.info
                        .soft_asserts()
                        .check_with_message(mismatch("hook cleanup"), "hook context")?;
                }
                Ok(())
            }))
            .after_each_with_context(ContextHook::new(move |ctx| {
                let capture = capture.clone();
                async move {
                    capture.lock().unwrap().push((
                        ctx.info.title.clone(),
                        ctx.info.status(),
                        ctx.info.errors().len(),
                    ));
                    Ok(())
                }
            }))
            .run(
                &browser,
                vec![
                    test_with_context("setup mismatch", |ctx| async move {
                        ctx.info.fail("setup must not be expected");
                        assert_eq!(ctx.info.status(), Some(AttemptStatus::Failed));
                        Ok(())
                    }),
                    test_with_context("hook mismatch", |ctx| async move {
                        ctx.info.fail("cleanup must not be expected");
                        ctx.info.soft_asserts().check(mismatch("expected body"))?;
                        Ok(())
                    }),
                    test_with_context("fixture mismatch", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.failed(), 3);
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            assert_eq!(attempt.status, AttemptStatus::Failed);
            let phase = if result.name == "setup mismatch" {
                "soft setup"
            } else {
                "soft cleanup"
            };
            assert!(attempt
                .soft_assertions
                .iter()
                .any(|f| f.error.phase == phase));
            assert!(!attempt.is_expected);
        }
        let seen = seen.lock().unwrap().clone();
        assert!(seen.iter().find(|x| x.0 == "hook mismatch").unwrap().2 >= 2);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_timeout_cancellation_and_failed_setup_keep_prior_mismatches_and_cleanup() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let cleaned = Arc::new(Mutex::new(0));
        let capture = cleaned.clone();
        let report = runner(&dir)
            .after_each_with_context(ContextHook::new(move |ctx| {
                let capture = capture.clone();
                async move {
                    *capture.lock().unwrap() += 1;
                    assert!(!ctx.info.soft_failures().is_empty());
                    Ok(())
                }
            }))
            .run(
                &browser,
                vec![
                    test_with_context("timeout", |ctx| async move {
                        let soft = ctx.info.soft_asserts();
                        soft.check(mismatch("before timeout"))?;
                        soft.run(
                            "operational timeout",
                            ctx.page
                                .locator("#missing")
                                .with_timeout(Duration::from_millis(70))
                                .click(),
                        )
                        .await
                    }),
                    test_with_context("canceled", |ctx| async move {
                        let soft = ctx.info.soft_asserts();
                        soft.check(mismatch("before cancellation"))?;
                        let token = CancellationToken::new();
                        token.cancel_with_reason("native caller");
                        soft.run("operational cancel", async {
                            ctx.page
                                .with_cancellation(token)
                                .locator("h1")
                                .count()
                                .await
                                .map(|_| ())
                        })
                        .await
                    }),
                ],
            )
            .await;
        assert_eq!(*cleaned.lock().unwrap(), 2);
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            assert_eq!(attempt.soft_assertions.len(), 1);
            assert_eq!(attempt.errors.len(), 2);
            let (status, code) = if result.name == "timeout" {
                (AttemptStatus::TimedOut, "FERRITE_E2E_TIMEOUT")
            } else {
                (AttemptStatus::Interrupted, "FERRITE_E2E_CANCELLED")
            };
            assert_eq!(attempt.status, status);
            assert_eq!(attempt.errors[1].code, code);
            assert_eq!(result.status, TestStatus::Failed);
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_failed_fixture_setup_retains_soft_and_hard_causes_and_releases_dependencies() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let released = Arc::new(Mutex::new(0));
        let cleanup = released.clone();
        let report = runner(&dir)
            .workers(1)
            .fixture_definition(
                Fixture::<Resource>::new(|values| async move {
                    Ok(Resource(values.require::<TestInfo>()?))
                })
                .dependency::<TestInfo>()
                .teardown(move |value| {
                    let cleanup = cleanup.clone();
                    async move {
                        assert_eq!(value.0.status(), Some(AttemptStatus::Failed));
                        assert_eq!(value.0.errors().len(), 2);
                        *cleanup.lock().unwrap() += 1;
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<u32>::new(|values| async move {
                    let resource = values.require::<Resource>()?;
                    resource
                        .0
                        .soft_asserts()
                        .check_with_message(mismatch("setup mismatch"), "before failed setup")?;
                    Err(E2eError::Config("setup hard error".into()))
                })
                .dependency::<Resource>(),
            )
            .run(
                &browser,
                vec![test("unreached body", |_| async {
                    panic!("body ran after failed setup")
                })
                .fixture::<u32>()],
            )
            .await;
        assert_eq!(*released.lock().unwrap(), 1);
        let attempt = &report.results[0].attempt_results[0];
        assert_eq!(attempt.soft_assertions.len(), 1);
        assert_eq!(attempt.soft_assertions[0].error.phase, "soft setup");
        assert_eq!(attempt.errors.len(), 2);
        assert_eq!(attempt.errors[1].code, "FERRITE_E2E_CONFIG");
        assert!(report.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("before failed setup"));
        assert!(report.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("setup hard error"));
        browser.close().await.unwrap();
    }
}
