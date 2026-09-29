//! Browser-driven integration tests, parametrized over engines.
//!
//! Each test runs against every available engine (Chromium, Firefox) and
//! skips engines with no installed browser:
//!
//! ```bash
//! FERRITE_CHROMIUM_PATH=/tmp/chrome-headless-shell-linux64/chrome-headless-shell \
//!   cargo test -p ferrite-e2e --test browser
//! ```

use std::time::Duration;

use ferrite_e2e::{
    describe, test, Browser, BrowserKind, ColorScheme, LaunchOptions, LoadState, NavigationOptions,
    Page, RecordedRequest, ReducedMotion, RouteRule, Runner, TestStatus, Timeout, VideoMode,
    VideoOptions,
};

const FIXTURE: &str = r#"<!doctype html><html><head><title>e2e fixture</title></head><body>
<h1 id="title">hello ferrite</h1>
<button id="inc">inc</button>
<span id="count">0</span>
<input id="name" />
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

const LOCATE_FIXTURE: &str = r#"<!doctype html><html><head><title>locate me</title></head><body>
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
</body></html>"#;

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
            "/api/hi",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"real": true})) }),
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
            .err()
            .expect("or_ across pages must fail");
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
