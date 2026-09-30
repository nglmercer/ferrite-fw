//! Native frame identity, readiness, dialog closure and event-wait lifecycle.
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
    let app = axum::Router::new().fallback(axum::routing::get(|| async {
        axum::response::Html("<!doctype html><title>events</title><body>fixture</body>")
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
            "lifecycle events {}: {}",
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
async fn collect(
    rx: &mut tokio::sync::broadcast::Receiver<PageEvent>,
    all: &mut Vec<PageEvent>,
    predicate: impl Fn(&[PageEvent]) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate(all) {
            all.push(
                rx.recv()
                    .await
                    .expect("native lifecycle event stream must not lag"),
            );
        }
    })
    .await
    .unwrap_or_else(|_| panic!("missing native lifecycle event; received {all:?}"));
}
fn navigated<'a>(events: &'a [PageEvent], suffix: &str) -> Option<&'a FrameEvent> {
    events.iter().find_map(|event| match event {
        PageEvent::FrameNavigated(frame)
            if frame
                .url
                .as_deref()
                .is_some_and(|url| url.ends_with(suffix)) =>
        {
            Some(frame)
        }
        _ => None,
    })
}
fn detached(events: &[PageEvent], id: &str) -> bool {
    events
        .iter()
        .any(|event| matches!(event,PageEvent::FrameDetached(frame) if frame.frame_id==id))
}
fn lifecycle(event: &PageEvent) -> Option<Value> {
    let data = match event {
        PageEvent::FrameAttached(frame)
        | PageEvent::FrameNavigated(frame)
        | PageEvent::FrameDetached(frame)
        | PageEvent::DomContentLoaded(frame)
        | PageEvent::Load(frame) => json!(frame),
        PageEvent::DialogClosed(info) => json!(info),
        _ => return None,
    };
    Some(json!({"kind":format!("{:?}",event.kind()),"data":data}))
}
fn reference(name: &str) -> Value {
    let data: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/event-reference.json"
    ))
    .unwrap();
    assert_eq!(data["playwright"], "1.63.0");
    data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["result"]
        .clone()
}
fn frame_summary(events: &[PageEvent], base: &str, main: &str) -> Value {
    let mut ids = vec![main.to_owned()];
    let mut rows = Vec::new();
    for event in events {
        let (kind, frame) = match event {
            PageEvent::FrameAttached(frame) => ("attached", frame),
            PageEvent::FrameNavigated(frame) => ("navigated", frame),
            PageEvent::FrameDetached(frame) => ("detached", frame),
            PageEvent::DomContentLoaded(frame) => ("dom", frame),
            PageEvent::Load(frame) => ("load", frame),
            _ => continue,
        };
        if kind == "attached" && !ids.contains(&frame.frame_id) {
            ids.push(frame.frame_id.clone());
        }
        rows.push((kind, frame));
    }
    let label = |id: &str| {
        let index = ids
            .iter()
            .position(|known| known == id)
            .expect("every observed frame is attached or is the main frame");
        if index == 0 {
            "main".into()
        } else {
            format!("f{index}")
        }
    };
    json!(ids.iter().map(|id|{
        let rows:Vec<_>=rows.iter().filter(|(_,frame)|&frame.frame_id==id).collect();
        let parent=rows.iter().find_map(|(_,frame)|frame.parent_frame_id.as_deref()).map(label);
        json!({"frame":label(id),"parent":parent,"attached":rows.iter().filter(|(kind,_)|*kind=="attached").count(),"detached":rows.iter().filter(|(kind,_)|*kind=="detached").count(),"navigations":rows.iter().filter(|(kind,_)|*kind=="navigated").filter_map(|(_,frame)|frame.url.as_deref().and_then(|url|url.strip_prefix(base))).collect::<Vec<_>>(),"ready":rows.iter().filter(|(kind,_)|*kind=="dom"||*kind=="load").map(|(kind,_)|*kind).collect::<Vec<_>>()})
    }).collect::<Vec<_>>())
}

#[tokio::test]
async fn native_frames_readiness_history_and_context_forwarding_keep_identity() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        let main = page.main_frame().await.unwrap().id().to_owned();
        let mut rx = page.subscribe();
        let mut ctx = context.subscribe();
        let mut events = Vec::new();
        page.goto("/main").await.unwrap();
        collect(&mut rx,&mut events,|events|events.iter().any(|event|matches!(event,PageEvent::Load(frame) if frame.url.as_deref().is_some_and(|url|url.ends_with("/main"))))).await;
        let root = navigated(&events, "/main").expect("native main commit");
        assert_eq!(root.frame_id, main);
        assert!(root.is_main_frame && root.parent_frame_id.is_none());
        let ready:Vec<_>=events.iter().filter(|event|matches!(event,PageEvent::FrameNavigated(frame)|PageEvent::DomContentLoaded(frame)|PageEvent::Load(frame) if frame.is_main_frame)).map(PageEvent::kind).collect();
        assert_eq!(
            ready,
            [
                PageEventKind::FrameNavigated,
                PageEventKind::DomContentLoaded,
                PageEventKind::Load
            ]
        );
        page.evaluate_value("new Promise(resolve=>{const frame=document.createElement('iframe');frame.id='outer';frame.onload=()=>resolve(true);frame.src='/frame?phase=1';document.body.append(frame)})").await.unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/frame?phase=1").is_some()
        })
        .await;
        let outer = navigated(&events, "/frame?phase=1").unwrap().clone();
        assert_eq!(outer.parent_frame_id.as_deref(), Some(main.as_str()));
        assert!(!outer.is_main_frame && !outer.same_document);
        page.evaluate_value("new Promise(resolve=>{const doc=document.querySelector('#outer').contentDocument;const frame=doc.createElement('iframe');frame.onload=()=>resolve(true);frame.src='/nested';doc.body.append(frame)})").await.unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/nested").is_some()
        })
        .await;
        let nested = navigated(&events, "/nested").unwrap().clone();
        assert_eq!(
            nested.parent_frame_id.as_deref(),
            Some(outer.frame_id.as_str())
        );
        page.evaluate_value("new Promise(resolve=>{const frame=document.querySelector('#outer');frame.onload=()=>resolve(true);frame.src='/frame?phase=2'})").await.unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/frame?phase=2").is_some() && detached(events, &nested.frame_id)
        })
        .await;
        let second = navigated(&events, "/frame?phase=2").unwrap();
        assert_eq!(second.frame_id, outer.frame_id);
        if let (Some(before), Some(after)) = (&outer.document_id, &second.document_id) {
            assert_ne!(before, after);
        }
        page.evaluate_value("location.hash='fragment';true")
            .await
            .unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/main#fragment").is_some()
        })
        .await;
        page.evaluate_value("history.pushState({},'', '/main?history=1');true")
            .await
            .unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/main?history=1").is_some()
        })
        .await;
        page.evaluate_value("history.pushState({},'', '/main?history=1');true")
            .await
            .unwrap();
        collect(&mut rx,&mut events,|events|events.iter().filter(|event|matches!(event,PageEvent::FrameNavigated(frame) if frame.url.as_deref().is_some_and(|url|url.ends_with("/main?history=1")))).count()==2).await;
        for event in &events {
            if let PageEvent::FrameNavigated(frame) = event {
                if frame
                    .url
                    .as_deref()
                    .is_some_and(|url| url.contains("fragment") || url.contains("history=1"))
                {
                    assert!(frame.same_document);
                    assert_eq!(frame.frame_id, main);
                }
            }
        }
        page.evaluate_value("document.querySelector('#outer').remove();true")
            .await
            .unwrap();
        collect(&mut rx, &mut events, |events| {
            detached(events, &outer.frame_id)
        })
        .await;
        page.evaluate_value("new Promise(resolve=>{const frame=document.createElement('iframe');frame.id='outer';frame.onload=()=>resolve(true);frame.src='/frame?phase=3';document.body.append(frame)})").await.unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/frame?phase=3").is_some()
        })
        .await;
        let replacement = navigated(&events, "/frame?phase=3").unwrap().clone();
        assert_ne!(replacement.frame_id, outer.frame_id);
        page.evaluate_value("new Promise(resolve=>{const doc=document.querySelector('#outer').contentDocument;const frame=doc.createElement('iframe');frame.onload=()=>resolve(true);frame.src='/nested?phase=2';doc.body.append(frame)})").await.unwrap();
        collect(&mut rx, &mut events, |events| {
            navigated(events, "/nested?phase=2").is_some()
        })
        .await;
        let leaf = navigated(&events, "/nested?phase=2").unwrap().clone();
        page.evaluate_value("document.querySelector('#outer').remove();true")
            .await
            .unwrap();
        collect(&mut rx, &mut events, |events| {
            detached(events, &replacement.frame_id) && detached(events, &leaf.frame_id)
        })
        .await;
        for id in [
            &outer.frame_id,
            &nested.frame_id,
            &replacement.frame_id,
            &leaf.frame_id,
        ] {
            let trace:Vec<_>=events.iter().filter(|event|matches!(event,PageEvent::FrameAttached(frame)|PageEvent::FrameNavigated(frame)|PageEvent::FrameDetached(frame) if &frame.frame_id==id)).collect();
            assert_eq!(trace.first().unwrap().kind(), PageEventKind::FrameAttached);
            assert_eq!(trace.last().unwrap().kind(), PageEventKind::FrameDetached);
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.kind() == PageEventKind::FrameAttached)
                    .count(),
                1
            );
            assert_eq!(
                trace
                    .iter()
                    .filter(|event| event.kind() == PageEventKind::FrameDetached)
                    .count(),
                1
            );
        }
        assert_eq!(
            frame_summary(&events, &base, &main),
            reference("frame-lifecycle"),
            "{}",
            browser.kind().name()
        );
        let page_trace: Vec<_> = events.iter().filter_map(lifecycle).collect();
        let mut context_trace = Vec::new();
        while let Ok(event) = ctx.try_recv() {
            if let ContextEvent::PageEvent { page_id, event } = event {
                assert_eq!(page_id, page.target_id());
                if let Some(event) = lifecycle(&event) {
                    context_trace.push(event);
                }
            }
        }
        assert_eq!(
            context_trace, page_trace,
            "page events must forward exactly once"
        );
        for event in &page_trace {
            assert_eq!(event["data"]["page_id"], page.target_id());
        }
        if browser.kind() == BrowserKind::Firefox {
            for event in &page_trace {
                assert!(event["data"]["name"].is_null());
            }
        }
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_dialog_closure_and_repeated_set_content_readiness() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/main").await.unwrap();
        let mut rx = page.subscribe();
        let mut events = Vec::new();
        for index in 1..=2 {
            page.set_content(&format!(
                "<!doctype html><title>content {index}</title><body>replacement</body>"
            ))
            .await
            .unwrap();
            collect(&mut rx, &mut events, |events| {
                events
                    .iter()
                    .filter(|event| matches!(event, PageEvent::Load(_)))
                    .count()
                    == index
            })
            .await;
        }
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, PageEvent::DomContentLoaded(_)))
                .count(),
            2
        );
        let ready: Vec<_> = events
            .iter()
            .filter(|event| matches!(event, PageEvent::DomContentLoaded(_) | PageEvent::Load(_)))
            .map(PageEvent::kind)
            .collect();
        assert_eq!(
            ready,
            [
                PageEventKind::DomContentLoaded,
                PageEventKind::Load,
                PageEventKind::DomContentLoaded,
                PageEventKind::Load
            ]
        );
        let main = page.main_frame().await.unwrap().id().to_owned();
        let same_main = events.iter().all(|event| match event {
            PageEvent::DomContentLoaded(frame) | PageEvent::Load(frame) => frame.frame_id == main,
            _ => true,
        });
        let ready: Vec<_> = ready
            .iter()
            .map(|kind| {
                if *kind == PageEventKind::DomContentLoaded {
                    "dom"
                } else {
                    "load"
                }
            })
            .collect();
        assert_eq!(
            json!({"sameMain":same_main,"ready":ready}),
            reference("repeated-readiness")
        );
        let mut context_events = context.subscribe();
        page.handle_dialogs_with_prompt(true, "fixture")
            .await
            .unwrap();
        let (closed, prompt) = tokio::join!(
            page.wait_for_event(PageEventKind::DialogClosed, Duration::from_secs(3)),
            page.evaluate::<String>("prompt('accept prompt','default')")
        );
        assert_eq!(prompt.unwrap(), "fixture");
        let PageEvent::DialogClosed(closed) = closed.unwrap() else {
            panic!()
        };
        assert_eq!(closed.page_id, page.target_id());
        assert_eq!(closed.accepted, Some(true));
        assert_eq!(closed.user_text.as_deref(), Some("fixture"));
        let expected = reference("prompt-accepted");
        assert_eq!(json!(closed.accepted), expected["accepted"]);
        assert_eq!(json!(closed.user_text), expected["userText"]);
        assert_eq!(
            json!(closed.page_id == page.target_id()),
            expected["pageOwned"]
        );
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        assert_eq!(
            json!(events
                .iter()
                .filter(|event| matches!(event, PageEvent::DialogClosed(_)))
                .count()),
            expected["pageClosed"]
        );
        let mut forwarded = Vec::new();
        while let Ok(event) = context_events.try_recv() {
            if let ContextEvent::PageEvent {
                event: PageEvent::DialogClosed(info),
                ..
            } = event
            {
                forwarded.push(info);
            }
        }
        assert_eq!(json!(forwarded.len()), expected["contextClosed"]);
        assert_eq!(forwarded[0], closed);
        if browser.kind() == BrowserKind::Firefox {
            assert_eq!(closed.frame_id.as_deref(), Some(page.target_id()));
            assert_eq!(closed.dialog_type.as_deref(), Some("prompt"));
        } else {
            assert!(closed.frame_id.is_none() && closed.dialog_type.is_none());
        }
        page.stop_dialog_handling().await;
        page.handle_dialogs(false).await.unwrap();
        let (closed, confirm) = tokio::join!(
            page.wait_for_event(PageEventKind::DialogClosed, Duration::from_secs(3)),
            page.evaluate::<bool>("confirm('dismiss confirm')")
        );
        assert!(!confirm.unwrap());
        let PageEvent::DialogClosed(closed) = closed.unwrap() else {
            panic!()
        };
        let expected = reference("confirm-dismissed");
        assert_eq!(json!(closed.accepted), expected["accepted"]);
        assert_eq!(
            json!(closed.page_id == page.target_id()),
            expected["pageOwned"]
        );
        let mut page_closed = 0;
        while let Ok(event) = rx.try_recv() {
            if matches!(event, PageEvent::DialogClosed(_)) {
                page_closed += 1;
            }
        }
        let mut context_closed = 0;
        while let Ok(event) = context_events.try_recv() {
            if let ContextEvent::PageEvent {
                event: PageEvent::DialogClosed(info),
                ..
            } = event
            {
                assert_eq!(info, closed);
                context_closed += 1;
            }
        }
        assert_eq!(json!(page_closed), expected["pageClosed"]);
        assert_eq!(json!(context_closed), expected["contextClosed"]);
        if let Some(kind) = &closed.dialog_type {
            assert_eq!(json!(kind), expected["type"]);
        }
        page.stop_dialog_handling().await;
        context.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_lifecycle_waits_zero_cancellation_disposal_retries_and_disconnect() {
    let (base, _stop) = fixture().await;
    for browser in browsers(&base).await {
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/main").await.unwrap();
        let (event, trigger) = tokio::join!(
            page.wait_for_event(PageEventKind::FrameAttached, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                page.evaluate_value(
                    "const f=document.createElement('iframe');document.body.append(f);true",
                )
                .await
            }
        );
        trigger.unwrap();
        assert!(matches!(event.unwrap(), PageEvent::FrameAttached(_)));
        context.set_default_timeout(Duration::from_millis(20));
        assert!(matches!(
            context
                .wait_for_event_with_options(
                    ContextEventKind::FrameAttached,
                    OperationOptions::default()
                )
                .await,
            Err(E2eError::Timeout(_, _))
        ));
        context.set_default_timeout(Duration::ZERO);
        assert!(matches!(
            page.wait_for_event(PageEventKind::FrameAttached, Duration::from_millis(20))
                .await,
            Err(E2eError::Timeout(_, _))
        ));
        let cancel = CancellationToken::new();
        let signal = cancel.clone();
        let (result, ()) = tokio::join!(
            page.wait_for_event_with_options(
                PageEventKind::FrameDetached,
                OperationOptions {
                    timeout: Some(Duration::ZERO),
                    cancellation: Some(cancel)
                }
            ),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                signal.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let (result, ()) = tokio::join!(
            page.wait_for_event(PageEventKind::Load, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                context.clone().close().await.unwrap();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let output = tempfile::tempdir().unwrap();
        let report = Runner::default()
            .test_timeout(Duration::from_millis(850))
            .retries(1)
            .output_dir(output.path().display().to_string())
            .run(
                &browser,
                vec![test(
                    "native event wait enclosing deadline",
                    |page| async move {
                        page.goto("/main").await?;
                        page.wait_for_event(PageEventKind::FrameDetached, Duration::ZERO)
                            .await
                            .map(|_| ())
                    },
                )],
            )
            .await;
        assert_eq!(report.failed(), 1);
        assert_eq!(report.results[0].attempts, 2);
        assert!(report.results[0]
            .attempt_results
            .iter()
            .all(|attempt| attempt
                .errors
                .iter()
                .any(|error| error.message.contains("timed out"))));
        let context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        let page = context.new_page().await.unwrap();
        page.goto("/main").await.unwrap();
        let empty_context = browser
            .new_context(ContextOptions::default())
            .await
            .unwrap();
        assert!(empty_context.pages().is_empty());
        let (result, context_result, empty_result, ()) = tokio::join!(
            page.wait_for_event(PageEventKind::FrameAttached, Duration::ZERO),
            context.wait_for_event(ContextEventKind::FrameAttached, Duration::ZERO),
            empty_context.wait_for_event(ContextEventKind::FrameAttached, Duration::ZERO),
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
        assert!(matches!(empty_result, Err(E2eError::Disconnected(_))));
        browser.close().await.unwrap();
    }
}
