//! Vite compat: dev pipeline (HTML, HMR, env, assets, CSS, dynamic).

use ferrite::graph::ModuleGraph;
use ferrite::graph::ModuleNode;
use ferrite::ModuleId;
use ferrite::ModuleType;
use ferrite_test::assert_contains_all;
use ferrite_test::vanilla_files;
use ferrite_test::TempProject;

mod common;

use common::dev_server;

// --- HTML ----------------------------------------------------------------------

#[tokio::test]
async fn html_transform_injects_client_and_resolves_entries() {
    let project = TempProject::new(&vanilla_files());
    let server = dev_server(&project).await;
    let html = server.transform_index_html("/index.html").await.unwrap();
    assert_contains_all(&html, &["/@ferrite/client", "/src/main.ts"]);
}

// --- HMR ------------------------------------------------------------------------

#[test]
fn hmr_boundaries_and_protocol() {
    let graph = ModuleGraph::new();
    // CSS is always its own boundary.
    graph.upsert(ModuleNode::new(
        ModuleId::new("/a.css"),
        "/a.css".to_string(),
        ModuleType::Css,
    ));
    assert!(graph.hmr_boundaries(&ModuleId::new("/a.css")).is_some());
    // JS without a boundary needs a full reload.
    graph.upsert(ModuleNode::new(
        ModuleId::new("/entry.ts"),
        "/entry.ts".to_string(),
        ModuleType::Ts,
    ));
    graph.upsert(ModuleNode::new(
        ModuleId::new("/leaf.ts"),
        "/leaf.ts".to_string(),
        ModuleType::Ts,
    ));
    graph.add_edge(
        &ModuleId::new("/entry.ts"),
        ferrite::graph::ImportEdge {
            specifier: "./leaf".to_string(),
            resolved: ModuleId::new("/leaf.ts"),
            kind: ferrite::graph::ImportKind::Static,
        },
    );
    assert!(graph.hmr_boundaries(&ModuleId::new("/leaf.ts")).is_none());
    // Protocol shape (§32).
    let message = ferrite::hmr::HmrMessage::Update {
        updates: vec![ferrite::hmr::HmrUpdate {
            kind: "js-update".to_string(),
            path: "/src/App.tsx".to_string(),
            accepted_path: "/src/App.tsx".to_string(),
            timestamp: 1,
            css_only: false,
        }],
    };
    let text = serde_json::to_string(&message).unwrap();
    assert!(text.contains("acceptedPath"));
}

// --- import.meta.env --------------------------------------------------------------

#[tokio::test]
async fn import_meta_env_prefix_filtering() {
    let project = TempProject::new(&[
        (".env", "FERRITE_KEY=abc\nSECRET=topsecret\n"),
        (
            "src/main.ts",
            "export const a = import.meta.env.FERRITE_KEY;\nexport const b = import.meta.env.SECRET;\nexport const m = import.meta.env.MODE;\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(module.code.contains("\"abc\""), "{}", module.code);
    assert!(!module.code.contains("topsecret"), "{}", module.code);
    assert!(module.code.contains("development"), "{}", module.code);
}

// --- assets -----------------------------------------------------------------------

#[tokio::test]
async fn assets_raw_shim_and_direct() {
    let project = TempProject::new(&[
        ("src/notes.txt", "hello notes"),
        ("src/logo.svg", "<svg></svg>"),
        (
            "src/main.ts",
            "import text from \"./notes.txt?raw\";\nimport logo from \"./logo.svg\";\nconsole.log(text, logo);\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(
        main.code.contains("/src/logo.svg?asset-shim"),
        "{}",
        main.code
    );
    let raw = server
        .pipeline_module(&ModuleId::new("/src/notes.txt?raw"), None, "client")
        .await
        .unwrap();
    assert!(raw.code.contains("hello notes"), "{}", raw.code);
    let shim = server
        .pipeline_module(&ModuleId::new("/src/logo.svg?asset-shim"), None, "client")
        .await
        .unwrap();
    assert!(shim.code.contains("/src/logo.svg"), "{}", shim.code);
    let direct = server
        .pipeline_module(&ModuleId::new("/src/logo.svg"), None, "client")
        .await
        .unwrap();
    assert!(direct.is_raw_bytes);
}

// --- CSS ----------------------------------------------------------------------------

#[tokio::test]
async fn css_modules_and_injection() {
    let project = TempProject::new(&[
        ("src/a.module.css", ".btn { color: red; }\n"),
        (
            "src/main.ts",
            "import classes from \"./a.module.css\";\nconsole.log(classes);\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let css = server
        .pipeline_module(&ModuleId::new("/src/a.module.css"), None, "client")
        .await
        .unwrap();
    assert_contains_all(&css.code, &["updateStyle", "btn_"]);
    let direct = server
        .pipeline_module(&ModuleId::new("/src/a.module.css?direct"), None, "client")
        .await
        .unwrap();
    assert!(!direct.code.contains("updateStyle"));
    assert!(direct.code.contains(".btn_"));
}

// --- dynamic imports -------------------------------------------------------------------

#[tokio::test]
async fn dynamic_imports_rewritten_and_graphed() {
    let project = TempProject::new(&[
        ("src/lazy.ts", "export const lazy = 1;\n"),
        (
            "src/main.ts",
            "export async function load() {\n  return import(\"./lazy.ts\");\n}\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(
        main.code.contains("import(\"/src/lazy.ts\")"),
        "{}",
        main.code
    );
    let node = server
        .inner()
        .graph
        .get(&ModuleId::new("/src/main.ts"))
        .unwrap();
    assert!(node
        .imports
        .iter()
        .any(|edge| matches!(edge.kind, ferrite::graph::ImportKind::Dynamic)));
}
