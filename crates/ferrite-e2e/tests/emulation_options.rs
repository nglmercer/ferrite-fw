//! Application-observable media, screen/touch metrics and UA client hints.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::Duration;
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
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{http::HeaderMap, response::Html, routing::get, Router};
    let app=Router::new().route("/",get(||async{Html("<meta name=viewport content='width=device-width'><title>Emulation</title><div>fixture</div>")}))
        .route("/headers",get(|headers:HeaderMap|async move{axum::Json(json!({"ua":headers.get("user-agent").and_then(|v|v.to_str().ok()),"language":headers.get("accept-language").and_then(|v|v.to_str().ok()),"brands":headers.get("sec-ch-ua").and_then(|v|v.to_str().ok()),"mobile":headers.get("sec-ch-ua-mobile").and_then(|v|v.to_str().ok()),"platform":headers.get("sec-ch-ua-platform").and_then(|v|v.to_str().ok())}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
    .abort_handle();
    (base, task)
}
async fn media(page: &Page) -> Value {
    page.evaluate_value("Object.fromEntries(['print','screen','(prefers-color-scheme: dark)','(prefers-reduced-motion: reduce)','(forced-colors: active)','(prefers-contrast: more)','(prefers-contrast: less)','(prefers-contrast: custom)'].map(q=>[q,matchMedia(q).matches]))").await.unwrap()
}
async fn metrics(page: &Page) -> Value {
    page.evaluate_value("({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,screenWidth:screen.width,screenHeight:screen.height,x:screenX,y:screenY,type:screen.orientation.type,angle:screen.orientation.angle,touch:navigator.maxTouchPoints})").await.unwrap()
}
#[tokio::test]
async fn media_keep_reset_and_legacy_updates_preserve_other_features() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        let baseline = media(&page).await;
        if browser.kind() == BrowserKind::Firefox {
            assert!(matches!(
                page.emulate_media_with(MediaOptions {
                    media: EmulationOverride::Set(MediaType::Print),
                    ..Default::default()
                })
                .await,
                Err(E2eError::Config(_))
            ));
            page.emulate_media_with(MediaOptions::default())
                .await
                .unwrap();
        } else {
            page.emulate_media_with(MediaOptions {
                media: EmulationOverride::Set(MediaType::Print),
                color_scheme: EmulationOverride::Set(MediaColorScheme::Dark),
                reduced_motion: EmulationOverride::Set(ReducedMotion::Reduce),
                forced_colors: EmulationOverride::Set(ForcedColors::Active),
                contrast: EmulationOverride::Set(ContrastPreference::More),
                ..Default::default()
            })
            .await
            .unwrap();
            let observed = media(&page).await;
            assert_eq!(observed["print"], json!(true));
            assert_eq!(observed["(prefers-color-scheme: dark)"], json!(true));
            assert_eq!(observed["(forced-colors: active)"], json!(true));
            assert_eq!(observed["(prefers-contrast: more)"], json!(true));
            for contrast in [ContrastPreference::Less, ContrastPreference::Custom] {
                page.emulate_media_with(MediaOptions {
                    contrast: EmulationOverride::Set(contrast),
                    ..Default::default()
                })
                .await
                .unwrap();
                let observed = media(&page).await;
                assert_eq!(observed["print"], json!(true));
                assert_eq!(observed["(prefers-reduced-motion: reduce)"], json!(true));
                assert_eq!(
                    observed[if contrast == ContrastPreference::Less {
                        "(prefers-contrast: less)"
                    } else {
                        "(prefers-contrast: custom)"
                    }],
                    json!(true)
                );
            }
            page.emulate_media(None, Some(ReducedMotion::NoPreference))
                .await
                .unwrap();
            let observed = media(&page).await;
            assert_eq!(observed["print"], json!(true));
            assert_eq!(observed["(prefers-reduced-motion: reduce)"], json!(false));
            assert_eq!(observed["(forced-colors: active)"], json!(true));
            page.emulate_media_with(MediaOptions {
                color_scheme: EmulationOverride::Reset,
                reduced_motion: EmulationOverride::Reset,
                forced_colors: EmulationOverride::Reset,
                contrast: EmulationOverride::Reset,
                ..Default::default()
            })
            .await
            .unwrap();
            assert_eq!(media(&page).await["print"], json!(true));
            page.emulate_media_with(MediaOptions::reset())
                .await
                .unwrap();
            assert_eq!(media(&page).await, baseline);
            let token = CancellationToken::new();
            token.cancel();
            assert!(matches!(
                page.emulate_media_with(MediaOptions {
                    media: EmulationOverride::Set(MediaType::Print),
                    operation: OperationOptions {
                        timeout: None,
                        cancellation: Some(token)
                    },
                    ..Default::default()
                })
                .await,
                Err(E2eError::Cancelled(_))
            ));
            assert_eq!(media(&page).await, baseline);
            page.close().await.unwrap();
            assert!(matches!(
                page.emulate_media_with(MediaOptions::reset()).await,
                Err(E2eError::Cancelled(_))
            ));
        }
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn device_metrics_user_agent_hints_and_resets() {
    let (base, server) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        if browser.kind() == BrowserKind::Firefox {
            assert!(matches!(
                page.emulate_device_metrics(DeviceMetricsOptions::new(320, 240))
                    .await,
                Err(E2eError::Config(_))
            ));
            assert!(matches!(
                page.reset_device_metrics(OperationOptions::default()).await,
                Err(E2eError::Config(_))
            ));
            assert!(matches!(
                page.set_user_agent_with(UserAgentOptions {
                    user_agent: "Ferrite/1".into(),
                    ..Default::default()
                })
                .await,
                Err(E2eError::Config(_))
            ));
        } else {
            let baseline = metrics(&page).await;
            let mut options = DeviceMetricsOptions::new(320, 240);
            options.device_scale_factor = 2.0;
            options.mobile = true;
            options.screen = Some(Viewport {
                width: 800,
                height: 900,
            });
            options.position = Some(ScreenPosition { x: 20, y: 40 });
            options.orientation = Some(ScreenOrientation {
                kind: ScreenOrientationType::LandscapePrimary,
                angle: 90,
            });
            options.touch_points = Some(3);
            options.scale = Some(1.0);
            page.emulate_device_metrics(options.clone()).await.unwrap();
            let observed = metrics(&page).await;
            eprintln!("custom metrics {observed}");
            assert_eq!(
                observed,
                json!({"width":320,"height":240,"dpr":2,"screenWidth":800,"screenHeight":900,"x":20,"y":40,"type":"landscape-primary","angle":90,"touch":3})
            );
            let shot = page.screenshot(ScreenshotOptions::default()).await.unwrap();
            let image = image::load_from_memory(&shot).unwrap();
            assert_eq!((image.width(), image.height()), (640, 480));
            options.orientation.as_mut().unwrap().angle = 37;
            page.emulate_device_metrics(options).await.unwrap();
            assert_eq!(metrics(&page).await["angle"], json!(37));
            page.reset_device_metrics(OperationOptions::default())
                .await
                .unwrap();
            assert_eq!(metrics(&page).await, baseline);
            let original=page.evaluate_value("({ua:navigator.userAgent,platform:navigator.platform,language:navigator.language})").await.unwrap();
            let metadata = UserAgentMetadata {
                brands: vec![UserAgentBrandVersion {
                    brand: "Ferrite".into(),
                    version: "7".into(),
                }],
                full_version_list: vec![UserAgentBrandVersion {
                    brand: "Ferrite".into(),
                    version: "7.2.3".into(),
                }],
                platform: "FerriteOS".into(),
                platform_version: "1.2".into(),
                architecture: "arm".into(),
                model: "Phone".into(),
                mobile: true,
                bitness: Some("64".into()),
                wow64: Some(false),
                form_factors: vec!["Mobile".into()],
            };
            page.set_user_agent_with(UserAgentOptions {
                user_agent: "Ferrite/7.2".into(),
                accept_language: Some("es-PE,en".into()),
                platform: Some("FerritePlatform".into()),
                metadata: Some(metadata),
                operation: OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: None,
                },
            })
            .await
            .unwrap();
            page.reload().await.unwrap();
            let observed=page.evaluate_value("(async()=>({ua:navigator.userAgent,platform:navigator.platform,language:navigator.language,hints:await navigator.userAgentData.getHighEntropyValues(['architecture','model','bitness','platformVersion','fullVersionList','wow64','formFactors'])}))()").await.unwrap();
            eprintln!("UA hints {observed}");
            assert_eq!(observed["ua"], json!("Ferrite/7.2"));
            assert_eq!(observed["platform"], json!("FerritePlatform"));
            assert_eq!(observed["language"], json!("es-PE"));
            assert_eq!(observed["hints"]["platform"], json!("FerriteOS"));
            assert_eq!(observed["hints"]["mobile"], json!(true));
            assert_eq!(observed["hints"]["architecture"], json!("arm"));
            assert_eq!(observed["hints"]["model"], json!("Phone"));
            let headers = page
                .evaluate_value("fetch('/headers').then(r=>r.json())")
                .await
                .unwrap();
            assert_eq!(headers["ua"], json!("Ferrite/7.2"));
            assert_eq!(headers["mobile"], json!("?1"));
            assert_eq!(headers["platform"], json!("\"FerriteOS\""));
            assert!(headers["language"].as_str().unwrap().starts_with("es-PE"));
            page.reset_user_agent(OperationOptions::default())
                .await
                .unwrap();
            page.reload().await.unwrap();
            assert_eq!(page.evaluate_value("({ua:navigator.userAgent,platform:navigator.platform,language:navigator.language})").await.unwrap(),original);
            let mut invalid = DeviceMetricsOptions::new(400, 300);
            invalid.device_scale_factor = f64::NAN;
            assert!(matches!(
                page.emulate_device_metrics(invalid).await,
                Err(E2eError::Config(_))
            ));
            assert_eq!(metrics(&page).await, baseline);
        }
        browser.close().await.unwrap();
    }
    server.abort();
}
