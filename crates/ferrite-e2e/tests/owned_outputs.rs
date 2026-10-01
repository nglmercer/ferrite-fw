use ferrite_e2e::*;
use std::{path::Path, time::Duration};
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
fn test_case(stamp: &'static str) -> Test {
    test_with_context("same name", move |ctx| async move {
        ctx.page
            .set_content(&format!("<h1>{stamp} retry {}</h1>", ctx.info.retry))
            .await?;
        ctx.info.attach("state", stamp.as_bytes(), "text/plain")?;
        std::fs::write(ctx.info.output_path("marker.txt")?, stamp)?;
        match_text_snapshot_with("baseline", "shared baseline", &ctx.info.snapshot_options())?;
        if ctx.info.retry == 0 {
            return Err(E2eError::Expect("first attempt".into()));
        }
        Ok(())
    })
}
#[tokio::test]
async fn attempts_and_repeat_runs_preserve_prior_owned_files_and_baselines() {
    for browser in browsers().await {
        let root = tempfile::tempdir().unwrap();
        let runner = Runner::from_config(&E2eConfig {
            screenshot: "on".into(),
            ..Default::default()
        })
        .workers(1)
        .retries(1)
        .test_timeout(Duration::from_secs(10))
        .list_progress(false)
        .output_dir(root.path().display().to_string());
        let first = runner.run(&browser, vec![test_case("first")]).await;
        assert!(first.ok(), "{}", first.to_json());
        let second = runner.run(&browser, vec![test_case("second")]).await;
        assert!(second.ok(), "{}", second.to_json());
        let mut directories = std::collections::HashSet::new();
        for (report, stamp) in [(&first, "first"), (&second, "second")] {
            let result = &report.results[0];
            assert_eq!(result.attempt_results.len(), 2);
            for attempt in &result.attempt_results {
                let settings = attempt.settings.as_ref().unwrap();
                let directory = Path::new(&settings.output_dir);
                assert!(directories.insert(settings.output_dir.clone()));
                assert!(directory.starts_with(root.path()));
                assert_eq!(
                    std::fs::read_to_string(directory.join("marker.txt")).unwrap(),
                    stamp
                );
                for path in attempt
                    .screenshots
                    .iter()
                    .chain(attempt.trace.iter())
                    .chain(attempt.attachments.iter().map(|a| &a.path))
                {
                    assert!(
                        Path::new(path).starts_with(directory),
                        "{path} outside {}",
                        directory.display()
                    );
                    assert!(Path::new(path).is_file());
                }
                let baseline = Path::new(&settings.snapshot_dir).join("baseline.snap");
                assert!(baseline.is_file());
                assert!(!baseline.starts_with(directory));
            }
            let exported = tempfile::tempdir().unwrap();
            report.write_bundle(exported.path()).unwrap();
        }
        browser.close().await.unwrap();
    }
}
