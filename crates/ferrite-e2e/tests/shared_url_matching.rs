use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
const WAIT: Duration = Duration::from_secs(4);
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
                .expect("installed browser must launch");
            eprintln!(
                "shared URL native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            browsers.push(browser);
        } else {
            eprintln!("shared URL browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
async fn server() -> (String, tokio::task::AbortHandle) {
    let router = axum::Router::new().fallback(|| async { axum::response::Html("network") });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (origin, task.abort_handle())
}
async fn fetch(page: &Page, path: &str) -> String {
    page.evaluate(&format!(
        "fetch({}).then(r=>r.text())",
        serde_json::to_string(path).unwrap()
    ))
    .await
    .unwrap()
}
#[tokio::test]
async fn pinned_matchers_agree_across_waits_assertions_and_native_routes() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/url-reference.json"
    ))
    .unwrap();
    let (origin, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(format!("{origin}/base/")));
        let page = browser.new_page().await.unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let source = case["pattern"]
                .as_str()
                .unwrap()
                .replace("$ORIGIN", &origin);
            let matcher = match case["kind"].as_str().unwrap() {
                "glob" => UrlMatcher::glob(&source).unwrap(),
                "exact" => UrlMatcher::exact(&source),
                "regex" => UrlMatcher::regex(&source).unwrap(),
                kind => panic!("{kind}"),
            };
            let path = case["path"].as_str().unwrap();
            let matches = case["matches"].as_bool().unwrap();
            page.goto(path).await.unwrap();
            let waited = page
                .wait_for_url_matching(
                    &matcher,
                    if matches {
                        WAIT
                    } else {
                        Duration::from_millis(60)
                    },
                )
                .await;
            if matches {
                waited.unwrap();
                page.expect().url_matching(&matcher).await.unwrap();
            } else {
                assert!(matches!(waited, Err(E2eError::Timeout(..))), "{case}");
                page.expect().not().url_matching(&matcher).await.unwrap();
            }
            let hits = Arc::new(AtomicUsize::new(0));
            let observed = hits.clone();
            page.route_matching(&matcher, move |_| {
                observed.fetch_add(1, Ordering::SeqCst);
                async { Ok(RouteAction::fulfill(200, "matched", "text/plain")) }
            })
            .await
            .unwrap();
            assert_eq!(
                fetch(&page, path).await,
                if matches { "matched" } else { "network" },
                "{case}"
            );
            assert_eq!(hits.load(Ordering::SeqCst), usize::from(matches), "{case}");
            assert_eq!(page.unroute_matching(&matcher).await.unwrap(), 1);
        }
        let cancel = CancellationToken::new();
        let scoped = page.with_cancellation(cancel.clone());
        let expectation = scoped.expect().timeout(Duration::ZERO);
        let (wait, ()) = tokio::join!(expectation.url_where(|_| false), async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            cancel.cancel();
        });
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        assert!(UrlMatcher::glob("{bad,{nested}}").is_err());
        assert!(matches!(
            page.route_with_handler("[", |_| async { Ok(RouteAction::Fallback) })
                .await,
            Err(E2eError::Config(_))
        ));
        assert_eq!(
            page.unroute_all().await.unwrap(),
            0,
            "invalid pattern cannot persist a registration"
        );
        browser.close().await.unwrap();
    }
    stop.abort();
}
#[tokio::test]
async fn shared_context_rules_limits_removal_har_filters_and_legacy_contracts() {
    let (origin, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(format!("{origin}/base/")));
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let matcher = UrlMatcher::glob("/api/**/item").unwrap();
        context
            .route_matching_times(&matcher, 1, |_| async {
                Ok(RouteAction::fulfill(200, "context", "text/plain"))
            })
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        assert_eq!(fetch(&page, "/unrelated").await, "network");
        assert_eq!(fetch(&page, "/api/item").await, "context");
        assert_eq!(
            fetch(&page, "/api/deep/item").await,
            "network",
            "only matching requests consume the limit"
        );
        assert_eq!(context.unroute_matching(&matcher).await.unwrap(), 1);
        page.route(vec![RouteRule::fulfill("**", 200, "rule", "text/plain")
            .matching(UrlMatcher::exact("/api/item"))])
            .await
            .unwrap();
        assert_eq!(fetch(&page, "/api/item").await, "rule");
        assert_eq!(fetch(&page, "/api/item/extra").await, "network");
        assert_eq!(
            page.unroute_matching(&UrlMatcher::exact("/api/item"))
                .await
                .unwrap(),
            1
        );
        // Existing globset string routing remains broader across slashes.
        page.route(vec![RouteRule::fulfill(
            "**/api/*",
            200,
            "legacy",
            "text/plain",
        )])
        .await
        .unwrap();
        assert_eq!(fetch(&page, "/api/deep/item").await, "legacy");
        page.unroute_all().await.unwrap();
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("fixture.har");
        let entries:Vec<Value>=[("/api/item","har-one"),("/api/deep/item","har-two"),("/other","har-other")].into_iter().map(|(path,text)|json!({"request":{"method":"GET","url":format!("{origin}{path}")},"response":{"status":200,"statusText":"OK","headers":[{"name":"content-type","value":"text/plain"}],"content":{"text":text}}})).collect();
        std::fs::write(
            &file,
            serde_json::to_vec(&json!({"log":{"entries":entries}})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            page.route_from_har(
                &file,
                RouteFromHarOptions::default().matching(matcher.clone())
            )
            .await
            .unwrap(),
            2
        );
        assert_eq!(fetch(&page, "/api/item").await, "har-one");
        assert_eq!(fetch(&page, "/other").await, "network");
        page.unroute_all().await.unwrap();
        assert_eq!(
            context
                .route_from_har(
                    &file,
                    RouteFromHarOptions::default().matching(UrlMatcher::regex("/other$").unwrap())
                )
                .await
                .unwrap(),
            1
        );
        let future = context.new_page().await.unwrap();
        future.goto("/").await.unwrap();
        assert_eq!(fetch(&future, "/other").await, "har-other");
        context.unroute_all().await.unwrap();
        let both = RouteFromHarOptions::default()
            .matching(matcher)
            .url_filter("**");
        assert!(matches!(
            page.route_from_har(&file, both).await,
            Err(E2eError::Config(_))
        ));
        assert_eq!(page.unroute_all().await.unwrap(), 0);
        assert!(matches!(
            context
                .route_with_handler("[", |_| async { Ok(RouteAction::Fallback) })
                .await,
            Err(E2eError::Config(_))
        ));
        assert_eq!(context.unroute_all().await.unwrap(), 0);
        browser.close().await.unwrap();
    }
    stop.abort();
}
