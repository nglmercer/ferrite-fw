//! Vite compat: docs site.

use ferrite::plugin::Plugin;
use ferrite_test::TempProject;
use std::sync::Arc;

// --- docs site (markdown + tailwind) ------------------------------------------------------------------

/// Mini docs fixture through the real production pipeline: `.md` becomes a
/// JS module with rendered HTML, `ferrite:tailwind.css` becomes extracted
/// CSS with compiled utilities.
#[tokio::test]
async fn docs_pipeline_builds_markdown_and_tailwind() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <main class=\"flex\"></main>\
             <script type=\"module\" src=\"/src/main.js\"></script></body></html>",
        ),
        (
            "src/main.js",
            "import \"ferrite:tailwind.css\";\n\
             import page from \"../pages/guide.md\";\n\
             document.body.innerHTML = page.html;\n",
        ),
        (
            "pages/guide.md",
            "---\ntitle: Guide\norder: 1\n---\n# Guide\n\nHello docs.\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(ferrite::docs::MarkdownPlugin::new(project.root.clone())),
        Arc::new(ferrite::tailwind::TailwindPlugin::new(project.root.clone())),
    ];
    let builder = ferrite::Builder::new(config, plugins);
    let report = builder.build("client").await.unwrap();
    let mut js = String::new();
    let mut css = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        let text = std::fs::read_to_string(&path).expect("chunk");
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("js") => js.push_str(&text),
            Some("css") => css.push_str(&text),
            _ => {}
        }
    }
    assert!(js.contains("<h1"), "{js}");
    assert!(js.contains("Hello docs."), "{js}");
    assert!(css.contains(".flex"), "{css}");
    let html = std::fs::read_to_string(report.out_dir.join("index.html")).expect("html");
    assert!(html.contains("stylesheet"), "{html}");
}

/// Every docs locale ships the same page slugs, and every page renders
/// to a titled module. Guards against half-translated locale additions.
#[test]
fn docs_site_locales_are_complete() {
    let pages = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/pages");
    let locales = ["en", "es", "cn"];
    let mut expected: Option<Vec<String>> = None;
    for locale in locales {
        let dir = pages.join(locale);
        assert!(dir.is_dir(), "missing locale dir {}", dir.display());
        let mut slugs: Vec<String> = std::fs::read_dir(&dir)
            .expect("read locale dir")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.ends_with(".md"))
            .collect();
        slugs.sort();
        assert!(!slugs.is_empty(), "locale {locale} has no pages");
        if let Some(expected) = &expected {
            assert_eq!(&slugs, expected, "locale {locale} slug drift");
        } else {
            expected = Some(slugs.clone());
        }
        for slug in &slugs {
            let source = std::fs::read_to_string(dir.join(slug)).expect("read page");
            let module = ferrite::docs::render_module(&format!("/{locale}/{slug}"), &source);
            assert!(module.contains("export default page;"), "{locale}/{slug}");
            assert!(module.contains("<h"), "{locale}/{slug} renders no headings");
            assert!(
                !module.contains("\"title\":null"),
                "{locale}/{slug} has no title"
            );
        }
    }
}

/// The docs shell wires the theme toggle and the locale switcher.
/// Static check (no JS runner in the repo harness): the strings the
/// router depends on must exist.
#[test]
fn docs_site_shell_wires_theme_and_locales() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site");
    // Shell sources (TypeScript): router in main.ts, widgets in components.ts.
    let main = ["src/main.ts", "src/components.ts"]
        .iter()
        .map(|rel| std::fs::read_to_string(root.join(rel)).expect("shell source"))
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "ferrite-docs-theme",
        "ferrite-docs-locale",
        "locale-select",
        "theme-toggle",
        "\"en\", \"es\", \"cn\"",
        "#/${locale}/${slug}",
        "copy-btn",
        "writeText",
        "toc-list",
        "nav-toggle",
        "mobile-drawer",
        "drawer-open",
        "page-prev",
        "page-next",
        "data-tip",
        "theme-btn-label",
    ] {
        assert!(main.contains(needle), "site shell lost `{needle}`");
    }
    let html = std::fs::read_to_string(root.join("index.html")).expect("index.html");
    for needle in ["ferrite-docs-theme", "prefers-color-scheme"] {
        assert!(
            html.contains(needle),
            "pre-paint theme script lost `{needle}`"
        );
    }
    let css = std::fs::read_to_string(root.join("src/docs.css")).expect("docs.css");
    for needle in [
        "html.dark",
        "--surface",
        "--ink",
        ".nav-link",
        ".theme-btn",
        ".toc-list",
        ".copy-btn",
        "prefers-reduced-motion",
        ".theme-header .theme-btn",
        ".drawer",
        ".drawer-backdrop",
        ".page-nav",
        ".theme-select",
        "#nav-toggle",
        ".theme-btn-label",
        "overflow-wrap",
    ] {
        assert!(css.contains(needle), "docs.css lost `{needle}`");
    }
}
