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

// --- re-exported vocabulary --------------------------------------------------
pub use ferrite_config::{
    load_user_config, merge_user_config, resolve_config, CliOverrides, ResolvedConfig, UserConfig,
};
pub use ferrite_core::{
    Environment, EnvironmentKind, FerriteError, Hash, ModuleId, ModuleType, Result, SourceMap,
    Target, VERSION,
};
pub use ferrite_plugin::{Apply, Enforce, Plugin, PluginContainer};
pub use ferrite_server::DevServer;

mod api;
mod app;
mod builder;
mod loader;
mod report;

pub use api::{build, create_builder, create_server, preview, Config};
pub use app::{Ferrite, ModuleRequest};
pub use builder::Builder;
pub use loader::{preview_dir, BuildLoader};
pub use report::BuildReport;

/// Prelude for framework and plugin authors.
pub mod prelude {
    pub use crate::{
        Apply, Config, Enforce, Environment, EnvironmentKind, FerriteError, Hash, ModuleId,
        ModuleType, Plugin, PluginContainer, ResolvedConfig, Result, SourceMap, Target, UserConfig,
        VERSION,
    };
    pub use async_trait::async_trait;
}
