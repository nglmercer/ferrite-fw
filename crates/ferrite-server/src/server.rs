//! Dev server lifecycle, router, and request entry points.

use crate::routes::client_handler;
use crate::routes::fallback_handler;
use crate::routes::inspect_handler;
use crate::routes::rpc_handler;
use crate::routes::shutdown_signal;
use crate::routes::ws_handler;
use crate::util::*;
use crate::DevServer;
use crate::PipelineResponse;
use axum::routing::get;
use axum::routing::post;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_hmr::client_source;
use ferrite_hmr::plan_update;
use ferrite_hmr::HmrPlan;
use ferrite_hmr::HMR_ENDPOINT;
use ferrite_plugin::HtmlTransformContext;
use ferrite_ssr::SsrModule;
use ferrite_ssr::RPC_ROUTE_PREFIX;
use std::net::SocketAddr;

impl DevServer {
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
        if let Ok(mut bound) = self.inner.bound_addr.lock() {
            *bound = Some(addr);
        }
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

    /// Print Local/Network URLs, the `printUrls` equivalent.
    pub fn print_urls(&self) {
        ferrite_plugin::ServerControl::print_urls(self);
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
        let fallback = rewritten.clone();
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
        // A tags-only hook must keep the core rewrites, not the raw file.
        let html = result.html.unwrap_or(fallback);
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
    pub(crate) async fn collect_import_map(&self, html: &str) {
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
        self.publish_hmr_plan(id, plan_update(&self.inner.graph, id, timestamp))
            .await;
    }

    /// Validate compiler output before browser disposal/import or full reload.
    /// A failed edit retains the previous client and graph acceptance boundary.
    pub(crate) async fn publish_hmr_plan(&self, changed: &ModuleId, plan: HmrPlan) {
        let acceptance: std::collections::HashMap<_, _> = self
            .inner
            .graph
            .module_ids()
            .into_iter()
            .filter_map(|id| self.inner.graph.get(&id).map(|node| (id, node.hmr)))
            .collect();
        if let Err(error) = self.validate_hmr_modules(changed).await {
            // Compile-time graph edges track new dependencies for subsequent
            // fixes, but only the previously executed client can accept HMR.
            for id in self.inner.graph.module_ids() {
                if let Some(mut node) = self.inner.graph.get(&id) {
                    node.hmr = acceptance.get(&id).cloned().unwrap_or_default();
                    self.inner.graph.upsert(node);
                }
            }
            self.report_hmr_error(changed, &error);
            return;
        }
        match plan {
            HmrPlan::Update(updates) => self.inner.hmr.send_update(updates),
            HmrPlan::FullReload => self.inner.hmr.send_full_reload(Some(changed.0.clone())),
        }
    }

    pub(crate) fn report_hmr_error(&self, changed: &ModuleId, error: &FerriteError) {
        let mut diagnostic = error.diagnostic();
        if diagnostic.id.is_none() {
            diagnostic.id = Some(changed.0.clone());
        }
        self.inner.hmr.send_error(diagnostic);
    }

    async fn validate_hmr_modules(&self, changed: &ModuleId) -> Result<()> {
        let mut queue = std::collections::VecDeque::from([changed.clone()]);
        // Watched preprocessor/type inputs can invalidate compiled owners too.
        let mut affected = self.inner.graph.module_ids();
        affected.sort_by(|a, b| a.0.cmp(&b.0));
        queue.extend(affected.into_iter().filter(|id| {
            self.inner
                .graph
                .get(id)
                .is_some_and(|node| node.client.invalidated && node.client.code.is_some())
        }));
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = queue.pop_front() {
            if id.0 == ferrite_hmr::CLIENT_ID || !seen.insert(id.clone()) {
                continue;
            }
            let module = self.pipeline_module(&id, None, "client").await?;
            for (_, dependency, _) in module.imports {
                if self
                    .inner
                    .graph
                    .get(&dependency)
                    .is_none_or(|node| node.client.invalidated || node.client.code.is_none())
                    && !self
                        .resolve_module(&dependency.0, Some(&module.id), "client")
                        .await?
                        .external
                {
                    queue.push_back(dependency);
                }
            }
        }
        Ok(())
    }

    /// Resolve an entry specifier to a module id (build entry discovery).
    pub async fn resolve_entry(&self, specifier: &str, env: &str) -> Result<ModuleId> {
        Ok(self.resolve_module(specifier, None, env).await?.id)
    }

    /// Resolve a module while preserving its type, ownership and side effects.
    pub async fn resolve_module(
        &self,
        specifier: &str,
        importer: Option<&ModuleId>,
        env: &str,
    ) -> Result<ferrite_resolver::ResolvedId> {
        let environment = self.environment_for(env);
        let ctx = self.plugin_context(&environment);
        self.resolve_id(&ctx, specifier, importer, &environment)
            .await
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
}
