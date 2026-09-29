//! Demo e2e suite for `ferrite e2e`.
//!
//! Run from this directory:
//!
//! ```bash
//! ferrite e2e
//! ```

use ferrite_e2e::{test, test_with_context, Browser, BrowserKind, LaunchOptions, Runner};

fn browser_or_skip() -> Option<(BrowserKind, std::path::PathBuf)> {
    let name = std::env::var("FERRITE_E2E_BROWSER").unwrap_or_else(|_| "chromium".to_string());
    let kind = BrowserKind::parse(&name).unwrap_or(BrowserKind::Chromium);
    let found = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    };
    match found {
        Some(exe) => Some((kind, exe)),
        None => {
            eprintln!("skipping e2e demo: no {} found", kind.name());
            None
        }
    }
}

#[tokio::test]
async fn demo_suite() {
    let Some((kind, exe)) = browser_or_skip() else {
        return;
    };
    let mut browser = Browser::launch(LaunchOptions::default().browser(kind).executable(exe))
        .await
        .unwrap();
    if let Ok(base) = std::env::var("FERRITE_E2E_BASE_URL") {
        browser.set_base_url(Some(base));
    }
    // Opt-in recording: `ferrite e2e --video on` (off by default).
    let video = std::env::var("FERRITE_E2E_VIDEO")
        .ok()
        .and_then(|mode| ferrite_e2e::VideoMode::parse(&mode).ok())
        .unwrap_or(ferrite_e2e::VideoMode::Off);
    let report = Runner::default()
        .video_mode(video)
        .fixture(|| async { Ok::<_, ferrite_e2e::E2eError>("named-user".to_string()) })
        .run(
            &browser,
            vec![
                test("counter increments", |page| async move {
                    page.goto("/").await?;
                    page.expect_title("e2e demo — ferrite").await?;
                    page.locator("#inc").click().await?;
                    page.locator("#inc").click().await?;
                    page.locator("#count").expect_text("2").await?;
                    Ok(())
                }),
                test("greeting greets", |page| async move {
                    page.goto("/").await?;
                    page.locator("#name").fill("ada").await?;
                    page.locator("#greeting")
                        .expect_contains_text("hello ada")
                        .await?;
                    Ok(())
                })
                .annotate("area", "greeting"),
                test_with_context("context greets the fixture", |ctx| async move {
                    let name = ctx
                        .get::<String>()
                        .map(|name| name.as_str().to_string())
                        .unwrap_or_else(|| "ada".to_string());
                    ctx.goto("/").await?;
                    ctx.locator("#name").fill(&name).await?;
                    ctx.locator("#greeting")
                        .expect_contains_text("hello named-user")
                        .await?;
                    let title = ctx.title().await?;
                    ctx.info.attach("title", title.as_bytes(), "text/plain")?;
                    Ok(())
                }),
            ],
        )
        .await;
    browser.close().await.unwrap();
    assert!(report.ok(), "{}", report.to_list());
}
