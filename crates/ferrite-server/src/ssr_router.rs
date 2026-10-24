//! Lazy route dispatch is an ordinary compiled module, shared by every SSR host.
use ferrite_config::{ResolvedConfig, SSR_ROUTER_ENTRY};
use ferrite_core::{ModuleType, Result};
use ferrite_plugin::{LoadRequest, LoadResult, Plugin, PluginContext, ResolveHookRequest};

pub(crate) struct SsrRouterPlugin {
    code: String,
}
impl SsrRouterPlugin {
    pub(crate) fn new(config: &ResolvedConfig) -> Result<Self> {
        ferrite_config::resolve_js_server_entry(config)?;
        let mut fallback_config = config.clone();
        fallback_config.ssr.entries.clear();
        let fallback = ferrite_config::resolve_js_server_entry(&fallback_config)?;
        let mut entries: Vec<_> = config.ssr.entries.iter().collect();
        entries.sort_by_key(|entry| {
            let rank = if entry.route == "*" {
                2
            } else if entry.route.ends_with("/*") {
                1
            } else {
                0
            };
            (rank, std::cmp::Reverse(entry.route.len()), &entry.route)
        });
        let base = format!("{}/", config.base.trim_end_matches('/'));
        let not_found = if config.ssr.streaming {
            "{body:new ReadableStream({start(c){c.enqueue('Not Found');c.close();}}),status:404}"
        } else {
            "{html:'Not Found',status:404}"
        };
        let mut code = format!("export async function render(url, request) {{\nconst full = url.split('?')[0];\nconst base = {};\nif (!full.startsWith(base)) return {not_found};\nconst path = '/' + full.slice(base.length);\n", serde_json::to_string(&base)?);
        for entry in entries {
            let pattern = serde_json::to_string(&entry.route)?;
            let condition = if entry.route == "*" {
                "true".into()
            } else if let Some(prefix) = entry.route.strip_suffix("/*") {
                format!(
                    "path === {} || path.startsWith({})",
                    serde_json::to_string(prefix)?,
                    serde_json::to_string(&format!("{prefix}/"))?
                )
            } else {
                format!("path === {pattern}")
            };
            code.push_str(&format!(
                "if ({condition}) return (await import({})).render(url, request);\n",
                serde_json::to_string(&format!("/{}", entry.entry))?
            ));
        }
        if let Some(fallback) = fallback {
            code.push_str(&format!(
                "return (await import({})).render(url, request);\n",
                serde_json::to_string(&format!("/{fallback}"))?
            ));
        } else {
            code.push_str(&format!("return {not_found};\n"));
        }
        code.push_str("}\n");
        Ok(Self { code })
    }
}
#[async_trait::async_trait]
impl Plugin for SsrRouterPlugin {
    fn name(&self) -> &'static str {
        "ferrite:ssr-route-dispatch"
    }
    async fn resolve_id(
        &self,
        _: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ferrite_resolver::ResolvedId>> {
        if request.ssr
            && [
                format!("/{SSR_ROUTER_ENTRY}"),
                format!("\0{SSR_ROUTER_ENTRY}"),
            ]
            .iter()
            .any(|id| id == request.specifier)
        {
            Ok(Some(ferrite_resolver::ResolvedId::new(format!(
                "\0{SSR_ROUTER_ENTRY}"
            ))))
        } else {
            Ok(None)
        }
    }
    async fn load(&self, _: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if request.environment.is_ssr() && request.id.as_str() == format!("\0{SSR_ROUTER_ENTRY}") {
            Ok(Some(LoadResult {
                code: self.code.clone(),
                module_type: ModuleType::Js,
                map: None,
                dependencies: Vec::new(),
                side_effects: None,
            }))
        } else {
            Ok(None)
        }
    }
}
