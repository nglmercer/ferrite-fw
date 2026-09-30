//! Native tree lookup keeps frame identities through navigation and replacement.
use ferrite_e2e::*;
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
                .expect("installed browser must launch");
            eprintln!(
                "Frame lookup native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("Frame lookup browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{response::Html, routing::get, Router};
    let app=Router::new()
        .route("/",get(||async {Html("<iframe id='outer' name='outer' src='/outer'></iframe><iframe name='side' src='/side'></iframe>")}))
        .route("/outer",get(||async {Html("<p>outer</p><iframe name='inner' src='/inner'></iframe>")}))
        .route("/inner",get(||async {Html("<p>inner</p>")}))
        .route("/other",get(||async {Html("<p>other</p>")}))
        .route("/side",get(||async {Html("<p>side</p>")}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}

#[tokio::test]
async fn native_main_matching_predicates_navigation_replacement_and_detachment() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let main = page.main_frame().await.unwrap();
        assert_eq!(main.parent_id(), None);
        assert_eq!(main.page().target_id(), page.target_id());
        assert!(main.parent().await.unwrap().is_none());
        assert_eq!(main.child_frames().await.unwrap().len(), 2);
        let outer = page
            .frame_by_url_matching(&UrlMatcher::exact("/outer"))
            .await
            .unwrap()
            .unwrap();
        let inner = page
            .frame_by_url_matching(&UrlMatcher::glob("**/inner").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inner.parent().await.unwrap().unwrap().id(), outer.id());
        assert!(!inner.is_detached().await.unwrap());
        assert_eq!(
            inner.locator("p").text_content().await.unwrap().as_deref(),
            Some("inner")
        );
        let side = page
            .frame_by_url_matching(&UrlMatcher::regex("/side$").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(side.parent_id(), Some(main.id()));
        assert!(page
            .frame_by_url_matching(&UrlMatcher::exact("/absent"))
            .await
            .unwrap()
            .is_none());
        let mut calls = 0;
        assert!(page
            .frame_by_url_where(|_| {
                calls += 1;
                false
            })
            .await
            .unwrap()
            .is_none());
        assert_eq!(calls, 4);
        assert_eq!(
            page.frame_by_url_where(|url| url == base)
                .await
                .unwrap()
                .unwrap()
                .id(),
            main.id()
        );
        match browser.kind() {
            BrowserKind::Chromium => assert_eq!(
                page.frame_by_name("outer").await.unwrap().unwrap().id(),
                outer.id()
            ),
            BrowserKind::Firefox => {
                assert!(page
                    .document_frames()
                    .await
                    .unwrap()
                    .iter()
                    .all(|f| f.name().is_empty()));
                assert!(page.frame_by_name("outer").await.unwrap().is_none());
            }
        }
        outer.goto(&format!("{base}other")).await.unwrap();
        outer
            .wait_for_url("/other", Duration::from_secs(3))
            .await
            .unwrap();
        assert!(
            outer.url().ends_with("/outer"),
            "lookup URL remains a snapshot"
        );
        assert!(outer.current_url().await.unwrap().ends_with("/other"));
        assert_eq!(
            page.frame_by_url_matching(&UrlMatcher::exact("/other"))
                .await
                .unwrap()
                .unwrap()
                .id(),
            outer.id()
        );
        assert!(inner.is_detached().await.unwrap());
        assert!(inner.current_url().await.is_err());
        page.evaluate_value("(() => {const old=document.querySelector('#outer'); const next=document.createElement('iframe'); next.id='outer'; next.src='/outer'; old.replaceWith(next); return true})()").await.unwrap();
        page.locator("#outer")
            .content_frame()
            .locator("p")
            .expect()
            .text("outer")
            .await
            .unwrap();
        let replacement = page
            .frame_by_url_matching(&UrlMatcher::exact("/outer"))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(replacement.id(), outer.id());
        assert!(outer.is_detached().await.unwrap());
        assert!(!replacement.is_detached().await.unwrap());
        assert_eq!(page.main_frame().await.unwrap().id(), main.id());
        page.goto("/other").await.unwrap();
        assert_eq!(page.main_frame().await.unwrap().id(), main.id());
        assert!(main.current_url().await.unwrap().ends_with("/other"));
        assert!(replacement.is_detached().await.unwrap());
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            page.with_cancellation(token).main_frame().await,
            Err(E2eError::Cancelled(_))
        ));
        assert!(!main.is_detached().await.unwrap());
        page.close().await.unwrap();
        assert!(main.is_detached().await.unwrap());
        assert!(matches!(
            page.main_frame().await,
            Err(E2eError::Cancelled(_))
        ));
        browser.close().await.unwrap();
    }
    stop.abort();
}
