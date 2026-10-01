use ferrite_e2e::*;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const CONTENT: &str = r#"<!doctype html><title>Ferrite print</title><style>
html,body{margin:0;padding:0;font-family:Arial,sans-serif}#swatch{width:96px;height:48px;background:rgb(20,80,210)}
h1{font-size:20px;margin:16px 0 10px}p{font-size:12px}.print{display:none}@media print{.print{display:block}.screen{display:none}}
</style><div id=swatch></div><h1>Ferrite PDF</h1><p>Native content</p><p class=print>PrintMarker</p><p class=screen>ScreenMarker</p>"#;
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
fn command(binary: &str, args: &[&std::ffi::OsStr]) -> String {
    let output = std::process::Command::new(binary)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{binary}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn text(path: &Path) -> String {
    command("pdftotext", &[path.as_os_str(), std::ffi::OsStr::new("-")])
}
fn dimensions(path: &Path, width: f64, height: f64, pages: usize) {
    let info = command("pdfinfo", &[path.as_os_str()]);
    let dimensions = info
        .lines()
        .find_map(|line| line.strip_prefix("Page size:"))
        .unwrap();
    let values: Vec<_> = dimensions.split_whitespace().collect();
    assert!(
        (values[0].parse::<f64>().unwrap() - width).abs() < 1.0,
        "{info}"
    );
    assert!(
        (values[2].parse::<f64>().unwrap() - height).abs() < 1.0,
        "{info}"
    );
    let count = info
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap();
    assert_eq!(count, pages, "{info}");
}
fn render(path: &Path) -> image::RgbImage {
    let stem = path.with_extension("");
    command(
        "pdftoppm",
        &[
            std::ffi::OsStr::new("-f"),
            std::ffi::OsStr::new("1"),
            std::ffi::OsStr::new("-singlefile"),
            std::ffi::OsStr::new("-r"),
            std::ffi::OsStr::new("72"),
            std::ffi::OsStr::new("-png"),
            path.as_os_str(),
            stem.as_os_str(),
        ],
    );
    image::open(stem.with_extension("png")).unwrap().to_rgb8()
}
async fn capture(page: &Page, root: &Path, name: &str, options: PdfOptions) -> PathBuf {
    let bytes = page.pdf_with(options).await.unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let path = root.join(format!("{name}.pdf"));
    std::fs::write(&path, bytes).unwrap();
    path
}
fn custom() -> PdfOptions {
    PdfOptions::default().size(PdfLength::Pixels(384.0), PdfLength::Centimeters(15.24))
}

#[tokio::test]
async fn native_paper_content_margins_background_scale_templates_css_and_ranges() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(CONTENT).await.unwrap();
        let legacy = page.pdf().await.unwrap();
        assert!(legacy.starts_with(b"%PDF"));
        if browser.kind() == BrowserKind::Firefox {
            let error = page.pdf_with(PdfOptions::default()).await.unwrap_err();
            assert!(
                matches!(error,E2eError::Config(message) if message.contains("only on Chromium"))
            );
            page.close().await.unwrap();
            browser.close().await.unwrap();
            continue;
        }
        let scratch = tempfile::tempdir().unwrap();
        let root = std::env::var_os("FERRITE_PDF_PREVIEW_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| scratch.path().into());
        std::fs::create_dir_all(&root).unwrap();
        let default = capture(&page, &root, "default", PdfOptions::default()).await;
        dimensions(&default, 612.0, 792.0, 1);
        let content = text(&default);
        assert!(
            content.contains("Ferrite PDF")
                && content.contains("Native content")
                && content.contains("PrintMarker")
        );
        assert!(!content.contains("ScreenMarker"));
        let a4=capture(&page,&root,"a4-header",PdfOptions::default().format(PdfFormat::A4).landscape(true)
            .margins(PdfMargins::all(PdfLength::Millimeters(12.7))).background(true).tagged(true).outline(true)
            .header_footer("<div style='font-size:10px;width:100%;text-align:center'>HeaderMarker</div>","<div style='font-size:10px;width:100%;text-align:center'>FooterMarker <span class='pageNumber'></span>/<span class='totalPages'></span></div>")).await;
        dimensions(&a4, 842.4, 595.44, 1);
        let content = text(&a4);
        assert!(
            content.contains("HeaderMarker")
                && content.contains("FooterMarker")
                && content.contains("1/1"),
            "{content}"
        );
        assert!(std::fs::read(&a4)
            .unwrap()
            .windows(b"/Outlines".len())
            .any(|bytes| bytes == b"/Outlines"));
        assert!(command("pdfinfo", &[a4.as_os_str()])
            .lines()
            .any(|line| line.starts_with("Tagged:") && line.contains("yes")));
        render(&a4);
        let off = capture(&page, &root, "background-off", custom()).await;
        let on = capture(&page, &root, "background-on", custom().background(true)).await;
        let half = capture(
            &page,
            &root,
            "scaled-half",
            custom().background(true).scale(0.5),
        )
        .await;
        for path in [&off, &on, &half] {
            dimensions(path, 288.0, 432.0, 1);
        }
        let off = render(&off);
        let on = render(&on);
        let half = render(&half);
        assert!(off.get_pixel(20, 10).0.iter().all(|channel| *channel > 245));
        let blue = on.get_pixel(20, 10).0;
        assert!(blue[2] > 180 && blue[0] < 40, "{blue:?}");
        assert!(half
            .get_pixel(50, 10)
            .0
            .iter()
            .all(|channel| *channel > 245));
        assert!(on.get_pixel(50, 10).0[2] > 180 && on.get_pixel(50, 10).0[0] < 40);
        page.set_content(&format!(
            "{CONTENT}<style>@page{{size:3in 5in;margin:0}}</style>"
        ))
        .await
        .unwrap();
        let css = capture(
            &page,
            &root,
            "css-size",
            custom().prefer_css_page_size(true),
        )
        .await;
        let fit = capture(&page, &root, "css-fit", custom()).await;
        dimensions(&css, 216.0, 360.0, 1);
        dimensions(&fit, 288.0, 432.0, 1);
        render(&css);
        render(&fit);
        page.set_content("<style>body{margin:0;font:18px Arial}section{break-after:page;height:2in}</style><section>FirstMarker</section><section>SecondMarker</section><section style='break-after:auto'>ThirdMarker</section>").await.unwrap();
        let range = capture(&page, &root, "range-second", custom().page_ranges("2")).await;
        dimensions(&range, 288.0, 432.0, 1);
        let content = text(&range);
        assert!(
            content.contains("SecondMarker")
                && !content.contains("FirstMarker")
                && !content.contains("ThirdMarker")
        );
        render(&range);
        render(&default);
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn early_validation_and_font_print_deadlines_cancellation_and_disposal() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(CONTENT).await.unwrap();
        if browser.kind() == BrowserKind::Firefox {
            page.close().await.unwrap();
            browser.close().await.unwrap();
            continue;
        }
        let asymmetric = page
            .pdf_with(
                PdfOptions::default()
                    .size(PdfLength::Inches(2.0), PdfLength::Inches(6.0))
                    .landscape(true)
                    .margins(PdfMargins {
                        left: PdfLength::Inches(2.0),
                        right: PdfLength::Inches(1.0),
                        ..Default::default()
                    }),
            )
            .await
            .unwrap();
        assert!(asymmetric.starts_with(b"%PDF"));
        // A held native font request proves the font phase stays on the print clock.
        let app = axum::Router::new().route(
            "/font",
            axum::routing::get(|| async { std::future::pending::<axum::http::StatusCode>().await }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        page.set_content(&format!("<style>@font-face{{font-family:Held;src:url('http://{address}/font')}}body{{font-family:Held}}</style>Held native font")).await.unwrap();
        assert_eq!(
            page.evaluate::<String>("document.fonts.status")
                .await
                .unwrap(),
            "loading"
        );
        let error = page
            .pdf_with(PdfOptions::default().timeout(Duration::from_millis(30)))
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Timeout(..)), "{error}");
        let token = CancellationToken::new();
        let (print, ()) = tokio::join!(
            page.pdf_with(
                PdfOptions::default()
                    .timeout(Duration::ZERO)
                    .cancellation(token.clone())
            ),
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                token.cancel_with_reason("cancel PDF");
            }
        );
        assert!(matches!(print,Err(E2eError::Cancelled(reason)) if reason=="cancel PDF"));
        let error = page
            .step_with(
                "print outer clock",
                StepOptions::default().timeout(Duration::from_millis(20)),
                |_| async {
                    page.pdf_with(PdfOptions::default().timeout(Duration::ZERO))
                        .await
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Timeout(..)));
        let (print, closed) = tokio::join!(
            page.pdf_with(PdfOptions::default().timeout(Duration::ZERO)),
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                page.close().await
            }
        );
        closed.unwrap();
        assert!(matches!(print, Err(E2eError::Cancelled(_))));
        // Invalid values are detected even after disposal, before lifecycle/native work.
        assert!(matches!(
            page.pdf_with(PdfOptions::default().scale(f64::NAN)).await,
            Err(E2eError::Config(_))
        ));
        browser.close().await.unwrap();
        server.abort();
    }
}
