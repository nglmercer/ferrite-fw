//! Successive native PNG assertions, update policies and shared assertion clocks.
use ferrite_e2e::*;
use image::GenericImageView;
use std::time::Duration;

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
                "snapshot stability {} {}",
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

async fn page(browser: &Browser) -> Page {
    let page = browser.new_page().await.unwrap();
    page.set_viewport(Viewport {
        width: 100,
        height: 80,
    })
    .await
    .unwrap();
    page.set_content("<style>html,body{margin:0}#patch{width:60px;height:40px;background:red}</style><div id=patch></div>").await.unwrap();
    page
}

#[tokio::test]
async fn stable_page_and_locator_updates_negation_and_typed_validation() {
    for browser in browsers().await {
        let page = page(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let options = SnapshotOptions {
            dir: Some(dir.path().into()),
            update: Some(SnapshotUpdate::Missing),
            ..Default::default()
        };
        page.expect()
            .screenshot_with("page", &options)
            .await
            .unwrap();
        page.locator("#patch")
            .expect()
            .screenshot_with("patch", &options)
            .await
            .unwrap();
        let original = std::fs::read(dir.path().join("patch.png")).unwrap();
        assert_eq!(
            image::load_from_memory(&original).unwrap().dimensions(),
            (60, 40)
        );
        page.evaluate_value("document.getElementById('patch').style.background='blue'; true")
            .await
            .unwrap();
        page.locator("#patch")
            .expect()
            .not()
            .screenshot_with("patch", &options)
            .await
            .unwrap();
        let error = page
            .locator("#patch")
            .expect()
            .not()
            .screenshot_with("missing-negated", &options)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
        assert!(!dir.path().join("missing-negated.png").exists());
        let changed = SnapshotOptions {
            update: Some(SnapshotUpdate::Changed),
            ..options.clone()
        };
        page.locator("#patch")
            .expect()
            .screenshot_with("patch", &changed)
            .await
            .unwrap();
        let bytes = std::fs::read(dir.path().join("patch.png")).unwrap();
        assert_ne!(bytes, original);
        assert_eq!(
            image::load_from_memory(&bytes)
                .unwrap()
                .to_rgba8()
                .get_pixel(10, 10)
                .0,
            [0, 0, 255, 255]
        );
        page.locator("#patch")
            .expect()
            .screenshot_with("patch", &options)
            .await
            .unwrap();
        page.evaluate_value("document.getElementById('patch').style.width='70px'; true")
            .await
            .unwrap();
        let error = page
            .locator("#patch")
            .expect()
            .timeout(Timeout::ms(500))
            .screenshot_with("patch", &options)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
        assert!(error.to_string().contains("size differs"), "{error}");
        assert_eq!(
            image::load_from_memory(&std::fs::read(dir.path().join("patch.actual.png")).unwrap())
                .unwrap()
                .dimensions(),
            (70, 40)
        );
        assert_eq!(std::fs::read(dir.path().join("patch.png")).unwrap(), bytes);
        for invalid in [
            SnapshotOptions {
                max_diff_ratio: f32::NAN,
                ..options.clone()
            },
            SnapshotOptions {
                capture: Some(ScreenshotOptions {
                    quality: Some(80),
                    ..Default::default()
                }),
                ..options.clone()
            },
        ] {
            let error = page
                .expect()
                .screenshot_with("invalid", &invalid)
                .await
                .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
            assert!(!dir.path().join("invalid.png").exists());
        }
        let limited = SnapshotOptions {
            capture: Some(ScreenshotOptions {
                style: Some("#patch {background:green!important}".into()),
                timeout: Some(Duration::from_millis(150)),
                ..Default::default()
            }),
            ..options.clone()
        };
        let error = page
            .locator("#missing-target")
            .expect()
            .screenshot_with("native-timeout", &limited)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT", "{error}");
        assert!(!dir.path().join("native-timeout.png").exists());
        assert!(!dir.path().join("native-timeout.actual.png").exists());
        assert_eq!(
            page.locator("#patch")
                .css_value("background-color")
                .await
                .unwrap(),
            Some("rgb(0, 0, 255)".to_string())
        );
        assert!(page.take_screenshot_cleanup_errors().is_empty());
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn continuously_changing_captures_do_not_write_unstable_baselines_and_can_recover() {
    for browser in browsers().await {
        let page = page(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let options = SnapshotOptions {
            dir: Some(dir.path().into()),
            update: Some(SnapshotUpdate::All),
            capture: Some(ScreenshotOptions::default()),
            ..Default::default()
        };
        page.locator("#patch")
            .expect()
            .screenshot_with("existing", &options)
            .await
            .unwrap();
        let original = std::fs::read(dir.path().join("existing.png")).unwrap();
        page.evaluate_value("globalThis.paint=0;globalThis.animating=true;globalThis.draw=()=>{if(!animating)return;paint++;document.getElementById('patch').style.background=`rgb(${paint&255},${(paint>>8)&255},${(paint>>16)&255})`;requestAnimationFrame(draw)};draw();true").await.unwrap();
        for (name, mode) in [
            ("changing", SnapshotUpdate::All),
            ("existing", SnapshotUpdate::All),
            ("existing", SnapshotUpdate::Changed),
        ] {
            let options = SnapshotOptions {
                update: Some(mode),
                ..options.clone()
            };
            let start = tokio::time::Instant::now();
            let error = page
                .locator("#patch")
                .expect()
                .timeout(Timeout::ms(650))
                .screenshot_with(name, &options)
                .await
                .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
            assert!(error.to_string().contains("consecutive stable"), "{error}");
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "assertion must not open a final capture window"
            );
            assert!(!dir.path().join("changing.png").exists());
            assert!(dir.path().join(format!("{name}.actual.png")).is_file());
            assert_eq!(
                std::fs::read(dir.path().join("existing.png")).unwrap(),
                original
            );
        }
        page.evaluate_value(
            "animating=false;document.getElementById('patch').style.background='red';true",
        )
        .await
        .unwrap();
        page.locator("#patch")
            .expect()
            .screenshot_with("changing", &options)
            .await
            .unwrap();
        assert_eq!(
            image::load_from_memory(&std::fs::read(dir.path().join("changing.png")).unwrap())
                .unwrap()
                .to_rgba8()
                .get_pixel(10, 10)
                .0,
            [255, 0, 0, 255]
        );
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn font_wait_is_inside_local_window_and_caller_and_owner_cancellation() {
    for browser in browsers().await {
        let page = page(&browser).await;
        let dir = tempfile::tempdir().unwrap();
        let options = SnapshotOptions {
            dir: Some(dir.path().into()),
            capture: Some(ScreenshotOptions {
                style: Some("#patch {background:green!important}".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        // An explicitly held font readiness promise isolates native evaluation
        // waiting from raster timing. Actual downloadable-font coverage follows.
        page.evaluate_value("globalThis.fontWaits=0;globalThis.fontPrepared=false;globalThis.fontReady=new Promise(resolve=>globalThis.releaseFonts=resolve);Object.defineProperty(document.fonts,'ready',{configurable:true,get(){fontWaits++;fontPrepared=getComputedStyle(document.getElementById('patch')).backgroundColor==='rgb(0, 128, 0)';return fontReady}});true").await.unwrap();
        let error = page
            .expect()
            .timeout(Timeout::ms(500))
            .screenshot_with("hung-font", &options)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
        assert!(
            error.to_string().contains("no completed capture"),
            "{error}"
        );
        assert!(!dir.path().join("hung-font.png").exists());
        assert!(
            page.evaluate::<bool>("fontPrepared").await.unwrap(),
            "fonts must wait after owned style preparation"
        );
        for dispose in [false, true] {
            page.evaluate_value("fontWaits=0;fontPrepared=false;true")
                .await
                .unwrap();
            let token = CancellationToken::new();
            let scoped = page.with_cancellation(token.clone());
            let assertion = scoped.expect().timeout(Duration::ZERO);
            let capture = assertion.screenshot_with("cancelled-font", &options);
            tokio::pin!(capture);
            tokio::select! {
                error=&mut capture=>panic!("font wait finished early: {error:?}"),
                ready=page.wait_for_function("fontWaits>0",Duration::from_secs(2))=>{ready.unwrap();},
            }
            assert!(page.evaluate::<u32>("fontWaits").await.unwrap() > 0);
            assert!(page.evaluate::<bool>("fontPrepared").await.unwrap());
            if dispose {
                page.close().await.unwrap();
            } else {
                token.cancel_with_reason("cancelled font assertion");
            }
            let result = tokio::time::timeout(Duration::from_secs(2), &mut capture)
                .await
                .unwrap();
            assert!(
                matches!(
                    result,
                    Err(E2eError::Cancelled(_)) | Err(E2eError::Disconnected(_))
                ),
                "{result:?}"
            );
            assert!(!dir.path().join("cancelled-font.png").exists());
            if !dispose {
                let restored = page.screenshot(ScreenshotOptions::default()).await.unwrap();
                assert_eq!(
                    image::load_from_memory(&restored)
                        .unwrap()
                        .to_rgba8()
                        .get_pixel(10, 10)
                        .0,
                    [255, 0, 0, 255]
                );
                assert_eq!(
                    page.evaluate::<u32>(
                        "document.querySelectorAll('[data-ferrite-screenshot]').length"
                    )
                    .await
                    .unwrap(),
                    0
                );
                assert!(page.take_screenshot_cleanup_errors().is_empty());
            }
        }
        assert!(page.is_closed());
        browser.close().await.unwrap();
    }
}
