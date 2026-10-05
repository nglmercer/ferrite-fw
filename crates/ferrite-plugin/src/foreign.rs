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
    dependencies: std::collections::BTreeMap<PathBuf, String>,
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
        let dependencies = host.loaded_dependencies()?;
        let identity = Hash::of_str(
            &serde_json::json!({"registration":identity,"dependencies":dependencies}).to_string(),
        )
        .0;
        Ok(Self {
            host,
            handle: PluginHandle {
                name: name.into(),
                host: "node-adapter".into(),
            },
            identity,
            entry,
            source_hash,
            dependencies,
        })
    }

    fn watched_dependencies(&self, mut dependencies: Vec<String>) -> Vec<String> {
        dependencies.extend(
            self.dependencies
                .keys()
                .filter_map(|path| path.to_str().map(str::to_owned)),
        );
        dependencies.sort();
        dependencies.dedup();
        dependencies
    }

    fn register_watches(&self, ctx: &PluginContext) {
        for path in self.dependencies.keys() {
            if let Some(path) = path.to_str() {
                ctx.add_watch_file(path);
            }
        }
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
    #[serde(alias = "moduleSideEffects")]
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
    #[serde(alias = "moduleSideEffects")]
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
        let dependency_state: Vec<_> = self
            .dependencies
            .keys()
            .map(|path| {
                (
                    path,
                    std::fs::read(path)
                        .map(|source| Hash::of_bytes(&source).0)
                        .unwrap_or_else(|_| "missing-dependency".into()),
                )
            })
            .collect();
        let dependency_state = Hash::of_str(
            &serde_json::to_string(&dependency_state).expect("serializable dependency paths"),
        )
        .0;
        format!(
            "foreign-hooks-v3:{}:{}:{entry_state}:{dependency_state}",
            self.handle.name, self.identity
        )
    }

    async fn resolve_id(
        &self,
        ctx: &PluginContext,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ResolvedId>> {
        self.register_watches(ctx);
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

    async fn load(&self, ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        self.register_watches(ctx);
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
            dependencies: self.watched_dependencies(result.dependencies),
            map: source_map(result.map)?,
            side_effects: result.side_effects,
        }))
    }

    async fn transform(
        &self,
        ctx: &PluginContext,
        request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        self.register_watches(ctx);
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
            dependencies: self.watched_dependencies(result.dependencies),
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

    #[tokio::test]
    #[ignore = "requires real Node; executed explicitly"]
    async fn real_node_official_side_effects_and_importer_context() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("metadata.mjs");
        std::fs::write(&entry, "export default {resolveId(id, importer, options) { return {id, moduleSideEffects: id.endsWith('true'), meta: {importer, ssr: options.ssr}}; }, load(id) { return {code: 'export default 42;', moduleSideEffects: id.endsWith('true')}; }, transform(code) { return {code, moduleSideEffects: false}; }};").unwrap();
        let host = Arc::new(NodeAdapterHost::spawn(None).unwrap());
        let plugin =
            ForeignHookPlugin::register(host.clone(), "metadata", &entry, Value::Null).unwrap();
        let graph = ferrite_graph::ModuleGraph::new();
        let emitted = std::sync::Mutex::new(std::collections::HashMap::new());
        let watches = std::sync::Mutex::new(Vec::new());
        let warnings = std::sync::Mutex::new(Vec::new());
        let importer = ferrite_core::ModuleId::new("/entry.js");
        for kind in [
            ferrite_core::EnvironmentKind::Client,
            ferrite_core::EnvironmentKind::Ssr,
        ] {
            let resolver = ferrite_resolver::Resolver::for_environment(
                dir.path().into(),
                &ferrite_config::ResolveConfig::default(),
                &kind,
            );
            let environment = ferrite_core::Environment::new("fixture", kind.clone());
            let context = PluginContext {
                graph: &graph,
                resolver: &resolver,
                environment: &environment,
                emitted: &emitted,
                watch_files: &watches,
                warnings: &warnings,
            };
            for flag in [false, true] {
                let specifier = format!("/module-{flag}");
                let resolved = plugin
                    .resolve_id(
                        &context,
                        ResolveHookRequest {
                            kind: ferrite_resolver::ResolveKind::Import,
                            specifier: &specifier,
                            importer: Some(&importer),
                            environment: kind.clone(),
                            ssr: environment.kind.is_ssr(),
                        },
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(resolved.side_effects, Some(flag));
                assert_eq!(resolved.meta["importer"], "/entry.js");
                assert_eq!(resolved.meta["ssr"], environment.kind.is_ssr());
                let loaded = plugin
                    .load(
                        &context,
                        LoadRequest {
                            id: specifier,
                            environment: kind.clone(),
                        },
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(loaded.side_effects, Some(flag));
                assert_eq!(loaded.code, "export default 42;");
                assert!(loaded
                    .dependencies
                    .iter()
                    .any(|path| std::path::Path::new(path) == entry.canonicalize().unwrap()));
            }
        }
        host.shutdown();
    }

    #[test]
    fn official_module_side_effects_preserve_boolean_metadata() {
        for field in ["moduleSideEffects", "sideEffects"] {
            for flag in [false, true] {
                let mut resolution = serde_json::json!({"id": "/module.js"});
                resolution[field] = flag.into();
                assert_eq!(
                    decode::<Resolution>(resolution, "resolveId")
                        .unwrap()
                        .side_effects,
                    Some(flag)
                );
                let mut loaded = serde_json::json!({"code": "export default 42;"});
                loaded[field] = flag.into();
                assert_eq!(
                    code_result(loaded, "load").unwrap().side_effects,
                    Some(flag)
                );
            }
        }
        assert!(decode::<Resolution>(
            serde_json::json!({"id": "/module.js", "moduleSideEffects": "no-treeshake"}),
            "resolveId"
        )
        .is_err());
        assert!(decode::<Resolution>(serde_json::json!({"id": "/module.js", "moduleSideEffects": false, "sideEffects": true}), "resolveId").is_err());
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
