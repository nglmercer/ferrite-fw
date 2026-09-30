//! Real gated font resources and native animated rendering under snapshot clocks.
use ferrite_e2e::*;
use image::GenericImageView;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{watch, Semaphore},
    task::{JoinHandle, JoinSet},
};

const FONT: &[u8] = include_bytes!("fixtures/snapshot-probe.ttf");
const FONT_CSS: &str = "@font-face{font-family:FerriteSnapshotProbe;src:url('/font.ttf') format('truetype');font-display:swap}html,body{margin:0}#probe{display:inline-block;font-family:FerriteSnapshotProbe,monospace;font-size:40px;line-height:40px;color:black}";

struct FontServer {
    base: String,
    release: Arc<Semaphore>,
    requests: watch::Receiver<usize>,
    task: Option<JoinHandle<()>>,
}
impl FontServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let release = Arc::new(Semaphore::new(0));
        let (requests, received) = watch::channel(0);
        let gate = release.clone();
        let task = tokio::spawn(async move {
            let mut handlers = JoinSet::new();
            loop {
                tokio::select! {
                    accepted=listener.accept()=>{
                        let (stream, _) = accepted.unwrap();
                        handlers.spawn(serve(stream, gate.clone(), requests.clone()));
                    }
                    Some(_)=handlers.join_next(), if !handlers.is_empty()=>{},
                }
            }
        });
        Self {
            base,
            release,
            requests: received,
            task: Some(task),
        }
    }
    async fn requested(&mut self) {
        tokio::time::timeout(
            Duration::from_secs(3),
            self.requests.wait_for(|count| *count > 0),
        )
        .await
        .unwrap()
        .unwrap();
    }
    fn deliver(&self) {
        self.release.add_permits(1);
    }
    async fn stop(mut self) {
        self.release.close();
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for FontServer {
    fn drop(&mut self) {
        self.release.close();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn serve(
    mut stream: TcpStream,
    release: Arc<Semaphore>,
    requests: watch::Sender<usize>,
) -> std::io::Result<()> {
    let mut input = Vec::new();
    loop {
        let mut buffer = [0; 1024];
        let length = stream.read(&mut buffer).await?;
        if length == 0 {
            return Ok(());
        }
        input.extend_from_slice(&buffer[..length]);
        if input.windows(4).any(|part| part == b"\r\n\r\n") {
            break;
        }
        if input.len() > 8192 {
            return Ok(());
        }
    }
    let text = String::from_utf8_lossy(&input);
    let path = text.split_whitespace().nth(1).unwrap_or("");
    let (content_type, body) = if path == "/font.ttf" {
        requests.send_modify(|count| *count += 1);
        let Ok(permit) = release.acquire().await else {
            return Ok(());
        };
        permit.forget();
        ("font/ttf", FONT.to_vec())
    } else {
        let body = match path {
            "/plain" => format!("<!doctype html><style>{FONT_CSS}</style><span id=probe>FFFF</span>"),
            "/shadow" => format!("<!doctype html><style>{FONT_CSS}</style><div id=host></div><script>document.getElementById('host').attachShadow({{mode:'open'}}).innerHTML='<style>#probe{{display:inline-block;font-family:FerriteSnapshotProbe,monospace;font-size:40px;line-height:40px;color:black}}</style><span id=probe>FFFF</span>'</script>"),
            "/iframe" => "<!doctype html><style>html,body{margin:0}iframe{border:0;width:250px;height:80px}</style><iframe src='/plain'></iframe>".into(),
            _ => "".into(),
        };
        ("text/html; charset=utf-8", body.into_bytes())
    };
    stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len()).as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await
}
async fn browsers() -> Vec<Browser> {
    let mut browsers = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let path = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        if let Some(path) = path {
            let browser = Browser::launch(LaunchOptions::default().browser(kind).executable(path))
                .await
                .unwrap();
            eprintln!(
                "snapshot fonts/animations {} {}",
                kind.name(),
                browser.version().await.unwrap()
            );
            browsers.push(browser);
        }
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(browsers.len(), 2);
    }
    browsers
}
async fn load(page: &Page, server: &mut FontServer, kind: &str) -> Locator {
    page.set_viewport(Viewport {
        width: 300,
        height: 120,
    })
    .await
    .unwrap();
    page.goto_with_options(
        &format!("{}/{kind}", server.base),
        NavigationOptions {
            wait_until: LoadState::DomContentLoaded,
            timeout: Some(Duration::from_secs(5)),
        },
    )
    .await
    .unwrap();
    server.requested().await;
    page.wait_for_function(
        if kind == "iframe" {
            "document.querySelector('iframe')?.contentDocument?.fonts.status==='loading'"
        } else {
            "document.fonts.status==='loading'"
        },
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    if kind == "iframe" {
        page.frame_locator("iframe").locator("#probe")
    } else {
        page.locator("#probe")
    }
}
fn options(dir: &tempfile::TempDir) -> SnapshotOptions {
    SnapshotOptions {
        dir: Some(dir.path().into()),
        update: Some(SnapshotUpdate::All),
        ..Default::default()
    }
}
fn dimensions(path: impl AsRef<std::path::Path>) -> (u32, u32) {
    image::load_from_memory(&std::fs::read(path).unwrap())
        .unwrap()
        .dimensions()
}

#[tokio::test]
async fn real_font_responses_gate_page_locator_shadow_and_same_origin_frame_baselines() {
    for browser in browsers().await {
        for (kind, page_capture) in [
            ("plain", true),
            ("plain", false),
            ("shadow", true),
            ("shadow", false),
            ("iframe", true),
            ("iframe", false),
        ] {
            let mut server = FontServer::start().await;
            let page = browser.new_page().await.unwrap();
            let probe = load(&page, &mut server, kind).await;
            let fallback = probe.screenshot().await.unwrap();
            assert_ne!(
                image::load_from_memory(&fallback).unwrap().dimensions(),
                (160, 40)
            );
            let dir = tempfile::tempdir().unwrap();
            let options = options(&dir);
            let locator_expectation = probe.expect().timeout(Duration::from_secs(3));
            let page_expectation = page.expect().timeout(Duration::from_secs(3));
            let capture = async {
                if page_capture {
                    page_expectation.screenshot_with("font", &options).await
                } else {
                    locator_expectation.screenshot_with("font", &options).await
                }
            };
            tokio::pin!(capture);
            tokio::select! {
                result=&mut capture=>panic!("assertion must wait for a real pending font response: {kind} {result:?}"),
                _=tokio::time::sleep(Duration::from_millis(200))=>{},
            }
            assert!(!dir.path().join("font.png").exists());
            server.deliver();
            tokio::time::timeout(Duration::from_secs(3), &mut capture)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                dimensions(dir.path().join("font.png")),
                if page_capture { (300, 120) } else { (160, 40) }
            );
            assert!(probe
                .evaluate::<bool>("el=>el.ownerDocument.fonts.check('40px FerriteSnapshotProbe')")
                .await
                .unwrap());
            page.expect()
                .screenshot_with("page", &options)
                .await
                .unwrap();
            assert_eq!(dimensions(dir.path().join("page.png")), (300, 120));
            page.close().await.unwrap();
            server.stop().await;
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn font_opt_out_captures_fallback_and_default_changed_capture_replaces_it_after_loading() {
    for browser in browsers().await {
        let mut server = FontServer::start().await;
        let page = browser.new_page().await.unwrap();
        let probe = load(&page, &mut server, "plain").await;
        let dir = tempfile::tempdir().unwrap();
        let mut options = options(&dir);
        options.wait_for_fonts = false;
        probe
            .expect()
            .timeout(Duration::from_secs(2))
            .screenshot_with("font", &options)
            .await
            .unwrap();
        let before = std::fs::read(dir.path().join("font.png")).unwrap();
        assert_ne!(
            image::load_from_memory(&before).unwrap().dimensions(),
            (160, 40)
        );
        assert_eq!(
            page.evaluate::<String>("document.fonts.status")
                .await
                .unwrap(),
            "loading"
        );
        options.wait_for_fonts = true;
        options.update = Some(SnapshotUpdate::Changed);
        server.deliver();
        probe
            .expect()
            .screenshot_with("font", &options)
            .await
            .unwrap();
        assert_eq!(dimensions(dir.path().join("font.png")), (160, 40));
        assert_ne!(std::fs::read(dir.path().join("font.png")).unwrap(), before);
        assert_eq!(
            page.evaluate::<String>("document.fonts.status")
                .await
                .unwrap(),
            "loaded"
        );
        page.close().await.unwrap();
        server.stop().await;
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn real_blocked_fonts_timeout_cancel_and_dispose_without_overwriting_baselines() {
    for browser in browsers().await {
        for control in ["timeout", "cancel", "dispose"] {
            let mut server = FontServer::start().await;
            let page = browser.new_page().await.unwrap();
            let probe = load(&page, &mut server, "plain").await;
            let dir = tempfile::tempdir().unwrap();
            let mut options = options(&dir);
            let fallback = probe.screenshot().await.unwrap();
            assert_snapshot_png("font", &fallback, &options).unwrap();
            options.capture = Some(ScreenshotOptions {
                style: Some("#probe{color:green!important}".into()),
                ..Default::default()
            });
            let token = CancellationToken::new();
            let scoped = page.with_cancellation(token.clone());
            let expectation = scoped.expect().timeout(if control == "timeout" {
                Duration::from_millis(350)
            } else {
                Duration::ZERO
            });
            let capture = expectation.screenshot_with("font", &options);
            tokio::pin!(capture);
            tokio::select! {
                result=&mut capture=>panic!("font response must remain pending: {result:?}"),
                prepared=page.wait_for_function("getComputedStyle(document.getElementById('probe')).color==='rgb(0, 128, 0)'", Duration::from_secs(2))=>prepared.unwrap(),
            }
            if control == "cancel" {
                token.cancel();
            }
            if control == "dispose" {
                page.close().await.unwrap();
            }
            let error = tokio::time::timeout(Duration::from_secs(2), &mut capture)
                .await
                .unwrap()
                .unwrap_err();
            if control == "timeout" {
                assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
            } else {
                assert!(matches!(
                    error,
                    E2eError::Cancelled(_) | E2eError::Disconnected(_)
                ));
            }
            assert_eq!(
                std::fs::read(dir.path().join("font.png")).unwrap(),
                fallback
            );
            assert!(!dir.path().join("font.actual.png").exists());
            if control != "dispose" {
                // The public basic capture does not wait for fonts and queues
                // behind owned restoration. No second font response is needed.
                page.screenshot(ScreenshotOptions::default()).await.unwrap();
                assert_eq!(
                    page.evaluate::<String>(
                        "getComputedStyle(document.getElementById('probe')).color"
                    )
                    .await
                    .unwrap(),
                    "rgb(0, 0, 0)"
                );
                server.deliver();
                let recovery = SnapshotOptions {
                    capture: None,
                    ..options.clone()
                };
                probe
                    .expect()
                    .screenshot_with("font", &recovery)
                    .await
                    .unwrap();
                assert_eq!(dimensions(dir.path().join("font.png")), (160, 40));
                page.close().await.unwrap();
            }
            server.stop().await;
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn default_css_animation_capture_is_stable_and_explicit_live_animation_is_diagnosed() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 120,
            height: 80,
        })
        .await
        .unwrap();
        for repeat in ["1", "infinite"] {
            let markup = format!("<style>body{{margin:0}}@keyframes paint{{from{{background:red}}to{{background:blue}}}}#patch{{width:80px;height:60px;background:red;animation:paint 10s linear {repeat} forwards}}</style><div id=patch></div><script>globalThis.ends=0;document.getElementById('patch').addEventListener('animationend',()=>ends++)</script>");
            page.set_content(&markup).await.unwrap();
            let dir = tempfile::tempdir().unwrap();
            let options = options(&dir);
            let patch = page.locator("#patch");
            patch
                .expect()
                .timeout(Duration::from_secs(2))
                .screenshot_with("animation", &options)
                .await
                .unwrap();
            let first = std::fs::read(dir.path().join("animation.png")).unwrap();
            eprintln!(
                "native {} CSS animation {repeat} snapshot pixel {:?}",
                browser.kind().name(),
                image::load_from_memory(&first)
                    .unwrap()
                    .to_rgba8()
                    .get_pixel(5, 5)
                    .0
            );
            assert_eq!(
                image::load_from_memory(&first)
                    .unwrap()
                    .to_rgba8()
                    .get_pixel(5, 5)
                    .0,
                if repeat == "1" {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                }
            );
            assert_eq!(
                page.evaluate::<String>("document.getAnimations()[0].playState")
                    .await
                    .unwrap(),
                if repeat == "1" { "finished" } else { "running" }
            );
            if repeat == "1" {
                assert!(page.evaluate::<u32>("ends").await.unwrap() > 0);
            }
            assert_eq!(
                image::load_from_memory(&first).unwrap().dimensions(),
                (80, 60)
            );
            patch
                .expect()
                .timeout(Duration::from_secs(2))
                .screenshot_with(
                    "animation",
                    &SnapshotOptions {
                        update: Some(SnapshotUpdate::None),
                        ..options.clone()
                    },
                )
                .await
                .unwrap();
            assert_eq!(
                page.evaluate::<String>(
                    "getComputedStyle(document.getElementById('patch')).animationDuration"
                )
                .await
                .unwrap(),
                "10s"
            );
            let live = SnapshotOptions {
                capture: Some(ScreenshotOptions::default()),
                ..options
            };
            // Finite disabled animations really finish. Use a fresh document
            // to measure explicitly allowed live playback, rather than its endpoint.
            page.set_content(&markup).await.unwrap();
            let error = patch
                .expect()
                .timeout(Duration::from_millis(600))
                .screenshot_with("live", &live)
                .await
                .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_EXPECT");
            assert!(error.to_string().contains("consecutive stable"), "{error}");
            assert!(!dir.path().join("live.png").exists());
            assert!(dir.path().join("live.actual.png").is_file());
            assert_eq!(
                std::fs::read(dir.path().join("animation.png")).unwrap(),
                first
            );
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn web_animation_objects_finish_resume_preserve_zero_rate_and_surface_native_method_errors() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let options = options(&dir);
        for kind in [
            "finite",
            "infinite",
            "zero-rate",
            "failure",
            "resume-failure",
        ] {
            page.set_content("<style>body{margin:0}#patch{width:80px;height:60px;background:red}</style><div id=patch></div>").await.unwrap();
            let infinite = matches!(kind, "infinite" | "resume-failure");
            page.evaluate_value(&format!("globalThis.animation=document.getElementById('patch').animate([{{background:'red'}},{{background:'blue'}}],{{duration:10000,iterations:{},fill:'forwards'}});animation.finished.catch(()=>{{}});true", if infinite {"Infinity"} else {"1"})).await.unwrap();
            let patch = page.locator("#patch");
            if kind == "zero-rate" {
                page.evaluate_value("animation.playbackRate=0;animation.currentTime=5000;true")
                    .await
                    .unwrap();
            }
            let before = patch.screenshot().await.unwrap();
            if kind == "failure" {
                page.evaluate_value("globalThis.finish=animation.finish;animation.finish=()=>{throw Error('native finish rejected')};true").await.unwrap();
                let error = patch
                    .expect()
                    .screenshot_with(kind, &options)
                    .await
                    .unwrap_err();
                assert_ne!(error.code(), "FERRITE_E2E_EXPECT");
                assert!(error.to_string().contains("native finish rejected"));
                assert!(!dir.path().join("failure.png").exists());
                assert_eq!(
                    page.evaluate::<u32>(
                        "document.querySelectorAll('[data-ferrite-screenshot]').length"
                    )
                    .await
                    .unwrap(),
                    0
                );
                page.evaluate_value("animation.finish=finish;true")
                    .await
                    .unwrap();
            }
            if kind == "resume-failure" {
                page.evaluate_value("globalThis.play=animation.play;animation.play=()=>{throw Error('native resume rejected')};true").await.unwrap();
                let error = patch
                    .expect()
                    .screenshot_with(kind, &options)
                    .await
                    .unwrap_err();
                assert_ne!(error.code(), "FERRITE_E2E_EXPECT");
                assert!(error.to_string().contains("native resume rejected"));
                assert!(!dir.path().join("resume-failure.png").exists());
                assert_eq!(
                    page.evaluate::<u32>(
                        "document.querySelectorAll('[data-ferrite-screenshot]').length"
                    )
                    .await
                    .unwrap(),
                    0
                );
                let errors = page.take_screenshot_cleanup_errors();
                assert_eq!(errors.len(), 1);
                assert!(errors[0].contains("native resume rejected"));
                page.evaluate_value("animation.play=play;animation.play();true")
                    .await
                    .unwrap();
            }
            patch
                .expect()
                .screenshot_with(kind, &options)
                .await
                .unwrap();
            let bytes = std::fs::read(dir.path().join(format!("{kind}.png"))).unwrap();
            if kind == "zero-rate" {
                assert_eq!(compare_png(&bytes, &before, 0).unwrap().diff_pixels, 0);
                assert_eq!(
                    page.evaluate::<f64>("animation.playbackRate")
                        .await
                        .unwrap(),
                    0.0
                );
                assert_eq!(
                    page.evaluate::<f64>("animation.currentTime").await.unwrap(),
                    5000.0
                );
            } else {
                assert_eq!(
                    image::load_from_memory(&bytes)
                        .unwrap()
                        .to_rgba8()
                        .get_pixel(5, 5)
                        .0,
                    if infinite {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 255]
                    }
                );
                assert_eq!(
                    page.evaluate::<String>("animation.playState")
                        .await
                        .unwrap(),
                    if infinite { "running" } else { "finished" }
                );
            }
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn css_animations_started_during_real_font_waits_are_owned_and_resume_after_cancellation() {
    for browser in browsers().await {
        for kind in ["plain", "shadow", "iframe"] {
            let mut server = FontServer::start().await;
            let page = browser.new_page().await.unwrap();
            let probe = load(&page, &mut server, kind).await;
            let dir = tempfile::tempdir().unwrap();
            let token = CancellationToken::new();
            let scoped = page.with_cancellation(token.clone());
            let options = SnapshotOptions {
                capture: Some(ScreenshotOptions {
                    disable_animations: true,
                    style: Some("#probe{color:green!important}".into()),
                    ..Default::default()
                }),
                ..options(&dir)
            };
            let expectation = scoped.expect().timeout(Duration::ZERO);
            let capture = expectation.screenshot_with("font", &options);
            tokio::pin!(capture);
            let prepared = async {
                loop {
                    if probe
                        .evaluate::<String>("el=>getComputedStyle(el).color")
                        .await
                        .unwrap()
                        == "rgb(0, 128, 0)"
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            };
            tokio::select! {
                result=&mut capture=>panic!("font should remain pending: {result:?}"),
                ready=tokio::time::timeout(Duration::from_secs(2), prepared)=>ready.unwrap(),
            }
            probe.evaluate::<bool>("el=>{const root=el.getRootNode(),style=el.ownerDocument.createElement('style');style.textContent='@keyframes late{from{background:red}to{background:blue}}';(root.head||root).appendChild(style);el.style.background='red';el.style.animation='late 10s linear infinite';el.ownerDocument.defaultView.captureAnimation=el.getAnimations()[0];return true}").await.unwrap();
            page.wait_for_function(if kind == "iframe" {"document.querySelector('iframe').contentWindow.captureAnimation?.playState==='idle'"} else {"captureAnimation?.playState==='idle'"}, Duration::from_secs(2)).await.unwrap();
            token.cancel();
            let error = tokio::time::timeout(Duration::from_secs(2), &mut capture)
                .await
                .unwrap()
                .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CANCELLED");
            page.screenshot(ScreenshotOptions::default()).await.unwrap();
            assert_eq!(
                probe
                    .evaluate::<String>(
                        "el=>el.ownerDocument.defaultView.captureAnimation.playState"
                    )
                    .await
                    .unwrap(),
                "running"
            );
            assert_eq!(
                probe
                    .evaluate::<String>("el=>getComputedStyle(el).color")
                    .await
                    .unwrap(),
                "rgb(0, 0, 0)"
            );
            assert_eq!(
                probe
                    .evaluate::<u32>(
                        "el=>el.getRootNode().querySelectorAll('[data-ferrite-screenshot]').length"
                    )
                    .await
                    .unwrap(),
                0
            );
            assert!(!dir.path().join("font.png").exists());
            page.close().await.unwrap();
            server.stop().await;
        }
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn animation_limit_rejects_overflow_and_restores_every_cancelled_object() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_viewport(Viewport {
            width: 20,
            height: 20,
        })
        .await
        .unwrap();
        for count in [4096, 4097] {
            page.set_content("<style>body{margin:0}#patch{width:20px;height:20px;background:red}</style><div id=patch></div>").await.unwrap();
            page.evaluate_value(&format!("globalThis.animations=Array.from({{length:{count}}},()=>{{const animation=document.getElementById('patch').animate([{{opacity:1}},{{opacity:1}}],{{duration:1000000,iterations:Infinity}});animation.finished.catch(()=>{{}});return animation}});true")).await.unwrap();
            assert_eq!(
                page.evaluate::<usize>("document.getAnimations().length")
                    .await
                    .unwrap(),
                count
            );
            let result = page
                .screenshot(ScreenshotOptions {
                    disable_animations: true,
                    timeout: Some(Duration::from_secs(10)),
                    ..Default::default()
                })
                .await;
            if count == 4096 {
                assert_eq!(
                    image::load_from_memory(&result.unwrap())
                        .unwrap()
                        .dimensions(),
                    (20, 20)
                );
            } else {
                let error = result.unwrap_err();
                assert_ne!(error.code(), "FERRITE_E2E_EXPECT");
                assert!(
                    error.to_string().contains("4096 animation limit"),
                    "{error}"
                );
            }
            assert_eq!(
                page.evaluate::<usize>(
                    "animations.filter(animation=>animation.playState==='running').length"
                )
                .await
                .unwrap(),
                count
            );
            assert_eq!(
                page.evaluate::<usize>(
                    "document.querySelectorAll('[data-ferrite-screenshot]').length"
                )
                .await
                .unwrap(),
                0
            );
            assert!(page.take_screenshot_cleanup_errors().is_empty());
            // A capture after the rejected preparation proves that ownership/gate cleanup settled.
            assert_eq!(
                image::load_from_memory(
                    &page.screenshot(ScreenshotOptions::default()).await.unwrap()
                )
                .unwrap()
                .dimensions(),
                (20, 20)
            );
        }
        browser.close().await.unwrap();
    }
}
