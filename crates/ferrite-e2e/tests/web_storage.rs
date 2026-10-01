use ferrite_e2e::*;

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
fn entry(name: &str, value: &str) -> StorageEntry {
    StorageEntry {
        name: name.into(),
        value: value.into(),
    }
}
fn pairs(entries: Vec<StorageEntry>) -> Vec<(String, String)> {
    entries.into_iter().map(|e| (e.name, e.value)).collect()
}
#[tokio::test]
async fn typed_storage_snapshots_bulk_errors_and_origin_lifecycle() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            axum::Router::new().fallback(|| async { "storage" }),
        )
        .await
        .unwrap();
    });
    for browser in browsers().await {
        let context = browser.new_context(Default::default()).await.unwrap();
        let page = context.new_page().await.unwrap();
        assert!(page.local_storage_items().await.is_err());
        assert!(page
            .session_storage_set_items(&[entry("opaque", "x")])
            .await
            .is_err());
        page.goto(&base).await.unwrap();
        assert!(page.local_storage_items().await.unwrap().is_empty());
        assert!(page.session_storage_items().await.unwrap().is_empty());
        let input = [
            entry("雪", "😺\n\""),
            entry("", ""),
            entry("__proto__", "safe"),
            entry("snow", "old"),
            entry("snow", "new"),
        ];
        page.local_storage_set_items(&input).await.unwrap();
        page.session_storage_set_items(&input).await.unwrap();
        let expected = vec![
            ("".into(), "".into()),
            ("__proto__".into(), "safe".into()),
            ("snow".into(), "new".into()),
            ("雪".into(), "😺\n\"".into()),
        ];
        let snapshot = page.local_storage_items().await.unwrap();
        assert_eq!(pairs(snapshot.clone()), expected);
        assert_eq!(pairs(page.session_storage_items().await.unwrap()), expected);
        page.local_storage_set_items(&[]).await.unwrap();
        page.local_storage_set_items(&[entry("snow", "changed")])
            .await
            .unwrap();
        assert_eq!(pairs(snapshot), expected); // owned snapshot does not change
        let peer = context.new_page().await.unwrap();
        peer.goto(&base).await.unwrap();
        assert_eq!(
            peer.local_storage_get("snow").await.unwrap().as_deref(),
            Some("changed")
        );
        assert!(peer.session_storage_items().await.unwrap().is_empty());
        page.goto(&format!("{base}/next")).await.unwrap();
        assert_eq!(
            page.session_storage_get("snow").await.unwrap().as_deref(),
            Some("new")
        );
        page.goto(&base.replace("127.0.0.1", "localhost"))
            .await
            .unwrap();
        assert!(page.local_storage_items().await.unwrap().is_empty());
        page.goto(&base).await.unwrap();
        // A native quota failure preserves preceding successful writes, not later ones.
        let huge = "x".repeat(16 * 1024 * 1024);
        assert!(page
            .local_storage_set_items(&[
                entry("before", "written"),
                entry("too-large", &huge),
                entry("after", "absent")
            ])
            .await
            .is_err());
        assert_eq!(
            page.local_storage_get("before").await.unwrap().as_deref(),
            Some("written")
        );
        assert!(page.local_storage_get("after").await.unwrap().is_none());
        assert!(page
            .session_storage_set_items(&[
                entry("before", "written"),
                entry("too-large", &huge),
                entry("after", "absent")
            ])
            .await
            .is_err());
        assert_eq!(
            page.session_storage_get("before").await.unwrap().as_deref(),
            Some("written")
        );
        assert!(page.session_storage_get("after").await.unwrap().is_none());
        let state = page.storage_state().await.unwrap();
        let restored = browser.new_context(Default::default()).await.unwrap();
        restored.apply_storage_state(&state).await.unwrap();
        let restored_page = restored.new_page().await.unwrap();
        restored_page.goto(&base).await.unwrap();
        assert_eq!(
            pairs(restored_page.local_storage_items().await.unwrap()),
            pairs(page.local_storage_items().await.unwrap())
        );
        assert!(restored_page
            .session_storage_items()
            .await
            .unwrap()
            .is_empty());
        restored.close().await.unwrap();
        page.close().await.unwrap();
        assert!(page.local_storage_items().await.is_err());
        assert!(page.session_storage_set_items(&input).await.is_err());
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
    server.abort();
}

#[tokio::test]
async fn storage_helpers_respect_attempt_cancellation() {
    for browser in browsers().await {
        for bulk in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let cancel = CancellationToken::new();
            let abort = cancel.clone();
            let report = Runner::from_config(&E2eConfig::default())
                .output_dir(root.path().display().to_string()).list_progress(false)
                .with_cancellation(cancel)
                .run(&browser, vec![test_with_context("storage cancellation", move |ctx| {
                    let abort = abort.clone();
                    async move {
                        // Delay native access, so cancellation occurs while the helper
                        // awaits the transport, rather than before the call is started.
                        let storage = if bulk { "sessionStorage" } else { "localStorage" };
                        ctx.page.evaluate_value(&format!("Object.defineProperty(window, '{storage}', {{get() {{ const end = performance.now() + 500; while (performance.now() < end) {{}} throw new Error('delayed storage'); }} }})")).await?;
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            abort.cancel();
                        });
                        if bulk {
                            ctx.page.session_storage_set_items(&[entry("late", "x")]).await?;
                        } else {
                            ctx.page.local_storage_items().await?;
                        }
                        Err(E2eError::Expect("storage unexpectedly completed".into()))
                    }
                })]).await;
            assert!(!report.ok());
            assert!(report.to_json().contains("cancel"), "{}", report.to_json());
        }
        browser.close().await.unwrap();
    }
}
