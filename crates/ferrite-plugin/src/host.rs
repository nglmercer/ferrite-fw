//! Foreign plugin host.

use ferrite_core::Result;

/// Handle to a foreign (JS/Node-hosted) plugin (§57).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PluginHandle {
    /// Plugin name.
    pub name: String,
    /// Host that owns it (`embedded-js`, `node-adapter`).
    pub host: String,
}

/// A hook invocation on a foreign plugin host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HookName {
    /// `resolveId`.
    ResolveId,
    /// `load`.
    Load,
    /// `transform`.
    Transform,
    /// `transformIndexHtml`.
    TransformIndexHtml,
    /// `handleHotUpdate`.
    HandleHotUpdate,
    /// `generateBundle`.
    GenerateBundle,
}

/// Foreign plugin host: embedded JS runtime or Node adapter (§56–§57).
///
/// Tier-2/3 compatibility surface. The embedded runtime exposes only this
/// narrow JSON bridge — never unrestricted filesystem or memory access.
#[async_trait::async_trait]
pub trait ForeignPluginHost: Send + Sync {
    /// Host name.
    fn name(&self) -> &'static str;

    /// Call one hook on one foreign plugin.
    async fn call_hook(
        &self,
        plugin: &PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

/// True for internal (`\0`-prefixed) virtual ids (§14).
#[must_use]
pub fn is_virtual_id(id: &str) -> bool {
    id.starts_with('\0')
}

/// Convert a user-facing `virtual:*` specifier to its internal id.
#[must_use]
pub fn to_virtual_id(specifier: &str) -> String {
    if let Some(rest) = specifier.strip_prefix("virtual:") {
        format!("\0virtual:{rest}")
    } else {
        specifier.to_string()
    }
}
