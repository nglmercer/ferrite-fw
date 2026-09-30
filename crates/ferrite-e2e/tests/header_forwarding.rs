//! Duplicate headers through API transport, JSON and native route fulfillment.
use ferrite_e2e::*;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

const FIRST_COOKIE: &str = "first=1; Path=/; Expires=Wed, 21 Oct 2037 07:28:00 GMT";
const SECOND_COOKIE: &str = "second=2; Path=/";

fn payload() -> Vec<u8> {
    (0..4097).map(|index| (index % 256) as u8).collect()
}

async fn server() -> (String, tokio::task::AbortHandle) {
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async { axum::response::Html("<p>headers</p>") }),
        )
        .route(
            "/forward",
            axum::routing::get(|| async {
                let mut response = axum::http::Response::new(axum::body::Body::from(payload()));
                for (name, value) in [
                    ("content-type", "application/octet-stream"),
                    ("set-cookie", FIRST_COOKIE),
                    ("set-cookie", SECOND_COOKIE),
                    ("x-repeat", "alpha"),
                    ("x-repeat", "beta, gamma"),
                    ("x-empty", ""),
                ] {
                    response
                        .headers_mut()
                        .append(name, axum::http::HeaderValue::from_static(value));
                }
                response
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}

fn assert_api_headers(response: &ApiResponse) {
    assert_eq!(
        response.header_values("SET-COOKIE"),
        [FIRST_COOKIE, SECOND_COOKIE]
    );
    assert_eq!(
        response.header_value("Set-Cookie"),
        Some(format!("{FIRST_COOKIE}\n{SECOND_COOKIE}"))
    );
    assert_eq!(response.header_values("X-REPEAT"), ["alpha", "beta, gamma"]);
    assert_eq!(
        response.header_value("x-repeat"),
        Some("alpha, beta, gamma".into())
    );
    assert_eq!(response.header_value("x-empty"), Some(String::new()));
    assert_eq!(response.header_value("x-absent"), None);
    assert_eq!(
        response.header("set-cookie"),
        Some(FIRST_COOKIE),
        "legacy lookup still returns the first value"
    );
    let headers = response.headers_array();
    let json = serde_json::to_value(&headers).unwrap();
    let round_trip: Vec<HttpHeader> = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(headers, round_trip);
    assert!(json
        .as_array()
        .unwrap()
        .iter()
        .all(|header| header.get("name").is_some() && header.get("value").is_some()));
    assert_eq!(response.bytes(), payload());
}

#[tokio::test]
async fn duplicate_headers_survive_api_json_and_native_route_forwarding() {
    let (base, stop) = server().await;
    let direct = ApiClient::with_base_url(&base)
        .get("/forward")
        .await
        .unwrap();
    assert_api_headers(&direct);
    let mut executed = 0;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(path) = path else {
            eprintln!("header forwarding browser absent: {}", kind.name());
            continue;
        };
        let mut browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
            .await
            .unwrap();
        executed += 1;
        eprintln!(
            "header forwarding native {}: {}",
            kind.name(),
            browser.version().await.unwrap()
        );
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let forwarded = Arc::new(Mutex::new(Vec::new()));
        let captured = forwarded.clone();
        page.route_matching(&UrlMatcher::exact("/forward"), move |info| {
            let captured = captured.clone();
            async move {
                let response = info.fetch().await?;
                assert_api_headers(&response);
                *captured.lock().unwrap() = response.headers_array();
                Ok(RouteAction::fulfill_full(
                    response.status(),
                    response.status_text(),
                    response.headers().to_vec(),
                    response.bytes().to_vec(),
                ))
            }
        })
        .await
        .unwrap();
        let matcher = UrlMatcher::exact("/forward");
        let (response, body) = tokio::join!(
            page.wait_for_response_handle(&matcher, OperationOptions { timeout: Some(Duration::from_secs(5)), cancellation: None }),
            page.evaluate::<Vec<u8>>("fetch('/forward').then(async response => Array.from(new Uint8Array(await response.arrayBuffer())))")
        );
        assert_eq!(body.unwrap(), payload());
        let response = response.unwrap();
        response.finished().await.unwrap();
        assert!(response.request().snapshot().response_headers_from_route);
        let mut older_snapshot = serde_json::to_value(response.request().snapshot()).unwrap();
        older_snapshot
            .as_object_mut()
            .unwrap()
            .remove("response_headers_from_route");
        let older_snapshot: RequestSnapshot = serde_json::from_value(older_snapshot).unwrap();
        assert!(
            !older_snapshot.response_headers_from_route,
            "older typed snapshots retain serde compatibility"
        );
        assert_eq!(
            response.header_values("set-cookie"),
            [FIRST_COOKIE, SECOND_COOKIE],
            "{} must preserve separate cookies containing commas",
            kind.name()
        );
        assert_eq!(
            response.header_value("x-repeat"),
            Some("alpha, beta, gamma".into())
        );
        let repeats = response.header_values("x-repeat");
        assert!(
            repeats == ["alpha", "beta, gamma"] || repeats == ["alpha, beta, gamma"],
            "native comma-folded fields must stay intact: {repeats:?}"
        );
        let headers = response.headers_array();
        let round_trip: Vec<HttpHeader> =
            serde_json::from_slice(&serde_json::to_vec(&headers).unwrap()).unwrap();
        assert_eq!(headers, round_trip);
        assert_eq!(
            forwarded
                .lock()
                .unwrap()
                .iter()
                .filter(|header| header.name.eq_ignore_ascii_case("set-cookie"))
                .count(),
            2
        );
        let cookies = page.cookies().await.unwrap();
        assert!(cookies
            .iter()
            .any(|cookie| cookie.name == "first" && cookie.value == "1"));
        assert!(cookies
            .iter()
            .any(|cookie| cookie.name == "second" && cookie.value == "2"));
        page.unroute_all().await.unwrap();

        // Same-URL synthetic redirects reuse native IDs on some backends;
        // each intercepted hop must retain its own submitted header pairs.
        let hops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        page.route_matching(&UrlMatcher::exact("/redirect-forward"), move |_| {
            let hop = hops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                assert!(hop < 2, "the synthetic redirect must terminate");
                let mut headers = vec![
                    ("x-hop".into(), hop.to_string()),
                    ("set-cookie".into(), format!("hop{hop}first=1; Path=/")),
                    ("set-cookie".into(), format!("hop{hop}second=2; Path=/")),
                ];
                if hop == 0 {
                    headers.push(("location".into(), "/redirect-forward".into()));
                }
                Ok(RouteAction::fulfill_full(
                    if hop == 0 { 302 } else { 200 },
                    "",
                    headers,
                    b"hop body".to_vec(),
                ))
            }
        })
        .await
        .unwrap();
        let body: String = page
            .evaluate("fetch('/redirect-forward').then(response=>response.text())")
            .await
            .unwrap();
        assert_eq!(body, "hop body");
        let requests: Vec<_> = page
            .network_requests()
            .into_iter()
            .filter(|request| request.url().ends_with("/redirect-forward"))
            .collect();
        assert_eq!(requests.len(), 2);
        for (hop, request) in requests.iter().enumerate() {
            let Some(response) = request.response() else {
                assert_eq!(kind, BrowserKind::Firefox);
                assert_eq!(hop, 0);
                assert!(
                    matches!(request.completion(), RequestCompletion::Unavailable(_)),
                    "missing native redirect completion must settle explicitly: {:?}",
                    request.snapshot()
                );
                continue;
            };
            response.finished().await.unwrap();
            assert_eq!(response.header_value("x-hop"), Some(hop.to_string()));
            assert_eq!(response.header_values("set-cookie").len(), 2);
            assert!(request.snapshot().response_headers_from_route);
        }
        assert_eq!(requests[0].redirected_to().unwrap().id(), requests[1].id());
        assert_eq!(
            requests[1].redirected_from().unwrap().id(),
            requests[0].id()
        );
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(executed, 2);
    }
    stop.abort();
}
