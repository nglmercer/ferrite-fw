//! Explicit official compiler integration with the shared dev/build pipeline.
use crate::compiler_host::{CompileRequest, CompileTarget, Framework, NodeCompilerHost};
use ferrite_core::{FerriteError, ModuleId, ModuleType, Result, SourceMap};
use ferrite_plugin::{Enforce, LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};
use std::sync::Arc;

/// Experimental component adapter. Creating it requires an explicitly enabled
/// compiler host; it supplies compilation, not an SSR runtime or HMR protocol.
pub struct HostedFrameworkPlugin {
    framework: Framework,
    host: Arc<NodeCompilerHost>,
    development: bool,
}
impl HostedFrameworkPlugin {
    pub fn new(framework: Framework, host: Arc<NodeCompilerHost>, development: bool) -> Self {
        Self {
            framework,
            host,
            development,
        }
    }
    fn framework_name(&self) -> &'static str {
        match self.framework {
            Framework::Vue => "vue",
            Framework::Svelte => "svelte",
        }
    }
    fn owns(&self, id: &str) -> bool {
        crate::registry::owns_component(self.framework_name(), id)
    }
}
// CSS resource queries are exclusive and deliberately not framework legacy block
// queries. A generated resource is always loaded from its owner's current source.
fn style_query(query: &str) -> Result<Option<usize>> {
    if let Some(index) = query.strip_prefix("ferrite-style=") {
        return index
            .parse()
            .map(Some)
            .map_err(|_| FerriteError::Build(format!("invalid generated style query {query}")));
    }
    Ok(None)
}
#[async_trait::async_trait]
impl Plugin for HostedFrameworkPlugin {
    fn name(&self) -> &'static str {
        match self.framework {
            Framework::Vue => "ferrite:vue-official",
            Framework::Svelte => "ferrite:svelte-official",
        }
    }
    async fn config_resolved(&self, config: &ferrite_config::ResolvedConfig) -> Result<()> {
        if config.is_production && self.development {
            return Err(FerriteError::Build(format!("{} production pipeline requires development=false when constructing the official compiler adapter", self.name())));
        }
        Ok(())
    }
    fn enforce(&self) -> Enforce {
        Enforce::Pre
    }
    fn compiler_defines(
        &self,
        _: &ferrite_core::Environment,
    ) -> std::collections::HashMap<String, String> {
        if matches!(self.framework, Framework::Vue) {
            std::collections::HashMap::from([
                ("__VUE_OPTIONS_API__".into(), "true".into()),
                ("__VUE_PROD_DEVTOOLS__".into(), "false".into()),
                (
                    "__VUE_PROD_HYDRATION_MISMATCH_DETAILS__".into(),
                    "false".into(),
                ),
            ])
        } else {
            std::collections::HashMap::new()
        }
    }
    fn cache_key(&self) -> String {
        format!(
            "{}:node:{}:development={}:resources-v1",
            self.name(),
            self.host.cache_identity(),
            self.development
        )
    }
    async fn resolve_id(
        &self,
        ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        let mut resolved = ctx.resolver.resolve(&ferrite_resolver::ResolveRequest {
            specifier: request.specifier,
            importer: request.importer,
            environment: request.environment,
            kind: request.kind,
        });
        // Don't intercept unrelated resolution failures or virtual ids.
        if !self.owns(request.specifier) && !resolved.as_ref().is_ok_and(|id| self.owns(&id.id.0)) {
            return Ok(None);
        }
        let resolved = resolved
            .as_mut()
            .map_err(|error| FerriteError::Resolve(error.to_string()))?;
        let (_, query) = resolved.id.split_query();
        resolved.module_type = Some(if query.map(style_query).transpose()?.flatten().is_some() {
            ModuleType::Css
        } else {
            ModuleType::Custom(self.framework_name().into())
        });
        Ok(Some(resolved.clone()))
    }
    async fn load(&self, ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if !self.owns(&request.id) {
            return Ok(None);
        }
        let id = ModuleId::new(&request.id);
        let (path, query) = id.split_query();
        let style = query.map(style_query).transpose()?.flatten();
        if query.is_some_and(|query| !query.is_empty() && style.is_none()) {
            if matches!(query, Some("raw" | "url")) {
                return Ok(None);
            }
            return Err(FerriteError::Build(format!("unsupported {} component query {}; use a component import or a generated style resource", self.framework_name(), request.id)));
        }
        let filename = if path.starts_with("/@npm/") {
            ctx.resolver.npm_url_to_path(path)
        } else if path.starts_with("/@fs/") {
            return Err(FerriteError::Build(format!(
                "linked component {path} requires explicit package/compiler ownership integration"
            )));
        } else {
            ctx.resolver.root.join(path.trim_start_matches('/'))
        };
        if ctx.resolver.root.canonicalize()? != self.host.root() {
            return Err(FerriteError::Build("compiler host belongs to a different project; explicitly construct a host for this pipeline root".into()));
        }
        let source = tokio::fs::read_to_string(&filename).await?;
        let server = request.environment.is_ssr();
        let module = path.ends_with(".svelte.js") || path.ends_with(".svelte.ts");
        let result = self
            .host
            .compile(CompileRequest {
                framework: self.framework,
                filename: filename.clone(),
                source,
                target: if server {
                    CompileTarget::Server
                } else {
                    CompileTarget::Client
                },
                development: self.development && !server,
                module,
            })
            .await?;
        let mut dependencies = result.dependencies;
        dependencies.push(filename.to_string_lossy().into_owned());
        for diagnostic in result.diagnostics {
            ctx.warn(&format!(
                "{}: {} [{}] {}",
                diagnostic.filename, diagnostic.severity, diagnostic.code, diagnostic.message
            ));
        }
        if let Some(index) = style {
            let css = result.css.get(index).ok_or_else(|| {
                FerriteError::Build(format!(
                    "generated style {} no longer exists in {path}; reload the owning component",
                    request.id
                ))
            })?;
            return Ok(Some(LoadResult {
                code: css.code.clone(),
                module_type: ModuleType::Css,
                dependencies,
                map: css
                    .map
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?
                    .map(SourceMap::external),
                side_effects: Some(true),
            }));
        }
        let mut code = result.code;
        if !server {
            for index in 0..result.css.len() {
                let resource = format!("{path}?ferrite-style={index}");
                code.push_str(&format!("import {};\n", serde_json::to_string(&resource)?));
            }
        }
        Ok(Some(LoadResult {
            code,
            module_type: result.module_type,
            dependencies,
            map: result.map,
            side_effects: Some(true),
        }))
    }
}
