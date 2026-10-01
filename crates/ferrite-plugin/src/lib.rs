//! Rust-native plugin API with Vite/Rollup semantics (spec §11–§14).
//!
//! [`Plugin`] mirrors the Rollup/Vite hook surface; [`PluginContainer`]
//! orders hooks by [`Enforce`] and filters by [`Apply`].

/// Tier-2 JS plugin host (spec §56–§57).
pub mod js_host;
pub mod node_adapter;

mod builtin;
mod container;
mod host;
mod plugin;
mod types;

pub use builtin::RawTextPlugin;
pub use container::PluginContainer;
pub use host::{is_virtual_id, to_virtual_id, ForeignPluginHost, HookName, PluginHandle};
pub use plugin::Plugin;
pub use types::{
    Apply, BuildEnd, BundleOptions, CachedModuleInfo, ChunkWrapper, DynamicImportRequest,
    EmittedFile, Enforce, HookFilter, HotUpdateEvent, HotUpdateResult, HtmlInjectTo, HtmlTag,
    HtmlTransformContext, HtmlTransformResult, LoadRequest, LoadResult, ModuleParsed, OutputBundle,
    OutputOptions, PluginContext, PreviewControl, PreviewMount, ProxyRule, RenderChunk,
    RenderChunkResult, RenderStart, ResolveFileUrlRequest, ResolveHookRequest, ServerControl,
    ServerUrls, TransformPhase, TransformRequest, TransformResult, WatchEvent, WatchKind,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct OrderPlugin {
        name: &'static str,
        enforce: Enforce,
    }

    #[async_trait::async_trait]
    impl Plugin for OrderPlugin {
        fn name(&self) -> &'static str {
            self.name
        }

        fn enforce(&self) -> Enforce {
            self.enforce
        }
    }

    #[test]
    fn container_orders_by_enforce() {
        let container = PluginContainer::new(
            vec![
                Arc::new(OrderPlugin {
                    name: "b",
                    enforce: Enforce::Post,
                }),
                Arc::new(OrderPlugin {
                    name: "a",
                    enforce: Enforce::Pre,
                }),
            ],
            Apply::All,
        );
        assert_eq!(container.names(), vec!["a", "b"]);
    }

    #[test]
    fn hook_filter_matches() {
        let filter = HookFilter {
            id: Some(r"\.tsx?$".to_string()),
            code: None,
            query: None,
        };
        assert!(filter.matches("/src/a.ts", None));
        assert!(!filter.matches("/src/a.css", None));
    }

    #[test]
    fn proxy_rule_matching() {
        let rule = ProxyRule {
            prefix: "/api".to_string(),
            target: "http://localhost:3000/".to_string(),
        };
        assert!(rule.matches("/api"));
        assert!(rule.matches("/api/users"));
        assert!(!rule.matches("/apix"));
        assert!(!rule.matches("/other"));
        assert_eq!(
            rule.forward_url("/api/users?page=2"),
            "http://localhost:3000/api/users?page=2"
        );
    }

    #[test]
    fn chunk_wrapper_apply() {
        let wrapper = ChunkWrapper {
            banner: Some("/* banner */".to_string()),
            intro: None,
            outro: Some("console.log('out');".to_string()),
            footer: None,
        };
        let code = wrapper.apply("const a = 1;");
        assert!(code.starts_with("/* banner */\nconst a = 1;\n"), "{code}");
        assert!(code.ends_with("console.log('out');\n"), "{code}");
        assert_eq!(ChunkWrapper::default().apply("x"), "x");
    }

    struct HotUpdatePlugin {
        modern: bool,
    }

    #[async_trait::async_trait]
    impl Plugin for HotUpdatePlugin {
        fn name(&self) -> &'static str {
            "hot-update"
        }

        async fn handle_hot_update(
            &self,
            _ctx: &PluginContext,
            _event: HotUpdateEvent,
        ) -> ferrite_core::Result<Option<HotUpdateResult>> {
            Ok(Some(HotUpdateResult {
                modules: Vec::new(),
                full_reload: false,
            }))
        }

        async fn hot_update(
            &self,
            _ctx: &PluginContext,
            _event: HotUpdateEvent,
        ) -> ferrite_core::Result<Option<HotUpdateResult>> {
            if self.modern {
                return Ok(Some(HotUpdateResult {
                    modules: Vec::new(),
                    full_reload: true,
                }));
            }
            Ok(None)
        }
    }

    fn test_context<'a>(
        graph: &'a ferrite_graph::ModuleGraph,
        resolver: &'a ferrite_resolver::Resolver,
        environment: &'a ferrite_core::Environment,
        emitted: &'a std::sync::Mutex<std::collections::HashMap<String, EmittedFile>>,
        watch_files: &'a std::sync::Mutex<Vec<String>>,
        warnings: &'a std::sync::Mutex<Vec<String>>,
    ) -> PluginContext<'a> {
        PluginContext {
            graph,
            resolver,
            environment,
            emitted,
            watch_files,
            warnings,
        }
    }

    #[tokio::test]
    async fn hot_update_prefers_modern_hook() {
        let graph = ferrite_graph::ModuleGraph::new();
        let resolver = ferrite_resolver::Resolver::for_environment(
            std::path::PathBuf::from("/tmp/ferrite-hot-test"),
            &ferrite_config::ResolveConfig::default(),
            &ferrite_core::EnvironmentKind::Client,
        );
        let environment =
            ferrite_core::Environment::new("client", ferrite_core::EnvironmentKind::Client);
        let emitted = std::sync::Mutex::new(std::collections::HashMap::new());
        let watch_files = std::sync::Mutex::new(Vec::new());
        let warnings = std::sync::Mutex::new(Vec::new());
        let ctx = test_context(
            &graph,
            &resolver,
            &environment,
            &emitted,
            &watch_files,
            &warnings,
        );
        let event = HotUpdateEvent {
            file: "/src/a.js".to_string(),
            modules: Vec::new(),
            timestamp: 1,
        };
        // Modern hook wins.
        let container =
            PluginContainer::new(vec![Arc::new(HotUpdatePlugin { modern: true })], Apply::All);
        let result = container
            .hook_hot_update(&ctx, event.clone())
            .await
            .unwrap()
            .unwrap();
        assert!(result.full_reload);
        // Legacy fallback when the modern hook abstains.
        let container = PluginContainer::new(
            vec![Arc::new(HotUpdatePlugin { modern: false })],
            Apply::All,
        );
        let result = container
            .hook_hot_update(&ctx, event)
            .await
            .unwrap()
            .unwrap();
        assert!(!result.full_reload);
    }

    #[test]
    fn preview_control_seeds_and_orders_proxies() {
        let mut user = ferrite_config::UserConfig::default();
        user.server
            .proxy
            .insert("/".to_string(), "http://fallback".to_string());
        user.server
            .proxy
            .insert("/api".to_string(), "http://api".to_string());
        let resolved = ferrite_config::resolve_config(
            user,
            Some(std::path::PathBuf::from("/tmp/ferrite-preview-test")),
            ferrite_config::CliOverrides::default(),
        )
        .unwrap();
        let control = PreviewControl::new(resolved);
        // Longest prefix wins.
        assert_eq!(control.match_proxy("/api/x").unwrap().target, "http://api");
        assert_eq!(
            control.match_proxy("/other").unwrap().target,
            "http://fallback"
        );
        assert!(control.headers.is_empty());
    }
}
