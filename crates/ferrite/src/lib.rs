//! Ferrite — Rust-native SSR web toolchain (spec §9).
//!
//! Public API (Vite programmatic-API equivalent):
//!
//! ```rust,no_run
//! #[tokio::main]
//! async fn main() -> ferrite::Result<()> {
//!     let mut server = ferrite::create_server(ferrite::Config::default()).await?;
//!     server.listen().await?;
//!     Ok(())
//! }
//! ```
//!
//! Builder:
//!
//! ```rust,no_run
//! # async fn run() -> ferrite::Result<()> {
//! let builder = ferrite::create_builder(ferrite::Config::default()).await?;
//! builder.build_app().await?;
//! # Ok(())
//! # }
//! ```

// --- re-exported crates ------------------------------------------------------
pub use ferrite_assets as assets;
pub use ferrite_bundler as bundler;
pub use ferrite_cache as cache;
pub use ferrite_config as config;
pub use ferrite_core as core;
pub use ferrite_css as css;
pub use ferrite_docs as docs;
pub use ferrite_e2e as e2e;
pub use ferrite_frameworks as frameworks;
pub use ferrite_graph as graph;
pub use ferrite_hmr as hmr;
pub use ferrite_html as html;
pub use ferrite_manifest as manifest;
pub use ferrite_npm as npm;
pub use ferrite_plugin as plugin;
pub use ferrite_resolver as resolver;
pub use ferrite_runtime as runtime;
pub use ferrite_server as server;
pub use ferrite_ssr as ssr;
pub use ferrite_tailwind as tailwind;
pub use ferrite_transform as transform;
pub use ferrite_wasm as wasm;

pub mod package;
mod ssr_build;
pub use ssr_build::{load_built_ssr_graph, write_built_ssr_artifact, SsrRendererArtifact};

// --- re-exported vocabulary --------------------------------------------------
pub use ferrite_config::{
    define_config, load_config_from_file, load_user_config, load_user_config_path, merge_config,
    merge_user_config, resolve_config, CliOverrides, LoadedConfigFile, ResolvedConfig, UserConfig,
};
pub use ferrite_core::{
    normalize_path, search_for_workspace_root, Environment, EnvironmentKind, FerriteError, Hash,
    ModuleId, ModuleType, Result, SourceMap, Target, VERSION,
};
pub use ferrite_plugin::{
    Apply, BundleOptions, CachedModuleInfo, ChunkWrapper, DynamicImportRequest, Enforce,
    OutputOptions, Plugin, PluginContainer, PreviewControl, PreviewMount, ProxyRule,
    ResolveFileUrlRequest, ServerControl, ServerUrls, WatchEvent, WatchKind,
};
pub use ferrite_server::{
    expand_vars, forward_proxy, load_env, match_proxy, rules_from_config, DevServer, ModuleRunner,
    SsrTransformResult,
};

mod api;
mod app;
mod builder;
mod loader;
mod report;
mod ssr_dev;

pub use api::{build, create_builder, create_server, preview, Config};
pub use app::{Ferrite, ModuleRequest};
pub use builder::Builder;
pub use loader::{preview_dir, preview_with_plugins, BuildLoader};
pub use report::BuildReport;
pub use ssr_dev::{create_dev_ssr_adapter, inspect_capabilities};

/// Prelude for framework and plugin authors.
pub mod prelude {
    pub use crate::{
        define_config, load_env, merge_config, normalize_path, search_for_workspace_root, Apply,
        Config, Enforce, Environment, EnvironmentKind, FerriteError, Hash, ModuleId, ModuleType,
        Plugin, PluginContainer, ResolvedConfig, Result, SourceMap, Target, UserConfig, VERSION,
    };
    pub use async_trait::async_trait;
}
