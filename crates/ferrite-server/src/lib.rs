//! Dev server (spec §24–§26, §76, §79).
//!
//! Native-ESM dev mode: resolve → load → transform → rewrite imports →
//! serve, with file watching, HMR broadcast, and middleware embedding.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use ferrite_assets::{asset_to_js, raw_to_js, to_data_url};
use ferrite_cache::MemoryCache;
use ferrite_config::ResolvedConfig;
use ferrite_core::{EnvironmentKind, FerriteError, Hash, ModuleId, ModuleType, Result};
use ferrite_graph::{ImportEdge, ImportKind, ModuleGraph, ModuleNode};
use ferrite_hmr::{client_source, plan_update, HmrPlan, HmrServer, HMR_ENDPOINT};
use ferrite_plugin::{
    Apply, HotUpdateEvent, HtmlTransformContext, LoadRequest, Plugin, PluginContainer,
    PluginContext, ResolveHookRequest, ServerControl, TransformRequest as HookTransformRequest,
};
use ferrite_resolver::{ResolveKind, ResolveRequest, ResolvedId, Resolver};
use ferrite_ssr::{
    RpcEncoding, RpcRegistry, SsrAdapter, SsrContext, SsrHttpRequest, SsrModule, RPC_ROUTE_PREFIX,
};
use ferrite_transform::{
    compiler_for_engine, rewrite_import_meta_hot, rewrite_specifiers, with_hmr_client, JsCompiler,
    OxcCompiler, OxcOptions, TransformRequest,
};
use notify::{Event, EventKind, RecursiveMode, Watcher as _};

/// Shared server state (cloneable handle).
#[derive(Clone)]
pub struct DevServer {
    /// Inner state.
    inner: Arc<DevServerInner>,
    /// File watcher (kept alive; `None` after [`DevServer::close`]).
    watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
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
    debounce: Mutex<HashMap<PathBuf, Instant>>,
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

    /// Build the axum router (middleware mode, §76).
    pub fn router(&self) -> axum::Router {
        axum::Router::new()
            .route(HMR_ENDPOINT, get(ws_handler))
            .route("/@ferrite/client", get(client_handler))
            .route("/@ferrite/inspect", get(inspect_handler))
            .route(&format!("{RPC_ROUTE_PREFIX}{{*hash}}"), post(rpc_handler))
            .fallback(fallback_handler)
            .with_state(self.inner.clone())
    }

    /// Listen until Ctrl-C (or error). Honors `strict_port`.
    pub async fn listen(&self) -> Result<SocketAddr> {
        let mut port = self.inner.config.server.port;
        let host = self.inner.config.server.host.clone();
        let strict = self.inner.config.server.strict_port;
        let mut attempts = 0;
        let listener = loop {
            match tokio::net::TcpListener::bind(format!("{host}:{port}")).await {
                Ok(listener) => break listener,
                Err(error) if !strict && attempts < 10 => {
                    tracing::warn!("port {port} busy ({error}); trying {}", port + 1);
                    port += 1;
                    attempts += 1;
                }
                Err(error) => {
                    return Err(FerriteError::Other(format!(
                        "cannot bind {host}:{port}: {error}"
                    )));
                }
            }
        };
        let addr = listener
            .local_addr()
            .map_err(|error| FerriteError::Other(format!("cannot read bound address: {error}")))?;
        tracing::info!("ferrite dev server on http://{addr}");
        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|error| FerriteError::Other(format!("server error: {error}")))?;
        Ok(addr)
    }

    /// Stop the watcher.
    pub fn close(&self) {
        if let Ok(mut watcher) = self.watcher.lock() {
            *watcher = None;
        }
    }

    /// Transform a dev request URL (`/src/main.ts`) into a response.
    pub async fn transform_request(&self, url: &str) -> Result<PipelineResponse> {
        let (path, _) = url
            .split_once('?')
            .map_or((url, None), |(p, q)| (p, Some(q)));
        // HMR client.
        if path == ferrite_hmr::CLIENT_ID {
            return Ok(PipelineResponse::code(
                client_source().to_string(),
                "text/javascript",
            ));
        }
        // HTML entry.
        if path == "/" || (path.ends_with(".html") && self.is_project_file(path)) {
            let html_path = if path == { "/" } { "/index.html" } else { path };
            let html = self.transform_index_html(html_path).await?;
            return Ok(PipelineResponse::code(html, "text/html"));
        }
        // Public dir (served verbatim, §25).
        let public_file = self
            .inner
            .config
            .root
            .join("public")
            .join(path.trim_start_matches('/'));
        if public_file.is_file() {
            let bytes = std::fs::read(&public_file)?;
            let content_type = ferrite_assets::content_type(path);
            return Ok(PipelineResponse::bytes(bytes, content_type));
        }
        // Module pipeline.
        let module = self
            .pipeline_module(&ModuleId::new(url), None, "client")
            .await?;
        if module.is_raw_bytes {
            let bytes = std::fs::read(self.id_to_file(&module.id)?)?;
            let content_type = ferrite_assets::content_type(&module.id.0);
            return Ok(PipelineResponse::bytes(bytes, content_type));
        }
        let content_type = if module.module_type == ModuleType::Css && url.contains("direct") {
            "text/css"
        } else {
            "text/javascript"
        };
        Ok(PipelineResponse::code(module.code, content_type))
    }

    /// Transform `index.html` (core rewrites + plugin hooks, §31).
    pub async fn transform_index_html(&self, path: &str) -> Result<String> {
        let file = self.inner.config.root.join(path.trim_start_matches('/'));
        let html = std::fs::read_to_string(&file)
            .map_err(|_| FerriteError::Resolve(format!("cannot resolve `{path}`")))?;
        let environment = self.inner.config.client_env();
        let ctx = self.plugin_context(&environment);
        // Core rewrites run between pre/normal/post conceptually; the
        // container already orders hooks, so: core → hooks → inject.
        let rewritten = ferrite_html::apply_core_rewrites(&html, &self.inner.config.base, true);
        // Rewrite module script sources to resolved URLs.
        let rewritten = self.rewrite_html_scripts(&rewritten).await;
        let result = self
            .inner
            .plugins
            .hook_transform_index_html(
                &ctx,
                HtmlTransformContext {
                    html: rewritten,
                    path: path.to_string(),
                    ssr: false,
                },
            )
            .await?;
        let html = result.html.unwrap_or(html);
        let html = ferrite_html::inject_tags(&html, &result.tags);
        if self.inner.config.npm.dev_strategy == "import-map" {
            self.collect_import_map(&html).await;
            let map = self
                .inner
                .import_map
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            if !map.is_empty() {
                let script = ferrite_html::import_map_script(&map);
                return Ok(ferrite_html::inject_import_map(&html, &script));
            }
        }
        Ok(html)
    }

    /// Walk the client module closure reachable from `html` entries so the
    /// `import-map` store covers every statically reachable bare import.
    ///
    /// Only statically analyzable imports (static + dynamic-with-literal)
    /// are collected; runtime-computed specifiers keep the rewrite
    /// strategy's behavior of failing at fetch time.
    async fn collect_import_map(&self, html: &str) {
        let environment = self.inner.config.client_env();
        let ctx = self.plugin_context(&environment);
        let mut seen = std::collections::HashSet::new();
        let mut stack = Vec::new();
        for entry in ferrite_html::discover_entries(html) {
            if !entry.is_module {
                continue;
            }
            let spec = entry.src.as_str();
            if spec.starts_with("/@")
                || spec.starts_with("http")
                || spec.starts_with("data:")
                || spec.starts_with("blob:")
            {
                continue;
            }
            match self.resolve_id(&ctx, spec, None, &environment).await {
                Ok(resolved) if !resolved.external => stack.push(resolved.id),
                Ok(_) => {}
                Err(error) => tracing::warn!("cannot resolve entry `{spec}`: {error}"),
            }
        }
        while let Some(id) = stack.pop() {
            if !seen.insert(id.0.clone()) {
                continue;
            }
            match self.pipeline_module(&id, None, "client").await {
                Ok(module) => {
                    for (_, dependency, _) in &module.imports {
                        stack.push(dependency.clone());
                    }
                }
                Err(error) => tracing::warn!("cannot load `{}` for import map: {error}", id.0),
            }
        }
    }

    /// Load an SSR module graph (`ssrLoadModule`, §22).
    pub async fn ssr_load_module(&self, url: &str) -> Result<SsrModule> {
        let root = self
            .pipeline_module(&ModuleId::new(url), None, "ssr")
            .await?;
        let mut dependencies = Vec::new();
        let mut queue: std::collections::VecDeque<(ModuleId, ModuleId)> = root
            .imports
            .iter()
            .map(|(_, id, _)| (id.clone(), root.id.clone()))
            .collect();
        let mut seen = std::collections::HashSet::from([root.id.clone()]);
        while let Some((id, importer)) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if id.is_virtual() {
                continue;
            }
            match self.pipeline_module(&id, Some(&importer), "ssr").await {
                Ok(module) => {
                    dependencies.push(id.0.clone());
                    for (_, dep, _) in &module.imports {
                        queue.push_back((dep.clone(), id.clone()));
                    }
                }
                Err(error) => {
                    tracing::warn!("ssr: skipping `{id}`: {error}");
                }
            }
        }
        Ok(SsrModule {
            id: root.id.0,
            code: root.code,
            dependencies,
        })
    }

    /// Invalidate a module and broadcast its HMR plan.
    pub async fn invalidate_module(&self, id: &ModuleId) {
        self.inner.graph.invalidate_tree(id);
        let timestamp = now_millis();
        match plan_update(&self.inner.graph, id, timestamp) {
            HmrPlan::Update(updates) => self.inner.hmr.send_update(updates),
            HmrPlan::FullReload => self.inner.hmr.send_full_reload(Some(id.0.clone())),
        }
    }

    /// Resolve an entry specifier to a module id (build entry discovery).
    pub async fn resolve_entry(&self, specifier: &str, env: &str) -> Result<ModuleId> {
        let environment = if env == "ssr" {
            self.inner.config.ssr_env()
        } else {
            self.inner.config.client_env()
        };
        let ctx = self.plugin_context(&environment);
        Ok(self
            .resolve_id(&ctx, specifier, None, &environment)
            .await?
            .id)
    }

    /// Force-reload a module (invalidate + full reload).
    pub async fn reload_module(&self, id: &ModuleId) {
        self.inner.graph.invalidate_tree(id);
        self.inner.hmr.send_full_reload(Some(id.0.clone()));
    }

    /// Restart: invalidate everything, clear caches, full reload.
    pub async fn restart(&self) {
        for id in self.inner.graph.module_ids() {
            self.inner.graph.invalidate(&id);
        }
        self.inner.cache.clear();
        self.inner.hmr.send_full_reload(None);
    }

    /// Full module pipeline shared by HTTP + bundler loader.
    pub async fn pipeline_module(
        &self,
        id: &ModuleId,
        importer: Option<&ModuleId>,
        env: &str,
    ) -> Result<PipelineModule> {
        let ssr = env == "ssr";
        let environment = if ssr {
            self.inner.config.ssr_env()
        } else {
            self.inner.config.client_env()
        };
        let ctx = self.plugin_context(&environment);
        // 0. `/@id/` URLs map back to internal `\0` virtual ids (§14).
        let id_owned;
        let id = match url_to_virtual(&id.0) {
            Some(virtual_id) => {
                id_owned = virtual_id;
                &id_owned
            }
            None => id,
        };
        // 1. Resolve (plugin first, then resolver).
        let resolved = self.resolve_id(&ctx, &id.0, importer, &environment).await?;
        if resolved.external {
            return Err(FerriteError::Resolve(format!(
                "cannot load external module `{}` in dev (mark it `noExternal` or add a shim)",
                resolved.id.0
            )));
        }
        let resolved_id = resolved.id.clone();
        let (path_part, query) = resolved_id.split_query();
        // 2. Asset-shim requests (`?asset-shim` appended by import rewriting).
        if query.is_some_and(|q| q.starts_with("asset-shim")) {
            let plain = path_part.to_string();
            return Ok(PipelineModule::code_only(
                resolved_id.clone(),
                asset_to_js(&plain),
                ModuleType::Js,
            ));
        }
        // 3. `?direct` CSS (from `<link>` tags).
        let direct_css = query.is_some_and(|q| q.contains("direct")) && path_part.ends_with(".css");
        // 4. Load (plugin first, then built-ins / fs).
        let (source, mut module_type) = self.load_source(&ctx, &resolved_id, &environment).await?;
        if direct_css {
            module_type = ModuleType::Css;
        }
        // 5. Raw bytes (images, fonts, wasm, ...): served verbatim.
        if is_raw_asset(path_part, &module_type, query) && !direct_css {
            return Ok(PipelineModule::raw(resolved_id, module_type));
        }
        // 6. `?raw` / `?url` / `?inline` / `?worker` / `?wasm`.
        if let Some(q) = query {
            if let Some(shim) = self
                .asset_query_shim(&resolved_id, path_part, q, &source)
                .await?
            {
                return Ok(shim);
            }
        }
        // 7. Cache lookup.
        let cache_key = self.cache_key(&resolved_id, &source, env);
        if let Some(cached) = self.inner.cache.get(&cache_key.0) {
            if let Ok(cached) = serde_json::from_slice::<CachedTransform>(&cached) {
                let module = PipelineModule::from_cached(resolved_id.clone(), cached);
                self.update_graph(&module);
                return Ok(module);
            }
        }
        // 8. Core transform.
        let mut module = self
            .core_transform(&ctx, &resolved_id, &source, &module_type, &environment)
            .await?;
        // 9. Plugin transform chain (JS-like only).
        if module.module_type.is_js_like() {
            let hooked = self
                .inner
                .plugins
                .hook_transform(
                    &ctx,
                    HookTransformRequest {
                        id: resolved_id.0.clone(),
                        code: module.code.clone(),
                        module_type: module.module_type.clone(),
                        environment: environment.kind.clone(),
                        ssr,
                    },
                )
                .await?;
            module.code = hooked.code;
        }
        // 10. CJS conversion (dev ESM interop, §18).
        if needs_cjs_conversion(&resolved_id, &module.code, module.has_module_syntax) {
            module = self.convert_cjs(&ctx, module, &environment).await?;
        }
        // 11. Import rewriting (dev URLs, §26/§28).
        if module.module_type.is_js_like() {
            module = self
                .rewrite_module_imports(&ctx, module, &environment)
                .await?;
        }
        // 12. HMR injection (dev client only; never in production builds).
        if !ssr && !self.inner.config.is_production && module.module_type.is_js_like() {
            if module.uses_import_meta_hot {
                module.code = rewrite_import_meta_hot(&module.code, &resolved_id.0);
            }
            module.code = with_hmr_client(&module.code);
        }
        // 13. Graph update + hooks + cache.
        self.update_graph(&module);
        let parsed_id = module.id.clone();
        self.inner
            .plugins
            .hook_module_parsed(&ctx, ferrite_plugin::ModuleParsed { id: parsed_id })
            .await?;
        self.inner.cache.insert(
            &cache_key.0,
            serde_json::to_vec(&CachedTransform::from_module(&module)).unwrap_or_default(),
        );
        Ok(module)
    }

    /// Resolve an id through plugins, then the resolver.
    async fn resolve_id(
        &self,
        ctx: &PluginContext<'_>,
        specifier: &str,
        importer: Option<&ModuleId>,
        environment: &ferrite_core::Environment,
    ) -> Result<ResolvedId> {
        // Plugin `resolveId` first.
        if let Some(resolved) = self
            .inner
            .plugins
            .hook_resolve_id(
                ctx,
                ResolveHookRequest {
                    specifier,
                    importer,
                    environment: environment.kind.clone(),
                    ssr: environment.kind.is_ssr(),
                },
            )
            .await?
        {
            return Ok(resolved);
        }
        // `node:` shims for the browser (§19).
        if environment.kind == EnvironmentKind::Client {
            if let Some(name) = specifier.strip_prefix("node:") {
                if self.inner.config.node_compat.enabled {
                    return Ok(ResolvedId::new(format!("\0node:{name}")));
                }
                return Err(FerriteError::Resolve(format!(
                    "package requires `node:{name}`; Ferrite SSR runtime does not provide it \
                     (enable `[node_compat]`, mark it external with a Node adapter, or replace it)"
                )));
            }
        }
        // Remote imports (§72): absolute URLs plus relative imports from a
        // remote module (rebased onto the remote base URL).
        if specifier.starts_with("https://") || specifier.starts_with("http://") {
            return self.resolve_remote(specifier);
        }
        if specifier.starts_with("./") || specifier.starts_with("../") {
            if let Some(base) = importer.and_then(|importer| importer.0.strip_prefix("\0remote:")) {
                let joined = url::Url::parse(base)
                    .and_then(|base| base.join(specifier))
                    .map_err(|error| {
                        FerriteError::Resolve(format!("bad remote import `{specifier}`: {error}"))
                    })?;
                return self.resolve_remote(joined.as_str());
            }
        }
        let resolver = if environment.kind.is_ssr() {
            &self.inner.ssr_resolver
        } else {
            &self.inner.client_resolver
        };
        resolver.resolve(&ResolveRequest {
            specifier,
            importer,
            environment: environment.kind.clone(),
            kind: ResolveKind::Import,
        })
    }

    /// Resolve a remote URL against `[remote]` (§72).
    fn resolve_remote(&self, specifier: &str) -> Result<ResolvedId> {
        let parsed = url::Url::parse(specifier).map_err(|error| {
            FerriteError::Resolve(format!("bad remote URL `{specifier}`: {error}"))
        })?;
        let host = parsed.host_str().unwrap_or("").to_string();
        let loopback = host == "localhost" || host == "127.0.0.1" || host == "::1";
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err(FerriteError::Resolve(format!(
                "remote imports must use https (`{specifier}`); plain http is only allowed for loopback"
            )));
        }
        if !self.inner.config.remote.enabled {
            return Err(FerriteError::Resolve(format!(
                "remote import `{specifier}` is disabled (enable `[remote]`)"
            )));
        }
        if !remote_host_allowed(&host, &self.inner.config.remote.allow) {
            return Err(FerriteError::Resolve(format!(
                "remote host `{host}` is not in `[remote] allow`"
            )));
        }
        Ok(ResolvedId::new(format!("\0remote:{specifier}"))
            .with_meta(serde_json::json!({"remote": true})))
    }

    /// Fetch a remote module (disk-cached by URL hash, §72).
    async fn fetch_remote(&self, url: &str) -> Result<String> {
        let cache = ferrite_cache::DiskCache::new(self.inner.config.root.join(".ferrite"))?;
        let key = ferrite_core::Hash::of_bytes(url.as_bytes());
        if let Some(bytes) = cache.get(ferrite_cache::CacheLayer::Remote, &key) {
            return String::from_utf8(bytes).map_err(|error| {
                FerriteError::Resolve(format!("cached remote `{url}` is not UTF-8: {error}"))
            });
        }
        let response = reqwest::get(url).await.map_err(|error| {
            FerriteError::Resolve(format!("cannot fetch remote `{url}`: {error}"))
        })?;
        if !response.status().is_success() {
            return Err(FerriteError::Resolve(format!(
                "remote `{url}` responded with {}",
                response.status()
            )));
        }
        let bytes = response.bytes().await.map_err(|error| {
            FerriteError::Resolve(format!("cannot read remote `{url}`: {error}"))
        })?;
        if bytes.len() > MAX_REMOTE_BYTES {
            return Err(FerriteError::Resolve(format!(
                "remote `{url}` exceeds the {} byte cap",
                MAX_REMOTE_BYTES
            )));
        }
        cache.insert(ferrite_cache::CacheLayer::Remote, &key, &bytes)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|error| FerriteError::Resolve(format!("remote `{url}` is not UTF-8: {error}")))
    }

    /// Load source through plugins, built-in virtuals, then the fs.
    async fn load_source(
        &self,
        ctx: &PluginContext<'_>,
        id: &ModuleId,
        environment: &ferrite_core::Environment,
    ) -> Result<(String, ModuleType)> {
        // Plugin `load` first.
        if let Some(loaded) = self
            .inner
            .plugins
            .hook_load(
                ctx,
                LoadRequest {
                    id: id.0.clone(),
                    environment: environment.kind.clone(),
                },
            )
            .await?
        {
            return Ok((loaded.code, loaded.module_type));
        }
        // Built-in virtual modules.
        if let Some(virtual_source) = self.builtin_virtual(id) {
            return Ok((virtual_source, ModuleType::Js));
        }
        // `node:` shims.
        if let Some(name) = id.0.strip_prefix("\0node:") {
            return Ok((node_shim(name), ModuleType::Js));
        }
        // Remote modules (§72).
        if let Some(url) = id.0.strip_prefix("\0remote:") {
            let source = self.fetch_remote(url).await?;
            return Ok((source, ModuleType::Js));
        }
        if id.is_virtual() {
            return Err(FerriteError::Resolve(format!(
                "unknown virtual module `{id}`"
            )));
        }
        let file = self.id_to_file(id)?;
        let bytes = std::fs::read(&file)
            .map_err(|_| FerriteError::Resolve(format!("cannot load `{id}`")))?;
        let (path_part, _) = id.split_query();
        let module_type = ModuleType::from_path(path_part);
        // Binary assets: content served separately; loader returns a marker.
        if is_binary_asset(path_part, &bytes) {
            return Ok((String::new(), ModuleType::Asset));
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| FerriteError::Resolve(format!("`{id}` is not valid UTF-8")))?;
        Ok((source, module_type))
    }

    /// Built-in virtual modules (`virtual:ferrite/*`, §14).
    fn builtin_virtual(&self, id: &ModuleId) -> Option<String> {
        let name = id.0.strip_prefix("\0virtual:")?;
        match name {
            "ferrite/env" => {
                let mut lines = vec!["// generated by ferrite: import.meta.env".to_string()];
                for (key, value) in &self.inner.env_vars {
                    lines.push(format!("export const {key} = {value:?};"));
                }
                let json = serde_json::to_string(&self.inner.env_vars).unwrap_or_default();
                lines.push(format!("export default {json};"));
                Some(lines.join("\n"))
            }
            "ferrite/manifest" => Some("export default {};\n".to_string()),
            "ferrite/routes" => Some("export const routes = [];\n".to_string()),
            _ => None,
        }
    }

    /// Core transform dispatch by module type.
    async fn core_transform(
        &self,
        _ctx: &PluginContext<'_>,
        id: &ModuleId,
        source: &str,
        module_type: &ModuleType,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        match module_type {
            &ModuleType::Css => {
                let (path_part, query) = id.split_query();
                let direct = query.is_some_and(|q| q.contains("direct"));
                let css_id = path_part.to_string();
                let result = ferrite_css::transform_css(
                    &css_id,
                    source,
                    &ferrite_css::CssOptions {
                        modules: css_id.contains(".module.css"),
                        minify: false,
                        dev: true,
                    },
                );
                // Rewrite @import/url() relative to the CSS file.
                let mut mapping: HashMap<String, String> = HashMap::new();
                for import in &result.imports {
                    if let Ok(resolved) =
                        self.resolve_relative(&css_id, &import.specifier, environment)
                    {
                        mapping.insert(import.specifier.clone(), resolved);
                    }
                }
                for url in &result.urls {
                    if url.url.starts_with("./")
                        || url.url.starts_with("../")
                        || url.url.starts_with('/')
                    {
                        if let Ok(resolved) = self.resolve_relative(&css_id, &url.url, environment)
                        {
                            mapping.insert(url.url.clone(), resolved);
                        }
                    }
                }
                let code =
                    ferrite_css::rewrite_css_refs(&result.code, |spec| mapping.get(spec).cloned());
                if direct {
                    return Ok(PipelineModule {
                        id: id.clone(),
                        code,
                        imports: Vec::new(),
                        side_effects: None,
                        module_type: ModuleType::Css,
                        map: None,
                        has_module_syntax: false,
                        uses_import_meta_hot: false,
                        is_raw_bytes: false,
                        shake: Some(ferrite_transform::ShakeInfo::default()),
                    });
                }
                // Production builds emit self-contained JS (no /@ferrite/client
                // import); file extraction is the bundler roadmap (§29).
                let production = self.inner.config.is_production;
                let js = if production {
                    production_css_js(&id.0, &code, &result.exports)
                } else {
                    ferrite_css::css_to_js(&id.0, &code, &result.exports)
                };
                let mut module = PipelineModule::code_only(id.clone(), js, ModuleType::Js);
                if !production {
                    module.imports = vec![(
                        "/@ferrite/client".to_string(),
                        ModuleId::new("/@ferrite/client"),
                        ImportKind::Static,
                    )];
                }
                Ok(module)
            }
            _ if module_type.is_js_like() || *module_type == ModuleType::Json => {
                let mut define = environment.define.clone();
                define.extend(self.env_defines(environment.kind.is_ssr()));
                let result = self.inner.compiler.transform(TransformRequest {
                    id: id.0.clone(),
                    code: source.to_string(),
                    module_type: module_type.clone(),
                    environment: environment.kind.clone(),
                    ssr: environment.kind.is_ssr(),
                    target: environment.target.clone(),
                    minify: false,
                    sourcemap: self.inner.config.build.sourcemap.enabled(),
                    define,
                    jsx_runtime: self.inner.config.react.runtime.clone(),
                    development: !self.inner.config.is_production,
                })?;
                let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
                    id: id.0.clone(),
                    code: result.code.clone(),
                    module_type: ModuleType::Js,
                })?;
                Ok(PipelineModule {
                    id: id.clone(),
                    code: result.code,
                    imports: Vec::new(), // filled during rewriting
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: result.map.map(|map| map.mappings),
                    has_module_syntax: parsed.has_module_syntax,
                    uses_import_meta_hot: parsed.uses_import_meta_hot,
                    is_raw_bytes: false,
                    shake: None, // filled during rewriting
                })
            }
            _ => Ok(PipelineModule::code_only(
                id.clone(),
                source.to_string(),
                module_type.clone(),
            )),
        }
    }

    /// Rewrite a module's imports to dev URLs.
    async fn rewrite_module_imports(
        &self,
        ctx: &PluginContext<'_>,
        mut module: PipelineModule,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
            id: module.id.0.clone(),
            code: module.code.clone(),
            module_type: ModuleType::Js,
        })?;
        // `import-map` is client-only: SSR has no browser to resolve maps.
        let use_import_map =
            self.inner.config.npm.dev_strategy == "import-map" && !environment.kind.is_ssr();
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut imports = Vec::new();
        let mut import_bindings = Vec::new();
        for import in &parsed.imports {
            if import.is_type {
                continue;
            }
            match self
                .resolve_id(ctx, &import.specifier, Some(&module.id), environment)
                .await
            {
                Ok(resolved) => {
                    let mut url = resolved.id.0.clone();
                    if url.starts_with('\0') {
                        url = virtual_url(&url);
                    } else if should_shim_asset(&url) {
                        url = format!("{url}?asset-shim");
                    }
                    let kind = match import.kind {
                        ferrite_transform::ParsedImportKind::Static => ImportKind::Static,
                        ferrite_transform::ParsedImportKind::Dynamic => ImportKind::Dynamic,
                    };
                    // Bare specifiers stay for the browser; record the URL
                    // for the inline map instead of rewriting.
                    if use_import_map && !resolved.external && is_bare_specifier(&import.specifier)
                    {
                        if let Ok(mut map) = self.inner.import_map.lock() {
                            map.insert(import.specifier.clone(), url.clone());
                        }
                        imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                        import_bindings.push(import.bindings.clone());
                        continue;
                    }
                    mapping.insert(import.specifier.clone(), url.clone());
                    imports.push((import.specifier.clone(), ModuleId::new(url), kind));
                    import_bindings.push(import.bindings.clone());
                }
                Err(error) => {
                    tracing::warn!(
                        "cannot resolve `{}` from `{}`: {error}",
                        import.specifier,
                        module.id.0
                    );
                }
            }
        }
        if !mapping.is_empty() {
            let (code, _) = rewrite_specifiers(&module.code, &ModuleType::Js, &mapping)?;
            module.code = code;
        }
        // Resolve re-export sources to module ids (specifier matching would
        // not survive the resolved-URL rewriting below; `mapping` misses
        // the import-map path, so key off the recorded imports instead).
        let mut exports = parsed.export_details.clone();
        for export in &mut exports {
            if let Some(from) = export.from.as_deref() {
                if let Some((_, id, _)) = imports.iter().find(|(spec, _, _)| spec == from) {
                    export.target = Some(id.clone());
                }
            }
        }
        module.imports = imports;
        module.shake = Some(ferrite_transform::ShakeInfo {
            import_bindings,
            exports,
        });
        Ok(module)
    }

    /// Convert CJS to an ESM wrapper (§18).
    async fn convert_cjs(
        &self,
        ctx: &PluginContext<'_>,
        module: PipelineModule,
        environment: &ferrite_core::Environment,
    ) -> Result<PipelineModule> {
        let requires = collect_requires(&module.code);
        let mut mapping: HashMap<String, String> = HashMap::new();
        let mut imports = module.imports.clone();
        let mut shake = module.shake.clone();
        for specifier in &requires {
            if let Ok(resolved) = self
                .resolve_id(ctx, specifier, Some(&module.id), environment)
                .await
            {
                mapping.insert(specifier.clone(), resolved.id.0.clone());
                imports.push((specifier.clone(), resolved.id, ImportKind::Static));
                // The interop reads `default ?? whole`: namespace use.
                if let Some(shake) = shake.as_mut() {
                    shake
                        .import_bindings
                        .push(vec![ferrite_transform::ImportBinding::Namespace]);
                }
            }
        }
        let mut prelude = String::from(
            "const __ferrite_interop__ = (m) => (m && m.__esModule ? m.default : (m?.default ?? m));\n",
        );
        let mut code = module.code.clone();
        for (index, specifier) in requires.iter().enumerate() {
            if let Some(url) = mapping.get(specifier) {
                prelude.push_str(&format!(
                    "import * as __ferrite_cjs_dep{index}__ from {url:?};\n"
                ));
                let replacement = format!("__ferrite_interop__(__ferrite_cjs_dep{index}__)");
                code = replace_require(&code, specifier, &replacement);
            }
        }
        let wrapped = format!(
            "{prelude}\
             const module = {{ exports: {{}} }};\n\
             const exports = module.exports;\n\
             const __filename = {:?};\n\
             const __dirname = {:?};\n\
             {code}\n\
             export default module.exports;\n",
            module.id.0,
            module.id.0.rsplit_once('/').map_or("/", |(dir, _)| dir),
        );
        // The wrapper exports only `default`; re-export facts from the
        // original source no longer describe this code.
        if let Some(shake) = shake.as_mut() {
            shake.exports = vec![ferrite_transform::ParsedExport {
                exported: "default".to_string(),
                local: None,
                from: None,
                imported: None,
                target: None,
            }];
        }
        Ok(PipelineModule {
            code: wrapped,
            imports,
            has_module_syntax: true,
            shake,
            ..module
        })
    }

    /// Asset query shims (`?raw`, `?url`, `?inline`, `?worker`, `?wasm`).
    async fn asset_query_shim(
        &self,
        id: &ModuleId,
        path_part: &str,
        query: &str,
        source: &str,
    ) -> Result<Option<PipelineModule>> {
        let kind = query.split('&').next().unwrap_or("");
        match kind {
            "raw" => Ok(Some(PipelineModule::code_only(
                id.clone(),
                raw_to_js(source),
                ModuleType::Js,
            ))),
            "url" => Ok(Some(PipelineModule::code_only(
                id.clone(),
                asset_to_js(path_part),
                ModuleType::Js,
            ))),
            "inline" => {
                let file = self
                    .id_to_file(id)
                    .unwrap_or_else(|_| PathBuf::from(path_part));
                let bytes = std::fs::read(&file).unwrap_or_default();
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    asset_to_js(&to_data_url(&bytes, path_part)),
                    ModuleType::Js,
                )))
            }
            "worker" => {
                let js = format!(
                    "export default class FerriteWorker extends Worker {{\n  constructor(options) {{\n    super({path_part:?}, {{ type: \"module\", ...options }});\n  }}\n}}\n"
                );
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    js,
                    ModuleType::Js,
                )))
            }
            "wasm" => {
                let js = format!(
                    "export default async function init(input) {{\n  const source = input ?? {path_part:?};\n  const bytes = await fetch(source).then((r) => r.arrayBuffer());\n  const {{ instance }} = await WebAssembly.instantiate(bytes);\n  return instance.exports;\n}}\n"
                );
                Ok(Some(PipelineModule::code_only(
                    id.clone(),
                    js,
                    ModuleType::Js,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Resolve a path relative to a module (CSS refs).
    fn resolve_relative(
        &self,
        from_id: &str,
        specifier: &str,
        environment: &ferrite_core::Environment,
    ) -> Result<String> {
        let importer = ModuleId::new(from_id);
        let resolver = if environment.kind.is_ssr() {
            &self.inner.ssr_resolver
        } else {
            &self.inner.client_resolver
        };
        Ok(resolver
            .resolve(&ResolveRequest {
                specifier,
                importer: Some(&importer),
                environment: environment.kind.clone(),
                kind: ResolveKind::Css,
            })?
            .id
            .0)
    }

    /// Rewrite `<script type="module">` + `<link>` sources in HTML.
    async fn rewrite_html_scripts(&self, html: &str) -> String {
        let environment = self.inner.config.client_env();
        let ctx = self.plugin_context(&environment);
        let mut mapping: HashMap<String, String> = HashMap::new();
        for entry in ferrite_html::discover_entries(html) {
            let spec = entry.src.clone();
            if spec.starts_with("/@") || spec.starts_with("http") {
                continue;
            }
            if let Ok(resolved) = self.resolve_id(&ctx, &spec, None, &environment).await {
                mapping.insert(spec, resolved.id.0);
            }
        }
        let rewritten = ferrite_html::rewrite_module_scripts(html, |src| {
            mapping.get(src).cloned().unwrap_or_else(|| src.to_string())
        });
        // Stylesheet links get `?direct` so the pipeline serves CSS, not the JS wrapper.
        rewrite_link_direct(&rewritten)
    }

    /// `import.meta.env.*` defines (§42).
    fn env_defines(&self, ssr: bool) -> HashMap<String, String> {
        let mut defines = HashMap::new();
        for (key, value) in &self.inner.env_vars {
            defines.insert(
                format!("import.meta.env.{key}"),
                serde_json::to_string(value).unwrap_or_default(),
            );
        }
        defines.insert(
            "import.meta.env.MODE".to_string(),
            serde_json::to_string(&self.inner.config.mode).unwrap_or_default(),
        );
        defines.insert(
            "import.meta.env.BASE_URL".to_string(),
            serde_json::to_string(&self.inner.config.base).unwrap_or_default(),
        );
        defines.insert(
            "import.meta.env.DEV".to_string(),
            (!self.inner.config.is_production).to_string(),
        );
        defines.insert(
            "import.meta.env.PROD".to_string(),
            self.inner.config.is_production.to_string(),
        );
        defines.insert("import.meta.env.SSR".to_string(), ssr.to_string());
        defines
    }

    /// Raw source for one module id: plugin `load` first, then fs.
    ///
    /// Build CSS extraction uses this so virtual stylesheets (e.g.
    /// `ferrite:tailwind.css`) resolve through plugins instead of failing
    /// on a missing file.
    pub async fn load_raw_source(&self, id: &ModuleId, env: &str) -> Result<(String, ModuleType)> {
        let environment = if env == "ssr" {
            self.inner.config.ssr_env()
        } else {
            self.inner.config.client_env()
        };
        let ctx = self.plugin_context(&environment);
        // Same `/@id/` → `\0` mapping as `pipeline_module` step 0 (§14).
        let owned;
        let id = match url_to_virtual(&id.0) {
            Some(virtual_id) => {
                owned = virtual_id;
                &owned
            }
            None => id,
        };
        self.load_source(&ctx, id, &environment).await
    }

    /// Map a module id to a file path.
    pub fn id_to_file(&self, id: &ModuleId) -> Result<PathBuf> {
        let (path, _) = id.split_query();
        if let Some(rest) = path.strip_prefix("/@npm/") {
            return Ok(self
                .inner
                .config
                .root
                .join(".ferrite/npm/packages")
                .join(rest));
        }
        if let Some(rest) = path.strip_prefix("/@fs") {
            return Ok(PathBuf::from(rest));
        }
        if let Some(rest) = path.strip_prefix("/@ferrite/") {
            return Err(FerriteError::Resolve(format!(
                "no file for virtual `{rest}`"
            )));
        }
        Ok(self.inner.config.root.join(path.trim_start_matches('/')))
    }

    /// True when `url` maps to a project file.
    fn is_project_file(&self, url: &str) -> bool {
        self.id_to_file(&ModuleId::new(url))
            .is_ok_and(|file| file.is_file())
    }

    /// Cache key for a transform.
    fn cache_key(&self, id: &ModuleId, source: &str, env: &str) -> Hash {
        let pipeline = self.inner.plugins.names().join(",");
        ferrite_cache::transform_key(&ferrite_cache::TransformKeyInput {
            source,
            module_id: &id.0,
            compiler_version: "oxc-0.151",
            pipeline_hash: &Hash::of_str(&pipeline).0,
            environment: env,
            target: &self.inner.config.build.target,
            mode: &self.inner.config.mode,
        })
    }

    /// Record a module in the graph.
    fn update_graph(&self, module: &PipelineModule) {
        let mut node = self.inner.graph.get(&module.id).unwrap_or_else(|| {
            ModuleNode::new(
                module.id.clone(),
                module.id.0.clone(),
                module.module_type.clone(),
            )
        });
        node.module_type = module.module_type.clone();
        node.url = module.id.0.clone();
        if let Ok(file) = self.id_to_file(&module.id) {
            node.file = Some(file);
        }
        node.hmr.self_accepting = module.uses_import_meta_hot;
        self.inner.graph.upsert(node);
        let edges: Vec<ImportEdge> = module
            .imports
            .iter()
            .map(|(specifier, resolved, kind)| ImportEdge {
                specifier: specifier.clone(),
                resolved: resolved.clone(),
                kind: kind.clone(),
            })
            .collect();
        self.inner.graph.set_imports(&module.id, edges);
        self.inner.graph.set_transformed(
            &module.id,
            "client",
            module.code.clone(),
            &Hash::of_str(&module.code),
        );
    }

    /// Build a plugin context for `environment`.
    fn plugin_context<'a>(
        &'a self,
        environment: &'a ferrite_core::Environment,
    ) -> PluginContext<'a> {
        PluginContext {
            graph: &self.inner.graph,
            resolver: &self.inner.client_resolver,
            environment,
            emitted: &self.inner.emitted,
            watch_files: &self.inner.watch_files,
            warnings: &self.inner.warnings,
        }
    }

    /// Start the file watcher (§60 HMR invalidation).
    fn start_watcher(&self) -> Result<()> {
        let inner = self.inner.clone();
        let debounce_window = Duration::from_millis(80);
        let mut watcher = notify::recommended_watcher(
            move |result: std::result::Result<Event, notify::Error>| {
                let Ok(event) = result else { return };
                if !matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                ) {
                    return;
                }
                for path in event.paths {
                    // Debounce.
                    let now = Instant::now();
                    if let Ok(mut debounce) = inner.debounce.lock() {
                        if let Some(last) = debounce.get(&path) {
                            if now.duration_since(*last) < debounce_window {
                                continue;
                            }
                        }
                        debounce.insert(path.clone(), now);
                    }
                    // Skip output/cache dirs.
                    if path.components().any(|component| {
                        matches!(
                            component.as_os_str().to_str(),
                            Some("dist" | ".ferrite" | "node_modules" | "target")
                        )
                    }) {
                        continue;
                    }
                    let url = ferrite_core::file_to_url(&inner.config.root, &path);
                    let id = ModuleId::new(url.clone());
                    if !inner.graph.contains(&id) {
                        // Untracked file (e.g. new CSS referenced later): if it is
                        // tracked by importers, full-reload is safest.
                        continue;
                    }
                    inner.graph.invalidate_tree(&id);
                    let timestamp = now_millis();
                    // Plugin `handleHotUpdate` hooks run on the async runtime.
                    let inner_clone = inner.clone();
                    let file = url.clone();
                    tokio::spawn(async move {
                        let modules = vec![id.clone()];
                        let event = HotUpdateEvent {
                            file,
                            modules,
                            timestamp,
                        };
                        let environment = inner_clone.config.client_env();
                        let ctx = PluginContext {
                            graph: &inner_clone.graph,
                            resolver: &inner_clone.client_resolver,
                            environment: &environment,
                            emitted: &inner_clone.emitted,
                            watch_files: &inner_clone.watch_files,
                            warnings: &inner_clone.warnings,
                        };
                        if let Ok(Some(custom)) = inner_clone
                            .plugins
                            .hook_handle_hot_update(&ctx, event)
                            .await
                        {
                            if custom.full_reload {
                                inner_clone.hmr.send_full_reload(Some(id.0.clone()));
                                return;
                            }
                            if !custom.modules.is_empty() {
                                let updates = custom
                                    .modules
                                    .iter()
                                    .map(|module| ferrite_hmr::HmrUpdate {
                                        kind: "js-update".to_string(),
                                        path: module.0.clone(),
                                        accepted_path: module.0.clone(),
                                        timestamp,
                                        css_only: false,
                                    })
                                    .collect();
                                inner_clone.hmr.send_update(updates);
                                return;
                            }
                        }
                        match plan_update(&inner_clone.graph, &id, timestamp) {
                            HmrPlan::Update(updates) => inner_clone.hmr.send_update(updates),
                            HmrPlan::FullReload => {
                                inner_clone.hmr.send_full_reload(Some(id.0.clone()));
                            }
                        }
                    });
                }
            },
        )
        .map_err(|error| FerriteError::Other(format!("watcher failed: {error}")))?;
        watcher
            .watch(&self.inner.config.root, RecursiveMode::Recursive)
            .map_err(|error| FerriteError::Other(format!("watch failed: {error}")))?;
        if let Ok(mut slot) = self.watcher.lock() {
            *slot = Some(watcher);
        }
        Ok(())
    }
}

/// A transformed module in the dev/build pipeline.
#[derive(Debug, Clone)]
pub struct PipelineModule {
    /// Module id.
    pub id: ModuleId,
    /// Final code.
    pub code: String,
    /// Resolved imports.
    pub imports: Vec<(String, ModuleId, ImportKind)>,
    /// Package `sideEffects`, when known.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Source map JSON.
    pub map: Option<String>,
    /// True when the code uses ESM syntax.
    pub has_module_syntax: bool,
    /// True when `import.meta.hot` is used.
    pub uses_import_meta_hot: bool,
    /// True when the response must be raw file bytes.
    pub is_raw_bytes: bool,
    /// Statement-DCE facts (`None` = opaque, keep everything).
    pub shake: Option<ferrite_transform::ShakeInfo>,
}

impl PipelineModule {
    /// Code-only module.
    #[must_use]
    pub fn code_only(id: ModuleId, code: String, module_type: ModuleType) -> Self {
        Self {
            id,
            code,
            imports: Vec::new(),
            side_effects: None,
            module_type,
            map: None,
            has_module_syntax: true,
            uses_import_meta_hot: false,
            is_raw_bytes: false,
            shake: None,
        }
    }

    /// Raw-bytes module (served verbatim).
    #[must_use]
    pub fn raw(id: ModuleId, module_type: ModuleType) -> Self {
        Self {
            id,
            code: String::new(),
            imports: Vec::new(),
            side_effects: None,
            module_type,
            map: None,
            has_module_syntax: false,
            uses_import_meta_hot: false,
            is_raw_bytes: true,
            shake: None,
        }
    }

    /// Rebuild from cache.
    #[must_use]
    pub fn from_cached(id: ModuleId, cached: CachedTransform) -> Self {
        Self {
            id,
            code: cached.code,
            imports: cached
                .imports
                .into_iter()
                .map(|(specifier, resolved, kind)| (specifier, ModuleId::new(resolved), kind))
                .collect(),
            side_effects: cached.side_effects,
            module_type: cached.module_type,
            map: cached.map,
            has_module_syntax: true,
            uses_import_meta_hot: cached.uses_import_meta_hot,
            is_raw_bytes: false,
            shake: cached.shake,
        }
    }
}

/// Cached transform payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedTransform {
    /// Code.
    pub code: String,
    /// Imports (`specifier`, `resolved`, `kind`).
    pub imports: Vec<(String, String, ImportKind)>,
    /// Side effects.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Map JSON.
    pub map: Option<String>,
    /// HMR flag.
    pub uses_import_meta_hot: bool,
    /// Statement-DCE facts (`None` for stale caches → keep everything).
    #[serde(default)]
    pub shake: Option<ferrite_transform::ShakeInfo>,
}

impl CachedTransform {
    /// Snapshot a module.
    #[must_use]
    pub fn from_module(module: &PipelineModule) -> Self {
        Self {
            code: module.code.clone(),
            imports: module
                .imports
                .iter()
                .map(|(specifier, id, kind)| (specifier.clone(), id.0.clone(), kind.clone()))
                .collect(),
            side_effects: module.side_effects,
            module_type: module.module_type.clone(),
            map: module.map.clone(),
            uses_import_meta_hot: module.uses_import_meta_hot,
            shake: module.shake.clone(),
        }
    }
}

/// An HTTP-ready pipeline response.
#[derive(Debug)]
pub struct PipelineResponse {
    /// Body bytes.
    pub body: Vec<u8>,
    /// Content type.
    pub content_type: String,
}

impl PipelineResponse {
    /// Text response.
    #[must_use]
    pub fn code(code: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            body: code.into().into_bytes(),
            content_type: content_type.into(),
        }
    }

    /// Binary response.
    #[must_use]
    pub fn bytes(bytes: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            body: bytes,
            content_type: content_type.into(),
        }
    }
}

// --- HTTP handlers -----------------------------------------------------------

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(inner): State<Arc<DevServerInner>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, inner))
}

async fn handle_socket(socket: WebSocket, inner: Arc<DevServerInner>) {
    use futures::{SinkExt as _, StreamExt as _};
    let (mut sender, mut receiver) = socket.split();
    // Handshake.
    let hello = serde_json::to_string(&ferrite_hmr::HmrMessage::Connected {
        version: ferrite_core::VERSION.to_string(),
    })
    .unwrap_or_default();
    if sender.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    let mut rx = inner.hmr.subscribe();
    let send_task = tokio::spawn(async move {
        while let Ok(text) = rx.recv().await {
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(message) = receiver.next().await {
        match message {
            Ok(Message::Text(text)) => {
                tracing::debug!("hmr client message: {text:?}");
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
    send_task.abort();
}

async fn client_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/javascript")],
        client_source(),
    )
}

async fn inspect_handler(State(inner): State<Arc<DevServerInner>>) -> impl IntoResponse {
    let modules = inner.graph.module_ids();
    let body = serde_json::json!({
        "version": ferrite_core::VERSION,
        "root": inner.config.root,
        "mode": inner.config.mode,
        "plugins": inner.plugins.names(),
        "modules": modules.len(),
        "moduleSample": modules.iter().take(50).map(|id| &id.0).collect::<Vec<_>>(),
        "hmrReceivers": inner.hmr.receivers(),
        "warnings": inner.warnings.lock().map(|w| w.clone()).unwrap_or_default(),
    });
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
}

async fn rpc_handler(
    State(inner): State<Arc<DevServerInner>>,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let hash = uri
        .path()
        .strip_prefix(RPC_ROUTE_PREFIX)
        .unwrap_or("")
        .to_string();
    let encoding = RpcEncoding::from_content_type(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
    );
    let args = match encoding.decode(&body) {
        Ok(args) => args,
        Err(error) => return error_response(&error),
    };
    let registry = inner.rpc.read().await;
    match registry
        .invoke(&ferrite_ssr::RpcRequest { hash, args })
        .await
    {
        Ok(value) => match encoding.encode(&value) {
            Ok(bytes) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, encoding.content_type())],
                bytes,
            )
                .into_response(),
            Err(error) => error_response(&error),
        },
        Err(error) => error_response(&error),
    }
}

async fn fallback_handler(
    State(inner): State<Arc<DevServerInner>>,
    uri: Uri,
    _headers: HeaderMap,
) -> Response {
    let path = uri.path().to_string();
    let query = uri
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let url = format!("{path}{query}");
    // Assemble a lightweight server handle for the pipeline.
    let server = DevServerRef {
        inner: inner.clone(),
    };
    match server.transform_request_owned(&url).await {
        Ok(response) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                response.content_type.clone(),
            )],
            response.body,
        )
            .into_response(),
        Err(error) => {
            // SSR adapter fallback for app routes (no extension, not a file).
            if !path.contains('.') {
                let adapter = inner.ssr_adapter.read().await.clone();
                if let Some(adapter) = adapter {
                    let request = SsrHttpRequest {
                        method: "GET".to_string(),
                        uri: url.clone(),
                        headers: Vec::new(),
                        body: Vec::new(),
                    };
                    let context = SsrContext {
                        url,
                        ..Default::default()
                    };
                    match adapter.render(request, context).await {
                        Ok(ssr) => {
                            let status = StatusCode::from_u16(ssr.status).unwrap_or(StatusCode::OK);
                            match ssr.body {
                                ferrite_ssr::RenderBody::Full(html) => {
                                    return (
                                        status,
                                        [(
                                            axum::http::header::CONTENT_TYPE,
                                            "text/html; charset=utf-8",
                                        )],
                                        html,
                                    )
                                        .into_response();
                                }
                                ferrite_ssr::RenderBody::Stream(stream) => {
                                    use futures::StreamExt as _;
                                    let mapped = stream.map(|item| {
                                        item.map_err(|error| {
                                            std::io::Error::other(error.to_string())
                                        })
                                    });
                                    let body = axum::body::Body::from_stream(mapped);
                                    return (status, body).into_response();
                                }
                            }
                        }
                        Err(error) => return error_response(&error),
                    }
                }
            }
            error_response(&error)
        }
    }
}

/// Pipeline access without watcher ownership (HTTP handlers).
struct DevServerRef {
    inner: Arc<DevServerInner>,
}

impl DevServerRef {
    /// Mirror of [`DevServer::transform_request`] for borrowed state.
    async fn transform_request_owned(&self, url: &str) -> Result<PipelineResponse> {
        // Reuse the same logic by constructing a temporary handle. The
        // watcher field is unused on this path.
        let server = DevServer {
            inner: self.inner.clone(),
            watcher: Arc::new(Mutex::new(None)),
        };
        // Avoid re-running watcher setup: call the pipeline directly.
        server.transform_request_inner(url).await
    }
}

impl DevServer {
    /// Inner request transform (shared by owned + borrowed handles).
    async fn transform_request_inner(&self, url: &str) -> Result<PipelineResponse> {
        self.transform_request(url).await
    }
}

fn error_response(error: &FerriteError) -> Response {
    let diagnostic = error.diagnostic();
    let body = serde_json::json!({
        "error": diagnostic.message,
        "code": diagnostic.code,
        "id": diagnostic.id,
        "frame": diagnostic.frame,
    });
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

// --- helpers -----------------------------------------------------------------

/// Load `.env*` files (§42), keeping only `prefix`-allowed keys.
#[must_use]
pub fn load_env_files(root: &Path, mode: &str, prefixes: &[String]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for file in [
        ".env",
        ".env.local",
        &format!(".env.{mode}"),
        &format!(".env.{mode}.local"),
    ] {
        let path = root.join(file);
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (key, value) in parse_dotenv(&text) {
                values.insert(key, value);
            }
        }
    }
    values.retain(|key, _| prefixes.iter().any(|prefix| key.starts_with(prefix)));
    values
}

/// Minimal dotenv parser (no variable expansion in v0.1).
#[must_use]
pub fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        if !key.is_empty() {
            pairs.push((key, value));
        }
    }
    pairs
}

/// True when the module must be served as raw bytes.
fn is_raw_asset(path: &str, module_type: &ModuleType, query: Option<&str>) -> bool {
    if query.is_some() {
        return false; // queries are handled as shims/transforms
    }
    matches!(
        module_type,
        ModuleType::Asset | ModuleType::Wasm | ModuleType::Data
    ) || is_binary_extension(path)
}

/// True for binary extensions served verbatim.
fn is_binary_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".ico", ".bmp", ".svg", ".woff",
        ".woff2", ".ttf", ".otf", ".eot", ".mp3", ".wav", ".ogg", ".mp4", ".webm", ".pdf", ".zip",
        ".wasm",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// Heuristic binary detection (NUL byte in the first 8 KiB).
fn is_binary_asset(path: &str, bytes: &[u8]) -> bool {
    if is_binary_extension(path) {
        return true;
    }
    bytes.iter().take(8192).any(|byte| *byte == 0)
}

/// Map an internal `\0` virtual id to its servable `/@id/` URL (Vite-style).
#[must_use]
pub fn virtual_url(id: &str) -> String {
    format!("/@id/{}", id.trim_start_matches('\0'))
}

/// Map a `/@id/` URL back to its internal virtual id.
#[must_use]
pub fn url_to_virtual(url: &str) -> Option<ModuleId> {
    url.strip_prefix("/@id/")
        .map(|rest| ModuleId::new(format!("\0{rest}")))
}

/// True when an imported URL should go through the `?asset-shim` path.
fn should_shim_asset(url: &str) -> bool {
    let path = url.split('?').next().unwrap_or(url);
    matches!(ModuleType::from_path(path), ModuleType::Asset) || is_binary_extension(path)
}

/// True when CJS conversion applies (§18).
fn needs_cjs_conversion(id: &ModuleId, code: &str, has_module_syntax: bool) -> bool {
    if has_module_syntax {
        return false;
    }
    let (path, _) = id.split_query();
    if path.ends_with(".cjs") || path.ends_with(".cts") {
        return true;
    }
    code.contains("module.exports") || code.contains("require(")
}

/// Collect `require("...")` specifiers (documented approximation).
fn collect_requires(code: &str) -> Vec<String> {
    let pattern = regex_lite_require();
    pattern
        .captures_iter(code)
        .filter_map(|captures| captures.get(1).map(|m| m.as_str().to_string()))
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn regex_lite_require() -> regex::Regex {
    regex::Regex::new(r#"require\(\s*["']([^"']+)["']\s*\)"#).unwrap()
}

/// Replace `require("spec")` with `replacement`.
fn replace_require(code: &str, specifier: &str, replacement: &str) -> String {
    let mut output = code.to_string();
    for quote in ['"', '\''] {
        for space in ["", " ", "  "] {
            let from = format!("require({space}{quote}{specifier}{quote}{space})");
            output = output.replace(&from, replacement);
        }
    }
    output
}

/// Self-contained production CSS injection (no dev client import).
fn production_css_js(module_id: &str, css: &str, exports: &HashMap<String, String>) -> String {
    let exports_json = serde_json::to_string(exports).unwrap_or_else(|_| "{}".to_string());
    format!(
        "const __css__ = {css:?};\n\
         if (typeof document !== \"undefined\") {{\n\
         const __el__ = document.createElement(\"style\");\n\
         __el__.setAttribute(\"data-ferrite-id\", {module_id:?});\n\
         __el__.textContent = __css__;\n\
         document.head.appendChild(__el__);\n\
         }}\n\
         export default {exports_json};\n"
    )
}

/// Minimal `node:` shim (§19 browser-shims mode).
fn node_shim(name: &str) -> String {
    match name {
        "process" => "export const env = {};\nexport const argv = [];\nexport default { env, argv };\n".to_string(),
        "buffer" => "export const Buffer = globalThis.Buffer;\nexport default globalThis.Buffer;\n".to_string(),
        "events" => "export class EventEmitter { on(){} off(){} emit(){} }\nexport default EventEmitter;\n".to_string(),
        "util" => "export const format = (...a) => a.join(\" \");\nexport const inspect = (v) => String(v);\nexport default { format, inspect };\n".to_string(),
        "path" => "export const sep = \"/\";\nexport const join = (...p) => p.join(\"/\");\nexport const dirname = (p) => p.split(\"/\").slice(0, -1).join(\"/\");\nexport const basename = (p) => p.split(\"/\").pop();\nexport default { sep, join, dirname, basename };\n".to_string(),
        "url" => "export const URL = globalThis.URL;\nexport const URLSearchParams = globalThis.URLSearchParams;\nexport default { URL, URLSearchParams };\n".to_string(),
        _ => format!("// ferrite node:{name} shim (browser-shims)\nexport default {{}};\n"),
    }
}

/// Maximum fetched remote module size (§72).
const MAX_REMOTE_BYTES: usize = 8 * 1024 * 1024;

/// True when `host` matches the `[remote] allow` list (exact or `*.` suffix).
fn remote_host_allowed(host: &str, allow: &[String]) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host).to_ascii_lowercase();
    allow.iter().any(|entry| {
        let entry = entry
            .strip_suffix('.')
            .unwrap_or(entry)
            .to_ascii_lowercase();
        entry == host
            || entry
                .strip_prefix("*.")
                .is_some_and(|suffix| !suffix.is_empty() && host.ends_with(&format!(".{suffix}")))
    })
}

/// True for bare npm-style specifiers (resolver step 11): anything that is
/// not virtual, builtin, remote, absolute, relative, or a `#` import.
fn is_bare_specifier(specifier: &str) -> bool {
    !(specifier.starts_with('\0')
        || specifier.starts_with("node:")
        || specifier.starts_with("rust:")
        || specifier.starts_with("https://")
        || specifier.starts_with("http://")
        || specifier.starts_with("data:")
        || specifier.starts_with("blob:")
        || specifier.starts_with('/')
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier == "."
        || specifier == ".."
        || specifier.starts_with('#'))
}

/// Rewrite `<link rel="stylesheet" href>` to `?direct` URLs.
fn rewrite_link_direct(html: &str) -> String {
    let pattern = regex::Regex::new(r#"<link([^>]*?)href="([^"]+)"([^>]*?)>"#).unwrap();
    pattern
        .replace_all(html, |captures: &regex::Captures| {
            let (pre, href, post) = (&captures[1], &captures[2], &captures[3]);
            let is_stylesheet = pre.contains("stylesheet") || post.contains("stylesheet");
            if is_stylesheet && !href.contains('?') && !href.starts_with("http") {
                format!("<link{pre}href=\"{href}?direct\"{post}>")
            } else {
                captures[0].to_string()
            }
        })
        .into_owned()
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Default compiler: Oxc (spec §5).
#[must_use]
pub fn default_compiler() -> Arc<dyn JsCompiler> {
    Arc::new(OxcCompiler::new(OxcOptions::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
