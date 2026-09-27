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
    Apply, BuildEnd, EmittedFile, Enforce, HookFilter, HotUpdateEvent, HotUpdateResult,
    HtmlInjectTo, HtmlTag, HtmlTransformContext, HtmlTransformResult, LoadRequest, LoadResult,
    ModuleParsed, OutputBundle, PluginContext, RenderChunk, RenderChunkResult, RenderStart,
    ResolveHookRequest, ServerControl, TransformRequest, TransformResult,
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
}
