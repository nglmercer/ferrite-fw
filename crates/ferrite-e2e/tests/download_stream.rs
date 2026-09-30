use ferrite_e2e::*;
use std::time::Duration;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn completed_file_cancellation_missing_failed_and_delete_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("binary.bin");
    tokio::fs::write(&path, [0, 255, 42, 0, 128]).await.unwrap();
    let download = Download::from_path(path.clone());
    assert_eq!(download.page_id(), None);
    assert_eq!(
        download
            .read_with_options(OperationOptions {
                timeout: Some(Duration::ZERO),
                cancellation: None
            })
            .await
            .unwrap(),
        vec![0, 255, 42, 0, 128]
    );
    let token = CancellationToken::new();
    token.cancel_with_reason("read canceled");
    let canceled = OperationOptions {
        timeout: Some(Duration::ZERO),
        cancellation: Some(token.clone()),
    };
    assert!(matches!(
        download.read_with_options(canceled.clone()).await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(matches!(
        download.create_read_stream_with_options(canceled).await,
        Err(E2eError::Cancelled(_))
    ));
    let mut stream = download.create_read_stream().await.unwrap();
    let token = CancellationToken::new();
    token.cancel();
    let mut bytes = Vec::new();
    assert!(matches!(
        token
            .run(async {
                stream.read_to_end(&mut bytes).await?;
                Ok(())
            })
            .await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(bytes.is_empty());
    drop(stream);
    let mut failed = download.clone();
    failed.failure = Some("interrupted".into());
    assert!(matches!(failed.read().await, Err(E2eError::Config(_))));
    assert!(matches!(
        failed.create_read_stream().await,
        Err(E2eError::Config(_))
    ));
    download.delete().await.unwrap();
    download.delete().await.unwrap();
    assert!(
        matches!(download.read().await,Err(E2eError::Io(error)) if error.kind()==std::io::ErrorKind::NotFound)
    );
    assert!(
        matches!(download.create_read_stream().await,Err(E2eError::Io(error)) if error.kind()==std::io::ErrorKind::NotFound)
    );
    assert!(
        matches!(
            Download::from_path(dir.path().into()).delete().await,
            Err(E2eError::Io(_))
        ),
        "directory deletion failure must propagate"
    );
    assert!(dir.path().is_dir());
}

#[tokio::test]
async fn native_large_binary_download_stream_and_owning_page() {
    use axum::{response::Html, routing::get, Router};
    let payload: Vec<u8> = (0..2 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let served = payload.clone();
    let app = Router::new()
        .route(
            "/",
            get(|| async { Html("<a id='download' href='/binary'>download</a>") }),
        )
        .route(
            "/binary",
            get(move || {
                let body = served.clone();
                async move {
                    (
                        [
                            ("content-type", "application/octet-stream"),
                            ("content-disposition", "attachment; filename=report.bin"),
                        ],
                        body,
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut executed = 0;
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(executable) = executable else {
            eprintln!("download stream browser absent: {}", kind.name());
            continue;
        };
        let dir = tempfile::tempdir().unwrap();
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable)
                .download_dir(dir.path()),
        )
        .await
        .expect("installed browser must launch");
        eprintln!(
            "download streams native {}: {}",
            kind.name(),
            browser.version().await.unwrap()
        );
        executed += 1;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        if kind == BrowserKind::Chromium {
            page.set_download_dir(dir.path()).await.unwrap();
        }
        page.evaluate_value("document.querySelector('#download').click(); true")
            .await
            .unwrap();
        let download = page
            .wait_for_download_file(dir.path(), Duration::from_secs(15))
            .await
            .unwrap();
        assert_eq!(download.page_id(), Some(page.target_id()));
        assert_eq!(download.suggested_filename, "report.bin");
        assert_eq!(download.read().await.unwrap(), payload);
        let mut stream = download.create_read_stream().await.unwrap();
        let mut buffer = [0u8; 8192];
        let mut offset = 0;
        loop {
            let count = stream.read(&mut buffer).await.unwrap();
            if count == 0 {
                break;
            }
            assert_eq!(&buffer[..count], &payload[offset..offset + count]);
            offset += count;
        }
        assert_eq!(offset, payload.len());
        drop(stream);
        page.close().await.unwrap();
        assert_eq!(download.clone().page_id(), Some(page.target_id()));
        assert_eq!(
            download.read().await.unwrap(),
            payload,
            "completed file I/O has no live page requirement"
        );
        download.delete().await.unwrap();
        download.delete().await.unwrap();
        browser.close().await.unwrap();
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(executed, 2);
    }
    server.abort();
}
