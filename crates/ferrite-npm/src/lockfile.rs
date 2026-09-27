//! Lockfiles.

use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::collections::BTreeMap;
use std::path::Path;

/// A locked package (§81).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    /// Package name.
    pub name: String,
    /// Locked version.
    pub version: String,
    /// Source (`npm`).
    pub source: String,
    /// Integrity hash.
    pub integrity: Option<String>,
    /// Locked dependencies (`name` → `version`).
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

/// `ferrite.lock` (§81; npm packages only — `Cargo.lock` owns Rust).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Lockfile {
    /// Locked packages.
    #[serde(default)]
    pub package: Vec<LockedPackage>,
}

impl Lockfile {
    /// Read a lockfile (missing file → empty).
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))
    }

    /// Write a lockfile (sorted for stable diffs).
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut sorted = self.clone();
        sorted
            .package
            .sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
        let text = toml::to_string_pretty(&sorted)
            .map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Find a locked package.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&LockedPackage> {
        self.package.iter().find(|package| package.name == name)
    }

    /// Insert or replace a locked package.
    pub fn upsert(&mut self, package: LockedPackage) {
        if let Some(existing) = self.package.iter_mut().find(|p| p.name == package.name) {
            *existing = package;
        } else {
            self.package.push(package);
        }
    }

    /// Remove a locked package. Returns true when removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.package.len();
        self.package.retain(|package| package.name != name);
        self.package.len() != before
    }
}
