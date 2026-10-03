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
    /// Explicit config file/directory; replaces automatic file discovery.
    pub config_path: Option<PathBuf>,
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
        let root_hint = self.root.clone().unwrap_or_else(|| {
            self.config_path.as_ref().map_or_else(
                || PathBuf::from("."),
                |path| {
                    if path.is_dir() {
                        path.clone()
                    } else {
                        path.parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .to_path_buf()
                    }
                },
            )
        });
        let file_config = match self.config_path {
            Some(path) => ferrite_config::load_user_config_path(&path)?,
            None => load_user_config(&root_hint)?,
        };
        let merged = merge_user_config(file_config, user);
        let resolved = resolve_config(merged, Some(root_hint), self.overrides)?;
        let mut plugins = self.plugins;
        for profile in resolved.foreign_plugins.iter().flatten() {
            let node = profile.node.as_ref().map(|path| resolved.root.join(path));
            let host = Arc::new(
                ferrite_plugin::node_adapter::NodeAdapterHost::spawn_with_timeout(
                    node,
                    std::time::Duration::from_millis(profile.timeout_ms.unwrap_or(10_000)),
                ).map_err(|error| ferrite_core::FerriteError::Config(format!("foreign plugin {} explicit Node host failed: {error}; check foreign_plugins.node and timeout_ms", profile.name)))?,
            );
            let entry = resolved.root.join(&profile.entry);
            plugins.push(Arc::new(ferrite_plugin::ForeignHookPlugin::register(
                host,
                &profile.name,
                &entry,
                profile.options.clone(),
            ).map_err(|error| ferrite_core::FerriteError::Config(format!("foreign plugin {} at {} failed registration: {error}; check entry/options and the supported hook subset", profile.name, entry.display())))?));
        }
        if let Some(profile) = &resolved.framework {
            if !profile.enabled.is_empty() {
                // Only an explicit validated host selection can reach this constructor.
                for name in &profile.enabled {
                    let official = format!("ferrite:{name}-official");
                    if plugins.iter().any(|plugin| plugin.name() == official) {
                        return Err(ferrite_core::FerriteError::Config(format!(
                            "framework `{name}` is configured both through framework.enabled and an explicit official plugin; select one"
                        )));
                    }
                }
                let node = profile.node.as_ref().map(|path| {
                    if path.is_absolute() {
                        path.clone()
                    } else {
                        resolved.root.join(path)
                    }
                });
                let host = Arc::new(
                    crate::frameworks::compiler_host::NodeCompilerHost::new(
                        resolved.root.clone(),
                        resolved.lockfile(),
                        node,
                        std::time::Duration::from_millis(profile.timeout_ms),
                    )
                    .await?,
                );
                for name in &profile.enabled {
                    let legacy = format!("ferrite:{name}");
                    plugins.retain(|plugin| plugin.name() != legacy);
                    let framework = match name.as_str() {
                        "vue" => crate::frameworks::compiler_host::Framework::Vue,
                        "svelte" => crate::frameworks::compiler_host::Framework::Svelte,
                        _ => unreachable!("validated framework profile"),
                    };
                    plugins.push(Arc::new(crate::frameworks::HostedFrameworkPlugin::new(
                        framework,
                        host.clone(),
                        !resolved.is_production,
                    )));
                }
            }
        }
        Ok((resolved, plugins))
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

#[cfg(test)]
mod foreign_profile_tests {
    use super::*;

    #[tokio::test]
    async fn foreign_profiles_require_opt_in_and_do_not_fallback_from_missing_node() {
        let root = tempfile::tempdir().unwrap();
        for (host, node, expected) in [
            (None, None, "requires explicit host"),
            (
                Some("node".to_string()),
                Some(root.path().join("missing-node")),
                "cannot spawn",
            ),
        ] {
            let user = ferrite_config::UserConfig {
                foreign_plugins: Some(vec![ferrite_config::ForeignPluginConfig {
                    name: "fixture".into(),
                    entry: "missing.mjs".into(),
                    host,
                    node,
                    ..Default::default()
                }]),
                ..Default::default()
            };
            let config = Config {
                root: Some(root.path().into()),
                user,
                ..Default::default()
            };
            let error = match config.resolve().await {
                Err(error) => error,
                Ok(_) => panic!("unavailable profile must fail"),
            };
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[tokio::test]
    #[ignore = "requires explicitly enabled real Node hook profile"]
    async fn configured_foreign_hooks_are_selected_by_shared_config_resolution() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("plugin.mjs"), r#"export default options => ({transform(code, id, context) {
            if (!id.endsWith('/entry.js')) return null;
            if (context.ssr !== false) throw new Error('lost client options');
            return {code: code + '\nexport const configured = ' + JSON.stringify(options.value) + ';'};
        }});"#).unwrap();
        std::fs::write(root.path().join("entry.js"), "export const original = 1;").unwrap();
        std::fs::write(root.path().join("selected.toml"), "[[foreign_plugins]]\nname = 'configured'\nentry = 'plugin.mjs'\nhost = 'node'\n[foreign_plugins.options]\nvalue = 'configured-through-file'\n").unwrap();
        for production in [false, true] {
            let config = Config {
                root: Some(root.path().into()),
                config_path: Some(root.path().join("selected.toml")),
                overrides: CliOverrides {
                    mode: Some(
                        if production {
                            "production"
                        } else {
                            "development"
                        }
                        .into(),
                    ),
                    ..Default::default()
                },
                ..Default::default()
            };
            let (resolved, plugins) = config.resolve().await.unwrap();
            assert_eq!(resolved.is_production, production);
            assert_eq!(resolved.runtime.backend, "auto");
            let server = DevServer::new_without_watcher(resolved, plugins)
                .await
                .unwrap();
            let output = server
                .pipeline_module(&ferrite_core::ModuleId::new("/entry.js"), None, "client")
                .await
                .unwrap();
            assert!(output.code.contains("configured-through-file"));
            server.close();
        }
    }
}
