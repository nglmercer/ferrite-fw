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
struct Stop(tokio::task::AbortHandle);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn serve(app: axum::Router) -> (String, Stop) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, Stop(task.abort_handle()))
}
async fn preview(page: &Page, name: &str, index: usize) {
    if let Some(dir) = std::env::var_os("FERRITE_REPORT_PREVIEW_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}-{index}.png")),
            page.screenshot(Default::default()).await.unwrap(),
        )
        .unwrap();
    }
}
#[tokio::test]
async fn retry_network_survives_closed_pages_and_relocated_bundle() {
    let app = axum::Router::new()
        .route(
            "/page/{retry}",
            axum::routing::get(|| async { axum::response::Html("<title>network report</title>") }),
        )
        .route(
            "/redirect/{retry}",
            axum::routing::get(
                |axum::extract::Path(retry): axum::extract::Path<u32>| async move {
                    axum::response::Redirect::temporary(&format!("/final/{retry}"))
                },
            ),
        )
        .route("/final/{retry}", axum::routing::get(|| async { "success" }))
        .route(
            "/fail/{retry}",
            axum::routing::get(|| async {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "failure")
            }),
        );
    let (base, _server) = serve(app).await;
    let refused = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bad = format!("http://{}/refused", refused.local_addr().unwrap());
    drop(refused);
    for (index, browser) in browsers().await.into_iter().enumerate() {
        let source = tempfile::tempdir().unwrap();
        let base = base.clone();
        let bad = bad.clone();
        let report = Runner::from_config(&E2eConfig { screenshot: "on".into(), ..Default::default() })
            .workers(1).retries(1).test_timeout(Duration::from_secs(15)).list_progress(false)
            .output_dir(source.path().display().to_string())
            .run(&browser, vec![test_with_context("retry <script>window.pwned=1</script>", move |ctx| {
                let base = base.clone(); let bad = bad.clone();
                async move {
                    let retry = ctx.info.retry;
                    ctx.page.goto(&format!("{base}/page/{retry}")).await?;
                    ctx.page.evaluate_value(&format!("Promise.all([fetch('/redirect/{retry}'),fetch('/fail/{retry}'),fetch({}) .catch(()=>null)]).then(()=>true)", serde_json::to_string(&bad).unwrap())).await?;
                    let closed = ctx.context.new_page().await?;
                    closed.goto(&format!("{base}/final/{retry}?closed=1")).await?; closed.close().await?;
                    tokio::time::timeout(Duration::from_secs(5), async {
                        loop {
                            let summary = ctx.context.network_summary();
                            if summary.requests.iter().any(|r| matches!(r.completion, RequestCompletion::Failed(_))) && summary.requests.iter().filter(|r| r.url.contains("/final/")).count() >= 2 { break; }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    }).await.map_err(|_| E2eError::Expect("network diagnostics did not settle".into()))?;
                    ctx.page.evaluate_value("console.log('<img src=x onerror=window.pwned=1>');true").await?;
                    ctx.info.attach("state <&>", b"owned artifact", "text/plain")?;
                    if retry == 0 { return Err(E2eError::Expect("first attempt <&>".into())); }
                    Ok(())
                }
            })]).await;
        assert!(report.ok(), "{}", report.to_json());
        let result = &report.results[0];
        assert!(result.flaky);
        assert_eq!(result.attempt_results.len(), 2);
        for (retry, attempt) in result.attempt_results.iter().enumerate() {
            let summary = attempt.network.as_ref().unwrap();
            assert_eq!(summary.omitted_requests, 0);
            assert!(summary.requests.iter().any(|r| r.status == Some(500)));
            assert!(summary.requests.iter().any(|r| r.redirected_to.is_some()));
            assert!(summary
                .requests
                .iter()
                .any(|r| matches!(r.completion, RequestCompletion::Failed(_))));
            assert!(summary.requests.iter().any(|r| r.url.contains("closed=1")));
            assert!(summary
                .requests
                .iter()
                .filter(|r| r.url.contains("/page/"))
                .all(|r| r.url.ends_with(&retry.to_string())));
        }
        let export = tempfile::tempdir().unwrap();
        let bundle = report.write_bundle(export.path().join("original")).unwrap();
        assert!(bundle.artifact_count >= 4);
        std::fs::rename(export.path().join("original"), export.path().join("moved")).unwrap();
        let root = export.path().join("moved");
        let html = std::fs::read_to_string(root.join("report.html")).unwrap();
        let artifacts = root.join("artifacts");
        let app = axum::Router::new()
            .route(
                "/report",
                axum::routing::get(move || {
                    let html = html.clone();
                    async move { axum::response::Html(html) }
                }),
            )
            .route(
                "/artifacts/{name}",
                axum::routing::get(
                    move |axum::extract::Path(name): axum::extract::Path<String>| {
                        let artifacts = artifacts.clone();
                        async move { std::fs::read(artifacts.join(name)).unwrap() }
                    },
                ),
            );
        let (url, _report_server) = serve(app).await;
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 1440,
            height: 1100,
        })
        .await
        .unwrap();
        page.goto(&format!("{url}/report")).await.unwrap();
        page.evaluate_value("for(const d of document.querySelectorAll('details'))d.open=!d.querySelector('summary').textContent.startsWith('Effective');true")
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("typeof window.pwned").await.unwrap(),
            "undefined"
        );
        assert_eq!(
            page.evaluate_value("document.querySelectorAll('tr.test-result').length")
                .await
                .unwrap(),
            1
        );
        let links: Vec<String> = page
            .evaluate("[...document.querySelectorAll('a')].map(a=>a.href)")
            .await
            .unwrap();
        assert!(!links.is_empty());
        for link in links {
            assert!(reqwest::get(link).await.unwrap().status().is_success());
        }
        preview(&page, "expanded-network-report", index).await;
        page.evaluate_value("[...document.querySelectorAll('details')].find(d=>d.querySelector('summary').textContent.startsWith('Network:')).scrollIntoView();true").await.unwrap();
        preview(&page, "network-detail", index).await;
        browser.close().await.unwrap();
    }
}
fn synthetic(count: usize) -> TestReport {
    let mut report = TestReport::default();
    for index in 0..count {
        report.results.push(TestResult {
            name: format!("case {index:04} <&>"),
            status: if index % 2 == 0 {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            },
            attempts: 1,
            duration_ms: 1,
            error: None,
            screenshots: vec![],
            trace: None,
            video: None,
            project: Some(format!("project-{}", index % 3)),
            repeat_each_index: 0,
            annotations: vec![],
            attachments: vec![],
            flaky: index % 10 == 0,
            attempt_results: vec![],
        });
    }
    report
}
async fn visible(page: &Page) -> usize {
    page.evaluate("[...document.querySelectorAll('tr.test-result')].filter(r=>!r.hidden).length")
        .await
        .unwrap()
}
#[tokio::test]
async fn filters_pagination_empty_and_large_reports() {
    for (index, browser) in browsers().await.into_iter().enumerate() {
        let page = browser.new_page().await.unwrap();
        page.set_content(&synthetic(1500).to_html()).await.unwrap();
        assert_eq!(visible(&page).await, 50);
        page.locator("#next-page").click().await.unwrap();
        assert!(page
            .evaluate_value(
                "document.querySelector('#result-count').textContent.includes('page 2')"
            )
            .await
            .unwrap()
            .as_bool()
            .unwrap());
        page.locator("#test-search").fill("CASE 000").await.unwrap();
        assert_eq!(visible(&page).await, 10);
        page.locator("#status-filter")
            .select_option("failed")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 5);
        page.locator("#project-filter")
            .select_option("sproject-1")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 2);
        page.locator("#test-search")
            .fill("no matches")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 0);
        assert!(page.locator("#no-results").is_visible().await.unwrap());
        page.locator("#clear-filters").click().await.unwrap();
        assert_eq!(visible(&page).await, 50);
        for (size, expected) in [("100", 100), ("25", 25), ("50", 50)] {
            page.locator("#page-size")
                .select_option(size)
                .await
                .unwrap();
            assert_eq!(visible(&page).await, expected);
        }
        page.locator("#status-filter")
            .select_option("flaky")
            .await
            .unwrap();
        assert!(page
            .evaluate_value(
                "document.querySelector('#result-count').textContent.startsWith('150 of 1500')"
            )
            .await
            .unwrap()
            .as_bool()
            .unwrap());
        preview(&page, "large-filtered-report", index).await;
        let mut statuses = synthetic(5);
        statuses.results[0].status = TestStatus::Skipped;
        statuses.results[0].flaky = false;
        statuses.results[0].project = None;
        statuses.results[1].status = TestStatus::FailedExpected;
        statuses.results[1].project = Some("n".into());
        statuses.results[2].project = Some("<script>window.pwned=2</script>".into());
        statuses.results[4].flaky = true;
        page.set_content(&statuses.to_html()).await.unwrap();
        for (status, expected) in [
            ("skipped", 1),
            ("expected-failed", 1),
            ("failed", 1),
            ("passed", 2),
            ("flaky", 1),
        ] {
            page.locator("#status-filter")
                .select_option(status)
                .await
                .unwrap();
            assert_eq!(visible(&page).await, expected);
        }
        page.locator("#clear-filters").click().await.unwrap();
        page.locator("#project-filter")
            .select_option("n")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 1);
        page.locator("#project-filter")
            .select_option("sn")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 1);
        page.locator("#project-filter")
            .select_option("s<script>window.pwned=2</script>")
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 1);
        assert_eq!(
            page.evaluate_value("typeof window.pwned").await.unwrap(),
            "undefined"
        );
        page.set_content(&TestReport::default().to_html())
            .await
            .unwrap();
        assert_eq!(visible(&page).await, 0);
        assert!(page.locator("#no-results").is_visible().await.unwrap());
        preview(&page, "empty-report", index).await;
        browser.close().await.unwrap();
    }
}
