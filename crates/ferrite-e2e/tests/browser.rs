//! Browser-driven integration tests, parametrized over engines.
//!
//! Each test runs against every available engine (Chromium, Firefox) and
//! skips engines with no installed browser:
//!
//! ```bash
//! FERRITE_CHROMIUM_PATH=/tmp/chrome-headless-shell-linux64/chrome-headless-shell \
//!   cargo test -p ferrite-e2e --test browser
//! ```

use std::time::Duration;

use ferrite_e2e::{
    Browser, BrowserKind, LaunchOptions, LoadState, NavigationOptions, RouteRule, Runner,
    TestStatus,
};

const FIXTURE: &str = r#"<!doctype html><html><head><title>e2e fixture</title></head><body>
<h1 id="title">hello ferrite</h1>
<button id="inc">inc</button>
<span id="count">0</span>
<input id="name" />
<select id="pick"><option value="a">A</option><option value="b">B</option></select>
<input type="checkbox" id="agree" />
<script>
console.log('fixture loaded');
document.getElementById('inc').addEventListener('click', () => {
  const s = document.getElementById('count');
  s.textContent = String(Number(s.textContent) + 1);
});
</script>
</body></html>"#;

/// Spawn the fixture app; returns (base_url, shutdown).
async fn serve() -> (String, tokio::task::AbortHandle) {
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async { axum::response::Html(FIXTURE.to_string()) }),
        )
        .route(
            "/api/hi",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"real": true})) }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await });
    (url, task.abort_handle())
}

/// Launch one browser per available engine (skips missing engines loudly).
async fn browsers() -> Vec<(BrowserKind, Browser)> {
    let mut out = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let exe = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        };
        let Some(exe) = exe else {
            eprintln!("skipping {}: no executable found", kind.name());
            continue;
        };
        match Browser::launch(LaunchOptions::default().browser(kind).executable(exe)).await {
            Ok(browser) => out.push((kind, browser)),
            Err(error) => eprintln!("skipping {}: {error}", kind.name()),
        }
    }
    if out.is_empty() {
        eprintln!("skipping browser test: no chromium or firefox found");
    }
    out
}

#[tokio::test]
async fn navigation_title_and_content() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto_with_options(
            &base,
            NavigationOptions {
                wait_until: LoadState::NetworkIdle,
                timeout: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.title().await.unwrap(), "e2e fixture", "{tag}");
        assert!(
            page.url().await.unwrap().starts_with("http://127.0.0.1"),
            "{tag}"
        );
        assert!(
            page.content().await.unwrap().contains("hello ferrite"),
            "{tag}"
        );
        page.wait_for_url("127.0.0.1", Duration::from_secs(5))
            .await
            .unwrap();
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn click_and_selector_engines() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#inc").click().await.unwrap();
        page.locator("#inc").click().await.unwrap();
        page.locator("#count").expect_text("2").await.unwrap();

        // Alternate engines resolve the same DOM.
        page.locator("text=hello ferrite")
            .expect_visible()
            .await
            .unwrap();
        page.locator("xpath=//h1[@id='title']")
            .expect_contains_text("ferrite")
            .await
            .unwrap();
        page.locator("css=#count").expect_text("2").await.unwrap();
        page.locator("role=button[name=\"inc\"]")
            .click()
            .await
            .unwrap();
        page.locator("#count").expect_text("3").await.unwrap();

        assert_eq!(page.locator("button").count().await.unwrap(), 1, "{tag}");
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn form_fill_select_check() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#name").fill("ada").await.unwrap();
        page.locator("#name").expect().value("ada").await.unwrap();
        page.locator("#name").clear().await.unwrap();
        page.locator("#name").expect().value("").await.unwrap();

        page.locator("#pick").select_option("b").await.unwrap();
        page.locator("#pick").expect().value("b").await.unwrap();

        page.locator("#agree").check().await.unwrap();
        page.locator("#agree").expect().checked().await.unwrap();
        page.locator("#agree").uncheck().await.unwrap();
        page.locator("#agree").expect().unchecked().await.unwrap();

        let _ = tag;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn keyboard_types_and_deletes() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#name").fill("a").await.unwrap();
        page.locator("#name").press_sequentially("b").await.unwrap();
        page.locator("#name").expect().value("ab").await.unwrap();
        page.locator("#name").press("Backspace").await.unwrap();
        page.locator("#name").expect().value("a").await.unwrap();

        let _ = tag;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn screenshot_pdf_console_and_cookies() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        let png = page
            .screenshot(ferrite_e2e::ScreenshotOptions::default())
            .await
            .unwrap();
        assert!(
            png.len() > 100,
            "{tag}: screenshot too small: {}",
            png.len()
        );
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "{tag}");

        let pdf = page.pdf().await.unwrap();
        assert!(
            pdf.starts_with(b"%PDF"),
            "{tag}: not a pdf: {:?}",
            &pdf[..8.min(pdf.len())]
        );

        page.wait_for_timeout(Duration::from_millis(300))
            .await
            .unwrap();
        let console = page.console_messages();
        assert!(
            console.iter().any(|m| m.text.contains("fixture loaded")),
            "{tag}: console: {console:?}"
        );

        page.set_cookie("e2e", "yum").await.unwrap();
        let cookies = page.cookies().await.unwrap();
        assert!(
            cookies.iter().any(|c| c.name == "e2e" && c.value == "yum"),
            "{tag}: {cookies:?}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn request_routing_aborts_and_fulfills() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.route(vec![
            RouteRule::abort("**/api/slow"),
            RouteRule::fulfill("**/api/hi", 200, r#"{"mocked":true}"#, "application/json"),
        ])
        .await
        .unwrap();

        let aborted: String = page
            .evaluate("fetch('/api/slow').then(() => 'ok', () => 'failed')")
            .await
            .unwrap();
        assert_eq!(aborted, "failed", "{tag}");

        let mocked: serde_json::Value = page
            .evaluate("fetch('/api/hi').then(r => r.json())")
            .await
            .unwrap();
        assert_eq!(mocked, serde_json::json!({"mocked": true}), "{tag}");

        page.stop_routing().await;
        let real: serde_json::Value = page
            .evaluate("fetch('/api/hi').then(r => r.json())")
            .await
            .unwrap();
        assert_eq!(real, serde_json::json!({"real": true}), "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn contexts_isolate_cookies() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let one = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        let two = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        let page_one = one.new_page().await.unwrap();
        page_one.goto(&base).await.unwrap();
        page_one.set_cookie("ctx", "one").await.unwrap();

        let page_two = two.new_page().await.unwrap();
        page_two.goto(&base).await.unwrap();
        let cookies = page_two.cookies().await.unwrap();
        assert!(
            !cookies.iter().any(|c| c.name == "ctx"),
            "{tag}: contexts share cookies: {cookies:?}"
        );

        page_one.close().await.unwrap();
        page_two.close().await.unwrap();
        one.close().await.unwrap();
        two.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn dialogs_auto_handle() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.handle_dialogs(true).await.unwrap();
        let done: String = page.evaluate("alert('hi'); 'done'").await.unwrap();
        assert_eq!(done, "done", "{tag}");
        page.stop_dialog_handling().await;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn launch_user_agent_override() {
    // Firefox UA is launch-wide (profile pref); per-page override errors loudly.
    let Some(exe) = ferrite_e2e::find_firefox(None) else {
        eprintln!("skipping firefox UA test: no firefox found");
        return;
    };
    let mut options = LaunchOptions::default().browser(BrowserKind::Firefox);
    options.user_agent = Some("FerriteE2E/1.0".to_string());
    let browser = Browser::launch(options.executable(exe)).await.unwrap();
    let page = browser.new_page().await.unwrap();
    let agent: String = page.evaluate("navigator.userAgent").await.unwrap();
    assert!(agent.contains("FerriteE2E/1.0"), "{agent}");
    let error = page.set_user_agent("other").await.unwrap_err();
    assert!(error.to_string().contains("launch-wide"), "{error}");
    page.close().await.unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
async fn runner_passes_and_writes_artifacts() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let mut browser = browser;
        let (base, shutdown) = serve().await;
        browser.set_base_url(Some(base.clone()));
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-run-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let report = Runner::default()
            .workers(2)
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .run(
                &browser,
                vec![
                    ferrite_e2e::test("home title", |page| async move {
                        page.goto("/").await?;
                        page.expect_title("e2e fixture").await?;
                        Ok(())
                    }),
                    ferrite_e2e::test("counter clicks", |page| async move {
                        page.goto("/").await?;
                        page.locator("#inc").click().await?;
                        page.locator("#count").expect_text("1").await?;
                        Ok(())
                    }),
                ],
            )
            .await;
        assert_eq!(report.failed(), 0, "{tag}: {}", report.to_list());
        assert_eq!(report.passed(), 2, "{tag}");
        assert!(dir.join("home-title.json").is_file(), "{tag}");
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn runner_reports_failures() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let mut browser = browser;
        let (base, shutdown) = serve().await;
        browser.set_base_url(Some(base));
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-fail-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let report = Runner::default()
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .run(
                &browser,
                vec![ferrite_e2e::test("always fails", |page| async move {
                    page.goto("/").await?;
                    page.locator("#missing").expect_visible().await?;
                    Ok(())
                })],
            )
            .await;
        assert_eq!(report.failed(), 1, "{tag}");
        assert_eq!(report.results[0].status, TestStatus::Failed, "{tag}");
        assert!(!report.results[0].screenshots.is_empty(), "{tag}");
        assert!(
            report.results[0]
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("missing"),
            "{tag}"
        );
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
