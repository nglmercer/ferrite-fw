//! A configured registry keeps hydration imports statically discoverable.
use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};
pub(crate) struct IslandsPlugin {
    code: String,
}
impl IslandsPlugin {
    pub(crate) fn new(config: &ferrite_config::ResolvedConfig) -> Result<Self> {
        config.ssr.validate_routes()?;
        let mut code = "const registry=Object.create(null);\n".to_string();
        for (name, entry) in &config.ssr.islands {
            code.push_str(&format!(
                "registry[{}]=()=>import({});\n",
                serde_json::to_string(name)?,
                serde_json::to_string(&format!("/{entry}"))?
            ));
        }
        code.push_str(ferrite_ssr::island_registry_runtime());
        Ok(Self { code })
    }
}
#[async_trait::async_trait]
impl Plugin for IslandsPlugin {
    fn name(&self) -> &'static str {
        "ferrite:ssr-islands"
    }
    async fn resolve_id(
        &self,
        _: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if !request.ssr
            && matches!(
                request.specifier,
                "/@ferrite/islands-client" | "ferrite:islands"
            )
        {
            Ok(Some(ferrite_resolver::ResolvedId::new(
                "/@ferrite/islands-client",
            )))
        } else {
            Ok(None)
        }
    }
    async fn load(&self, _: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if !request.environment.is_ssr() && request.id == "/@ferrite/islands-client" {
            Ok(Some(LoadResult {
                code: self.code.clone(),
                module_type: ModuleType::Js,
                map: None,
                dependencies: vec![],
                side_effects: None,
            }))
        } else {
            Ok(None)
        }
    }
}
