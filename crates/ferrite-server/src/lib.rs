//! Dev server (spec §24–§26, §76, §79).
//!
//! Native-ESM dev mode: resolve → load → transform → rewrite imports →
//! serve, with file watching, HMR broadcast, and middleware embedding.

mod env;
mod loader;
mod pipeline_context;
#[cfg(test)]
mod pipeline_tests;
mod pipeline_transform;
mod proxy;
mod routes;
mod server;
mod ssr_tools;
mod state;
mod types;
mod util;
mod watcher;

pub use env::{expand_vars, load_env, load_env_files, parse_dotenv};
pub use proxy::{forward as forward_proxy, match_proxy, rules_from_config};
pub use ssr_tools::{ModuleRunner, SsrTransformResult};
pub use state::{DevServer, DevServerInner};
pub use types::{CachedTransform, PipelineModule, PipelineResponse};
pub use util::{default_compiler, url_to_virtual, virtual_url};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::*;
    use ferrite_core::ModuleId;
    use ferrite_plugin::{HtmlTransformContext, Plugin, PluginContext};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    #[test]
    fn dotenv_parses() {
        let pairs = parse_dotenv("A=1\n# comment\nexport B=\"two\"\nEMPTY=\n");
        assert!(pairs.contains(&("A".to_string(), "1".to_string())));
        assert!(pairs.contains(&("B".to_string(), "two".to_string())));
    }

    #[test]
    fn dotenv_quotes_comments_escapes() {
        let pairs = parse_dotenv(
            "PLAIN=hi # trailing\nSINGLE='a#b' # kept\nESC=\"a\\n\\\"b\\\"\"\nHASH=\"a#b\"\n",
        );
        let get = |key: &str| {
            pairs
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, value)| value.clone())
                .unwrap()
        };
        assert_eq!(get("PLAIN"), "hi");
        assert_eq!(get("SINGLE"), "a#b");
        assert_eq!(get("ESC"), "a\n\"b\"");
        assert_eq!(get("HASH"), "a#b");
    }

    #[test]
    fn env_expansion_forms() {
        let loaded = HashMap::from([("BASE".to_string(), "/srv".to_string())]);
        assert_eq!(expand_vars("$BASE/x", &loaded), "/srv/x");
        assert_eq!(expand_vars("${BASE}/x", &loaded), "/srv/x");
        assert_eq!(expand_vars("${MISSING:-dflt}", &loaded), "dflt");
        assert_eq!(expand_vars("$$BASE", &loaded), "$BASE");
        assert_eq!(expand_vars("${MISSING}", &loaded), "");
        // Process environment wins over loaded values for references.
        let unique = format!("FERRITE_TEST_EXPAND_{}", std::process::id());
        std::env::set_var(&unique, "proc");
        let loaded = HashMap::from([(unique.clone(), "file".to_string())]);
        assert_eq!(expand_vars(&format!("${{{unique}}}"), &loaded), "proc");
        std::env::remove_var(&unique);
    }

    #[test]
    fn load_env_layers_and_filters() {
        let dir = std::env::temp_dir().join(format!("ferrite-loadenv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "FERRITE_A=1\nSECRET=no\n").unwrap();
        std::fs::write(
            dir.join(".env.production"),
            "FERRITE_A=2\nFERRITE_B=\"${FERRITE_A}/b\"\n",
        )
        .unwrap();
        let prefixes = vec!["FERRITE_".to_string()];
        let values = load_env("production", &dir, &prefixes);
        assert_eq!(values.get("FERRITE_A").unwrap(), "2");
        assert_eq!(values.get("FERRITE_B").unwrap(), "2/b");
        assert!(!values.contains_key("SECRET"));
        let dev = load_env("development", &dir, &prefixes);
        assert_eq!(dev.get("FERRITE_A").unwrap(), "1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn link_direct_rewrite() {
        let html = "<link rel=\"stylesheet\" href=\"/style.css\">";
        assert!(rewrite_link_direct(html).contains("/style.css?direct"));
    }

    #[test]
    fn requires_collected() {
        let code = "const a = require(\"./a\"); const b = require('./b');";
        let requires = ferrite_transform::analyze_commonjs("/fixture.cjs", code)
            .unwrap()
            .requires;
        assert_eq!(requires.len(), 2);
    }

    #[test]
    fn bare_specifier_classification() {
        for bare in ["react", "lodash-es", "@scope/name", "my-lib/sub"] {
            assert!(is_bare_specifier(bare), "{bare}");
        }
        for resolved in [
            "/src/a.js",
            "./a.js",
            "../a.js",
            ".",
            "..",
            "#internal",
            "node:path",
            "rust:serde",
            "https://x/y.js",
            "data:text/javascript,1",
            "blob:xyz",
            "\0virtual",
        ] {
            assert!(!is_bare_specifier(resolved), "{resolved}");
        }
    }

    #[tokio::test]
    async fn import_map_strategy_leaves_bare_imports() {
        let dir = std::env::temp_dir().join(format!("ferrite-importmap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("index.html"),
            "<!doctype html><html><head></head><body>\
             <script type=\"module\" src=\"/src/main.js\"></script></body></html>",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/main.js"),
            "import { x } from \"my-lib\";\nimport { y } from \"./other.js\";\nconsole.log(x, y);\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/other.js"), "export const y = 2;\n").unwrap();
        std::fs::write(dir.join("src/lib.js"), "export const x = 1;\n").unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.npm.dev_strategy = "import-map".to_string();
        user.resolve
            .alias
            .insert("my-lib".to_string(), "./src/lib.js".to_string());
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let html = server.transform_index_html("/index.html").await.unwrap();
        assert!(html.contains("<script type=\"importmap\">"), "{html}");
        assert!(html.contains("\"my-lib\""), "{html}");
        assert!(html.contains("/src/lib.js"), "{html}");
        let map_pos = html.find("importmap").expect("map");
        let module_pos = html.find("type=\"module\"").expect("module script");
        assert!(map_pos < module_pos, "{html}");
        let module = server
            .pipeline_module(&ModuleId::new("/src/main.js"), None, "client")
            .await
            .unwrap();
        assert!(module.code.contains("from \"my-lib\""), "{}", module.code);
        assert!(!module.code.contains("./other.js"), "{}", module.code);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn graph_keeps_ssr_and_client_envs_separate() {
        let dir = std::env::temp_dir().join(format!("ferrite-envsep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.js"), "export const x = 1;\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let id = ModuleId::new("/src/main.js");
        let client = server.pipeline_module(&id, None, "client").await.unwrap();
        let ssr = server.pipeline_module(&id, None, "ssr").await.unwrap();
        assert_ne!(client.code, ssr.code);
        let node = server.inner().graph.get(&id).unwrap();
        let graph_client = node
            .env("client")
            .and_then(|data| data.code.clone())
            .unwrap();
        let graph_ssr = node.env("ssr").and_then(|data| data.code.clone()).unwrap();
        assert_eq!(graph_client, client.code);
        assert_eq!(graph_ssr, ssr.code);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn cache_key_tracks_backend_and_defines() {
        let dir = std::env::temp_dir().join(format!("ferrite-cachekey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let mut swc_user = ferrite_config::UserConfig::default();
        swc_user.compiler.engine = "swc".to_string();
        let swc_config = ferrite_config::resolve_config(
            swc_user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let swc_server = DevServer::new_without_watcher(swc_config, Vec::new())
            .await
            .unwrap();
        let id = ModuleId::new("/src/a.js");
        let defines = HashMap::from([("A".to_string(), "1".to_string())]);
        let key_oxc = server.cache_key(&id, "const a = 1;", "client", &defines);
        let key_swc = swc_server.cache_key(&id, "const a = 1;", "client", &defines);
        assert_ne!(key_oxc, key_swc);
        let other_defines = HashMap::from([("A".to_string(), "2".to_string())]);
        let key_changed = server.cache_key(&id, "const a = 1;", "client", &other_defines);
        assert_ne!(key_oxc, key_changed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct TagsOnlyPlugin;

    #[async_trait::async_trait]
    impl Plugin for TagsOnlyPlugin {
        fn name(&self) -> &'static str {
            "tags-only"
        }

        async fn transform_index_html(
            &self,
            _ctx: &PluginContext,
            _html: HtmlTransformContext,
        ) -> ferrite_core::Result<Option<ferrite_plugin::HtmlTransformResult>> {
            Ok(Some(ferrite_plugin::HtmlTransformResult {
                html: None,
                tags: vec![ferrite_plugin::HtmlTag {
                    tag: "meta".to_string(),
                    attrs: HashMap::from([("name".to_string(), "tags-only".to_string())]),
                    children: None,
                    inject_to: ferrite_plugin::HtmlInjectTo::Head,
                }],
            }))
        }
    }

    #[tokio::test]
    async fn tags_only_hook_keeps_core_rewrites() {
        let dir = std::env::temp_dir().join(format!("ferrite-tagsonly-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("index.html"),
            "<!doctype html><html><head></head><body>\
             <script type=\"module\" src=\"/src/main.js\"></script></body></html>",
        )
        .unwrap();
        std::fs::write(dir.join("src/main.js"), "console.log(1);\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, vec![Arc::new(TagsOnlyPlugin)])
            .await
            .unwrap();
        let html = server.transform_index_html("/index.html").await.unwrap();
        // Core rewrite (dev client injection) survives the tags-only hook.
        assert!(html.contains("/@ferrite/client"), "{html}");
        assert!(html.contains("/src/main.js"), "{html}");
        assert!(html.contains("tags-only"), "{html}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn watcher_pushes_hmr_update_on_change() {
        // Regression: the watcher callback used `tokio::spawn` with no
        // runtime context, panicking the notify thread on first change.
        let dir = std::env::temp_dir().join(format!(
            "ferrite-hmrpush-{}-{}",
            std::process::id(),
            now_millis()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.js"), "console.log(1);\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new(config, Vec::new()).await.unwrap();
        server
            .pipeline_module(&ModuleId::new("/src/main.js"), None, "client")
            .await
            .unwrap();
        let mut updates = server.inner().hmr.subscribe();
        std::fs::write(dir.join("src/main.js"), "console.log(2);\n").unwrap();
        let message = tokio::time::timeout(Duration::from_secs(10), updates.recv())
            .await
            .expect("hmr push arrives")
            .expect("message");
        assert!(
            message.contains("update") || message.contains("reload"),
            "{message}"
        );
        server.close();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remote_allow_matching() {
        let allow = ["esm.example".to_string(), "*.cdn.example".to_string()];
        assert!(remote_host_allowed("esm.example", &allow));
        assert!(remote_host_allowed("ESM.EXAMPLE.", &allow));
        assert!(remote_host_allowed("a.cdn.example", &allow));
        assert!(!remote_host_allowed("cdn.example", &allow));
        assert!(!remote_host_allowed("evil.com", &allow));
        assert!(!remote_host_allowed("a.cdn.example.evil.com", &allow));
        assert!(!remote_host_allowed("esm.example", &[]));
    }

    #[tokio::test]
    async fn remote_import_allowed_and_cached() {
        let app = axum::Router::new()
            .route(
                "/pkg.js",
                axum::routing::get(|| async {
                    "import { d } from \"./dep.js\";\nexport const v = d + 1;\n"
                }),
            )
            .route(
                "/dep.js",
                axum::routing::get(|| async { "export const d = 41;\n" }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await });

        let dir = std::env::temp_dir().join(format!("ferrite-remote-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.remote.enabled = true;
        user.remote.allow = vec!["127.0.0.1".to_string()];
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let url = format!("http://{addr}/pkg.js");
        let id = ModuleId::new(format!("\0remote:{url}"));
        let module = server.pipeline_module(&id, None, "client").await.unwrap();
        // Relative dep rebased onto the remote origin, served virtually.
        assert!(!module.code.contains("./dep.js"), "{}", module.code);
        assert!(module.code.contains("/@id/"), "{}", module.code);
        // Kill the origin: a fresh server on the same root serves from disk.
        task.abort();
        let mut user = ferrite_config::UserConfig::default();
        user.remote.enabled = true;
        user.remote.allow = vec!["127.0.0.1".to_string()];
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let offline = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let cached = offline.pipeline_module(&id, None, "client").await.unwrap();
        assert_eq!(cached.code, module.code);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn remote_import_denied_loudly() {
        let dir = std::env::temp_dir().join(format!("ferrite-remote-deny-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Disabled by default.
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let error = server
            .pipeline_module(&ModuleId::new("https://esm.example/pkg.js"), None, "client")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("[remote]"), "{error}");
        // Wrong host.
        let mut user = ferrite_config::UserConfig::default();
        user.remote.enabled = true;
        user.remote.allow = vec!["esm.example".to_string()];
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let error = server
            .pipeline_module(&ModuleId::new("https://evil.example/x.js"), None, "client")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("evil.example"), "{error}");
        // Plain http outside loopback, even when allowlisted.
        let mut user = ferrite_config::UserConfig::default();
        user.remote.enabled = true;
        user.remote.allow = vec!["esm.example".to_string()];
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let error = server
            .pipeline_module(&ModuleId::new("http://esm.example/x.js"), None, "client")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("https"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rpc_handler_negotiates_binary_encoding() {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

        let dir = std::env::temp_dir().join(format!("ferrite-rpc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        server
            .inner()
            .rpc
            .write()
            .await
            .register("echo1", |args| async move { Ok(args) });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, server.router()).await });

        for encoding in [
            ferrite_ssr::RpcEncoding::MessagePack,
            ferrite_ssr::RpcEncoding::Cbor,
            ferrite_ssr::RpcEncoding::Json,
        ] {
            let body = encoding.encode(&serde_json::json!({"n": 2})).unwrap();
            let head = format!(
                "POST /_ferrite/rpc/echo1 HTTP/1.1\r\nhost: x\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                encoding.content_type(),
                body.len()
            );
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            stream.write_all(head.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
            let mut raw = Vec::new();
            stream.read_to_end(&mut raw).await.unwrap();
            let text = String::from_utf8_lossy(&raw).into_owned();
            let split = text.find("\r\n\r\n").expect("header/body split");
            let (head, _) = text.split_at(split);
            assert!(head.contains("200"), "{head:?}");
            assert!(head.contains(encoding.content_type()), "{head:?}");
            // Body bytes follow the header block verbatim.
            let body_bytes = &raw[split + 4..];
            assert_eq!(
                encoding.decode(body_bytes).unwrap(),
                serde_json::json!({"n": 2}),
                "{encoding:?}"
            );
        }
        task.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct RetransformPlugin {
        transforms: Arc<Mutex<usize>>,
        checks: Arc<Mutex<usize>>,
    }

    #[async_trait::async_trait]
    impl Plugin for RetransformPlugin {
        fn name(&self) -> &'static str {
            "retransform"
        }

        async fn transform(
            &self,
            _ctx: &PluginContext,
            request: ferrite_plugin::TransformRequest,
        ) -> ferrite_core::Result<Option<ferrite_plugin::TransformResult>> {
            *self.transforms.lock().unwrap() += 1;
            Ok(Some(ferrite_plugin::TransformResult {
                code: format!("{}\n// touched\n", request.code),
                map: None,
                dependencies: Vec::new(),

                module_type: None,
            }))
        }

        async fn should_transform_cached_module(
            &self,
            _ctx: &PluginContext,
            _module: ferrite_plugin::CachedModuleInfo,
        ) -> ferrite_core::Result<Option<bool>> {
            let mut checks = self.checks.lock().unwrap();
            *checks += 1;
            Ok(Some(*checks == 1))
        }
    }

    #[tokio::test]
    async fn should_transform_cached_module_forces_retransform() {
        let dir = std::env::temp_dir().join(format!("ferrite-retransform-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.js"), "console.log(1);\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let transforms = Arc::new(Mutex::new(0));
        let checks = Arc::new(Mutex::new(0));
        let server = DevServer::new_without_watcher(
            config,
            vec![Arc::new(RetransformPlugin {
                transforms: transforms.clone(),
                checks: checks.clone(),
            })],
        )
        .await
        .unwrap();
        let id = ModuleId::new("/src/main.js");
        server.pipeline_module(&id, None, "client").await.unwrap();
        server.pipeline_module(&id, None, "client").await.unwrap();
        server.pipeline_module(&id, None, "client").await.unwrap();
        // Miss, forced re-transform, cached.
        assert_eq!(*transforms.lock().unwrap(), 2);
        assert_eq!(*checks.lock().unwrap(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct DynamicPlugin;

    #[async_trait::async_trait]
    impl Plugin for DynamicPlugin {
        fn name(&self) -> &'static str {
            "dynamic"
        }

        async fn resolve_dynamic_import(
            &self,
            _ctx: &PluginContext,
            request: ferrite_plugin::DynamicImportRequest,
        ) -> ferrite_core::Result<Option<ferrite_resolver::ResolvedId>> {
            if request.specifier == "virtual:dyn" {
                return Ok(Some(ferrite_resolver::ResolvedId::new("/src/dep.js")));
            }
            Ok(None)
        }
    }

    #[tokio::test]
    async fn resolve_dynamic_import_hook_wins() {
        let dir = std::env::temp_dir().join(format!("ferrite-dynhook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.js"),
            "const m = await import(\"virtual:dyn\");\nconsole.log(m);\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/dep.js"), "export const x = 1;\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, vec![Arc::new(DynamicPlugin)])
            .await
            .unwrap();
        let module = server
            .pipeline_module(&ModuleId::new("/src/main.js"), None, "client")
            .await
            .unwrap();
        assert!(
            module.imports.iter().any(|(_, dep, kind)| {
                dep.0 == "/src/dep.js" && *kind == ferrite_graph::ImportKind::Dynamic
            }),
            "{:?}",
            module.imports
        );
        assert!(module.code.contains("/src/dep.js"), "{}", module.code);
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct FileUrlPlugin;

    #[async_trait::async_trait]
    impl Plugin for FileUrlPlugin {
        fn name(&self) -> &'static str {
            "file-url"
        }

        async fn resolve_file_url(
            &self,
            _ctx: &PluginContext,
            _request: ferrite_plugin::ResolveFileUrlRequest,
        ) -> ferrite_core::Result<Option<String>> {
            Ok(Some("https://cdn.example/x.png?v=1".to_string()))
        }
    }

    #[tokio::test]
    async fn resolve_file_url_rewrites_dev_url_shim() {
        let dir = std::env::temp_dir().join(format!("ferrite-fileurl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("logo.png"), b"fakepng").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, vec![Arc::new(FileUrlPlugin)])
            .await
            .unwrap();
        let module = server
            .pipeline_module(&ModuleId::new("/logo.png?url"), None, "client")
            .await
            .unwrap();
        assert!(
            module.code.contains("https://cdn.example/x.png?v=1"),
            "{}",
            module.code
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn ssr_transform_reports_deps_without_graph() {
        let dir = std::env::temp_dir().join(format!("ferrite-ssr-t-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/dep.js"), "export const x = 1;\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let result = server
            .ssr_transform(
                "import { x } from \"./dep.js\";\nexport const y = x + 1;\n",
                "/src/main.js",
            )
            .await
            .unwrap();
        assert_eq!(result.deps, vec!["/src/dep.js".to_string()]);
        assert!(result.dynamic_deps.is_empty());
        assert!(!result.code.contains("/@ferrite/client"), "{}", result.code);
        assert!(server.inner().graph.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn ssr_fix_stacktrace_normalizes_and_maps() {
        let dir = std::env::temp_dir().join(format!("ferrite-ssr-fix-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/a.js"),
            "export function boom() {\n  throw new Error(\"x\");\n}\n",
        )
        .unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        // Populate the SSR transform cache with a map.
        server
            .pipeline_module(&ModuleId::new("/src/a.js"), None, "ssr")
            .await
            .unwrap();
        let stack =
            "Error: x\n    at boom (http://127.0.0.1:5173/src/a.js:2:9)\n    at /src/a.js:1:1";
        let fixed = server.ssr_fix_stacktrace(stack).await;
        assert!(!fixed.contains("http://127.0.0.1:5173"), "{fixed}");
        assert!(fixed.contains("/src/a.js:"), "{fixed}");
        assert!(fixed.contains("Error: x"), "{fixed}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn module_runner_caches_and_invalidates() {
        let dir = std::env::temp_dir().join(format!("ferrite-runner-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.js"), "export const x = 1;\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let runner = server.module_runner();
        let first = runner.import("/src/main.js").await.unwrap();
        let second = runner.import("/src/main.js").await.unwrap();
        assert_eq!(first.code, second.code);
        assert_eq!(runner.cached_urls(), vec!["/src/main.js".to_string()]);
        runner.invalidate("/src/main.js");
        assert!(runner.cached_urls().is_empty());
        runner.import("/src/main.js").await.unwrap();
        runner.close().await;
        assert!(runner.cached_urls().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    struct WatchPlugin {
        events: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl Plugin for WatchPlugin {
        fn name(&self) -> &'static str {
            "watch"
        }

        async fn watch_change(
            &self,
            _ctx: &PluginContext,
            event: ferrite_plugin::WatchEvent,
        ) -> ferrite_core::Result<()> {
            self.events
                .lock()
                .unwrap()
                .push(format!("{}:{:?}", event.path.display(), event.kind));
            Ok(())
        }
    }

    #[tokio::test]
    async fn watch_change_fires_on_edit() {
        let dir = std::env::temp_dir().join(format!("ferrite-watchhook-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.js"), "console.log(1);\n").unwrap();
        let config = ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let server = DevServer::new(
            config,
            vec![Arc::new(WatchPlugin {
                events: events.clone(),
            })],
        )
        .await
        .unwrap();
        assert!(ferrite_plugin::ServerControl::watcher_alive(&server));
        std::fs::write(dir.join("src/main.js"), "console.log(2);\n").unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            if !events.lock().unwrap().is_empty() || std::time::Instant::now() > deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let seen = events.lock().unwrap().join("\n");
        assert!(seen.contains("main.js"), "{seen}");
        server.close();
        assert!(!ferrite_plugin::ServerControl::watcher_alive(&server));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn server_handles_and_urls() {
        let dir = std::env::temp_dir().join(format!("ferrite-handles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.server.port = 5199;
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        assert!(ferrite_plugin::ServerControl::module_graph(&server).is_some());
        assert!(ferrite_plugin::ServerControl::local_addr(&server).is_none());
        assert_eq!(ferrite_plugin::ServerControl::hmr_clients(&server), 0);
        let urls = ferrite_plugin::ServerControl::server_urls(&server);
        assert!(urls.local.contains("5199"), "{}", urls.local);
        server.print_urls();
        // Extra watch paths are recorded even without a running watcher.
        server.watch_extra(&dir.join("extra"));
        assert!(server
            .inner()
            .watch_files
            .lock()
            .map(|files| files.iter().any(|file| file.contains("extra")))
            .unwrap_or(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn proxy_forwards_to_origin() {
        let app = axum::Router::new().route(
            "/api/echo",
            axum::routing::post(|headers: axum::http::HeaderMap, body: String| async move {
                let tag = headers
                    .get("x-tag")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("-")
                    .to_string();
                format!("{tag}:{body}")
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await });

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-tag", axum::http::HeaderValue::from_static("t1"));
        let client = reqwest::Client::new();
        let response = forward_proxy(
            &client,
            &axum::http::Method::POST,
            &format!("http://{addr}/api/echo"),
            &headers,
            axum::body::Bytes::from("hello"),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"t1:hello");

        let rules = rules_from_config(&HashMap::from([(
            "/api".to_string(),
            format!("http://{addr}"),
        )]));
        assert!(match_proxy(&rules, "/api/echo").is_some());
        assert!(match_proxy(&rules, "/other").is_none());
        task.abort();
    }

    #[tokio::test]
    async fn rewrite_strategy_still_rewrites_bare_imports() {
        let dir = std::env::temp_dir().join(format!("ferrite-rewrite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.js"),
            "import { x } from \"my-lib\";\nconsole.log(x);\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/lib.js"), "export const x = 1;\n").unwrap();
        let mut user = ferrite_config::UserConfig::default();
        user.resolve
            .alias
            .insert("my-lib".to_string(), "./src/lib.js".to_string());
        let config = ferrite_config::resolve_config(
            user,
            Some(dir.clone()),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let server = DevServer::new_without_watcher(config, Vec::new())
            .await
            .unwrap();
        let module = server
            .pipeline_module(&ModuleId::new("/src/main.js"), None, "client")
            .await
            .unwrap();
        assert!(!module.code.contains("from \"my-lib\""), "{}", module.code);
        assert!(module.code.contains("/src/lib.js"), "{}", module.code);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
