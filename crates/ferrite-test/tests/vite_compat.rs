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

// --- tree-shaking -------------------------------------------------------------------------

#[tokio::test]
async fn production_build_drops_unused_exports() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { keepme } from \"./lib\";\nconsole.log(keepme);\n",
        ),
        (
            "src/lib.ts",
            "export const keepme = 1;\nexport const dropme = 2;\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut found_keep = false;
    let mut found_drop = false;
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let code = std::fs::read_to_string(&path).expect("chunk");
        // Skip the entry chunk (entries keep everything); the lib chunk
        // must keep `keepme` and lose `dropme`.
        if code.contains("console") {
            continue;
        }
        found_keep |= code.contains("keepme");
        found_drop |= code.contains("dropme");
    }
    assert!(found_keep, "shaken lib chunk missing `keepme`");
    assert!(!found_drop, "shaken lib chunk still contains `dropme`");
}

#[tokio::test]
async fn production_build_keeps_barrel_reexports_in_use() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { via } from \"./barrel\";\nconsole.log(via);\n",
        ),
        ("src/barrel.ts", "export { via } from \"./leaf\";\n"),
        (
            "src/leaf.ts",
            "export const via = 1;\nexport const gone = 2;\n",
        ),
    ]);
    let config = project.resolve_config_mode("production");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut blob = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    assert!(blob.contains("via"), "{blob}");
    assert!(!blob.contains("gone"), "{blob}");
}

// --- standalone packaging -------------------------------------------------------------------

fn standalone_config(project: &TempProject, target: Option<&str>) -> ferrite::ResolvedConfig {
    ferrite::resolve_config(
        ferrite::UserConfig::default(),
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            standalone: Some(true),
            target: target.map(str::to_string),
            ..Default::default()
        },
    )
    .expect("resolve")
}

fn standalone_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\ndocument.body.textContent = greet(\"ferrite\");\n",
        ),
        (
            "src/greet.ts",
            "export function greet(name: string): string {\n  return `hello ${name}`;\n}\n",
        ),
    ]
}

#[tokio::test]
async fn standalone_build_embeds_scaffold() {
    let project = TempProject::new(&standalone_files());
    let config = standalone_config(&project, None);
    assert!(config.package.target.is_none());
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let scaffold = report.out_dir.join("standalone");
    assert!(scaffold.join("Cargo.toml").is_file());
    assert!(scaffold.join("src/main.rs").is_file());
    let assets = std::fs::read_to_string(scaffold.join("src/assets.rs")).expect("assets.rs");
    assert!(assets.contains("/index.html"), "{assets}");
    assert!(assets.contains("include_bytes!"), "{assets}");
    let main = std::fs::read_to_string(scaffold.join("src/main.rs")).expect("main.rs");
    assert!(main.contains("fallback(handler)"), "{main}");
    assert!(report.standalone_binary.is_none());
}

/// Compiles the generated scaffold for the host triple and boots the
/// binary: the single-binary claim, end to end. Slow (real cargo build).
#[tokio::test]
#[ignore = "slow: real cargo build + boot of the standalone binary"]
async fn standalone_build_compiles_and_serves() {
    let host = rustc_host_triple();
    let project = TempProject::new(&standalone_files());
    let config = standalone_config(&project, Some(&host));
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let binary = report.standalone_binary.expect("prebuilt binary");
    assert!(binary.is_file(), "{}", binary.display());
    // Boot from an empty cwd: the binary must serve embedded bytes with no
    // sibling dist/.
    let serve_dir = tempfile::tempdir().expect("tempdir");
    let port = free_port();
    let mut child = std::process::Command::new(&binary)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .current_dir(serve_dir.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn");
    wait_until_ready(port);
    let body = http_get(port, "/index.html");
    assert!(body.contains("200"), "{body}");
    assert!(body.contains("<script"), "{body}");
    let missing = http_get(port, "/does-not-exist");
    assert!(missing.contains("404"), "{missing}");
    child.kill().ok();
    child.wait().ok();
}

/// Cross-compiles the scaffold for musl: the cross-target claim.
/// Slow (real `cargo build --target`).
#[tokio::test]
#[ignore = "slow: real musl cross build"]
async fn standalone_build_cross_compiles_musl() {
    let project = TempProject::new(&standalone_files());
    let config = standalone_config(&project, Some("x86_64-unknown-linux-musl"));
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let binary = report.standalone_binary.expect("prebuilt binary");
    assert!(binary.is_file(), "{}", binary.display());
    assert!(binary
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains("x86_64-unknown-linux-musl"));
}

/// Production build through the SWC engine: TS strip, JSX, minify,
/// and shake re-minification all run on SWC.
#[cfg(feature = "swc")]
#[tokio::test]
async fn production_build_with_swc_engine() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.tsx\"></script></body></html>",
        ),
        (
            "src/main.tsx",
            "import { keepme } from \"./lib\";\n\
             export const el = <div>{keepme}</div>;\n\
             console.log(el);\n",
        ),
        (
            "src/lib.ts",
            "export const keepme: number = 1;\nexport const dropme: number = 2;\n",
        ),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.compiler.engine = "swc".to_string();
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert_eq!(config.compiler.engine, "swc");
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let mut blob = String::new();
    for entry in std::fs::read_dir(report.out_dir.join("assets")).expect("assets") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        blob.push_str(&std::fs::read_to_string(&path).expect("chunk"));
    }
    // JSX compiled, TS stripped, unused export shaken — all via SWC.
    assert!(
        blob.contains("jsx-runtime") || blob.contains("jsxDEV"),
        "{blob}"
    );
    assert!(!blob.contains(": number"), "{blob}");
    assert!(!blob.contains("dropme"), "{blob}");
}

#[tokio::test]
async fn production_build_scope_hoists_entry_closure() {
    let project = TempProject::new(&[
        (
            "index.html",
            "<!doctype html><html><head><title>t</title></head><body>\
             <script type=\"module\" src=\"/src/main.ts\"></script></body></html>",
        ),
        (
            "src/main.ts",
            "import { greet } from \"./greet\";\nconsole.log(greet(\"hoist\"));\n",
        ),
        (
            "src/greet.ts",
            "import { punct } from \"./punct\";\nexport function greet(n: string): string { return `hi ${n}${punct}`; }\n",
        ),
        ("src/punct.ts", "export const punct = \"!\";\n"),
    ]);
    let mut user = ferrite::UserConfig::default();
    user.build.scope_hoist = true;
    let config = ferrite::resolve_config(
        user,
        Some(project.root.clone()),
        ferrite::CliOverrides {
            mode: Some("production".to_string()),
            ..Default::default()
        },
    )
    .expect("resolve");
    assert!(config.build.scope_hoist);
    let builder = ferrite::Builder::new(config, vec![]);
    let report = builder.build("client").await.unwrap();
    let js: Vec<std::path::PathBuf> = std::fs::read_dir(report.out_dir.join("assets"))
        .expect("assets")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("js"))
        .collect();
    assert_eq!(js.len(), 1, "{js:?}");
    let code = std::fs::read_to_string(&js[0]).expect("chunk");
    // One scope: no surviving inside imports, prefixed bindings, kept log.
    assert!(!code.contains("from\"./"), "{code}");
    assert!(!code.contains("from \"./"), "{code}");
    assert!(code.contains("$f"), "{code}");
    assert!(code.contains("console.log"), "{code}");
}

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

fn rustc_host_triple() -> String {
    let output = std::process::Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_string))
        .expect("host triple")
}

fn free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    listener.local_addr().expect("addr").port()
}

fn wait_until_ready(port: u16) {
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("server on {port} never came up");
}

fn http_get(port: u16, path: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
    write!(stream, "GET {path} HTTP/1.0\r\nHost: x\r\n\r\n").expect("write");
    let mut body = String::new();
    stream.read_to_string(&mut body).expect("read");
    body
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
