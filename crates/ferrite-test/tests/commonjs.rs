//! Real CommonJS interop through the shared pipeline and Rust browser drivers.
use ferrite::{server::DevServer, Ferrite, ModuleId, ModuleRequest};
use ferrite_e2e::{Browser, BrowserKind, LaunchOptions};
use ferrite_test::TempProject;
use std::time::Duration;

fn fixture() -> TempProject {
    TempProject::new(&[
        ("index.html", "<!doctype html><html><head><link rel='icon' href='data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22/%3E'></head><body><button id='count'>0</button><script type='module' src='/main.js'></script></body></html>"),
        ("main.js", r#"
import math, { answer, 'hyphen-name' as hyphen, getter, extra } from './math.cjs';
import * as namespace from './barrel.cjs';
import cycle from './a.cjs';
import conditional from './conditional.cjs';
import nullValue from './null.cjs';
window.result = { answer, hyphen, getter, extra, same: namespace.default === math, reexport: namespace.answer, cycle: cycle.peer, conditional, nullValue, runs: globalThis.runs, lazy: globalThis.shouldNotRun || 0, esmMarker: math.__esModule, nestedDefault: math.default };
document.querySelector('#count').onclick = event => event.target.textContent = String(Number(event.target.textContent) + math.answer);
"#),
        ("math.cjs", r#"
globalThis.runs = (globalThis.runs || 0) + 1;
const json = require /* comment */ ('./data.json');
exports.answer = json.answer;
exports['hyphen-name'] = 'literal';
Object.defineProperty(exports, 'getter', { get() { return 7; } });
Object.assign(module.exports, { extra: true });
exports.__esModule = true;
exports.default = 'inner-default';
"#),
        ("barrel.cjs", "module.exports = require('./math.cjs');"),
        ("a.cjs", "exports.name = 'a'; const b = require('./b.cjs'); exports.peer = b.seen;"),
        ("b.cjs", "exports.seen = require('./a.cjs').name;"),
        ("conditional.cjs", "module.exports = false ? require('./unused.cjs') : 3;"),
        ("unused.cjs", "globalThis.shouldNotRun = 99; module.exports = 99;"),
        ("null.cjs", "module.exports = null;"),
        ("data.json", "{\"answer\":42}"),
    ])
}

#[tokio::test]
async fn maps_exports_cache_and_library_parity() {
    let project = fixture();
    let mut config = project.resolve_config();
    config.build.sourcemap = ferrite::config::SourceMapConfig::Bool(true);
    let server = DevServer::new_without_watcher(config.clone(), vec![])
        .await
        .unwrap();
    let facade = server
        .pipeline_module(&ModuleId::new("/barrel.cjs"), None, "client")
        .await
        .unwrap();
    let cached = server
        .pipeline_module(&ModuleId::new("/barrel.cjs"), None, "client")
        .await
        .unwrap();
    assert_eq!(cached.code, facade.code);
    assert!(facade
        .shake
        .as_ref()
        .unwrap()
        .exports
        .iter()
        .any(|export| export.exported == "answer"));
    let factory = server
        .pipeline_module(
            &ModuleId::new("/math.cjs?ferrite-cjs-factory"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(factory.map.is_some());
    assert_eq!(factory.commonjs.unwrap().names.len(), 5);
    let library = Ferrite::new(config, vec![]).unwrap();
    let result = library
        .transform_request(ModuleRequest {
            specifier: "/barrel.cjs".into(),
            importer: None,
            environment: ferrite::EnvironmentKind::Client,
        })
        .await
        .unwrap();
    assert!(result.exports.contains(&"answer".into()));
    assert!(result
        .imports
        .iter()
        .any(|import| import.specifier.contains("ferrite-cjs-factory")));
    // Re-export discovery depends on the child's final static facts.
    std::fs::write(project.root.join("math.cjs"), "exports.changed = 1;").unwrap();
    let changed = server
        .pipeline_module(&ModuleId::new("/barrel.cjs"), None, "client")
        .await
        .unwrap();
    assert_ne!(changed.code, facade.code);
    assert!(changed.code.contains("changed"));
    for (file, source, expected) in [
        (
            "dynamic.cjs",
            "module.exports = require(path);",
            "dynamic require",
        ),
        (
            "missing.cjs",
            "module.exports = require('not-installed');",
            "cannot resolve require",
        ),
        (
            "native.cjs",
            "module.exports = require('./binding.node');",
            "native",
        ),
        (
            "mixed.js",
            "export const x = require('./math.cjs');",
            "mixed ESM",
        ),
    ] {
        std::fs::write(project.root.join(file), source).unwrap();
        let error = server
            .pipeline_module(&ModuleId::new(format!("/{file}")), None, "client")
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
    }
    let inline = server
        .ssr_transform("exports.answer = 42;", "/not-on-disk.cjs")
        .await
        .unwrap();
    assert!(inline.code.contains("exports.answer = 42"));
    assert!(!inline.code.contains("not-on-disk.cjs?ferrite-cjs-factory"));
    assert!(inline.deps.is_empty());
    assert!(inline.map.is_some());
    server.close();
}

async fn browser_interaction(kind: BrowserKind) {
    let executable = match kind {
        BrowserKind::Chromium => ferrite_e2e::find_chromium(None),
        BrowserKind::Firefox => ferrite_e2e::find_firefox(None),
    }.expect("CommonJS browser acceptance requires this browser installed; missing execution is not a passing result");
    let browser = Browser::launch(
        LaunchOptions::default()
            .browser(kind)
            .executable(executable),
    )
    .await
    .unwrap();
    let page = browser.new_page().await.unwrap();
    let project = fixture();
    let server = DevServer::new(project.resolve_config(), vec![])
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = server.router();
    let serving = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    page.goto(&url).await.unwrap();
    assert_result(&page, 42).await;
    page.locator("#count").click().await.unwrap();
    assert_eq!(
        page.locator("#count")
            .text_content()
            .await
            .unwrap()
            .as_deref(),
        Some("42")
    );
    // Node-free CJS invalidation deliberately resets runtime module state on reload.
    std::fs::write(project.root.join("data.json"), "{\"answer\":43}").unwrap();
    page.wait_for_function("window.result?.answer === 43", Duration::from_secs(10))
        .await
        .unwrap();
    assert_result(&page, 43).await;
    assert_eq!(
        page.locator("#count")
            .text_content()
            .await
            .unwrap()
            .as_deref(),
        Some("0")
    );
    page.locator("#count").click().await.unwrap();
    assert_eq!(
        page.locator("#count")
            .text_content()
            .await
            .unwrap()
            .as_deref(),
        Some("43")
    );
    // Build the same graph and execute the emitted application through Ferrite preview.
    let mut config = project.resolve_config_mode("production");
    config.build.scope_hoist = true;
    let report = ferrite::Builder::new(config, vec![])
        .build("client")
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    for entry in std::fs::read_dir(report.out_dir.join("assets")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "js") {
            let code = std::fs::read_to_string(path).unwrap();
            assert!(!code.contains("/@ferrite/client"));
            assert!(!code.contains("__ferrite_create_hot__"));
        }
    }
    let out = report.out_dir;
    let preview = tokio::spawn(async move {
        ferrite::preview_dir(&out, port).await.unwrap();
    });
    let mut ready = false;
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(ready, "preview did not start");
    page.goto(&format!("http://127.0.0.1:{port}"))
        .await
        .unwrap();
    assert_result(&page, 43).await;
    serving.abort();
    server.close();
    page.locator("#count").click().await.unwrap();
    assert_eq!(
        page.locator("#count")
            .text_content()
            .await
            .unwrap()
            .as_deref(),
        Some("43")
    );
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
    assert!(
        page.console_messages()
            .iter()
            .all(|message| message.kind != "error"),
        "{:?}",
        page.console_messages()
    );
    browser.close().await.unwrap();
    preview.abort();
}

async fn assert_result(page: &ferrite_e2e::Page, answer: u32) {
    page.wait_for_function("Boolean(window.result)", Duration::from_secs(10))
        .await
        .unwrap_or_else(|error| panic!("{error}; browser errors: {:?}", page.page_errors()));
    let result: serde_json::Value = page.evaluate("window.result").await.unwrap();
    assert_eq!(
        result,
        serde_json::json!({ "answer": answer, "hyphen": "literal", "getter": 7, "extra": true, "same": true, "reexport": answer, "cycle": "a", "conditional": 3, "nullValue": null, "runs": 1, "lazy": 0, "esmMarker": true, "nestedDefault": "inner-default" })
    );
    assert!(page.page_errors().is_empty(), "{:?}", page.page_errors());
}
#[tokio::test]
async fn chromium_dev_edit_build_preview_interaction() {
    browser_interaction(BrowserKind::Chromium).await;
}
#[tokio::test]
async fn firefox_dev_edit_build_preview_interaction() {
    browser_interaction(BrowserKind::Firefox).await;
}
