//! Structural framework pipeline conformance; runtime support is verified separately.

use ferrite::server::DevServer;
use ferrite::ModuleId;
use ferrite_test::TempProject;

mod common;

use common::dev_config;

// --- frameworks ---------------------------------------------------------------------------

fn framework_plugins(root: &std::path::Path) -> Vec<std::sync::Arc<dyn ferrite::plugin::Plugin>> {
    vec![
        std::sync::Arc::new(ferrite_frameworks::ReactPlugin::new()),
        std::sync::Arc::new(ferrite_frameworks::VuePlugin::new(root.to_path_buf())),
        std::sync::Arc::new(ferrite_frameworks::SveltePlugin::new(root.to_path_buf())),
    ]
}

#[tokio::test]
async fn react_dev_appends_refresh_footer() {
    let project = TempProject::new(&[
        (
            "src/App.tsx",
            "export function App() { return <div>hi</div>; }\n",
        ),
        (
            ".ferrite/npm/packages/react@19.2.0/package.json",
            r#"{"name":"react","version":"19.2.0","exports":{".":"./index.js","./jsx-runtime":"./jsx-runtime.js","./jsx-dev-runtime":"./jsx-dev-runtime.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/react@19.2.0/jsx-runtime.js",
            "export function jsx() {}\n",
        ),
        (
            ".ferrite/npm/packages/react@19.2.0/jsx-dev-runtime.js",
            "export function jsxDEV() {}\n",
        ),
        (
            ".ferrite/npm/packages/react-refresh@0.17.0/package.json",
            r#"{"name":"react-refresh","version":"0.17.0","exports":{".":"./runtime.js","./runtime":"./runtime.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/react-refresh@0.17.0/runtime.js",
            "export function injectIntoGlobalHook() {}\n",
        ),
    ]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let module = server
        .pipeline_module(&ModuleId::new("/src/App.tsx"), None, "client")
        .await
        .unwrap();
    assert!(
        module.code.contains("__ferrite_refresh_reg__(_c, \"App\")"),
        "{}",
        module.code
    );
    assert!(
        module
            .code
            .contains("__ferrite_refresh_runtime__.register(type"),
        "{}",
        module.code
    );
    assert!(!module.code.contains("$RefreshReg$("), "{}", module.code);
    assert!(
        module.code.contains("createSignatureFunctionForTransform"),
        "{}",
        module.code
    );
    assert!(
        module.code.contains("performReactRefresh"),
        "{}",
        module.code
    );
    assert!(module.code.contains("react-refresh"), "{}", module.code);
    // Footer preamble import resolves to the virtual module.
    assert!(
        module
            .imports
            .iter()
            .any(|(_, dep, _)| dep.0.contains("react-refresh")),
        "{:?}",
        module.imports
    );
}

#[tokio::test]
async fn react_production_build_has_no_footer() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.tsx\"></script></body></html>",
        ),
        (
            "src/main.tsx",
            "import { App } from \"./App\";\nconsole.log(<App />);\n",
        ),
        (
            "src/App.tsx",
            "export function App() { return <div>hi</div>; }\n",
        ),
        (
            ".ferrite/npm/packages/react@19.2.0/package.json",
            r#"{"name":"react","version":"19.2.0","exports":{".":"./index.js","./jsx-runtime":"./jsx-runtime.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/react@19.2.0/jsx-runtime.js",
            "export function jsx() {}\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, framework_plugins(&project.root));
    let report = builder.build("client").await.unwrap();
    let mut blob = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    assert!(
        blob.contains("jsx-runtime") || blob.contains("jsxDEV"),
        "{blob}"
    );
    assert!(!blob.contains("$RefreshReg$"), "{blob}");
    assert!(!blob.contains("performReactRefresh"), "{blob}");
}

#[tokio::test]
async fn vue_component_requires_explicit_official_compiler_host() {
    let project = TempProject::new(&[
        (
            "src/App.vue",
            "<template>\n  <button>{{ msg }}</button>\n</template>\n\
             <script setup lang=\"ts\">\nimport { ref } from \"vue\";\nconst msg = ref(\"hi\");\n</script>\n\
             <style scoped>\nbutton { color: red; }\n</style>\n",
        ),
        (
            ".ferrite/npm/packages/vue@3.0.0/package.json",
            r#"{"name":"vue","version":"3.0.0","exports":{".":"./index.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/vue@3.0.0/index.js",
            "export function ref(v) { return v; }\n",
        ),
    ]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    for id in [
        "/src/App.vue",
        "/src/App.vue?vue&type=script&lang=ts",
        "/src/App.vue?vue&type=template",
        "/src/App.vue?vue&type=style",
    ] {
        let error = server
            .pipeline_module(&ModuleId::new(id), None, "client")
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("vue/compiler-sfc"), "{message}");
        assert!(message.contains("validated compiler host"), "{message}");
        assert!(
            message.contains("no compiler host has been explicitly configured"),
            "{message}"
        );
    }
}

#[tokio::test]
async fn react_dev_html_includes_preamble() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/App.tsx\"></script></body></html>",
        ),
        (
            ".ferrite/npm/packages/react-refresh@0.17.0/package.json",
            r#"{"name":"react-refresh","version":"0.17.0","exports":{".":"./runtime.js","./runtime":"./runtime.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/react-refresh@0.17.0/runtime.js",
            "export function injectIntoGlobalHook() {}\n",
        ),
    ]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let html = server.transform_index_html("/index.html").await.unwrap();
    assert!(html.contains("/@react-refresh"), "{html}");
}

#[tokio::test]
async fn react_dev_html_skips_preamble_without_refresh_package() {
    // No `react-refresh` installed (e.g. the docs site): injecting the
    // preamble would serve a bare `react-refresh/runtime` import that no
    // browser can resolve, so the plugin stays silent.
    let project = TempProject::new(&[(
        "index.html",
        "<!doctype html><html><head><title>t</title></head><body>\
         <script type=\"module\" src=\"/src/main.js\"></script></body></html>",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let html = server.transform_index_html("/index.html").await.unwrap();
    assert!(!html.contains("/@react-refresh"), "{html}");
}

#[tokio::test]
async fn svelte_component_requires_explicit_official_compiler_host() {
    let project = TempProject::new(&[(
        "src/App.svelte",
        "<script>\nlet count = 0;\n</script>\n<button>{count}</button>\n",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    for id in ["/src/App.svelte", "/src/App.svelte?svelte&type=markup"] {
        let error = server
            .pipeline_module(&ModuleId::new(id), None, "client")
            .await
            .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("svelte/compiler"), "{message}");
        assert!(message.contains("validated compiler host"), "{message}");
        assert!(
            message.contains("no compiler host has been explicitly configured"),
            "{message}"
        );
    }
}
