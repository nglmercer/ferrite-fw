//! Browser-driven integration tests, parametrized over engines.
//!
//! Each test runs against every available engine (Chromium, Firefox) and
//! skips engines with no installed browser:
//!
//! ```bash
//! FERRITE_CHROMIUM_PATH=/tmp/chrome-headless-shell-linux64/chrome-headless-shell \
//!   cargo test -p ferrite-e2e --test browser
//! ```

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ferrite_e2e::{
    describe, test, Browser, BrowserKind, ColorScheme, Cookie, DeviceDescriptor, DialogDecision,
    E2eError, LaunchOptions, LoadState, NavigationOptions, Page, PageEvent, PageEventKind,
    RecordedRequest, ReducedMotion, RouteAction, RouteInfo, RouteRule, Runner, ScreenshotOptions,
    TestStatus, Timeout, TracingOptions, VideoMode, VideoOptions, WebSocketDirection,
};

const FIXTURE: &str = r#"<!doctype html><html><head><title>e2e fixture</title></head><body>
<h1 id="title">hello ferrite</h1>
<button id="inc">inc</button>
<span id="count">0</span>
<input id="name" />
<input type="file" id="upload" multiple />
<select id="pick"><option value="a">A</option><option value="b">B</option></select>
<input type="checkbox" id="agree" />
<script>
console.log('fixture loaded');
document.getElementById('inc').addEventListener('click', () => {
  const s = document.getElementById('count');
  s.textContent = String(Number(s.textContent) + 1);
});
</script>
</body></html>"#;

const ASSERT_FIXTURE: &str = r#"<!doctype html><html><head><title>assert me</title></head><body>
<button id="btn" class="cta primary" title="press">Save</button>
<input id="txt" value="ada" />
<input id="blank" value="" />
<input id="off" disabled value="x" />
<div id="ed" contenteditable>edit me</div>
<span id="styled" style="color: rgb(255, 0, 0);">red</span>
<span id="empty"></span>
</body></html>"#;

const DRAG_FIXTURE: &str = r#"<!doctype html><html><head><title>drag me</title><style>
#drag{position:absolute;left:10px;top:10px;width:50px;height:50px;background:red}
#drop{position:absolute;left:300px;top:300px;width:100px;height:100px;border:2px solid blue}
</style></head><body>
<div id="drag"></div><div id="drop"></div>
<script>
const el = document.getElementById('drag');
let dx = 0, dy = 0, on = false;
el.addEventListener('pointerdown', (e) => {
  on = true; dx = e.clientX - el.offsetLeft; dy = e.clientY - el.offsetTop;
  el.setPointerCapture(e.pointerId);
});
el.addEventListener('pointermove', (e) => {
  if (!on) return;
  el.style.left = (e.clientX - dx) + 'px';
  el.style.top = (e.clientY - dy) + 'px';
});
el.addEventListener('pointerup', () => { on = false; });
</script></body></html>"#;

const FRAMES_FIXTURE: &str = r#"<!doctype html><html><head><title>frames host</title></head><body>
<iframe src="/assert" name="inner"></iframe>
</body></html>"#;

const LOCATE_FIXTURE: &str = r#"<!doctype html><html><head><title>locate me</title><meta name="viewport" content="width=device-width, initial-scale=1"></head><body>
<ul id="items"><li class="item">apple</li><li class="item">banana</li><li class="item">cherry</li></ul>
<form id="login">
<label for="user">User name</label><input id="user" placeholder="Enter name" />
<label>Password<input id="pass" type="password" /></label>
<button data-testid="submit-btn">Sign in</button>
</form>
<div id="card"><span class="who">ada</span></div>
<div id="other"><span class="who">bob</span></div>
<img src="data:," alt="Company Logo" />
<a href="/assert" title="Read docs">docs</a>
<div style="height: 2500px;"></div>
<button id="deep">deep</button>
<button id="dlg-alert" onclick="alert('hi there')">show alert</button>
<button id="dlg-confirm" onclick="document.title='c:'+confirm('sure?')">show confirm</button>
<button id="dlg-prompt" onclick="document.title='p:'+prompt('name?','ada')">show prompt</button>
<button id="a11y" aria-label="Close dialog" aria-describedby="a11y-desc">X</button>
<span id="a11y-desc">Closes the window</span>
</body></html>"#;

/// Echo the request method.
async fn echo_method(method: axum::http::Method) -> String {
    method.to_string()
}

/// Spawn the fixture app; returns (base_url, shutdown).
async fn serve() -> (String, tokio::task::AbortHandle) {
    let app = axum::Router::new()
        .route(
            "/",
            axum::routing::get(|| async { axum::response::Html(FIXTURE.to_string()) }),
        )
        .route(
            "/assert",
            axum::routing::get(|| async { axum::response::Html(ASSERT_FIXTURE.to_string()) }),
        )
        .route(
            "/locate",
            axum::routing::get(|| async { axum::response::Html(LOCATE_FIXTURE.to_string()) }),
        )
        .route(
            "/frames",
            axum::routing::get(|| async { axum::response::Html(FRAMES_FIXTURE.to_string()) }),
        )
        .route(
            "/drag",
            axum::routing::get(|| async { axum::response::Html(DRAG_FIXTURE.to_string()) }),
        )
        .route(
            "/api/hi",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"real": true})) }),
        )
        .route(
            "/api/method",
            axum::routing::get(echo_method).post(echo_method),
        )
        .route(
            "/api/echo",
            axum::routing::post(|body: axum::body::Bytes| async move { body }),
        )
        .route(
            "/download/report.txt",
            axum::routing::get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_DISPOSITION,
                        "attachment; filename=\"report.txt\"",
                    )],
                    "ferrite download contents",
                )
            }),
        )
        .route(
            "/static/app.js",
            axum::routing::get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/javascript")],
                    "window.__tagScript = 'loaded';",
                )
            }),
        )
        .route(
            "/static/app.css",
            axum::routing::get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css")],
                    "#name { color: rgb(7, 8, 9); }",
                )
            }),
        )
        .route(
            "/api/echo-headers",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                let probe = headers
                    .get("x-ferrite-probe")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("absent");
                let lang = headers
                    .get(axum::http::header::ACCEPT_LANGUAGE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("absent");
                format!("probe={probe} lang={lang}")
            }),
        )
        .route(
            "/api/auth",
            axum::routing::get(|headers: axum::http::HeaderMap| async move {
                headers
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("absent")
                    .to_string()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await });
    (url, task.abort_handle())
}

/// Launch one browser per available engine (skips missing engines loudly).
async fn browsers() -> Vec<(BrowserKind, Browser)> {
    let mut out = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let exe = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        };
        let Some(exe) = exe else {
            eprintln!("skipping {}: no executable found", kind.name());
            continue;
        };
        match Browser::launch(LaunchOptions::default().browser(kind).executable(exe)).await {
            Ok(browser) => out.push((kind, browser)),
            Err(error) => eprintln!("skipping {}: {error}", kind.name()),
        }
    }
    if out.is_empty() {
        eprintln!("skipping browser test: no chromium or firefox found");
    }
    out
}

/// Poll recorded requests until one matches (events arrive async).
async fn wait_for_recorded(page: &Page, mut matches: impl FnMut(&RecordedRequest) -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if page.requests().iter().any(&mut matches) {
            return true;
        }
        if tokio::time::Instant::now() > deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn navigation_title_and_content() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto_with_options(
            &base,
            NavigationOptions {
                wait_until: LoadState::NetworkIdle,
                timeout: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(page.title().await.unwrap(), "e2e fixture", "{tag}");
        assert!(
            page.url().await.unwrap().starts_with("http://127.0.0.1"),
            "{tag}"
        );
        assert!(
            page.content().await.unwrap().contains("hello ferrite"),
            "{tag}"
        );
        page.wait_for_url("127.0.0.1", Duration::from_secs(5))
            .await
            .unwrap();
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn click_and_selector_engines() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#inc").click().await.unwrap();
        page.locator("#inc").click().await.unwrap();
        page.locator("#count").expect_text("2").await.unwrap();

        // Alternate engines resolve the same DOM.
        page.locator("text=hello ferrite")
            .expect_visible()
            .await
            .unwrap();
        page.locator("xpath=//h1[@id='title']")
            .expect_contains_text("ferrite")
            .await
            .unwrap();
        page.locator("css=#count").expect_text("2").await.unwrap();
        page.locator("role=button[name=\"inc\"]")
            .click()
            .await
            .unwrap();
        page.locator("#count").expect_text("3").await.unwrap();

        assert_eq!(page.locator("button").count().await.unwrap(), 1, "{tag}");
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn form_fill_select_check() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#name").fill("ada").await.unwrap();
        page.locator("#name").expect().value("ada").await.unwrap();
        page.locator("#name").clear().await.unwrap();
        page.locator("#name").expect().value("").await.unwrap();

        page.locator("#pick").select_option("b").await.unwrap();
        page.locator("#pick").expect().value("b").await.unwrap();

        page.locator("#agree").check().await.unwrap();
        page.locator("#agree").expect().checked().await.unwrap();
        page.locator("#agree").uncheck().await.unwrap();
        page.locator("#agree").expect().unchecked().await.unwrap();

        let _ = tag;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn keyboard_types_and_deletes() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.locator("#name").fill("a").await.unwrap();
        page.locator("#name").press_sequentially("b").await.unwrap();
        page.locator("#name").expect().value("ab").await.unwrap();
        page.locator("#name").press("Backspace").await.unwrap();
        page.locator("#name").expect().value("a").await.unwrap();

        let _ = tag;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn screenshot_pdf_console_and_cookies() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        let png = page
            .screenshot(ferrite_e2e::ScreenshotOptions::default())
            .await
            .unwrap();
        assert!(
            png.len() > 100,
            "{tag}: screenshot too small: {}",
            png.len()
        );
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "{tag}");

        let pdf = page.pdf().await.unwrap();
        assert!(
            pdf.starts_with(b"%PDF"),
            "{tag}: not a pdf: {:?}",
            &pdf[..8.min(pdf.len())]
        );

        page.wait_for_timeout(Duration::from_millis(300))
            .await
            .unwrap();
        let console = page.console_messages();
        assert!(
            console.iter().any(|m| m.text.contains("fixture loaded")),
            "{tag}: console: {console:?}"
        );

        page.set_cookie("e2e", "yum").await.unwrap();
        let cookies = page.cookies().await.unwrap();
        assert!(
            cookies.iter().any(|c| c.name == "e2e" && c.value == "yum"),
            "{tag}: {cookies:?}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn request_routing_aborts_and_fulfills() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.route(vec![
            RouteRule::abort("**/api/slow"),
            RouteRule::fulfill("**/api/hi", 200, r#"{"mocked":true}"#, "application/json"),
        ])
        .await
        .unwrap();

        let aborted: String = page
            .evaluate("fetch('/api/slow').then(() => 'ok', () => 'failed')")
            .await
            .unwrap();
        assert_eq!(aborted, "failed", "{tag}");

        let mocked: serde_json::Value = page
            .evaluate("fetch('/api/hi').then(r => r.json())")
            .await
            .unwrap();
        assert_eq!(mocked, serde_json::json!({"mocked": true}), "{tag}");

        page.stop_routing().await;
        let real: serde_json::Value = page
            .evaluate("fetch('/api/hi').then(r => r.json())")
            .await
            .unwrap();
        assert_eq!(real, serde_json::json!({"real": true}), "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn contexts_isolate_cookies() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let one = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        let two = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        let page_one = one.new_page().await.unwrap();
        page_one.goto(&base).await.unwrap();
        page_one.set_cookie("ctx", "one").await.unwrap();

        let page_two = two.new_page().await.unwrap();
        page_two.goto(&base).await.unwrap();
        let cookies = page_two.cookies().await.unwrap();
        assert!(
            !cookies.iter().any(|c| c.name == "ctx"),
            "{tag}: contexts share cookies: {cookies:?}"
        );

        page_one.close().await.unwrap();
        page_two.close().await.unwrap();
        one.close().await.unwrap();
        two.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn dialogs_auto_handle() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.handle_dialogs(true).await.unwrap();
        let done: String = page.evaluate("alert('hi'); 'done'").await.unwrap();
        assert_eq!(done, "done", "{tag}");
        page.stop_dialog_handling().await;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn launch_user_agent_override() {
    // Firefox UA is launch-wide (profile pref); per-page override errors loudly.
    let Some(exe) = ferrite_e2e::find_firefox(None) else {
        eprintln!("skipping firefox UA test: no firefox found");
        return;
    };
    let mut options = LaunchOptions::default().browser(BrowserKind::Firefox);
    options.user_agent = Some("FerriteE2E/1.0".to_string());
    let browser = Browser::launch(options.executable(exe)).await.unwrap();
    let page = browser.new_page().await.unwrap();
    let agent: String = page.evaluate("navigator.userAgent").await.unwrap();
    assert!(agent.contains("FerriteE2E/1.0"), "{agent}");
    let error = page.set_user_agent("other").await.unwrap_err();
    assert!(error.to_string().contains("launch-wide"), "{error}");
    page.close().await.unwrap();
    browser.close().await.unwrap();
}

#[tokio::test]
async fn runner_passes_and_writes_artifacts() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let mut browser = browser;
        let (base, shutdown) = serve().await;
        browser.set_base_url(Some(base.clone()));
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-run-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let report = Runner::default()
            .workers(2)
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .run(
                &browser,
                vec![
                    ferrite_e2e::test("home title", |page| async move {
                        page.goto("/").await?;
                        page.expect_title("e2e fixture").await?;
                        Ok(())
                    }),
                    ferrite_e2e::test("counter clicks", |page| async move {
                        page.goto("/").await?;
                        page.locator("#inc").click().await?;
                        page.locator("#count").expect_text("1").await?;
                        Ok(())
                    }),
                ],
            )
            .await;
        assert_eq!(report.failed(), 0, "{tag}: {}", report.to_list());
        assert_eq!(report.passed(), 2, "{tag}");
        assert!(dir.join("home-title.json").is_file(), "{tag}");
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn runner_reports_failures() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let mut browser = browser;
        let (base, shutdown) = serve().await;
        browser.set_base_url(Some(base));
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-fail-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let report = Runner::default()
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .run(
                &browser,
                vec![ferrite_e2e::test("always fails", |page| async move {
                    page.goto("/").await?;
                    page.locator("#missing").expect_visible().await?;
                    Ok(())
                })],
            )
            .await;
        assert_eq!(report.failed(), 1, "{tag}");
        assert_eq!(report.results[0].status, TestStatus::Failed, "{tag}");
        assert!(!report.results[0].screenshots.is_empty(), "{tag}");
        assert!(
            report.results[0]
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("missing"),
            "{tag}"
        );
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Probe the first video stream: (codec, width). `None` without ffprobe.
fn probe_video(path: &std::path::Path) -> Option<(String, u32)> {
    let ffprobe = ferrite_e2e::find_ffprobe()?;
    let output = std::process::Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let (codec, width) = text.trim().split_once(',')?;
    Some((codec.to_string(), width.parse().ok()?))
}

#[tokio::test]
async fn video_records_playable_file() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        if kind == BrowserKind::Chromium && ferrite_e2e::find_ffmpeg().is_none() {
            eprintln!("skipping chromium video: no ffmpeg found");
            browser.close().await.unwrap();
            continue;
        }
        let (base, shutdown) = serve().await;
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-vid-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.start_video(VideoOptions {
            dir: dir.clone(),
            fps: 10,
            ..VideoOptions::default()
        })
        .await
        .unwrap();
        page.locator("#inc").click().await.unwrap();
        page.wait_for_timeout(Duration::from_millis(1200))
            .await
            .unwrap();
        page.locator("#inc").click().await.unwrap();
        let path = page.stop_video(&dir.join("rec.webm")).await.unwrap();
        assert!(path.is_file(), "{tag}: no video at {}", path.display());
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len > 1024, "{tag}: video too small ({len}b)");
        if let Some((codec, width)) = probe_video(&path) {
            assert!(
                codec == "vp8" || codec == "vp9",
                "{tag}: unexpected codec {codec}"
            );
            assert!(width > 0, "{tag}: zero width");
        } else {
            eprintln!("{tag}: ffprobe unavailable, skipped stream check");
        }
        // Nothing stranded next to the output (spool cleaned, source moved).
        let strays: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != "rec.webm")
            .collect();
        assert!(strays.is_empty(), "{tag}: stray files: {strays:?}");
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn frames_stream_yields_images() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let mut stream = page.frames(VideoOptions::default()).await.unwrap();
        // Chromium screencast emits on repaint only: keep the page moving.
        let mover = page.clone();
        let motion = tokio::spawn(async move {
            for _ in 0..30 {
                let _ = mover.locator("#inc").click().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        let mut count = 0;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while count < 5 {
            assert!(
                tokio::time::Instant::now() < deadline,
                "{tag}: only {count} frames in 15s"
            );
            let frame = tokio::time::timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .expect("stream ended early");
            assert_eq!(frame.index as usize, count, "{tag}");
            // Chromium screencast emits JPEG; Firefox polling emits PNG.
            let jpeg = frame.data.starts_with(&[0xFF, 0xD8]);
            let png = frame.data.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]);
            assert!(jpeg || png, "{tag}: unknown frame magic");
            assert!(frame.data.len() > 100, "{tag}");
            count += 1;
        }
        motion.abort();
        page.stop_frames().await;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_screenshot_clips() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        let clip = page.locator("#inc").screenshot().await.unwrap();
        assert_eq!(&clip[..8], &[137, 80, 78, 71, 13, 10, 26, 10], "{tag}");
        assert!(clip.len() > 100, "{tag}");
        let full = page
            .screenshot(ferrite_e2e::ScreenshotOptions::default())
            .await
            .unwrap();
        assert!(clip.len() < full.len(), "{tag}: clip not smaller than page");
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn video_capture_guard() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-guard-{}-{tag}", std::process::id()));
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        // Stop without start is loud.
        assert!(page.stop_video(&dir.join("x.webm")).await.is_err(), "{tag}");
        // Second capture while one is active is loud (and keeps the first).
        page.start_video(VideoOptions {
            dir: dir.clone(),
            ..VideoOptions::default()
        })
        .await
        .unwrap();
        let error = page.frames(VideoOptions::default()).await.unwrap_err();
        assert!(
            error.to_string().contains("already active"),
            "{tag}: {error}"
        );
        page.cancel_video().await;
        // After cancel, frames work again (motion first: Chromium emits on
        // repaint only, so bound the wait).
        page.locator("#inc").click().await.unwrap();
        let mut stream = page.frames(VideoOptions::default()).await.unwrap();
        let mover = page.clone();
        let motion = tokio::spawn(async move {
            for _ in 0..10 {
                let _ = mover.locator("#inc").click().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });
        assert!(
            stream.next_timeout(Duration::from_secs(10)).await.is_some(),
            "{tag}"
        );
        motion.abort();
        page.stop_frames().await;
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn runner_video_only_on_failure() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let mut browser = browser;
        if kind == BrowserKind::Chromium && ferrite_e2e::find_ffmpeg().is_none() {
            eprintln!("skipping chromium runner video: no ffmpeg found");
            browser.close().await.unwrap();
            continue;
        }
        let (base, shutdown) = serve().await;
        browser.set_base_url(Some(base));
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-rvid-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let report = Runner::default()
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .video_mode(VideoMode::OnlyOnFailure)
            .run(
                &browser,
                vec![
                    ferrite_e2e::test("passes quietly", |page| async move {
                        page.goto("/").await?;
                        page.expect_title("e2e fixture").await?;
                        Ok(())
                    }),
                    ferrite_e2e::test("fails loudly", |page| async move {
                        page.goto("/").await?;
                        page.locator("#missing").expect_visible().await?;
                        Ok(())
                    }),
                ],
            )
            .await;
        assert_eq!(report.failed(), 1, "{tag}");
        let passed = report
            .results
            .iter()
            .find(|r| r.name == "passes quietly")
            .unwrap();
        assert!(passed.video.is_none(), "{tag}: {:?}", passed.video);
        let failed = report
            .results
            .iter()
            .find(|r| r.name == "fails loudly")
            .unwrap();
        let video = failed.video.as_ref().expect("no video attached");
        assert!(std::path::Path::new(video).is_file(), "{tag}: {video}");
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn expect_completions_and_negation() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}assert")).await.unwrap();

        page.expect().title("assert me").await.unwrap();
        page.expect().not().title("zzz").await.unwrap();

        page.locator("#btn")
            .expect()
            .attribute("title", "press")
            .await
            .unwrap();
        page.locator("#btn").expect().id("btn").await.unwrap();
        page.locator("#btn")
            .expect()
            .contains_class("cta")
            .await
            .unwrap();
        page.locator("#btn")
            .expect()
            .contains_class("primary")
            .await
            .unwrap();
        page.locator("#btn")
            .expect()
            .not()
            .contains_class("nope")
            .await
            .unwrap();
        page.locator("#styled")
            .expect()
            .css("color", "rgb(255, 0, 0)")
            .await
            .unwrap();
        page.locator("#txt")
            .expect()
            .js_property("value", &"ada")
            .await
            .unwrap();
        page.locator("#txt")
            .expect()
            .js_property("readOnly", &false)
            .await
            .unwrap();
        page.locator("#txt")
            .expect()
            .not()
            .js_property("zzz", &"x")
            .await
            .unwrap();

        page.locator("#off").expect().disabled().await.unwrap();
        page.locator("#txt")
            .expect()
            .not()
            .disabled()
            .await
            .unwrap();
        page.locator("#txt").expect().enabled().await.unwrap();

        page.locator("#txt").expect().editable().await.unwrap();
        page.locator("#ed").expect().editable().await.unwrap();
        page.locator("#btn")
            .expect()
            .not()
            .editable()
            .await
            .unwrap();
        page.locator("#off")
            .expect()
            .not()
            .editable()
            .await
            .unwrap();

        page.locator("#blank").expect().empty().await.unwrap();
        page.locator("#empty").expect().empty().await.unwrap();
        page.locator("#txt").expect().not().empty().await.unwrap();

        page.locator("#txt").focus().await.unwrap();
        page.locator("#txt").expect().focused().await.unwrap();
        page.locator("#btn").expect().not().focused().await.unwrap();

        page.locator("#btn").expect().attached().await.unwrap();
        page.locator("#missing")
            .expect()
            .not()
            .attached()
            .await
            .unwrap();

        page.locator("#btn")
            .expect()
            .not()
            .not()
            .visible()
            .await
            .unwrap();
        page.locator("#btn")
            .expect()
            .not()
            .text("zzz")
            .await
            .unwrap();
        page.locator("#txt")
            .expect()
            .not()
            .value("bob")
            .await
            .unwrap();

        // Failure paths report the last value and the negation.
        let error = page
            .locator("#btn")
            .expect()
            .timeout(Timeout::ms(150))
            .text("zzz")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("text was"), "{tag}: {error}");
        let error = page
            .locator("#btn")
            .expect()
            .not()
            .timeout(Timeout::ms(150))
            .text("Save")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("(not)"), "{tag}: {error}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_addressing() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}locate")).await.unwrap();

        // get_by_* conveniences.
        page.get_by_test_id("submit-btn")
            .expect_text("Sign in")
            .await
            .unwrap();
        page.get_by_text("banana").expect_visible().await.unwrap();
        page.get_by_role("button", "Sign in")
            .expect_visible()
            .await
            .unwrap();
        page.get_by_role("link", "")
            .expect_text("docs")
            .await
            .unwrap();
        page.get_by_label("User name")
            .expect_text("User name")
            .await
            .unwrap();
        assert_eq!(
            page.get_by_label("User name").count().await.unwrap(),
            1,
            "{tag}"
        );
        page.get_by_placeholder("ENTER")
            .expect()
            .id("user")
            .await
            .unwrap();
        page.get_by_alt("logo").expect().attached().await.unwrap();
        page.get_by_title("DOCS").expect_text("docs").await.unwrap();

        // first/last/nth narrowing.
        assert_eq!(page.locator(".item").count().await.unwrap(), 3, "{tag}");
        page.locator(".item")
            .first()
            .expect_text("apple")
            .await
            .unwrap();
        page.locator(".item")
            .last()
            .expect_text("cherry")
            .await
            .unwrap();
        page.locator(".item")
            .nth(1)
            .expect_text("banana")
            .await
            .unwrap();
        assert_eq!(
            page.locator(".item").nth(1).count().await.unwrap(),
            1,
            "{tag}"
        );
        assert_eq!(
            page.locator(".item").nth(9).count().await.unwrap(),
            0,
            "{tag}"
        );
        assert!(page.locator(".item").nth(9).click().await.is_err(), "{tag}");

        // Chaining (multi-engine, multi-level).
        assert_eq!(
            page.locator("#login")
                .locator("input")
                .count()
                .await
                .unwrap(),
            2,
            "{tag}"
        );
        page.locator("#card")
            .locator(".who")
            .expect_text("ada")
            .await
            .unwrap();
        assert_eq!(
            page.locator("body")
                .locator("#login")
                .locator("input")
                .count()
                .await
                .unwrap(),
            2,
            "{tag}"
        );
        assert_eq!(
            page.locator("#login")
                .locator("xpath=.//input")
                .count()
                .await
                .unwrap(),
            2,
            "{tag}"
        );
        page.locator("#login")
            .get_by_text("Sign in")
            .expect_text("Sign in")
            .await
            .unwrap();
        page.locator("#login")
            .get_by_role("button", "")
            .expect_text("Sign in")
            .await
            .unwrap();
        page.locator("#login")
            .get_by_placeholder("Enter")
            .expect()
            .id("user")
            .await
            .unwrap();

        // Text filters (case-insensitive).
        assert_eq!(
            page.locator(".item").filter("an").count().await.unwrap(),
            1,
            "{tag}"
        );
        page.locator(".who")
            .filter("BO")
            .expect_text("bob")
            .await
            .unwrap();

        // Combinators.
        assert_eq!(
            page.locator("#user")
                .or_(&page.locator("#pass"))
                .unwrap()
                .count()
                .await
                .unwrap(),
            2,
            "{tag}"
        );
        let both = page
            .locator(".item")
            .and_(&page.locator("text=an"))
            .unwrap();
        assert_eq!(both.count().await.unwrap(), 1, "{tag}");
        both.expect_text("banana").await.unwrap();

        // One locator per match.
        let items = page.locator(".item").all().await.unwrap();
        assert_eq!(items.len(), 3, "{tag}");
        assert_eq!(items[0].text().await.unwrap(), "apple", "{tag}");
        assert_eq!(items[1].text().await.unwrap(), "banana", "{tag}");
        assert_eq!(items[2].text().await.unwrap(), "cherry", "{tag}");

        // Cross-page combination is a loud error.
        let other = browser.new_page().await.unwrap();
        let error = page
            .locator("#user")
            .or_(&other.locator("#user"))
            .expect_err("or_ across pages must fail");
        assert!(
            error.to_string().contains("different pages"),
            "{tag}: {error}"
        );
        other.close().await.unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_micro_actions() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}locate")).await.unwrap();

        // dispatch_event with detail reaches a listener.
        page.evaluate::<serde_json::Value>(
            "document.getElementById('user').addEventListener('ping', e => { \
             document.title = 'got:' + e.detail.n; })",
        )
        .await
        .unwrap();
        page.locator("#user")
            .dispatch_event("ping", Some(&serde_json::json!({"n": 7})))
            .await
            .unwrap();
        page.expect().title("got:7").await.unwrap();
        assert!(
            page.locator("#missing")
                .dispatch_event("ping", None)
                .await
                .is_err(),
            "{tag}"
        );

        // select_text on an input selects the whole value.
        page.locator("#user").fill("hello").await.unwrap();
        page.locator("#user").select_text().await.unwrap();
        let selected = page
            .evaluate_value(
                "(() => { const el = document.getElementById('user'); \
                 return el.selectionStart === 0 && el.selectionEnd === 5; })()",
            )
            .await
            .unwrap();
        assert_eq!(selected, serde_json::Value::Bool(true), "{tag}");
        // select_text on rendered text populates the selection.
        page.locator("#card .who").select_text().await.unwrap();
        let text = page
            .evaluate_value("document.getSelection().toString()")
            .await
            .unwrap();
        assert_eq!(text, serde_json::json!("ada"), "{tag}");
        assert!(
            page.locator("#missing").select_text().await.is_err(),
            "{tag}"
        );

        // scroll_into_view brings an off-screen element into view.
        let top_before = page
            .evaluate_value("document.getElementById('deep').getBoundingClientRect().top")
            .await
            .unwrap()
            .as_f64()
            .unwrap();
        assert!(top_before > 600.0, "{tag}: {top_before}");
        page.locator("#deep").scroll_into_view().await.unwrap();
        let top_after = page
            .evaluate_value("document.getElementById('deep').getBoundingClientRect().top")
            .await
            .unwrap()
            .as_f64()
            .unwrap();
        let height = page
            .evaluate_value("window.innerHeight")
            .await
            .unwrap()
            .as_f64()
            .unwrap();
        assert!(
            top_after >= 0.0 && top_after < height,
            "{tag}: {top_after} of {height}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn full_input() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}locate")).await.unwrap();

        // A held Shift extends a text selection.
        page.locator("#user").fill("hello").await.unwrap();
        page.evaluate_value(
            "(() => { const el = document.getElementById('user'); \
             el.focus(); el.setSelectionRange(5, 5); return true; })()",
        )
        .await
        .unwrap();
        page.key_down("Shift").await.unwrap();
        page.press_key("ArrowLeft").await.unwrap();
        page.key_up("Shift").await.unwrap();
        let selection = page
            .evaluate_value(
                "(() => { const el = document.getElementById('user'); \
                 return [el.selectionStart, el.selectionEnd]; })()",
            )
            .await
            .unwrap();
        assert_eq!(selection, serde_json::json!([4, 5]), "{tag}");

        // Mouse down/up fire in order.
        page.evaluate_value(
            "(() => { const el = document.getElementById('user'); \
             el.addEventListener('mousedown', () => { document.title = 'down'; }); \
             el.addEventListener('mouseup', () => { document.title = 'up'; }); \
             return true; })()",
        )
        .await
        .unwrap();
        let center = page
            .evaluate::<(f64, f64)>(
                "(() => { const r = document.getElementById('user').getBoundingClientRect(); \
                 return [r.x + r.width / 2, r.y + r.height / 2]; })()",
            )
            .await
            .unwrap();
        page.mouse_down(center.0, center.1).await.unwrap();
        page.expect().title("down").await.unwrap();
        page.mouse_up(center.0, center.1).await.unwrap();
        page.expect().title("up").await.unwrap();

        // The wheel scrolls the page both ways.
        page.mouse_wheel(400.0, 300.0, 0.0, 240.0).await.unwrap();
        page.wait_for_function("window.scrollY > 0", Duration::from_secs(5))
            .await
            .unwrap();
        let scrolled = page
            .evaluate_value("window.scrollY")
            .await
            .unwrap()
            .as_f64()
            .unwrap();
        page.mouse_wheel(400.0, 300.0, 0.0, -240.0).await.unwrap();
        page.wait_for_function(
            &format!("window.scrollY < {scrolled}"),
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn dialog_contents() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}locate")).await.unwrap();
        assert!(page.last_dialog().is_none(), "{tag}");

        page.handle_dialogs(true).await.unwrap();
        page.locator("#dlg-alert").click().await.unwrap();
        let dialog = page.last_dialog().expect("alert recorded");
        assert_eq!(dialog.dialog_type, "alert", "{tag}");
        assert_eq!(dialog.message, "hi there", "{tag}");

        page.locator("#dlg-confirm").click().await.unwrap();
        page.expect().title("c:true").await.unwrap();
        let dialog = page.last_dialog().expect("confirm recorded");
        assert_eq!(dialog.dialog_type, "confirm", "{tag}");
        assert_eq!(dialog.message, "sure?", "{tag}");

        page.handle_dialogs(false).await.unwrap();
        page.locator("#dlg-confirm").click().await.unwrap();
        page.expect().title("c:false").await.unwrap();

        page.handle_dialogs_with_prompt(true, "bob").await.unwrap();
        page.locator("#dlg-prompt").click().await.unwrap();
        page.expect().title("p:bob").await.unwrap();
        let dialog = page.last_dialog().expect("prompt recorded");
        assert_eq!(dialog.dialog_type, "prompt", "{tag}");
        assert_eq!(dialog.message, "name?", "{tag}");

        page.stop_dialog_handling().await;
        assert_eq!(page.dialogs().len(), 4, "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn permissions_and_emulation() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Granted permissions read back as granted.
        page.grant_permissions(&["geolocation"]).await.unwrap();
        page.evaluate_value(
            "navigator.permissions.query({name:'geolocation'}) \
             .then(r => document.title = 'perm:' + r.state)",
        )
        .await
        .unwrap();
        page.expect().title("perm:granted").await.unwrap();

        // Geolocation override (Firefox: loud error when unsupported).
        match page.set_geolocation(51.5, -0.12).await {
            Ok(()) => {
                page.evaluate_value(
                    "navigator.geolocation.getCurrentPosition( \
                     p => document.title = 'geo:' + p.coords.latitude + ',' + p.coords.longitude, \
                     e => document.title = 'geo-err:' + e.code + ':' + e.message)",
                )
                .await
                .unwrap();
                page.expect().title("geo:51.5,-0.12").await.unwrap();
            }
            Err(error) => {
                assert_eq!(tag, "firefox", "{tag}: unexpected {error}");
                assert!(error.to_string().contains("newer build"), "{tag}: {error}");
            }
        }

        if kind == BrowserKind::Chromium {
            // Offline blocks navigation; back online restores it.
            page.set_offline(true).await.unwrap();
            assert!(page.goto(&format!("{base}assert")).await.is_err(), "{tag}");
            page.set_offline(false).await.unwrap();
            page.goto(&format!("{base}assert")).await.unwrap();

            // Extra headers reach the server.
            page.set_extra_http_headers(&[("X-Ferrite-Probe", "probe-1")])
                .await
                .unwrap();
            page.goto(&format!("{base}api/echo-headers")).await.unwrap();
            assert!(page.content().await.unwrap().contains("probe-1"), "{tag}");

            // Locale + timezone (apply to the next document).
            page.set_locale("fr-FR").await.unwrap();
            page.set_timezone("America/New_York").await.unwrap();
            page.goto(&base).await.unwrap();
            // The override drives Intl (navigator.language follows --lang).
            let intl = page
                .evaluate_value("Intl.DateTimeFormat().resolvedOptions().locale")
                .await
                .unwrap();
            assert_eq!(intl, serde_json::json!("fr-FR"), "{tag}");
            let zone = page
                .evaluate_value("Intl.DateTimeFormat().resolvedOptions().timeZone")
                .await
                .unwrap();
            assert_eq!(zone, serde_json::json!("America/New_York"), "{tag}");

            // Media emulation.
            page.emulate_media(Some(ColorScheme::Dark), Some(ReducedMotion::Reduce))
                .await
                .unwrap();
            let dark = page
                .evaluate_value("matchMedia('(prefers-color-scheme: dark)').matches")
                .await
                .unwrap();
            assert_eq!(dark, serde_json::Value::Bool(true), "{tag}");
            let reduce = page
                .evaluate_value("matchMedia('(prefers-reduced-motion: reduce)').matches")
                .await
                .unwrap();
            assert_eq!(reduce, serde_json::Value::Bool(true), "{tag}");
            page.emulate_media(None, None).await.unwrap();
        } else {
            // Firefox loud errors for BiDi gaps.
            let error = page.set_offline(true).await.unwrap_err();
            assert!(
                error.to_string().contains("not supported"),
                "{tag}: {error}"
            );
            let error = page
                .set_extra_http_headers(&[("X-Ferrite-Probe", "probe-1")])
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("not supported"),
                "{tag}: {error}"
            );
            let error = page.set_locale("fr-FR").await.unwrap_err();
            assert!(
                error.to_string().contains("not supported"),
                "{tag}: {error}"
            );
            let error = page.set_timezone("America/New_York").await.unwrap_err();
            assert!(
                error.to_string().contains("not supported"),
                "{tag}: {error}"
            );
            let error = page
                .emulate_media(Some(ColorScheme::Dark), None)
                .await
                .unwrap_err();
            assert!(
                error.to_string().contains("not supported"),
                "{tag}: {error}"
            );
            page.emulate_media(None, None).await.unwrap();
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn storage_state_round_trip() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.set_cookie("sess", "abc123").await.unwrap();
        page.evaluate_value("localStorage.setItem('theme', 'dark')")
            .await
            .unwrap();

        let path =
            std::env::temp_dir().join(format!("ferrite-storage-{}-{tag}.json", std::process::id()));
        page.save_storage_state(&path).await.unwrap();
        page.close().await.unwrap();

        // A fresh page restores cookies + localStorage from the file.
        let fresh = browser.new_page().await.unwrap();
        fresh.goto(&base).await.unwrap();
        fresh.load_storage_state(&path).await.unwrap();
        let cookies = fresh.cookies().await.unwrap();
        assert!(
            cookies
                .iter()
                .any(|c| c.name == "sess" && c.value == "abc123"),
            "{tag}: {cookies:?}"
        );
        let theme = fresh
            .evaluate_value("localStorage.getItem('theme')")
            .await
            .unwrap();
        assert_eq!(theme, serde_json::json!("dark"), "{tag}");

        // Loading on the wrong origin is a loud error.
        let blank = browser.new_page().await.unwrap();
        let error = blank.load_storage_state(&path).await.unwrap_err();
        assert!(
            error.to_string().contains("navigate there first"),
            "{tag}: {error}"
        );
        blank.close().await.unwrap();

        fresh.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_file(&path);
    }
}

#[tokio::test]
async fn network_observe_and_unroute() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();

        // Capture records completed requests.
        page.start_request_capture();
        page.goto(&format!("{base}assert")).await.unwrap();
        assert!(
            wait_for_recorded(&page, |r| {
                r.url.ends_with("/assert") && r.method == "GET" && r.status == 200
            })
            .await,
            "{tag}: {:?}",
            page.requests()
        );

        // Restarting clears the buffer; stopping keeps it.
        page.start_request_capture();
        assert!(page.requests().is_empty(), "{tag}");
        page.goto(&base).await.unwrap();
        assert!(
            wait_for_recorded(&page, |r| r.url == base && r.status == 200).await,
            "{tag}: {:?}",
            page.requests()
        );
        page.stop_request_capture();
        page.stop_request_capture();
        assert!(!page.requests().is_empty(), "{tag}");

        // unroute removes one rule and restores the real response.
        page.route(vec![RouteRule::fulfill(
            "**/api/hi",
            200,
            r#"{"mocked":true}"#,
            "application/json",
        )])
        .await
        .unwrap();
        page.goto(&format!("{base}api/hi")).await.unwrap();
        assert!(page.content().await.unwrap().contains("mocked"), "{tag}");
        assert_eq!(page.unroute("**/api/hi").await.unwrap(), 1, "{tag}");
        page.goto(&format!("{base}api/hi")).await.unwrap();
        assert!(page.content().await.unwrap().contains("real"), "{tag}");
        assert_eq!(page.unroute("**/nothing").await.unwrap(), 0, "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn runner_tags_grep_shard_describe() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-p9-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut tests = describe(
            "auth",
            vec![
                test("login", |_| async { Ok(()) }).tag("fast"),
                test("logout", |_| async { Ok(()) }).tag("slow"),
            ],
        );
        tests.push(test("home", |_| async { Ok(()) }).tag("fast"));
        let out = || {
            Runner::default()
                .output_dir(dir.display().to_string())
                .list_progress(false)
        };
        let names_of = |report: ferrite_e2e::TestReport| {
            report
                .results
                .iter()
                .map(|r| r.name.clone())
                .collect::<Vec<_>>()
        };

        // describe() prefixes group names.
        assert_eq!(tests[0].name, "auth > login", "{tag}");

        // filter() matches names and tags.
        let report = out().filter("auth").run(&browser, tests.clone()).await;
        assert_eq!(
            names_of(report),
            vec!["auth > login".to_string(), "auth > logout".to_string()],
            "{tag}"
        );
        let report = out().filter("fast").run(&browser, tests.clone()).await;
        assert_eq!(
            names_of(report),
            vec!["auth > login".to_string(), "home".to_string()],
            "{tag}"
        );

        // grep() ANDs with filter().
        let report = out()
            .filter("auth")
            .grep("slow")
            .run(&browser, tests.clone())
            .await;
        assert_eq!(names_of(report), vec!["auth > logout".to_string()], "{tag}");

        // Shards split by name order: auth > login, auth > logout, home.
        let report = out().shard(1, 2).run(&browser, tests.clone()).await;
        assert_eq!(
            names_of(report),
            vec!["auth > login".to_string(), "home".to_string()],
            "{tag}"
        );
        let report = out().shard(2, 2).run(&browser, tests.clone()).await;
        assert_eq!(names_of(report), vec!["auth > logout".to_string()], "{tag}");

        browser.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn touchscreen_a11y_storage() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // A touchscreen tap activates the counter.
        let center = page
            .evaluate::<(f64, f64)>(
                "(() => { const r = document.getElementById('inc').getBoundingClientRect(); \
                 return [r.x + r.width / 2, r.y + r.height / 2]; })()",
            )
            .await
            .unwrap();
        page.touchscreen_tap(center.0, center.1).await.unwrap();
        page.locator("#count").expect_text("1").await.unwrap();

        // Web-storage helpers.
        page.local_storage_set("theme", "dark").await.unwrap();
        assert_eq!(
            page.local_storage_get("theme").await.unwrap().as_deref(),
            Some("dark"),
            "{tag}"
        );
        page.local_storage_remove("theme").await.unwrap();
        assert!(
            page.local_storage_get("theme").await.unwrap().is_none(),
            "{tag}"
        );
        page.local_storage_set("a", "1").await.unwrap();
        page.local_storage_clear().await.unwrap();
        assert!(
            page.local_storage_get("a").await.unwrap().is_none(),
            "{tag}"
        );
        page.session_storage_set("tab", "7").await.unwrap();
        assert_eq!(
            page.session_storage_get("tab").await.unwrap().as_deref(),
            Some("7"),
            "{tag}"
        );
        page.session_storage_remove("tab").await.unwrap();
        assert!(
            page.session_storage_get("tab").await.unwrap().is_none(),
            "{tag}"
        );
        page.session_storage_set("b", "2").await.unwrap();
        page.session_storage_clear().await.unwrap();
        assert!(
            page.session_storage_get("b").await.unwrap().is_none(),
            "{tag}"
        );

        // innerHTML + accessible names.
        page.goto(&format!("{base}locate")).await.unwrap();
        let html = page
            .locator("#items")
            .inner_html()
            .await
            .unwrap()
            .expect("items html");
        assert!(html.contains("banana"), "{tag}: {html}");
        assert!(
            page.locator("#missing")
                .inner_html()
                .await
                .unwrap()
                .is_none(),
            "{tag}"
        );
        page.locator("#a11y")
            .expect()
            .accessible_name("Close dialog")
            .await
            .unwrap();
        page.locator("#a11y")
            .expect()
            .accessible_description("Closes the window")
            .await
            .unwrap();
        page.locator("#a11y")
            .expect()
            .not()
            .accessible_name("X")
            .await
            .unwrap();
        page.locator("#deep")
            .expect()
            .accessible_name("deep")
            .await
            .unwrap();

        // Viewport intersection.
        page.locator("#user").expect().in_viewport().await.unwrap();
        page.locator("#deep")
            .expect()
            .not()
            .in_viewport()
            .await
            .unwrap();
        page.locator("#deep").scroll_into_view().await.unwrap();
        page.locator("#deep").expect().in_viewport().await.unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn runner_modes_and_overrides() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-modes-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = || {
            Runner::default()
                .output_dir(dir.display().to_string())
                .list_progress(false)
        };

        // skip/fixme report Skipped without running (bodies would fail).
        let report = out()
            .run(
                &browser,
                vec![
                    test("skipped", |_| async {
                        Err(E2eError::Config("must not run".to_string()))
                    })
                    .skip(),
                    test("fixme", |_| async {
                        Err(E2eError::Config("must not run".to_string()))
                    })
                    .fixme(),
                    test("runs", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.results.len(), 3, "{tag}");
        for result in &report.results {
            match result.name.as_str() {
                "runs" => assert_eq!(result.status, TestStatus::Passed, "{tag}"),
                _ => {
                    assert_eq!(result.status, TestStatus::Skipped, "{tag}");
                    assert_eq!(result.attempts, 0, "{tag}");
                }
            }
        }

        // only() restricts the run.
        let report = out()
            .run(
                &browser,
                vec![
                    test("a", |_| async { Ok(()) }),
                    test("b", |_| async { Ok(()) }).only(),
                ],
            )
            .await;
        assert_eq!(report.results.len(), 1, "{tag}");
        assert_eq!(report.results[0].name, "b", "{tag}");

        // Per-test retries override the runner default.
        let tries = std::sync::Arc::new(AtomicUsize::new(0));
        let flaky = {
            let tries = tries.clone();
            test("flaky", move |_| {
                let tries = tries.clone();
                async move {
                    if tries.fetch_add(1, Ordering::SeqCst) == 0 {
                        Err(E2eError::Config("first attempt fails".to_string()))
                    } else {
                        Ok(())
                    }
                }
            })
            .retries(1)
        };
        let report = out().retries(0).run(&browser, vec![flaky]).await;
        assert_eq!(report.results[0].status, TestStatus::Passed, "{tag}");
        assert_eq!(report.results[0].attempts, 2, "{tag}");

        // Per-test timeouts fail fast; slow() triples them.
        let report = out()
            .run(
                &browser,
                vec![test("tight", |_| async {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    Ok(())
                })
                .timeout(Duration::from_millis(100))],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Failed, "{tag}");
        let report = out()
            .run(
                &browser,
                vec![test("roomy", |_| async {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    Ok(())
                })
                .timeout(Duration::from_millis(200))
                .slow()],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Passed, "{tag}");

        browser.close().await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn runner_hooks_steps_and_invert() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-hooks-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        // Hooks run around every attempt; steps land in the trace.
        let before = std::sync::Arc::new(AtomicUsize::new(0));
        let after = std::sync::Arc::new(AtomicUsize::new(0));
        let setup = std::sync::Arc::new(AtomicUsize::new(0));
        let teardown = std::sync::Arc::new(AtomicUsize::new(0));
        let h_before = before.clone();
        let h_after = after.clone();
        let h_setup = setup.clone();
        let h_teardown = teardown.clone();
        let seen_title = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let seen_title_hook = seen_title.clone();
        let report = Runner::default()
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .global_setup(move || {
                let h_setup = h_setup.clone();
                async move {
                    h_setup.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .global_teardown(move || {
                let h_teardown = h_teardown.clone();
                async move {
                    h_teardown.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .before_each({
                let h_before = h_before.clone();
                let hook_base = base.clone();
                move |page| {
                    let h_before = h_before.clone();
                    let hook_base = hook_base.clone();
                    async move {
                        h_before.fetch_add(1, Ordering::SeqCst);
                        page.goto(&hook_base).await.map(|_| ())
                    }
                }
            })
            .after_each(move |page| {
                let h_after = h_after.clone();
                let seen_title_hook = seen_title_hook.clone();
                async move {
                    h_after.fetch_add(1, Ordering::SeqCst);
                    let title = page.title().await.unwrap_or_default();
                    *seen_title_hook.lock().unwrap() = title;
                    Ok(())
                }
            })
            .run(
                &browser,
                vec![test("stepped", |page| async move {
                    page.step("assert title", async {
                        page.expect().title("e2e fixture").await
                    })
                    .await
                })],
            )
            .await;
        assert_eq!(report.results[0].status, TestStatus::Passed, "{tag}");
        assert_eq!(before.load(Ordering::SeqCst), 1, "{tag}");
        assert_eq!(after.load(Ordering::SeqCst), 1, "{tag}");
        assert_eq!(setup.load(Ordering::SeqCst), 1, "{tag}");
        assert_eq!(teardown.load(Ordering::SeqCst), 1, "{tag}");
        assert_eq!(seen_title.lock().unwrap().as_str(), "e2e fixture", "{tag}");
        let trace_raw = std::fs::read_to_string(dir.join("stepped.json")).unwrap();
        assert!(trace_raw.contains("assert title"), "{tag}: {trace_raw}");

        // grep_invert drops matches.
        let report = Runner::default()
            .output_dir(dir.display().to_string())
            .list_progress(false)
            .grep_invert("slow")
            .run(
                &browser,
                vec![
                    test("quick", |_| async { Ok(()) }),
                    test("slowpoke", |_| async { Ok(()) }),
                ],
            )
            .await;
        assert_eq!(report.results.len(), 1, "{tag}");
        assert_eq!(report.results[0].name, "quick", "{tag}");

        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn strict_cookies_device_clock_screenshot() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}locate")).await.unwrap();

        // Strict mode: three `.item` matches is a violation; one match works.
        let err = page.locator(".item").strict().click().await.unwrap_err();
        assert!(
            err.to_string().contains("strict mode violation"),
            "{tag}: {err}"
        );
        page.locator("#a11y").strict().click().await.unwrap();

        // Full-fidelity cookies round-trip (Chrome caps lifetime at 400 days).
        let fresh = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 3600;
        page.add_cookies(&[Cookie {
            name: "full".to_string(),
            value: "fidelity".to_string(),
            domain: None,
            path: Some("/".to_string()),
            http_only: true,
            secure: false,
            expires: Some(fresh),
        }])
        .await
        .unwrap();
        let found = page
            .cookies()
            .await
            .unwrap()
            .into_iter()
            .find(|cookie| cookie.name == "full")
            .unwrap();
        assert_eq!(found.value, "fidelity", "{tag}");
        assert_eq!(found.path.as_deref(), Some("/"), "{tag}");
        assert!(found.http_only, "{tag}");
        assert_eq!(found.expires, Some(fresh), "{tag}");

        // Device emulation is Chromium-only.
        if kind == BrowserKind::Chromium {
            page.emulate_device(DeviceDescriptor::IPHONE_15)
                .await
                .unwrap();
            let width: i64 = page.evaluate("window.innerWidth").await.unwrap();
            assert_eq!(width, 393, "{tag}");
            let touch: i64 = page.evaluate("navigator.maxTouchPoints").await.unwrap();
            assert!(touch > 0, "{tag}");
            page.emulate_device(DeviceDescriptor::DESKTOP_1080P)
                .await
                .unwrap();
        } else {
            let err = page
                .emulate_device(DeviceDescriptor::IPHONE_15)
                .await
                .unwrap_err();
            assert!(
                err.to_string().contains("only supported on Chromium"),
                "{tag}: {err}"
            );
        }

        // Fake clock: advancing without installing fails loudly.
        let err = page.clock_advance(1).await.unwrap_err();
        assert!(err.to_string().contains("clock_install"), "{tag}: {err}");
        page.clock_install().await.unwrap();
        let before: i64 = page.evaluate("Date.now()").await.unwrap();
        page.wait_for_timeout(Duration::from_millis(200))
            .await
            .unwrap();
        let frozen: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(before, frozen, "{tag}: clock not frozen");
        page.evaluate_value(
            "window.__fired = []; \
             setTimeout(() => window.__fired.push('a'), 1000); \
             setTimeout(() => window.__fired.push('b'), 3000); 1",
        )
        .await
        .unwrap();
        page.clock_advance(1500).await.unwrap();
        let fired: Vec<String> = page.evaluate("window.__fired").await.unwrap();
        assert_eq!(fired, vec!["a".to_string()], "{tag}");
        let mid: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(mid - frozen, 1500, "{tag}");
        page.clock_advance(2000).await.unwrap();
        let fired: Vec<String> = page.evaluate("window.__fired").await.unwrap();
        assert_eq!(fired, vec!["a".to_string(), "b".to_string()], "{tag}");
        let advanced: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(advanced - mid, 2000, "{tag}");
        page.clock_uninstall().await.unwrap();
        let r1: i64 = page.evaluate("Date.now()").await.unwrap();
        page.wait_for_timeout(Duration::from_millis(100))
            .await
            .unwrap();
        let r2: i64 = page.evaluate("Date.now()").await.unwrap();
        assert!(r2 > r1, "{tag}: clock did not resume");

        // Screenshot options: mask + flags capture and clean up.
        let shot = page
            .screenshot(ScreenshotOptions {
                mask: vec![page.locator("#items")],
                disable_animations: true,
                hide_caret: true,
                ..ScreenshotOptions::default()
            })
            .await
            .unwrap();
        assert!(!shot.is_empty(), "{tag}");
        let leftover: i64 = page
            .evaluate("document.querySelectorAll('.ferrite-shot-mask').length")
            .await
            .unwrap();
        assert_eq!(leftover, 0, "{tag}");
        let err = page
            .screenshot(ScreenshotOptions {
                full_page: true,
                mask: vec![page.locator("#items")],
                ..ScreenshotOptions::default()
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("mask"), "{tag}: {err}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn network_wait_and_route_overrides() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // wait_for_request resolves on a later matching request (the
        // delayed trigger fires strictly after the waiter arms).
        page.start_request_capture();
        page.evaluate_value(
            "setTimeout(() => fetch('api/hi').then(r => r.text()).then(t => window.__hi = t), 500); 1",
        )
        .await
        .unwrap();
        let seen = page
            .wait_for_request("api/hi", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(seen.method, "GET", "{tag}");
        assert!(seen.url.contains("api/hi"), "{tag}");
        page.wait_for_function("window.__hi !== undefined", Duration::from_secs(5))
            .await
            .unwrap();

        // wait_for_response resolves with the HTTP status.
        page.evaluate_value("setTimeout(() => fetch('nope-404').catch(() => 'failed'), 500); 1")
            .await
            .unwrap();
        let response = page
            .wait_for_response("nope-404", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(response.status, 404, "{tag}");

        // Nothing matches: loud timeout.
        let err = page
            .wait_for_request("never-happens-xyz", Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(matches!(err, E2eError::Timeout(..)), "{tag}: {err}");

        // URL override: /api/hi serves the echo-headers response
        // (Chromium-only; Firefox aborts redirected requests).
        if kind == BrowserKind::Chromium {
            page.route(vec![RouteRule::continue_with(
                "**/api/hi",
                Some(format!("{base}api/echo-headers")),
                None,
                None,
                None,
            )])
            .await
            .unwrap();
            let text: String = page
                .evaluate("fetch('api/hi').then(r => r.text())")
                .await
                .unwrap();
            assert!(text.contains("probe="), "{tag}: {text}");
        } else {
            let err = page
                .route(vec![RouteRule::continue_with(
                    "**/api/hi",
                    Some(format!("{base}api/echo-headers")),
                    None,
                    None,
                    None,
                )])
                .await
                .unwrap_err();
            assert!(
                err.to_string().contains("not supported on Firefox"),
                "{tag}: {err}"
            );
        }

        // Header override replaces the header set.
        page.route(vec![RouteRule::continue_with(
            "**/api/echo-headers",
            None,
            None,
            Some(vec![(
                "x-ferrite-probe".to_string(),
                "override-1".to_string(),
            )]),
            None,
        )])
        .await
        .unwrap();
        let text: String = page
            .evaluate("fetch('api/echo-headers').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("probe=override-1"), "{tag}: {text}");

        // Method override: the server sees POST.
        page.route(vec![RouteRule::continue_with(
            "**/api/method",
            None,
            Some("POST".to_string()),
            None,
            None,
        )])
        .await
        .unwrap();
        let text: String = page
            .evaluate("fetch('api/method').then(r => r.text())")
            .await
            .unwrap();
        assert_eq!(text, "POST", "{tag}");

        // Body override: the server sees the replacement bytes.
        page.route(vec![RouteRule::continue_with(
            "**/api/echo",
            None,
            None,
            None,
            Some(b"overridden".to_vec()),
        )])
        .await
        .unwrap();
        let text: String = page
            .evaluate("fetch('api/echo', { method: 'POST', body: 'original' }).then(r => r.text())")
            .await
            .unwrap();
        assert_eq!(text, "overridden", "{tag}");
        page.stop_routing().await;

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn files_and_downloads() {
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let tag = kind.name();
        let exe = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        };
        let Some(exe) = exe else {
            eprintln!("skipping {tag}: no executable found");
            continue;
        };
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-dl-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let browser = match Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(exe)
                .download_dir(dir.clone()),
        )
        .await
        {
            Ok(browser) => browser,
            Err(error) => {
                eprintln!("skipping {tag}: {error}");
                continue;
            }
        };
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Upload fixtures.
        let upload_dir = dir.join("uploads");
        std::fs::create_dir_all(&upload_dir).unwrap();
        let a = upload_dir.join("a.txt");
        let b = upload_dir.join("b.bin");
        std::fs::write(&a, "alpha").unwrap();
        std::fs::write(&b, [0u8, 1, 2, 3]).unwrap();

        // set_input_files via Page and Locator (empty list clears).
        page.set_input_files("#upload", &[&a, &b]).await.unwrap();
        let names: Vec<String> = page
            .evaluate("[...document.getElementById('upload').files].map(f => f.name)")
            .await
            .unwrap();
        assert_eq!(
            names,
            vec!["a.txt".to_string(), "b.bin".to_string()],
            "{tag}"
        );
        let mime: String = page
            .evaluate("document.getElementById('upload').files[0].type")
            .await
            .unwrap();
        assert_eq!(mime, "text/plain", "{tag}");
        page.locator("#upload")
            .set_input_files::<std::path::PathBuf>(&[])
            .await
            .unwrap();
        let len: i64 = page
            .evaluate("document.getElementById('upload').files.length")
            .await
            .unwrap();
        assert_eq!(len, 0, "{tag}");

        // Loud errors: missing file, wrong element.
        let err = page
            .set_input_files("#upload", &[upload_dir.join("nope.txt")])
            .await
            .unwrap_err();
        assert!(err.to_string().contains("nope.txt"), "{tag}: {err}");
        let err = page.set_input_files("#name", &[&a]).await.unwrap_err();
        assert!(err.to_string().contains("not a file input"), "{tag}: {err}");

        // Downloads land in the launch dir.
        if kind == BrowserKind::Chromium {
            page.set_download_dir(&dir).await.unwrap();
        } else {
            let err = page.set_download_dir(&dir).await.unwrap_err();
            assert!(err.to_string().contains("launch-wide"), "{tag}: {err}");
        }
        page.evaluate_value("window.location = 'download/report.txt'; true")
            .await
            .unwrap();
        let got = page
            .wait_for_download(&dir, Duration::from_secs(15))
            .await
            .unwrap();
        assert_eq!(got.file_name().unwrap(), "report.txt", "{tag}");
        assert_eq!(
            std::fs::read_to_string(&got).unwrap(),
            "ferrite download contents",
            "{tag}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn scripts_and_exposed_functions() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();

        // Init scripts run in future documents only, once per document.
        page.add_init_script("window.__initSeen = (window.__initSeen || 0) + 1;")
            .await
            .unwrap();
        let before: Option<i64> = page.evaluate("window.__initSeen ?? null").await.unwrap();
        assert_eq!(before, None, "{tag}");
        page.goto(&base).await.unwrap();
        let after: i64 = page.evaluate("window.__initSeen").await.unwrap();
        assert_eq!(after, 1, "{tag}");
        page.goto(&base).await.unwrap();
        // A fresh document starts undefined, so 1 proves the script ran again.
        let again: i64 = page.evaluate("window.__initSeen").await.unwrap();
        assert_eq!(again, 1, "{tag}");

        // Exposed functions round-trip args and results.
        page.expose_function("add", |args| {
            let sum: i64 = args.iter().filter_map(|value| value.as_i64()).sum();
            serde_json::json!(sum)
        })
        .await
        .unwrap();
        let sum: i64 = page.evaluate("window.add(2, 3)").await.unwrap();
        assert_eq!(sum, 5, "{tag}");

        // Panics reject the page promise.
        page.expose_function("boom", |_| -> serde_json::Value { panic!("rust panic") })
            .await
            .unwrap();
        let rejected: String = page
            .evaluate("window.boom().then(() => 'resolved', (error) => String(error))")
            .await
            .unwrap();
        assert!(rejected.contains("panicked"), "{tag}: {rejected}");

        // Bad names fail loudly.
        let err = page
            .expose_function("not an identifier", |_| serde_json::Value::Null)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("identifier"), "{tag}: {err}");

        // Clearing removes the globals.
        page.clear_exposed_functions().await;
        let gone: bool = page
            .evaluate("typeof window.add === 'undefined'")
            .await
            .unwrap();
        assert!(gone, "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn frames_listing_and_evaluate() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}frames")).await.unwrap();

        // The iframe may attach after the host load event.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let frames = loop {
            let frames = page.document_frames().await.unwrap();
            if frames.len() >= 2 {
                break frames;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "{tag}: only {} frame(s)",
                frames.len()
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        assert!(
            frames[0].url().contains("/frames"),
            "{tag}: {:?}",
            frames[0]
        );
        assert!(
            frames[1].url().contains("/assert"),
            "{tag}: {:?}",
            frames[1]
        );

        // Frame-scoped evaluate reads the iframe document, not the host.
        let inner = page.frame_by_url("assert").await.unwrap().unwrap();
        let title: String = inner.evaluate("document.title").await.unwrap();
        assert_eq!(title, "assert me", "{tag}");
        assert_eq!(page.title().await.unwrap(), "frames host", "{tag}");

        // Names are Chromium-only (Firefox reports none).
        if kind == BrowserKind::Chromium {
            assert_eq!(inner.name(), "inner", "{tag}");
            assert!(
                page.frame_by_name("inner").await.unwrap().is_some(),
                "{tag}"
            );
        } else {
            assert_eq!(inner.name(), "", "{tag}");
            assert!(
                page.frame_by_name("inner").await.unwrap().is_none(),
                "{tag}"
            );
        }
        assert!(
            page.frame_by_url("no-such-frame").await.unwrap().is_none(),
            "{tag}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn clock_fixed_time() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Fixed time needs an installed clock.
        let err = page.clock_set_fixed_time(1_000_000).await.unwrap_err();
        assert!(err.to_string().contains("clock_install"), "{tag}: {err}");
        page.clock_install().await.unwrap();
        page.clock_set_fixed_time(1_000_000).await.unwrap();
        let fixed: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(fixed, 1_000_000, "{tag}");
        page.clock_advance(500).await.unwrap();
        let advanced: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(advanced, 1_000_500, "{tag}");
        page.clock_uninstall().await.unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_tap_and_blur() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Touchscreen tap activates the button.
        page.locator("#inc").tap().await.unwrap();
        page.locator("#count").expect_text("1").await.unwrap();
        // Tapping nothing fails loudly.
        let err = page.locator("#nope").tap().await.unwrap_err();
        assert!(err.to_string().contains("#nope"), "{tag}: {err}");

        // Focus then blur moves the active element.
        page.locator("#name").focus().await.unwrap();
        let active: String = page.evaluate("document.activeElement.id").await.unwrap();
        assert_eq!(active, "name", "{tag}");
        page.locator("#name").blur().await.unwrap();
        let blurred: String = page
            .evaluate("document.activeElement.tagName")
            .await
            .unwrap();
        assert_eq!(blurred, "BODY", "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn browser_contexts_and_pages() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        assert!(browser.contexts().is_empty(), "{tag}");
        assert!(browser.pages().is_empty(), "{tag}");

        let first = browser.new_page().await.unwrap();
        assert_eq!(browser.contexts().len(), 1, "{tag}");
        assert_eq!(browser.pages().len(), 1, "{tag}");

        let context = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        assert_eq!(browser.contexts().len(), 2, "{tag}");
        let second = context.new_page().await.unwrap();
        let third = context.new_page().await.unwrap();
        assert_eq!(context.pages().len(), 2, "{tag}");
        assert_eq!(browser.pages().len(), 3, "{tag}");

        third.close().await.unwrap();
        assert_eq!(context.pages().len(), 1, "{tag}");
        assert_eq!(browser.pages().len(), 2, "{tag}");
        context.close().await.unwrap();
        assert_eq!(browser.contexts().len(), 1, "{tag}");
        assert_eq!(browser.pages().len(), 1, "{tag}");

        first.close().await.unwrap();
        second.close().await.unwrap();
        assert!(browser.pages().is_empty(), "{tag}");
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn script_and_style_tags() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.add_script_tag_content("window.__tagInline = 40 + 2;")
            .await
            .unwrap();
        let inline: i64 = page.evaluate("window.__tagInline").await.unwrap();
        assert_eq!(inline, 42, "{tag}");
        page.add_script_tag_url(&format!("{base}static/app.js"))
            .await
            .unwrap();
        let loaded: String = page.evaluate("window.__tagScript").await.unwrap();
        assert_eq!(loaded, "loaded", "{tag}");
        let err = page
            .add_script_tag_url(&format!("{base}static/missing.js"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("load failed"), "{tag}: {err}");

        page.add_style_tag_content("#name { color: rgb(4, 5, 6); }")
            .await
            .unwrap();
        let color: String = page
            .evaluate("getComputedStyle(document.getElementById('name')).color")
            .await
            .unwrap();
        assert_eq!(color, "rgb(4, 5, 6)", "{tag}");
        page.add_style_tag_url(&format!("{base}static/app.css"))
            .await
            .unwrap();
        let linked: String = page
            .evaluate("getComputedStyle(document.getElementById('name')).color")
            .await
            .unwrap();
        assert_eq!(linked, "rgb(7, 8, 9)", "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn network_capture_details() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.start_request_capture();

        page.evaluate_value(
            "setTimeout(() => fetch('api/echo', { method: 'POST', \
             headers: { 'x-detail-probe': 'seen' }, body: 'hello-body' }), 500); 1",
        )
        .await
        .unwrap();
        let seen = page
            .wait_for_response("api/echo", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(seen.method, "POST", "{tag}");
        assert!(
            seen.headers
                .iter()
                .any(|(name, value)| name.eq_ignore_ascii_case("x-detail-probe")
                    && value == "seen"),
            "{tag}: {:?}",
            seen.headers
        );
        // Request bodies are reported on Chromium only.
        if kind == BrowserKind::Chromium {
            assert_eq!(seen.post_data.as_deref(), Some("hello-body"), "{tag}");
        } else {
            assert_eq!(seen.post_data, None, "{tag}");
        }
        assert!(seen.duration_ms.is_some(), "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn frame_locators() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}frames")).await.unwrap();
        page.wait_for_function(
            "document.querySelector('iframe').contentDocument.readyState === 'complete'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        let inner = page.frame_by_url("assert").await.unwrap().unwrap();
        // Actions, getters, and assertions all run inside the frame.
        inner.locator("#btn").expect_visible().await.unwrap();
        let text = inner.locator("#btn").text().await.unwrap();
        assert_eq!(text, "Save", "{tag}");
        inner.locator("#txt").fill("grace").await.unwrap();
        let value = inner.locator("#txt").input_value().await.unwrap();
        assert_eq!(value, "grace", "{tag}");
        inner
            .get_by_role("button", "Save")
            .expect_visible()
            .await
            .unwrap();
        // The host document does not see iframe elements.
        assert_eq!(page.locator("#btn").count().await.unwrap(), 0, "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_drag_to() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}drag")).await.unwrap();

        page.locator("#drag")
            .drag_to(&page.locator("#drop"), 5)
            .await
            .unwrap();
        // Grab offset keeps the center within half the box of the target.
        let dragged = page.locator("#drag").state().await.unwrap().rects[0].clone();
        let target = page.locator("#drop").state().await.unwrap().rects[0].clone();
        let (cx, cy) = (
            dragged.x + dragged.width / 2.0,
            dragged.y + dragged.height / 2.0,
        );
        let (tx, ty) = (
            target.x + target.width / 2.0,
            target.y + target.height / 2.0,
        );
        assert!((cx - tx).abs() < 30.0, "{tag}: {cx} vs {tx}");
        assert!((cy - ty).abs() < 30.0, "{tag}: {cy} vs {ty}");

        // Misuse fails loudly.
        let other = browser.new_page().await.unwrap();
        let err = page
            .locator("#drag")
            .drag_to(&other.locator("#drop"), 5)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("same page"), "{tag}: {err}");
        let err = page
            .locator("#drag")
            .drag_to(&page.locator("#drop"), 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("at least 1 step"), "{tag}: {err}");

        other.close().await.unwrap();
        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn context_rules_apply_to_pages() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let context = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();

        // Context routes hit every page, current and future.
        context
            .route(vec![RouteRule::fulfill(
                "**/api/hi",
                200,
                r#"{"ctx":true}"#,
                "application/json",
            )])
            .await
            .unwrap();
        let one = context.new_page().await.unwrap();
        one.goto(&base).await.unwrap();
        let text: String = one
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"ctx\":true"), "{tag}: {text}");
        let two = context.new_page().await.unwrap();
        two.goto(&base).await.unwrap();
        let text: String = two
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"ctx\":true"), "{tag}: {text}");

        // Page rules win on overlap; other pages keep the context rule.
        one.route(vec![RouteRule::fulfill(
            "**/api/hi",
            200,
            r#"{"page":true}"#,
            "application/json",
        )])
        .await
        .unwrap();
        let text: String = one
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"page\":true"), "{tag}: {text}");
        let text: String = two
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"ctx\":true"), "{tag}: {text}");

        // Unrouting restores the real response (page rule still wins on one).
        assert_eq!(context.unroute("**/api/hi").await.unwrap(), 1, "{tag}");
        let text: String = two
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"real\":true"), "{tag}: {text}");
        let text: String = one
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"page\":true"), "{tag}: {text}");

        // Context cookies are visible to pages.
        context
            .add_cookies(
                &[Cookie {
                    name: "ctx".to_string(),
                    value: "yum".to_string(),
                    domain: None,
                    path: Some("/".to_string()),
                    http_only: false,
                    secure: false,
                    expires: None,
                }],
                &base,
            )
            .await
            .unwrap();
        let found = one
            .cookies()
            .await
            .unwrap()
            .into_iter()
            .find(|cookie| cookie.name == "ctx")
            .unwrap();
        assert_eq!(found.value, "yum", "{tag}");

        // Permissions inherit to future pages without their own call.
        context.grant_permissions(&["geolocation"]).await.unwrap();
        let three = context.new_page().await.unwrap();
        three.goto(&base).await.unwrap();
        three
            .evaluate_value(
                "navigator.permissions.query({name:'geolocation'}) \
                 .then(r => document.title = 'perm:' + r.state)",
            )
            .await
            .unwrap();
        three.expect().title("perm:granted").await.unwrap();

        // Geolocation override inherits too (Firefox: loud error when old).
        match context.set_geolocation(48.85, 2.35).await {
            Ok(()) => {
                let four = context.new_page().await.unwrap();
                four.goto(&base).await.unwrap();
                four.evaluate_value(
                    "navigator.geolocation.getCurrentPosition( \
                     p => document.title = 'geo:' + p.coords.latitude + ',' + p.coords.longitude, \
                     e => document.title = 'geo-err:' + e.code + ':' + e.message)",
                )
                .await
                .unwrap();
                four.expect().title("geo:48.85,2.35").await.unwrap();
                four.close().await.unwrap();
            }
            Err(error) => {
                assert_eq!(tag, "firefox", "{tag}: unexpected {error}");
                assert!(error.to_string().contains("newer build"), "{tag}: {error}");
            }
        }

        one.close().await.unwrap();
        two.close().await.unwrap();
        three.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn route_modify_response() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Firefox rejects every response-phase override (probe-verified on
        // 156: body, headers and statusCode are request-phase-only).
        if kind != BrowserKind::Chromium {
            let err = page
                .route(vec![RouteRule::modify_response(
                    "**/api/hi",
                    Some(418),
                    None,
                    Some(br#"{"mocked":true}"#.to_vec()),
                )])
                .await
                .unwrap_err();
            assert!(
                err.to_string().contains("not supported on Firefox"),
                "{tag}: {err}"
            );
            page.close().await.unwrap();
            browser.close().await.unwrap();
            shutdown.abort();
            continue;
        }

        // Full override: status + headers + body.
        page.route(vec![RouteRule::modify_response(
            "**/api/hi",
            Some(418),
            Some(vec![
                ("x-modified".to_string(), "yes".to_string()),
                ("content-type".to_string(), "application/json".to_string()),
            ]),
            Some(br#"{"mocked":true}"#.to_vec()),
        )])
        .await
        .unwrap();
        let probe: serde_json::Value = page
            .evaluate(
                "fetch('api/hi').then(async r => \
                 ({ status: r.status, body: await r.text(), \
                 header: r.headers.get('x-modified') }))",
            )
            .await
            .unwrap();
        assert_eq!(probe["status"], serde_json::json!(418), "{tag}: {probe}");
        assert_eq!(
            probe["body"],
            serde_json::json!(r#"{"mocked":true}"#),
            "{tag}: {probe}"
        );
        assert_eq!(probe["header"], serde_json::json!("yes"), "{tag}: {probe}");

        // Non-matching traffic passes through untouched.
        let method: String = page
            .evaluate("fetch('api/method').then(r => r.text())")
            .await
            .unwrap();
        assert_eq!(method, "GET", "{tag}");

        // Status-only edits merge over the real response.
        page.route(vec![RouteRule::modify_response(
            "**/api/hi",
            Some(500),
            None,
            None,
        )])
        .await
        .unwrap();
        let probe: serde_json::Value = page
            .evaluate(
                "fetch('api/hi').then(async r => \
                 ({ status: r.status, body: await r.text() }))",
            )
            .await
            .unwrap();
        assert_eq!(probe["status"], serde_json::json!(500), "{tag}: {probe}");
        assert!(
            probe["body"].as_str().unwrap_or_default().contains("real"),
            "{tag}: {probe}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn page_events_and_popups() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Console events.
        let (event, done) = tokio::join!(
            page.wait_for_event(PageEventKind::Console, Duration::from_secs(10)),
            page.evaluate_value("console.log('event-probe-1'); true")
        );
        done.unwrap();
        assert!(
            matches!(&event, Ok(PageEvent::Console(message)) if message.text.contains("event-probe-1")),
            "{tag}: {event:?}"
        );

        // Request + response events.
        let (event, done) = tokio::join!(
            page.wait_for_event(PageEventKind::Request, Duration::from_secs(10)),
            page.evaluate("fetch('api/hi').then(r => r.text())")
        );
        let _: String = done.unwrap();
        assert!(
            matches!(&event, Ok(PageEvent::Request { url, .. }) if url.ends_with("api/hi")),
            "{tag}: {event:?}"
        );
        let (event, done) = tokio::join!(
            page.wait_for_event(PageEventKind::Response, Duration::from_secs(10)),
            page.evaluate("fetch('api/hi').then(r => r.text())")
        );
        let _: String = done.unwrap();
        assert!(
            matches!(&event, Ok(PageEvent::Response { url, status: 200 })
                if url.ends_with("api/hi")),
            "{tag}: {event:?}"
        );

        // Dialog events (handling must be armed).
        page.handle_dialogs(true).await.unwrap();
        let (event, done) = tokio::join!(
            page.wait_for_event(PageEventKind::Dialog, Duration::from_secs(10)),
            page.evaluate("alert('hi-event'); 'done'")
        );
        let done: String = done.unwrap();
        assert_eq!(done, "done", "{tag}");
        assert!(
            matches!(&event, Ok(PageEvent::Dialog(dialog)) if dialog.message == "hi-event"),
            "{tag}: {event:?}"
        );
        page.stop_dialog_handling().await;

        // Popup adoption: the event carries a usable, registered page.
        let (event, done) = tokio::join!(
            page.wait_for_event(PageEventKind::Popup, Duration::from_secs(15)),
            page.evaluate_value("window.open('about:blank'); true")
        );
        done.unwrap();
        let PageEvent::Popup(popup) = event.unwrap() else {
            panic!("{tag}: expected popup");
        };
        popup.goto(&base).await.unwrap();
        assert_eq!(popup.title().await.unwrap(), "e2e fixture", "{tag}");
        assert!(
            browser
                .default_context()
                .pages()
                .iter()
                .any(|known| known.target_id() == popup.target_id()),
            "{tag}: popup not registered"
        );
        let (event, done) = tokio::join!(
            popup.wait_for_event(PageEventKind::Closed, Duration::from_secs(10)),
            popup.close()
        );
        done.unwrap();
        assert!(matches!(event, Ok(PageEvent::Closed)), "{tag}: {event:?}");

        // Download events (Firefox download dirs are launch-wide).
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-evdl-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if kind == BrowserKind::Chromium {
            page.set_download_dir(&dir).await.unwrap();
            let (event, done) = tokio::join!(
                page.wait_for_event(PageEventKind::Download, Duration::from_secs(15)),
                page.evaluate_value("window.location = 'download/report.txt'; true")
            );
            done.unwrap();
            assert!(
                matches!(&event, Ok(PageEvent::Download(path)) if path.is_file()),
                "{tag}: {event:?}"
            );
        } else {
            let exe = ferrite_e2e::find_firefox(None).unwrap();
            let dl_browser = Browser::launch(
                LaunchOptions::default()
                    .browser(kind)
                    .executable(exe)
                    .download_dir(dir.clone()),
            )
            .await
            .unwrap();
            let dl_page = dl_browser.new_page().await.unwrap();
            dl_page.goto(&base).await.unwrap();
            let (event, done) = tokio::join!(
                dl_page.wait_for_event(PageEventKind::Download, Duration::from_secs(15)),
                dl_page.evaluate_value("window.location = 'download/report.txt'; true")
            );
            done.unwrap();
            assert!(
                matches!(&event, Ok(PageEvent::Download(path)) if path.is_file()),
                "{tag}: {event:?}"
            );
            dl_page.close().await.unwrap();
            dl_browser.close().await.unwrap();
        }
        let _ = std::fs::remove_dir_all(&dir);

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn route_and_dialog_handlers() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Handler-style route: the handler sees method/URL/headers and
        // answers dynamically (Playwright `page.route(url, handler)`).
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&seen);
        page.route_with_handler("**/api/hi", move |info| {
            let record = Arc::clone(&record);
            async move {
                record.lock().unwrap().push((
                    info.method.clone(),
                    info.url.clone(),
                    info.headers.len(),
                ));
                Ok(RouteAction::fulfill(
                    200,
                    format!("handled:{}", info.method),
                    "text/plain",
                ))
            }
        })
        .await
        .unwrap();
        let body: String = page
            .evaluate("fetch('/api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert_eq!(body, "handled:GET", "{tag}");
        {
            let seen = seen.lock().unwrap();
            assert_eq!(seen.len(), 1, "{tag}");
            assert_eq!(seen[0].0, "GET", "{tag}");
            assert!(seen[0].1.ends_with("/api/hi"), "{tag}: {}", seen[0].1);
            assert!(seen[0].2 > 0, "{tag}: no headers captured");
        }

        // `unroute` removes handlers too; traffic passes through again.
        assert_eq!(page.unroute("**/api/hi").await.unwrap(), 1, "{tag}");
        let real: serde_json::Value = page
            .evaluate("fetch('/api/hi').then(r => r.json())")
            .await
            .unwrap();
        assert_eq!(real, serde_json::json!({"real": true}), "{tag}");

        // Dialog handler: per-dialog decisions with prompt text
        // (Playwright `page.on('dialog', ...)` + `dialog.accept(text)`).
        let dialogs = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&dialogs);
        page.handle_dialogs_with_handler(move |info| {
            record
                .lock()
                .unwrap()
                .push((info.dialog_type.clone(), info.message.clone()));
            DialogDecision::accept_with("typed")
        })
        .await
        .unwrap();
        let answer: String = page.evaluate("prompt('your name?')").await.unwrap();
        assert_eq!(answer, "typed", "{tag}");
        {
            let dialogs = dialogs.lock().unwrap();
            assert_eq!(
                dialogs.as_slice(),
                [("prompt".to_string(), "your name?".to_string())],
                "{tag}"
            );
        }
        page.stop_dialog_handling().await;

        // `RouteInfo::fetch` replays the request over plain HTTP
        // (Playwright `route.fetch`).
        let info = RouteInfo {
            url: format!("{base}api/method"),
            method: "GET".to_string(),
            headers: Vec::new(),
            post_data: None,
        };
        let response = info.fetch().await.unwrap();
        assert_eq!(response.status(), 200, "{tag}");
        assert_eq!(response.text(), "GET", "{tag}");

        // Handler-decided response edits: Chromium applies them, Firefox
        // loud-aborts (BiDi is request-phase-only).
        page.route_with_handler("**/api/echo", |_info| async {
            Ok(RouteAction::ModifyResponse {
                status: Some(418),
                headers: None,
                body: None,
            })
        })
        .await
        .unwrap();
        let probe: String = page
            .evaluate(
                "fetch('/api/echo', { method: 'POST', body: 'x' }) \
                 .then(r => String(r.status)).catch(() => 'failed')",
            )
            .await
            .unwrap();
        if kind == BrowserKind::Chromium {
            assert_eq!(probe, "418", "{tag}");
        } else {
            assert_eq!(probe, "failed", "{tag}");
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn jshandle_roundtrip() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // Remote object: serialize, drill into properties, run functions.
        let handle = page
            .evaluate_handle("({ a: 1, nested: { b: 'x' }, list: [1, 2, 3] })")
            .await
            .unwrap();
        assert!(!handle.is_primitive(), "{tag}");
        let root: serde_json::Value = handle.json_value().await.unwrap();
        assert_eq!(
            root,
            serde_json::json!({ "a": 1, "nested": { "b": "x" }, "list": [1, 2, 3] }),
            "{tag}"
        );
        let nested = handle.get_property("nested").await.unwrap();
        let nested_json: serde_json::Value = nested.json_value().await.unwrap();
        assert_eq!(nested_json, serde_json::json!({ "b": "x" }), "{tag}");
        let sum: i64 = handle
            .evaluate("(o) => o.list.reduce((x, y) => x + y, 0)")
            .await
            .unwrap();
        assert_eq!(sum, 6, "{tag}");
        nested.dispose().await.unwrap();
        handle.dispose().await.unwrap();

        // Primitives inline (no remote reference) but keep the same API.
        let number = page.evaluate_handle("40 + 2").await.unwrap();
        assert!(number.is_primitive(), "{tag}");
        assert_eq!(number.json_value::<i64>().await.unwrap(), 42, "{tag}");
        assert_eq!(
            number.evaluate::<i64>("(n) => n * 2").await.unwrap(),
            84,
            "{tag}"
        );
        number.dispose().await.unwrap();

        // DOM nodes are remote values too.
        let node = page
            .evaluate_handle("document.getElementById('title')")
            .await
            .unwrap();
        assert!(!node.is_primitive(), "{tag}");
        let text: String = node.evaluate("(el) => el.textContent").await.unwrap();
        assert_eq!(text, "hello ferrite", "{tag}");
        node.dispose().await.unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

/// Echo WebSocket server (`ws://127.0.0.1:<port>/socket`).
async fn serve_ws() -> (String, tokio::task::AbortHandle) {
    use futures::{SinkExt, StreamExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/socket", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
                    return;
                };
                let (mut write, mut read) = ws.split();
                while let Some(Ok(msg)) = read.next().await {
                    if msg.is_text() {
                        if write.send(msg).await.is_err() {
                            break;
                        }
                    } else if msg.is_close() {
                        let _ = write.send(msg).await;
                        break;
                    }
                }
            });
        }
    });
    (url, task.abort_handle())
}

#[tokio::test]
async fn har_export() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        page.start_request_capture();
        page.evaluate_value("fetch('api/hi').then(r => r.json())")
            .await
            .unwrap();
        page.evaluate_value("fetch('api/echo', { method: 'POST', body: 'x' }).then(r => r.text())")
            .await
            .unwrap();

        let path =
            std::env::temp_dir().join(format!("ferrite-har-{}-{tag}.har", std::process::id()));
        page.save_har(&path).unwrap();
        let har: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(har["log"]["version"], serde_json::json!("1.2"), "{tag}");
        assert_eq!(
            har["log"]["creator"]["name"],
            serde_json::json!("ferrite"),
            "{tag}"
        );
        let entries = har["log"]["entries"].as_array().unwrap();
        let hi = entries
            .iter()
            .find(|e| {
                e["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .ends_with("/api/hi")
            })
            .unwrap_or_else(|| panic!("{tag}: no /api/hi entry in {entries:?}"));
        assert_eq!(hi["request"]["method"], serde_json::json!("GET"), "{tag}");
        assert_eq!(hi["response"]["status"], serde_json::json!(200), "{tag}");
        assert!(
            hi["response"]["content"]["mimeType"]
                .as_str()
                .unwrap_or_default()
                .contains("application/json"),
            "{tag}: {hi}"
        );
        assert!(
            !hi["response"]["headers"].as_array().unwrap().is_empty(),
            "{tag}: no response headers"
        );
        assert!(
            hi["startedDateTime"]
                .as_str()
                .unwrap_or_default()
                .ends_with('Z'),
            "{tag}: {}",
            hi["startedDateTime"]
        );
        assert_eq!(hi["timings"]["wait"], hi["time"], "{tag}");

        // POST body metadata survives the round trip.
        let echo = entries
            .iter()
            .find(|e| {
                e["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .ends_with("/api/echo")
            })
            .unwrap_or_else(|| panic!("{tag}: no /api/echo entry"));
        assert_eq!(
            echo["request"]["method"],
            serde_json::json!("POST"),
            "{tag}"
        );
        if kind == BrowserKind::Chromium {
            assert_eq!(
                echo["request"]["postData"]["text"],
                serde_json::json!("x"),
                "{tag}"
            );
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn websocket_events() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let (ws_url, ws_shutdown) = serve_ws().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        // BiDi has no socket-frame events: waiting fails loudly.
        if kind != BrowserKind::Chromium {
            let err = page
                .wait_for_event(PageEventKind::WebSocket, Duration::from_secs(1))
                .await
                .unwrap_err();
            assert!(
                err.to_string().contains("not supported on Firefox"),
                "{tag}: {err}"
            );
            page.close().await.unwrap();
            browser.close().await.unwrap();
            shutdown.abort();
            ws_shutdown.abort();
            continue;
        }

        let mut events = page.subscribe();
        let script = format!(
            "(() => new Promise((resolve, reject) => {{ \
               const ws = new WebSocket('{ws_url}'); \
               ws.onopen = () => ws.send('ping-1'); \
               ws.onmessage = (e) => {{ ws.close(); resolve(e.data); }}; \
               ws.onerror = () => reject(new Error('ws failed')); \
             }}))()"
        );
        let echo = page.evaluate::<String>(&script).await;
        assert_eq!(echo.unwrap(), "ping-1", "{tag}");

        // Drain socket events until close (or timeout).
        let mut seen: Vec<(WebSocketDirection, String)> = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while tokio::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, events.recv()).await {
                Ok(Ok(PageEvent::WebSocket(event))) => {
                    assert!(event.url.ends_with("/socket"), "{tag}: {}", event.url);
                    let closed = event.direction == WebSocketDirection::Closed;
                    seen.push((event.direction, event.payload));
                    if closed {
                        break;
                    }
                }
                Ok(Ok(_)) => {}
                _ => break,
            }
        }
        assert!(
            seen.iter().any(|(d, _)| *d == WebSocketDirection::Created),
            "{tag}: {seen:?}"
        );
        assert!(
            seen.contains(&(WebSocketDirection::Sent, "ping-1".to_string())),
            "{tag}: {seen:?}"
        );
        assert!(
            seen.contains(&(WebSocketDirection::Received, "ping-1".to_string())),
            "{tag}: {seen:?}"
        );
        assert_eq!(
            seen.last().map(|(d, _)| *d),
            Some(WebSocketDirection::Closed),
            "{tag}: {seen:?}"
        );

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        ws_shutdown.abort();
    }
}

#[tokio::test]
async fn tracing_capture() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let context = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();

        // Stopping without starting is a loud error.
        let err = context
            .stop_tracing(std::env::temp_dir().join("ferrite-trace-unused.json"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not started"), "{tag}: {err}");

        context.start_tracing(TracingOptions::default().screenshots(true));
        let page = context.new_page().await.unwrap();
        page.start_request_capture();
        page.step("open fixture", page.goto(&base)).await.unwrap();
        page.step(
            "probe api",
            page.evaluate_value("fetch('api/hi').then(r => r.json())"),
        )
        .await
        .unwrap();
        page.step("log line", page.evaluate_value("console.log('traced'); 1"))
            .await
            .unwrap();

        let path =
            std::env::temp_dir().join(format!("ferrite-trace-{}-{tag}.json", std::process::id()));
        context.stop_tracing(&path).await.unwrap();
        let trace: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(trace["tool"], serde_json::json!("ferrite"), "{tag}");
        assert_eq!(
            trace["screenshots_enabled"],
            serde_json::json!(true),
            "{tag}"
        );
        let pages = trace["pages"].as_array().unwrap();
        assert_eq!(pages.len(), 1, "{tag}");
        let actions = pages[0]["actions"].as_array().unwrap();
        for name in ["open fixture", "probe api", "log line"] {
            assert!(
                actions
                    .iter()
                    .any(|a| a["detail"].as_str().unwrap_or_default().starts_with(name)),
                "{tag}: missing step {name} in {actions:?}"
            );
        }
        let console = pages[0]["console"].as_array().unwrap();
        assert!(
            console
                .iter()
                .any(|m| m["text"].as_str().unwrap_or_default().contains("traced")),
            "{tag}: {console:?}"
        );
        let requests = pages[0]["requests"].as_array().unwrap();
        assert!(
            requests
                .iter()
                .any(|r| r["url"].as_str().unwrap_or_default().ends_with("/api/hi")),
            "{tag}: {requests:?}"
        );
        let shots = trace["screenshots"].as_array().unwrap();
        assert_eq!(shots.len(), 3, "{tag}");
        assert_eq!(shots[0]["step"], serde_json::json!("open fixture"), "{tag}");
        for shot in shots {
            let png = shot["png_base64"].as_str().unwrap_or_default();
            assert!(png.starts_with("iVBOR"), "{tag}: not a PNG shot");
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn locator_state_checks() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&format!("{base}assert")).await.unwrap();

        assert!(page.locator("#btn").is_visible().await.unwrap(), "{tag}");
        assert!(!page.locator("#btn").is_hidden().await.unwrap(), "{tag}");
        assert!(page.locator("#nope").is_hidden().await.unwrap(), "{tag}");
        assert!(!page.locator("#nope").is_attached().await.unwrap(), "{tag}");
        assert!(page.locator("#btn").is_attached().await.unwrap(), "{tag}");
        assert!(page.locator("#txt").is_enabled().await.unwrap(), "{tag}");
        assert!(!page.locator("#txt").is_disabled().await.unwrap(), "{tag}");
        assert!(page.locator("#off").is_disabled().await.unwrap(), "{tag}");
        assert!(page.locator("#ed").is_editable().await.unwrap(), "{tag}");
        assert!(!page.locator("#btn").is_editable().await.unwrap(), "{tag}");

        page.locator("#txt").focus().await.unwrap();
        assert!(page.locator("#txt").is_focused().await.unwrap(), "{tag}");

        let bounds = page.locator("#btn").bounding_box().await.unwrap();
        assert!(bounds.is_some(), "{tag}");
        let bounds = bounds.unwrap();
        assert!(
            bounds.width > 0.0 && bounds.height > 0.0,
            "{tag}: {bounds:?}"
        );
        assert!(
            page.locator("#nope")
                .bounding_box()
                .await
                .unwrap()
                .is_none(),
            "{tag}"
        );

        page.locator("#btn").highlight().await.unwrap();
        let outline: String = page
            .locator("#btn")
            .evaluate("(el) => el.style.outline")
            .await
            .unwrap();
        assert!(outline.contains("solid"), "{tag}: {outline}");
        let id: String = page
            .locator("#btn")
            .evaluate("(el) => el.id")
            .await
            .unwrap();
        assert_eq!(id, "btn", "{tag}");

        page.goto(&base).await.unwrap();
        assert!(!page.locator("#agree").is_checked().await.unwrap(), "{tag}");
        page.locator("#agree").set_checked(true).await.unwrap();
        assert!(page.locator("#agree").is_checked().await.unwrap(), "{tag}");
        page.locator("#agree").set_checked(false).await.unwrap();
        assert!(!page.locator("#agree").is_checked().await.unwrap(), "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn page_wait_helpers_and_close() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        assert!(browser.is_connected(), "{tag}");
        let page = browser.new_page().await.unwrap();
        assert!(!page.is_closed(), "{tag}");
        page.goto(&base).await.unwrap();

        let found = page
            .wait_for_selector("#title", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(found.text().await.unwrap(), "hello ferrite", "{tag}");
        let err = page
            .wait_for_selector("#never-here", Duration::from_millis(300))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("wait for"), "{tag}: {err}");

        // Dialogs: arm-on-wait answers and reports the dialog.
        page.evaluate_value("setTimeout(() => alert('w4-hi'), 100); true")
            .await
            .unwrap();
        let dialog = page
            .wait_for_dialog(true, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(dialog.dialog_type, "alert", "{tag}");
        assert_eq!(dialog.message, "w4-hi", "{tag}");

        // Popups resolve to an adopted, usable page.
        let popup_url = format!("{base}assert");
        let open = format!("window.open('{popup_url}'); true");
        let (popup, done) = tokio::join!(
            page.wait_for_popup(Duration::from_secs(15)),
            page.evaluate_value(&open)
        );
        done.unwrap();
        let popup = popup.unwrap();
        popup
            .wait_for_function("document.title.length > 0", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(popup.title().await.unwrap(), "assert me", "{tag}");
        popup.close().await.unwrap();
        assert!(popup.is_closed(), "{tag}");

        page.close().await.unwrap();
        assert!(page.is_closed(), "{tag}");
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn clock_controls() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        let err = page.clock_now().await.unwrap_err();
        assert!(err.to_string().contains("clock_install"), "{tag}: {err}");
        page.clock_install().await.unwrap();
        let t0 = page.clock_now().await.unwrap();
        assert!(t0 > 0, "{tag}");

        page.clock_fast_forward(1_000).await.unwrap();
        assert_eq!(page.clock_now().await.unwrap(), t0 + 1_000, "{tag}");
        page.clock_run_for(500).await.unwrap();
        assert_eq!(page.clock_now().await.unwrap(), t0 + 1_500, "{tag}");

        page.clock_pause().await.unwrap();
        let paused: bool = page
            .evaluate("window.__ferriteClock.isPaused()")
            .await
            .unwrap();
        assert!(paused, "{tag}");
        page.clock_resume().await.unwrap();
        let paused: bool = page
            .evaluate("window.__ferriteClock.isPaused()")
            .await
            .unwrap();
        assert!(!paused, "{tag}");

        page.clock_set_system_time(1_700_000_000_000).await.unwrap();
        assert_eq!(page.clock_now().await.unwrap(), 1_700_000_000_000, "{tag}");
        let date_now: i64 = page.evaluate("Date.now()").await.unwrap();
        assert_eq!(date_now, 1_700_000_000_000, "{tag}");

        page.clock_uninstall().await.unwrap();
        let err = page.clock_pause().await.unwrap_err();
        assert!(err.to_string().contains("clock_install"), "{tag}: {err}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn permissions_geo_clear() {
    for (kind, browser) in browsers().await {
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.grant_permissions(&["notifications"]).await.unwrap();
        page.clear_permissions().await.unwrap();
        // Clearing twice is idempotent.
        page.clear_permissions().await.unwrap();

        if kind == BrowserKind::Chromium {
            page.set_geolocation(48.0, 2.0).await.unwrap();
            page.clear_geolocation().await.unwrap();
            page.clear_geolocation().await.unwrap();
        } else {
            // Firefox geo needs a recent build; clear is always safe.
            let _ = page.set_geolocation(48.0, 2.0).await;
            page.clear_geolocation().await.unwrap();
        }

        let context = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        let cpage = context.new_page().await.unwrap();
        cpage.goto(&base).await.unwrap();
        context.grant_permissions(&["notifications"]).await.unwrap();
        context.clear_permissions().await.unwrap();
        if kind == BrowserKind::Chromium {
            context.set_geolocation(48.0, 2.0).await.unwrap();
            context.clear_geolocation().await.unwrap();
        } else {
            context.clear_geolocation().await.unwrap();
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn download_object() {
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let tag = kind.name();
        let exe = match kind {
            BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
            BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
        };
        let Some(exe) = exe else {
            eprintln!("skipping {tag}: no executable found");
            continue;
        };
        let dir =
            std::env::temp_dir().join(format!("ferrite-e2e-dlobj-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let browser = match Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(exe)
                .download_dir(dir.clone()),
        )
        .await
        {
            Ok(browser) => browser,
            Err(error) => {
                eprintln!("skipping {tag}: {error}");
                continue;
            }
        };
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();
        if kind == BrowserKind::Chromium {
            page.set_download_dir(&dir).await.unwrap();
        }

        page.evaluate_value("window.location = 'download/report.txt'; true")
            .await
            .unwrap();
        let download = page
            .wait_for_download_file(&dir, Duration::from_secs(15))
            .await
            .unwrap();
        assert_eq!(download.suggested_filename, "report.txt", "{tag}");
        assert!(download.path.is_file(), "{tag}");

        let saved = download
            .save_as(dir.join("nested").join("copy.txt"))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&saved).unwrap(),
            "ferrite download contents",
            "{tag}"
        );
        download.delete().await.unwrap();
        assert!(!download.path.exists(), "{tag}");
        download.delete().await.unwrap();

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[tokio::test]
async fn response_body_and_aria() {
    for (_kind, browser) in browsers().await {
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        let body = page.response_body(&format!("{base}api/hi")).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json, serde_json::json!({"real": true}));
        let err = page
            .response_body(&format!("{base}api/missing"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("fetch"), "{err}");

        page.goto(&format!("{base}locate")).await.unwrap();
        let snapshot = page.aria_snapshot().await.unwrap();
        assert!(snapshot.contains("button"), "{snapshot}");
        assert!(snapshot.contains("Sign in"), "{snapshot}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn unroute_all_clears() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        page.route(vec![
            RouteRule::fulfill("**/api/hi", 200, r#"{"mock":1}"#, "application/json"),
            RouteRule::abort("**/api/echo"),
        ])
        .await
        .unwrap();
        page.route_with_handler("**/api/method", |info| async move {
            Ok(RouteAction::fulfill(
                200,
                format!("saw {}", info.method),
                "text/plain",
            ))
        })
        .await
        .unwrap();
        let removed = page.unroute_all().await.unwrap();
        assert_eq!(removed, 3, "{tag}");
        assert_eq!(page.unroute_all().await.unwrap(), 0, "{tag}");
        let text: String = page
            .evaluate("fetch('api/hi').then(r => r.text())")
            .await
            .unwrap();
        assert!(text.contains("\"real\":true"), "{tag}: {text}");

        let context = browser
            .new_context(ferrite_e2e::ContextOptions::default())
            .await
            .unwrap();
        context
            .route(vec![RouteRule::fulfill(
                "**/api/hi",
                200,
                r#"{"ctx":1}"#,
                "application/json",
            )])
            .await
            .unwrap();
        assert_eq!(context.unroute_all().await.unwrap(), 1, "{tag}");

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn http_credentials() {
    for (kind, browser) in browsers().await {
        let tag = kind.name();
        let (base, shutdown) = serve().await;
        let page = browser.new_page().await.unwrap();
        page.goto(&base).await.unwrap();

        if kind == BrowserKind::Chromium {
            page.set_http_credentials(Some("ada"), Some("s3cret"))
                .await
                .unwrap();
            let auth: String = page
                .evaluate("fetch('api/auth').then(r => r.text())")
                .await
                .unwrap();
            assert!(auth.starts_with("Basic "), "{tag}: {auth}");
            page.set_http_credentials(None, None).await.unwrap();
            let auth: String = page
                .evaluate("fetch('api/auth').then(r => r.text())")
                .await
                .unwrap();
            assert_eq!(auth, "absent", "{tag}");
            let err = page
                .set_http_credentials(Some("ada"), None)
                .await
                .unwrap_err();
            assert!(err.to_string().contains("both username"), "{tag}: {err}");
        } else {
            let err = page
                .set_http_credentials(Some("ada"), Some("s3cret"))
                .await
                .unwrap_err();
            assert!(
                err.to_string()
                    .contains("extra HTTP headers are not supported"),
                "{tag}: {err}"
            );
        }

        page.close().await.unwrap();
        browser.close().await.unwrap();
        shutdown.abort();
    }
}

#[tokio::test]
async fn runner_before_all_after_all() {
    let browsers = browsers().await;
    if browsers.is_empty() {
        eprintln!("skipping runner_before_all_after_all: no browser");
        return;
    }
    let order = Arc::new(Mutex::new(Vec::<String>::new()));
    let mark = order.clone();
    let runner = Runner::default()
        .workers(1)
        .list_progress(false)
        .before_all(move || {
            let mark = mark.clone();
            async move {
                mark.lock().unwrap().push("before_all".to_string());
                Ok(())
            }
        });
    let mark = order.clone();
    let runner = runner.after_all(move || {
        let mark = mark.clone();
        async move {
            mark.lock().unwrap().push("after_all".to_string());
            Ok(())
        }
    });
    let mark = order.clone();
    let runner = runner.before_each(move |_page| {
        let mark = mark.clone();
        async move {
            mark.lock().unwrap().push("before_each".to_string());
            Ok(())
        }
    });
    let (_kind, browser) = &browsers[0];
    let report = runner
        .run(
            browser,
            vec![test("w4 hooks", |page| async move {
                assert!(!page.is_closed());
                Ok(())
            })],
        )
        .await;
    assert_eq!(report.passed(), 1);
    assert_eq!(
        order.lock().unwrap().clone(),
        vec!["before_all", "before_each", "after_all"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );

    // Failing before_all aborts with one failed result.
    let failing = Runner::default()
        .list_progress(false)
        .before_all(|| async { Err(E2eError::Config("boom".to_string())) });
    let report = failing
        .run(
            browser,
            vec![test("never runs", |_page| async move { Ok(()) })],
        )
        .await;
    assert_eq!(report.failed(), 1);
    assert_eq!(report.results[0].name, "<before_all>");

    for (_kind, browser) in browsers {
        browser.close().await.unwrap();
    }
}
