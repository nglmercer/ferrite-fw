//! Server-side rendering (spec §21–§23, §46–§50).
//!
//! [`SsrAdapter`] is the framework boundary: pure-Rust SSR (Leptos/Dioxus/
//! custom), embedded-JS SSR via [`ferrite_runtime::JsRuntime`], or hybrid
//! Rust routes + JS islands. Also: externals, streaming, islands, and the
//! `/_ferrite/rpc/` server-function transport.

mod adapter;
mod external;
mod islands;
mod request;
mod router;
mod rpc;

pub use ferrite_runtime::{CompiledModule, CompiledModuleGraph};

pub use adapter::{FnAdapter, JsSsrAdapter, SsrAdapter, SsrModule, StaticShellAdapter};
pub use external::{is_external, is_native_specifier, native_shim_module};
pub use islands::{island_hydration_script, island_tag, Island};
pub use request::{RenderBody, SsrContext, SsrHttpRequest, SsrResponse};
pub use router::{match_route, SsrRouter};
pub use rpc::{RpcEncoding, RpcRegistry, RpcRequest, RPC_ROUTE_PREFIX};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::inject_shell;
    use ferrite_config::{ResolvedConfig, SsrConfig};
    use ferrite_runtime::CompiledModule;

    fn test_resolved() -> ResolvedConfig {
        ferrite_config::resolve_config(
            ferrite_config::UserConfig::default(),
            Some(std::env::temp_dir()),
            ferrite_config::CliOverrides::default(),
        )
        .expect("resolve test config")
    }

    #[test]
    fn external_matching() {
        let config = SsrConfig {
            external: vec!["pg".to_string()],
            no_external: vec!["my-esm-package".to_string()],
            ..Default::default()
        };
        assert!(is_external("pg", &config));
        assert!(is_external("pg/lib", &config));
        assert!(!is_external("my-esm-package", &config));
        assert!(!is_external("./local.ts", &config));
    }

    #[test]
    fn native_binaries_are_always_external() {
        let config = SsrConfig {
            bundle_all: true,
            no_external: vec!["./addon.node".to_string()],
            ..Default::default()
        };
        assert!(is_native_specifier("./addon.node"));
        assert!(is_native_specifier("./addon.node?v=1"));
        assert!(is_native_specifier("pkg/prebuilds/a.node"));
        assert!(!is_native_specifier("./addon.js"));
        assert!(is_external("./addon.node", &config));
        assert!(is_external("./addon.node?v=1", &SsrConfig::default()));
    }

    #[test]
    fn native_shim_throws_actionable_error() {
        let module = native_shim_module("./na'tive\\addon.node");
        assert_eq!(module.id, "./na'tive\\addon.node");
        assert!(module.dependencies.is_empty());
        assert!(
            module.code.starts_with("throw new Error("),
            "{}",
            module.code
        );
        assert!(module.code.contains("native_allow"), "{}", module.code);
        assert!(
            module.code.contains("--features napi-vm"),
            "{}",
            module.code
        );
        // Escaped literal: no raw quote or backslash breaks the JS string.
        assert!(
            module.code.contains("./na\\'tive\\\\addon.node"),
            "{}",
            module.code
        );
    }

    #[test]
    fn inject_shell_combines_body_and_preloads() {
        let shell = "<html><head></head><body><!--ssr-outlet--></body></html>";
        let html = inject_shell(
            shell,
            "<h1>hi</h1>",
            &[
                "/a.css".to_string(),
                "/b.js".to_string(),
                "/c.png".to_string(),
            ],
        );
        assert!(html.contains("<h1>hi</h1>"), "{html}");
        assert!(
            html.contains("<link rel=\"stylesheet\" href=\"/a.css\">"),
            "{html}"
        );
        assert!(
            html.contains("<link rel=\"modulepreload\" href=\"/b.js\">"),
            "{html}"
        );
        assert!(!html.contains("c.png"), "{html}");
        assert!(!html.contains("<!--ssr-outlet-->"), "{html}");
    }

    #[tokio::test]
    async fn unknown_backend_errors_loudly() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "does-not-exist".to_string();
        let adapter = JsSsrAdapter::from_resolved(
            &resolved,
            CompiledModule {
                id: "entry".to_string(),
                code: "export function render() { return \"x\"; }".to_string(),
                url: None,
            },
        );
        let error = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("does-not-exist"), "{error}");
    }

    #[cfg(feature = "napi-vm")]
    #[tokio::test]
    async fn renders_through_napi_vm() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "napi-vm".to_string();
        let adapter = JsSsrAdapter::from_resolved_graph(
            &resolved,
            ferrite_runtime::CompiledModuleGraph {
                entry: "/compiled/server.js".into(),
                modules: vec![
                    CompiledModule {
                        id: "/compiled/server.js".into(),
                        code: "import { greeting } from '/compiled/greeting.js'; export function render(url) { return `<h1>${greeting} ${url}</h1>`; }".into(),
                        url: None,
                    },
                    CompiledModule {
                        id: "/compiled/greeting.js".into(),
                        code: "export const greeting = 'hello from';".into(),
                        url: None,
                    },
                ],
            },
        ).expect("compiled graph adapter")
        .with_shell("<html><head></head><body><!--ssr-outlet--></body></html>");
        let response = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/about".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext {
                    preload: vec!["/app.js".to_string()],
                    ..Default::default()
                },
            )
            .await
            .expect("render");
        let html = response.into_string().await.expect("body");
        assert!(html.contains("<h1>hello from /about</h1>"), "{html}");
        assert!(
            html.contains("<link rel=\"modulepreload\" href=\"/app.js\">"),
            "{html}"
        );
    }

    #[cfg(feature = "napi-vm")]
    #[tokio::test]
    async fn bad_allowlist_mapping_fails_loudly() {
        let mut resolved = test_resolved();
        resolved.runtime.backend = "napi-vm".to_string();
        resolved.runtime.native_allow = vec!["/nonexistent-ferrite/missing.node".to_string()];
        let adapter = JsSsrAdapter::from_resolved(
            &resolved,
            CompiledModule {
                id: "entry".to_string(),
                code: "export function render() { return \"x\"; }".to_string(),
                url: None,
            },
        );
        let error = adapter
            .render(
                SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                SsrContext::default(),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("missing.node"), "{error}");
    }

    #[test]
    fn rpc_encodings_roundtrip() {
        let value = serde_json::json!({"id": 1, "tags": ["a", "b"], "nested": {"ok": true}});
        for encoding in [
            RpcEncoding::Json,
            RpcEncoding::MessagePack,
            RpcEncoding::Cbor,
        ] {
            let bytes = encoding.encode(&value).expect("encode");
            assert!(!bytes.is_empty());
            let back = encoding.decode(&bytes).expect("decode");
            assert_eq!(back, value, "{encoding:?}");
        }
        // Binary encodings are more compact than JSON here.
        let json_len = RpcEncoding::Json.encode(&value).unwrap().len();
        assert!(RpcEncoding::MessagePack.encode(&value).unwrap().len() < json_len);
        assert!(RpcEncoding::Cbor.encode(&value).unwrap().len() < json_len);
    }

    #[test]
    fn rpc_content_type_detection() {
        assert_eq!(RpcEncoding::from_content_type(None), RpcEncoding::Json);
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/json")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/json; charset=utf-8")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/msgpack")),
            RpcEncoding::MessagePack
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/x-msgpack")),
            RpcEncoding::MessagePack
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("application/cbor")),
            RpcEncoding::Cbor
        );
        assert_eq!(
            RpcEncoding::from_content_type(Some("text/plain")),
            RpcEncoding::Json
        );
        assert_eq!(
            RpcEncoding::MessagePack.content_type(),
            "application/msgpack"
        );
        assert_eq!(RpcEncoding::Cbor.content_type(), "application/cbor");
    }

    #[test]
    fn rpc_decode_rejects_garbage() {
        // Invalid UTF-8 (JSON), 0xc1 never-used byte (msgpack), tag(1)
        // followed by break (CBOR): all three must fail.
        for encoding in [
            RpcEncoding::Json,
            RpcEncoding::MessagePack,
            RpcEncoding::Cbor,
        ] {
            assert!(encoding.decode(b"\xc1\xff").is_err(), "{encoding:?}");
            assert!(encoding.decode(b"").is_err(), "{encoding:?} empty");
        }
    }

    #[tokio::test]
    async fn rpc_roundtrip() {
        let mut registry = RpcRegistry::new();
        registry.register("abc123", |args| async move { Ok(args) });
        let value = registry
            .invoke(&RpcRequest {
                hash: "abc123".to_string(),
                args: serde_json::json!({"id": 1}),
            })
            .await
            .unwrap();
        assert_eq!(value, serde_json::json!({"id": 1}));
    }

    #[tokio::test]
    async fn stream_collects() {
        let response = SsrResponse::stream(vec!["<a>".to_string(), "</a>".to_string()]);
        assert_eq!(response.into_string().await.unwrap(), "<a></a>");
    }

    #[test]
    fn route_params_match() {
        let params = super::match_route("/users/:id", "/users/42").unwrap();
        assert_eq!(params.get("id").unwrap(), "42");
        assert!(super::match_route("/users/:id", "/users").is_none());
        assert!(super::match_route("/a", "/b").is_none());
    }

    #[tokio::test]
    async fn router_dispatches_and_404s() {
        let mut router = super::SsrRouter::new();
        router.route("/users/:id", |_request, _context, params| async move {
            Ok(super::SsrResponse::html(format!("user {}", params["id"])))
        });
        let found = router
            .handle(
                super::SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/users/7".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                super::SsrContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(found.status, 200);
        let missing = router
            .handle(
                super::SsrHttpRequest {
                    method: "GET".to_string(),
                    uri: "/nope".to_string(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                super::SsrContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status, 404);
    }
}
