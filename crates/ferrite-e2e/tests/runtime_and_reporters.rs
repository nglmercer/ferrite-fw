//! Attempt resources, live reporting and runtime controls on native engines.
use ferrite_e2e::*;
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
            eprintln!("validating {kind:?}: {}", path.display());
            result.push(
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap(),
            );
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::default()
        .workers(1)
        .test_timeout(Duration::from_secs(15))
        .cleanup_timeout(Duration::from_secs(5))
        .output_dir(dir.path().display().to_string())
        .list_progress(false)
}
#[derive(Clone)]
struct Events(Arc<Mutex<Vec<String>>>);
impl Events {
    fn push(&self, value: impl Into<String>) {
        self.0.lock().unwrap().push(value.into());
    }
    fn read(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}
impl Reporter for Events {
    fn on_begin(&self, _: &[Test]) {
        self.push("run+");
    }
    fn on_test_begin(&self, a: &AttemptInfo) {
        self.push(format!("test+ {} {}", a.name, a.retry));
    }
    fn on_test_end(&self, a: &AttemptInfo, r: &TestResult) {
        self.push(format!("test- {} {} {:?}", a.name, a.retry, r.status));
        if let Some(trace) = &r.trace {
            self.push(format!(
                "trace {} {} {}",
                a.name,
                a.retry,
                std::path::Path::new(trace)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
            ));
        }
        self.push(format!(
            "attachments {} {} {}",
            a.name,
            a.retry,
            r.attachments.len()
        ));
    }
    fn on_step_begin(&self, _: &AttemptInfo, s: &StepInfo) {
        self.push(format!("step+ {}", s.title));
        if s.interrupted {
            self.push("invalid begin interruption");
        }
    }
    fn on_step_end(&self, _: &AttemptInfo, s: &StepInfo) {
        self.push(format!("step- {} {}", s.title, s.interrupted));
    }
    fn on_attachment(&self, _: &AttemptInfo, a: &Attachment) {
        self.push(format!("attach {}", a.name));
    }
    fn on_error(&self, a: Option<&AttemptInfo>, _: &str) {
        self.push(if a.is_some() {
            "error attempt"
        } else {
            "error global"
        });
    }
    fn on_end(&self, _: &TestReport) {
        self.push("run-");
    }
}
struct Login(Page);
struct Account;

#[tokio::test]
async fn builtin_dependencies_and_context_hooks_have_ordered_lifetimes() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let log = Events(Arc::default());
        let worker_up = log.clone();
        let worker_down = log.clone();
        let login_up = log.clone();
        let login_down = log.clone();
        let before_all = log.clone();
        let after_all = log.clone();
        let before = log.clone();
        let body = log.clone();
        let after = log.clone();
        let tests = Suite::new("login")
            .before_all_with_context(
                WorkerHook::new(move |ctx| {
                    let log = before_all.clone();
                    async move {
                        ctx.require::<Account>()?;
                        assert_eq!(
                            ctx.info.worker_index,
                            ctx.require::<WorkerInfo>()?.worker_index
                        );
                        assert_eq!(ctx.browser.kind(), ctx.require::<Browser>()?.kind());
                        log.push("beforeAll");
                        Ok(())
                    }
                })
                .fixture::<Account>(),
            )
            .after_all_with_context(
                WorkerHook::new(move |ctx| {
                    let log = after_all.clone();
                    async move {
                        ctx.require::<Account>()?;
                        log.push("afterAll");
                        Ok(())
                    }
                })
                .fixture::<Account>(),
            )
            .before_each_with_context(
                ContextHook::new(move |ctx| {
                    let log = before.clone();
                    async move {
                        ctx.require::<Login>()?;
                        ctx.info.annotate("hook", "login ready");
                        ctx.context
                            .add_cookies(
                                &[Cookie {
                                    name: "browser-only".into(),
                                    value: "session".into(),
                                    domain: Some("example.test".into()),
                                    path: Some("/".into()),
                                    http_only: false,
                                    secure: false,
                                    same_site: None,
                                    expires: None,
                                }],
                                "http://example.test",
                            )
                            .await?;
                        assert_eq!(ctx.request.storage_state().await?.cookies.len(), 0);
                        assert_eq!(ctx.context.cookies().await?.len(), 1);
                        assert_eq!(ctx.context.pages().len(), 1);
                        log.push("beforeEach");
                        Ok(())
                    }
                })
                .fixture::<Login>(),
            )
            .after_each_with_context(
                ContextHook::new(move |ctx| {
                    let log = after.clone();
                    async move {
                        assert!(!ctx.require::<Login>()?.0.is_closed());
                        log.push("afterEach");
                        Ok(())
                    }
                })
                .fixture::<Login>(),
            )
            .tests(vec![test_with_context("ready", move |ctx| {
                let log = body.clone();
                async move {
                    assert_eq!(
                        ctx.page
                            .evaluate_value("document.body.dataset.login")
                            .await?,
                        "ready"
                    );
                    assert!(ctx
                        .info
                        .annotations()
                        .iter()
                        .any(|(kind, _)| kind == "hook"));
                    log.push("body");
                    Ok(())
                }
            })]);
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<Login>::new(move |map| {
                    let log = login_up.clone();
                    async move {
                        let page = map.require::<Page>()?;
                        map.require::<BrowserContext>()?;
                        map.require::<ApiClient>()?;
                        map.require::<TestInfo>()?.annotate("fixture", "login");
                        page.evaluate_value("document.body.dataset.login = 'ready'")
                            .await?;
                        log.push("login+");
                        Ok(Login((*page).clone()))
                    }
                })
                .dependency::<Page>()
                .dependency::<BrowserContext>()
                .dependency::<ApiClient>()
                .dependency::<TestInfo>()
                .teardown(move |_| {
                    let log = login_down.clone();
                    async move {
                        log.push("login-");
                        Ok(())
                    }
                }),
            )
            .fixture_definition(
                Fixture::<Account>::new(move |map| {
                    let log = worker_up.clone();
                    async move {
                        map.require::<Browser>()?;
                        map.require::<WorkerInfo>()?;
                        log.push("account+");
                        Ok(Account)
                    }
                })
                .scope(FixtureScope::Worker)
                .dependency::<Browser>()
                .dependency::<WorkerInfo>()
                .teardown(move |_| {
                    let log = worker_down.clone();
                    async move {
                        log.push("account-");
                        Ok(())
                    }
                }),
            )
            .run(&browser, tests)
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(
            log.read(),
            [
                "account+",
                "beforeAll",
                "login+",
                "beforeEach",
                "body",
                "afterEach",
                "login-",
                "afterAll",
                "account-"
            ]
        );
        assert_eq!(report.results[0].annotations.len(), 2);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn reporter_events_are_live_and_attempt_specific_even_on_retry_and_cancellation() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let log = Events(Arc::default());
        let body = log.clone();
        let next = log.clone();
        let hook = log.clone();
        let report = runner(&dir)
            .custom_reporter(log.clone())
            .retries(1)
            .after_each(move |_| {
                let log = hook.clone();
                async move {
                    log.push("cleanup");
                    Ok(())
                }
            })
            .run(
                &browser,
                vec![
                    test_with_context("retry", move |ctx| {
                        let log = body.clone();
                        async move {
                            assert!(log
                                .read()
                                .contains(&format!("test+ retry {}", ctx.info.retry)));
                            ctx.page
                                .step("read", async {
                                    assert!(log.read().contains(&"step+ read".into()));
                                })
                                .await;
                            ctx.info.attach("state", b"attempt", "text/plain")?;
                            if ctx.info.retry == 0 {
                                Err(E2eError::Expect("retry".into()))
                            } else {
                                Ok(())
                            }
                        }
                    }),
                    test("next", move |_| {
                        let log = next.clone();
                        async move {
                            assert!(log.read().contains(&"test- retry 1 Passed".into()));
                            Ok(())
                        }
                    }),
                ],
            )
            .await;
        assert_eq!(report.exit_code(), 0, "{}", report.to_list());
        assert_eq!(
            report
                .results
                .iter()
                .find(|r| r.name == "retry")
                .unwrap()
                .attachments
                .len(),
            2
        );
        let events = log.read();
        assert!(!events.contains(&"invalid begin interruption".into()));
        assert_eq!(events.first().unwrap(), "run+");
        assert_eq!(events.last().unwrap(), "run-");
        assert_eq!(events.iter().filter(|e| e.starts_with("test+")).count(), 3);
        let failed_end = events
            .iter()
            .position(|e| e == "test- retry 0 Failed")
            .unwrap();
        assert_eq!(events[failed_end - 1], "error attempt");
        assert_eq!(events[failed_end - 2], "cleanup");
        assert!(events.contains(&"attachments retry 0 1".into()));
        assert!(events.contains(&"attachments retry 1 1".into()));
        assert!(events.contains(&"trace retry 0 retry-attempt1.json".into()));
        assert!(events.contains(&"trace retry 1 retry-attempt2.json".into()));
        let cancel = CancellationToken::new();
        let abort = cancel.clone();
        let log = Events(Arc::default());
        let body = log.clone();
        let report = runner(&dir)
            .custom_reporter(log.clone())
            .with_cancellation(cancel)
            .run(
                &browser,
                vec![test_with_context("interrupted", move |ctx| {
                    let abort = abort.clone();
                    let log = body.clone();
                    async move {
                        ctx.page
                            .step("pending", async {
                                assert!(log.read().contains(&"step+ pending".into()));
                                abort.cancel();
                                std::future::pending::<()>().await;
                            })
                            .await;
                        Ok(())
                    }
                })],
            )
            .await;
        assert_ne!(report.exit_code(), 0);
        assert!(log.read().contains(&"step- pending true".into()));
        assert!(log.read().contains(&"test- interrupted 0 Failed".into()));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn runtime_skip_expected_failure_annotations_and_cleanup_failures_are_reported() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .retries(2)
            .run(
                &browser,
                vec![
                    test_with_context("skip", |ctx| async move {
                        ctx.info.skip("unavailable")?;
                        panic!("skip must stop body")
                    }),
                    test_with_context("expected", |ctx| async move {
                        ctx.info.fail("known issue");
                        Err(E2eError::Expect("known".into()))
                    }),
                    test_with_context("unexpected pass", |ctx| async move {
                        ctx.info.fail("known issue");
                        Ok(())
                    }),
                ],
            )
            .await;
        let skipped = report.results.iter().find(|r| r.name == "skip").unwrap();
        assert_eq!(skipped.status, TestStatus::Skipped);
        assert_eq!(skipped.attempts, 1);
        assert_eq!(skipped.annotations, [("skip".into(), "unavailable".into())]);
        let expected = report
            .results
            .iter()
            .find(|r| r.name == "expected")
            .unwrap();
        assert_eq!(expected.status, TestStatus::FailedExpected);
        assert_eq!(expected.attempts, 1);
        let passed = report
            .results
            .iter()
            .find(|r| r.name == "unexpected pass")
            .unwrap();
        assert_eq!(passed.status, TestStatus::Failed);
        assert_eq!(passed.attempts, 1);
        let report = runner(&dir)
            .after_each(|_| async { Err(E2eError::Expect("cleanup failure".into())) })
            .run(
                &browser,
                vec![test_with_context("skip cleanup", |ctx| async move {
                    ctx.info.skip("skip")
                })],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Failed);
        assert!(report.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("cleanup failure"));
        let report = runner(&dir)
            .fixture_definition(
                Fixture::<Login>::new(|map| async move {
                    map.require::<TestInfo>()?.skip("fixture skip")?;
                    panic!("skip must stop setup")
                })
                .dependency::<TestInfo>(),
            )
            .run(
                &browser,
                vec![
                    test("fixture skip", |_| async { panic!("body must not run") })
                        .fixture::<Login>(),
                ],
            )
            .await;
        assert_eq!(
            report.results[0].status,
            TestStatus::Skipped,
            "{}",
            report.to_list()
        );
        let report = runner(&dir)
            .run(
                &browser,
                vec![test_with_context("expected timeout", |ctx| async move {
                    ctx.info.fail("known issue");
                    ctx.info.set_timeout(Duration::from_millis(1));
                    std::future::pending::<E2eResult<()>>().await
                })],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Failed);
        assert!(report.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("timed out"));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn invalid_worker_hook_scope_emits_run_errors_without_running_setup() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let log = Events(Arc::default());
        let tests = Suite::new("bad")
            .before_all_with_context(
                WorkerHook::new(|_| async { panic!("scope validation runs first") })
                    .fixture::<Page>(),
            )
            .tests(vec![test("body", |_| async {
                panic!("invalid suite must not run")
            })]);
        let report = runner(&dir)
            .custom_reporter(log.clone())
            .global_setup(|| async { panic!("validation before setup") })
            .run(&browser, tests)
            .await;
        assert_eq!(report.results.len(), 1);
        assert_ne!(report.exit_code(), 0);
        assert_eq!(log.read(), ["run+", "error global", "run-"]);
        browser.close().await.unwrap();
    }
}
