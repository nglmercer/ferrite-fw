use ferrite_e2e::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
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

fn metadata(value: &str) -> E2eMetadata {
    [
        ("origin".into(), serde_json::json!(value)),
        (
            "nested".into(),
            serde_json::json!([true, null, {"雪": "<&\""}]),
        ),
    ]
    .into_iter()
    .collect()
}
#[derive(Clone, Default)]
struct Observe {
    configurations: Arc<Mutex<Vec<ResolvedRunConfig>>>,
    summaries: Arc<Mutex<Vec<Vec<SlowTestSummary>>>>,
}
impl Reporter for Observe {
    fn on_configuration(&self, config: &ResolvedRunConfig) {
        self.configurations.lock().unwrap().push(config.clone());
    }
    fn on_end(&self, report: &TestReport) {
        self.summaries
            .lock()
            .unwrap()
            .push(report.slow_tests().unwrap());
    }
}
fn duplicate(retry: bool) -> Test {
    test_with_context(
        "duplicate <script>window.pwned=1</script>",
        move |ctx| async move {
            assert_eq!(
                ctx.info.config().metadata.as_ref().unwrap()["origin"],
                "builder <&"
            );
            assert_eq!(
                ctx.info.config().run_name.as_deref(),
                Some("build <script>window.pwned=1</script> &\"")
            );
            let project = ctx.info.project_config().unwrap();
            let worker = ctx.require::<WorkerInfo>()?;
            assert_eq!(worker.metadata, ctx.info.config().metadata);
            assert_eq!(worker.run_name, ctx.info.config().run_name);
            assert_eq!(worker.project_metadata, project.metadata);
            let mut owned = worker.metadata.clone().unwrap();
            owned.insert("origin".into(), serde_json::json!("changed"));
            assert_eq!(
                ctx.info.config().metadata.as_ref().unwrap()["origin"],
                "builder <&"
            );
            ctx.page.set_content("<h1>metadata test</h1>").await?;
            ctx.info.attach("marker", b"marker", "text/plain")?;
            if retry && ctx.info.retry == 0 {
                return Err(E2eError::Expect("retry this result".into()));
            }
            Ok(())
        },
    )
}
#[tokio::test]
async fn metadata_and_bounded_summaries_survive_live_callbacks_and_relocated_reports() {
    for browser in browsers().await {
        let source = tempfile::tempdir().unwrap();
        let observer = Observe::default();
        let config = E2eConfig {
            output_dir: source.path().display().to_string(),
            run_name: Some("config".into()),
            metadata: Some(metadata("config")),
            report_slow_tests: Some(Default::default()),
            projects: vec![
                E2eProjectConfig {
                    name: "red".into(),
                    metadata: Some(metadata("red <script>window.pwned=1</script>")),
                    ..Default::default()
                },
                E2eProjectConfig {
                    name: "blue".into(),
                    metadata: Some(metadata("blue")),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let report = Runner::from_config(&config)
            .workers(1)
            .retries(1)
            .repeat_each(2)
            .list_progress(false)
            .run_name("build <script>window.pwned=1</script> &\"")
            .metadata(metadata("builder <&"))
            .report_slow_tests(Some(SlowTestOptions {
                threshold_ms: 0,
                max: 3,
            }))
            .custom_reporter(observer.clone())
            .run(&browser, vec![duplicate(true), duplicate(false)])
            .await;
        assert!(report.ok(), "{}", report.to_json());
        assert_eq!(report.results.len(), 8);
        assert_eq!(
            report.results.iter().filter(|result| result.flaky).count(),
            4
        );
        let summaries = report.slow_tests().unwrap();
        assert_eq!(summaries.len(), 3);
        assert_eq!(
            summaries
                .iter()
                .map(|entry| entry.result_index)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert_eq!(observer.summaries.lock().unwrap()[0], summaries);
        {
            let configurations = observer.configurations.lock().unwrap();
            assert_eq!(configurations.len(), 1);
            assert_eq!(
                serde_json::to_value(&configurations[0]).unwrap(),
                serde_json::to_value(report.configuration.as_ref().unwrap()).unwrap()
            );
        }
        let export = tempfile::tempdir().unwrap();
        report.write_bundle(export.path().join("original")).unwrap();
        std::fs::rename(export.path().join("original"), export.path().join("moved")).unwrap();
        std::fs::remove_dir_all(source.path()).unwrap();
        let root = export.path().join("moved");
        let saved: TestReport =
            serde_json::from_slice(&std::fs::read(root.join("results.json")).unwrap()).unwrap();
        assert_eq!(saved.slow_tests().unwrap(), summaries);
        assert_eq!(
            saved.configuration.as_ref().unwrap().metadata,
            report.configuration.as_ref().unwrap().metadata
        );
        let html = std::fs::read_to_string(root.join("report.html")).unwrap();
        let xml = std::fs::read_to_string(root.join("junit.xml")).unwrap();
        let artifacts = root.join("artifacts");
        let app = axum::Router::new()
            .route(
                "/report",
                axum::routing::get(move || {
                    let html = html.clone();
                    async move { axum::response::Html(html) }
                }),
            )
            .route(
                "/junit",
                axum::routing::get(move || {
                    let xml = xml.clone();
                    async move { xml }
                }),
            )
            .route(
                "/artifacts/{name}",
                axum::routing::get(
                    move |axum::extract::Path(name): axum::extract::Path<String>| {
                        let artifacts = artifacts.clone();
                        async move { std::fs::read(artifacts.join(name)).unwrap() }
                    },
                ),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 1440,
            height: 1100,
        })
        .await
        .unwrap();
        page.goto(&format!("{url}/report")).await.unwrap();
        assert_eq!(page.locator("tr.test-result").count().await.unwrap(), 8);
        assert_eq!(
            page.locator(".slow-tests tbody tr").count().await.unwrap(),
            3
        );
        assert!(page
            .evaluate_value("window.pwned === undefined")
            .await
            .unwrap()
            .as_bool()
            .unwrap());
        let run_metadata = page
            .evaluate_value("document.querySelector('.run-metadata pre').textContent")
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<E2eMetadata>(run_metadata.as_str().unwrap()).unwrap(),
            metadata("builder <&")
        );
        let xml_data = page.evaluate_value("(async () => { const xml = new DOMParser().parseFromString(await (await fetch('/junit')).text(), 'application/xml'); return {errors: xml.querySelectorAll('parsererror').length, properties: [...xml.documentElement.querySelector(':scope > properties').children].map(node => ({name: node.getAttribute('name'), value: node.getAttribute('value')})), projects: [...xml.querySelectorAll('testcase')].map(node => ({name: node.getAttribute('classname'), value: node.querySelector('[name=\"ferrite.project.metadata\"]').getAttribute('value')}))}; })()").await.unwrap();
        assert_eq!(xml_data["errors"], 0);
        let properties = xml_data["properties"].as_array().unwrap();
        let property = |name: &str| {
            properties
                .iter()
                .find(|value| value["name"] == name)
                .unwrap()["value"]
                .as_str()
                .unwrap()
        };
        assert_eq!(
            serde_json::from_str::<E2eMetadata>(property("ferrite.run.metadata")).unwrap(),
            metadata("builder <&")
        );
        assert_eq!(
            serde_json::from_str::<Vec<SlowTestSummary>>(property("ferrite.slow_tests")).unwrap(),
            summaries
        );
        for entry in xml_data["projects"].as_array().unwrap() {
            let expected = report
                .configuration
                .as_ref()
                .unwrap()
                .project(entry["name"].as_str())
                .unwrap()
                .metadata
                .as_ref()
                .unwrap();
            assert_eq!(
                &serde_json::from_str::<E2eMetadata>(entry["value"].as_str().unwrap()).unwrap(),
                expected
            );
        }
        let project_data = page.evaluate_value("[...document.querySelectorAll('.project-metadata pre')].map(node => node.textContent)").await.unwrap();
        assert_eq!(project_data.as_array().unwrap().len(), 2);
        for (value, project) in project_data
            .as_array()
            .unwrap()
            .iter()
            .zip(&report.configuration.as_ref().unwrap().projects)
        {
            assert_eq!(
                serde_json::from_str::<E2eMetadata>(value.as_str().unwrap()).unwrap(),
                project.metadata.clone().unwrap()
            );
        }
        let downloads = page.evaluate_value("(async () => await Promise.all([...document.querySelectorAll('a[href]')].map(async link => { const response = await fetch(link.href); return {ok: response.ok, bytes: (await response.arrayBuffer()).byteLength}; })))()").await.unwrap();
        assert!(!downloads.as_array().unwrap().is_empty());
        assert!(downloads
            .as_array()
            .unwrap()
            .iter()
            .all(|value| value["ok"] == true && value["bytes"].as_u64().unwrap() > 0));
        page.evaluate_value("document.querySelectorAll('.run-metadata,.project-metadata').forEach(element => element.open=true);true").await.unwrap();
        if let Some(previews) = std::env::var_os("FERRITE_REPORT_PREVIEW_DIR") {
            std::fs::create_dir_all(&previews).unwrap();
            std::fs::write(
                Path::new(&previews).join(format!("metadata-{}.png", browser.kind().name())),
                page.screenshot(ScreenshotOptions {
                    full_page: true,
                    ..Default::default()
                })
                .await
                .unwrap(),
            )
            .unwrap();
        }
        page.close().await.unwrap();
        server.abort();
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn explicit_empty_metadata_and_disabled_summaries_round_trip() {
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let report = Runner::from_config(&E2eConfig {
            output_dir: root.path().display().to_string(),
            metadata: Some(metadata("base")),
            report_slow_tests: Some(Default::default()),
            ..Default::default()
        })
        .metadata(Default::default())
        .run_name("")
        .report_slow_tests(None)
        .list_progress(false)
        .run(
            &browser,
            vec![test_with_context("empty", |ctx| async move {
                assert!(ctx.info.config().metadata.as_ref().unwrap().is_empty());
                assert_eq!(ctx.info.config().run_name.as_deref(), Some(""));
                assert!(ctx
                    .info
                    .project_config()
                    .unwrap()
                    .metadata
                    .as_ref()
                    .unwrap()
                    .is_empty());
                assert!(ctx
                    .require::<WorkerInfo>()?
                    .project_metadata
                    .as_ref()
                    .unwrap()
                    .is_empty());
                assert!(ctx
                    .require::<WorkerInfo>()?
                    .metadata
                    .as_ref()
                    .unwrap()
                    .is_empty());
                Ok(())
            })],
        )
        .await;
        assert!(report.ok(), "{}", report.to_json());
        assert!(report.slow_tests().unwrap().is_empty());
        let copy: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert!(copy
            .configuration
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .is_empty());
        assert!(!copy.to_html().contains("class=\"slow-tests\""));
        assert!(copy.to_html().contains("class=\"run-metadata\""));
        let explicit = Runner::default()
            .output_dir(root.path().display().to_string())
            .metadata(metadata("global"))
            .project(Project::new("empty").metadata(Default::default()))
            .list_progress(false)
            .run(
                &browser,
                vec![test_with_context("replacement", |ctx| async move {
                    assert_eq!(
                        ctx.info.config().metadata.as_ref().unwrap()["origin"],
                        "global"
                    );
                    assert!(ctx
                        .info
                        .project_config()
                        .unwrap()
                        .metadata
                        .as_ref()
                        .unwrap()
                        .is_empty());
                    assert!(ctx
                        .require::<WorkerInfo>()?
                        .project_metadata
                        .as_ref()
                        .unwrap()
                        .is_empty());
                    Ok(())
                })],
            )
            .await;
        assert!(explicit.ok(), "{}", explicit.to_json());
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn environment_metadata_is_frozen_before_hooks_in_an_isolated_child() {
    const CHILD: &str = "FERRITE_METADATA_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "environment_metadata_is_frozen_before_hooks_in_an_isolated_child",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let config = E2eConfig {
            metadata: Some(metadata("config")),
            run_name: Some("config".into()),
            ..Default::default()
        };
        std::env::set_var(
            "FERRITE_E2E_CONFIG",
            serde_json::to_string(&config).unwrap(),
        );
        std::env::set_var(
            "FERRITE_E2E_METADATA",
            serde_json::to_string(&metadata("legacy")).unwrap(),
        );
        std::env::set_var("FERRITE_E2E_RUN_NAME", "legacy");
        std::env::set_var("FERRITE_E2E_SLOW_TESTS", r#"{"threshold_ms":0,"max":2}"#);
        let loaded = config_from_env().unwrap();
        assert_eq!(loaded.metadata.unwrap(), metadata("legacy"));
        assert_eq!(loaded.run_name.as_deref(), Some("legacy"));
        let report = Runner::from_env()
            .unwrap()
            .output_dir(root.path().display().to_string())
            .list_progress(false)
            .metadata(metadata("builder"))
            .run_name("builder")
            .report_slow_tests(Some(SlowTestOptions {
                threshold_ms: 0,
                max: 1,
            }))
            .before_all(|| async {
                std::env::set_var("FERRITE_E2E_METADATA", r#"{"origin":"late"}"#);
                std::env::set_var("FERRITE_E2E_RUN_NAME", "late");
                std::env::set_var("FERRITE_E2E_SLOW_TESTS", "null");
                Ok(())
            })
            .run(
                &browser,
                vec![test_with_context("frozen", |ctx| async move {
                    assert_eq!(
                        ctx.info.config().metadata.as_ref().unwrap()["origin"],
                        "builder"
                    );
                    assert_eq!(ctx.info.config().run_name.as_deref(), Some("builder"));
                    assert_eq!(ctx.info.config().report_slow_tests.unwrap().max, 1);
                    Ok(())
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_json());
        assert_eq!(report.slow_tests().unwrap().len(), 1);
        browser.close().await.unwrap();
    }
}
