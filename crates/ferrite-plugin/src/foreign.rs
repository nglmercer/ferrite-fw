//! Validated JSON hook adapters for the shared native plugin pipeline.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use ferrite_core::{FerriteError, Hash, ModuleType, Result, SourceMap};
use ferrite_resolver::ResolvedId;
use serde::Deserialize;
use serde_json::Value;

use crate::{
    node_adapter::NodeAdapterHost, HookName, LoadRequest, LoadResult, Plugin, PluginContext,
    PluginHandle, ResolveHookRequest, TransformRequest, TransformResult,
};

/// Explicit Node hook profile participating in the native pipeline. Registration
/// validates the factory contract; this adapter never starts a Node fallback.
pub struct ForeignHookPlugin {
    host: Arc<NodeAdapterHost>,
    handle: PluginHandle,
    identity: String,
    entry: PathBuf,
    source_hash: Hash,
}

impl ForeignHookPlugin {
    /// Register one local plugin module on an explicitly constructed host.
    pub fn register(
        host: Arc<NodeAdapterHost>,
        name: &str,
        entry: &Path,
        options: Value,
    ) -> Result<Self> {
        let entry = entry.canonicalize()?;
        let source = std::fs::read(&entry)?;
        let host_identity = host.cache_identity()?;
        let identity = Hash::of_str(
            &serde_json::json!({"entry": entry, "source": source, "options": options, "host":host_identity}).to_string(),
        )
        .0;
        let source_hash = Hash::of_bytes(&source);
        let path = entry.to_str().ok_or_else(|| {
            FerriteError::Build("foreign plugin entry must be a UTF-8 path".into())
        })?;
        host.register_hook_plugin(name, path, options)?;
        Ok(Self {
            host,
            handle: PluginHandle {
                name: name.into(),
                host: "node-adapter".into(),
            },
            identity,
            entry,
            source_hash,
        })
    }

    async fn call(&self, hook: HookName, input: Value) -> Result<Value> {
        if Hash::of_bytes(&std::fs::read(&self.entry)?) != self.source_hash {
            return Err(FerriteError::Build(format!("foreign plugin {} entry changed; recreate its explicit Node host and plugin registration before compiling", self.handle.name)));
        }
        crate::ForeignPluginHost::call_hook(self.host.as_ref(), &self.handle, hook, input).await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Resolution {
    id: String,
    #[serde(default)]
    external: bool,
    side_effects: Option<bool>,
    module_type: Option<ModuleType>,
    #[serde(default)]
    meta: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CodeResult {
    code: String,
    #[serde(default)]
    map: Value,
    #[serde(default)]
    dependencies: Vec<String>,
    module_type: Option<ModuleType>,
    side_effects: Option<bool>,
}

fn decode<T: serde::de::DeserializeOwned>(value: Value, hook: &str) -> Result<T> {
    serde_json::from_value(value).map_err(|error| {
        FerriteError::Build(format!(
            "foreign {hook} returned unsupported result: {error}"
        ))
    })
}

fn code_result(value: Value, hook: &str) -> Result<CodeResult> {
    if let Value::String(code) = value {
        return Ok(CodeResult {
            code,
            map: Value::Null,
            dependencies: vec![],
            module_type: None,
            side_effects: None,
        });
    }
    decode(value, hook)
}

fn source_map(value: Value) -> Result<Option<SourceMap>> {
    if value.is_null() {
        return Ok(None);
    }
    let json = match value {
        Value::String(json) => json,
        Value::Object(_) => value.to_string(),
        _ => {
            return Err(FerriteError::Build(
                "foreign source map must be v3 JSON or null".into(),
            ))
        }
    };
    oxc_sourcemap::SourceMap::from_json_string(&json)
        .map_err(|error| FerriteError::Build(format!("foreign source map is invalid: {error}")))?;
    Ok(Some(SourceMap::external(json)))
}

#[async_trait::async_trait]
impl Plugin for ForeignHookPlugin {
    fn name(&self) -> &'static str {
        "ferrite:foreign-hook"
    }
    fn cache_key(&self) -> String {
        let entry_state = std::fs::read(&self.entry)
            .map(|source| Hash::of_bytes(&source).0)
            .unwrap_or_else(|_| "missing-entry".into());
        format!(
            "foreign-hooks-v2:{}:{}:{entry_state}",
            self.handle.name, self.identity
        )
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ResolvedId>> {
        let value = self.call(HookName::ResolveId, serde_json::json!({"id": request.specifier, "importer": request.importer.map(|id| id.0.as_str()), "options": {"ssr": request.ssr, "environment": request.environment, "kind": format!("{:?}", request.kind)}})).await?;
        if value.is_null() {
            return Ok(None);
        }
        if let Value::String(id) = value {
            return Ok(Some(ResolvedId::new(id)));
        }
        let result: Resolution = decode(value, "resolveId")?;
        Ok(Some(ResolvedId {
            id: ferrite_core::ModuleId::new(result.id),
            external: result.external,
            side_effects: result.side_effects,
            module_type: result.module_type,
            meta: result.meta,
        }))
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        let value = self.call(HookName::Load, serde_json::json!({"id": request.id, "options": {"ssr": request.environment.is_ssr(), "environment": request.environment}})).await?;
        if value.is_null() {
            return Ok(None);
        }
        let result = code_result(value, "load")?;
        Ok(Some(LoadResult {
            code: result.code,
            module_type: result
                .module_type
                .unwrap_or_else(|| ModuleType::from_path(&request.id)),
            dependencies: result.dependencies,
            map: source_map(result.map)?,
            side_effects: result.side_effects,
        }))
    }

    async fn transform(
        &self,
        _ctx: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        let value = self.call(HookName::Transform, serde_json::json!({"id": request.id, "code": request.code, "options": {"ssr": request.ssr, "environment": request.environment, "moduleType": request.module_type}})).await?;
        if value.is_null() {
            return Ok(None);
        }
        let result = code_result(value, "transform")?;
        if result.side_effects.is_some() {
            return Err(FerriteError::Build("foreign transform sideEffects is unsupported; return it from resolveId/load or use the native plugin API".into()));
        }
        Ok(Some(TransformResult {
            code: result.code,
            map: source_map(result.map)?,
            dependencies: result.dependencies,
            module_type: result.module_type,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_results_reject_metadata_that_cannot_be_preserved() {
        assert!(code_result(serde_json::json!({"code":"x", "css":"lost"}), "transform").is_err());
        assert!(decode::<Resolution>(
            serde_json::json!({"id":"/x.js", "syntheticNamedExports":true}),
            "resolveId"
        )
        .is_err());
        assert!(code_result(Value::Bool(false), "load").is_err());
        assert!(code_result(serde_json::json!({"map":null}), "transform").is_err());
        assert_eq!(
            code_result(Value::String("source".into()), "load")
                .unwrap()
                .code,
            "source"
        );
    }

    #[test]
    fn maps_accept_objects_and_json_and_reject_invalid_data() {
        let map = serde_json::json!({"version":3,"sources":["source.js"],"sourcesContent":["source"],"names":[],"mappings":"AAAA"});
        let object = source_map(map.clone()).unwrap().unwrap();
        let string = source_map(Value::String(map.to_string())).unwrap().unwrap();
        assert_eq!(object.mappings, string.mappings);
        assert!(!object.inline);
        assert!(source_map(Value::Null).unwrap().is_none());
        assert!(source_map(Value::Bool(false)).is_err());
        assert!(source_map(Value::String("not JSON".into())).is_err());
    }
}
