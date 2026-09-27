//! Vite compat: build manifest.

use ferrite_test::vanilla_files;
use ferrite_test::TempProject;

// --- build manifest -----------------------------------------------------------------------

#[tokio::test]
async fn production_build_emits_manifest_and_html() {
    let project = TempProject::new(&vanilla_files());
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    assert_eq!(report.env, "client");
    assert!(!report.entries.is_empty());
    let manifest_text =
        std::fs::read_to_string(report.out_dir.join("manifest.json")).expect("manifest");
    assert!(manifest_text.contains("assets/"), "{manifest_text}");
    let html = std::fs::read_to_string(report.out_dir.join("index.html")).expect("html");
    assert!(html.contains("assets/"), "{html}");
    assert!(!html.contains("/@ferrite/client"), "{html}");
    assert!(report.out_dir.join("favicon.ico").exists());
    // Regression: chunk imports must point at hashed siblings, never dev urls.
    let main_chunk = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("main-") && name.ends_with(".js"))
        })
        .expect("main chunk");
    let chunk = std::fs::read_to_string(&main_chunk).expect("chunk");
    // CSS extraction (§29): no style chunk import; styles live in .css.
    assert!(!chunk.contains("style"), "{chunk}");
    let css_chunk = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("style-") && name.ends_with(".css"))
        })
        .expect("style css");
    let css = std::fs::read_to_string(&css_chunk).expect("css");
    assert!(css.contains(".app"), "{css}");
    assert!(html.contains("<link rel=\"stylesheet\""), "{html}");
    assert!(manifest_text.contains("\"css\""), "{manifest_text}");
}

#[tokio::test]
async fn production_build_extracts_modules_imports_and_urls() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import styles from \"./app.module.css\";\n\
             import \"./theme.css\";\n\
             document.body.className = styles.btn;\n",
        ),
        ("src/app.module.css", ".btn { color: green; }\n"),
        (
            "src/theme.css",
            "@import \"./base.css\";\nbody { background: url(\"./bg.png\"); }\n",
        ),
        ("src/base.css", ":root { --x: 1; }\n"),
        ("src/bg.png", "fakepng"),
    ]);
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let assets: Vec<std::path::PathBuf> = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    let find = |prefix: &str, ext: &str| {
        assets
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(prefix) && name.ends_with(ext))
            })
            .unwrap_or_else(|| panic!("{prefix}*{ext} in {assets:?}"))
            .clone()
    };
    // Per-module css files with rewritten @import and emitted url() asset.
    let theme = std::fs::read_to_string(find("theme-", ".css")).expect("theme");
    assert!(theme.contains("@import \"./base-"), "{theme}");
    assert!(theme.contains("url(\"/assets/bg."), "{theme}");
    assert!(find("bg.", ".png").exists());
    assert!(find("base-", ".css").exists());
    // Modules stub chunk keeps the exports map; bare import stripped.
    let main = std::fs::read_to_string(find("main-", ".js")).expect("main");
    assert!(!main.contains("theme.css"), "{main}");
    assert!(main.contains("./app-"), "{main}");
    let stub = std::fs::read_to_string(find("app-", ".js")).expect("stub");
    assert!(stub.contains("btn_"), "{stub}");
    let html = std::fs::read_to_string(report.out_dir.join("index.html")).expect("html");
    assert!(html.contains("<link rel=\"stylesheet\""), "{html}");
}
