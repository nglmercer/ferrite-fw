//! package.json compatibility.

use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::collections::HashMap;
use std::path::Path;

/// Minimal `package.json` dependency view (compatibility).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct JsPackageJson {
    /// Dependencies.
    pub dependencies: HashMap<String, String>,
    /// Dev dependencies.
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: HashMap<String, String>,
    /// Rest of the file, preserved verbatim.
    #[serde(flatten)]
    pub rest: HashMap<String, serde_json::Value>,
}

impl JsPackageJson {
    /// Read (missing → empty).
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(FerriteError::Json)
    }

    /// Write back, preserving unknown fields.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(FerriteError::Json)?;
        std::fs::write(path, format!("{text}\n"))?;
        Ok(())
    }
}
