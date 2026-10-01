//! Scoped session ownership, native detach isolation and lifecycle failures.
use ferrite_e2e::*;
use serde_json::json;
use std::time::Duration;
async fn browser(kind: BrowserKind) -> Option<Browser> {
    let path = match kind {
        BrowserKind::Chromium => find_chromium(None),
        BrowserKind::Firefox => find_firefox(None),
    };
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert!(path.is_some());
    }
    match path {
        Some(path) => Some(
            Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .unwrap(),
        ),
        None => None,
    }
}
#[tokio::test]
async fn detach_isolates_clones_commands_events_and_other_pages() {
    let Some(browser) = browser(BrowserKind::Chromium).await else {
        return;
    };
    let page = browser.new_page().await.unwrap();
    let other = browser.new_page().await.unwrap();
    let first = page.new_cdp_session().await.unwrap();
    let second = page.new_cdp_session().await.unwrap();
    assert_ne!(first.id(), second.id());
    let clone = first.clone();
    let mut events = first.events();
    first.send("Runtime.enable", json!({})).await.unwrap();
    assert_eq!(
        events
            .next(OperationOptions::default())
            .await
            .unwrap()
            .method,
        "Runtime.executionContextCreated"
    );
    let pending = clone.send(
        "Runtime.evaluate",
        json!({"expression":"new Promise(()=>{})","awaitPromise":true}),
    );
    let detach = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        first.detach().await.unwrap();
    };
    let (result, ()) = tokio::join!(pending, detach);
    assert!(matches!(result, Err(E2eError::Cancelled(_))));
    assert!(clone.is_detached());
    first.detach().await.unwrap();
    clone.detach().await.unwrap();
    assert!(matches!(
        events.next(OperationOptions::default()).await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(matches!(
        clone.send("Runtime.enable", json!({})).await,
        Err(E2eError::Cancelled(_))
    ));
    assert_eq!(
        second
            .send(
                "Runtime.evaluate",
                json!({"expression":"6*7","returnByValue":true})
            )
            .await
            .unwrap()["result"]["value"],
        json!(42)
    );
    assert_eq!(page.evaluate_value("21*2").await.unwrap(), json!(42));
    assert_eq!(other.evaluate_value("21*2").await.unwrap(), json!(42));
    assert!(browser
        .cdp()
        .unwrap()
        .call(
            None,
            "Browser.getVersion",
            json!({}),
            Duration::from_secs(5)
        )
        .await
        .is_ok());
    browser.close().await.unwrap();
}
#[tokio::test]
async fn last_owner_drop_detaches_and_native_external_detach_settles_streams() {
    let Some(browser) = browser(BrowserKind::Chromium).await else {
        return;
    };
    let page = browser.new_page().await.unwrap();
    let session = page.new_cdp_session().await.unwrap();
    let id = session.id();
    let mut events = session.events();
    drop(session);
    assert!(matches!(
        events.next(OperationOptions::default()).await,
        Err(E2eError::Cancelled(_))
    ));
    // The queued detach shares the FIFO writer with this command.
    let result = browser
        .cdp()
        .unwrap()
        .call(
            Some(&id),
            "Runtime.enable",
            json!({}),
            Duration::from_secs(5),
        )
        .await;
    assert!(matches!(result, Err(E2eError::Cdp { .. })));
    let session = page.new_cdp_session().await.unwrap();
    let mut events = session.events();
    browser
        .cdp()
        .unwrap()
        .call(
            None,
            "Target.detachFromTarget",
            json!({"sessionId":session.id()}),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(matches!(
        events.next(OperationOptions::default()).await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(session.is_detached());
    session.detach().await.unwrap();
    assert_eq!(page.evaluate_value("1").await.unwrap(), json!(1));
    browser.close().await.unwrap();
}
#[tokio::test]
async fn operation_controls_context_disposal_and_disconnect_are_explicit() {
    let Some(browser) = browser(BrowserKind::Chromium).await else {
        return;
    };
    let context = browser
        .new_context(ContextOptions::default())
        .await
        .unwrap();
    let page = context.new_page().await.unwrap();
    let token = CancellationToken::new();
    token.cancel();
    assert!(matches!(
        page.new_cdp_session_with(OperationOptions {
            cancellation: Some(token.clone()),
            timeout: None
        })
        .await,
        Err(E2eError::Cancelled(_))
    ));
    let session = page
        .new_cdp_session_with(OperationOptions {
            timeout: Some(Duration::ZERO),
            cancellation: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        session
            .send_with(
                "Runtime.enable",
                json!({}),
                OperationOptions {
                    timeout: None,
                    cancellation: Some(token)
                }
            )
            .await,
        Err(E2eError::Cancelled(_))
    ));
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        session
            .detach_with(OperationOptions {
                timeout: None,
                cancellation: Some(cancelled)
            })
            .await,
        Err(E2eError::Cancelled(_))
    ));
    assert!(!session.is_detached());
    let mut events = session.events();
    assert!(matches!(
        events
            .next(OperationOptions {
                timeout: Some(Duration::from_millis(20)),
                cancellation: None
            })
            .await,
        Err(E2eError::Timeout(_, _))
    ));
    session.send("Runtime.enable", json!({})).await.unwrap();
    assert!(events
        .next(OperationOptions {
            timeout: Some(Duration::ZERO),
            cancellation: None
        })
        .await
        .is_ok());
    let wait = session.send(
        "Runtime.evaluate",
        json!({"expression":"new Promise(()=>{})","awaitPromise":true}),
    );
    let close = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        context.close().await.unwrap();
    };
    let (result, ()) = tokio::join!(wait, close);
    assert!(matches!(result, Err(E2eError::Cancelled(_))));
    assert!(matches!(
        session.send("Runtime.enable", json!({})).await,
        Err(E2eError::Cancelled(_))
    ));
    let page = browser.new_page().await.unwrap();
    let session = page.new_cdp_session().await.unwrap();
    let mut events = session.events();
    browser.close().await.unwrap();
    assert!(matches!(
        events.next(OperationOptions::default()).await,
        Err(E2eError::Disconnected(_))
    ));
}
#[tokio::test]
async fn firefox_rejects_scoped_cdp_sessions() {
    let Some(browser) = browser(BrowserKind::Firefox).await else {
        return;
    };
    let page = browser.new_page().await.unwrap();
    assert!(matches!(
        page.new_cdp_session().await,
        Err(E2eError::Config(_))
    ));
    browser.close().await.unwrap();
}

#[tokio::test]
async fn stream_lag_and_dropped_detach_waits_release_native_resources() {
    let Some(browser) = browser(BrowserKind::Chromium).await else {
        return;
    };
    let page = browser.new_page().await.unwrap();
    let session = page.new_cdp_session().await.unwrap();
    let mut events = session.events();
    session.send("Runtime.enable", json!({})).await.unwrap();
    session
        .send(
            "Runtime.evaluate",
            json!({"expression":"for(let i=0;i<300;i++)console.log(i)"}),
        )
        .await
        .unwrap();
    assert!(
        matches!(events.next(OperationOptions::default()).await,Err(E2eError::Config(message)) if message.contains("lost"))
    );
    assert!(matches!(
        events.next(OperationOptions::default()).await,
        Err(E2eError::Cancelled(_))
    ));
    let id = session.id();
    let clone = session.clone();
    let wait = tokio::spawn(async move { clone.detach().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !session.is_detached() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    wait.abort();
    let _ = wait.await;
    session.detach().await.unwrap();
    assert!(matches!(
        browser
            .cdp()
            .unwrap()
            .call(
                Some(&id),
                "Runtime.enable",
                json!({}),
                Duration::from_secs(5)
            )
            .await,
        Err(E2eError::Cdp { .. })
    ));
    let retry = page.new_cdp_session().await.unwrap();
    retry.send("Runtime.enable", json!({})).await.unwrap();
    retry.detach().await.unwrap();
    browser.close().await.unwrap();
}
