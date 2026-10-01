//! HAR candidate matching, scoped miss policies, native binary/redirect replay.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
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
async fn server() -> (String, Arc<AtomicUsize>, tokio::task::AbortHandle) {
    use axum::{
        response::{Html, Redirect},
        routing::get,
        Router,
    };
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let app = Router::new()
        .route("/", get(|| async { Html("<title>HAR fixture</title>") }))
        .route(
            "/replay/miss",
            get(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    "network"
                }
            }),
        )
        .route("/outside", get(|| async { "outside" }))
        .route(
            "/native/from",
            get(|| async { Redirect::temporary("/native/to") }),
        )
        .route(
            "/native/to",
            get(|| async {
                (
                    [("Content-Type", "application/octet-stream")],
                    vec![0_u8, 255, 128, 65],
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
    .abort_handle();
    (base, hits, task)
}
fn entry(base: &str, path: &str, text: &str) -> Value {
    json!({"request":{"method":"GET","url":format!("{base}{path}")},"response":{"status":200,"headers":[{"name":"Content-Type","value":"text/plain"}],"content":{"text":text}}})
}
async fn fetch(page: &Page, path: &str) -> Value {
    page.evaluate_value(&format!("fetch({}).then(async r=>({{status:r.status,text:await r.text(),url:r.url}})).catch(e=>({{error:e.name}}))",serde_json::to_string(path).unwrap())).await.unwrap()
}
#[tokio::test]
async fn har_replay_duplicates_redirects_binary_miss_scope_and_context_pages() {
    let (base, hits, server) = server().await;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("replay.har");
    let mut first = entry(&base, "/replay/choice", "first");
    first["request"]["headers"] = json!([{"name":"X-Choice","value":"one"}]);
    let mut second = entry(&base, "/replay/choice", "second");
    second["request"]["headers"] = json!([{"name":"X-Choice","value":"two"}]);
    let mut redirect = entry(&base, "/replay/from", "");
    redirect["response"]["status"] = json!(302);
    redirect["response"]["redirectURL"] = json!("/replay/to");
    let mut binary = entry(&base, "/replay/binary", "");
    binary["response"]["content"] = json!({"text":"AP+AQQ==","encoding":"base64"});
    let mut post = entry(&base, "/replay/post", "posted");
    post["request"]["method"] = json!("POST");
    post["request"]["postData"] = json!({"text":"payload"});
    std::fs::write(&path,serde_json::to_vec(&json!({"log":{"entries":[first,second,redirect,entry(&base,"/replay/to","redirected"),binary,post]}})).unwrap()).unwrap();
    let empty = directory.path().join("empty.har");
    std::fs::write(&empty, r#"{"log":{"entries":[]}}"#).unwrap();
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let matcher = UrlMatcher::glob("**/replay/**").unwrap();
        assert_eq!(
            page.route_from_har(
                &path,
                RouteFromHarOptions::default()
                    .matching(matcher.clone())
                    .not_found(HarNotFound::Abort)
            )
            .await
            .unwrap(),
            6
        );
        let chosen = page
            .evaluate_value(
                "fetch('/replay/choice',{headers:{'X-Choice':'two'}}).then(r=>r.text())",
            )
            .await
            .unwrap();
        assert_eq!(chosen, json!("second"));
        assert_eq!(
            fetch(&page, "/replay/from").await["text"],
            json!("redirected")
        );
        let bytes=page.evaluate_value("fetch('/replay/binary').then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)))").await.unwrap();
        assert_eq!(bytes, json!([0, 255, 128, 65]));
        let post=page.evaluate_value("fetch('/replay/post',{method:'POST',body:'payload'}).then(r=>r.text()).catch(e=>e.name)").await.unwrap();
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(post, json!("posted"));
        } else {
            assert_eq!(
                post,
                json!("TypeError"),
                "Firefox request bytes are explicitly unavailable"
            );
        }
        let mismatch = page.evaluate_value("fetch('/replay/post',{method:'POST',body:'wrong'}).then(()=>false).catch(()=>true)").await.unwrap();
        assert_eq!(mismatch, json!(true));
        let bad = directory.path().join("invalid.har");
        std::fs::write(&bad,r#"{"log":{"entries":[{"request":{"method":"GET","url":"http://host/bad"},"response":{"status":200,"content":{"text":"!!!","encoding":"base64"}}}]}}"#).unwrap();
        assert!(matches!(
            page.route_from_har(&bad, RouteFromHarOptions::default())
                .await,
            Err(E2eError::Config(_))
        ));
        assert_eq!(
            fetch(&page, "/replay/to").await["text"],
            json!("redirected"),
            "invalid installation preserves existing routes"
        );
        let before = hits.load(Ordering::SeqCst);
        assert_eq!(
            fetch(&page, "/replay/miss").await["error"],
            json!("TypeError")
        );
        assert_eq!(hits.load(Ordering::SeqCst), before);
        assert_eq!(fetch(&page, "/outside").await["text"], json!("outside"));
        assert_eq!(page.unroute_matching(&matcher).await.unwrap(), 1);
        assert_eq!(fetch(&page, "/replay/miss").await["text"], json!("network"));
        assert_eq!(
            page.route_from_har(
                &empty,
                RouteFromHarOptions::default()
                    .matching(UrlMatcher::exact("/replay/miss"))
                    .not_found(HarNotFound::Abort)
            )
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            fetch(&page, "/replay/miss").await["error"],
            json!("TypeError")
        );
        page.unroute_all().await.unwrap();
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        assert_eq!(
            context
                .route_from_har(&path, RouteFromHarOptions::default().matching(matcher))
                .await
                .unwrap(),
            6
        );
        let future = context.new_page().await.unwrap();
        future.goto("/").await.unwrap();
        assert_eq!(
            fetch(&future, "/replay/to").await["text"],
            json!("redirected")
        );
        assert_eq!(
            fetch(&future, "/replay/miss").await["text"],
            json!("network")
        );
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    server.abort();
}
#[tokio::test]
async fn har_export_redirect_hops_minimal_content_and_cancellation() {
    let (base, _, server) = server().await;
    let directory = tempfile::tempdir().unwrap();
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        page.start_request_capture();
        let matcher = UrlMatcher::exact("/native/to");
        let (response, trigger) = tokio::join!(
            page.wait_for_response_handle(&matcher, OperationOptions::default()),
            page.evaluate_value("fetch('/native/from').then(r=>r.arrayBuffer())")
        );
        trigger.unwrap();
        let response = response.unwrap();
        response.finished().await.unwrap();
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(response.body().await.unwrap(), vec![0, 255, 128, 65]);
        }
        let path = directory
            .path()
            .join(format!("{}.har", browser.kind().name()));
        page.save_har_with_options(
            &path,
            HarExportOptions {
                content: HarContentMode::Embed,
                timing: HarTimingMode::Omit,
                url_matcher: Some(UrlMatcher::glob("**/native/**").unwrap()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let entries = document["log"]["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["response"]["status"], json!(307));
        assert_eq!(
            entries[0]["response"]["redirectURL"],
            json!(format!("{base}/native/to"))
        );
        assert_eq!(entries[1]["timings"]["wait"], json!(-1));
        if browser.kind() == BrowserKind::Chromium {
            assert_eq!(entries[1]["response"]["content"]["text"], json!("AP+AQQ=="));
        } else {
            assert!(entries[1]["response"]["content"].get("text").is_none());
        }
        let loaded = HarFile::load(&path).unwrap();
        assert_eq!(loaded.entries().len(), 2);
        let token = CancellationToken::new();
        token.cancel();
        let original = std::fs::read(&path).unwrap();
        assert!(matches!(
            page.save_har_with_options(
                &path,
                HarExportOptions {
                    operation: OperationOptions {
                        timeout: Some(Duration::ZERO),
                        cancellation: Some(token.clone())
                    },
                    ..Default::default()
                }
            )
            .await,
            Err(E2eError::Cancelled(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(matches!(
            page.route_from_har(
                &path,
                RouteFromHarOptions {
                    operation: OperationOptions {
                        timeout: None,
                        cancellation: Some(token)
                    },
                    ..Default::default()
                }
            )
            .await,
            Err(E2eError::Cancelled(_))
        ));
        page.save_har_with_options(
            &path,
            HarExportOptions {
                mode: HarRecordMode::Minimal,
                operation: OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: None,
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let minimal: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(minimal["log"].get("pages").is_none());
        assert!(minimal["log"]["entries"][0].get("timings").is_none());
        browser.close().await.unwrap();
    }
    server.abort();
}
