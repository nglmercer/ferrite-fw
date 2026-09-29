//! Demo e2e suite for `ferrite e2e`.
//!
//! Run from this directory:
//!
//! ```bash
//! ferrite e2e
//! ```

use ferrite_e2e::{test, Browser, LaunchOptions, Runner};

fn chromium_or_skip() -> Option<std::path::PathBuf> {
    let found = ferrite_e2e::find_chromium(None);
    if found.is_none() {
        eprintln!("skipping e2e demo: no chromium found");
    }
    found
}

#[tokio::test]
async fn demo_suite() {
    let Some(exe) = chromium_or_skip() else {
        return;
    };
    let mut browser = Browser::launch(LaunchOptions::default().executable(exe))
        .await
        .unwrap();
    if let Ok(base) = std::env::var("FERRITE_E2E_BASE_URL") {
        browser.set_base_url(Some(base));
    }
    let report = Runner::default()
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
                }),
            ],
        )
        .await;
    browser.close().await.unwrap();
    assert!(report.ok(), "{}", report.to_list());
}
