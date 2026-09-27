//! Package installer.

use crate::client::*;
use crate::lockfile::*;
use crate::metadata::*;
use crate::package::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Installer: resolves, downloads, verifies, and extracts packages.
pub struct Installer {
    /// Registry client.
    pub client: RegistryClient,
    /// Install root (`.ferrite/npm/packages`).
    pub store: PathBuf,
    /// Tarball cache.
    pub tarball_cache: PathBuf,
}

impl Installer {
    /// Create an installer rooted at `project_root/.ferrite/npm`.
    pub fn new(client: RegistryClient, npm_root: PathBuf) -> Self {
        Self {
            client,
            store: npm_root.join("packages"),
            tarball_cache: npm_root.join("tarballs"),
        }
    }

    /// Install `name@range` and its dependencies, updating `lockfile`.
    /// Returns the locked version.
    pub async fn install(
        &self,
        name: &str,
        range: &str,
        lockfile: &mut Lockfile,
        optional_ok: bool,
    ) -> Result<String> {
        let mut visiting = Vec::new();
        self.install_inner(
            name.to_string(),
            range.to_string(),
            lockfile,
            &mut visiting,
            optional_ok,
        )
        .await
    }

    /// Recursive installer (boxed: async recursion).
    #[allow(clippy::too_many_lines)]
    fn install_inner<'a>(
        &'a self,
        name: String,
        range: String,
        lockfile: &'a mut Lockfile,
        visiting: &'a mut Vec<String>,
        optional_ok: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            if visiting.contains(&name) {
                return Ok(range); // cycle: keep the requested range
            }
            // Reuse the lock when it still satisfies the range.
            if let Some(locked) = lockfile.find(&name) {
                if range_satisfied(&locked.version, &range)
                    && self.installed_dir(&name, &locked.version).exists()
                {
                    return Ok(locked.version.clone());
                }
            }
            visiting.push(name.clone());
            let metadata = self.client.metadata(&name).await?;
            let version = resolve_version(&metadata, &range)?;
            let info = metadata.versions.get(&version).ok_or_else(|| {
                FerriteError::Npm(format!("version `{version}` of `{name}` disappeared"))
            })?;
            // Platform filter (§16 OS/CPU filtering): skip mismatched optional deps.
            if !platform_matches(&info.os, &info.cpu) {
                visiting.pop();
                if optional_ok {
                    return Ok(version);
                }
                return Err(FerriteError::Npm(format!(
                    "`{name}@{version}` does not support this platform"
                )));
            }
            // Native addons cannot run in the embedded runtime (§88).
            if info.binary.is_some() || info.gypfile == Some(true) {
                tracing::warn!(
                    "native Node addon detected: `{name}@{version}` ships `.node` binaries; \
                     use a Rust-native replacement, a WASM alternative, or the Node \
                     compatibility host"
                );
            }
            self.fetch_extract(&name, &version, &info.dist).await?;
            let dependencies: Vec<(String, String)> = info
                .dependencies
                .iter()
                .map(|(dep, dep_range)| (dep.clone(), dep_range.clone()))
                .collect();
            let mut locked_deps = BTreeMap::new();
            for (dep, dep_range) in dependencies {
                match self
                    .install_inner(
                        dep.clone(),
                        dep_range,
                        &mut *lockfile,
                        &mut *visiting,
                        false,
                    )
                    .await
                {
                    Ok(locked) => {
                        locked_deps.insert(dep, locked);
                    }
                    Err(error) => {
                        visiting.pop();
                        return Err(error);
                    }
                }
            }
            // Optional deps: best effort, never fatal.
            let optional: Vec<(String, String)> = info
                .optional_dependencies
                .iter()
                .map(|(dep, dep_range)| (dep.clone(), dep_range.clone()))
                .collect();
            for (dep, dep_range) in optional {
                if let Ok(locked) = self
                    .install_inner(dep.clone(), dep_range, &mut *lockfile, &mut *visiting, true)
                    .await
                {
                    locked_deps.insert(dep, locked);
                }
            }
            // Peer deps: warn when missing (recording them as locked
            // dependencies would be wrong, so only warn here).
            for (peer, peer_range) in &info.peer_dependencies {
                if lockfile.find(peer).is_none() {
                    tracing::warn!(
                        "`{name}@{version}` wants peer `{peer}@{peer_range}` (not installed)"
                    );
                }
            }
            lockfile.upsert(LockedPackage {
                name: name.clone(),
                version: version.clone(),
                source: "npm".to_string(),
                integrity: info.dist.integrity.clone(),
                dependencies: locked_deps,
            });
            visiting.pop();
            Ok(version)
        })
    }

    /// Download (cached), verify, and extract one tarball.
    async fn fetch_extract(&self, name: &str, version: &str, dist: &RegistryDist) -> Result<()> {
        let dest = self.installed_dir(name, version);
        if dest.join("package.json").exists() {
            return Ok(());
        }
        let cached = self
            .tarball_cache
            .join(format!("{name}-{version}.tgz").replace('/', "__"));
        let bytes = if cached.exists() {
            std::fs::read(&cached)?
        } else {
            let bytes = self.client.tarball(&dist.tarball).await?;
            verify_integrity(&bytes, dist.integrity.as_deref())?;
            if let Some(parent) = cached.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cached, &bytes);
            bytes
        };
        verify_integrity(&bytes, dist.integrity.as_deref())?;
        let _ = std::fs::remove_dir_all(&dest);
        extract_tarball(&bytes, &dest)?;
        Ok(())
    }

    /// Installed directory for `name@version`.
    #[must_use]
    pub fn installed_dir(&self, name: &str, version: &str) -> PathBuf {
        self.store.join(format!("{name}@{version}"))
    }
}

/// True when `version` satisfies `range` (or range is a tag/unknown).
fn range_satisfied(version: &str, range: &str) -> bool {
    if version == range {
        return true;
    }
    let (Ok(requirement), Ok(parsed)) = (
        semver::VersionReq::parse(range),
        semver::Version::parse(version),
    ) else {
        return true; // tags: assume satisfied (re-resolve on `ferrite update`)
    };
    requirement.matches(&parsed)
}

/// True when the current platform matches `os`/`cpu` allowlists.
fn platform_matches(os: &Option<Vec<String>>, cpu: &Option<Vec<String>>) -> bool {
    if let Some(list) = os {
        let current = std::env::consts::OS;
        if !list.iter().any(|entry| entry == current || entry == "any") {
            return false;
        }
    }
    if let Some(list) = cpu {
        let current = std::env::consts::ARCH;
        if !list.iter().any(|entry| entry == current || entry == "any") {
            return false;
        }
    }
    true
}
