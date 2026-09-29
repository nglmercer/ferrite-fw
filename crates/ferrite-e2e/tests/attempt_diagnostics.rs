//! Structured diagnostics, hook outcomes, and complete retry history on both engines.
use ferrite_e2e::*;
use std::{
    sync::{Arc, Mutex},
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
            eprintln!("validating {kind:?}: {}", path.display());
            browsers.push(
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap(),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::from_config(&E2eConfig {
        screenshot: "on".into(),
        ..E2eConfig::default()
    })
    .workers(1)
    .test_timeout(Duration::from_secs(15))
    .cleanup_timeout(Duration::from_secs(5))
    .output_dir(dir.path().display().to_string())
    .list_progress(false)
}
#[derive(Clone, Default)]
struct AttemptEvents(Arc<Mutex<Vec<AttemptResult>>>);
impl Reporter for AttemptEvents {
    fn on_test_end(&self, _: &AttemptInfo, result: &TestResult) {
        assert_eq!(result.attempt_results.len(), 1);
        self.0
            .lock()
            .unwrap()
            .push(result.attempt_results[0].clone());
    }
}

#[tokio::test]
async fn retries_preserve_nested_steps_source_errors_and_all_artifacts() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let events = AttemptEvents::default();
        let hooks = Arc::new(Mutex::new(Vec::new()));
        let hook_log = hooks.clone();
        let report = runner(&dir)
            .retries(1)
            .custom_reporter(events.clone())
            .after_each_with_context(ContextHook::new(move |ctx| {
                let log = hook_log.clone();
                async move {
                    log.lock()
                        .unwrap()
                        .push((ctx.info.status(), ctx.info.errors()));
                    Ok(())
                }
            }))
            .run(
                &browser,
                vec![test_with_context("retry diagnostics", |ctx| async move {
                    assert_eq!(ctx.info.status(), None);
                    ctx.page.set_content("<h1>attempt</h1>").await?;
                    let line = line!() + 3;
                    let result = ctx
                        .page
                        .step_result("outer", async {
                            ctx.page
                                .step_result("inner", async {
                                    ctx.info.attach(
                                        "state",
                                        format!("retry {}", ctx.info.retry).as_bytes(),
                                        "text/plain",
                                    )?;
                                    ctx.info.annotate("attempt", ctx.info.retry.to_string());
                                    if ctx.info.retry == 0 {
                                        Err(E2eError::Expect("first failure".into()))
                                    } else {
                                        Ok(())
                                    }
                                })
                                .await
                        })
                        .await;
                    ctx.info
                        .attach("source line", line.to_string().as_bytes(), "text/plain")?;
                    result
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        let result = &report.results[0];
        assert!(result.flaky);
        assert_eq!(report.flaky(), 1);
        assert_eq!(result.attempt_results.len(), 2);
        assert_eq!(events.0.lock().unwrap().len(), 2);
        for (index, attempt) in result.attempt_results.iter().enumerate() {
            assert_eq!(attempt.info.retry, index as u32);
            assert_eq!(attempt.info.name, "retry diagnostics");
            assert_eq!(attempt.steps.len(), 1);
            let outer = &attempt.steps[0];
            let inner = &outer.steps[0];
            assert_eq!(inner.parent_id, Some(outer.id));
            assert_eq!(inner.attachments.len(), 1);
            assert!(outer.attachments.is_empty());
            let source = attempt
                .attachments
                .iter()
                .find(|a| a.name == "source line")
                .unwrap();
            let line: u32 = std::fs::read_to_string(&source.path)
                .unwrap()
                .parse()
                .unwrap();
            assert_eq!(outer.location.line, line);
            assert!(outer.location.file.ends_with("attempt_diagnostics.rs"));
            assert!(outer.location.column > 0);
            assert!(!outer.interrupted);
            assert_eq!(outer.error.is_some(), index == 0);
            assert_eq!(inner.error.is_some(), index == 0);
            assert_eq!(attempt.errors.len(), usize::from(index == 0));
            assert_eq!(attempt.attachments.len(), 2);
            assert_eq!(attempt.screenshots.len(), 1);
            for path in [&attempt.screenshots[0], attempt.trace.as_ref().unwrap()] {
                assert!(std::path::Path::new(path).exists());
            }
            assert_eq!(attempt.annotations[0].1, index.to_string());
        }
        assert_eq!(result.attachments.len(), 4);
        assert_eq!(result.attempt_results[0].status, AttemptStatus::Failed);
        assert_eq!(result.attempt_results[1].status, AttemptStatus::Passed);
        assert_eq!(hooks.lock().unwrap()[0].0, Some(AttemptStatus::Failed));
        assert_eq!(hooks.lock().unwrap()[0].1[0].phase, "body");
        assert_eq!(hooks.lock().unwrap()[1].0, Some(AttemptStatus::Passed));
        let round_trip: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(round_trip.results[0].attempt_results.len(), 2);
        for expected in [
            "Attempt 1",
            "Attempt 2",
            "first failure",
            "outer",
            "inner",
            "flaky",
        ] {
            assert!(report.to_html().contains(expected));
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn hooks_see_raw_status_expected_status_and_errors_before_cleanup() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let hooks = Arc::new(Mutex::new(Vec::new()));
        let log = hooks.clone();
        let report = runner(&dir)
            .retries(2)
            .after_each_with_context(ContextHook::new(move |ctx| {
                let log = log.clone();
                async move {
                    log.lock().unwrap().push((
                        ctx.info.title.clone(),
                        ctx.info.status(),
                        ctx.info.expected_status(),
                        ctx.info.errors(),
                    ));
                    Ok(())
                }
            }))
            .run(
                &browser,
                vec![
                    test_with_context("handled", |ctx| async move {
                        let result = ctx
                            .page
                            .step_result("handled error", async {
                                Err::<(), _>(E2eError::Expect("handled".into()))
                            })
                            .await;
                        assert!(result.is_err());
                        Ok(())
                    }),
                    test_with_context("expected", |ctx| async move {
                        ctx.info.fail("known");
                        Err(E2eError::Expect("known failure".into()))
                    }),
                    test_with_context("unexpected pass", |ctx| async move {
                        ctx.info.fail("known");
                        Ok(())
                    }),
                    test_with_context("skipped", |ctx| async move {
                        ctx.info.skip("unavailable")?;
                        Ok(())
                    }),
                    test_with_context("timeout", |ctx| async move {
                        ctx.info.fail("timeout must remain unexpected");
                        ctx.info.set_timeout(Duration::from_secs(1));
                        ctx.page.step("waiting", std::future::pending::<()>()).await;
                        Ok(())
                    })
                    .retries(0),
                    test_with_context("panic", |ctx| async move {
                        ctx.page
                            .step_result("panic step", async {
                                panic!("step exploded");
                                #[allow(unreachable_code)]
                                Ok::<(), E2eError>(())
                            })
                            .await
                    })
                    .retries(0),
                ],
            )
            .await;
        let hook_data = hooks.lock().unwrap().clone();
        let find = |name: &str| hook_data.iter().find(|v| v.0 == name).unwrap();
        assert_eq!(find("handled").1, Some(AttemptStatus::Passed));
        assert!(find("handled").3.is_empty());
        assert_eq!(find("expected").1, Some(AttemptStatus::Failed));
        assert_eq!(find("expected").2, AttemptStatus::Failed);
        assert_eq!(find("expected").3[0].code, "FERRITE_E2E_EXPECT");
        assert_eq!(find("unexpected pass").1, Some(AttemptStatus::Passed));
        assert_eq!(find("unexpected pass").2, AttemptStatus::Failed);
        assert_eq!(find("skipped").1, Some(AttemptStatus::Skipped));
        assert_eq!(find("skipped").2, AttemptStatus::Skipped);
        assert!(find("skipped").3.is_empty());
        assert_eq!(find("timeout").1, Some(AttemptStatus::TimedOut));
        assert_eq!(find("timeout").3[0].code, "FERRITE_E2E_TIMEOUT");
        assert_eq!(find("panic").1, Some(AttemptStatus::Failed));
        let result = |name: &str| report.results.iter().find(|r| r.name == name).unwrap();
        assert!(!result("handled").flaky);
        assert!(result("handled").attempt_results[0].steps[0]
            .error
            .is_some());
        assert_eq!(result("expected").status, TestStatus::FailedExpected);
        assert_eq!(result("expected").attempts, 1);
        assert!(result("expected").attempt_results[0].is_expected);
        assert_eq!(result("unexpected pass").attempts, 1);
        assert_eq!(
            result("unexpected pass").attempt_results[0].status,
            AttemptStatus::Passed
        );
        assert_eq!(
            result("unexpected pass").attempt_results[0].errors[0].code,
            "unexpected_pass"
        );
        assert!(!result("unexpected pass").attempt_results[0].is_expected);
        assert!(result("timeout").attempt_results[0].steps[0].interrupted);
        assert_eq!(
            result("panic").attempt_results[0].steps[0]
                .error
                .as_ref()
                .unwrap()
                .code,
            "panic"
        );
        browser.close().await.unwrap();
    }
}

struct OutcomeFixture(TestInfo);
struct DependentOutcome(TestInfo);
#[tokio::test]
async fn cleanup_errors_update_following_hooks_and_dependency_teardown() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<OutcomeFixture>::new(|map| async move {
                    Ok(OutcomeFixture((*map.require::<TestInfo>()?).clone()))
                })
                .dependency::<TestInfo>()
                .teardown(|value| async move {
                    assert_eq!(value.0.status(), Some(AttemptStatus::Failed));
                    let errors = value.0.errors();
                    assert_eq!(
                        errors.iter().map(|e| e.phase.as_str()).collect::<Vec<_>>(),
                        ["body", "after_each", "fixture teardown"]
                    );
                    Ok(())
                }),
            )
            .fixture_definition(
                Fixture::<DependentOutcome>::new(|map| async move {
                    map.require::<OutcomeFixture>()?;
                    Ok(DependentOutcome((*map.require::<TestInfo>()?).clone()))
                })
                .dependency::<OutcomeFixture>()
                .dependency::<TestInfo>()
                .automatic(true)
                .teardown(|value| async move {
                    assert_eq!(value.0.errors().len(), 2);
                    Err(E2eError::Expect("fixture cleanup failed".into()))
                }),
            )
            .after_each_with_context(ContextHook::new(|ctx| async move {
                assert_eq!(ctx.info.status(), Some(AttemptStatus::Failed));
                assert_eq!(ctx.info.expected_status(), AttemptStatus::Failed);
                assert_eq!(ctx.info.errors().len(), 1);
                Err(E2eError::Expect("hook failed".into()))
            }))
            .after_each_with_context(ContextHook::new(|ctx| async move {
                assert_eq!(ctx.info.errors().len(), 2);
                Ok(())
            }))
            .run(
                &browser,
                vec![test_with_context("cleanup failure", |ctx| async move {
                    ctx.info.fail("known failure");
                    Err(E2eError::Expect("body failed".into()))
                })],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Failed);
        let attempt = &report.results[0].attempt_results[0];
        assert!(!attempt.is_expected);
        assert_eq!(attempt.errors.len(), 3);
        assert_eq!(
            attempt.errors[2].message,
            "expect failed: fixture cleanup failed"
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn a_cleanup_failure_changes_a_passed_body_for_later_hooks_and_fixtures() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<OutcomeFixture>::new(|map| async move {
                    Ok(OutcomeFixture((*map.require::<TestInfo>()?).clone()))
                })
                .dependency::<TestInfo>()
                .automatic(true)
                .teardown(|value| async move {
                    assert_eq!(value.0.status(), Some(AttemptStatus::Failed));
                    assert_eq!(value.0.errors()[0].phase, "after_each");
                    Ok(())
                }),
            )
            .after_each_with_context(ContextHook::new(|ctx| async move {
                assert_eq!(ctx.info.status(), Some(AttemptStatus::Passed));
                assert!(ctx.info.errors().is_empty());
                Err(E2eError::Expect("cleanup failed".into()))
            }))
            .after_each_with_context(ContextHook::new(|ctx| async move {
                assert_eq!(ctx.info.status(), Some(AttemptStatus::Failed));
                assert_eq!(ctx.info.errors().len(), 1);
                Ok(())
            }))
            .run(&browser, vec![test("passed body", |_| async { Ok(()) })])
            .await;
        assert_eq!(report.results[0].status, TestStatus::Failed);
        assert_eq!(
            report.results[0].attempt_results[0].status,
            AttemptStatus::Failed
        );
        assert_eq!(report.results[0].attempt_results[0].errors.len(), 1);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn early_setup_failures_are_in_every_attempt_with_original_status() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .retries(1)
            .fixture_definition(
                Fixture::<u32>::new(|_| async {
                    Err(E2eError::Expect("worker setup failed".into()))
                })
                .scope(FixtureScope::Worker)
                .automatic(true),
            )
            .run(
                &browser,
                vec![test("not reached", |_| async {
                    panic!("body should not run")
                })],
            )
            .await;
        assert_eq!(report.results[0].attempt_results.len(), 2);
        for attempt in &report.results[0].attempt_results {
            assert_eq!(attempt.status, AttemptStatus::Failed);
            assert_eq!(attempt.errors[0].phase, "worker fixture setup");
            assert!(attempt.errors[0].message.contains("worker setup failed"));
            assert!(attempt.steps.is_empty());
        }
        let tests = Suite::new("blocked setup")
            .before_all(|| async { std::future::pending::<E2eResult<()>>().await })
            .tests(vec![test("never", |_| async { Ok(()) })]);
        let report = runner(&dir)
            .test_timeout(Duration::from_millis(50))
            .run(&browser, tests)
            .await;
        assert_eq!(
            report.results[0].attempt_results[0].status,
            AttemptStatus::TimedOut
        );
        assert_eq!(
            report.results[0].attempt_results[0].errors[0].phase,
            "before_all"
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn cancelled_steps_and_cleanup_preserve_interrupted_outcome() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let token = CancellationToken::new();
        let abort = token.clone();
        let report = runner(&dir)
            .with_cancellation(token)
            .after_each_with_context(ContextHook::new(|ctx| async move {
                assert_eq!(ctx.info.status(), Some(AttemptStatus::Interrupted));
                assert_eq!(ctx.info.errors()[0].code, "FERRITE_E2E_CANCELLED");
                Err(E2eError::Expect("cleanup after cancellation".into()))
            }))
            .run(
                &browser,
                vec![test_with_context("cancel", move |ctx| {
                    let abort = abort.clone();
                    async move {
                        ctx.page
                            .step("outer", async {
                                ctx.page
                                    .step("inner", async {
                                        abort.cancel();
                                        std::future::pending::<()>().await
                                    })
                                    .await
                            })
                            .await;
                        Ok(())
                    }
                })],
            )
            .await;
        let attempt = &report
            .results
            .iter()
            .find(|r| r.name == "cancel")
            .unwrap()
            .attempt_results[0];
        assert_eq!(attempt.status, AttemptStatus::Interrupted);
        assert_eq!(attempt.errors.len(), 2);
        assert!(attempt.steps[0].interrupted);
        assert!(attempt.steps[0].steps[0].interrupted);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn parallel_project_repetitions_keep_attempts_and_step_trees_isolated() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .workers(2)
            .repeat_each(2)
            .retries(1)
            .project(Project::new("one"))
            .project(Project::new("two"))
            .run(
                &browser,
                vec![test_with_context("same name", |ctx| async move {
                    ctx.page
                        .step_result("parent", async {
                            tokio::join!(
                                ctx.page
                                    .step("left", async { tokio::task::yield_now().await }),
                                ctx.page.step("right", async {})
                            );
                            ctx.info.attach(
                                "identity",
                                format!(
                                    "{:?}:{}:{}",
                                    ctx.info.project, ctx.info.repeat_each_index, ctx.info.retry
                                )
                                .as_bytes(),
                                "text/plain",
                            )?;
                            if ctx.info.retry == 0 {
                                Err(E2eError::Expect("retry".into()))
                            } else {
                                Ok(())
                            }
                        })
                        .await
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(report.results.len(), 4);
        let mut ids = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for result in &report.results {
            assert!(result.flaky);
            assert_eq!(result.attempt_results.len(), 2);
            for attempt in &result.attempt_results {
                assert_eq!(attempt.info.project, result.project);
                assert_eq!(attempt.info.repeat_each_index, result.repeat_each_index);
                assert!(ids.insert(attempt.steps[0].id));
                assert_eq!(attempt.steps[0].steps.len(), 2);
                for child in &attempt.steps[0].steps {
                    assert_eq!(child.parent_id, Some(attempt.steps[0].id));
                }
                assert!(paths.insert(attempt.attachments[0].path.clone()));
            }
        }
        browser.close().await.unwrap();
    }
}
