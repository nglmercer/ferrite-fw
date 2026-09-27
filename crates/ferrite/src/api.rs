//! Programmatic configuration.

use crate::builder::*;
use crate::loader::*;
use crate::report::*;
use ferrite_config::load_user_config;
use ferrite_config::merge_user_config;
use ferrite_config::resolve_config;
use ferrite_config::CliOverrides;
use ferrite_config::ResolvedConfig;
use ferrite_config::UserConfig;
use ferrite_core::Result;
use ferrite_plugin::Apply;
use ferrite_plugin::Plugin;
use ferrite_plugin::PluginContainer;
use ferrite_server::DevServer;
use std::path::PathBuf;
use std::sync::Arc;

/// Programmatic configuration (spec §10 Rust config).
///
/// ```rust
/// use ferrite::prelude::*;
/// let config = ferrite::Config::default().alias("@", "./src");
/// ```
#[derive(Default)]
pub struct Config {
    /// User configuration (merged over file config at resolve time).
    pub user: UserConfig,
    /// Plugins.
    pub plugins: Vec<Arc<dyn Plugin>>,
    /// Project root hint.
    pub root: Option<PathBuf>,
    /// CLI overrides (highest precedence).
    pub overrides: CliOverrides,
}

impl Config {
    /// Empty config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a plugin.
    #[must_use]
    pub fn plugin(mut self, plugin: impl Plugin + 'static) -> Self {
        self.plugins.push(Arc::new(plugin));
        self
    }

    /// Add an import alias.
    #[must_use]
    pub fn alias(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.user.resolve.alias.insert(from.into(), to.into());
        self
    }

    /// Add a build entry (HTML file or module, §77).
    #[must_use]
    pub fn entry(mut self, entry: impl Into<String>) -> Self {
        let entry = entry.into();
        if !self.user.build.entries.contains(&entry) {
            self.user.build.entries.push(entry);
        }
        self
    }

    /// Resolve into concrete config + plugins (runs `config` hooks).
    pub async fn resolve(self) -> Result<(ResolvedConfig, Vec<Arc<dyn Plugin>>)> {
        let mut user = self.user;
        let container = PluginContainer::new(self.plugins.clone(), Apply::All);
        container.hook_config(&mut user).await?;
        // Merge file config under programmatic config.
        let root_hint = self.root.clone().unwrap_or_else(|| PathBuf::from("."));
        let file_config = load_user_config(&root_hint).unwrap_or_default();
        let merged = merge_user_config(file_config, user);
        let resolved = resolve_config(merged, Some(root_hint), self.overrides)?;
        Ok((resolved, self.plugins))
    }
}

/// Create a dev server (spec §9, §106).
pub async fn create_server(config: Config) -> Result<DevServer> {
    let (resolved, plugins) = config.resolve().await?;
    DevServer::new(resolved, plugins).await
}

/// Create a builder (spec §9).
pub async fn create_builder(config: Config) -> Result<Builder> {
    let mut config = config;
    // Builds default to production mode unless explicitly set.
    if config.overrides.mode.is_none() && config.user.mode.is_none() {
        config.overrides.mode = Some("production".to_string());
    }
    let (resolved, plugins) = config.resolve().await?;
    Ok(Builder::new(resolved, plugins))
}

/// One-shot production build (spec §9).
pub async fn build(config: Config) -> Result<Vec<BuildReport>> {
    create_builder(config).await?.build_app().await
}

/// Preview a production build (spec §9), running `configResolved` and
/// the preview-server hooks.
pub async fn preview(config: Config) -> Result<()> {
    let (resolved, plugins) = config.resolve().await?;
    preview_with_plugins(&resolved, &plugins).await
}
