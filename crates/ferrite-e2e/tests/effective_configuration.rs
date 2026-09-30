//! Effective scheduling snapshots, library/CLI projects, native settings and portable reports.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    path::Path,
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
            let mut options = LaunchOptions::default().browser(kind).executable(path);
            options.ignore_https_errors = true;
            let browser = Browser::launch(options).await.unwrap();
            eprintln!(
                "effective configuration {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            browsers.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
fn runner(dir: &tempfile::TempDir) -> Runner {
    Runner::from_config(&E2eConfig {
        screenshot: "off".into(),
        ..Default::default()
    })
    .output_dir(dir.path().display().to_string())
    .workers(2)
    .test_timeout(Duration::from_secs(10))
    .cleanup_timeout(Duration::from_secs(3))
    .list_progress(false)
}
#[derive(Clone, Default)]
struct Events {
    config: Arc<Mutex<Vec<Value>>>,
    attempts: Arc<Mutex<Vec<(AttemptInfo, Value)>>>,
    order: Arc<Mutex<Vec<String>>>,
}
impl Reporter for Events {
    fn on_configuration(&self, config: &ResolvedRunConfig) {
        self.config
            .lock()
            .unwrap()
            .push(serde_json::to_value(config).unwrap());
        self.order.lock().unwrap().push("configuration".into());
    }
    fn on_test_configuration(&self, info: &AttemptInfo, settings: &ResolvedTestSettings) {
        self.attempts
            .lock()
            .unwrap()
            .push((info.clone(), serde_json::to_value(settings).unwrap()));
        self.order.lock().unwrap().push(format!(
            "settings:{}:{}:{}",
            info.name, info.repeat_each_index, info.retry
        ));
    }
    fn on_test_begin(&self, info: &AttemptInfo) {
        self.order.lock().unwrap().push(format!(
            "begin:{}:{}:{}",
            info.name, info.repeat_each_index, info.retry
        ));
    }
}
fn project_tests() -> Vec<Test> {
    vec![test_with_context("probe", |ctx| async move {
        let settings = ctx.info.settings();
        let project = ctx.info.project_config().unwrap();
        assert_eq!(settings.project, project.name);
        assert_eq!(settings.browser, ctx.context.browser().unwrap().kind());
        assert_eq!(settings.browser_version, ctx.context.browser().unwrap().version().await?);
        assert!(settings.context.ignore_https_errors);
        assert_eq!(ctx.info.config().projects.len(), if ctx.info.config().selected_projects.is_empty() { 2 } else { 1 });
        let viewport:Vec<u32> = ctx.page.evaluate("[innerWidth,innerHeight]").await?;
        let initial = json!({"project":settings.project,"repeat":settings.repeat_each_index,
            "repeatEach":settings.repeat_each,"retries":settings.retries,"timeout":settings.timeout_ms,"viewport":viewport});
        let initial_timeout = settings.timeout_ms;
        ctx.info.set_timeout(Duration::from_millis(9000));
        assert_eq!(ctx.info.settings().timeout_ms,9000);
        assert_eq!(ctx.info.project_config().unwrap().timeout_ms,initial_timeout);
        let mut observed=initial;
        observed["currentTimeout"]=json!(ctx.info.settings().timeout_ms);
        ctx.info.attach("settings",&serde_json::to_vec(&observed)?,"application/json")?;
        Ok(())
    }).tag("focus"),
    test("excluded", |_| async { panic!("project exclusion failed") }).tag("focus").tag("blocked"),
    test("not selected", |_| async { panic!("project inclusion failed") })]
}
fn observations(report: &TestReport) -> Vec<Value> {
    let mut values = report
        .results
        .iter()
        .flat_map(|result| &result.attempt_results)
        .flat_map(|attempt| &attempt.attachments)
        .filter(|attachment| attachment.name == "settings")
        .map(|attachment| {
            serde_json::from_slice::<Value>(&std::fs::read(&attachment.path).unwrap()).unwrap()
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|value| {
        (
            value["project"].as_str().unwrap().to_string(),
            value["repeat"].as_u64().unwrap(),
        )
    });
    values
}
#[tokio::test]
async fn pinned_projects_filters_repetitions_selection_and_shard_match_native_settings() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/configuration-reference.json"
    ))
    .unwrap();
    assert_eq!(reference["playwright"], "1.63.0");
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let events = Events::default();
        let configured = runner(&dir)
            .retries(2)
            .repeat_each(3)
            .context_options(ContextOptions::default().viewport(360, 240))
            .project(
                Project::new("alpha")
                    .grep("focus")
                    .grep_invert("blocked")
                    .repeat_each(2)
                    .retries(1)
                    .timeout(Duration::from_millis(8000))
                    .context_options(ContextOptions::default().viewport(480, 320)),
            )
            .project(
                Project::new("beta")
                    .grep("focus")
                    .grep_invert("blocked")
                    .repeat_each(0)
                    .retries(0)
                    .timeout(Duration::ZERO)
                    .context_options(ContextOptions::default().viewport(600, 400)),
            );
        let planned = configured.resolve_config(&browser).await.unwrap();
        assert_eq!(planned.browser, browser.kind());
        assert_eq!(planned.browser_version, browser.version().await.unwrap());
        assert_eq!(planned.project(Some("beta")).unwrap().repeat_each, 1);
        assert_eq!(planned.project(Some("beta")).unwrap().timeout_ms, 0);
        let report = configured
            .clone()
            .custom_reporter(events.clone())
            .run(&browser, project_tests())
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(
            observations(&report),
            reference["cases"].as_array().unwrap().clone()
        );
        let serialized = serde_json::to_value(report.configuration.as_ref().unwrap()).unwrap();
        assert_eq!(*events.config.lock().unwrap(), vec![serialized]);
        let order = events.order.lock().unwrap().clone();
        assert_eq!(order[0], "configuration");
        for (position, item) in order.iter().enumerate() {
            if let Some(suffix) = item.strip_prefix("begin:") {
                assert!(order[..position].contains(&format!("settings:{suffix}")));
            }
        }
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            let recorded = events
                .attempts
                .lock()
                .unwrap()
                .iter()
                .find(|(info, _)| {
                    info.name == result.name && info.repeat_each_index == result.repeat_each_index
                })
                .unwrap()
                .1
                .clone();
            let settings = attempt.settings.as_ref().unwrap();
            assert_eq!(
                recorded["timeout_ms"],
                if result.project.as_deref() == Some("alpha") {
                    json!(8000)
                } else {
                    json!(0)
                }
            );
            assert_eq!(settings.timeout_ms, 9000);
            assert_eq!(recorded["output_dir"], settings.output_dir);
            assert_eq!(
                settings.context.viewport.as_ref().unwrap().width,
                if result.project.as_deref() == Some("alpha") {
                    480
                } else {
                    600
                }
            );
        }
        let selected = configured
            .clone()
            .selected_projects(["beta"])
            .run(&browser, project_tests())
            .await;
        assert!(selected.ok(), "{}", selected.to_list());
        assert_eq!(
            observations(&selected),
            reference["selected"].as_array().unwrap().clone()
        );
        let sharded = configured.shard(2, 2).run(&browser, project_tests()).await;
        assert!(sharded.ok(), "{}", sharded.to_list());
        assert_eq!(sharded.results.len(), 1);
        assert_eq!(sharded.results[0].project.as_deref(), Some("alpha"));
        assert_eq!(sharded.results[0].repeat_each_index, 1);
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn suite_test_overrides_retry_isolation_runtime_timeout_and_owned_copies() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let rows = seen.clone();
        let inherited = test_with_context("inherit", move |mut ctx| {
            let rows = rows.clone();
            async move {
                let before = ctx.info.settings();
                let viewport: Vec<u32> = ctx.page.evaluate("[innerWidth,innerHeight]").await?;
                assert_eq!(viewport, [700, 500]);
                assert_eq!(before.timeout_ms, 4500);
                assert_eq!(before.retries, 1);
                assert!(!before.context.has_touch);
                assert!(before.context.ignore_https_errors);
                let mut copy = before.clone();
                copy.context.viewport = None;
                copy.output_dir = "unrelated".into();
                assert_eq!(ctx.info.settings().context, before.context);
                assert_eq!(ctx.info.settings().output_dir, ctx.info.output_dir);
                let original = ctx.info.project.clone();
                ctx.info.project = Some("edited public field".into());
                assert_eq!(ctx.info.project_config().unwrap().name, original);
                assert_eq!(ctx.info.settings().project, original);
                rows.lock()
                    .unwrap()
                    .push((before.project.clone(), ctx.info.retry));
                ctx.info.set_timeout(Duration::from_millis(6000));
                if ctx.info.retry == 0 {
                    return Err(E2eError::Expect("retry once".into()));
                }
                Ok(())
            }
        });
        let explicit = test_with_context("explicit", |ctx| async move {
            let settings = ctx.info.settings();
            let viewport: Vec<u32> = ctx.page.evaluate("[innerWidth,innerHeight]").await?;
            assert_eq!(viewport, [800, 600]);
            assert_eq!(settings.retries, 0);
            assert_eq!(settings.timeout_ms, 3000);
            ctx.info.set_timeout(Duration::ZERO);
            assert_eq!(ctx.info.settings().timeout_ms, 0);
            assert_eq!(ctx.info.config().timeout_ms, 10000);
            Ok(())
        })
        .retries(0)
        .timeout(Duration::from_secs(1))
        .slow()
        .context_options(ContextOptions::default().viewport(800, 600));
        let tests = Suite::new("suite")
            .retries(1)
            .timeout(Duration::from_millis(4500))
            .context_options(ContextOptions::default().viewport(700, 500))
            .tests(vec![inherited, explicit]);
        let report = runner(&dir)
            .retries(2)
            .context_options(ContextOptions::default().viewport(360, 240).has_touch(true))
            .project(
                Project::new("alpha")
                    .retries(3)
                    .timeout(Duration::from_secs(8))
                    .context_options(ContextOptions::default().viewport(480, 320)),
            )
            .project(
                Project::new("beta")
                    .retries(0)
                    .timeout(Duration::ZERO)
                    .context_options(ContextOptions::default().viewport(600, 400)),
            )
            .run(&browser, tests)
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(report.results.len(), 4);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 4);
        for result in &report.results {
            if result.name.ends_with("inherit") {
                assert!(result.flaky);
                assert_eq!(result.attempt_results.len(), 2);
                for attempt in &result.attempt_results {
                    assert_eq!(attempt.settings.as_ref().unwrap().timeout_ms, 6000);
                }
            } else {
                assert_eq!(result.attempt_results.len(), 1);
                assert_eq!(
                    result.attempt_results[0]
                        .settings
                        .as_ref()
                        .unwrap()
                        .timeout_ms,
                    0
                );
            }
        }
        let mut old = serde_json::to_value(&report).unwrap();
        old.as_object_mut().unwrap().remove("configuration");
        for result in old["results"].as_array_mut().unwrap() {
            for attempt in result["attempt_results"].as_array_mut().unwrap() {
                attempt.as_object_mut().unwrap().remove("settings");
            }
        }
        let historical: TestReport = serde_json::from_value(old).unwrap();
        assert!(historical.configuration.is_none());
        assert!(historical
            .results
            .iter()
            .flat_map(|r| &r.attempt_results)
            .all(|attempt| attempt.settings.is_none()));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn shared_projects_launch_actual_engines_and_use_resolved_artifact_and_snapshot_paths() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let root_kind = browser.kind();
        let other = if root_kind == BrowserKind::Chromium {
            BrowserKind::Firefox
        } else {
            BrowserKind::Chromium
        };
        let output = dir.path().join("project <output>");
        let baseline = dir.path().join("project baseline");
        let supplied_output = dir.path().join("supplied output");
        let config = E2eConfig {
            // These launch inputs deliberately do not describe the supplied owner.
            browser: other.name().into(),
            executable_path: Some(
                if other == BrowserKind::Chromium {
                    find_chromium(None).unwrap()
                } else {
                    find_firefox(None).unwrap()
                }
                .display()
                .to_string(),
            ),
            output_dir: dir.path().join("global output").display().to_string(),
            timeout_ms: 15000,
            cleanup_timeout_ms: 4000,
            workers: 1,
            screenshot: "on".into(),
            update_snapshots: "missing".into(),
            projects: vec![
                E2eProjectConfig {
                    name: "dedicated <script>".into(),
                    browser: Some(other.name().into()),
                    output_dir: Some(output.display().to_string()),
                    snapshot_dir: Some(baseline.display().to_string()),
                    viewport: Some(ferrite_config::ViewportConfig {
                        width: 320,
                        height: 240,
                    }),
                    ..Default::default()
                },
                E2eProjectConfig {
                    name: "supplied".into(),
                    output_dir: Some(supplied_output.display().to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let events = Events::default();
        let configured = Runner::try_from_config(&config)
            .unwrap()
            .list_progress(false)
            .custom_reporter(events.clone());
        let planned = configured.resolve_config(&browser).await.unwrap();
        assert_eq!(planned.browser, root_kind);
        let dedicated = planned.project(Some("dedicated <script>")).unwrap();
        assert_eq!(dedicated.browser, other);
        assert!(dedicated.browser_version.is_none());
        assert!(dedicated.launch_options.is_some());
        assert_eq!(dedicated.snapshot_dir, baseline.display().to_string());
        assert_eq!(
            planned.project(Some("supplied")).unwrap().snapshot_dir,
            supplied_output.join("snapshots").display().to_string()
        );
        let owners = Arc::new(Mutex::new(Vec::<Browser>::new()));
        let captured = owners.clone();
        let report = configured
            .run(
                &browser,
                vec![test_with_context("paths", move |ctx| {
                    let captured = captured.clone();
                    async move {
                        let owner = ctx.context.browser().unwrap();
                        assert_eq!(ctx.info.settings().browser, owner.kind());
                        assert_eq!(ctx.info.settings().browser_version, owner.version().await?);
                        captured.lock().unwrap().push(owner);
                        ctx.page
                            .set_content(
                                "<style>body{margin:0;background:white}</style><h1>snapshot</h1>",
                            )
                            .await?;
                        ctx.info
                            .attach("attachment <script>", b"bytes", "text/plain")?;
                        assert!(ctx
                            .info
                            .output_path("marker.txt")?
                            .starts_with(&ctx.info.settings().project_output_dir));
                        ctx.page
                            .locator("h1")
                            .expect()
                            .screenshot("inherited")
                            .await?;
                        let explicit = SnapshotOptions {
                            dir: Some(Path::new(&ctx.info.output_dir).join("explicit-baseline")),
                            update: Some(SnapshotUpdate::None),
                            ..Default::default()
                        };
                        assert!(ctx
                            .page
                            .locator("h1")
                            .expect()
                            .timeout(Duration::from_millis(40))
                            .screenshot_with("missing-explicit", &explicit)
                            .await
                            .is_err());
                        assert!(!explicit.dir.unwrap().join("missing-explicit.png").exists());
                        Ok(())
                    }
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        let resolved = report.configuration.as_ref().unwrap();
        assert_eq!(
            *events.config.lock().unwrap(),
            vec![serde_json::to_value(resolved).unwrap()]
        );
        assert!(baseline.join("inherited.png").is_file());
        assert!(supplied_output.join("snapshots/inherited.png").is_file());
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            let settings = attempt.settings.as_ref().unwrap();
            assert!(!attempt.screenshots.is_empty());
            assert!(attempt.trace.is_some());
            for path in attempt
                .screenshots
                .iter()
                .chain(attempt.trace.iter())
                .chain(
                    attempt
                        .attachments
                        .iter()
                        .map(|attachment| &attachment.path),
                )
            {
                assert!(Path::new(path).is_file(), "{path}");
                assert!(
                    Path::new(path).starts_with(&settings.project_output_dir),
                    "{path}"
                );
            }
        }
        let owner_snapshots = owners.lock().unwrap().clone();
        assert_eq!(owner_snapshots.len(), 2);
        for owner in &owner_snapshots {
            assert_eq!(owner.is_connected(), owner.kind() == root_kind);
        }
        assert!(browser.is_connected());
        let bundle = report.write_bundle(dir.path().join("bundle")).unwrap();
        assert!(bundle.artifact_count >= 6);
        let moved = dir.path().join("relocated");
        std::fs::rename(bundle.html.parent().unwrap(), &moved).unwrap();
        let exported: TestReport =
            serde_json::from_slice(&std::fs::read(moved.join("results.json")).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(exported.configuration).unwrap(),
            serde_json::to_value(report.configuration.clone()).unwrap()
        );
        for result in exported.results {
            for attempt in result.attempt_results {
                for path in attempt
                    .screenshots
                    .iter()
                    .chain(attempt.trace.iter())
                    .chain(attempt.attachments.iter().map(|a| &a.path))
                {
                    assert!(moved.join(path).is_file());
                }
            }
        }
        let html = std::fs::read_to_string(moved.join("report.html")).unwrap();
        assert!(html.contains("Effective configuration"));
        assert!(html.contains("Effective settings"));
        assert!(html.contains("dedicated &lt;script&gt;"));
        assert!(!html.contains("dedicated <script>"));
        if let Some(preview) = std::env::var_os("FERRITE_CONFIG_REPORT_PREVIEW") {
            let preview = Path::new(&preview).join(root_kind.name());
            let bundle = report.write_bundle(&preview).unwrap();
            let page = browser.new_page().await.unwrap();
            page.goto(&format!("file://{}", bundle.html.display()))
                .await
                .unwrap();
            page.evaluate::<Value>("document.querySelectorAll('details').forEach(d=>{if(d.querySelector('summary')?.textContent.includes('Effective configuration'))d.open=true});null").await.unwrap();
            page.save_screenshot(
                &preview.join("report-preview.png"),
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
async fn configuration_validation_and_early_setup_failure_publish_consistent_snapshots() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        for invalid in [
            runner(&dir)
                .project(Project::new("duplicate"))
                .project(Project::new("duplicate")),
            runner(&dir).project(Project::new(" ")),
            runner(&dir).selected_projects(["unknown"]),
            Runner::from_config(&E2eConfig {
                shard: Some((0, 2)),
                output_dir: dir.path().display().to_string(),
                ..Default::default()
            }),
            runner(&dir).snapshot_dir(""),
            runner(&dir).project(Project {
                name: "conflict".into(),
                browser: Some(BrowserKind::Firefox),
                launch_options: Some(LaunchOptions::default().browser(BrowserKind::Chromium)),
                ..Default::default()
            }),
        ] {
            assert!(invalid.resolve_config(&browser).await.is_err());
            let report = invalid
                .run(
                    &browser,
                    vec![test("never", |_| async {
                        panic!("invalid settings must fail before body")
                    })],
                )
                .await;
            assert!(!report.ok());
            assert_eq!(report.results[0].name, "<configuration>");
            assert!(report.configuration.is_none());
            assert!(browser.is_connected());
        }
        let events = Events::default();
        let report = runner(&dir)
            .custom_reporter(events.clone())
            .project(
                Project::new("never launched")
                    .launch_options(LaunchOptions::default().browser(browser.kind())),
            )
            .global_setup(|| async { Err(E2eError::Expect("setup abort".into())) })
            .run(
                &browser,
                vec![test("never", |_| async { panic!("setup failed") })],
            )
            .await;
        assert!(!report.ok());
        assert!(report.configuration.as_ref().unwrap().projects[0]
            .browser_version
            .is_none());
        assert_eq!(
            *events.config.lock().unwrap(),
            vec![serde_json::to_value(report.configuration.as_ref().unwrap()).unwrap()]
        );
        assert!(events.attempts.lock().unwrap().is_empty());
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn legacy_environment_is_frozen_before_hooks_in_an_isolated_child() {
    if std::env::var_os("FERRITE_CONFIG_ENV_CHILD").is_none() {
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "legacy_environment_is_frozen_before_hooks_in_an_isolated_child",
                "--nocapture",
            ])
            .env("FERRITE_CONFIG_ENV_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        assert!(String::from_utf8_lossy(&child.stdout).contains("1 passed"));
        return;
    }
    // Only this test runs in the child process; no parent environment or concurrent test is mutated.
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let baseline = dir.path().join("legacy baseline");
        for (name, value) in [
            ("FERRITE_E2E_FILTER", "include".to_string()),
            ("FERRITE_E2E_GREP", "keep".to_string()),
            ("FERRITE_E2E_GREP_INVERT", "blocked".to_string()),
            ("FERRITE_E2E_SHARD", "1/1".to_string()),
            ("FERRITE_E2E_PROJECT", "desktop,desktop".to_string()),
            ("FERRITE_SNAPSHOT_DIR", baseline.display().to_string()),
            ("FERRITE_UPDATE_SNAPSHOTS", "all".to_string()),
        ] {
            std::env::set_var(name, value);
        }
        let configured = runner(&dir)
            .snapshot_update(SnapshotUpdate::All)
            .project(Project::new("desktop").repeat_each(1))
            .global_setup(|| async {
                for (name, value) in [
                    ("FERRITE_E2E_FILTER", "no match"),
                    ("FERRITE_E2E_GREP", "no match"),
                    ("FERRITE_E2E_PROJECT", "unknown"),
                    ("FERRITE_SNAPSHOT_DIR", "changed baseline"),
                    ("FERRITE_UPDATE_SNAPSHOTS", "none"),
                ] {
                    std::env::set_var(name, value);
                }
                Ok(())
            });
        let report = configured
            .run(
                &browser,
                vec![
                    test_with_context("include keep", |ctx| async move {
                        assert_eq!(ctx.info.config().filter.as_deref(), Some("include"));
                        assert_eq!(ctx.info.config().grep.as_deref(), Some("keep"));
                        assert_eq!(ctx.info.config().selected_projects, ["desktop"]);
                        assert_eq!(ctx.info.settings().snapshot_update, SnapshotUpdate::All);
                        ctx.page
                            .set_content("<h1>fixed snapshot inputs</h1>")
                            .await?;
                        ctx.page.locator("h1").expect().screenshot("frozen").await?;
                        Ok(())
                    }),
                    test("include keep blocked", |_| async {
                        panic!("inverse selection ignored")
                    }),
                ],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(report.results.len(), 1);
        assert_eq!(
            report.configuration.as_ref().unwrap().snapshot_dir,
            baseline.display().to_string()
        );
        assert!(baseline.join("frozen.png").is_file());
        browser.close().await.unwrap();
    }
}
