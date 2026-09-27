//! Vite compat: framework plugins.

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
    let project = TempProject::new(&[(
        "src/App.tsx",
        "export function App() { return <div>hi</div>; }\n",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let module = server
        .pipeline_module(&ModuleId::new("/src/App.tsx"), None, "client")
        .await
        .unwrap();
    assert!(module.code.contains("$RefreshReg$(App"), "{}", module.code);
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
async fn vue_sfc_splits_through_pipeline() {
    let project = TempProject::new(&[(
        "src/App.vue",
        "<template>\n  <button>{{ msg }}</button>\n</template>\n\
         <script setup lang=\"ts\">\nimport { ref } from \"vue\";\nconst msg = ref(\"hi\");\n</script>\n\
         <style scoped>\nbutton { color: red; }\n</style>\n",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let main = server
        .pipeline_module(&ModuleId::new("/src/App.vue"), None, "client")
        .await
        .unwrap();
    assert!(main.code.contains("?vue&type=script"), "{}", main.code);
    assert!(main.code.contains("?vue&type=template"), "{}", main.code);
    assert!(main.code.contains("?vue&type=style"), "{}", main.code);
    let script = server
        .pipeline_module(
            &ModuleId::new("/src/App.vue?vue&type=script&lang=ts"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(!script.code.contains(": string"), "{}", script.code);
    assert!(script.code.contains("ref(\"hi\")"), "{}", script.code);
    let template = server
        .pipeline_module(
            &ModuleId::new("/src/App.vue?vue&type=template"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(template.code.contains("render"), "{}", template.code);
    assert!(template.code.contains("button"), "{}", template.code);
}

#[tokio::test]
async fn react_dev_html_includes_preamble() {
    let project = TempProject::new(&[(
        "index.html",
        "<!doctype html><html><head><title>t</title></head><body>\
         <script type=\"module\" src=\"/src/App.tsx\"></script></body></html>",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let html = server.transform_index_html("/index.html").await.unwrap();
    assert!(html.contains("/@react-refresh"), "{html}");
}

#[tokio::test]
async fn svelte_splits_through_pipeline() {
    let project = TempProject::new(&[(
        "src/App.svelte",
        "<script>\nlet count = 0;\n</script>\n<button>{count}</button>\n",
    )]);
    let server =
        DevServer::new_without_watcher(dev_config(&project), framework_plugins(&project.root))
            .await
            .unwrap();
    let main = server
        .pipeline_module(&ModuleId::new("/src/App.svelte"), None, "client")
        .await
        .unwrap();
    assert!(main.code.contains("?svelte&type=script"), "{}", main.code);
    assert!(main.code.contains("?svelte&type=markup"), "{}", main.code);
    let markup = server
        .pipeline_module(
            &ModuleId::new("/src/App.svelte?svelte&type=markup"),
            None,
            "client",
        )
        .await
        .unwrap();
    assert!(markup.code.contains("{count}"), "{}", markup.code);
}
