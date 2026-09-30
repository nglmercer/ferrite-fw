//! Native structured diagnostics, bounded previews and owned retry artifacts.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::time::Duration;

const SCRIPT: &str = "const cyclic={answer:42};cyclic.self=cyclic; console.log('metadata', undefined, null, NaN, Infinity, -0, 12n, Symbol('mark'), function named(){}, cyclic, {array:[1,'x'],obj:{nested:true}}, new Error('console error')); function outer(){function inner(){const e=new TypeError('structured error');e.name='RenamedError';throw e;}inner();} setTimeout(outer,0); true;\n//# sourceURL=metadata-fixture.js";
const SECOND: Duration = Duration::from_secs(5);
struct Stop(tokio::task::AbortHandle);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn fixture() -> (String, Stop) {
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async {
                axum::response::Html("<!doctype html><title>structured console</title>")
            }),
        )
        .route(
            "/popup",
            axum::routing::get(|| async {
                axum::response::Html(format!("<!doctype html><script>{SCRIPT}</script>"))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base, Stop(task.abort_handle()))
}
async fn browsers(base: &str) -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let mut browser =
                Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                    .await
                    .unwrap();
            eprintln!(
                "structured console {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            browser.set_base_url(Some(base.into()));
            result.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
async fn messages(context: &BrowserContext) -> Vec<ConsoleMessage> {
    tokio::time::timeout(SECOND, async {
        loop {
            let messages = context.console_messages();
            if messages.iter().any(|message| {
                message.error.as_ref().is_some_and(|error| {
                    error
                        .description
                        .as_ref()
                        .is_some_and(|value| value.contains("structured error"))
                })
            }) {
                return messages;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "native structured error missing: {:?}",
            context.console_messages()
        )
    })
}
fn metadata(messages: &[ConsoleMessage]) -> &ConsoleMessage {
    messages
        .iter()
        .find(|message| {
            message.arguments.as_ref().is_some_and(|args| {
                args.values.first().is_some_and(|argument| {
                    argument.value == ConsoleArgumentValue::Json(json!("metadata"))
                })
            })
        })
        .unwrap()
}
fn exception(messages: &[ConsoleMessage]) -> &ConsoleMessage {
    messages
        .iter()
        .find(|message| {
            message.error.as_ref().is_some_and(|error| {
                error
                    .description
                    .as_ref()
                    .is_some_and(|value| value.contains("structured error"))
            })
        })
        .unwrap()
}
fn assert_native(message: &ConsoleMessage, kind: BrowserKind) {
    let info = message.error.as_ref().unwrap();
    assert!(info
        .frames
        .iter()
        .any(|frame| frame.function_name.as_deref() == Some("inner")));
    assert!(info
        .frames
        .iter()
        .any(|frame| frame.function_name.as_deref() == Some("outer")));
    if kind == BrowserKind::Chromium {
        assert_eq!(info.name.as_deref(), Some("RenamedError"));
        assert_eq!(info.class_name.as_deref(), Some("TypeError"));
        assert_eq!(info.message.as_deref(), Some("structured error"));
        assert_eq!(
            json!({"name":info.name,"message":info.message,
            "inner":info.frames.iter().any(|frame|frame.function_name.as_deref()==Some("inner")),
            "outer":info.frames.iter().any(|frame|frame.function_name.as_deref()==Some("outer"))}),
            reference("mutable-error-name")
        );
    } else {
        assert!(info.name.is_none() && info.message.is_none() && info.class_name.is_none());
        assert!(info.description.as_ref().unwrap().contains("RenamedError"));
    }
}
fn reference(name: &str) -> Value {
    let source: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/console-reference.json"
    ))
    .unwrap();
    assert_eq!(source["playwright"], "1.63.0");
    source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["result"]
        .clone()
}
fn primitives(arguments: &ConsoleArguments) -> Value {
    json!(arguments.values.iter().take(9).map(|argument| {
        match &argument.value {
            ConsoleArgumentValue::Json(Value::Null)=>json!({"kind":"null","value":null}),
            ConsoleArgumentValue::Json(value)=>json!({"kind":argument.kind,"value":value}),
            ConsoleArgumentValue::Unserializable(_) if argument.kind=="undefined"=>json!({"kind":"undefined"}),
            ConsoleArgumentValue::Unserializable(value)=>json!({"kind":argument.kind,"special":if argument.kind=="bigint" {value.trim_end_matches('n')} else {value.as_str()}}),
            _=>json!({"kind":argument.kind}),
        }
    }).collect::<Vec<_>>())
}
#[tokio::test]
async fn native_values_error_frames_page_context_and_trace_forward_once() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let mut events = context.subscribe();
        let (event, trigger) = tokio::join!(
            context.wait_for_event(ContextEventKind::PageError, SECOND),
            page.evaluate_value(SCRIPT)
        );
        trigger.unwrap();
        let ContextEvent::PageEvent {
            page_id,
            event: PageEvent::Console(message),
        } = event.unwrap()
        else {
            panic!("expected native page error")
        };
        assert_eq!(page_id, page.target_id());
        assert_native(&message, browser.kind());
        let history = messages(&context).await;
        let log = metadata(&history);
        let args = log.arguments.as_ref().unwrap();
        let actual = primitives(args);
        let expected = reference("primitive-arguments");
        // Actual Playwright 1.63 yields an undefined console handle for bigint.
        // Retain native data instead of reproducing that loss in Ferrite.
        assert_eq!(expected[6]["kind"], "undefined");
        assert_eq!(actual[6], json!({"kind":"bigint","special":"12"}));
        for index in [0, 1, 2, 3, 4, 5, 7, 8] {
            assert_eq!(actual[index], expected[index]);
        }
        assert_eq!(args.values.len(), 12);
        assert_eq!(
            args.values[1].value,
            ConsoleArgumentValue::Unserializable("undefined".into())
        );
        assert_eq!(
            args.values[2].value,
            ConsoleArgumentValue::Json(Value::Null)
        );
        assert_eq!(
            args.values[3].value,
            ConsoleArgumentValue::Unserializable("NaN".into())
        );
        assert_eq!(
            args.values[4].value,
            ConsoleArgumentValue::Unserializable("Infinity".into())
        );
        assert_eq!(
            args.values[5].value,
            ConsoleArgumentValue::Unserializable("-0".into())
        );
        assert!(matches!(
            args.values[9].value,
            ConsoleArgumentValue::Preview(_)
        ));
        let page_messages = page.console_messages();
        assert_eq!(
            serde_json::to_value(metadata(&page_messages)).unwrap(),
            serde_json::to_value(log).unwrap()
        );
        assert_eq!(
            serde_json::to_value(exception(&page_messages)).unwrap(),
            serde_json::to_value(exception(&history)).unwrap()
        );
        let trace = page.trace();
        assert!(trace.iter().any(|entry| entry
            .console
            .as_ref()
            .is_some_and(|message| message.arguments.as_ref() == Some(args))));
        let mut forwarded = 0;
        while let Ok(event) = events.try_recv() {
            if matches!(event,ContextEvent::PageEvent{event:PageEvent::Console(message),..} if message.arguments.as_ref().is_some_and(|args|args.values.first().is_some_and(|argument|argument.value==ConsoleArgumentValue::Json(json!("metadata")))))
            {
                forwarded += 1;
            }
        }
        assert_eq!(forwarded, 1);
        page.close().await.unwrap();
        context.clone().close().await.unwrap();
        assert_eq!(
            serde_json::to_value(metadata(&context.console_messages())).unwrap(),
            serde_json::to_value(log).unwrap()
        );
        browser.close().await.unwrap();
    }
}
#[tokio::test]
async fn native_preview_caps_and_remote_references_are_explicit_after_disposal() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        page.evaluate_value("console.log('bounded',...Array(90).fill('😀'.repeat(5000)));true")
            .await
            .unwrap();
        let bounded = tokio::time::timeout(SECOND, async {
            loop {
                if let Some(message) = context.console_messages().into_iter().find(|message| {
                    message.arguments.as_ref().is_some_and(|args| {
                        args.values.first().is_some_and(|argument| {
                            argument.value == ConsoleArgumentValue::Json(json!("bounded"))
                        })
                    })
                }) {
                    break message;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let args = bounded.arguments.as_ref().unwrap();
        assert!(args.truncated && args.dropped_arguments >= 27);
        assert!(args.values.iter().any(|argument| argument.truncated));
        assert!(serde_json::to_vec(args).unwrap().len() <= 32 * 1024);
        page.evaluate_value(SCRIPT).await.unwrap();
        let history = messages(&context).await;
        let log = metadata(&history);
        let encoded = serde_json::to_string(log).unwrap();
        assert!(
            !encoded.contains("\"objectId\"")
                && !encoded.contains("\"internalId\"")
                && !encoded.contains("\"handle\"")
        );
        let decoded: ConsoleMessage = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.arguments, log.arguments);
        assert_native(exception(&history), browser.kind());
        context.close().await.unwrap();
        browser.close().await.unwrap();
        assert!(serde_json::to_string(&history)
            .unwrap()
            .contains("structured error"));
    }
}
#[tokio::test]
async fn native_popup_retry_metadata_survives_trace_json_reports_and_old_schema() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let kind = browser.kind();
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .retries(1)
            .test_timeout(Duration::from_secs(10))
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test_with_context(
                    "structured popup retry",
                    move |ctx| async move {
                        ctx.page.goto("/").await?;
                        let (popup, trigger) = tokio::join!(
                            ctx.page.wait_for_popup(SECOND),
                            ctx.page.evaluate_value("window.open('/popup');true")
                        );
                        trigger?;
                        let popup = popup?;
                        let history = messages(&ctx.context).await;
                        assert_native(exception(&history), kind);
                        popup.close().await?;
                        Err(E2eError::Expect("intentional structured retry".into()))
                    },
                )],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempt_results.len(), 2);
        let mut ids = std::collections::HashSet::new();
        for attempt in &report.results[0].attempt_results {
            let error = exception(&attempt.console);
            assert_native(error, browser.kind());
            assert!(ids.insert(error.page_id.clone().unwrap()));
            let popup = attempt
                .popup_diagnostics
                .entries
                .iter()
                .find(|popup| Some(&popup.page_id) == error.page_id.as_ref())
                .unwrap();
            assert_eq!(
                serde_json::to_value(exception(&popup.console)).unwrap(),
                serde_json::to_value(error).unwrap()
            );
            let trace: Value = serde_json::from_str(
                &std::fs::read_to_string(attempt.trace.as_ref().unwrap()).unwrap(),
            )
            .unwrap();
            assert!(trace["console"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| *value == serde_json::to_value(error).unwrap()));
            assert!(trace["popup_diagnostics"]["entries"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["console"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value["error"].is_object())));
        }
        let encoded = serde_json::to_string(&report).unwrap();
        let decoded: TestReport = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            serde_json::to_value(exception(&decoded.results[0].attempt_results[0].console))
                .unwrap(),
            serde_json::to_value(exception(&report.results[0].attempt_results[0].console)).unwrap()
        );
        let html = report.to_html();
        assert!(
            html.contains("Structured page error")
                && html.contains("Console arguments:")
                && html.contains("metadata-fixture.js")
        );
        if let Some(preview) = std::env::var_os("FERRITE_E2E_REPORT_PREVIEW_DIR") {
            report
                .write_bundle(std::path::PathBuf::from(preview).join(browser.kind().name()))
                .unwrap();
        }
        let mut escaped = decoded;
        let error = escaped.results[0].attempt_results[0]
            .console
            .iter_mut()
            .find(|message| message.error.is_some())
            .unwrap()
            .error
            .as_mut()
            .unwrap();
        error.name = Some("<script>bad-name</script>".into());
        error.frames[0].function_name = Some("<img src=x onerror=bad()>".into());
        let html = escaped.to_html();
        assert!(
            !html.contains("<script>bad-name</script>")
                && html.contains("&lt;script&gt;bad-name&lt;/script&gt;")
        );
        assert!(!html.contains("<img src=x onerror=bad()>"));
        let mut legacy: Value = serde_json::from_str(&encoded).unwrap();
        for attempt in legacy["results"][0]["attempt_results"]
            .as_array_mut()
            .unwrap()
        {
            for message in attempt["console"].as_array_mut().unwrap() {
                let object = message.as_object_mut().unwrap();
                object.remove("arguments");
                object.remove("error");
            }
        }
        let legacy: TestReport = serde_json::from_value(legacy).unwrap();
        assert!(legacy.results[0].attempt_results[0]
            .console
            .iter()
            .all(|message| message.arguments.is_none() && message.error.is_none()));
        browser.close().await.unwrap();
    }
}
