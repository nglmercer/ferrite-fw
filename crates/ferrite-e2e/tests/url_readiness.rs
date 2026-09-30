//! URL matching and document readiness share a single cancelable budget.
use ferrite_e2e::*;
use std::time::{Duration, Instant};
const WAIT: Duration = Duration::from_secs(4);

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
                "URL readiness native {}: {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            result.push(browser);
        } else {
            eprintln!("URL readiness browser absent: {}", kind.name());
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}

async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{response::Html, routing::get, Router};
    let app = Router::new()
        .route("/", get(|| async { Html("<iframe src='/done'></iframe><p>home</p>") }))
        .route("/doc", get(|| async { Html("<script defer src='/defer.js'></script><img src='/image.svg'><p>document</p>") }))
        .route("/defer.js", get(|| async { tokio::time::sleep(Duration::from_millis(220)).await; ([("content-type","application/javascript")],"window.deferred=true;") }))
        .route("/image.svg", get(|| async { tokio::time::sleep(Duration::from_millis(550)).await; ([("content-type","image/svg+xml")],"<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>") }))
        .route("/redirect", get(|| async { (axum::http::StatusCode::FOUND,[("location","/doc")],"redirect") }))
        .route("/done", get(|| async { Html("<p>done</p>") }))
        // A fresh URL keeps the replacement's load blocked even when /doc's
        // earlier image was cached by the browser.
        .route("/swap", get(|| async { Html("<script>setTimeout(()=>location.replace('/done'),40)</script><img src='/image.svg?swap=1'>") }))
        .route("/slow-header", get(|| async { tokio::time::sleep(Duration::from_millis(150)).await; Html("<script defer src='/defer.js'></script><img src='/image.svg'>") }))
        .route("/short", get(|| async { tokio::time::sleep(Duration::from_millis(5)).await; "short" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}
fn options(state: LoadState) -> UrlWaitOptions {
    UrlWaitOptions::default().wait_until(state).timeout(WAIT)
}

#[tokio::test]
async fn url_readiness_redirects_history_hash_and_frames() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        eprintln!("readiness case {}", browser.kind().name());
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let target = UrlMatcher::exact("/doc");
        let started = Instant::now();
        let (wait, navigation) = tokio::join!(
            page.wait_for_url_matching_with_options(&target, options(LoadState::Commit)),
            page.goto_with_options(
                "/redirect",
                NavigationOptions {
                    wait_until: LoadState::Commit,
                    timeout: Some(WAIT)
                }
            )
        );
        wait.unwrap();
        navigation.unwrap();
        assert_ne!(
            page.evaluate_value("document.readyState").await.unwrap(),
            "complete"
        );
        page.wait_for_url_matching_with_options(&target, options(LoadState::DomContentLoaded))
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert_eq!(
            page.evaluate_value("document.readyState").await.unwrap(),
            "interactive"
        );
        assert_eq!(page.evaluate_value("window.deferred").await.unwrap(), true);
        page.wait_for_url_matching_with_options(&target, options(LoadState::Load))
            .await
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(500));
        assert_eq!(
            page.evaluate_value("document.readyState").await.unwrap(),
            "complete"
        );
        page.evaluate_value(
            "history.pushState({},'', '/doc?history=1'); location.hash='ready'; true",
        )
        .await
        .unwrap();
        page.wait_for_url_where_with_options(
            |url| url.ends_with("?history=1#ready"),
            options(LoadState::Load),
        )
        .await
        .unwrap();
        page.wait_for_url_with_options("history=1#ready", options(LoadState::DomContentLoaded))
            .await
            .unwrap();
        // A matched document replaced before load must not settle on its old state.
        let either = UrlMatcher::glob("**/{swap,done}").unwrap();
        let (wait, navigation) = tokio::join!(
            page.wait_for_url_matching_with_options(&either, options(LoadState::Load)),
            page.goto_with_options(
                "/swap",
                NavigationOptions {
                    wait_until: LoadState::Commit,
                    timeout: Some(WAIT)
                }
            )
        );
        wait.unwrap();
        navigation.unwrap();
        assert!(page.url().await.unwrap().ends_with("/done"));
        page.goto("/").await.unwrap();
        let frame = page.frame_by_url("/done").await.unwrap().unwrap();
        let (wait, change) = tokio::join!(
            frame.wait_for_url_matching_with_options(&target, options(LoadState::Load)),
            async {
                page.evaluate_value("document.querySelector('iframe').src='/doc'; true")
                    .await
            }
        );
        wait.unwrap();
        change.unwrap();
        assert_eq!(
            frame.evaluate_value("document.readyState").await.unwrap(),
            "complete"
        );
        frame
            .wait_for_url_where_with_options(
                |url| url.ends_with("/doc"),
                options(LoadState::DomContentLoaded),
            )
            .await
            .unwrap();
        assert!(matches!(
            frame
                .wait_for_url_with_options("never", options(LoadState::NetworkIdle))
                .await,
            Err(E2eError::Config(_))
        ));
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn url_readiness_shared_budget_quiet_bursts_cancel_and_dispose() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        eprintln!("budget case {}", browser.kind().name());
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/done").await.unwrap();
        let before = page.timeout();
        let started = Instant::now();
        let slow = UrlMatcher::exact("/slow-header");
        let (wait, navigation) = tokio::join!(
            page.wait_for_url_matching_with_options(
                &slow,
                UrlWaitOptions::default().timeout(Duration::from_millis(300))
            ),
            page.goto_with_options(
                "/slow-header",
                NavigationOptions {
                    wait_until: LoadState::Commit,
                    timeout: Some(WAIT)
                }
            )
        );
        navigation.unwrap();
        assert!(matches!(wait, Err(E2eError::Timeout(300, _))));
        assert!(
            started.elapsed() < Duration::from_millis(430),
            "matching and readiness must not each get a fresh timeout"
        );
        assert_eq!(page.timeout(), before);
        page.goto("/done").await.unwrap();
        let started = Instant::now();
        let done = UrlMatcher::exact("/done");
        let (wait, burst) = tokio::join!(
            page.wait_for_url_matching_with_options(&done, options(LoadState::NetworkIdle)),
            async {
                tokio::time::sleep(Duration::from_millis(350)).await;
                page.evaluate_value("fetch('/short').then(r=>r.text())")
                    .await
            }
        );
        wait.unwrap();
        burst.unwrap();
        assert!(
            started.elapsed() >= Duration::from_millis(850),
            "a short request resets observed quiet time"
        );
        page.set_navigation_timeout(Duration::from_millis(75));
        assert!(matches!(
            page.wait_for_url_with_options("never", UrlWaitOptions::default())
                .await,
            Err(E2eError::Timeout(75, _))
        ));
        assert_eq!(page.timeout(), before);
        let token = CancellationToken::new();
        let (wait, ()) = tokio::join!(
            page.wait_for_url_where_with_options(
                |_| false,
                UrlWaitOptions::default()
                    .timeout(Duration::ZERO)
                    .cancellation(token.clone())
            ),
            async {
                tokio::time::sleep(Duration::from_millis(120)).await;
                token.cancel();
            }
        );
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        let (wait, closed) = tokio::join!(
            page.wait_for_url_with_options(
                "never",
                UrlWaitOptions::default().timeout(Duration::ZERO)
            ),
            async {
                tokio::time::sleep(Duration::from_millis(40)).await;
                page.close().await
            }
        );
        assert!(matches!(wait, Err(E2eError::Cancelled(_))));
        closed.unwrap();
        browser.close().await.unwrap();
    }
    stop.abort();
}

#[tokio::test]
async fn url_readiness_runner_enclosing_deadline_and_retry() {
    let (base, stop) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(600))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test(
                    "URL readiness enclosing deadline",
                    |page| async move {
                        page.goto("/done").await?;
                        page.wait_for_url_with_options(
                            "never",
                            UrlWaitOptions::default().timeout(Duration::ZERO),
                        )
                        .await
                    },
                )],
            )
            .await;
        assert_eq!(report.results[0].attempts, 2);
        assert_eq!(report.failed(), 1);
        assert!(report.results[0]
            .attempt_results
            .iter()
            .all(|a| !a.errors.is_empty()));
        browser.close().await.unwrap();
    }
    stop.abort();
}
