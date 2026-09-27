//! Registry metadata.

use std::collections::BTreeMap;
use std::collections::HashMap;

/// Registry version metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryVersion {
    /// Package name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Dependencies.
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    /// Peer dependencies.
    #[serde(default, rename = "peerDependencies")]
    pub peer_dependencies: HashMap<String, String>,
    /// Optional dependencies.
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, String>,
    /// Distribution info.
    pub dist: RegistryDist,
    /// OS allowlist.
    pub os: Option<Vec<String>>,
    /// CPU allowlist.
    pub cpu: Option<Vec<String>>,
    /// Native binding metadata (`node-gyp` packages, §88).
    #[serde(default)]
    pub binary: Option<serde_json::Value>,
    /// `binding.gyp` presence flag (native addon, §88).
    #[serde(default)]
    pub gypfile: Option<bool>,
}

/// Registry dist info.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryDist {
    /// Tarball URL.
    pub tarball: String,
    /// Subresource integrity (`sha512-...`).
    pub integrity: Option<String>,
    /// Legacy shasum.
    pub shasum: Option<String>,
}

/// Registry packument (abbreviated).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryMetadata {
    /// Package name.
    pub name: String,
    /// Dist tags.
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    /// Versions.
    #[serde(default)]
    pub versions: BTreeMap<String, RegistryVersion>,
}
