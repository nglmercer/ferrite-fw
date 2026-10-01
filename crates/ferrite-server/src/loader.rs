//! Dev server module loading.

//! Dev server module transform pipeline.

use crate::util::*;
use crate::CachedTransform;
use crate::DevServer;
use crate::PipelineModule;
use ferrite_assets::asset_to_js;
use ferrite_core::EnvironmentKind;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_plugin::LoadRequest;
use ferrite_plugin::PluginContext;
use ferrite_plugin::ResolveHookRequest;
use ferrite_plugin::TransformRequest as HookTransformRequest;
use ferrite_resolver::ResolveKind;
use ferrite_resolver::ResolveRequest;
use ferrite_resolver::ResolvedId;

impl DevServer {
    /// Full module pipeline shared by HTTP + bundler loader.
    pub async fn pipeline_module(
        &self,
        id: &ModuleId,
        importer: Option<&ModuleId>,
        env: &str,
    ) -> Result<PipelineModule> {
        let ssr = env == "ssr";
        let environment = self.environment_for(env);
        let mut ctx = self.plugin_context(&environment);
        // Watch registrations belong to this request, not unrelated concurrent modules.
        let module_watches = std::sync::Mutex::new(Vec::new());
        ctx.watch_files = &module_watches;
        // 0. `/@id/` URLs map back to internal `\0` virtual ids (§14).
        let id = unvirtualize(id);
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
        let loaded = self
            .load_source_full(&ctx, &resolved_id, &environment)
            .await?;
        let source = loaded.code;
        let mut module_type = loaded.module_type;
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
                .asset_query_shim(&ctx, &resolved_id, path_part, q, &source)
                .await?
            {
                return Ok(shim);
            }
        }
        // 7. Cache lookup (`shouldTransformCachedModule` may force a
        //    re-transform; by default the cached entry wins).
        let defines = self.transform_defines(&environment);
        let source_identity = format!(
            "{source}\0{module_type:?}\0{:?}\0{:?}\0{:?}\0{:?}",
            loaded.dependencies, loaded.map, loaded.side_effects, resolved.side_effects
        );
        let cache_key = self.cache_key(&resolved_id, &source_identity, env, &defines);
        if let Some(cached) = self.inner.cache.get(&cache_key.0) {
            if let Ok(cached) = serde_json::from_slice::<CachedTransform>(&cached) {
                let retransform = self
                    .inner
                    .plugins
                    .hook_should_transform_cached_module(
                        &ctx,
                        ferrite_plugin::CachedModuleInfo {
                            id: resolved_id.clone(),
                            environment: environment.kind.clone(),
                        },
                    )
                    .await?;
                if !retransform && cached.dependencies_current() {
                    let module = PipelineModule::from_cached(resolved_id.clone(), cached);
                    self.update_graph(&module, env);
                    return Ok(module);
                }
            }
        }
        // Framework/pre transforms see the original syntax, before JS/TS lowering.
        let pre = self
            .inner
            .plugins
            .hook_transform_phase(
                &ctx,
                HookTransformRequest {
                    id: resolved_id.0.clone(),
                    code: source.clone(),
                    module_type: module_type.clone(),
                    environment: environment.kind.clone(),
                    ssr,
                },
                ferrite_plugin::TransformPhase::BeforeLowering,
            )
            .await?;
        if let Some(kind) = pre.module_type {
            module_type = kind;
        }
        let pre_map = merge_maps(
            pre.map.map(|map| map.mappings),
            loaded.map.map(|map| map.mappings),
            pre.code == source,
        )?;
        let mut module = self
            .core_transform(&ctx, &resolved_id, &pre.code, &module_type, &environment)
            .await?;
        module.map = merge_maps(module.map, pre_map, module.code == pre.code)?;
        module.side_effects = loaded.side_effects.or(resolved.side_effects);
        module.dependencies = loaded.dependencies;
        module.dependencies.push(self.inner.config.lockfile().to_string_lossy().into_owned());
        module.dependencies.extend(pre.dependencies);
        // Normal/post transforms see lowered output; retain maps and watches.
        if module.module_type.is_js_like() {
            let hooked = self
                .inner
                .plugins
                .hook_transform_phase(
                    &ctx,
                    HookTransformRequest {
                        id: resolved_id.0.clone(),
                        code: module.code.clone(),
                        module_type: module.module_type.clone(),
                        environment: environment.kind.clone(),
                        ssr,
                    },
                    ferrite_plugin::TransformPhase::AfterLowering,
                )
                .await?;
            module.map = merge_maps(
                hooked.map.map(|map| map.mappings),
                module.map,
                hooked.code == module.code,
            )?;
            module.code = hooked.code;
            if let Some(kind) = hooked.module_type {
                module.module_type = kind;
            }
            module.dependencies.extend(hooked.dependencies);
        }
        module.dependencies.extend(
            module_watches
                .lock()
                .map_err(|_| FerriteError::Other("module watch lock poisoned".into()))?
                .clone(),
        );
        module.dependencies = module
            .dependencies
            .iter()
            .map(|dependency| {
                let path = std::path::Path::new(dependency);
                let file = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    self.inner.config.root.join(path)
                };
                ferrite_core::normalize_path(&file)
            })
            .collect();
        module.dependencies.sort();
        module.dependencies.dedup();
        for dependency in &module.dependencies {
            self.watch_extra(std::path::Path::new(dependency));
        }
        // Determine CJS from the final plugin output, never stale core metadata.
        if module.module_type.is_js_like() {
            let parsed = self.inner.compiler.parse(ferrite_transform::ParseRequest {
                id: module.id.0.clone(),
                code: module.code.clone(),
                module_type: module.module_type.clone(),
            })?;
            module.has_module_syntax = parsed.has_module_syntax;
            module.uses_import_meta_hot = parsed.uses_import_meta_hot;
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
            let (code, map) = ferrite_transform::inject_hmr_mapped(
                &module.id.0,
                &module.code,
                module.map.is_some(),
            )?;
            module.map = merge_maps(map, module.map, code == module.code)?;
            module.code = code;
        }
        // 13. Graph update + hooks + cache.
        self.update_graph(&module, env);
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
    pub(crate) async fn resolve_id(
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
    pub(crate) fn resolve_remote(&self, specifier: &str) -> Result<ResolvedId> {
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
    pub(crate) async fn fetch_remote(&self, url: &str) -> Result<String> {
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
    pub(crate) async fn load_source(
        &self,
        ctx: &PluginContext<'_>,
        id: &ModuleId,
        environment: &ferrite_core::Environment,
    ) -> Result<(String, ModuleType)> {
        let loaded = self.load_source_full(ctx, id, environment).await?;
        Ok((loaded.code, loaded.module_type))
    }

    pub(crate) async fn load_source_full(
        &self,
        ctx: &PluginContext<'_>,
        id: &ModuleId,
        environment: &ferrite_core::Environment,
    ) -> Result<ferrite_plugin::LoadResult> {
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
            return Ok(loaded);
        }
        // Built-in virtual modules.
        if let Some(virtual_source) = self.builtin_virtual(id) {
            return Ok(source_result(virtual_source, ModuleType::Js));
        }
        // `node:` shims.
        if let Some(name) = id.0.strip_prefix("\0node:") {
            return Ok(source_result(node_shim(name), ModuleType::Js));
        }
        // Remote modules (§72).
        if let Some(url) = id.0.strip_prefix("\0remote:") {
            let source = self.fetch_remote(url).await?;
            return Ok(source_result(source, ModuleType::Js));
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
            return Ok(source_result(String::new(), ModuleType::Asset));
        }
        let source = String::from_utf8(bytes)
            .map_err(|_| FerriteError::Resolve(format!("`{id}` is not valid UTF-8")))?;
        Ok(source_result(source, module_type))
    }

    /// Built-in virtual modules (`virtual:ferrite/*`, §14).
    pub(crate) fn builtin_virtual(&self, id: &ModuleId) -> Option<String> {
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
}

fn source_result(code: String, module_type: ModuleType) -> ferrite_plugin::LoadResult {
    ferrite_plugin::LoadResult {
        code,
        module_type,
        ..Default::default()
    }
}

pub(crate) fn merge_maps(
    outer: Option<String>,
    inner: Option<String>,
    unchanged: bool,
) -> Result<Option<String>> {
    match (outer, inner) {
        (Some(outer), Some(inner)) => {
            ferrite_transform::chain_source_maps(&outer, &inner).map(Some)
        }
        (Some(outer), None) => Ok(Some(outer)),
        (None, inner) if unchanged => Ok(inner),
        (None, _) => Ok(None),
    }
}
