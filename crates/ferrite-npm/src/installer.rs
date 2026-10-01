//! Package installation services shared by CLI and project generation.

use crate::{
    extract_tarball, range_satisfied, resolve_version, validate_package_name, verify_integrity,
    JsPackageJson, LockedImporter, LockedPackage, Lockfile, RegistryClient, RegistryDist,
    RegistryVersion,
};
use ferrite_core::{FerriteError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub struct Installer {
    pub client: RegistryClient,
    pub store: PathBuf,
    pub tarball_cache: PathBuf,
}

#[derive(Clone)]
struct Provider {
    version: String,
    id: String,
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
        if optional_ok {
            let info = self.select(name, range, None, None, lockfile).await?;
            if !platform_matches(&info.os, &info.cpu) {
                return Err(FerriteError::Npm(format!(
                    "optional root `{name}` is unavailable on this platform"
                )));
            }
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
        for field in ["workspaces", "optionalDependencies"] {
            if manifest.rest.get(field).is_some_and(|value| {
                !value.is_null()
                    && value.as_object().is_none_or(|map| !map.is_empty())
                    && value.as_array().is_none_or(|array| !array.is_empty())
            }) {
                return Err(FerriteError::Npm(format!("root {field} installation is unavailable in this graph profile; this field cannot be silently omitted")));
            }
        }
        for (name, range) in &manifest.dependencies {
            if manifest
                .dev_dependencies
                .get(name)
                .is_some_and(|dev| dev != range)
            {
                return Err(FerriteError::Npm(format!("conflicting dependency and devDependency requests for {name}; each importer must select one root version")));
            }
        }
        let specifiers = manifest_requests(manifest);
        if frozen {
            lockfile.validate()?;
            let importer = lockfile.importers.get(".").ok_or_else(|| FerriteError::Npm("frozen install needs a v2 importer graph; run a non-frozen install to migrate ferrite.lock".into()))?;
            if importer.specifiers != specifiers {
                return Err(FerriteError::Npm("frozen install: package.json requests differ from ferrite.lock; run ferrite install without --frozen-lockfile and review the lock change".into()));
            }
            if importer.dependencies.keys().ne(specifiers.keys()) {
                return Err(FerriteError::Npm(
                    "frozen install: root dependency edges are incomplete".into(),
                ));
            }
            self.validate_peers(lockfile)?;
            for package in &lockfile.package {
                let tarball = package.tarball.clone().ok_or_else(|| FerriteError::Npm(format!("frozen install: {} lacks a locked tarball URL; run a non-frozen install to migrate", package.id())))?;
                self.fetch_extract(
                    package,
                    &RegistryDist {
                        tarball,
                        integrity: package.integrity.clone(),
                        shasum: None,
                    },
                )
                .await?;
            }
            return Ok(());
        }
        let mut updated = lockfile.clone();
        let mut infos = BTreeMap::new();
        let mut providers = BTreeMap::new();
        for (name, range) in &specifiers {
            let old = updated
                .importers
                .get(".")
                .and_then(|root| root.dependencies.get(name))
                .cloned()
                .or_else(|| {
                    updated
                        .importers
                        .is_empty()
                        .then(|| updated.find(name).map(LockedPackage::id))
                        .flatten()
                });
            let info = self
                .select(
                    name,
                    range,
                    old.as_deref(),
                    updated
                        .importers
                        .get(".")
                        .and_then(|root| root.specifiers.get(name))
                        .map(String::as_str),
                    &updated,
                )
                .await?;
            providers.insert(
                name.clone(),
                Provider {
                    version: info.version.clone(),
                    id: format!("{name}@{}", info.version),
                },
            );
            infos.insert(name.clone(), info);
        }
        let mut dependencies = BTreeMap::new();
        let mut completed = BTreeMap::new();
        for (name, info) in infos {
            let id = self
                .install_inner(
                    info,
                    &mut updated,
                    &mut BTreeSet::new(),
                    &mut completed,
                    &providers,
                    false,
                )
                .await?
                .expect("required root");
            dependencies.insert(name, id);
        }
        updated.importers.insert(
            ".".into(),
            LockedImporter {
                specifiers,
                dependencies,
            },
        );
        prune_unreachable(&mut updated);
        updated.validate()?;
        self.validate_peers(&updated)?;
        *lockfile = updated;
        Ok(())
    }

    async fn select(
        &self,
        name: &str,
        range: &str,
        old_id: Option<&str>,
        old_range: Option<&str>,
        lock: &Lockfile,
    ) -> Result<RegistryVersion> {
        validate_package_name(name)?;
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

    fn install_inner<'a>(
        &'a self,
        info: RegistryVersion,
        lock: &'a mut Lockfile,
        visiting: &'a mut BTreeSet<String>,
        completed: &'a mut BTreeMap<String, LockedPackage>,
        providers: &'a BTreeMap<String, Provider>,
        optional: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<String>>> + Send + 'a>>
    {
        Box::pin(async move {
            if !platform_matches(&info.os, &info.cpu) {
                if optional {
                    return Ok(None);
                }
                return Err(FerriteError::Npm(format!(
                    "{}@{} does not support this OS/CPU",
                    info.name, info.version
                )));
            }
            let mut package = LockedPackage {
                name: info.name.clone(),
                version: info.version.clone(),
                source: "npm".into(),
                integrity: info.dist.integrity.clone(),
                tarball: Some(info.dist.tarball.clone()),
                peer_requirements: info
                    .peer_dependencies
                    .iter()
                    .map(|(name, range)| (name.clone(), range.clone()))
                    .collect(),
                optional_peers: info
                    .peer_dependencies_meta
                    .iter()
                    .filter(|(_, meta)| meta.optional)
                    .map(|(name, _)| name.clone())
                    .collect(),
                ..Default::default()
            };
            package.optional_peers.sort();
            for (name, range) in &package.peer_requirements {
                match providers.get(name) {
                    Some(provider) if range_satisfied(&provider.version, range) => { package.peers.insert(name.clone(), provider.id.clone()); },
                    None if package.optional_peers.contains(name) => (),
                    Some(provider) => return Err(FerriteError::Npm(format!("{}@{} needs peer {name}@{range}, but its importer provides {}; change the importer dependency", info.name, info.version, provider.version))),
                    None => return Err(FerriteError::Npm(format!("{}@{} needs peer {name}@{range}; add it to this importer's package.json", info.name, info.version))),
                }
            }
            let id = package.id();
            if !visiting.insert(id.clone()) {
                return Ok(Some(id));
            }
            let previous = lock.package_by_id(&id).cloned();
            // Register the concrete identity before descending, so cycles close
            // onto an existing node instead of leaving a range in the lock.
            lock.upsert(package.clone());
            self.fetch_extract(&package, &info.dist).await?;
            let mut requests: BTreeMap<_, _> = info.dependencies.into_iter().collect();
            requests.extend(info.optional_dependencies.clone());
            package.dependency_specifiers = requests.clone();
            let mut children = BTreeMap::new();
            let mut child_providers = providers.clone();
            child_providers.insert(
                package.name.clone(),
                Provider {
                    version: package.version.clone(),
                    id: id.clone(),
                },
            );
            for (name, range) in &requests {
                let old = previous.as_ref().and_then(|old| old.dependencies.get(name));
                let child = self
                    .select(
                        name,
                        range,
                        old.map(String::as_str),
                        previous
                            .as_ref()
                            .and_then(|old| old.dependency_specifiers.get(name))
                            .map(String::as_str),
                        lock,
                    )
                    .await?;
                if !platform_matches(&child.os, &child.cpu)
                    && info.optional_dependencies.contains_key(name)
                {
                    continue;
                }
                child_providers.insert(
                    name.clone(),
                    Provider {
                        version: child.version.clone(),
                        id: format!("{name}@{}", child.version),
                    },
                );
                children.insert(name.clone(), child);
            }
            for (name, child) in children {
                if let Some(child_id) = self
                    .install_inner(
                        child,
                        lock,
                        visiting,
                        completed,
                        &child_providers,
                        info.optional_dependencies.contains_key(&name),
                    )
                    .await?
                {
                    package.dependencies.insert(name, child_id);
                }
            }
            // Peer edges are also module-resolution edges. They still have
            // their own field for context identity and conformance validation.
            for (name, peer) in &package.peers {
                package
                    .dependencies
                    .entry(name.clone())
                    .or_insert_with(|| peer.clone());
            }
            if completed
                .get(&id)
                .is_some_and(|old| old.dependencies != package.dependencies)
            {
                return Err(FerriteError::Npm(format!("{id} needs distinct transitive peer contexts; this combination is unavailable until contextual ancestor instances are implemented")));
            }
            completed.insert(id.clone(), package.clone());
            lock.upsert(package);
            visiting.remove(&id);
            Ok(Some(id))
        })
    }

    fn validate_peers(&self, lock: &Lockfile) -> Result<()> {
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

    async fn fetch_extract(&self, package: &LockedPackage, dist: &RegistryDist) -> Result<()> {
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
            verify_integrity(&bytes, dist.integrity.as_deref())?;
            std::fs::create_dir_all(&self.tarball_cache)?;
            std::fs::write(&cached, &bytes)?;
            bytes
        };
        verify_integrity(&bytes, dist.integrity.as_deref())?;
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
        .map(|(name, range)| (name.clone(), range.clone()))
        .collect()
}

fn prune_unreachable(lock: &mut Lockfile) {
    let mut pending: Vec<_> = lock
        .importers
        .values()
        .flat_map(|root| root.dependencies.values().cloned())
        .collect();
    let mut reachable = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !reachable.insert(id.clone()) {
            continue;
        }
        if let Some(package) = lock.package_by_id(&id) {
            pending.extend(package.dependencies.values().cloned());
            pending.extend(package.peers.values().cloned());
        }
    }
    lock.package
        .retain(|package| reachable.contains(&package.id()));
}

fn platform_matches(os: &Option<Vec<String>>, cpu: &Option<Vec<String>>) -> bool {
    let os_name = match std::env::consts::OS {
        "macos" => "darwin",
        other => other,
    };
    let cpu_name = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "ia32",
        other => other,
    };
    matches_list(os, os_name) && matches_list(cpu, cpu_name)
}
fn matches_list(list: &Option<Vec<String>>, current: &str) -> bool {
    let Some(list) = list else {
        return true;
    };
    if list
        .iter()
        .any(|entry| entry.strip_prefix('!') == Some(current))
    {
        return false;
    }
    let allowed: Vec<_> = list
        .iter()
        .filter(|entry| !entry.starts_with('!'))
        .collect();
    allowed.is_empty()
        || allowed
            .iter()
            .any(|entry| entry.as_str() == current || entry.as_str() == "any")
}
