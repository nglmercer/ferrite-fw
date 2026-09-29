use ferrite_e2e::*;
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
                .expect("installed engine must launch");
            eprintln!(
                "daily API {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("daily API browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
fn cookie(name: &str, value: &str, domain: &str, path: &str) -> Cookie {
    Cookie {
        name: name.into(),
        value: value.into(),
        domain: Some(domain.into()),
        path: Some(path.into()),
        secure: false,
        http_only: false,
        same_site: Some("Lax".into()),
        expires: None,
    }
}
#[tokio::test]
async fn filtered_cookies_preserve_keys_metadata_and_linked_clients() {
    use axum::{http::HeaderMap, routing::get, Router};
    let app = Router::new().fallback(get(|headers: HeaderMap| async move {
        headers
            .get("cookie")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    for browser in browsers().await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        for value in [
            cookie("same", "root", "127.0.0.1", "/"),
            cookie("same", "admin", "127.0.0.1", "/admin"),
            cookie("same", "other", "localhost", "/"),
            cookie("keep", "safe", "127.0.0.1", "/"),
        ] {
            let url = format!("http://{}/", value.domain.as_deref().unwrap());
            context.add_cookies(&[value], &url).await.unwrap();
        }
        let mut protected = cookie("protected", "secret", "secure.example.test", "/");
        protected.secure = true;
        protected.http_only = true;
        protected.same_site = Some("Strict".into());
        protected.expires = Some(2_000_000_000);
        context
            .add_cookies(&[protected], "https://secure.example.test/")
            .await
            .unwrap();
        let original = context.cookies().await.unwrap();
        assert_eq!(original.len(), 5);
        let protected = original.iter().find(|c| c.name == "protected").unwrap();
        let protected = json!(protected);
        let api = context.request();
        let before = api.get(&format!("{base}admin")).await.unwrap().text();
        assert!(before.contains("same=admin") && before.contains("same=root"));
        context
            .clear_cookies_with(
                CookieFilter::default()
                    .name("same")
                    .domain("127.0.0.1")
                    .path("/admin"),
            )
            .await
            .unwrap();
        assert_eq!(context.cookies().await.unwrap().len(), 4);
        let after = api.get(&format!("{base}admin")).await.unwrap().text();
        assert!(!after.contains("same=admin") && after.contains("same=root"));
        assert!(
            context.pages().is_empty(),
            "cookie/API operations never create a scratch page"
        );
        context
            .clear_cookies_with(CookieFilter::default().name("KEEP"))
            .await
            .unwrap();
        assert_eq!(
            context.cookies().await.unwrap().len(),
            4,
            "exact filters are case-sensitive"
        );
        context.set_default_timeout(Duration::ZERO);
        context
            .clear_cookies_with(
                CookieFilter::default()
                    .name(TextMatcher::regex("^same$").unwrap())
                    .domain(TextMatcher::regex("^(127\\.0\\.0\\.1|localhost)$").unwrap())
                    .path(TextMatcher::regex("^/$").unwrap()),
            )
            .await
            .unwrap();
        let remaining = context.cookies().await.unwrap();
        assert_eq!(remaining.len(), 2);
        assert_eq!(
            json!(remaining.iter().find(|c| c.name == "protected").unwrap()),
            protected,
            "untouched native metadata survives"
        );
        assert_eq!(api.get(&base).await.unwrap().text(), "keep=safe");
        assert!(TextMatcher::regex("[").is_err());
        let page = context.new_page().await.unwrap();
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(matches!(
            page.with_cancellation(cancellation)
                .clear_cookies_with(CookieFilter::default())
                .await,
            Err(E2eError::Cancelled(_))
        ));
        assert_eq!(context.cookies().await.unwrap().len(), 2);
        page.clear_cookies_with(CookieFilter::default())
            .await
            .unwrap();
        assert!(context.cookies().await.unwrap().is_empty());
        assert_eq!(api.get(&base).await.unwrap().text(), "");
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    server.abort();
}
