//! Tier-2 JS plugin host (spec §56–§57).
//!
//! [`JsPluginHost`] runs foreign (JS) plugins inside an embedded
//! [`JsRuntime`](ferrite_runtime::JsRuntime) — e.g. napi-vm with feature
//! `napi-vm` — and exposes the §56 hook bridge (`resolveId`, `load`,
//! `transform`) through the §57 [`ForeignPluginHost`] interface.
//!
//! Guest contract: each hook is an exported function
//! `(<hook-name>)(input) -> output | null | undefined`, where `input` and
//! `output` are plain JSON. Only JSON-serializable values cross the
//! boundary; the host exposes no filesystem, network, or process access.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferrite_core::{FerriteError, Result};
use ferrite_runtime::{
    js_value_to_json, CompiledModule, JsRuntime, JsValue, ModuleNamespace, RuntimeEnvironment,
};

/// Handle to a registered foreign plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginHandle {
    /// Plugin name (registration key).
    pub name: String,
}

impl PluginHandle {
    /// New handle for `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Hook callable through the foreign host (§56 bridge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookName {
    /// `resolveId`.
    ResolveId,
    /// `load`.
    Load,
    /// `transform`.
    Transform,
}

impl HookName {
    /// Guest export implementing the hook.
    #[must_use]
    pub fn export_name(self) -> &'static str {
        match self {
            Self::ResolveId => "resolveId",
            Self::Load => "load",
            Self::Transform => "transform",
        }
    }
}

/// Foreign (non-Rust) plugin host (§57).
#[async_trait::async_trait]
pub trait ForeignPluginHost: Send + Sync {
    /// Call one hook on one plugin: JSON in, JSON out.
    ///
    /// `null` means the plugin does not implement the hook (the caller
    /// skips it, Rollup-style). Unknown plugins and guest throws are
    /// errors, never silent.
    async fn call_hook(
        &self,
        plugin: PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

/// [`ForeignPluginHost`] over an embedded [`JsRuntime`](ferrite_runtime::JsRuntime).
///
/// Register each plugin module once; namespaces are cached and hooks are
/// resolved per call so re-registration picks up new code.
pub struct JsPluginHost {
    runtime: Arc<dyn JsRuntime>,
    plugins: Mutex<HashMap<String, ModuleNamespace>>,
}

impl JsPluginHost {
    /// Host plugins on `runtime`.
    #[must_use]
    pub fn new(runtime: Arc<dyn JsRuntime>) -> Self {
        Self {
            runtime,
            plugins: Mutex::new(HashMap::new()),
        }
    }

    /// Evaluate and register a plugin module under `name`.
    pub async fn register_plugin(&self, name: &str, module: CompiledModule) -> Result<()> {
        let namespace = self
            .runtime
            .evaluate_module(
                module,
                RuntimeEnvironment {
                    ssr: false,
                    request_id: None,
                },
            )
            .await?;
        if let Ok(mut plugins) = self.plugins.lock() {
            plugins.insert(name.to_string(), namespace);
        }
        Ok(())
    }

    /// True when `name` is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.plugins
            .lock()
            .map(|plugins| plugins.contains_key(name))
            .unwrap_or(false)
    }

    /// Backend name of the underlying runtime.
    #[must_use]
    pub fn backend(&self) -> &'static str {
        self.runtime.name()
    }
}

#[async_trait::async_trait]
impl ForeignPluginHost for JsPluginHost {
    async fn call_hook(
        &self,
        plugin: PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let failure = |message: String| FerriteError::Plugin {
            plugin: plugin.name.clone(),
            hook: hook.export_name().to_string(),
            message,
        };
        let handle = {
            let plugins = self
                .plugins
                .lock()
                .map_err(|_| failure("js host lock poisoned".to_string()))?;
            let namespace = plugins
                .get(&plugin.name)
                .ok_or_else(|| failure(format!("unknown js plugin `{}`", plugin.name)))?;
            match namespace.get(hook.export_name()) {
                None => return Ok(serde_json::Value::Null),
                Some(JsValue::Handle(handle)) => handle.clone(),
                Some(_) => {
                    return Err(failure(format!(
                        "export `{}` is not a function",
                        hook.export_name()
                    )));
                }
            }
        };
        let result = self
            .runtime
            .call(&handle, vec![JsValue::from(input)])
            .await?;
        js_value_to_json(&result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_runtime::JsHandle;

    /// Canned runtime: fixed namespace, scripted call results.
    struct StubRuntime {
        namespace: ModuleNamespace,
        result: JsValue,
        seen: Mutex<Vec<JsValue>>,
    }

    #[async_trait::async_trait]
    impl JsRuntime for StubRuntime {
        fn name(&self) -> &'static str {
            "stub"
        }

        async fn evaluate_module(
            &self,
            _module: CompiledModule,
            _env: RuntimeEnvironment,
        ) -> Result<ModuleNamespace> {
            Ok(self.namespace.clone())
        }

        async fn call(&self, _handle: &JsHandle, args: Vec<JsValue>) -> Result<JsValue> {
            if let Ok(mut seen) = self.seen.lock() {
                seen.extend(args);
            }
            Ok(self.result.clone())
        }
    }

    fn stub(result: JsValue) -> StubRuntime {
        let mut namespace = ModuleNamespace::default();
        namespace.exports.insert(
            "transform".to_string(),
            JsValue::Handle(JsHandle { id: 7, label: None }),
        );
        namespace.exports.insert(
            "load".to_string(),
            JsValue::String("not-a-function".to_string()),
        );
        StubRuntime {
            namespace,
            result,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn module() -> CompiledModule {
        CompiledModule {
            id: "plugin".to_string(),
            code: String::new(),
            url: None,
        }
    }

    #[tokio::test]
    async fn missing_hook_returns_null() {
        let host = JsPluginHost::new(Arc::new(stub(JsValue::Null)));
        host.register_plugin("demo", module()).await.unwrap();
        assert!(host.contains("demo"));
        let output = host
            .call_hook(
                PluginHandle::new("demo"),
                HookName::ResolveId,
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(output, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn unknown_plugin_errors() {
        let host = JsPluginHost::new(Arc::new(stub(JsValue::Null)));
        let error = host
            .call_hook(
                PluginHandle::new("ghost"),
                HookName::Transform,
                serde_json::json!({}),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("ghost"), "{error}");
    }

    #[tokio::test]
    async fn non_function_export_errors() {
        let host = JsPluginHost::new(Arc::new(stub(JsValue::Null)));
        host.register_plugin("demo", module()).await.unwrap();
        let error = host
            .call_hook(
                PluginHandle::new("demo"),
                HookName::Load,
                serde_json::json!({}),
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not a function"), "{error}");
    }

    #[tokio::test]
    async fn input_crosses_as_json_value() {
        let runtime = Arc::new(stub(JsValue::String("ok".to_string())));
        let host = JsPluginHost::new(runtime.clone());
        host.register_plugin("demo", module()).await.unwrap();
        let output = host
            .call_hook(
                PluginHandle::new("demo"),
                HookName::Transform,
                serde_json::json!({"id": "a.js"}),
            )
            .await
            .unwrap();
        assert_eq!(output, serde_json::json!("ok"));
        let seen = runtime.seen.lock().unwrap();
        assert!(
            matches!(&seen[..], [JsValue::Object(map)] if matches!(map.get("id"), Some(JsValue::String(id)) if id == "a.js")),
            "{seen:?}"
        );
    }

    #[cfg(feature = "napi-vm")]
    #[tokio::test]
    async fn guest_transform_hook_runs() {
        let host = JsPluginHost::new(Arc::new(
            ferrite_runtime::napi_vm::NapiVmRuntime::with_defaults(),
        ));
        assert_eq!(host.backend(), "napi-vm");
        host.register_plugin(
            "banner",
            CompiledModule {
                id: "banner".to_string(),
                code: "export function transform(input) {\n\
                 return { code: '/*banner*/\\n' + input.code };\n\
                 }\n"
                .to_string(),
                url: None,
            },
        )
        .await
        .unwrap();
        let output = host
            .call_hook(
                PluginHandle::new("banner"),
                HookName::Transform,
                serde_json::json!({"id": "a.js", "code": "const a = 1;\n"}),
            )
            .await
            .unwrap();
        assert_eq!(output["code"], "/*banner*/\nconst a = 1;\n");
        // Unimplemented hook skips cleanly.
        let skipped = host
            .call_hook(
                PluginHandle::new("banner"),
                HookName::ResolveId,
                serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(skipped, serde_json::Value::Null);
    }
}
