//! npm package manifests.

use std::collections::HashMap;

/// Default registry.
pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";

/// Package runtime classification (§17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PackageRuntime {
    /// Browser-first ESM.
    BrowserEsm,
    /// Browser-compatible CJS (convertible).
    BrowserCjs,
    /// Universal (works in browser + SSR).
    Universal,
    /// Usable in SSR with shims.
    NodeCompatible,
    /// Requires real Node.js (native addons, `node:*` internals).
    NodeRequired,
    /// Build-time only (never shipped to the browser).
    BuildTimeOnly,
}

/// Classify a package from its manifest.
#[must_use]
pub fn classify_package(manifest: &PackageManifest) -> PackageRuntime {
    let has_browser = manifest.browser.is_some();
    let is_module = manifest.package_type.as_deref() == Some("module") || manifest.module.is_some();
    let has_exports = manifest.exports.is_some();
    if manifest.has_native_binding() {
        return PackageRuntime::NodeRequired;
    }
    if manifest.requires_node_core() {
        return PackageRuntime::NodeCompatible;
    }
    match (
        has_browser || is_module || has_exports,
        manifest.main.is_some(),
    ) {
        (true, true) => PackageRuntime::Universal,
        (true, false) => PackageRuntime::BrowserEsm,
        (false, true) => PackageRuntime::BrowserCjs,
        (false, false) => PackageRuntime::BuildTimeOnly,
    }
}

/// Installed/registry package manifest (superset of resolver's view).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PackageManifest {
    /// Package name.
    pub name: Option<String>,
    /// Package version.
    pub version: Option<String>,
    /// CJS entry.
    pub main: Option<String>,
    /// ESM entry.
    pub module: Option<String>,
    /// Browser override.
    pub browser: Option<serde_json::Value>,
    /// Export map.
    pub exports: Option<serde_json::Value>,
    /// Package type (`module`/`commonjs`).
    #[serde(rename = "type")]
    pub package_type: Option<String>,
    /// Dependencies.
    pub dependencies: HashMap<String, String>,
    /// Peer dependencies.
    #[serde(rename = "peerDependencies")]
    pub peer_dependencies: HashMap<String, String>,
    /// Optional dependencies.
    #[serde(rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, String>,
    /// OS allowlist.
    pub os: Option<Vec<String>>,
    /// CPU allowlist.
    pub cpu: Option<Vec<String>>,
    /// Native binding hints (`binary`, `gypfile`).
    pub binary: Option<serde_json::Value>,
    /// `gypfile` presence indicates a native addon.
    pub gypfile: Option<String>,
}

impl PackageManifest {
    /// Heuristic: native `.node` binding required (§88).
    #[must_use]
    pub fn has_native_binding(&self) -> bool {
        self.gypfile.is_some() || self.binary.is_some()
    }

    /// Heuristic: depends on known server-only packages.
    #[must_use]
    pub fn requires_node_core(&self) -> bool {
        self.dependencies.keys().any(|name| {
            matches!(
                name.as_str(),
                "node-gyp" | "fsevents" | "sharp" | "esbuild" | "workerd"
            )
        })
    }
}
