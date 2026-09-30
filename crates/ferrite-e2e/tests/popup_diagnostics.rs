//! Earliest native popup observations and attempt-owned startup history.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::Duration;

struct Stop(tokio::task::AbortHandle);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn fixture() -> (String, Stop) {
    let app=axum::Router::new()
        .route("/",axum::routing::get(||async{axum::response::Html("<!doctype html><title>opener</title>")}))
        .route("/ping",axum::routing::get(||async{"ok"}))
        .route("/popup/{case}",axum::routing::get(|axum::extract::Path(case):axum::extract::Path<String>|async move {
            let close=case.starts_with("close");
            let script=if close {
                format!("console.log('startup:'+{0});const request=new XMLHttpRequest();request.open('GET','/ping?case='+{0},false);request.send();window.close();",json!(case))
            } else {
                format!("console.log('startup:'+{0});fetch('/ping?case='+{0}).then(r=>r.text()).then(()=>window.ready=true);queueMicrotask(()=>{{throw new Error('startup-error')}});",json!(case))
            };
            axum::response::Html(format!("<!doctype html><title>popup</title><script>{script}</script>"))
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, Stop(task.abort_handle()))
}
async fn browsers(base: &str) -> Vec<Browser> {
    let mut browsers = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(executable) = executable else {
            assert!(std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_none());
            continue;
        };
        let mut browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        browser.set_base_url(Some(base.into()));
        eprintln!(
            "popup diagnostics {} {}",
            kind.name(),
            browser.version().await.unwrap()
        );
        browsers.push(browser);
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
async fn until(
    context: &BrowserContext,
    predicate: impl Fn(&PopupDiagnosticsHistory) -> bool,
) -> PopupDiagnosticsHistory {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let history = context.popup_diagnostics();
            if predicate(&history) {
                break history;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "missing popup diagnostic: {:?}",
            context.popup_diagnostics()
        )
    })
}
fn contains(entry: &PopupDiagnostics, case: &str) -> bool {
    entry
        .console
        .iter()
        .any(|message| message.text.contains(&format!("startup:{case}")))
}
fn reference(name: &str) -> Value {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/popup-reference.json"
    ))
    .unwrap();
    assert_eq!(corpus["playwright"], "1.63.0");
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["result"]
        .clone()
}

#[tokio::test]
async fn native_startup_logs_errors_requests_closure_and_single_forwarding() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let mut events = context.subscribe();
        let (popup, opened) = tokio::join!(
            page.wait_for_popup(Duration::from_secs(8)),
            page.evaluate_value("window.open('/popup/live');true")
        );
        opened.unwrap();
        let popup = popup.unwrap();
        popup
            .wait_for_function("window.ready===true", Duration::from_secs(5))
            .await
            .unwrap();
        let history = until(&context, |history| {
            history.entries.iter().any(|entry| {
                contains(entry, "live")
                    && entry.requests.iter().any(|request| {
                        request.recorded.url.contains("/ping?case=live")
                            && matches!(request.completion, RequestCompletion::Finished)
                    })
            })
        })
        .await;
        let entry = history
            .entries
            .iter()
            .find(|entry| entry.page_id == popup.target_id())
            .unwrap();
        assert_eq!(entry.opener_id, page.target_id());
        assert_eq!(entry.adoption, PopupAdoption::Adopted);
        assert!(!entry.truncated);
        assert_eq!(
            entry
                .console
                .iter()
                .filter(|message| message.text.contains("startup:live"))
                .count(),
            1
        );
        assert!(entry
            .console
            .iter()
            .all(|message| message.page_id.as_deref() == Some(popup.target_id())));
        let messages = context.console_messages();
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.text.contains("startup:live"))
                .count(),
            1
        );
        assert!(messages.iter().any(|message| message.kind == "exception"
            && message.text.contains("startup-error")
            && message.page_id.as_deref() == Some(popup.target_id())));
        assert_eq!(
            popup
                .console_messages()
                .iter()
                .filter(|message| message.text.contains("startup:live"))
                .count(),
            1
        );
        let mut count = 0;
        while let Ok(event) = events.try_recv() {
            if matches!(event,ContextEvent::PageEvent{event:PageEvent::Console(message),..} if message.text.contains("startup:live"))
            {
                count += 1;
            }
        }
        assert_eq!(count, 1, "context forwarding must happen once");
        popup.close().await.unwrap();
        until(&context, |history| {
            history
                .entries
                .iter()
                .any(|entry| entry.page_id == popup.target_id() && entry.closed)
        })
        .await;
        assert!(popup.is_closed());
        let history = context.popup_diagnostics();
        let entry = history
            .entries
            .iter()
            .find(|entry| entry.page_id == popup.target_id())
            .unwrap();
        let mut closed = 0;
        while let Ok(event) = events.try_recv() {
            if matches!(event,ContextEvent::PageEvent{page_id,event:PageEvent::Closed} if page_id==popup.target_id())
            {
                closed += 1;
            }
        }
        assert_eq!(
            json!({"console":entry.console.iter().filter(|message|message.text.contains("startup:live")).count(),"contextConsole":count,"errors":context.console_messages().iter().filter(|message|message.page_id.as_deref()==Some(popup.target_id())&&message.kind=="exception"&&message.text.contains("startup-error")).count(),"ping":entry.requests.iter().filter(|request|request.recorded.url.ends_with("/ping?case=live")).count(),"closed":closed,"opener":entry.opener_id==page.target_id()}),
            reference("live-startup")
        );
        let saved = context.console_messages().len();
        context.clear_popup_diagnostics();
        assert!(context.popup_diagnostics().entries.is_empty());
        assert_eq!(context.console_messages().len(), saved);
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_immediate_close_and_concurrent_popup_startups_keep_source_identity() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let mut events = context.subscribe();
        page.evaluate_value("for(let i=0;i<4;i++)window.open('/popup/close-'+i);true")
            .await
            .unwrap();
        let history = until(&context, |history| {
            (0..4).all(|i| {
                history
                    .entries
                    .iter()
                    .any(|entry| contains(entry, &format!("close-{i}")) && entry.closed)
            })
        })
        .await;
        let mut ids = std::collections::HashSet::new();
        let mut normalized = Vec::new();
        let expected = reference("concurrent-immediate-close");
        for i in 0..4 {
            let case = format!("close-{i}");
            let entry = history
                .entries
                .iter()
                .find(|entry| contains(entry, &case))
                .unwrap();
            assert!(ids.insert(entry.page_id.clone()));
            assert_eq!(entry.opener_id, page.target_id());
            assert_eq!(
                entry
                    .console
                    .iter()
                    .filter(|message| message.text.contains(&format!("startup:{case}")))
                    .count(),
                1
            );
            assert!(
                entry
                    .requests
                    .iter()
                    .any(|request| request.recorded.url.contains(&format!("/ping?case={case}"))),
                "{} immediate request missing: {entry:?}",
                browser.kind().name()
            );
            assert!(entry
                .requests
                .iter()
                .all(|request| request.page_id.as_deref() == Some(&entry.page_id)));
            assert!(!entry
                .requests
                .iter()
                .any(|request| matches!(request.completion, RequestCompletion::Pending)));
            assert!(matches!(
                entry.adoption,
                PopupAdoption::Adopted
                    | PopupAdoption::ClosedBeforeAdoption
                    | PopupAdoption::Failed
            ));
            normalized.push(json!({"case":case,"console":entry.console.iter().filter(|message|message.text.contains(&format!("startup:{case}"))).count(),"contextConsole":context.console_messages().iter().filter(|message|message.text.contains(&format!("startup:{case}"))).count(),"ping":entry.requests.iter().filter(|request|request.recorded.url.ends_with(&format!("/ping?case={case}"))).count(),"closed":u8::from(entry.closed),"opener":entry.opener_id==page.target_id()}));
        }
        assert_eq!(json!(normalized), expected);
        let mut closed = std::collections::HashMap::<String, usize>::new();
        tokio::time::timeout(Duration::from_secs(3), async {
            while closed.len() < 4 {
                if let ContextEvent::PageEvent {
                    page_id,
                    event: PageEvent::Closed,
                } = events.recv().await.unwrap()
                {
                    if ids.contains(&page_id) {
                        *closed.entry(page_id).or_default() += 1;
                    }
                }
            }
        })
        .await
        .unwrap();
        while let Ok(event) = events.try_recv() {
            if let ContextEvent::PageEvent {
                page_id,
                event: PageEvent::Closed,
            } = event
            {
                if ids.contains(&page_id) {
                    *closed.entry(page_id).or_default() += 1;
                }
            }
        }
        assert!(closed.values().all(|count| *count == 1));
        assert_eq!(
            context
                .console_messages()
                .iter()
                .filter(|message| message.text.contains("startup:close-"))
                .count(),
            4
        );
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_startup_diagnostics_survive_runner_retries_trace_and_json() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .retries(1)
            .test_timeout(Duration::from_secs(10))
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test_with_context("popup startup retry", |ctx| async move {
                    ctx.page.goto("/").await?;
                    ctx.page
                        .evaluate_value("window.open('/popup/close-retry');true")
                        .await?;
                    until(&ctx.context, |history| {
                        history
                            .entries
                            .iter()
                            .any(|entry| contains(entry, "close-retry") && entry.closed)
                    })
                    .await;
                    Err(E2eError::Expect("intentional retry".into()))
                })],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempt_results.len(), 2);
        let mut ids = std::collections::HashSet::new();
        for attempt in &report.results[0].attempt_results {
            let entry = attempt
                .popup_diagnostics
                .entries
                .iter()
                .find(|entry| contains(entry, "close-retry"))
                .unwrap();
            assert!(entry.closed);
            assert!(ids.insert(entry.page_id.clone()));
            assert!(!entry.requests.is_empty());
            assert!(attempt
                .console
                .iter()
                .any(|message| message.text.contains("startup:close-retry")
                    && message.page_id.as_deref() == Some(&entry.page_id)));
            let trace: Value = serde_json::from_str(
                &std::fs::read_to_string(attempt.trace.as_ref().unwrap()).unwrap(),
            )
            .unwrap();
            assert!(trace["popup_diagnostics"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|candidate| candidate["page_id"] == entry.page_id));
        }
        let encoded = serde_json::to_string(&report).unwrap();
        let decoded: TestReport = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            decoded.results[0].attempt_results[0]
                .popup_diagnostics
                .entries
                .len(),
            1
        );
        let html = report.to_html();
        assert!(html.contains("Popup startup diagnostics"));
        assert!(html.contains("/ping?case=close-retry"));
        if let Some(preview) = std::env::var_os("FERRITE_E2E_REPORT_PREVIEW_DIR") {
            report
                .write_bundle(std::path::PathBuf::from(preview).join(browser.kind().name()))
                .unwrap();
        }
        let mut legacy: Value = serde_json::from_str(&encoded).unwrap();
        legacy["results"][0]["attempt_results"][0]
            .as_object_mut()
            .unwrap()
            .remove("popup_diagnostics");
        let legacy: TestReport = serde_json::from_value(legacy).unwrap();
        assert!(legacy.results[0].attempt_results[0]
            .popup_diagnostics
            .entries
            .is_empty());
        let mut escaped = decoded;
        escaped.results[0].attempt_results[0]
            .popup_diagnostics
            .entries[0]
            .page_id = "<script>popup</script>".into();
        let html = escaped.to_html();
        assert!(!html.contains("<script>popup</script>"));
        assert!(html.contains("&lt;script&gt;popup&lt;/script&gt;"));
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_popup_capture_waits_cancel_dispose_and_disconnect_without_holding_owners() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let (popup, opened) = tokio::join!(
            page.wait_for_popup(Duration::from_secs(5)),
            page.evaluate_value("window.open('/popup/lifecycle');true")
        );
        opened.unwrap();
        let popup = popup.unwrap();
        popup
            .wait_for_function("window.ready===true", Duration::from_secs(5))
            .await
            .unwrap();
        let (event, trigger) = tokio::join!(
            popup.wait_for_event(PageEventKind::FrameAttached, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                popup
                    .evaluate_value("document.body.append(document.createElement('iframe'));true")
                    .await
            }
        );
        trigger.unwrap();
        assert!(
            matches!(event.unwrap(),PageEvent::FrameAttached(frame) if frame.page_id==popup.target_id())
        );
        let token = CancellationToken::new();
        let signal = token.clone();
        let (result, ()) = tokio::join!(
            popup.wait_for_event_with_options(
                PageEventKind::FrameDetached,
                OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: Some(token)
                }
            ),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                signal.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let (result, ()) = tokio::join!(
            popup.wait_for_event(PageEventKind::FrameDetached, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                context.clone().close().await.unwrap();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert!(context.popup_diagnostics().entries.iter().all(|entry| entry
            .requests
            .iter()
            .all(|request| !matches!(request.completion, RequestCompletion::Pending))));
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let (popup, opened) = tokio::join!(
            page.wait_for_popup(Duration::from_secs(5)),
            page.evaluate_value("window.open('/popup/disconnect');true")
        );
        opened.unwrap();
        let popup = popup.unwrap();
        popup
            .wait_for_function("window.ready===true", Duration::from_secs(5))
            .await
            .unwrap();
        let (result, context_result, ()) = tokio::join!(
            popup.wait_for_event(PageEventKind::FrameDetached, Duration::ZERO),
            context.wait_for_event(ContextEventKind::FrameDetached, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                if let Some(connection) = browser.cdp() {
                    connection.close();
                } else {
                    browser.bidi().unwrap().close();
                }
            }
        );
        assert!(matches!(result, Err(E2eError::Disconnected(_))));
        assert!(matches!(context_result, Err(E2eError::Disconnected(_))));
        assert!(context
            .popup_diagnostics()
            .entries
            .iter()
            .all(|entry| entry.adoption == PopupAdoption::Adopted
                && entry
                    .requests
                    .iter()
                    .all(|request| !matches!(request.completion, RequestCompletion::Pending))));
        browser.close().await.unwrap();
    }
}
