//! Production manifest schemas (spec §40).
//!
//! Vite-compatible client manifest plus the SSR module→chunk map.

use std::collections::HashMap;
use std::path::Path;

use ferrite_core::Result;

/// One entry in the client manifest.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    /// Output file.
    pub file: String,
    /// Original source (for entries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// True for entries.
    #[serde(skip_serializing_if = "Option::is_none", rename = "isEntry")]
    pub is_entry: Option<bool>,
    /// True for dynamic entries.
    #[serde(skip_serializing_if = "Option::is_none", rename = "isDynamicEntry")]
    pub is_dynamic_entry: Option<bool>,
    /// Associated CSS files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<String>,
    /// Static chunk imports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<String>,
    /// Dynamic chunk imports.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "dynamicImports"
    )]
    pub dynamic_imports: Vec<String>,
    /// Static assets referenced by this chunk.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<String>,
}

/// Client manifest: source → hashed output.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BuildManifest {
    /// Entries by source key.
    #[serde(flatten)]
    pub entries: HashMap<String, ManifestEntry>,
}

impl BuildManifest {
    /// Write `manifest.json`.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(ferrite_core::FerriteError::Json)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Read `manifest.json`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(ferrite_core::FerriteError::Json)
    }
}

/// SSR manifest: source module → output chunks (§40).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SsrManifest {
    /// Module → chunk files.
    #[serde(flatten)]
    pub entries: HashMap<String, Vec<String>>,
}

impl SsrManifest {
    /// Write `ssr-manifest.json`.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(ferrite_core::FerriteError::Json)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Read `ssr-manifest.json`.
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(ferrite_core::FerriteError::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_shape_matches_vite() {
        let manifest = BuildManifest {
            entries: HashMap::from([(
                "src/main.ts".to_string(),
                ManifestEntry {
                    file: "assets/main-b13a9.js".to_string(),
                    src: Some("src/main.ts".to_string()),
                    is_entry: Some(true),
                    is_dynamic_entry: None,
                    css: vec!["assets/main-113aa.css".to_string()],
                    imports: vec!["assets/vendor-fa342.js".to_string()],
                    dynamic_imports: Vec::new(),
                    assets: Vec::new(),
                },
            )]),
        };
        let text = serde_json::to_string(&manifest).unwrap();
        assert!(text.contains("isEntry"));
        assert!(text.contains("assets/main-b13a9.js"));
    }
}
