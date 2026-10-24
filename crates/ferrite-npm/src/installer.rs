//! Package installation services shared by CLI and project generation.

use crate::{
    extract_tarball, range_satisfied, resolve_version, verify_integrity, JsPackageJson,
    LockedImporter, LockedPackage, Lockfile, RegistryClient, RegistryDist, RegistryVersion,
};
use ferrite_core::{FerriteError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct Installer {
    pub client: RegistryClient,
    pub store: PathBuf,
    pub tarball_cache: PathBuf,
}

impl Installer {
    pub fn new(client: RegistryClient, npm_root: PathBuf) -> Self {
        Self {
            client,
            store: npm_root.join("packages"),
            tarball_cache: npm_root.join("tarballs"),
        }
    }

    /// Install one root request transactionally in the in-memory graph.
    pub async fn install(
        &self,
        name: &str,
        range: &str,
        lockfile: &mut Lockfile,
        optional_ok: bool,
    ) -> Result<String> {
        // Reuse the manifest service so adding one root cannot overwrite an
        // ancestor instance still needed by another root.
        let mut manifest = JsPackageJson::default();
        if let Some(root) = lockfile.importers.get(".") {
            manifest.dependencies.extend(root.specifiers.clone());
        }
        manifest.dependencies.insert(name.into(), range.into());
        if let Some(root) = lockfile.importers.get(".") {
            for name in &root.optional_dependencies {
                if let Some(range) = root.specifiers.get(name) {
                    manifest
                        .optional_dependencies
                        .insert(name.clone(), range.clone());
                }
            }
        }
        if optional_ok {
            manifest
                .optional_dependencies
                .insert(name.into(), range.into());
        } else {
            manifest.optional_dependencies.remove(name);
        }
        self.install_manifest(&manifest, lockfile, false).await?;
        Ok(lockfile.find(name).expect("installed root").version.clone())
    }

    /// Resolve and install a manifest, retaining importer-specific concrete
    /// edges. Frozen installs validate requests first, replay exact tarballs,
    /// never consult registry metadata, and never mutate the supplied lock.
    pub async fn install_manifest(
        &self,
        manifest: &JsPackageJson,
        lockfile: &mut Lockfile,
        frozen: bool,
    ) -> Result<()> {
        self.install_manifest_for_platform(
            manifest,
            lockfile,
            frozen,
            &crate::InstallPlatform::current(),
        )
        .await
    }

    pub async fn install_manifest_for_platform(
        &self,
        manifest: &JsPackageJson,
        lockfile: &mut Lockfile,
        frozen: bool,
        platform: &crate::InstallPlatform,
    ) -> Result<()> {
        if manifest.rest.get("workspaces").is_some_and(|value| {
            !value.is_null() && value.as_array().is_none_or(|array| !array.is_empty())
        }) {
            return Err(FerriteError::Npm(
                "workspaces manifests require project-aware installation".into(),
            ));
        }
        for (name, range) in &manifest.dependencies {
            if !manifest.optional_dependencies.contains_key(name)
                && manifest
                    .dev_dependencies
                    .get(name)
                    .is_some_and(|dev| dev != range)
            {
                return Err(FerriteError::Npm(format!(
                    "conflicting dependency and devDependency requests for {name}"
                )));
            }
        }
        if manifest.rest.contains_key("optionalDependencies") {
            return Err(FerriteError::Npm(
                "reserved optionalDependencies must use the typed manifest field".into(),
            ));
        }
        let specifiers = manifest_requests(manifest);
        let optional: Vec<_> = manifest
            .optional_dependencies
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if frozen {
            lockfile.validate()?;
            if !lockfile.portable {
                return Err(FerriteError::Npm(
                    "frozen install needs a portable v3 graph; run a non-frozen install to migrate"
                        .into(),
                ));
            }
            let importer = lockfile.importers.get(".").ok_or_else(|| {
                FerriteError::Npm("frozen install needs a root importer graph".into())
            })?;
            if importer.specifiers != specifiers
                || importer.optional_dependencies != optional
                || importer.dependencies.keys().ne(specifiers.keys())
            {
                return Err(FerriteError::Npm(
                    "frozen install: package.json requests differ from ferrite.lock".into(),
                ));
            }
            self.validate_peers(lockfile)?;
            return self.fetch_active(lockfile, platform).await;
        }
        let mut infos = BTreeMap::new();
        for (name, range) in &specifiers {
            let importer = lockfile.importers.get(".");
            let old = importer.and_then(|importer| importer.dependencies.get(name));
            let old_range = importer.and_then(|importer| importer.specifiers.get(name));
            infos.insert(
                name.clone(),
                self.select(
                    name,
                    range,
                    old.map(String::as_str),
                    old_range.map(String::as_str),
                    lockfile,
                )
                .await?,
            );
        }
        let (package, dependencies) = crate::context_graph::resolve(self, infos, lockfile).await?;
        let updated = Lockfile {
            portable: true,
            package,
            importers: BTreeMap::from([(
                ".".into(),
                LockedImporter {
                    specifiers,
                    dependencies,
                    optional_dependencies: optional,
                },
            )]),
            ..Default::default()
        };
        updated.validate()?;
        self.validate_peers(&updated)?;
        self.fetch_active(&updated, platform).await?;
        *lockfile = updated;
        Ok(())
    }

    pub(crate) async fn fetch_active(
        &self,
        lock: &Lockfile,
        platform: &crate::InstallPlatform,
    ) -> Result<()> {
        let active = lock.active_graph(platform)?;
        for package in lock
            .package
            .iter()
            .filter(|package| active.packages.contains(&package.id()))
        {
            self.fetch_extract(
                package,
                &RegistryDist {
                    tarball: package
                        .tarball
                        .clone()
                        .ok_or_else(|| FerriteError::Npm("locked tarball URL missing".into()))?,
                    integrity: package.integrity.clone(),
                    shasum: None,
                },
            )
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn select(
        &self,
        name: &str,
        range: &str,
        old_id: Option<&str>,
        old_range: Option<&str>,
        lock: &Lockfile,
    ) -> Result<RegistryVersion> {
        let (target, request) = crate::registry_request(name, range)?;
        let old_request = old_range
            .map(|range| crate::registry_request(name, range))
            .transpose()?;
        let name = target.as_str();
        let range = request.as_str();
        let old_range = old_request
            .as_ref()
            .filter(|(target, _)| target == name)
            .map(|(_, range)| range.as_str());
        let metadata = self.client.metadata(name).await?;
        let preferred = old_id
            .and_then(|id| lock.package_by_id(id))
            .filter(|package| {
                package.name == name
                    && (range_satisfied(&package.version, range)
                        || metadata.dist_tags.contains_key(range) && old_range == Some(range))
            });
        let version = if let Some(package) = preferred {
            package.version.clone()
        } else {
            resolve_version(&metadata, range)?
        };
        let info = metadata.versions.get(&version).ok_or_else(|| {
            FerriteError::Npm(format!(
                "locked {name}@{version} is absent from registry metadata"
            ))
        })?;
        if info.name != name || info.version != version {
            return Err(FerriteError::Npm(format!(
                "registry identity mismatch for {name}@{version}"
            )));
        }
        Ok(info.clone())
    }

    pub(crate) fn validate_peers(&self, lock: &Lockfile) -> Result<()> {
        for package in &lock.package {
            for (name, range) in &package.peer_requirements {
                let peer = package
                    .peers
                    .get(name)
                    .and_then(|id| lock.package_by_id(id));
                if let Some(peer) = peer {
                    if !range_satisfied(&peer.version, range) {
                        return Err(FerriteError::Npm(format!(
                            "{} has an incompatible locked peer {name}",
                            package.id()
                        )));
                    }
                } else if !package.optional_peers.contains(name) {
                    return Err(FerriteError::Npm(format!("{} has a missing or unsupported contextual peer provider {name}; install a concrete provider without its own unresolved peer context", package.id())));
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn fetch_extract(
        &self,
        package: &LockedPackage,
        dist: &RegistryDist,
    ) -> Result<()> {
        if package
            .integrity
            .as_deref()
            .is_none_or(|integrity| integrity.trim().is_empty())
        {
            return Err(FerriteError::Npm(format!("{} lacks locked integrity; run a non-frozen install to record registry integrity or shasum", package.id())));
        }
        let dest = self.store.join(package.id());
        if dest.join("package.json").exists() {
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(dest.join("package.json"))?)?;
            if manifest["name"] == package.name && manifest["version"] == package.version {
                return Ok(());
            }
            return Err(FerriteError::Npm(format!("installed identity mismatch at {}; remove the corrupt package directory and reinstall", dest.display())));
        }
        let cached = self.tarball_cache.join(format!(
            "{}-{}.tgz",
            package.name.replace('/', "__"),
            package.version
        ));
        let bytes = if cached.exists() {
            std::fs::read(&cached)?
        } else {
            let bytes = self.client.tarball(&dist.tarball).await?;
            verify_integrity(&bytes, package.integrity.as_deref())?;
            std::fs::create_dir_all(&self.tarball_cache)?;
            std::fs::write(&cached, &bytes)?;
            bytes
        };
        verify_integrity(&bytes, package.integrity.as_deref())?;
        std::fs::create_dir_all(&self.store)?;
        let staged = tempfile::tempdir_in(&self.store)?;
        extract_tarball(&bytes, staged.path())?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(staged.path().join("package.json"))?)?;
        if manifest["name"] != package.name || manifest["version"] != package.version {
            return Err(FerriteError::Npm(format!(
                "tarball identity mismatch for {}",
                package.id()
            )));
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(staged.path(), &dest)?;
        // No lifecycle scripts execute. Compilation hosts are separate explicit profiles.
        Ok(())
    }

    pub fn installed_dir(&self, name: &str, version: &str) -> PathBuf {
        self.store.join(format!("{name}@{version}"))
    }
}

pub fn manifest_requests(manifest: &JsPackageJson) -> BTreeMap<String, String> {
    manifest
        .dependencies
        .iter()
        .chain(manifest.dev_dependencies.iter())
        .chain(manifest.optional_dependencies.iter())
        .map(|(name, range)| (name.clone(), range.clone()))
        .collect()
}
