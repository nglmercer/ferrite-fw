//! CI policy preserves attempt outcomes, scheduling and effective snapshots.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

async fn browsers() -> Vec<Browser> {
    let mut owners = Vec::new();
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
                "CI policy {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            owners.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(owners.len(), 2);
    }
    owners
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::from_config(&E2eConfig {
        screenshot: "off".into(),
        output_dir: dir.path().display().to_string(),
        timeout_ms: 10000,
        cleanup_timeout_ms: 3000,
        workers: 1,
        retries: 1,
        reporter: "json,junit,html".into(),
        ..Default::default()
    })
    .list_progress(false)
}
#[derive(Clone, Default)]
struct Events {
    attempts: Arc<Mutex<Vec<AttemptResult>>>,
    ended: Arc<Mutex<Vec<Value>>>,
}
impl Reporter for Events {
    fn on_test_end(&self, _info: &AttemptInfo, result: &TestResult) {
        self.attempts
            .lock()
            .unwrap()
            .extend(result.attempt_results.clone());
    }
    fn on_end(&self, report: &TestReport) {
        self.ended
            .lock()
            .unwrap()
            .push(serde_json::to_value(report).unwrap());
    }
}
fn flaky(expected_policy: bool) -> Test {
    test_with_context("recovers <&>", move |ctx| async move {
        assert_eq!(ctx.info.config().fail_on_flaky_tests, expected_policy);
        if ctx.info.retry == 0 {
            return Err(E2eError::Expect("first attempt".into()));
        }
        ctx.page.set_content("<h1>recovered</h1>").await?;
        Ok(())
    })
}
fn assert_aggregate(report: &TestReport, fail: bool, flaky_count: usize) {
    assert_eq!(report.flaky(), flaky_count, "{}", report.to_list());
    assert_eq!(report.flaky_policy_failed(), fail && flaky_count > 0);
    let json: Value = serde_json::from_str(&report.to_json()).unwrap();
    assert_eq!(json["exit_code"], report.exit_code());
    assert_eq!(
        json["status"],
        if report.ok() { "passed" } else { "failed" }
    );
    let parsed: TestReport = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.ok(), report.ok());
    assert_eq!(parsed.results.len(), report.results.len());
    assert!(report.to_html().contains(if report.ok() {
        ">run passed</span>"
    } else {
        ">run failed</span>"
    }));
}

#[tokio::test]
async fn retry_policy_keeps_passed_attempts_and_does_not_stop_later_tests() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/ci-policy-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for browser in browsers().await {
        for fail in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let events = Events::default();
            let report = runner(&dir)
                .fail_on_flaky_tests(fail)
                .max_failures(1)
                .custom_reporter(events.clone())
                .run(
                    &browser,
                    vec![
                        flaky(fail),
                        test("later", |_| async { Ok(()) }),
                        test("expected", |_| async {
                            Err(E2eError::Expect("expected".into()))
                        })
                        .fail(),
                        test("skip", |_| async { panic!("skip body") }).skip(),
                        test("fixme", |_| async { panic!("fixme body") }).fixme(),
                    ],
                )
                .await;
            assert_eq!(
                (report.passed(), report.failed(), report.expected_failed()),
                (2, 0, 1),
                "{}",
                report.to_list()
            );
            assert_eq!(report.ok(), !fail);
            assert_aggregate(&report, fail, 1);
            assert_eq!(
                *events.ended.lock().unwrap(),
                vec![serde_json::to_value(&report).unwrap()]
            );
            let result = report.results.iter().find(|r| r.flaky).unwrap();
            assert_eq!(result.status, TestStatus::Passed);
            assert_eq!(result.attempts, 2);
            assert_eq!(
                result
                    .attempt_results
                    .iter()
                    .map(|a| a.status)
                    .collect::<Vec<_>>(),
                [AttemptStatus::Failed, AttemptStatus::Passed]
            );
            assert!(result.attempt_results[1].errors.is_empty());
            {
                let attempts = events.attempts.lock().unwrap();
                assert_eq!(attempts.len(), 4);
                assert_eq!(
                    attempts
                        .iter()
                        .filter(|a| a.info.name == result.name && a.status == AttemptStatus::Passed)
                        .count(),
                    1
                );
            }
            let junit = report.to_junit();
            assert_eq!(junit.contains("type=\"FlakyTestPolicy\""), fail);
            assert_eq!(junit.matches("<testcase ").count(), 5);
            assert_eq!(junit.matches("<failure ").count(), usize::from(fail));
            assert!(junit.contains("name=\"ferrite.final_status\" value=\"passed\""));
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &std::fs::read(dir.path().join("results.json")).unwrap()
                )
                .unwrap()["exit_code"],
                i32::from(fail)
            );
            let pinned = reference["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| {
                    case["name"]
                        == if fail {
                            "reject-flaky"
                        } else {
                            "default-flaky"
                        }
                })
                .unwrap();
            assert_eq!(pinned["exitCode"], report.exit_code());
            assert_eq!(pinned["tests"][0]["outcome"], "flaky");
            assert_eq!(
                pinned["tests"][0]["attempts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|attempt| attempt["status"].clone())
                    .collect::<Vec<_>>(),
                result
                    .attempt_results
                    .iter()
                    .map(|attempt| serde_json::to_value(attempt.status).unwrap())
                    .collect::<Vec<_>>()
            );
            if fail {
                if let Some(preview) = std::env::var_os("FERRITE_CI_REPORT_PREVIEW") {
                    let path = std::path::Path::new(&preview).join(browser.kind().name());
                    let bundle = report.write_bundle(&path).unwrap();
                    let page = browser.new_page().await.unwrap();
                    page.goto(&format!("file://{}", bundle.html.display()))
                        .await
                        .unwrap();
                    assert_eq!(
                        page.evaluate::<String>("document.querySelector('.pill').textContent")
                            .await
                            .unwrap(),
                        "run failed"
                    );
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
            }
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn repetitions_and_projects_classify_policy_per_result() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .fail_on_flaky_tests(true)
            .workers(2)
            .repeat_each(2)
            .project(Project::new("alpha"))
            .project(Project::new("beta"))
            .run(&browser, vec![flaky(true)])
            .await;
        assert_eq!(
            (report.results.len(), report.passed(), report.failed()),
            (4, 4, 0),
            "{}",
            report.to_list()
        );
        assert_aggregate(&report, true, 4);
        for result in &report.results {
            assert_eq!(result.attempts, 2);
            assert_eq!(result.attempt_results[1].status, AttemptStatus::Passed);
            assert!(result.repeat_each_index < 2);
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn forbidden_focus_is_checked_before_filters_shards_and_bodies() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let entered = Arc::new(AtomicUsize::new(0));
        let body = |name| {
            let entered = entered.clone();
            test(name, move |_| {
                let entered = entered.clone();
                async move {
                    entered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
        };
        let variants = vec![
            vec![body("focus").only()],
            vec![body("focus").skip().only()],
            vec![body("focus").only().fixme()],
            vec![body("focus").only().fail()],
            Suite::new("suite").only().tests(vec![body("focus").skip()]),
            Suite::new("outer")
                .skip()
                .tests(Suite::new("inner").only().tests(vec![body("focus")])),
            Suite::new("suite").only().skip().tests(vec![body("focus")]),
        ];
        for tests in variants {
            for configured in [
                runner(&dir),
                runner(&dir).filter("hidden"),
                runner(&dir).grep("hidden"),
                runner(&dir).grep_invert("focus"),
                runner(&dir).shard(2, 2),
                runner(&dir).project(Project::new("project").grep("hidden")),
            ] {
                let report = configured
                    .forbid_only(true)
                    .run(&browser, tests.clone())
                    .await;
                assert!(!report.ok(), "{}", report.to_list());
                assert!(report.configuration.as_ref().unwrap().forbid_only);
                assert_eq!(report.results.len(), 1);
                assert_eq!(report.results[0].name, "<forbid-only>");
                assert!(report.results[0].error.as_ref().unwrap().contains("focus"));
                assert!(report.results[0].attempt_results.is_empty());
                assert_eq!(entered.load(Ordering::SeqCst), 0);
            }
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn ordinary_failures_expected_failures_and_interruption_remain_distinct() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        for fail in [false, true] {
            let clean = runner(&dir)
                .fail_on_flaky_tests(fail)
                .run(
                    &browser,
                    vec![
                        test("pass", |_| async { Ok(()) }),
                        test("expected", |_| async {
                            Err(E2eError::Expect("expected".into()))
                        })
                        .fail(),
                        test("skip", |_| async { panic!("skip body") }).skip(),
                        test("fixme", |_| async { panic!("fixme body") }).fixme(),
                    ],
                )
                .await;
            assert!(clean.ok(), "{}", clean.to_list());
            assert_aggregate(&clean, fail, 0);
            let report = runner(&dir)
                .fail_on_flaky_tests(fail)
                .max_failures(1)
                .run(
                    &browser,
                    vec![
                        test("fails", |_| async {
                            Err(E2eError::Expect("ordinary".into()))
                        }),
                        test("not scheduled", |_| async {
                            panic!("max_failures ignored")
                        }),
                    ],
                )
                .await;
            assert!(!report.ok());
            assert_eq!(report.failed(), 1);
            assert_eq!(report.results[0].attempts, 2);
            assert_aggregate(&report, fail, 0);
            let report = runner(&dir)
                .fail_on_flaky_tests(fail)
                .run(
                    &browser,
                    vec![
                        test("expected", |_| async {
                            Err(E2eError::Expect("expected".into()))
                        })
                        .fail(),
                        test("unexpected pass", |_| async { Ok(()) }).fail(),
                    ],
                )
                .await;
            assert_eq!(report.expected_failed(), 1);
            assert_eq!(report.failed(), 1);
            assert!(report.results.iter().all(|result| result.attempts == 1));
            assert_aggregate(&report, fail, 0);
        }
        for global in [false, true] {
            let token = CancellationToken::new();
            let captured = token.clone();
            let configured = runner(&dir)
                .fail_on_flaky_tests(true)
                .with_cancellation(token)
                .global_timeout(if global {
                    Duration::from_secs(2)
                } else {
                    Duration::ZERO
                });
            let report = configured
                .run(
                    &browser,
                    vec![test("interrupted", move |_| {
                        let captured = captured.clone();
                        async move {
                            if !global {
                                captured.cancel_with_reason("user interruption");
                            }
                            std::future::pending::<E2eResult<()>>().await
                        }
                    })],
                )
                .await;
            assert!(!report.ok(), "{}", report.to_list());
            let interrupted = report
                .results
                .iter()
                .find(|result| result.name == "interrupted")
                .unwrap();
            assert_eq!(
                interrupted.attempt_results[0].status,
                AttemptStatus::Interrupted
            );
            assert_eq!(interrupted.attempts, 1);
            assert_aggregate(&report, true, 0);
        }
        browser.close().await.unwrap();
    }
}

/// Also invoked by an actual `ferrite e2e -- ...` process in the CI-policy gate.
#[tokio::test]
async fn policy_child() {
    let Some(path) = std::env::var_os("FERRITE_CI_POLICY_CHILD_REPORT") else {
        return;
    };
    let config = config_from_env().unwrap();
    let owner = Browser::launch(LaunchOptions::from_config(&config).unwrap())
        .await
        .unwrap();
    let tests = if std::env::var_os("FERRITE_CI_POLICY_CHILD_FOCUS").is_some() {
        vec![test("focus", |_| async { panic!("forbidden focus body") })
            .only()
            .skip()]
    } else {
        vec![flaky(config.fail_on_flaky_tests)]
    };
    let report = Runner::try_from_config(&config)
        .unwrap()
        .list_progress(false)
        .run(&owner, tests)
        .await;
    std::fs::write(path, report.to_json()).unwrap();
    owner.close().await.unwrap();
    std::process::exit(report.exit_code());
}

#[test]
fn isolated_environment_child_honors_ci_and_policy_overrides() {
    for (fail, focus, ci) in [
        (false, false, false),
        (true, false, false),
        (false, true, true),
        (true, true, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("child.json");
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        // Child-specific settings prevent inherited CLI/project filters from hiding the probe.
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("FERRITE_E2E_") {
                command.env_remove(name);
            }
        }
        command
            .args(["--exact", "policy_child", "--nocapture"])
            .env("CI", if ci { "true" } else { "false" })
            .env(
                "FERRITE_E2E_CONFIG",
                json!({"retries":1,"workers":1,"screenshot":"off",
                "output_dir":dir.path().display().to_string(),"fail_on_flaky_tests":!fail,
                "forbid_only":focus,"filter":if focus {Some("hidden")} else {None}})
                .to_string(),
            )
            .env("FERRITE_E2E_FAIL_ON_FLAKY_TESTS", fail.to_string())
            .env("FERRITE_E2E_FORBID_ONLY", (focus && !ci).to_string())
            .env("FERRITE_CI_POLICY_CHILD_REPORT", &path);
        if focus {
            command.env("FERRITE_CI_POLICY_CHILD_FOCUS", "1");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(i32::from(fail || focus)),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: TestReport = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            report.configuration.as_ref().unwrap().fail_on_flaky_tests,
            fail
        );
        assert_eq!(report.configuration.as_ref().unwrap().forbid_only, focus);
        assert_eq!(report.exit_code(), output.status.code().unwrap());
        if !focus {
            assert_eq!(report.results[0].status, TestStatus::Passed);
            assert_eq!(report.results[0].attempts, 2);
        }
    }
}
