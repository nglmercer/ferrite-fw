//! Dev server shared state and constructors.

use crate::env::*;
use ferrite_cache::MemoryCache;
use ferrite_config::ResolvedConfig;
use ferrite_core::EnvironmentKind;
use ferrite_core::Result;
use ferrite_graph::ModuleGraph;
use ferrite_hmr::HmrServer;
use ferrite_plugin::Apply;
use ferrite_plugin::Plugin;
use ferrite_plugin::PluginContainer;
use ferrite_plugin::ServerControl;
use ferrite_resolver::Resolver;
use ferrite_ssr::RpcRegistry;
use ferrite_ssr::SsrAdapter;
use ferrite_transform::compiler_for_engine;
use ferrite_transform::JsCompiler;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

/// Shared server state (cloneable handle).
#[derive(Clone)]
pub struct DevServer {
    /// Inner state.
    pub(crate) inner: Arc<DevServerInner>,
    /// File watcher (kept alive; `None` after [`DevServer::close`]).
    pub(crate) watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

/// Server control surface for plugins (§11 `configure_server`).
impl ServerControl for DevServer {
    fn resolved_config(&self) -> &ResolvedConfig {
        &self.inner.config
    }

    fn root(&self) -> &Path {
        &self.inner.config.root
    }
}

/// Inner shared state.
pub struct DevServerInner {
    /// Resolved config.
    pub config: ResolvedConfig,
    /// Module graph.
    pub graph: ModuleGraph,
    /// Plugin container (serve mode).
    pub plugins: PluginContainer,
    /// Client resolver.
    pub client_resolver: Resolver,
    /// SSR resolver (SSR conditions).
    pub ssr_resolver: Resolver,
    /// JS compiler.
    pub compiler: Arc<dyn JsCompiler>,
    /// HMR hub.
    pub hmr: HmrServer,
    /// Emitted files shared with plugins.
    pub emitted: Mutex<HashMap<String, ferrite_plugin::EmittedFile>>,
    /// Bare-specifier → dev-URL map (`import-map` dev strategy).
    pub import_map: Mutex<BTreeMap<String, String>>,
    /// Extra watch files.
    pub watch_files: Mutex<Vec<String>>,
    /// Collected warnings.
    pub warnings: Mutex<Vec<String>>,
    /// Transform cache.
    pub cache: MemoryCache,
    /// Filtered client env vars (`FERRITE_*`, `PUBLIC_*`, ...).
    pub env_vars: HashMap<String, String>,
    /// Build mode (`development`/`production`).
    pub mode: String,
    /// Optional SSR adapter.
    pub ssr_adapter: tokio::sync::RwLock<Option<Arc<dyn SsrAdapter>>>,
    /// Server-function registry.
    pub rpc: tokio::sync::RwLock<RpcRegistry>,
    /// Debounce map for watcher events.
    pub(crate) debounce: Mutex<HashMap<PathBuf, Instant>>,
}

impl DevServer {
    /// Create a dev server (runs `config_resolved`, starts the watcher).
    pub async fn new(config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Result<Self> {
        Self::new_inner(config, plugins, true).await
    }

    /// Create a server without the file watcher (builds, one-shot transforms).
    pub async fn new_without_watcher(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
    ) -> Result<Self> {
        Self::new_inner(config, plugins, false).await
    }

    /// Inner constructor.
    async fn new_inner(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
        watch: bool,
    ) -> Result<Self> {
        let mode = if config.is_production {
            Apply::Build
        } else {
            Apply::Serve
        };
        let container = PluginContainer::new(plugins, mode);
        container.hook_config_resolved(&config).await?;
        let compiler = compiler_for_engine(&config.compiler.engine)?;
        let client_resolver = Resolver::for_environment(
            config.root.clone(),
            &config.resolve,
            &EnvironmentKind::Client,
        );
        let ssr_resolver =
            Resolver::for_environment(config.root.clone(), &config.resolve, &EnvironmentKind::Ssr);
        let env_vars = load_env_files(&config.root, &config.mode, &config.env.prefix);
        let mode_name = config.mode.clone();
        let inner = Arc::new(DevServerInner {
            config,
            graph: ModuleGraph::new(),
            plugins: container,
            client_resolver,
            ssr_resolver,
            compiler,
            hmr: HmrServer::default(),
            emitted: Mutex::new(HashMap::new()),
            import_map: Mutex::new(BTreeMap::new()),
            watch_files: Mutex::new(Vec::new()),
            warnings: Mutex::new(Vec::new()),
            cache: MemoryCache::new(),
            env_vars,
            mode: mode_name,
            ssr_adapter: tokio::sync::RwLock::new(None),
            rpc: tokio::sync::RwLock::new(RpcRegistry::new()),
            debounce: Mutex::new(HashMap::new()),
        });
        let server = Self {
            inner,
            watcher: Arc::new(Mutex::new(None)),
        };
        if watch {
            server.start_watcher()?;
        }
        // Unknown dev strategies fall back to `rewrite` with a warning.
        if !["rewrite", "import-map"].contains(&server.inner.config.npm.dev_strategy.as_str()) {
            tracing::warn!(
                "unknown `[npm] dev_strategy = \"{}\"`; using `rewrite`",
                server.inner.config.npm.dev_strategy
            );
        }
        // `configure_server` hooks.
        let mut control = server.clone();
        server
            .inner
            .plugins
            .hook_configure_server(&mut control as &mut dyn ServerControl)
            .await?;
        Ok(server)
    }

    /// Create a server with an explicit compiler (tests, embedding).
    pub async fn with_compiler(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
        compiler: Arc<dyn JsCompiler>,
    ) -> Result<Self> {
        let mut server = Self::new(config, plugins).await?;
        // Replace the compiler selected from config.
        let inner = Arc::get_mut(&mut server.inner).expect("fresh server");
        inner.compiler = compiler;
        Ok(server)
    }

    /// Set the SSR adapter.
    pub async fn set_ssr_adapter(&self, adapter: Arc<dyn SsrAdapter>) {
        *self.inner.ssr_adapter.write().await = Some(adapter);
    }

    /// Access shared state (router handlers, middleware mode).
    #[must_use]
    pub fn inner(&self) -> &Arc<DevServerInner> {
        &self.inner
    }
}
