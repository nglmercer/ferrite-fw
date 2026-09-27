//! Vite compatibility suite (spec §67).
//!
//! Categories: config, resolver, plugin ordering, virtual modules, HTML
//! transforms, HMR, import.meta.env, assets, CSS, dynamic imports, SSR, SSR
//! externalization, build manifest, source maps. No Node.js required.

use std::sync::{Arc, Mutex};

use ferrite::graph::{ModuleGraph, ModuleNode};
use ferrite::plugin::{
    Apply, Enforce, Plugin, PluginContainer, PluginContext,
    TransformRequest as HookTransformRequest, TransformResult as HookTransformResult,
};
use ferrite::server::DevServer;
use ferrite::{EnvironmentKind, ModuleId, ModuleType};
use ferrite_test::{assert_contains_all, vanilla_files, TempProject};

fn dev_config(project: &TempProject) -> ferrite::ResolvedConfig {
    project.resolve_config()
}

async fn dev_server(project: &TempProject) -> DevServer {
    DevServer::new_without_watcher(dev_config(project), vec![])
        .await
        .expect("server")
}

// --- config ------------------------------------------------------------------

#[test]
fn config_defaults_and_precedence() {
    let project = TempProject::new(&[("ferrite.toml", "[server]\nport = 3000\n")]);
    let user = ferrite::load_user_config(&project.root).unwrap();
    assert_eq!(user.server.port, 3000);
    // CLI overrides win.
    let resolved = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            port: Some(4000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(resolved.server.port, 4000);
    assert_eq!(resolved.base, "/");
}

// --- resolver -----------------------------------------------------------------

#[tokio::test]
async fn resolver_relative_and_alias() {
    let project = TempProject::new(&[
        ("src/main.ts", "import x from \"./x\";\nconsole.log(x);\n"),
        ("src/x.ts", "export default 1;"),
        ("src/deep/y.ts", "export default 2;"),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.resolve
        .alias
        .insert("@".to_string(), "/src".to_string());
    let resolved = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides::default(),
    )
    .unwrap();
    let server = DevServer::new_without_watcher(resolved, vec![])
        .await
        .unwrap();
    let importer = ModuleId::new("/src/main.ts");
    let relative = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), Some(&importer), "client")
        .await
        .unwrap();
    assert!(relative.code.contains("/src/x.ts"), "{}", relative.code);
    let aliased = server.resolve_entry("@/deep/y.ts", "client").await.unwrap();
    assert_eq!(aliased.0, "/src/deep/y.ts");
}

#[tokio::test]
async fn resolver_bare_exports_conditions() {
    let project = TempProject::new(&[
        (
            ".ferrite/npm/packages/greet@1.0.0/package.json",
            r#"{"name":"greet","version":"1.0.0","exports":{".":"./index.js","./feature":"./feature.js"}}"#,
        ),
        (
            ".ferrite/npm/packages/greet@1.0.0/index.js",
            "export default \"hi\";",
        ),
        (
            ".ferrite/npm/packages/greet@1.0.0/feature.js",
            "export const f = 1;",
        ),
        (
            "src/main.ts",
            "import g from \"greet\";\nimport { f } from \"greet/feature\";\nconsole.log(g, f);",
        ),
    ]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert_contains_all(
        &main.code,
        &["/@npm/greet@1.0.0/index.js", "/@npm/greet@1.0.0/feature.js"],
    );
}

// --- plugin ordering -----------------------------------------------------------

struct OrderPlugin {
    name: &'static str,
    enforce: Enforce,
    log: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Plugin for OrderPlugin {
    fn name(&self) -> &'static str {
        self.name
    }

    fn enforce(&self) -> Enforce {
        self.enforce
    }

    async fn transform(
        &self,
        _ctx: &PluginContext,
        request: HookTransformRequest,
    ) -> ferrite::Result<Option<HookTransformResult>> {
        self.log.lock().unwrap().push(self.name.to_string());
        Ok(Some(HookTransformResult {
            code: format!("{}// {}\n", request.code, self.name),
            map: None,
            dependencies: Vec::new(),
        }))
    }
}

#[tokio::test]
async fn plugin_ordering_pre_normal_post() {
    let project = TempProject::new(&[("src/a.ts", "export const a = 1;\n")]);
    let log = Arc::new(Mutex::new(Vec::new()));
    let plugins: Vec<Arc<dyn Plugin>> = vec![
        Arc::new(OrderPlugin {
            name: "post",
            enforce: Enforce::Post,
            log: log.clone(),
        }),
        Arc::new(OrderPlugin {
            name: "pre",
            enforce: Enforce::Pre,
            log: log.clone(),
        }),
        Arc::new(OrderPlugin {
            name: "normal",
            enforce: Enforce::Normal,
            log: log.clone(),
        }),
    ];
    let container = PluginContainer::new(plugins.clone(), Apply::All);
    assert_eq!(container.names(), vec!["pre", "normal", "post"]);
    let server = DevServer::new_without_watcher(dev_config(&project), plugins)
        .await
        .unwrap();
    server
        .pipeline_module(&ModuleId::new("/src/a.ts"), None, "client")
        .await
        .unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["pre", "normal", "post"]);
}

// --- virtual modules -----------------------------------------------------------

#[tokio::test]
async fn virtual_modules_roundtrip() {
    let project = TempProject::new(&[
        (".env", "FERRITE_KEY=abc\n"),
        (
            "src/main.ts",
            "import env from \"virtual:ferrite/env\";\nconsole.log(env);\n",
        ),
    ]);
    let server = dev_server(&project).await;
    // Resolver maps to the internal id.
    let resolved = server
        .resolve_entry("virtual:ferrite/env", "client")
        .await
        .unwrap();
    assert_eq!(resolved.0, "\0virtual:ferrite/env");
    // Rewriting uses the servable /@id/ URL.
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(
        main.code.contains("/@id/virtual:ferrite/env"),
        "{}",
        main.code
    );
    // The virtual module serves env content.
    let virtual_module = server
        .pipeline_module(&ModuleId::new("/@id/virtual:ferrite/env"), None, "client")
        .await
        .unwrap();
    assert!(
        virtual_module.code.contains("FERRITE_KEY"),
        "{}",
        virtual_module.code
    );
}

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

// --- SSR -------------------------------------------------------------------------------

#[tokio::test]
async fn ssr_load_module_traverses() {
    let project = TempProject::new(&[
        (
            "src/entry-server.ts",
            "import { render } from \"./view\";\nexport { render };\n",
        ),
        (
            "src/view.ts",
            "export function render(url: string): string {\n  return url;\n}\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .ssr_load_module("/src/entry-server.ts")
        .await
        .unwrap();
    assert_eq!(module.id, "/src/entry-server.ts");
    assert!(module.dependencies.iter().any(|dep| dep.contains("view")));
    assert!(!module.code.contains("/@ferrite/client"), "{}", module.code);
}

#[test]
fn ssr_externalization_rules() {
    let config = ferrite::config::SsrConfig {
        external: vec!["pg".to_string()],
        no_external: vec!["my-esm-package".to_string()],
        ..Default::default()
    };
    assert!(ferrite::ssr::is_external("pg", &config));
    assert!(!ferrite::ssr::is_external("my-esm-package", &config));
    assert!(!ferrite::ssr::is_external("./local.ts", &config));
}

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
    assert!(chunk.contains("./style-"), "{chunk}");
    assert!(!chunk.contains("/src/style.css"), "{chunk}");
}

// --- source maps ----------------------------------------------------------------------------

#[test]
fn source_maps_produced_on_request() {
    let compiler = ferrite::transform::OxcCompiler::new(Default::default());
    let mut request = ferrite::transform::TransformRequest::new(
        "/src/a.ts",
        "const x: number = 1;\nexport default x;\n",
        ModuleType::Ts,
    );
    request.sourcemap = true;
    let result = ferrite::transform::JsCompiler::transform(&compiler, request).unwrap();
    let map = result.map.expect("map");
    assert!(map.mappings.contains("mappings") || map.mappings.contains("version"));
}

// --- CJS --------------------------------------------------------------------------------------

#[tokio::test]
async fn cjs_converted_to_esm_wrapper() {
    let project = TempProject::new(&[
        ("dep.js", "export default 41;\n"),
        (
            "legacy.cjs",
            "const dep = require(\"./dep\");\nmodule.exports = { dep };\n",
        ),
    ]);
    let server = dev_server(&project).await;
    let module = server
        .pipeline_module(&ModuleId::new("/legacy.cjs"), None, "client")
        .await
        .unwrap();
    assert_contains_all(
        &module.code,
        &["__ferrite_interop__", "module.exports", "export default"],
    );
}

// --- node compat ---------------------------------------------------------------------------------

#[tokio::test]
async fn node_builtin_shims_in_browser() {
    let project = TempProject::new(&[(
        "src/main.ts",
        "import path from \"node:path\";\nconsole.log(path.sep);\n",
    )]);
    let server = dev_server(&project).await;
    let main = server
        .pipeline_module(&ModuleId::new("/src/main.ts"), None, "client")
        .await
        .unwrap();
    assert!(main.code.contains("/@id/node:path"), "{}", main.code);
    let shim = server
        .pipeline_module(&ModuleId::new("/@id/node:path"), None, "client")
        .await
        .unwrap();
    assert!(shim.code.contains("sep"), "{}", shim.code);
}

// --- TS/JSX ----------------------------------------------------------------------------------------

#[tokio::test]
async fn tsx_automatic_runtime() {
    let project = TempProject::new(&[(
        "src/app.tsx",
        "export const App = () => <div className=\"a\">hi</div>;\n",
    )]);
    let server = dev_server(&project).await;
    let module = server
        .pipeline_module(&ModuleId::new("/src/app.tsx"), None, "client")
        .await
        .unwrap();
    assert!(!module.code.contains("<div"), "{}", module.code);
    assert!(module.code.contains("jsx"), "{}", module.code);
}

// --- errors ------------------------------------------------------------------------------------------

#[tokio::test]
async fn parse_errors_carry_diagnostics() {
    let project = TempProject::new(&[("src/bad.ts", "const = ;;;\n")]);
    let server = dev_server(&project).await;
    let error = server
        .pipeline_module(&ModuleId::new("/src/bad.ts"), None, "client")
        .await
        .unwrap_err();
    let diagnostic = error.diagnostic();
    assert_eq!(diagnostic.code, "FERRITE_PARSE_001");
    assert!(diagnostic.id.is_some());
}

// --- env kinds ------------------------------------------------------------------------------------------

#[test]
fn environment_conditions_follow_spec() {
    assert!(EnvironmentKind::Ssr.is_ssr());
    assert!(!EnvironmentKind::Client.is_ssr());
    assert!(EnvironmentKind::Client
        .default_conditions()
        .contains(&"browser".to_string()));
}
