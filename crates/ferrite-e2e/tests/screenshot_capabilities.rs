//! Native capture probes: dimensions, document coordinates, scale and alpha.
use base64::Engine;
use ferrite_e2e::*;
use image::GenericImageView;
use serde_json::{json, Value};

fn decode(result: Value) -> image::DynamicImage {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(result["data"].as_str().unwrap())
        .unwrap();
    image::load_from_memory(&bytes).unwrap()
}

#[tokio::test]
async fn native_document_capture_preserves_metrics_and_observes_scale_background_capabilities() {
    let mut engines = 0;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(path) = path else { continue };
        engines += 1;
        let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
            .await
            .unwrap();
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 200,
            height: 120,
        })
        .await
        .unwrap();
        page.set_content("<style>html,body{margin:0;background:transparent}body{height:400px}#patch{position:absolute;left:40px;top:220px;width:60px;height:40px;background:rgb(255,0,0)}</style><div id=patch></div>").await.unwrap();
        page.evaluate_value("scrollTo(0,180);true").await.unwrap();
        let metrics = page
            .evaluate_value("[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]")
            .await
            .unwrap();
        let method = if kind == BrowserKind::Chromium {
            "Page.captureScreenshot"
        } else {
            "browsingContext.captureScreenshot"
        };
        let clip = if kind == BrowserKind::Chromium {
            json!({"clip":{"x":40,"y":220,"width":60,"height":40,"scale":1},"captureBeyondViewport":true})
        } else {
            json!({"origin":"document","clip":{"type":"box","x":40,"y":220,"width":60,"height":40}})
        };
        let patch = decode(page.call(method, clip).await.unwrap());
        assert_eq!(patch.dimensions(), (60, 40));
        assert_eq!(patch.to_rgba8().get_pixel(20, 20).0, [255, 0, 0, 255]);
        let full = if kind == BrowserKind::Chromium {
            json!({"clip":{"x":0,"y":0,"width":200,"height":400,"scale":1},"captureBeyondViewport":true})
        } else {
            json!({"origin":"document"})
        };
        let full = decode(page.call(method, full).await.unwrap());
        let document_width: u32 = page
            .evaluate("document.documentElement.scrollWidth")
            .await
            .unwrap();
        assert_eq!(full.dimensions(), (document_width, 400));
        assert_eq!(
            page.evaluate_value("[innerWidth,innerHeight,devicePixelRatio,scrollX,scrollY]")
                .await
                .unwrap(),
            metrics
        );
        eprintln!(
            "capture {} {} document {:?}, native white background {:?}",
            kind.name(),
            browser.version().await.unwrap(),
            full.dimensions(),
            full.to_rgba8().get_pixel(5, 5).0
        );
        if kind == BrowserKind::Chromium {
            page.call(
                "Emulation.setDeviceMetricsOverride",
                json!({"width":200,"height":120,"deviceScaleFactor":2,"mobile":false}),
            )
            .await
            .unwrap();
        } else {
            let result = page
                .call(
                    "browsingContext.setViewport",
                    json!({"viewport":{"width":200,"height":120},"devicePixelRatio":2}),
                )
                .await;
            eprintln!("Firefox devicePixelRatio override: {result:?}");
            result.unwrap();
        }
        let actual_dpr: f64 = page.evaluate("devicePixelRatio").await.unwrap();
        assert_eq!(actual_dpr, 2.0);
        let viewport = decode(
            page.call(
                method,
                if kind == BrowserKind::Chromium {
                    json!({"captureBeyondViewport":false})
                } else {
                    json!({"origin":"viewport"})
                },
            )
            .await
            .unwrap(),
        );
        eprintln!("{} native viewport {:?}, metrics {}",kind.name(),viewport.dimensions(),page.evaluate_value("[innerWidth,innerHeight,visualViewport.width,visualViewport.height,visualViewport.scale,devicePixelRatio]").await.unwrap());
        let device=decode(page.call(method,if kind==BrowserKind::Chromium {
            json!({"clip":{"x":0,"y":0,"width":200,"height":400,"scale":1},"captureBeyondViewport":true})
        } else {json!({"origin":"document"})}).await.unwrap());
        assert_eq!(device.dimensions(), (document_width * 2, 800));
        if kind == BrowserKind::Chromium {
            let css=decode(page.call(method,json!({"clip":{"x":0,"y":0,"width":200,"height":400,"scale":0.5},"captureBeyondViewport":true})).await.unwrap());
            assert_eq!(css.dimensions(), (200, 400));
            page.call(
                "Emulation.setDefaultBackgroundColorOverride",
                json!({"color":{"r":0,"g":0,"b":0,"a":0}}),
            )
            .await
            .unwrap();
            let alpha=decode(page.call(method,json!({"clip":{"x":0,"y":0,"width":200,"height":400,"scale":1},"captureBeyondViewport":true})).await.unwrap());
            assert_eq!(alpha.to_rgba8().get_pixel(5, 5).0[3], 0);
            page.call("Emulation.setDefaultBackgroundColorOverride", json!({}))
                .await
                .unwrap();
        }
        browser.close().await.unwrap();
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(engines, 2);
    }
}
