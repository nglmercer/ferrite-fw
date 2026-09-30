//! Native paths share runner metadata, explicit assertion precedence and retry identity.
use ferrite_e2e::*;
use image::GenericImageView;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
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
                "snapshot paths {} {}",
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
        reporter: "json,junit,html".into(),
        timeout_ms: 10000,
        output_dir: dir.path().join("output").display().to_string(),
        snapshot_dir: Some(dir.path().join("baselines").display().to_string()),
        snapshot_path_template: Some(
            "{snapshotDir}/{browserName}{-projectName}/{platform}/{testFileBaseName}/{arg}{ext}"
                .into(),
        ),
        ..Default::default()
    })
    .list_progress(false)
}

#[cfg(unix)]
#[tokio::test]
async fn atomic_page_locator_updates_preserve_aliases_and_parallel_missing_baselines() {
    use std::{io::Read, os::unix::fs::symlink};
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let aliases = root.path().join("aliases");
        let target = root.path().join("targets/expected.png");
        std::fs::create_dir_all(&aliases).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 40,
            height: 30,
        })
        .await
        .unwrap();
        page.set_content("<style>html,body{height:100%;margin:0;background:red}</style>")
            .await
            .unwrap();
        page.expect()
            .screenshot_with(
                "Expected",
                &SnapshotOptions {
                    dir: Some(target.parent().unwrap().to_path_buf()),
                    update: Some(SnapshotUpdate::All),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let red = std::fs::read(&target).unwrap();
        let alias = aliases.join("alias.png");
        symlink("../targets/expected.png", &alias).unwrap();
        let options = SnapshotOptions {
            dir: Some(aliases.clone()),
            update: Some(SnapshotUpdate::Missing),
            ..Default::default()
        };
        page.expect()
            .screenshot_with("Alias", &options)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), red);
        let mut old_reader = std::fs::File::open(&target).unwrap();
        page.evaluate_value("document.body.style.background='lime';true")
            .await
            .unwrap();
        page.expect()
            .screenshot_with(
                "Alias",
                &SnapshotOptions {
                    update: Some(SnapshotUpdate::Changed),
                    ..options.clone()
                },
            )
            .await
            .unwrap();
        assert!(std::fs::symlink_metadata(&alias).unwrap().is_symlink());
        let color = |path: &std::path::Path| {
            image::load_from_memory(&std::fs::read(path).unwrap())
                .unwrap()
                .into_rgba8()
                .get_pixel(5, 5)
                .0
        };
        assert_eq!(color(&target), [0, 255, 0, 255]);
        let mut old = Vec::new();
        old_reader.read_to_end(&mut old).unwrap();
        assert_eq!(old, red);
        page.evaluate_value("document.body.style.background='blue';true")
            .await
            .unwrap();
        page.expect()
            .screenshot_with(
                "Alias",
                &SnapshotOptions {
                    update: Some(SnapshotUpdate::All),
                    ..options.clone()
                },
            )
            .await
            .unwrap();
        assert!(std::fs::symlink_metadata(&alias).unwrap().is_symlink());
        assert_eq!(color(&alias), [0, 0, 255, 255]);
        let dangling = aliases.join("dangling.png");
        symlink("../new/nested/expected.png", &dangling).unwrap();
        page.expect()
            .screenshot_with("Dangling", &options)
            .await
            .unwrap();
        assert!(std::fs::symlink_metadata(&dangling).unwrap().is_symlink());
        assert_eq!(color(&dangling), [0, 0, 255, 255]);
        page.set_content("<style>body{margin:0}#patch{width:20px;height:20px;background:red}</style><div id=patch></div>").await.unwrap();
        page.locator("#patch")
            .expect()
            .screenshot_with(
                "Alias",
                &SnapshotOptions {
                    update: Some(SnapshotUpdate::Changed),
                    ..options.clone()
                },
            )
            .await
            .unwrap();
        assert!(std::fs::symlink_metadata(&alias).unwrap().is_symlink());
        assert_eq!(color(&alias), [255, 0, 0, 255]);
        assert_eq!(
            image::load_from_memory(&std::fs::read(&target).unwrap())
                .unwrap()
                .dimensions(),
            (20, 20)
        );
        let peer = browser.new_page().await.unwrap();
        peer.set_viewport(Viewport {
            width: 40,
            height: 30,
        })
        .await
        .unwrap();
        let blue = "<style>html,body{height:100%;margin:0;background:blue}</style>";
        page.set_content(blue).await.unwrap();
        peer.set_content(blue).await.unwrap();
        let first = page.expect();
        let second = peer.expect();
        let (first, second) = tokio::join!(
            first.screenshot_with("Shared", &options),
            second.screenshot_with("Shared", &options)
        );
        first.unwrap();
        second.unwrap();
        assert_eq!(color(&aliases.join("shared.png")), [0, 0, 255, 255]);
        assert_eq!(std::fs::read_dir(&aliases).unwrap().count(), 3);
        assert_eq!(
            std::fs::read_dir(target.parent().unwrap()).unwrap().count(),
            1
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn project_page_locator_and_text_paths_follow_effective_and_explicit_settings() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report=runner(&dir)
            .project(Project::new("Desktop & tablet"))
            .project(Project::new("Shared").snapshot_path_template("{snapshotDir}/project-override/{arg}{ext}"))
            .run(&browser,vec![test_with_context("suite > probe",|ctx|async move {
                let settings=ctx.info.settings();
                assert_eq!(settings.snapshot_path_template,ctx.info.project_config().unwrap().snapshot_path_template);
                assert_eq!(settings.snapshot_root_dir,ctx.info.config().snapshot_root_dir);
                ctx.page.set_viewport(Viewport{width:100,height:80}).await?;
                ctx.page.set_content("<style>body{margin:0}h1{margin:0;width:70px;height:40px;background:red}</style><h1>Hi</h1>").await?;
                let path=ctx.info.snapshot_path("nested/Card.png",SnapshotKind::Screenshot)?;
                let base=std::path::Path::new(&settings.snapshot_dir);
                let expected=if ctx.info.project.as_deref()==Some("Shared") {
                    base.join("project-override/nested/card.png")
                }else{
                    base.join(format!("{}-desktop-tablet",settings.browser.name()))
                        .join(match std::env::consts::OS{"macos"=>"darwin","windows"=>"win32",other=>other})
                        .join("snapshot_paths/nested/card.png")
                };
                assert_eq!(path,expected);
                ctx.page.expect().screenshot("nested/Card.png").await?;
                assert_eq!(image::load_from_memory(&std::fs::read(&path)?).unwrap().dimensions(),(100,80));
                let element=ctx.info.snapshot_path("Element",SnapshotKind::Screenshot)?;
                ctx.page.locator("h1").expect().screenshot("Element").await?;
                assert_eq!(image::load_from_memory(&std::fs::read(element)?).unwrap().dimensions(),(70,40));
                let options=ctx.info.snapshot_options();
                assert_snapshot_text("nested/Body","first",&options)?;
                let text=ctx.info.snapshot_path("nested/Body",SnapshotKind::Text)?;
                assert_eq!(std::fs::read_to_string(&text)?,"first");
                assert_eq!(assert_snapshot_text("nested/Body","second",&options).unwrap_err().code(),"FERRITE_E2E_EXPECT");
                assert_eq!(std::fs::read_to_string(text.with_extension("actual.snap"))?,"second");
                let explicit=SnapshotOptions {
                    dir:Some(std::path::Path::new(&ctx.info.output_dir).join("explicit-baseline")),
                    path_template:Some("{snapshotDir}/override/{arg}{ext}".into()),
                    ..Default::default()
                };
                ctx.page.expect().screenshot_with("Explicit",&explicit).await?;
                assert!(std::path::Path::new(&ctx.info.output_dir).join("explicit-baseline/override/explicit.png").is_file());
                assert!(!base.join("explicit.png").exists());
                let invalid=SnapshotOptions{path_template:Some("{unknown}".into()),..Default::default()};
                let error=ctx.page.expect().screenshot_with("invalid",&invalid).await.unwrap_err();
                assert_eq!(error.code(),"FERRITE_E2E_CONFIG");
                assert_eq!(ctx.page.evaluate::<u32>("document.querySelectorAll('[data-ferrite-screenshot]').length").await?,0);
                Ok(())
            })]).await;
        assert!(report.ok(), "{}", report.to_list());
        assert_eq!(report.passed(), 2);
        let configuration = report.configuration.as_ref().unwrap();
        let json = serde_json::to_value(configuration).unwrap();
        assert_eq!(
            json["snapshot_path_template"],
            configuration.snapshot_path_template.as_deref().unwrap()
        );
        assert!(report.to_html().contains("suite &gt; probe"));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn retries_share_baselines_and_invalid_builder_templates_never_run_bodies() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let report = runner(&dir)
            .retries(1)
            .run(
                &browser,
                vec![test_with_context("retry path", |ctx| async move {
                    ctx.page
                        .set_viewport(Viewport {
                            width: 100,
                            height: 80,
                        })
                        .await?;
                    ctx.page
                        .set_content(
                            "<style>html,body{margin:0;background:red;height:100%}</style>",
                        )
                        .await?;
                    let path = ctx.info.snapshot_path("retry", SnapshotKind::Screenshot)?;
                    if ctx.info.retry == 0 {
                        ctx.page.expect().screenshot("retry").await?;
                        ctx.page
                            .evaluate_value("document.body.style.background='blue';true")
                            .await?;
                        ctx.page
                            .expect()
                            .timeout(Duration::from_millis(300))
                            .screenshot("retry")
                            .await?;
                    } else {
                        assert!(path.is_file());
                        assert_eq!(
                            image::load_from_memory(&std::fs::read(&path)?)
                                .unwrap()
                                .to_rgba8()
                                .get_pixel(10, 10)
                                .0,
                            [255, 0, 0, 255]
                        );
                        ctx.page.expect().screenshot("retry").await?;
                    }
                    Ok(())
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        assert!(report.results[0].flaky);
        assert_eq!(report.results[0].attempt_results.len(), 2);
        assert_eq!(
            report.results[0].attempt_results[0].status,
            AttemptStatus::Failed
        );
        assert_eq!(
            report.results[0].attempt_results[1].status,
            AttemptStatus::Passed
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let called = calls.clone();
        let invalid = runner(&dir)
            .snapshot_path_template("{unsupported}")
            .run(
                &browser,
                vec![test("never", move |_| {
                    let called = called.clone();
                    async move {
                        called.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }
                })],
            )
            .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(invalid.failed(), 1);
        assert_eq!(invalid.results[0].name, "<configuration>");
        assert!(invalid.results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("unknown snapshot path template token"));
        browser.close().await.unwrap();
    }
}
