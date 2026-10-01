//! Versioned, concrete package graphs.

use ferrite_core::{FerriteError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const LOCKFILE_VERSION: u32 = 2;
fn legacy_version() -> u32 {
    1
}

/// A package identity is name + exact version, with peer context reserved for
/// contextual instances. Dependency values are identities, never semver ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub source: String,
    pub integrity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tarball: Option<String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    /// Original transitive requests, needed to reuse locked dist-tags correctly.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependency_specifiers: BTreeMap<String, String>,
    /// Concrete peer provider identities. Empty for packages without peers.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peers: BTreeMap<String, String>,
    /// Declared peer ranges retained for validation.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peer_requirements: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_peers: Vec<String>,
}

impl LockedPackage {
    pub fn id(&self) -> String {
        let base = format!("{}@{}", self.name, self.version);
        if self.peers.is_empty() {
            return base;
        }
        let context = serde_json::to_string(&self.peers).expect("string map serializes");
        format!("{base}__peers_{}", ferrite_core::Hash::of_str(&context).0)
    }
}

/// Manifest requests plus concrete root edges for an importing project.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LockedImporter {
    #[serde(default)]
    pub specifiers: BTreeMap<String, String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Lockfile {
    #[serde(default = "legacy_version")]
    pub version: u32,
    #[serde(default)]
    pub importers: BTreeMap<String, LockedImporter>,
    #[serde(default)]
    pub package: Vec<LockedPackage>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            importers: BTreeMap::new(),
            package: Vec::new(),
        }
    }
}

impl Lockfile {
    /// Read and validate. Legacy records migrate in memory; only a successful
    /// non-frozen install rewrites the file. Lost legacy edges cannot be guessed.
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let mut lock: Self = toml::from_str(&text)
            .map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))?;
        match lock.version {
            1 => {
                for package in &mut lock.package {
                    for (name, version) in &mut package.dependencies {
                        *version = format!("{name}@{version}");
                    }
                }
                lock.version = LOCKFILE_VERSION;
                // Roots cannot be inferred from a flat v1 list. The next install
                // reads package.json and records its requests explicitly.
            }
            LOCKFILE_VERSION => (),
            version => return Err(FerriteError::Npm(format!("unsupported ferrite.lock version {version}; this build supports {LOCKFILE_VERSION}"))),
        }
        lock.validate()?;
        Ok(lock)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let mut sorted = self.clone();
        sorted.package.sort_by_key(LockedPackage::id);
        let text = toml::to_string_pretty(&sorted)
            .map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)?;
        // Rename a same-directory temporary file: no partial lock on write failure.
        let mut staged = tempfile::NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut staged, text.as_bytes())?;
        staged.as_file().sync_all()?;
        staged
            .persist(path)
            .map_err(|error| FerriteError::Npm(format!("cannot commit lockfile: {error}")))?;
        Ok(())
    }

    pub fn package_by_id(&self, id: &str) -> Option<&LockedPackage> {
        self.package.iter().find(|package| package.id() == id)
    }

    /// Root edge if recorded, otherwise only an unambiguous package. Never
    /// chooses an arbitrary version when multiple installed versions exist.
    pub fn find(&self, name: &str) -> Option<&LockedPackage> {
        if let Some(id) = self
            .importers
            .get(".")
            .and_then(|root| root.dependencies.get(name))
        {
            return self.package_by_id(id);
        }
        let mut packages = self.package.iter().filter(|package| package.name == name);
        let package = packages.next()?;
        packages.next().is_none().then_some(package)
    }

    pub fn upsert(&mut self, package: LockedPackage) {
        let id = package.id();
        if let Some(existing) = self.package.iter_mut().find(|package| package.id() == id) {
            *existing = package;
        } else {
            self.package.push(package);
        }
    }

    /// Remove all identities and root requests of a package name. Callers that
    /// retain importer edges to it must regenerate their graph before writing.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.package.len();
        self.package.retain(|package| package.name != name);
        for importer in self.importers.values_mut() {
            importer.dependencies.remove(name);
            importer.specifiers.remove(name);
        }
        self.package.len() != before
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != LOCKFILE_VERSION {
            return Err(FerriteError::Npm(
                "lockfile must be migrated before use".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        for package in &self.package {
            validate_package_name(&package.name)?;
            if package.source != "npm" {
                return Err(FerriteError::Npm(format!(
                    "unsupported locked package source `{}` for {}",
                    package.source,
                    package.id()
                )));
            }
            node_semver::Version::parse(&package.version).map_err(|_| {
                FerriteError::Npm(format!(
                    "lockfile has non-concrete version {}@{}",
                    package.name, package.version
                ))
            })?;
            if !ids.insert(package.id()) {
                return Err(FerriteError::Npm(format!(
                    "duplicate package identity {}",
                    package.id()
                )));
            }
        }
        for package in &self.package {
            self.validate_edges(&package.id(), &package.dependencies)?;
            self.validate_edges(&package.id(), &package.peers)?;
        }
        for (name, importer) in &self.importers {
            if name.is_empty()
                || Path::new(name).components().any(|part| {
                    matches!(
                        part,
                        std::path::Component::ParentDir
                            | std::path::Component::RootDir
                            | std::path::Component::Prefix(_)
                    )
                })
            {
                return Err(FerriteError::Npm(format!("unsafe importer path `{name}`")));
            }
            self.validate_edges(name, &importer.dependencies)?;
        }
        Ok(())
    }

    fn validate_edges(&self, importer: &str, edges: &BTreeMap<String, String>) -> Result<()> {
        for (name, id) in edges {
            let target = self.package_by_id(id).ok_or_else(|| FerriteError::Npm(format!("lockfile edge {importer} -> {name} references missing {id}; back up and rename ferrite.lock, then run ferrite install to regenerate it (legacy name collisions cannot be recovered from this lock)")))?;
            if target.name != *name {
                return Err(FerriteError::Npm(format!(
                    "lockfile edge {importer} -> {name} targets {}, not {name}",
                    target.name
                )));
            }
        }
        Ok(())
    }
}

/// Restrict package-derived paths before network or filesystem operations.
pub fn validate_package_name(name: &str) -> Result<()> {
    let valid_part = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    };
    let valid = if let Some(scoped) = name.strip_prefix('@') {
        scoped
            .split_once('/')
            .is_some_and(|(scope, package)| valid_part(scope) && valid_part(package))
    } else {
        valid_part(name)
    };
    if valid {
        Ok(())
    } else {
        Err(FerriteError::Npm(format!(
            "unsafe or unsupported package name `{name}`"
        )))
    }
}
