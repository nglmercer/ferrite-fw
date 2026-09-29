//! Step controls, automatic lifecycle diagnostics and relocatable reports on native engines.
use ferrite_e2e::*;
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
        ..Default::default()
    })
    .workers(1)
    .test_timeout(Duration::from_secs(15))
    .cleanup_timeout(Duration::from_secs(5))
    .output_dir(dir.path().display().to_string())
    .list_progress(false)
}
fn flatten(steps: &[StepInfo]) -> Vec<&StepInfo> {
    steps
        .iter()
        .flat_map(|s| std::iter::once(s).chain(flatten(&s.steps)))
        .collect()
}

#[tokio::test]
async fn controlled_steps_skip_locally_and_expose_live_metadata() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .run(
                &browser,
                vec![test_with_context("controls", |ctx| async move {
                    let skipped = ctx
                        .page
                        .step_with(
                            "preset",
                            StepOptions::default().skip("not applicable"),
                            |_| {
                                panic!("preset skip must not construct the body");
                                #[allow(unreachable_code)]
                                async {
                                    Ok::<_, E2eError>(())
                                }
                            },
                        )
                        .await?;
                    assert_eq!(skipped, StepOutcome::Skipped("not applicable".into()));
                    let page = &ctx.page;
                    let outcome = page
                        .step_with(
                            "outer",
                            StepOptions::default().annotate("issue", "456"),
                            |step| async move {
                                assert_eq!(step.annotations()[0].description, "456");
                                assert_eq!(step.title_path().last().unwrap(), "outer");
                                step.annotate("detail", "live");
                                let nested = page
                                    .step_with(
                                        "nested",
                                        StepOptions::default(),
                                        |child| async move {
                                            assert_eq!(
                                                &child.title_path()[2..],
                                                ["outer", "nested"]
                                            );
                                            child.skip("planned fix")?;
                                            panic!("skip must exit the closure");
                                            #[allow(unreachable_code)]
                                            Ok::<_, E2eError>(())
                                        },
                                    )
                                    .await?;
                                assert_eq!(nested, StepOutcome::Skipped("planned fix".into()));
                                ctx.info
                                    .attach("inside", b"step attachment", "text/plain")?;
                                page.set_content("<h1>continuing</h1>").await?;
                                Ok(42)
                            },
                        )
                        .await?;
                    assert_eq!(outcome, StepOutcome::Completed(42));
                    page.locator("h1").expect_text("continuing").await?;
                    Ok(())
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        let attempt = &report.results[0].attempt_results[0];
        let outer = attempt.steps.iter().find(|s| s.title == "outer").unwrap();
        assert_eq!(outer.status, StepStatus::Passed);
        assert_eq!(outer.annotations.len(), 2);
        assert_eq!(outer.steps[0].status, StepStatus::Skipped);
        assert!(outer.steps[0].error.is_none());
        assert_eq!(outer.steps[1].category, StepCategory::Action);
        assert_eq!(outer.attachments.len(), 1);
        assert!(outer.location.column > 0);
        assert_eq!(outer.title_path[0], attempt.info.file);
        for text in ["planned fix", "not applicable", "issue", "456", "Skipped"] {
            assert!(report.to_html().contains(text), "missing {text}");
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn step_timeouts_preserve_outer_deadlines_and_cancellation() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .run(
                &browser,
                vec![
                    test("recovered", |page| async move {
                        let error = page
                            .step_with(
                                "short",
                                StepOptions::default().timeout(Duration::from_millis(20)),
                                |_| std::future::pending::<E2eResult<()>>(),
                            )
                            .await
                            .unwrap_err();
                        assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT");
                        let value = page
                            .step_with("zero", StepOptions::default(), |_| async {
                                tokio::time::sleep(Duration::from_millis(30)).await;
                                Ok(7)
                            })
                            .await?;
                        assert_eq!(value, StepOutcome::Completed(7));
                        Ok(())
                    }),
                    test("propagated", |page| async move {
                        page.step_with(
                            "short",
                            StepOptions::default().timeout(Duration::from_millis(20)),
                            |_| std::future::pending::<E2eResult<()>>(),
                        )
                        .await?;
                        Ok(())
                    }),
                    test("outer timeout", |page| async move {
                        page.step_with(
                            "long",
                            StepOptions::default().timeout(Duration::from_secs(10)),
                            |_| std::future::pending::<E2eResult<()>>(),
                        )
                        .await?;
                        Ok(())
                    })
                    .timeout(Duration::from_secs(1)),
                    test("outer zero", |page| async move {
                        page.step_with("zero", StepOptions::default(), |_| {
                            std::future::pending::<E2eResult<()>>()
                        })
                        .await?;
                        Ok(())
                    })
                    .timeout(Duration::from_secs(1)),
                ],
            )
            .await;
        for result in &report.results {
            let a = &result.attempt_results[0];
            if result.name == "recovered" {
                assert_eq!(a.status, AttemptStatus::Passed);
                assert_eq!(a.steps[0].status, StepStatus::TimedOut);
                assert_eq!(a.steps[1].status, StepStatus::Passed);
            } else {
                assert_eq!(a.status, AttemptStatus::TimedOut, "{}", result.name);
                assert_eq!(
                    a.steps[0].status,
                    if result.name == "propagated" {
                        StepStatus::TimedOut
                    } else {
                        StepStatus::Interrupted
                    }
                );
            }
        }
        let token = CancellationToken::new();
        let cancel = token.clone();
        let cancelled = runner(&dir)
            .with_cancellation(token)
            .run(
                &browser,
                vec![test("cancel step", move |page| {
                    let cancel = cancel.clone();
                    async move {
                        page.step_with("pending", StepOptions::default(), |_| async move {
                            cancel.cancel_with_reason("stop now");
                            std::future::pending::<E2eResult<()>>().await
                        })
                        .await?;
                        Ok(())
                    }
                })],
            )
            .await;
        let result = cancelled
            .results
            .iter()
            .find(|r| r.name == "cancel step")
            .unwrap();
        assert_eq!(result.attempt_results[0].status, AttemptStatus::Interrupted);
        assert_eq!(
            result.attempt_results[0].steps[0].status,
            StepStatus::Interrupted
        );
        browser.close().await.unwrap();
    }
}

struct Login(Page);
struct Account;
#[derive(Clone, Default)]
struct EndReport(Arc<Mutex<Option<TestReport>>>);
impl Reporter for EndReport {
    fn on_end(&self, report: &TestReport) {
        *self.0.lock().unwrap() = Some(report.clone());
    }
}
#[tokio::test]
async fn automatic_actions_hooks_and_fixtures_form_deduplicated_trees() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let end = EndReport::default();
        let tests = Suite::new("workflow")
            .before_all_with_context(
                WorkerHook::new(|ctx| async move {
                    ctx.require::<Account>()?;
                    Ok(())
                })
                .fixture::<Account>(),
            )
            .after_all_with_context(
                WorkerHook::new(|ctx| async move {
                    ctx.require::<Account>()?;
                    Ok(())
                })
                .fixture::<Account>(),
            )
            .before_each_with_context(
                ContextHook::new(|ctx| async move {
                    ctx.require::<Login>()?
                        .0
                        .locator("#name")
                        .fill("Ada")
                        .await?;
                    Ok(())
                })
                .fixture::<Login>(),
            )
            .after_each_with_context(ContextHook::new(|ctx| async move {
                ctx.page.locator("#result").expect_text("Ada").await?;
                Ok(())
            }))
            .tests(vec![test_with_context("save", |ctx| async move {
                ctx.page.locator("#save").click().await?;
                ctx.page.locator("#result").expect_text("Ada").await?;
                // A handled assertion failure remains visible without failing the test.
                assert!(ctx
                    .page
                    .locator("#result")
                    .expect()
                    .timeout(Duration::from_millis(20))
                    .text("wrong")
                    .await
                    .is_err());
                Ok(())
            })]);
        let report = runner(&dir).repeat_each(2).custom_reporter(end.clone())
            .global_setup(|| async { Ok(()) }).global_teardown(|| async { Ok(()) })
            .fixture_definition(Fixture::<Account>::new(|_| async { Ok(Account) }).scope(FixtureScope::Worker).teardown(|_| async { Ok(()) }))
            .fixture_definition(Fixture::<Login>::new(|map| async move {
                let page = map.require::<Page>()?;
                page.goto("data:text/html,<input id=name><button id=save onclick=\"document.getElementById('result').textContent=document.getElementById('name').value\">Save</button><p id=result></p>").await?;
                Ok(Login((*page).clone()))
            }).dependency::<Page>().teardown(|login| async move {
                login.0.locator("#name").clear().await?; Ok(())
            }))
            .run(&browser, tests).await;
        assert!(report.ok(), "{}", report.to_list());
        for result in &report.results {
            let a = &result.attempt_results[0];
            let all = flatten(&a.steps);
            assert_eq!(
                all.iter()
                    .filter(|s| s.title == "locator.click #save")
                    .count(),
                1
            );
            assert!(!all.iter().any(|s| s.title.contains("click_with_options")));
            let before = a.steps.iter().find(|s| s.title == "before_each").unwrap();
            assert_eq!(before.category, StepCategory::Hook);
            assert!(before.steps.iter().any(|s| s.title == "locator.fill #name"));
            let setup = flatten(&before.steps)
                .into_iter()
                .find(|s| s.category == StepCategory::Fixture)
                .unwrap();
            assert!(setup.title.contains("Login"));
            assert_eq!(setup.steps[0].title, "page.goto");
            assert!(setup.steps[0].steps.is_empty());
            let assertions: Vec<_> = all
                .iter()
                .filter(|s| s.category == StepCategory::Assertion)
                .collect();
            assert_eq!(assertions.len(), 3);
            assert!(assertions.iter().all(|s| s.steps.is_empty()));
            assert_eq!(
                assertions
                    .iter()
                    .filter(|s| s.status == StepStatus::Failed)
                    .count(),
                1
            );
            let teardown = a
                .steps
                .iter()
                .find(|s| {
                    s.category == StepCategory::Fixture
                        && s.title.contains("teardown")
                        && s.title.contains("Login")
                })
                .unwrap();
            assert_eq!(teardown.steps[0].title, "locator.clear #name");
            for s in all.into_iter().filter(|s| s.category != StepCategory::User) {
                assert_eq!(s.location.line, a.info.line);
                assert_eq!(s.location.column, 0);
            }
        }
        let lifecycle = flatten(&report.run_steps);
        for title in ["global setup", "global teardown", "worker 0", "after_all"] {
            assert!(
                lifecycle.iter().any(|s| s.title.contains(title)),
                "missing {title}: {lifecycle:?}"
            );
        }
        assert_eq!(
            lifecycle
                .iter()
                .filter(|s| s.title.contains("fixture teardown") && s.title.contains("Account"))
                .count(),
            1
        );
        assert!(lifecycle.iter().all(|s| s.status != StepStatus::Running));
        assert_eq!(
            end.0.lock().unwrap().as_ref().unwrap().run_steps.len(),
            report.run_steps.len()
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn portable_bundles_keep_real_artifacts_accessible_after_sources_are_deleted() {
    for browser in browsers().await {
        let source = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let export = tempfile::tempdir().unwrap();
        let moved = tempfile::tempdir().unwrap();
        let mut report = runner(&source)
            .video_mode(VideoMode::On)
            .run(
                &browser,
                vec![test_with_context("portable", |ctx| async move {
                    ctx.page
                        .step_with(
                            "render",
                            StepOptions::default().annotate("issue", "123"),
                            |_| async {
                                ctx.page.set_content("<h1>Portable report</h1>").await?;
                                ctx.info.attach("state", b"ready", "text/plain")?;
                                tokio::time::sleep(Duration::from_millis(450)).await;
                                Ok(())
                            },
                        )
                        .await?;
                    Ok(())
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert!(report.results[0].video.is_some());
        // Run-wide step attachments share the same deduplicated artifact copy.
        report.run_steps[0]
            .attachments
            .push(report.results[0].attachments[0].clone());
        for folder in ["a", "b"] {
            std::fs::create_dir(external.path().join(folder)).unwrap();
            let path = external.path().join(folder).join("same #\"é.txt");
            std::fs::write(&path, folder).unwrap();
            report.results[0].attachments.push(Attachment {
                name: folder.into(),
                path: path.display().to_string(),
                content_type: "text/plain".into(),
            });
        }
        let raw = report.to_json();
        let auto_export = runner(&source).write_artifacts(&report, "html,json,junit");
        assert_eq!(auto_export.len(), 3);
        let auto: TestReport =
            serde_json::from_slice(&std::fs::read(source.path().join("results.json")).unwrap())
                .unwrap();
        assert!(auto.results[0].screenshots[0].starts_with("artifacts/"));
        let bundle = report.write_bundle(export.path().join("bundle")).unwrap();
        assert_eq!(bundle.artifact_count, 6);
        assert_eq!(raw, report.to_json());
        for attachment in &report.results[0].attachments {
            assert!(Path::new(&attachment.path).exists());
        }
        std::fs::rename(export.path().join("bundle"), moved.path().join("bundle")).unwrap();
        drop(source);
        drop(external);
        drop(export);
        let root = moved.path().join("bundle");
        let portable: TestReport =
            serde_json::from_slice(&std::fs::read(root.join("results.json")).unwrap()).unwrap();
        let result = &portable.results[0];
        let a = &result.attempt_results[0];
        for path in result
            .screenshots
            .iter()
            .chain(result.trace.iter())
            .chain(result.video.iter())
            .chain(result.attachments.iter().map(|a| &a.path))
            .chain(
                flatten(&a.steps)
                    .into_iter()
                    .flat_map(|s| s.attachments.iter().map(|a| &a.path)),
            )
        {
            assert!(path.starts_with("artifacts/"));
            assert!(root.join(path).is_file());
        }
        assert_eq!(result.screenshots, a.screenshots);
        assert_eq!(result.attachments[0].path, a.steps[0].attachments[0].path);
        assert_eq!(
            portable.run_steps[0].attachments[0].path,
            result.attachments[0].path
        );
        assert_ne!(result.attachments[1].path, result.attachments[2].path);
        for (attachment, expected) in result.attachments[1..].iter().zip(["a", "b"]) {
            assert_eq!(
                std::fs::read_to_string(root.join(&attachment.path)).unwrap(),
                expected
            );
        }
        let files = Arc::new(root);
        let app = axum::Router::new().fallback(axum::routing::get(move |uri: axum::http::Uri| {
            let root = files.clone();
            async move {
                let name = uri.path().trim_start_matches('/');
                match tokio::fs::read(root.join(name)).await {
                    Ok(bytes) => (
                        axum::http::StatusCode::OK,
                        [(
                            axum::http::header::CONTENT_TYPE,
                            if name.ends_with(".html") {
                                "text/html"
                            } else if name.ends_with(".png") {
                                "image/png"
                            } else {
                                "application/octet-stream"
                            },
                        )],
                        bytes,
                    ),
                    Err(_) => (
                        axum::http::StatusCode::NOT_FOUND,
                        [(axum::http::header::CONTENT_TYPE, "text/plain")],
                        Vec::new(),
                    ),
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}/report.html")).await.unwrap();
        let checks = page.evaluate_value("Promise.all([...document.querySelectorAll('a[href]')].map(async a => [a.getAttribute('href'), (await fetch(a.href)).status]))").await.unwrap();
        let checks = checks.as_array().unwrap();
        assert!(checks.len() >= 6, "{checks:?}");
        for check in checks {
            assert_eq!(check[1], 200, "{check}");
        }
        page.goto(&format!("{base}/{}", result.screenshots[0]))
            .await
            .unwrap();
        assert!(page
            .evaluate_value("document.querySelector('img') !== null")
            .await
            .unwrap()
            .as_bool()
            .unwrap());
        page.close().await.unwrap();
        server.abort();
        browser.close().await.unwrap();
    }
}
