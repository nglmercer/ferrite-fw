//! Pixel-level capture options and reversible temporary state on both engines.
use ferrite_e2e::*;
use image::GenericImageView;
use serde_json::json;
use std::time::Duration;

async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
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
                "screenshot options {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
const HTML: &str="<style>html,body{margin:0;background:transparent}body{height:400px}#patch{position:absolute;left:40px;top:220px;width:60px;height:40px;background:rgb(255,0,0)}</style><div id=patch></div><style id=ferrite-shot-style>/* application style */</style><span class=ferrite-shot-mask>application node</span>";
fn image(bytes: &[u8]) -> image::DynamicImage {
    image::load_from_memory(bytes).unwrap()
}
fn pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
    image(bytes).to_rgba8().get_pixel(x, y).0
}
fn box_at(x: f64, y: f64, w: f64, h: f64) -> ElementRect {
    ElementRect {
        x,
        y,
        width: w,
        height: h,
    }
}
async fn clean(page: &Page) {
    assert_eq!(
        page.evaluate::<u32>("document.querySelectorAll('[data-ferrite-screenshot]').length")
            .await
            .unwrap(),
        0
    );
    assert_eq!(page.evaluate::<u32>("Object.getOwnPropertyNames(globalThis).filter(s=>s.startsWith('ferrite.screenshot.')).length").await.unwrap(),0);
    assert!(page.evaluate::<bool>("!!document.getElementById('ferrite-shot-style') && document.querySelector('.ferrite-shot-mask').textContent === 'application node'").await.unwrap());
    assert!(page.take_screenshot_cleanup_errors().is_empty());
}

#[tokio::test]
async fn page_locator_scrolled_full_document_clips_masks_and_temporary_styles_restore() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 200,
            height: 120,
        })
        .await
        .unwrap();
        page.set_content(HTML).await.unwrap();
        page.evaluate_value("scrollTo(0,180);true").await.unwrap();
        let before = page
            .evaluate_value("[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]")
            .await
            .unwrap();
        let clip = box_at(40.0, 40.0, 60.0, 40.0);
        let shot = page
            .screenshot(ScreenshotOptions {
                clip: Some(clip.clone()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(image(&shot).dimensions(), (60, 40));
        assert_eq!(pixel(&shot, 20, 20), [255, 0, 0, 255]);
        let mask = page.locator("#patch");
        let shot = page
            .screenshot(ScreenshotOptions {
                clip: Some(clip),
                mask: vec![mask.clone()],
                mask_color: Some("rgb(0,255,0)".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(pixel(&shot, 20, 20), [0, 255, 0, 255]);
        clean(&page).await;
        let shot = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                mask: vec![mask.clone()],
                mask_color: Some("#00ff00".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        let width: u32 = page
            .evaluate("document.documentElement.scrollWidth")
            .await
            .unwrap();
        assert_eq!(image(&shot).dimensions(), (width, 400));
        assert_eq!(pixel(&shot, 60, 240), [0, 255, 0, 255]);
        assert_eq!(
            page.evaluate_value("[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]")
                .await
                .unwrap(),
            before
        );
        clean(&page).await;
        let shot = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                clip: Some(box_at(40.0, 220.0, 60.0, 40.0)),
                style: Some("#patch{background:rgb(0,0,255)!important}".into()),
                hide_caret: true,
                disable_animations: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(pixel(&shot, 20, 20), [0, 0, 255, 255]);
        assert_eq!(
            page.evaluate::<String>(
                "getComputedStyle(document.querySelector('#patch')).backgroundColor"
            )
            .await
            .unwrap(),
            "rgb(255, 0, 0)"
        );
        clean(&page).await;
        let shot = mask
            .screenshot_with(ScreenshotOptions {
                mask: vec![mask.clone()],
                mask_color: Some("#00ff00".into()),
                style: Some("#patch{border:0!important}".into()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(image(&shot).dimensions(), (60, 40));
        assert_eq!(pixel(&shot, 20, 20), [0, 255, 0, 255]);
        let shot = mask.screenshot().await.unwrap();
        assert_eq!(pixel(&shot, 20, 20), [255, 0, 0, 255]);
        clean(&page).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_device_css_scales_jpeg_and_background_overrides_preserve_state() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 200,
            height: 120,
        })
        .await
        .unwrap();
        page.set_content(HTML).await.unwrap();
        if browser.kind() == BrowserKind::Chromium {
            page.call(
                "Emulation.setDeviceMetricsOverride",
                json!({"width":200,"height":120,"deviceScaleFactor":2,"mobile":false}),
            )
            .await
            .unwrap();
            page.call(
                "Emulation.setDefaultBackgroundColorOverride",
                json!({"color":{"r":10,"g":20,"b":30,"a":1}}),
            )
            .await
            .unwrap();
        } else {
            page.call(
                "browsingContext.setViewport",
                json!({"viewport":{"width":200,"height":120},"devicePixelRatio":2}),
            )
            .await
            .unwrap();
        }
        let before=page.evaluate_value("[innerWidth,innerHeight,devicePixelRatio,matchMedia('(resolution: 2dppx)').matches]").await.unwrap();
        for (scale, factor) in [(ScreenshotScale::Device, 2), (ScreenshotScale::Css, 1)] {
            let viewport = page
                .screenshot(ScreenshotOptions {
                    scale,
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(image(&viewport).dimensions(), (200 * factor, 120 * factor));
            let shot = page
                .screenshot(ScreenshotOptions {
                    full_page: true,
                    clip: Some(box_at(40.0, 220.0, 60.0, 40.0)),
                    scale,
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(image(&shot).dimensions(), (60 * factor, 40 * factor));
            assert_eq!(pixel(&shot, 20, 20), [255, 0, 0, 255]);
            let shot = page
                .locator("#patch")
                .screenshot_with(ScreenshotOptions {
                    scale,
                    quality: Some(90),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(&shot[..2], &[0xff, 0xd8]);
            assert_eq!(image(&shot).dimensions(), (60 * factor, 40 * factor));
            assert_eq!(page.evaluate_value("[innerWidth,innerHeight,devicePixelRatio,matchMedia('(resolution: 2dppx)').matches]").await.unwrap(),before);
            clean(&page).await;
        }
        if browser.kind() == BrowserKind::Chromium {
            let shot = page
                .screenshot(ScreenshotOptions {
                    omit_background: true,
                    clip: Some(box_at(0.0, 0.0, 20.0, 20.0)),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(pixel(&shot, 5, 5)[3], 0);
            let shot = page
                .screenshot(ScreenshotOptions {
                    clip: Some(box_at(0.0, 0.0, 20.0, 20.0)),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(pixel(&shot, 5, 5), [10, 20, 30, 255]);
        } else {
            let error = page
                .screenshot(ScreenshotOptions {
                    omit_background: true,
                    style: Some("body{background:red}".into()),
                    ..Default::default()
                })
                .await
                .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
            assert!(error.to_string().contains("Firefox"));
        }
        clean(&page).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn validation_failure_precancellation_zero_timeout_and_concurrent_styles_are_isolated() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 200,
            height: 120,
        })
        .await
        .unwrap();
        page.set_content(HTML).await.unwrap();
        for options in [
            ScreenshotOptions {
                quality: Some(0),
                ..Default::default()
            },
            ScreenshotOptions {
                quality: Some(101),
                ..Default::default()
            },
            ScreenshotOptions {
                clip: Some(box_at(f64::NAN, 0.0, 20.0, 20.0)),
                ..Default::default()
            },
            ScreenshotOptions {
                clip: Some(box_at(0.0, 0.0, 0.0, 20.0)),
                ..Default::default()
            },
            ScreenshotOptions {
                mask_color: Some("red;display:none".into()),
                ..Default::default()
            },
            ScreenshotOptions {
                omit_background: true,
                quality: Some(80),
                ..Default::default()
            },
        ] {
            assert_eq!(
                page.screenshot(options).await.unwrap_err().code(),
                "FERRITE_E2E_CONFIG"
            );
            clean(&page).await;
        }
        let other = browser.new_page().await.unwrap();
        assert_eq!(
            page.screenshot(ScreenshotOptions {
                mask: vec![other.locator("body")],
                ..Default::default()
            })
            .await
            .unwrap_err()
            .code(),
            "FERRITE_E2E_CONFIG"
        );
        assert_eq!(
            page.locator("#patch")
                .screenshot_with(ScreenshotOptions {
                    full_page: true,
                    ..Default::default()
                })
                .await
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CONFIG"
        );
        let error = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                clip: Some(box_at(9999.0, 9999.0, 20.0, 20.0)),
                style: Some("#patch{background:blue!important}".into()),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
        clean(&page).await;
        assert_eq!(
            page.evaluate::<String>(
                "getComputedStyle(document.querySelector('#patch')).backgroundColor"
            )
            .await
            .unwrap(),
            "rgb(255, 0, 0)"
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            page.with_cancellation(cancel)
                .screenshot(Default::default())
                .await
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CANCELLED"
        );
        clean(&page).await;
        let options = |color: &str| ScreenshotOptions {
            full_page: true,
            clip: Some(box_at(40.0, 220.0, 60.0, 40.0)),
            style: Some(format!("#patch{{background:{color}!important}}")),
            timeout: Some(Duration::ZERO),
            ..Default::default()
        };
        let (blue, green) = tokio::join!(
            page.screenshot(options("rgb(0,0,255)")),
            page.screenshot(options("rgb(0,255,0)"))
        );
        assert_eq!(pixel(&blue.unwrap(), 20, 20), [0, 0, 255, 255]);
        assert_eq!(pixel(&green.unwrap(), 20, 20), [0, 255, 0, 255]);
        clean(&page).await;
        browser.close().await.unwrap();
    }
}

async fn contexts(browser: &Browser) -> Vec<String> {
    let values = if let Some(cdp) = browser.cdp() {
        cdp.call(
            None,
            "Target.getBrowserContexts",
            json!({}),
            Duration::from_secs(5),
        )
        .await
        .unwrap()["browserContextIds"]
            .clone()
    } else {
        let value = browser
            .bidi()
            .unwrap()
            .call("browser.getUserContexts", json!({}), Duration::from_secs(5))
            .await
            .unwrap();
        json!(value["userContexts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["userContext"].as_str().unwrap())
            .collect::<Vec<_>>())
    };
    let mut values = values
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    values.sort();
    values
}

#[tokio::test]
async fn active_preparation_restores_after_cancellation_dropped_wait_and_timeout_and_disposal_releases_contexts(
) {
    for browser in browsers().await {
        for mode in 0..4 {
            let before = contexts(&browser).await;
            let context = browser
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            let page = context.new_page().await.unwrap();
            page.set_viewport(Viewport {
                width: 200,
                height: 120,
            })
            .await
            .unwrap();
            page.set_content(HTML).await.unwrap();
            let cancellation = CancellationToken::new();
            let locator = page
                .locator("#never-present")
                .with_cancellation(cancellation.clone());
            let options = ScreenshotOptions {
                style: Some("#patch{background:rgb(0,0,255)!important}".into()),
                timeout: Some(if mode == 2 {
                    Duration::from_secs(2)
                } else {
                    Duration::ZERO
                }),
                ..Default::default()
            };
            let capture = locator.screenshot_with(options);
            let prepared=page.wait_for_function("document.querySelector('[data-ferrite-screenshot]') && getComputedStyle(document.querySelector('#patch')).backgroundColor === 'rgb(0, 0, 255)'",Duration::from_secs(3));
            match mode {
                0 => {
                    let cancel = async {
                        prepared.await.unwrap();
                        cancellation.cancel_with_reason("cancel active capture");
                    };
                    let (result, ()) = tokio::join!(capture, cancel);
                    assert_eq!(result.unwrap_err().code(), "FERRITE_E2E_CANCELLED");
                }
                1 => {
                    // Once preparation is observable, dropping the whole future
                    // must leave restoration running independently with the gate.
                    {
                        tokio::pin!(capture);
                        tokio::select! {
                            result=&mut capture=>panic!("missing target unexpectedly settled: {result:?}"),
                            result=prepared=>result.unwrap(),
                        }
                    }
                }
                2 => {
                    let (result, observed) = tokio::join!(capture, prepared);
                    observed.unwrap();
                    assert_eq!(result.unwrap_err().code(), "FERRITE_E2E_TIMEOUT");
                }
                _ => {
                    let dispose = async {
                        prepared.await.unwrap();
                        context.clone().close().await.unwrap();
                    };
                    let (result, ()) = tokio::join!(capture, dispose);
                    assert!(matches!(
                        result.unwrap_err(),
                        E2eError::Cancelled(_) | E2eError::Disconnected(_)
                    ));
                    assert!(page.is_closed());
                }
            }
            if mode != 3 {
                // The recovery capture waits for the prior owned restoration;
                // no arbitrary sleep or polling of a completed caller is needed.
                let recovery = page
                    .screenshot(ScreenshotOptions {
                        full_page: true,
                        clip: Some(box_at(40.0, 220.0, 60.0, 40.0)),
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                assert_eq!(pixel(&recovery, 20, 20), [255, 0, 0, 255]);
                clean(&page).await;
                context.clone().close().await.unwrap();
            }
            assert_eq!(contexts(&browser).await, before);
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn styles_reach_open_shadow_roots_and_same_origin_frames_and_are_removed() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(HTML).await.unwrap();
        page.evaluate_value("const host=document.createElement('div');host.id='host';host.style.cssText='position:absolute;left:10px;top:30px';document.body.appendChild(host);host.attachShadow({mode:'open'}).innerHTML='<style>.swatch{width:60px;height:40px;background:rgb(255,0,0)}</style><div class=swatch id=shadow></div>';const frame=document.createElement('iframe');frame.id='frame';frame.style.cssText='position:absolute;left:100px;top:30px;width:100px;height:80px;border:0';frame.srcdoc='<style>html,body{margin:0}.swatch{width:60px;height:40px;background:rgb(255,0,0)}</style><div class=swatch id=inside></div>';document.body.appendChild(frame);true").await.unwrap();
        page.wait_for_function(
            "document.querySelector('#frame').contentDocument?.querySelector('#inside')",
            Duration::from_secs(3),
        )
        .await
        .unwrap();
        let options = || ScreenshotOptions {
            style: Some(".swatch{background:rgb(0,0,255)!important}".into()),
            ..Default::default()
        };
        let shot = page
            .locator("#shadow")
            .screenshot_with(options())
            .await
            .unwrap();
        assert_eq!(image(&shot).dimensions(), (60, 40));
        assert_eq!(pixel(&shot, 20, 20), [0, 0, 255, 255]);
        let shot = page
            .frame_locator("#frame")
            .locator("#inside")
            .screenshot_with(options())
            .await
            .unwrap();
        assert_eq!(image(&shot).dimensions(), (60, 40));
        assert_eq!(pixel(&shot, 20, 20), [0, 0, 255, 255]);
        assert_eq!(page.evaluate::<String>("getComputedStyle(document.querySelector('#host').shadowRoot.querySelector('#shadow')).backgroundColor").await.unwrap(),"rgb(255, 0, 0)");
        assert_eq!(page.evaluate::<String>("getComputedStyle(document.querySelector('#frame').contentDocument.querySelector('#inside')).backgroundColor").await.unwrap(),"rgb(255, 0, 0)");
        assert_eq!(page.evaluate::<u32>("document.querySelector('#host').shadowRoot.querySelectorAll('[data-ferrite-screenshot]').length + document.querySelector('#frame').contentDocument.querySelectorAll('[data-ferrite-screenshot]').length").await.unwrap(),0);
        clean(&page).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn restoration_failures_are_visible_and_do_not_discard_original_capture_errors() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(HTML).await.unwrap();
        // Simulate application code making the capture's private registry
        // nonconfigurable. Styles can still be removed, but registry release
        // genuinely fails in the native realm and must not be ignored.
        page.evaluate_value("window.sabotage=new MutationObserver(records=>{for(const r of records)for(const n of r.addedNodes){const key=n.getAttribute?.('data-ferrite-screenshot');if(key && Object.hasOwn(window,key))Object.defineProperty(window,key,{configurable:false})}});sabotage.observe(document,{childList:true,subtree:true});true").await.unwrap();
        let failed = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                clip: Some(box_at(9999.0, 9999.0, 20.0, 20.0)),
                style: Some("#patch{background:blue!important}".into()),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(failed.code(), "FERRITE_E2E_CONFIG");
        assert!(failed
            .to_string()
            .contains("screenshot restoration also failed"));
        assert!(failed
            .to_string()
            .contains("ownership state could not be removed"));
        assert_eq!(
            page.evaluate::<String>(
                "getComputedStyle(document.querySelector('#patch')).backgroundColor"
            )
            .await
            .unwrap(),
            "rgb(255, 0, 0)"
        );
        assert_eq!(
            page.evaluate::<u32>("document.querySelectorAll('[data-ferrite-screenshot]').length")
                .await
                .unwrap(),
            0
        );
        let errors = page.take_screenshot_cleanup_errors();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("ownership state could not be removed"));
        assert!(page.take_screenshot_cleanup_errors().is_empty());
        let timed_out = page
            .locator("#never-present")
            .screenshot_with(ScreenshotOptions {
                style: Some("#patch{background:blue!important}".into()),
                timeout: Some(Duration::from_secs(2)),
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert_eq!(timed_out.code(), "FERRITE_E2E_TIMEOUT");
        // A subsequent capture waits for the dropped caller's restoration and
        // reports its real failure before making any new temporary changes.
        let deferred = page.screenshot(Default::default()).await.unwrap_err();
        assert_eq!(deferred.code(), "FERRITE_E2E_CONFIG");
        assert!(deferred
            .to_string()
            .contains("previous screenshot restoration failed"));
        assert!(deferred
            .to_string()
            .contains("ownership state could not be removed"));
        assert!(page.take_screenshot_cleanup_errors().is_empty());
        assert_eq!(
            page.evaluate::<String>(
                "getComputedStyle(document.querySelector('#patch')).backgroundColor"
            )
            .await
            .unwrap(),
            "rgb(255, 0, 0)"
        );
        // Navigation releases the deliberately frozen application realm.
        page.goto("data:text/html,<p>new realm</p>").await.unwrap();
        page.set_content(HTML).await.unwrap();
        let recovery = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                clip: Some(box_at(40.0, 220.0, 60.0, 40.0)),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(pixel(&recovery, 20, 20), [255, 0, 0, 255]);
        clean(&page).await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn explicit_captures_emit_one_action_and_internal_trace_captures_add_no_steps() {
    for browser in browsers().await {
        let dir = tempfile::tempdir().unwrap();
        let trace = dir.path().join("capture-trace.json");
        let trace_output = trace.clone();
        let report = Runner::from_config(&E2eConfig {
            screenshot: "off".into(),
            workers: 1,
            output_dir: dir.path().display().to_string(),
            ..Default::default()
        })
        .list_progress(false)
        .run(
            &browser,
            vec![test_with_context("capture step scopes", move |ctx| {
                let trace = trace_output.clone();
                async move {
                    ctx.context
                        .start_tracing(TracingOptions::default().screenshots(true));
                    ctx.page.set_content(HTML).await?;
                    ctx.page
                        .step("user capture step", async {
                            let shot = ctx.page.screenshot(Default::default()).await?;
                            assert!(!shot.is_empty());
                            Ok::<(), E2eError>(())
                        })
                        .await?;
                    ctx.context.stop_tracing(&trace).await?;
                    Ok(())
                }
            })],
        )
        .await;
        assert!(report.ok(), "{}", report.to_list());
        fn flatten(steps: &[StepInfo]) -> Vec<&StepInfo> {
            steps
                .iter()
                .flat_map(|s| std::iter::once(s).chain(flatten(&s.steps)))
                .collect()
        }
        let steps = flatten(&report.results[0].attempt_results[0].steps);
        assert_eq!(
            steps
                .iter()
                .filter(|s| s.title == "page.screenshot")
                .count(),
            1
        );
        let trace: serde_json::Value =
            serde_json::from_slice(&std::fs::read(trace).unwrap()).unwrap();
        assert_eq!(trace["screenshots"].as_array().unwrap().len(), 1);
        browser.close().await.unwrap();
    }
}
