use ferrite_e2e::*;
use std::{path::Path, time::Duration};
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

async fn animate_video(page: &Page) -> E2eResult<()> {
    page.evaluate_value("new Promise(resolve => { const end = performance.now() + 800; function paint(now) { document.body.style.background = `hsl(${now % 360},80%,60%)`; if (now < end) requestAnimationFrame(paint); else resolve(true); } requestAnimationFrame(paint); })").await?;
    Ok(())
}
fn case(name: &'static str) -> Test {
    test_with_context(name, move |ctx| async move {
        ctx.page.set_content("<h1>retention</h1>").await?;
        if ctx.info.config().video != VideoMode::Off {
            animate_video(&ctx.page).await?;
        }
        ctx.info.attach("marker", b"marker", "text/plain")?;
        std::fs::write(ctx.info.output_path("marker.txt")?, "marker")?;
        // Even a caller-chosen baseline inside owned output is protected.
        let mut options = ctx.info.snapshot_options();
        options.dir = Some(ctx.info.output_dir.clone().into());
        tokio::task::spawn_blocking(move || {
            match_text_snapshot_with("protected", "baseline", &options)
        })
        .await
        .unwrap()?;
        match name {
            "retry" if ctx.info.retry == 0 => Err(E2eError::Expect("first failure".into())),
            "expected" => {
                ctx.info.fail("known");
                Err(E2eError::Expect("known".into()))
            }
            "unexpected pass" => {
                ctx.info.fail("known");
                Ok(())
            }
            "skip" => ctx.info.skip("runtime"),
            "timeout" => {
                ctx.info.set_timeout(Duration::from_millis(100));
                std::future::pending::<()>().await;
                Ok(())
            }
            _ => Ok(()),
        }
    })
}
#[derive(Clone)]
struct ReadAtEnd(std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Reporter for ReadAtEnd {
    fn on_end(&self, report: &TestReport) {
        for result in &report.results {
            for attempt in &result.attempt_results {
                for attachment in &attempt.attachments {
                    assert!(Path::new(&attachment.path).is_file());
                }
            }
        }
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
#[tokio::test]
async fn retention_policies_classify_attempts_protect_baselines_and_prune_links() {
    for browser in browsers().await {
        for policy in [
            OutputRetention::Always,
            OutputRetention::Never,
            OutputRetention::FailuresOnly,
        ] {
            let root = tempfile::tempdir().unwrap();
            let observer = ReadAtEnd(Default::default());
            let runner = Runner::from_config(&E2eConfig {
                screenshot: "on".into(),
                reporter: "json".into(),
                ..Default::default()
            })
            .workers(1)
            .retries(1)
            .test_timeout(Duration::from_secs(10))
            .list_progress(false)
            .output_dir(root.path().display().to_string())
            .output_retention(policy)
            .custom_reporter(observer.clone());
            let report = runner
                .run(
                    &browser,
                    vec![
                        case("pass"),
                        case("retry"),
                        case("expected"),
                        case("unexpected pass"),
                        case("skip"),
                        case("timeout"),
                    ],
                )
                .await;
            assert!(observer.0.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(
                report.configuration.as_ref().unwrap().output_retention,
                policy
            );
            assert_eq!(report.results.len(), 6, "{}", report.to_json());
            let persisted: TestReport =
                serde_json::from_slice(&std::fs::read(root.path().join("results.json")).unwrap())
                    .unwrap();
            assert_eq!(persisted.to_json(), report.to_json());
            for result in &report.results {
                for attempt in &result.attempt_results {
                    let dir = Path::new(&attempt.settings.as_ref().unwrap().output_dir);
                    let keep = policy.retains_attempt(attempt);
                    assert_eq!(
                        dir.join("marker.txt").is_file(),
                        keep,
                        "{} retry {} policy {policy:?}",
                        result.name,
                        attempt.info.retry
                    );
                    assert!(dir.join("protected.snap").is_file());
                    assert_eq!(!attempt.attachments.is_empty(), keep);
                    for path in attempt
                        .screenshots
                        .iter()
                        .chain(attempt.trace.iter())
                        .chain(attempt.attachments.iter().map(|a| &a.path))
                    {
                        assert!(Path::new(path).is_file(), "{path}");
                    }
                    if !keep {
                        assert!(attempt
                            .annotations
                            .iter()
                            .any(|(kind, _)| kind == "output-retention"));
                    }
                }
            }
            let old_dirs: Vec<_> = report
                .results
                .iter()
                .flat_map(|r| &r.attempt_results)
                .map(|a| a.settings.as_ref().unwrap().output_dir.clone())
                .collect();
            runner.run(&browser, vec![case("pass")]).await;
            for dir in old_dirs {
                assert!(Path::new(&dir).join("protected.snap").is_file());
            }
        }
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn never_keeps_portable_bundle_links_and_caller_sources() {
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let caller = tempfile::tempdir().unwrap();
        let source_path = caller.path().join("source.txt");
        std::fs::write(&source_path, "caller source").unwrap();
        let input = source_path.clone();
        let report = Runner::from_config(&E2eConfig {
            reporter: "html".into(),
            screenshot: "on".into(),
            video: "on".into(),
            video_fps: 5,
            ..Default::default()
        })
        .output_dir(root.path().display().to_string())
        .output_retention(OutputRetention::Never)
        .workers(1)
        .list_progress(false)
        .run(
            &browser,
            vec![
                case("pass"),
                test_with_context("z caller source", move |ctx| {
                    let input = input.clone();
                    async move {
                        ctx.page
                            .set_content("<h1>caller attachment video</h1>")
                            .await?;
                        animate_video(&ctx.page).await?;
                        ctx.info
                            .attach("caller", &std::fs::read(input)?, "text/plain")?;
                        Ok(())
                    }
                }),
            ],
        )
        .await;
        assert!(report.ok(), "{}", report.to_json());
        assert_eq!(
            std::fs::read_to_string(source_path).unwrap(),
            "caller source"
        );
        for attempt in &report.results[0].attempt_results {
            let source = Path::new(&attempt.settings.as_ref().unwrap().output_dir);
            assert!(!source.join("marker.txt").exists());
            assert!(attempt.video.is_some());
            for path in attempt
                .screenshots
                .iter()
                .chain(attempt.trace.iter())
                .chain(attempt.attachments.iter().map(|a| &a.path))
            {
                assert!(Path::new(path).is_file(), "{path}");
                assert!(Path::new(path).starts_with(root.path().join("artifacts")));
            }
        }
        let persisted: TestReport =
            serde_json::from_slice(&std::fs::read(root.path().join("results.json")).unwrap())
                .unwrap();
        for attempt in &persisted.results[0].attempt_results {
            for path in attempt
                .screenshots
                .iter()
                .chain(attempt.trace.iter())
                .chain(attempt.attachments.iter().map(|a| &a.path))
            {
                assert!(root.path().join(path).is_file());
            }
            assert!(attempt
                .annotations
                .iter()
                .any(|(kind, _)| kind == "output-retention"));
        }
        let exported = tempfile::tempdir().unwrap();
        report
            .write_bundle(exported.path().join("original"))
            .unwrap();
        std::fs::rename(
            exported.path().join("original"),
            exported.path().join("moved"),
        )
        .unwrap();
        let moved = exported.path().join("moved");
        let html = std::fs::read_to_string(moved.join("report.html")).unwrap();
        let artifacts = moved.join("artifacts");
        let app = axum::Router::new()
            .route(
                "/report",
                axum::routing::get(move || {
                    let html = html.clone();
                    async move { axum::response::Html(html) }
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
        // Even the original portable artifact folder can disappear after export.
        std::fs::remove_dir_all(root.path()).unwrap();
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{url}/report")).await.unwrap();
        let downloads = page.evaluate_value("(async () => await Promise.all([...document.querySelectorAll('a[href]')].map(async link => { const response = await fetch(link.href); return {ok: response.ok, size: (await response.arrayBuffer()).byteLength}; })))()").await.unwrap();
        let downloads = downloads.as_array().unwrap();
        assert!(downloads.len() >= 4);
        assert!(downloads
            .iter()
            .all(|value| value["ok"] == true && value["size"].as_u64().unwrap() > 0));
        assert!(page
            .evaluate_value("document.body.textContent.includes('owned outputs removed')")
            .await
            .unwrap()
            .as_bool()
            .unwrap());
        if let Some(previews) = std::env::var_os("FERRITE_REPORT_PREVIEW_DIR") {
            std::fs::create_dir_all(&previews).unwrap();
            std::fs::write(
                Path::new(&previews).join(format!("retention-{}.png", browser.kind().name())),
                page.screenshot(Default::default()).await.unwrap(),
            )
            .unwrap();
        }
        page.close().await.unwrap();
        server.abort();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn failure_retention_includes_cancellation_and_attempt_cleanup_errors() {
    for browser in browsers().await {
        for policy in [OutputRetention::Never, OutputRetention::FailuresOnly] {
            let root = tempfile::tempdir().unwrap();
            let runner = Runner::from_config(&E2eConfig::default())
                .list_progress(false)
                .output_dir(root.path().display().to_string())
                .output_retention(policy);
            let cleanup = runner
                .clone()
                .after_each(|_| async { Err(E2eError::Expect("cleanup failure".into())) })
                .run(&browser, vec![case("pass")])
                .await;
            assert!(!cleanup.ok());
            let attempt = &cleanup.results[0].attempt_results[0];
            assert!(!attempt.is_expected);
            assert_eq!(
                Path::new(&attempt.settings.as_ref().unwrap().output_dir)
                    .join("marker.txt")
                    .is_file(),
                policy == OutputRetention::FailuresOnly
            );
            let cancellation = CancellationToken::new();
            let abort = cancellation.clone();
            let report = runner
                .with_cancellation(cancellation)
                .run(
                    &browser,
                    vec![test_with_context("interrupted", move |ctx| {
                        let abort = abort.clone();
                        async move {
                            ctx.info.attach("marker", b"marker", "text/plain")?;
                            std::fs::write(ctx.info.output_path("marker.txt")?, "marker")?;
                            abort.cancel();
                            std::future::pending::<()>().await;
                            Ok(())
                        }
                    })],
                )
                .await;
            assert!(!report.ok());
            let result = report
                .results
                .iter()
                .find(|result| result.name == "interrupted")
                .unwrap();
            let attempt = &result.attempt_results[0];
            assert!(!attempt.is_expected);
            assert_eq!(
                Path::new(&attempt.settings.as_ref().unwrap().output_dir)
                    .join("marker.txt")
                    .is_file(),
                policy == OutputRetention::FailuresOnly
            );
        }
        browser.close().await.unwrap();
    }
}
#[cfg(unix)]
#[tokio::test]
async fn changed_owned_directory_is_reported_without_deleting_caller_data() {
    struct Replace(std::path::PathBuf);
    impl Reporter for Replace {
        fn on_end(&self, report: &TestReport) {
            let dir = Path::new(
                &report.results[0].attempt_results[0]
                    .settings
                    .as_ref()
                    .unwrap()
                    .output_dir,
            );
            std::fs::rename(dir, dir.with_file_name("preserved-original")).unwrap();
            std::os::unix::fs::symlink(&self.0, dir).unwrap();
        }
    }
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let caller = tempfile::tempdir().unwrap();
        std::fs::write(caller.path().join("caller.txt"), "untouched").unwrap();
        let report = Runner::from_config(&E2eConfig {
            reporter: "json".into(),
            ..Default::default()
        })
        .list_progress(false)
        .output_dir(root.path().display().to_string())
        .output_retention(OutputRetention::Never)
        .custom_reporter(Replace(caller.path().to_path_buf()))
        .run(&browser, vec![case("pass")])
        .await;
        assert!(!report.ok());
        assert!(report
            .results
            .iter()
            .any(|r| r.name == "<output retention>"));
        assert_eq!(
            std::fs::read_to_string(caller.path().join("caller.txt")).unwrap(),
            "untouched"
        );
        let dir = Path::new(
            &report.results[0].attempt_results[0]
                .settings
                .as_ref()
                .unwrap()
                .output_dir,
        );
        assert!(dir
            .with_file_name("preserved-original")
            .join("marker.txt")
            .is_file());
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn failed_pre_cleanup_publication_preserves_owned_outputs() {
    struct RefusePublication(std::path::PathBuf);
    impl Reporter for RefusePublication {
        fn on_end(&self, _: &TestReport) {
            let report = self.0.join("results.json");
            std::fs::rename(&report, self.0.join("initial-results.json")).unwrap();
            std::fs::create_dir(&report).unwrap();
        }
    }
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let report = Runner::from_config(&E2eConfig {
            reporter: "json".into(),
            ..Default::default()
        })
        .list_progress(false)
        .output_dir(root.path().display().to_string())
        .output_retention(OutputRetention::Never)
        .custom_reporter(RefusePublication(root.path().to_path_buf()))
        .run(&browser, vec![case("pass")])
        .await;
        assert!(!report.ok());
        assert!(report
            .results
            .iter()
            .any(|r| r.name == "<output retention>"));
        let attempt = &report.results[0].attempt_results[0];
        assert!(Path::new(&attempt.settings.as_ref().unwrap().output_dir)
            .join("marker.txt")
            .is_file());
        assert!(Path::new(&attempt.attachments[0].path).is_file());
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn overlapping_project_roots_do_not_prune_historical_runs() {
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let config = E2eConfig {
            output_dir: root.path().display().to_string(),
            projects: vec![
                E2eProjectConfig {
                    name: "outer".into(),
                    output_dir: Some(root.path().display().to_string()),
                    ..Default::default()
                },
                E2eProjectConfig {
                    name: "nested".into(),
                    output_dir: Some(root.path().join("nested").display().to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let runner = Runner::from_config(&config).workers(1).list_progress(false);
        let first = runner.clone().run(&browser, vec![case("pass")]).await;
        assert!(first.ok(), "{}", first.to_json());
        assert_eq!(first.results.len(), 2);
        let second = runner
            .output_retention(OutputRetention::Never)
            .run(&browser, vec![case("pass")])
            .await;
        assert!(second.ok(), "{}", second.to_json());
        assert_eq!(second.results.len(), 2);
        for result in &first.results {
            let dir = Path::new(
                &result.attempt_results[0]
                    .settings
                    .as_ref()
                    .unwrap()
                    .output_dir,
            );
            assert!(dir.join("marker.txt").is_file());
        }
        for result in &second.results {
            let dir = Path::new(
                &result.attempt_results[0]
                    .settings
                    .as_ref()
                    .unwrap()
                    .output_dir,
            );
            assert!(!dir.join("marker.txt").exists());
            assert!(dir.join("protected.snap").is_file());
        }
        browser.close().await.unwrap();
    }
}
