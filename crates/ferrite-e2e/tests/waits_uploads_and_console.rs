//! Native parity for URL/network matching, generated uploads and source-aware diagnostics.
use ferrite_e2e::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const WAIT: Duration = Duration::from_secs(5);

async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(executable) = executable {
            eprintln!("validating {kind:?}: {}", executable.display());
            result.push(
                Browser::launch(
                    LaunchOptions::default()
                        .browser(kind)
                        .executable(executable),
                )
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
async fn server() -> (String, tokio::task::AbortHandle) {
    use axum::{
        response::Html,
        routing::{get, post},
        Router,
    };
    let app = Router::new()
        .route("/", get(|| async { Html("<iframe name=child src='/child'></iframe><input id=file type=file><input id=multi type=file multiple>") }))
        .route("/child", get(|| async { Html("<p>child</p>") }))
        .route("/users/archive", get(|| async { "archive" }))
        .route("/users", get(|| async { "users" }).post(|| async { ([ ("x-reply", "ready") ], "posted") }))
        .route("/slow", get(|| async { tokio::time::sleep(Duration::from_millis(400)).await; "slow" }))
        .route("/stream", get(|| async {
            let body = axum::body::Body::from_stream(futures::stream::iter([0,1]).then(|n| async move {
                if n == 1 { tokio::time::sleep(Duration::from_millis(450)).await; }
                Ok::<_, std::io::Error>(if n == 0 { "first" } else { "last" })
            }));
            ([ ("x-reply", "stream") ], body)
        }))
        .route("/redirect", get(|| async { (axum::http::StatusCode::FOUND, [("location", "/users")], "redirect") }))
        .route("/upload", post(|body: axum::body::Bytes| async move { body }))
        .route("/console", get(|| async { Html("<script src='/console.js'></script><h1>Console</h1>") }))
        .route("/console.js", get(|| async { ([ ("content-type", "application/javascript") ], "console.log('native console');\nsetTimeout(() => { throw new Error('native exception'); }, 20);") }));
    use futures::StreamExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (base, task.abort_handle())
}
#[tokio::test]
async fn url_matchers_handle_exact_glob_regex_predicates_and_frame_history() {
    for mut browser in browsers().await {
        let (base, stop) = server().await;
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        page.wait_for_url_matching(&UrlMatcher::exact("/"), WAIT)
            .await
            .unwrap();
        page.wait_for_url_matching(&UrlMatcher::glob("http://**/").unwrap(), WAIT)
            .await
            .unwrap();
        let wrong = UrlMatcher::exact("/users");
        page.goto("/users/archive").await.unwrap();
        assert!(matches!(
            page.wait_for_url_matching(&wrong, Duration::from_millis(50))
                .await,
            Err(E2eError::Timeout(..))
        ));
        let matcher = UrlMatcher::glob("**/users?active=*").unwrap();
        let (wait, change) = tokio::join!(page.wait_for_url_matching(&matcher, WAIT), async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            page.evaluate_value("history.pushState({}, '', '/users?active=yes')")
                .await
        });
        wait.unwrap();
        change.unwrap();
        page.wait_for_url_matching(&UrlMatcher::regex(r"/users\?active=yes$").unwrap(), WAIT)
            .await
            .unwrap();
        let (wait, change) = tokio::join!(
            page.wait_for_url_where(|url| url.ends_with("#done"), WAIT),
            async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                page.evaluate_value("location.hash='done'").await
            }
        );
        wait.unwrap();
        change.unwrap();
        page.goto("/").await.unwrap();
        let frame = page.frame_by_url("/child").await.unwrap().unwrap();
        let frame_target = UrlMatcher::exact("/child?next=1");
        let (wait, change) =
            tokio::join!(frame.wait_for_url_matching(&frame_target, WAIT), async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                frame
                    .evaluate_value("history.pushState({}, '', '/child?next=1')")
                    .await
            });
        wait.unwrap();
        change.unwrap();
        frame
            .wait_for_url_where(|url| url.ends_with("?next=1"), WAIT)
            .await
            .unwrap();
        page.close().await.unwrap();
        browser.close().await.unwrap();
        stop.abort();
    }
}
#[tokio::test]
async fn network_predicates_observe_request_start_and_response_headers_without_stale_matches() {
    for mut browser in browsers().await {
        let (base, stop) = server().await;
        browser.set_base_url(Some(base.clone()));
        let page = browser.new_page().await.unwrap();
        page.goto("/").await.unwrap();
        let started = Instant::now();
        let slow = UrlMatcher::exact("/slow");
        let (request, trigger) = tokio::join!(
            page.wait_for_request_matching(&slow, WAIT),
            page.evaluate_value(
                "fetch('/slow').then(r=>r.text()).then(t=>window.slowDone=t); true"
            )
        );
        trigger.unwrap();
        let request = request.unwrap();
        assert_eq!(request.status, 0);
        assert_eq!(request.method, "GET");
        assert!(
            started.elapsed() < Duration::from_millis(350),
            "request wait did not resolve at start"
        );
        // This response belongs to a request that started BEFORE the response wait.
        let response = page.wait_for_response_matching(&slow, WAIT).await.unwrap();
        assert_eq!(request.request_id, response.request_id);
        assert_eq!(response.status, 200);
        // Existing completed traffic must not satisfy a newly armed wait.
        assert!(matches!(
            page.wait_for_response_matching(&slow, Duration::from_millis(60))
                .await,
            Err(E2eError::Timeout(..))
        ));
        let (request, response, trigger) = tokio::join!(
            page.wait_for_request_where(|r| r.method == "POST" && r.url.ends_with("/users") && r.headers.iter().any(|(k,v)| k.eq_ignore_ascii_case("x-input") && v == "yes"), WAIT),
            page.wait_for_response_async(|r| async move { tokio::time::sleep(Duration::from_millis(10)).await; Ok(r.method == "POST" && r.status == 200 && r.response_headers.iter().any(|(k,v)| k.eq_ignore_ascii_case("x-reply") && v == "ready")) }, WAIT),
            page.evaluate_value("fetch('/users/archive'); fetch('/users', {method:'POST',headers:{'x-input':'yes'},body:'payload'}); true")
        );
        trigger.unwrap();
        let request = request.unwrap();
        let response = response.unwrap();
        assert_eq!(request.request_id, response.request_id);
        let stream_target = UrlMatcher::glob("**/stream").unwrap();
        let (stream, trigger) = tokio::join!(page.wait_for_response_matching(&stream_target, WAIT), page.evaluate_value("window.streamDone=false;fetch('/stream').then(r=>r.text()).then(()=>window.streamDone=true);true"));
        trigger.unwrap();
        assert_eq!(stream.unwrap().status, 200);
        assert_eq!(
            page.evaluate_value("window.streamDone").await.unwrap(),
            false
        );
        let (redirect, trigger) = tokio::join!(
            page.wait_for_response_where(|r| r.url.ends_with("/redirect") && r.status == 302, WAIT),
            page.evaluate_value("fetch('/redirect');true")
        );
        trigger.unwrap();
        assert_eq!(redirect.unwrap().status, 302);
        page.close().await.unwrap();
        browser.close().await.unwrap();
        stop.abort();
    }
}
#[tokio::test]
async fn network_async_predicates_keep_timeouts_errors_cancellation_and_disposal() {
    for browser in browsers().await {
        let (base, stop) = server().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let (hung, trigger) = tokio::join!(
            page.wait_for_response_async(
                |_| std::future::pending::<E2eResult<bool>>(),
                Duration::from_millis(150)
            ),
            page.evaluate_value("fetch('/users');true")
        );
        trigger.unwrap();
        assert!(matches!(hung, Err(E2eError::Timeout(..))));
        let (error, trigger) = tokio::join!(
            page.wait_for_request_async(
                |_| async { Err(E2eError::Expect("predicate error".into())) },
                WAIT
            ),
            page.evaluate_value("fetch('/users');true")
        );
        trigger.unwrap();
        assert!(matches!(error, Err(E2eError::Expect(message)) if message == "predicate error"));
        let token = CancellationToken::new();
        let scoped = page.with_cancellation(token.clone());
        let (cancelled, ()) = tokio::join!(
            scoped.wait_for_response_async(
                |_| std::future::pending::<E2eResult<bool>>(),
                Duration::ZERO
            ),
            async {
                page.evaluate_value("fetch('/users');true").await.unwrap();
                tokio::time::sleep(Duration::from_millis(60)).await;
                token.cancel_with_reason("cancel pending predicate");
            }
        );
        assert!(matches!(cancelled, Err(E2eError::Cancelled(..))));
        let (disposed, close) = tokio::join!(
            page.wait_for_request_where(|_| false, Duration::ZERO),
            async {
                tokio::time::sleep(Duration::from_millis(40)).await;
                page.close().await
            }
        );
        close.unwrap();
        assert!(matches!(
            disposed,
            Err(E2eError::Cancelled(..)) | Err(E2eError::Disconnected(..))
        ));
        browser.close().await.unwrap();
        stop.abort();
    }
}
#[tokio::test]
async fn generated_uploads_preserve_binary_mime_names_events_and_http_form_data() {
    for browser in browsers().await {
        let (base, stop) = server().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.evaluate_value("window.events=[];document.querySelectorAll('input').forEach(i=>['input','change'].forEach(e=>i.addEventListener(e,()=>events.push(i.id+':'+e))));true").await.unwrap();
        let bytes = vec![0, 1, 255, 13, 10, 128];
        let payload = FilePayload::new("quoted \"é.bin", "application/x-ferrite", bytes.clone());
        page.set_input_file_payloads("#file", std::slice::from_ref(&payload))
            .await
            .unwrap();
        let metadata = page.evaluate_value("(async()=>{const f=document.querySelector('#file').files[0];return [f.name,f.type,[...new Uint8Array(await f.arrayBuffer())]]})()").await.unwrap();
        assert_eq!(metadata[0], payload.name);
        assert_eq!(metadata[1], payload.mime_type);
        assert_eq!(metadata[2], serde_json::json!(bytes));
        assert_eq!(
            page.evaluate_value("window.events").await.unwrap(),
            serde_json::json!(["file:input", "file:change"])
        );
        let posted = page.evaluate_value("(async()=>{const data=new FormData();data.append('file',document.querySelector('#file').files[0]);const r=await fetch('/upload',{method:'POST',body:data});return [...new Uint8Array(await r.arrayBuffer())]})()").await.unwrap();
        let posted: Vec<u8> = serde_json::from_value(posted).unwrap();
        assert!(posted.windows(bytes.len()).any(|window| window == bytes));
        assert!(String::from_utf8_lossy(&posted).contains("application/x-ferrite"));
        let files = [
            payload.clone(),
            FilePayload::new("empty.txt", "text/plain", Vec::new()),
        ];
        page.locator("#multi")
            .set_input_file_payloads(&files)
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("document.querySelector('#multi').files.length")
                .await
                .unwrap(),
            2
        );
        assert!(page
            .locator("#file")
            .with_timeout(Duration::from_millis(70))
            .set_input_file_payloads(&files)
            .await
            .unwrap_err()
            .to_string()
            .contains("multiple"));
        // Failed validation leaves the original files untouched.
        assert_eq!(
            page.evaluate_value("document.querySelector('#file').files[0].name")
                .await
                .unwrap(),
            payload.name
        );
        assert!(page
            .locator("#file")
            .set_input_file_payloads(&[FilePayload::new("", "text/plain", b"bad")])
            .await
            .is_err());
        page.locator("#multi")
            .set_input_file_payloads(&[])
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("document.querySelector('#multi').files.length")
                .await
                .unwrap(),
            0
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("disk.txt");
        std::fs::write(&path, "disk data").unwrap();
        page.locator("#file")
            .set_input_files(&[&path])
            .await
            .unwrap();
        assert_eq!(
            page.evaluate_value("document.querySelector('#file').files[0].name")
                .await
                .unwrap(),
            "disk.txt"
        );
        assert!(page
            .locator("#file")
            .set_input_files(&[dir.path()])
            .await
            .unwrap_err()
            .to_string()
            .contains("not a file"));
        page.close().await.unwrap();
        browser.close().await.unwrap();
        stop.abort();
    }
}
#[derive(Clone, Default)]
struct EndReports(Arc<Mutex<Vec<AttemptResult>>>);
impl Reporter for EndReports {
    fn on_test_end(&self, _: &AttemptInfo, result: &TestResult) {
        self.0
            .lock()
            .unwrap()
            .extend(result.attempt_results.clone());
    }
}
async fn await_console(context: &BrowserContext, needle: &str) -> E2eResult<()> {
    let started = Instant::now();
    while !context
        .console_messages()
        .iter()
        .any(|m| m.text.contains(needle))
    {
        if started.elapsed() > WAIT {
            return Err(E2eError::Expect(format!("missing console: {needle}")));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
}
#[tokio::test]
async fn console_metadata_survives_closed_pages_retries_cleanup_and_portable_reports() {
    for mut browser in browsers().await {
        let (base, stop) = server().await;
        browser.set_base_url(Some(base.clone()));
        let dir = tempfile::tempdir().unwrap();
        let exported = tempfile::tempdir().unwrap();
        let events = EndReports::default();
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let report = Runner::default()
            .workers(1)
            .retries(1)
            .list_progress(false)
            .output_dir(dir.path().display().to_string())
            .custom_reporter(events.clone())
            .after_each_with_context(ContextHook::new(|ctx| async move {
                ctx.page
                    .evaluate_value("console.log('cleanup console');true")
                    .await?;
                await_console(&ctx.context, "cleanup console").await
            }))
            .run(
                &browser,
                vec![test_with_context("console metadata", |ctx| async move {
                    ctx.page.goto("/console").await?;
                    await_console(&ctx.context, "native exception").await?;
                    let (popup, opened) = tokio::join!(
                        ctx.page.wait_for_popup(WAIT),
                        ctx.page.evaluate_value("window.open('about:blank');true")
                    );
                    opened?;
                    let other = popup?;
                    let other_id = other.target_id().to_owned();
                    other.goto("/console").await?;
                    other
                        .evaluate_value(&format!(
                            "console.log('closed page retry {} <script>');true",
                            ctx.info.retry
                        ))
                        .await?;
                    await_console(
                        &ctx.context,
                        &format!("closed page retry {}", ctx.info.retry),
                    )
                    .await?;
                    other.close().await?;
                    assert!(ctx
                        .context
                        .console_messages()
                        .iter()
                        .any(|m| m.page_id.as_deref() == Some(&other_id)));
                    // Page clearing must not erase the context-wide attempt archive.
                    ctx.page.clear_console_messages();
                    if ctx.info.retry == 0 {
                        Err(E2eError::Expect("retry once".into()))
                    } else {
                        Ok(())
                    }
                })],
            )
            .await;
        assert!(report.ok(), "{}", report.to_list());
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let result = &report.results[0];
        assert!(result.flaky);
        assert_eq!(result.attempt_results.len(), 2);
        assert_eq!(events.0.lock().unwrap().len(), 2);
        for (retry, attempt) in result.attempt_results.iter().enumerate() {
            let messages = &attempt.console;
            assert!(messages.iter().any(|m| m.text.contains("cleanup console")));
            assert!(messages
                .iter()
                .any(|m| m.text.contains(&format!("closed page retry {retry}"))));
            assert!(!messages
                .iter()
                .any(|m| m.text.contains(&format!("closed page retry {}", 1 - retry))));
            let pages: std::collections::HashSet<_> =
                messages.iter().filter_map(|m| m.page_id.clone()).collect();
            assert_eq!(pages.len(), 2);
            for message in messages.iter().filter(|m| {
                m.text.contains("native console") || m.text.contains("native exception")
            }) {
                let location = message.location.as_ref().expect("native script source");
                assert_eq!(location.url, format!("{base}console.js"));
                assert!(location.line <= 1);
                let timestamp = message.timestamp_ms.unwrap();
                assert!((before..=after).contains(&timestamp));
            }
            assert!(messages
                .iter()
                .any(|m| m.kind == "exception" && m.text.contains("native exception")));
            let trace: serde_json::Value =
                serde_json::from_slice(&std::fs::read(attempt.trace.as_ref().unwrap()).unwrap())
                    .unwrap();
            assert!(trace["console"]
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["text"].as_str().unwrap().contains("closed page retry")));
        }
        let bundle = report.write_bundle(exported.path()).unwrap();
        let roundtrip: TestReport =
            serde_json::from_slice(&std::fs::read(bundle.json).unwrap()).unwrap();
        assert_eq!(
            roundtrip.results[0].attempt_results[1].console.len(),
            result.attempt_results[1].console.len()
        );
        let html = std::fs::read_to_string(bundle.html).unwrap();
        assert!(html.contains("Console and page errors"));
        assert!(html.contains("console.js"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        browser.close().await.unwrap();
        stop.abort();
    }
}
