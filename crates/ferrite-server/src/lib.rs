//! Dev server (spec §24–§26, §76, §79).
//!
//! Native-ESM dev mode: resolve → load → transform → rewrite imports →
//! serve, with file watching, HMR broadcast, and middleware embedding.

mod env;
mod loader;
mod pipeline_context;
mod pipeline_transform;
mod routes;
mod server;
mod state;
mod types;
mod util;
mod watcher;

pub use env::{load_env_files, parse_dotenv};
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
    use std::time::Duration;

    #[test]
    fn dotenv_parses() {
        let pairs = parse_dotenv("A=1\n# comment\nexport B=\"two\"\nEMPTY=\n");
        assert!(pairs.contains(&("A".to_string(), "1".to_string())));
        assert!(pairs.contains(&("B".to_string(), "two".to_string())));
    }

    #[test]
    fn link_direct_rewrite() {
        let html = "<link rel=\"stylesheet\" href=\"/style.css\">";
        assert!(rewrite_link_direct(html).contains("/style.css?direct"));
    }

    #[test]
    fn requires_collected() {
        let code = "const a = require(\"./a\"); const b = require('./b');";
        let requires = collect_requires(code);
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
