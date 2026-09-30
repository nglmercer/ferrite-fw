//! Screenshot diagnostics remain owned by their attempt and survive portable export.
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
            let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .unwrap();
            eprintln!(
                "snapshot artifacts {} {}",
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
        workers: 1,
        timeout_ms: 15000,
        output_dir: dir.path().join("output").display().to_string(),
        snapshot_dir: Some(dir.path().join("baselines").display().to_string()),
        snapshot_path_template: Some("{snapshotDir}/{browserName}/{arg}{ext}".into()),
        ..Default::default()
    })
    .list_progress(false)
}
fn flatten(steps: &[StepInfo]) -> Vec<&StepInfo> {
    steps
        .iter()
        .flat_map(|step| std::iter::once(step).chain(flatten(&step.steps)))
        .collect()
}
fn attachment<'a>(attachments: &'a [Attachment], name: &str) -> &'a Attachment {
    attachments
        .iter()
        .find(|attachment| attachment.name == name)
        .unwrap()
}
fn image(path: impl AsRef<Path>) -> image::RgbaImage {
    image::load_from_memory(&std::fs::read(path).unwrap())
        .unwrap()
        .to_rgba8()
}
#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<(u32, String)>>>);
impl Reporter for Events {
    fn on_attachment(&self, attempt: &AttemptInfo, attachment: &Attachment) {
        self.0
            .lock()
            .unwrap()
            .push((attempt.retry, attachment.name.clone()));
    }
}

#[tokio::test]
async fn retry_copies_are_immutable_and_bundle_links_survive_source_removal_and_relocation() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let events = Events::default();
        let report = runner(&dir)
            .retries(1)
            .custom_reporter(events.clone())
            .run(
                &browser,
                vec![test_with_context("snapshot <retry>&", |ctx| async move {
                    ctx.page
                        .set_viewport(Viewport {
                            width: 80,
                            height: 60,
                        })
                        .await?;
                    ctx.page
                        .set_content(
                            "<style>html,body{height:100%;margin:0;background:red}</style>",
                        )
                        .await?;
                    if ctx.info.retry == 0 {
                        ctx.page.expect().screenshot("<card>&").await?;
                        ctx.page
                            .evaluate_value("document.body.style.background='blue';true")
                            .await?;
                        ctx.page
                            .expect()
                            .timeout(Duration::from_millis(450))
                            .screenshot("<card>&")
                            .await?;
                    } else {
                        ctx.page
                            .evaluate_value("document.body.style.background='lime';true")
                            .await?;
                        ctx.page
                            .expect()
                            .screenshot_with(
                                "<card>&",
                                &SnapshotOptions {
                                    update: Some(SnapshotUpdate::All),
                                    ..Default::default()
                                },
                            )
                            .await?;
                        let baseline = ctx
                            .info
                            .snapshot_path("<card>&", SnapshotKind::Screenshot)?;
                        assert_eq!(image(baseline).get_pixel(5, 5).0, [0, 255, 0, 255]);
                    }
                    Ok(())
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        let result = &report.results[0];
        assert!(result.flaky);
        assert_eq!(result.attempt_results.len(), 2);
        let failed = &result.attempt_results[0];
        assert_eq!(failed.status, AttemptStatus::Failed);
        assert_eq!(failed.attachments.len(), 3);
        assert!(result.attempt_results[1].attachments.is_empty());
        assert_eq!(
            image(&attachment(&failed.attachments, "<card>&-expected").path)
                .get_pixel(5, 5)
                .0,
            [255, 0, 0, 255]
        );
        assert_eq!(
            image(&attachment(&failed.attachments, "<card>&-actual").path)
                .get_pixel(5, 5)
                .0,
            [0, 0, 255, 255]
        );
        assert_eq!(
            image(&attachment(&failed.attachments, "<card>&-diff").path)
                .get_pixel(5, 5)
                .0,
            [255, 0, 0, 255]
        );
        let step = flatten(&failed.steps)
            .into_iter()
            .find(|step| {
                step.category == StepCategory::Assertion && step.status == StepStatus::Failed
            })
            .unwrap();
        assert_eq!(step.attachments.len(), 3);
        for item in &step.attachments {
            assert!(failed
                .attachments
                .iter()
                .any(|attachment| attachment.path == item.path));
            assert_eq!(item.content_type, "image/png");
        }
        {
            let emitted = events.0.lock().unwrap();
            assert_eq!(emitted.len(), 3);
            assert!(emitted.iter().all(|event| event.0 == 0));
        }
        assert!(report.to_html().contains("&lt;card&gt;&amp;-diff"));
        let export = tempfile::tempdir().unwrap();
        let bundle = report.write_bundle(export.path().join("bundle")).unwrap();
        let traces = result
            .attempt_results
            .iter()
            .map(|attempt| attempt.trace.as_deref().unwrap())
            .collect::<Vec<_>>();
        assert_ne!(traces[0], traces[1]);
        assert!(traces.iter().all(|path| Path::new(path).is_file()));
        assert_eq!(result.trace.as_deref(), Some(traces[1]));
        assert_eq!(bundle.artifact_count, 5); // Three image copies and two attempt traces.
        let moved = export.path().join("moved");
        std::fs::rename(bundle.html.parent().unwrap(), &moved).unwrap();
        dir.close().unwrap();
        let portable: TestReport =
            serde_json::from_slice(&std::fs::read(moved.join("results.json")).unwrap()).unwrap();
        let failed = &portable.results[0].attempt_results[0];
        assert!(portable.results[0]
            .attempt_results
            .iter()
            .all(|attempt| moved.join(attempt.trace.as_ref().unwrap()).is_file()));
        for item in &failed.attachments {
            assert!(!Path::new(&item.path).is_absolute());
            assert!(moved.join(&item.path).is_file());
            assert_eq!(image(moved.join(&item.path)).dimensions(), (80, 60));
            assert!(std::fs::read_to_string(moved.join("report.html"))
                .unwrap()
                .contains(&item.path));
        }
        let step = flatten(&failed.steps)
            .into_iter()
            .find(|step| step.attachments.len() == 3)
            .unwrap();
        assert!(step
            .attachments
            .iter()
            .all(|item| moved.join(&item.path).is_file()));
        if let Some(preview) = std::env::var_os("FERRITE_E2E_REPORT_PREVIEW_DIR") {
            // Preview is covered by the relocated JSON/HTML above; retain that folder directly.
            let preview = Path::new(&preview).join(browser.kind().name());
            std::fs::create_dir_all(&preview).unwrap();
            for entry in std::fs::read_dir(&moved).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_file() {
                    std::fs::copy(entry.path(), preview.join(entry.file_name())).unwrap();
                }
            }
            std::fs::create_dir_all(preview.join("artifacts")).unwrap();
            for entry in std::fs::read_dir(moved.join("artifacts")).unwrap() {
                let entry = entry.unwrap();
                std::fs::copy(
                    entry.path(),
                    preview.join("artifacts").join(entry.file_name()),
                )
                .unwrap();
            }
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn locator_dimensions_and_negated_soft_failures_keep_the_owning_assertion_images() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir).run(&browser, vec![test_with_context("soft images", |ctx| async move {
            ctx.page.set_content("<style>body{margin:0}#patch{width:20px;height:10px;background:red}</style><div id=patch></div>").await?;
            let patch = ctx.page.locator("#patch");
            patch.expect().screenshot("dimension").await?;
            ctx.page.evaluate_value("document.getElementById('patch').style.width='30px';true").await?;
            let soft = ctx.info.soft_asserts();
            soft.run("dimension mismatch", patch.expect().timeout(Duration::from_millis(450)).screenshot("dimension")).await?;
            patch.expect().screenshot("negated").await?;
            soft.run("must differ", patch.expect().not().timeout(Duration::from_millis(450)).screenshot("negated")).await?;
            assert_eq!(ctx.info.attachments().len(), 6);
            Ok(())
        })]).await;
        let attempt = &report.results[0].attempt_results[0];
        assert_eq!(attempt.status, AttemptStatus::Failed);
        assert_eq!(attempt.soft_assertions.len(), 2);
        assert_eq!(attempt.attachments.len(), 6);
        let diff = image(&attachment(&attempt.attachments, "dimension-diff").path);
        assert_eq!(diff.dimensions(), (30, 10));
        assert_eq!(diff.get_pixel(25, 5).0, [255, 0, 255, 255]);
        assert_eq!(
            image(&attachment(&attempt.attachments, "dimension-expected").path).dimensions(),
            (20, 10)
        );
        assert_eq!(
            image(&attachment(&attempt.attachments, "dimension-actual").path).dimensions(),
            (30, 10)
        );
        let steps = flatten(&attempt.steps);
        assert_eq!(
            steps
                .iter()
                .filter(|step| step.attachments.len() == 3)
                .count(),
            2
        );
        assert!(steps
            .iter()
            .filter(|step| !step.attachments.is_empty())
            .all(|step| step.category == StepCategory::Assertion));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn never_stable_images_keep_last_pair_and_never_update_baselines() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let tests = [true, false].into_iter().map(|existing| test_with_context(if existing { "existing" } else { "missing" }, move |ctx| async move {
            ctx.page.set_content("<style>body{margin:0}#patch{width:20px;height:10px;background:red}</style><div id=patch></div>").await?;
            let patch = ctx.page.locator("#patch");
            if existing { patch.expect().screenshot("changing").await?; }
            ctx.page.evaluate_value("globalThis.paint=0;globalThis.draw=()=>{paint++;document.getElementById('patch').style.background=`rgb(${paint&255},${(paint>>8)&255},${(paint>>16)&255})`;requestAnimationFrame(draw)};draw();true").await?;
            patch.expect().timeout(Duration::from_millis(650)).screenshot_with("changing", &SnapshotOptions { update: Some(SnapshotUpdate::All), ..Default::default() }).await
        })).collect();
        // Isolate the names' baseline directories despite their shared argument.
        let report = runner(&dir)
            .snapshot_path_template("{snapshotDir}/{browserName}/{testName}/{arg}{ext}")
            .run(&browser, tests)
            .await;
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            assert_eq!(attempt.status, AttemptStatus::Failed);
            assert!(result
                .error
                .as_deref()
                .unwrap()
                .contains("consecutive stable"));
            assert_eq!(
                attempt.attachments.len(),
                if result.name == "existing" { 5 } else { 3 }
            );
            let actual = &attachment(&attempt.attachments, "changing-actual").path;
            let previous = &attachment(&attempt.attachments, "changing-previous").path;
            assert_ne!(
                std::fs::read(actual).unwrap(),
                std::fs::read(previous).unwrap()
            );
            assert_eq!(
                image(&attachment(&attempt.attachments, "changing-stability-diff").path)
                    .get_pixel(5, 5)
                    .0,
                [255, 0, 0, 255]
            );
            let baseline = dir
                .path()
                .join("baselines")
                .join(browser.kind().name())
                .join(&result.name)
                .join("changing.png");
            if result.name == "existing" {
                assert_eq!(image(baseline).get_pixel(5, 5).0, [255, 0, 0, 255]);
            } else {
                assert!(!baseline.exists());
            }
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn diagnostic_io_preserves_mismatches_and_control_errors_publish_no_failure_images() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir).snapshot_path_template("{snapshotDir}/{browserName}/{testName}/{arg}{ext}").run(&browser, vec![
            test_with_context("diagnostic io", |ctx| async move {
                ctx.page.set_viewport(Viewport { width: 40, height: 30 }).await?;
                ctx.page.set_content("<style>html,body{height:100%;margin:0;background:red}</style>").await?;
                ctx.page.expect().screenshot("card").await?;
                std::fs::create_dir_all(&ctx.info.output_dir)?;
                std::fs::write(Path::new(&ctx.info.output_dir).join("attachments"), b"cannot be a directory")?;
                ctx.page.evaluate_value("document.body.style.background='blue';true").await?;
                let error = ctx.page.expect().timeout(Duration::from_millis(450)).screenshot("card").await.unwrap_err();
                assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
                assert!(error.to_string().contains("stable capture did not satisfy"));
                assert!(error.to_string().contains("diagnostics also failed"));
                assert!(ctx.info.attachments().is_empty());
                let baseline = ctx.info.snapshot_path("card", SnapshotKind::Screenshot)?;
                assert_eq!(image(&baseline).get_pixel(5,5).0, [255,0,0,255]);
                assert_eq!(image(baseline.with_extension("actual.png")).get_pixel(5,5).0, [0,0,255,255]);
                Err(error)
            }),
            test_with_context("native timeout", |ctx| async move {
                ctx.page.set_content("<div id=patch>ready</div>").await?;
                let options = SnapshotOptions { capture: Some(ScreenshotOptions {
                    timeout: Some(Duration::from_millis(150)), style: Some("#patch{color:red!important}".into()), ..Default::default()
                }), ..Default::default() };
                let error = ctx.page.locator("#missing").expect().screenshot_with("card", &options).await.unwrap_err();
                assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT");
                assert!(ctx.info.attachments().is_empty());
                Err(error)
            }),
            test_with_context("caller cancellation", |ctx| async move {
                ctx.page.set_viewport(Viewport { width: 40, height: 30 }).await?;
                ctx.page.set_content("<style>html,body{height:100%;margin:0;background:red}</style>").await?;
                ctx.page.evaluate_value("globalThis.waited=false;Object.defineProperty(document.fonts,'ready',{configurable:true,get(){globalThis.waited=true;return new Promise(()=>{})}});true").await?;
                let token = CancellationToken::new();
                let scoped = ctx.page.with_cancellation(token.clone());
                let options = SnapshotOptions { capture: Some(ScreenshotOptions {
                    style: Some("html,body{background:green!important}".into()), ..Default::default()
                }), ..Default::default() };
                let expectation = scoped.expect().timeout(Duration::ZERO);
                let assertion = expectation.screenshot_with("card", &options);
                let cancel = async {
                    ctx.page.wait_for_function("globalThis.waited===true", Duration::from_secs(2)).await?;
                    token.cancel();
                    Ok::<_, E2eError>(())
                };
                let (assertion, cancelled) = tokio::join!(assertion, cancel);
                cancelled?;
                let error = assertion.unwrap_err();
                assert_eq!(error.code(), "FERRITE_E2E_CANCELLED");
                assert!(ctx.info.attachments().is_empty());
                ctx.page.evaluate_value("delete document.fonts.ready;true").await?;
                let restored = ctx.page.screenshot(ScreenshotOptions::default()).await?;
                assert_eq!(image::load_from_memory(&restored).unwrap().to_rgba8().get_pixel(5,5).0, [255,0,0,255]);
                Err(error)
            }),
            test_with_context("retry probe successful", |ctx| async move {
                ctx.page.set_viewport(Viewport { width: 40, height: 30 }).await?;
                ctx.page.set_content("<style>html,body{height:100%;margin:0;background:red}</style>").await?;
                ctx.page.expect().screenshot("card").await?;
                let mut calls = 0;
                expect_to_pass_with("eventual snapshot", &PollingOptions::default().timeout(Duration::from_secs(3)), || {
                    let call = calls;
                    calls += 1;
                    let page = ctx.page.clone();
                    async move {
                        page.evaluate_value(if call == 0 {"document.body.style.background='blue';true"} else {"document.body.style.background='red';true"}).await?;
                        page.expect().timeout(Duration::from_millis(450)).screenshot("card").await
                    }
                }).await?;
                assert_eq!(calls, 2);
                assert!(ctx.info.attachments().is_empty());
                Ok(())
            }),
        ]).await;
        assert_eq!(report.results.len(), 4);
        for result in &report.results {
            let attempt = &result.attempt_results[0];
            assert!(attempt.attachments.is_empty());
            assert!(flatten(&attempt.steps)
                .iter()
                .all(|step| step.attachments.is_empty()));
            if result.name == "retry probe successful" {
                assert_eq!(attempt.status, AttemptStatus::Passed);
                assert!(attempt.errors.is_empty());
                assert!(flatten(&attempt.steps)
                    .iter()
                    .all(|step| step.status == StepStatus::Passed));
                continue;
            }
            let expected_code = match result.name.as_str() {
                "diagnostic io" => "FERRITE_E2E_EXPECT",
                "native timeout" => "FERRITE_E2E_TIMEOUT",
                "caller cancellation" => "FERRITE_E2E_CANCELLED",
                other => panic!("unexpected test {other}"),
            };
            assert!(attempt
                .errors
                .iter()
                .any(|error| error.code == expected_code));
        }
        browser.close().await.unwrap();
    }
}
