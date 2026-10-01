//! Real Chromium navigation/source/reset coverage and explicit Firefox limits.
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
    use axum::{response::Html, routing::get, Router};
    let app = Router::new()
        .route("/first",get(|| async { Html("<link rel=stylesheet href='/first.css'><script src='/first.js'></script><div class=first>first</div>") }))
        .route("/second",get(|| async { Html("<link rel=stylesheet href='/second.css'><script src='/second.js'></script><div class=second>second</div>") }))
        .route("/first.js",get(|| async { ([("Content-Type","application/javascript")],"function FirstMarker(flag){return flag ? 1 : 2}; FirstMarker(true);") }))
        .route("/second.js",get(|| async { ([("Content-Type","application/javascript")],"function SecondMarker(flag){return flag ? 3 : 4}; SecondMarker(true);") }))
        .route("/first.css",get(|| async { ([("Content-Type","text/css")],".first{color:red}.unused-first{color:blue}") }))
        .route("/second.css",get(|| async { ([("Content-Type","text/css")],".second{color:green}.unused-second{color:blue}") }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
    .abort_handle();
    (base, task)
}
#[tokio::test]
async fn coverage_navigation_sources_anonymous_and_restart() {
    let (base, server) = server().await;
    for mut browser in browsers().await {
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        let coverage = page.coverage();
        if browser.kind() == BrowserKind::Firefox {
            assert!(matches!(
                coverage
                    .start_js_coverage_with(JsCoverageOptions::default())
                    .await,
                Err(E2eError::Config(_))
            ));
            assert!(matches!(
                coverage
                    .start_css_coverage_with(CssCoverageOptions::default())
                    .await,
                Err(E2eError::Config(_))
            ));
            browser.close().await.unwrap();
            continue;
        }
        for reset in [true, false] {
            for include in [true, false] {
                coverage
                    .start_js_coverage_with(JsCoverageOptions {
                        reset_on_navigation: reset,
                        include_source: include,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                coverage
                    .start_css_coverage_with(CssCoverageOptions {
                        reset_on_navigation: reset,
                        include_source: include,
                        ..Default::default()
                    })
                    .await
                    .unwrap();
                assert!(coverage.start_js_coverage().await.is_err());
                assert!(coverage.start_css_coverage().await.is_err());
                page.goto("/first").await.unwrap();
                page.evaluate_value("getComputedStyle(document.querySelector('.first')).color")
                    .await
                    .unwrap();
                page.goto("/second").await.unwrap();
                page.evaluate_value("getComputedStyle(document.querySelector('.second')).color")
                    .await
                    .unwrap();
                let js = coverage.stop_js_coverage().await.unwrap();
                let css = coverage.stop_css_coverage().await.unwrap();
                let second = js
                    .iter()
                    .find(|entry| entry.url.ends_with("/second.js"))
                    .expect("second native JS entry");
                let style = css
                    .iter()
                    .find(|entry| {
                        entry.url.ends_with("/second.css")
                            && entry.ranges.iter().any(|range| range.count > 0)
                    })
                    .expect("second native CSS entry");
                eprintln!(
                    "reset={reset} include={include}: js={:?} css={:?}",
                    js.iter().map(|e| &e.url).collect::<Vec<_>>(),
                    css.iter().map(|e| &e.url).collect::<Vec<_>>()
                );
                if reset {
                    assert!(!js.iter().any(|entry| entry.url.ends_with("/first.js")));
                    assert!(!css.iter().any(|entry| entry.url.ends_with("/first.css")));
                }
                if !reset {
                    let first_style = css
                        .iter()
                        .find(|entry| entry.url.ends_with("/first.css"))
                        .expect("retained previous stylesheet");
                    if include {
                        assert!(first_style.source.contains(".first"));
                        assert_eq!(first_style.source_status, CoverageSourceStatus::Included);
                    }
                }
                for (source, status, marker) in [
                    (&second.source, &second.source_status, "SecondMarker"),
                    (&style.source, &style.source_status, ".second"),
                ] {
                    if include {
                        assert_eq!(*status, CoverageSourceStatus::Included);
                        assert!(source.contains(marker));
                    } else {
                        assert_eq!(*status, CoverageSourceStatus::Omitted);
                        assert!(source.is_empty());
                    }
                }
                assert!(second
                    .functions
                    .iter()
                    .flat_map(|f| &f.ranges)
                    .any(|r| r.count == 0));
                assert!(style.ranges.iter().any(|r| r.count > 0));
                assert!(coverage.stop_js_coverage().await.is_err());
                assert!(coverage.stop_css_coverage().await.is_err());
            }
        }
        for anonymous in [false, true] {
            coverage
                .start_js_coverage_with(JsCoverageOptions {
                    report_anonymous_scripts: anonymous,
                    ..Default::default()
                })
                .await
                .unwrap();
            page.evaluate_value("eval('window.AnonymousMarker = 42'); true")
                .await
                .unwrap();
            let entries = coverage.stop_js_coverage().await.unwrap();
            assert_eq!(
                entries
                    .iter()
                    .any(|entry| entry.url.is_empty() && entry.source.contains("AnonymousMarker")),
                anonymous
            );
        }
        coverage.start_js_coverage().await.unwrap();
        let oversized = format!(
            "/*{}*/window.LargeMarker=1; //# sourceURL=large.js",
            "x".repeat(1024 * 1024)
        );
        page.evaluate_value(&oversized).await.unwrap();
        let large = coverage.stop_js_coverage().await.unwrap();
        let large = large.iter().find(|entry| entry.url == "large.js").unwrap();
        assert_eq!(
            large.source_status,
            CoverageSourceStatus::Truncated { limit: 1024 * 1024 }
        );
        assert!(large.source.is_empty());
        coverage.start_js_coverage().await.unwrap();
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            coverage
                .stop_js_coverage_with_options(OperationOptions {
                    timeout: None,
                    cancellation: Some(token)
                })
                .await,
            Err(E2eError::Cancelled(_))
        ));
        page.evaluate_value("window.AfterCancelledStop=1; //# sourceURL=after-stop.js")
            .await
            .unwrap();
        assert!(coverage
            .stop_js_coverage()
            .await
            .unwrap()
            .iter()
            .any(|entry| entry.url == "after-stop.js"));
        coverage.start_js_coverage().await.unwrap();
        coverage.start_css_coverage().await.unwrap();
        page.close().await.unwrap();
        assert!(coverage.stop_js_coverage().await.is_err());
        assert!(coverage.stop_css_coverage().await.is_err());
        browser.close().await.unwrap();
    }
    server.abort();
}
#[tokio::test]
async fn coverage_caller_cancellation_and_zero_deadline() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        if browser.kind() == BrowserKind::Chromium {
            let token = CancellationToken::new();
            token.cancel();
            assert!(matches!(
                page.coverage()
                    .start_js_coverage_with(JsCoverageOptions {
                        operation: OperationOptions {
                            timeout: Some(Duration::ZERO),
                            cancellation: Some(token)
                        },
                        ..Default::default()
                    })
                    .await,
                Err(E2eError::Cancelled(_))
            ));
            page.coverage()
                .start_js_coverage_with(JsCoverageOptions {
                    operation: OperationOptions {
                        timeout: Some(Duration::ZERO),
                        cancellation: None,
                    },
                    ..Default::default()
                })
                .await
                .unwrap();
            page.evaluate_value("window.ZeroMarker=1; //# sourceURL=zero.js")
                .await
                .unwrap();
            let entries = page
                .coverage()
                .stop_js_coverage_with_options(OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: None,
                })
                .await
                .unwrap();
            assert!(entries.iter().any(|entry| entry.url == "zero.js"));
            for css in [false, true] {
                let timeout = OperationOptions {
                    timeout: Some(Duration::from_nanos(1)),
                    cancellation: None,
                };
                let result = if css {
                    page.coverage()
                        .start_css_coverage_with(CssCoverageOptions {
                            operation: timeout,
                            ..Default::default()
                        })
                        .await
                } else {
                    page.coverage()
                        .start_js_coverage_with(JsCoverageOptions {
                            operation: timeout,
                            ..Default::default()
                        })
                        .await
                };
                assert!(matches!(result, Err(E2eError::Timeout(_, _))));
                // A timed-out setup can be restarted; native dirty domains are reconciled.
                if css {
                    page.coverage().start_css_coverage().await.unwrap();
                    page.coverage().stop_css_coverage().await.unwrap();
                } else {
                    page.coverage().start_js_coverage().await.unwrap();
                    page.coverage().stop_js_coverage().await.unwrap();
                }
            }
            let context = browser
                .new_context(ContextOptions::default())
                .await
                .unwrap();
            let owned = context.new_page().await.unwrap();
            owned.coverage().start_js_coverage().await.unwrap();
            owned.coverage().start_css_coverage().await.unwrap();
            context.close().await.unwrap();
            assert!(matches!(
                owned.coverage().stop_js_coverage().await,
                Err(E2eError::Cancelled(_))
            ));
            assert!(matches!(
                owned.coverage().stop_css_coverage().await,
                Err(E2eError::Cancelled(_))
            ));
            page.coverage().start_js_coverage().await.unwrap();
            page.coverage().start_css_coverage().await.unwrap();
            browser.cdp().unwrap().close();
            assert!(matches!(
                page.coverage().stop_js_coverage().await,
                Err(E2eError::Disconnected(_))
            ));
            assert!(matches!(
                page.coverage().stop_css_coverage().await,
                Err(E2eError::Disconnected(_))
            ));
        }
        browser.close().await.unwrap();
    }
}
