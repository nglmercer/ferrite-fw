//! Dev server shared state and constructors.

use crate::env::*;
use crate::util::lan_ip;
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
use notify::Watcher as _;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::net::SocketAddr;
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

    fn module_graph(&self) -> Option<&ModuleGraph> {
        Some(&self.inner.graph)
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.inner.bound_addr.lock().ok().and_then(|addr| *addr)
    }

    fn server_urls(&self) -> ferrite_plugin::ServerUrls {
        let (host, port) = match ServerControl::local_addr(self) {
            Some(addr) => (addr.ip().to_string(), addr.port()),
            None => (
                self.inner.config.server.host.clone(),
                self.inner.config.server.port,
            ),
        };
        let display = if host == "0.0.0.0" || host == "::" {
            "127.0.0.1".to_string()
        } else {
            host
        };
        let local = format!("http://{display}:{port}/");
        let network = lan_ip()
            .map(|ip| format!("http://{ip}:{port}/"))
            .filter(|url| *url != local);
        ferrite_plugin::ServerUrls { local, network }
    }

    fn hmr_clients(&self) -> usize {
        self.inner.hmr.receivers()
    }

    fn send_full_reload(&self, path: Option<&str>) {
        self.inner.hmr.send_full_reload(path.map(str::to_string));
    }

    fn watcher_alive(&self) -> bool {
        self.watcher
            .lock()
            .map(|watcher| watcher.is_some())
            .unwrap_or(false)
    }

    fn watcher_add(&self, path: &Path) {
        self.watch_extra(path);
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
    /// Bound socket address (set by `listen`, `httpServer` equivalent).
    pub bound_addr: Mutex<Option<SocketAddr>>,
    /// Shared HTTP client (remote imports, proxy forwarding).
    pub http_client: reqwest::Client,
}

impl DevServer {
    /// Create a dev server (runs `config_resolved`, starts the watcher).
    pub async fn new(config: ResolvedConfig, plugins: Vec<Arc<dyn Plugin>>) -> Result<Self> {
        Self::new_inner(config, plugins, true, None).await
    }

    /// Create a server without the file watcher (builds, one-shot transforms).
    pub async fn new_without_watcher(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
    ) -> Result<Self> {
        Self::new_inner(config, plugins, false, None).await
    }

    /// One-shot shared pipeline with an explicit lowering compiler.
    pub async fn new_without_watcher_with_compiler(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
        compiler: Arc<dyn JsCompiler>,
    ) -> Result<Self> {
        Self::new_inner(config, plugins, false, Some(compiler)).await
    }

    /// Inner constructor.
    async fn new_inner(
        config: ResolvedConfig,
        plugins: Vec<Arc<dyn Plugin>>,
        watch: bool,
        compiler_override: Option<Arc<dyn JsCompiler>>,
    ) -> Result<Self> {
        let mode = if config.is_production {
            Apply::Build
        } else {
            Apply::Serve
        };
        let container = PluginContainer::new(plugins, mode);
        container.hook_config_resolved(&config).await?;
        let compiler = match compiler_override {
            Some(compiler) => compiler,
            None => compiler_for_engine(&config.compiler.engine)?,
        };
        let mut client_resolver = Resolver::for_environment(
            config.root.clone(),
            &config.resolve,
            &EnvironmentKind::Client,
        );
        let mut ssr_resolver =
            Resolver::for_environment(config.root.clone(), &config.resolve, &EnvironmentKind::Ssr);
        let env_vars = load_env_files(&config.root, &config.mode, &config.env.prefix);
        let mode_name = config.mode.clone();
        client_resolver.lockfile = config.lockfile();
        ssr_resolver.lockfile = config.lockfile();
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
            bound_addr: Mutex::new(None),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
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
        Self::new_inner(config, plugins, true, Some(compiler)).await
    }

    /// Set the SSR adapter.
    pub async fn set_ssr_adapter(&self, adapter: Arc<dyn SsrAdapter>) {
        *self.inner.ssr_adapter.write().await = Some(adapter);
    }

    /// Watch an extra path at runtime (`server.watcher.add`).
    ///
    /// The path is recorded in `watch_files` and, when the watcher is
    /// running, added to the notify watch so `watchChange` fires for it.
    pub fn watch_extra(&self, path: &Path) {
        let text = path.to_string_lossy().into_owned();
        if let Ok(mut watch) = self.inner.watch_files.lock() {
            if !watch.iter().any(|entry| entry == &text) {
                watch.push(text);
            }
        }
        if let Ok(mut slot) = self.watcher.lock() {
            if let Some(watcher) = slot.as_mut() {
                if let Err(error) = watcher.watch(path, notify::RecursiveMode::Recursive) {
                    tracing::warn!("cannot watch extra path `{}`: {error}", path.display());
                }
            }
        }
    }

    /// Access shared state (router handlers, middleware mode).
    #[must_use]
    pub fn inner(&self) -> &Arc<DevServerInner> {
        &self.inner
    }
}

#[cfg(test)]
mod compiler_embedding_tests {
    use super::*;
    #[tokio::test]
    async fn explicit_compiler_is_installed_before_watcher_and_transforms() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("main.ts"),
            "export const answer: number = 42;",
        )
        .unwrap();
        let mut config = ferrite_config::resolve_config(
            Default::default(),
            Some(root.path().to_path_buf()),
            Default::default(),
        )
        .unwrap();
        config.compiler.engine = "custom-embedding".into();
        let compiler: Arc<dyn JsCompiler> =
            Arc::new(ferrite_transform::OxcCompiler::new(Default::default()));
        let server = DevServer::with_compiler(config, vec![], compiler.clone())
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&server.inner().compiler, &compiler));
        let module = server
            .pipeline_module(&ferrite_core::ModuleId::new("/main.ts"), None, "client")
            .await
            .unwrap();
        assert!(module.code.contains("42"));
        assert!(!module.code.contains(": number"));
        server.close();
    }
}
