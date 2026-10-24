//! Project-local manifests and relocatable live source anchors.
use crate::{
    manifest_requests, Installer, JsPackageJson, Lockfile, PeerDependencyMeta, RegistryDist,
    RegistryVersion,
};
use ferrite_core::{FerriteError, Result};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Component, Path, PathBuf},
};
/// Registry JSON cannot construct this host-owned source descriptor.
#[derive(Clone, Debug, serde::Serialize)]
pub struct LocalSource {
    pub path: String,
    pub kind: String,
    pub manifest_hash: String,
}
pub(crate) struct ProjectSources {
    pub root: PathBuf,
    pub manifests: BTreeMap<String, JsPackageJson>,
    pub locals: BTreeMap<String, RegistryVersion>,
    names: BTreeMap<String, String>,
}
fn error(message: impl std::fmt::Display) -> FerriteError {
    FerriteError::Npm(format!("local sources: {message}"))
}
fn ordinary(path: &Path, file: bool) -> Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink()
        || if file {
            !meta.is_file()
        } else {
            !meta.is_dir()
        }
    {
        return Err(error(format!(
            "{} must be an ordinary {}",
            path.display(),
            if file { "file" } else { "directory" }
        )));
    }
    Ok(())
}
pub(crate) fn validate_source_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path == "."
        || path.contains(['\\', ':'])
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || path
            .split('/')
            .any(|part| matches!(part, "node_modules" | ".ferrite" | ".git"))
    {
        return Err(error(format!("unsafe project source path {path}")));
    }
    Ok(())
}
fn source_directory(root: &Path, path: &str) -> Result<PathBuf> {
    validate_source_path(path)?;
    let mut directory = root.to_path_buf();
    for part in path.split('/') {
        directory.push(part);
        ordinary(&directory, false)?;
    }
    Ok(directory)
}
fn local_info(root: &Path, path: &str, kind: &str) -> Result<(JsPackageJson, RegistryVersion)> {
    let directory = source_directory(root, path)?;
    ordinary(&directory.join("package.json"), true)?;
    let bytes = std::fs::read(directory.join("package.json"))?;
    let mut value: serde_json::Value = serde_json::from_slice(&bytes)?;
    value.sort_all_objects();
    let manifest: JsPackageJson = serde_json::from_value(value.clone())?;
    validate_requests(&manifest)?;
    let name = value["name"]
        .as_str()
        .ok_or_else(|| error("local package needs a name"))?
        .to_string();
    crate::validate_package_name(&name)?;
    let version = value["version"]
        .as_str()
        .ok_or_else(|| error("local package needs an exact version"))?
        .to_string();
    node_semver::Version::parse(&version).map_err(error)?;
    let peers = serde_json::from_value(
        value
            .get("peerDependencies")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    )?;
    let peer_meta: std::collections::HashMap<String, PeerDependencyMeta> = serde_json::from_value(
        value
            .get("peerDependenciesMeta")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    )?;
    let mut dependencies = manifest.dependencies.clone();
    dependencies.extend(manifest.dev_dependencies.clone());
    let local = LocalSource {
        path: path.into(),
        kind: kind.into(),
        manifest_hash: ferrite_core::Hash::of_str(&serde_json::to_string(&value)?).0,
    };
    Ok((
        manifest.clone(),
        RegistryVersion {
            name,
            version,
            dependencies,
            optional_dependencies: manifest.optional_dependencies,
            peer_dependencies: peers,
            peer_dependencies_meta: peer_meta,
            dist: RegistryDist {
                tarball: String::new(),
                integrity: None,
                shasum: None,
            },
            os: serde_json::from_value(
                value.get("os").cloned().unwrap_or(serde_json::Value::Null),
            )?,
            cpu: serde_json::from_value(
                value.get("cpu").cloned().unwrap_or(serde_json::Value::Null),
            )?,
            libc: serde_json::from_value(
                value
                    .get("libc")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            )?,
            binary: None,
            gypfile: None,
            local: Some(local),
        },
    ))
}
pub(crate) fn validate_requests(manifest: &JsPackageJson) -> Result<()> {
    for field in ["dependencies", "devDependencies", "optionalDependencies"] {
        if manifest.rest.contains_key(field) {
            return Err(error(format!(
                "reserved {field} must use the typed manifest field"
            )));
        }
    }
    for (name, range) in &manifest.dependencies {
        if !manifest.optional_dependencies.contains_key(name)
            && manifest
                .dev_dependencies
                .get(name)
                .is_some_and(|dev| dev != range)
        {
            return Err(error(format!(
                "conflicting dependency and devDependency for {name}"
            )));
        }
    }
    Ok(())
}
impl ProjectSources {
    pub(crate) fn discover(root: &Path, manifest: &JsPackageJson) -> Result<Self> {
        let root = root.canonicalize()?;
        let mut sources = Self {
            root,
            manifests: BTreeMap::from([(".".into(), manifest.clone())]),
            locals: BTreeMap::new(),
            names: BTreeMap::new(),
        };
        let declaration = manifest.rest.get("workspaces");
        let patterns = declaration.and_then(|value| {
            value
                .as_array()
                .or_else(|| value.get("packages").and_then(|value| value.as_array()))
        });
        if declaration.is_some_and(|value| !value.is_null()) && patterns.is_none() {
            return Err(error("workspaces must be an array or {packages: array}"));
        }
        let mut include = globset::GlobSetBuilder::new();
        let mut exclude = globset::GlobSetBuilder::new();
        for pattern in patterns.into_iter().flatten() {
            let pattern = pattern
                .as_str()
                .ok_or_else(|| error("workspace pattern must be a string"))?;
            let (negative, pattern) = pattern
                .strip_prefix('!')
                .map(|pattern| (true, pattern))
                .unwrap_or((false, pattern));
            validate_source_path(pattern)?;
            let glob = globset::GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map_err(error)?;
            if negative {
                exclude.add(glob);
            } else {
                include.add(glob);
            }
        }
        let include = include.build().map_err(error)?;
        let exclude = exclude.build().map_err(error)?;
        let mut directories = if include.is_empty() {
            VecDeque::new()
        } else {
            VecDeque::from([String::new()])
        };
        let mut scanned = 0;
        while let Some(parent) = directories.pop_front() {
            for entry in std::fs::read_dir(sources.root.join(&parent))? {
                scanned += 1;
                if scanned > 20_000 {
                    return Err(error("workspace discovery limit exceeded"));
                }
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with('.')
                    || matches!(name.as_str(), "node_modules" | "target" | "dist")
                {
                    continue;
                }
                let path = if parent.is_empty() {
                    name
                } else {
                    format!("{parent}/{name}")
                };
                let matched = include.is_match(&path) && !exclude.is_match(&path);
                let kind = entry.file_type()?;
                if matched && kind.is_symlink() {
                    return Err(error("workspace source directories cannot be symlinks"));
                }
                if !kind.is_dir() {
                    continue;
                }
                if matched {
                    let (manifest, info) = local_info(&sources.root, &path, "workspace")?;
                    if sources
                        .names
                        .insert(info.name.clone(), path.clone())
                        .is_some()
                    {
                        return Err(error(format!("duplicate workspace name {}", info.name)));
                    }
                    sources.manifests.insert(path.clone(), manifest);
                    sources.locals.insert(path.clone(), info);
                }
                directories.push_back(path);
            }
        }
        let mut pending: VecDeque<_> = sources
            .manifests
            .iter()
            .map(|(path, manifest)| (path.clone(), manifest.clone()))
            .collect();
        while let Some((owner, manifest)) = pending.pop_front() {
            validate_requests(&manifest)?;
            for request in manifest_requests(&manifest)
                .values()
                .filter_map(|range| range.strip_prefix("file:"))
            {
                let path = sources.file_path(&owner, request)?;
                if sources.locals.contains_key(&path) {
                    continue;
                }
                let (manifest, info) = local_info(&sources.root, &path, "link")?;
                sources.locals.insert(path.clone(), info);
                pending.push_back((path, manifest));
                if sources.locals.len() > 20_000 {
                    return Err(error("local source limit exceeded"));
                }
            }
        }
        Ok(sources)
    }
    fn file_path(&self, owner: &str, request: &str) -> Result<String> {
        if Path::new(request).is_absolute() || request.contains(['\\', ':']) {
            return Err(error("file link must be project-relative"));
        }
        let mut parts: Vec<&str> = if owner == "." {
            Vec::new()
        } else {
            owner.split('/').collect()
        };
        for part in request.split('/') {
            match part {
                "" | "." => (),
                ".." => {
                    if parts.pop().is_none() {
                        return Err(error("file link escapes project"));
                    }
                }
                part => parts.push(part),
            }
        }
        let path = parts.join("/");
        source_directory(&self.root, &path)?;
        Ok(path)
    }
    pub(crate) fn select(
        &self,
        name: &str,
        range: &str,
        owner: &str,
    ) -> Result<Option<RegistryVersion>> {
        if let Some(request) = range.strip_prefix("file:") {
            let path = self.file_path(owner, request)?;
            return self
                .locals
                .get(&path)
                .cloned()
                .map(Some)
                .ok_or_else(|| error("undiscovered file source"));
        }
        let explicit = range.strip_prefix("workspace:");
        if range.starts_with("npm:") {
            return Ok(None);
        }
        let Some(path) = self.names.get(name) else {
            return if explicit.is_some() {
                Err(error(format!("missing workspace {name}")))
            } else {
                Ok(None)
            };
        };
        let info = &self.locals[path];
        let request = explicit.unwrap_or(range);
        let compatible = explicit.is_some() && matches!(request, "*" | "^" | "~")
            || crate::range_satisfied(&info.version, request);
        if compatible {
            Ok(Some(info.clone()))
        } else if explicit.is_some() {
            Err(error(format!(
                "workspace {name}@{} does not satisfy {range}",
                info.version
            )))
        } else {
            Ok(None)
        }
    }
}

impl Installer {
    /// Install a root manifest plus discovered workspace importers and file links.
    pub async fn install_project(
        &self,
        root: &Path,
        lock: &mut Lockfile,
        frozen: bool,
    ) -> Result<()> {
        self.install_project_manifest(
            root,
            &JsPackageJson::read(&root.join("package.json"))?,
            lock,
            frozen,
        )
        .await
    }
    /// In-memory root requests support CLI add/remove without publishing a manifest first.
    pub async fn install_project_manifest(
        &self,
        root: &Path,
        manifest: &JsPackageJson,
        lock: &mut Lockfile,
        frozen: bool,
    ) -> Result<()> {
        let project = ProjectSources::discover(root, manifest)?;
        if frozen {
            lock.validate()?;
            if !lock.portable {
                return Err(error(
                    "frozen install requires a portable v3 graph; run a non-frozen install",
                ));
            }
            if lock.importers.keys().ne(project.manifests.keys()) {
                return Err(error("frozen workspace importer set changed"));
            }
            for (path, manifest) in &project.manifests {
                let importer = &lock.importers[path];
                let optional: Vec<_> = manifest
                    .optional_dependencies
                    .keys()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if importer.specifiers != manifest_requests(manifest)
                    || importer.optional_dependencies != optional
                    || importer.dependencies.keys().ne(importer.specifiers.keys())
                {
                    return Err(error(format!("frozen requests changed for {path}")));
                }
            }
            let paths: BTreeSet<_> = lock
                .package
                .iter()
                .filter_map(|package| package.path.as_ref())
                .collect();
            if paths != project.locals.keys().collect() {
                return Err(error("frozen local source set changed"));
            }
            for package in lock.package.iter().filter(|package| package.path.is_some()) {
                let info = &project.locals[package.path.as_ref().unwrap()];
                let local = info.local.as_ref().unwrap();
                if package.name != info.name
                    || package.version != info.version
                    || package.source != local.kind
                    || package.integrity.as_deref() != Some(&local.manifest_hash)
                {
                    return Err(error(format!(
                        "frozen local manifest changed: {}",
                        local.path
                    )));
                }
            }
            self.validate_peers(lock)?;
            return self.fetch_project(lock, &project.root).await;
        }
        let mut roots = BTreeMap::new();
        for (path, manifest) in &project.manifests {
            let mut infos = BTreeMap::new();
            for (name, range) in manifest_requests(manifest) {
                let local = project.select(&name, &range, path)?;
                let old = lock.importers.get(path);
                let info = if let Some(local) = local {
                    local
                } else {
                    self.select(
                        &name,
                        &range,
                        old.and_then(|importer| importer.dependencies.get(&name))
                            .map(String::as_str),
                        old.and_then(|importer| importer.specifiers.get(&name))
                            .map(String::as_str),
                        lock,
                    )
                    .await?
                };
                infos.insert(name, info);
            }
            if path != "." {
                infos.insert("\0workspace".into(), project.locals[path].clone());
            }
            roots.insert(path.clone(), infos);
        }
        let (package, mut edges) =
            crate::context_graph::resolve_importers(self, roots, lock, Some(&project)).await?;
        let importers = project
            .manifests
            .iter()
            .map(|(path, manifest)| {
                (
                    path.clone(),
                    crate::LockedImporter {
                        specifiers: manifest_requests(manifest),
                        dependencies: edges.remove(path).expect("resolved importer"),
                        optional_dependencies: manifest
                            .optional_dependencies
                            .keys()
                            .cloned()
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect(),
                    },
                )
            })
            .collect();
        let updated = Lockfile {
            portable: true,
            package,
            importers,
            ..Default::default()
        };
        updated.validate()?;
        self.validate_peers(&updated)?;
        for package in updated
            .package
            .iter()
            .filter(|package| package.path.is_some())
        {
            let directory = local_directory(&project.root, package)?;
            let store = if self.store.exists() {
                self.store.canonicalize()?
            } else if self.store.is_absolute() {
                self.store.clone()
            } else {
                std::env::current_dir()?.join(&self.store)
            };
            crate::projection::validate_existing(&directory.join("node_modules"), &store)?;
        }
        self.fetch_project(&updated, &project.root).await?;
        *lock = updated;
        Ok(())
    }
    async fn fetch_project(&self, lock: &Lockfile, root: &Path) -> Result<()> {
        let active = lock.active_graph(&crate::InstallPlatform::current())?;
        for package in lock
            .package
            .iter()
            .filter(|package| active.packages.contains(&package.id()))
        {
            if package.path.is_some() {
                install_anchor(&self.store, root, package)?;
            } else {
                self.fetch_extract(
                    package,
                    &RegistryDist {
                        tarball: package
                            .tarball
                            .clone()
                            .ok_or_else(|| error("locked tarball URL missing"))?,
                        integrity: package.integrity.clone(),
                        shasum: None,
                    },
                )
                .await?;
            }
        }
        Ok(())
    }
}
pub(crate) fn local_directory(root: &Path, package: &crate::LockedPackage) -> Result<PathBuf> {
    let path = package
        .path
        .as_deref()
        .ok_or_else(|| error("local source path missing"))?;
    let (.., info) = local_info(root, path, &package.source)?;
    if info.name != package.name
        || info.version != package.version
        || info
            .local
            .as_ref()
            .map(|source| source.manifest_hash.as_str())
            != package.integrity.as_deref()
    {
        return Err(error(format!(
            "local manifest identity changed: {path}; run ferrite install"
        )));
    }
    source_directory(root, path)
}
fn install_anchor(store: &Path, root: &Path, package: &crate::LockedPackage) -> Result<()> {
    let source = local_directory(root, package)?;
    let absolute = if store.is_absolute() {
        store.to_path_buf()
    } else {
        std::env::current_dir()?.join(store)
    };
    let mut directory = PathBuf::new();
    for component in absolute.components() {
        directory.push(component.as_os_str());
        match std::fs::create_dir(&directory) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        ordinary(&directory, false)?;
    }
    let store = absolute.canonicalize()?;
    let destination = store.join(package.id());
    let parent = destination
        .parent()
        .ok_or_else(|| error("invalid local anchor"))?;
    if parent != store.as_path() {
        match std::fs::create_dir(parent) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
        ordinary(parent, false)?;
    }
    let relative = crate::projection::relative_target(parent, &source)?;
    match std::fs::symlink_metadata(&destination) {
        Ok(meta)
            if meta.file_type().is_symlink() && std::fs::read_link(&destination)? == relative =>
        {
            Ok(())
        }
        Ok(_) => Err(error(format!(
            "unmanaged local store anchor {}",
            destination.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            crate::projection::link_directory(&relative, &destination)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(root: &Path, path: &str, content: &str) {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, content).unwrap();
    }
    fn installer(root: &Path) -> Installer {
        let npm = root.join(".ferrite/npm");
        Installer::new(
            crate::RegistryClient::new("http://127.0.0.1:9", npm.join("metadata")).unwrap(),
            npm,
        )
    }
    #[tokio::test]
    async fn live_workspaces_file_aliases_frozen_manifest_checks_and_relocation() {
        let container = tempfile::tempdir().unwrap();
        let root = container.path().join("project");
        write(
            &root,
            "package.json",
            r#"{"workspaces":{"packages":["packages/*"]},"dependencies":{"app":"workspace:*","offset":"file:local"}}"#,
        );
        write(
            &root,
            "packages/app/package.json",
            r#"{"name":"app","version":"1.0.0","exports":"./index.js","dependencies":{"shared":"workspace:^","offset":"file:../../local"}}"#,
        );
        write(
            &root,
            "packages/app/index.js",
            "export { value } from 'shared';",
        );
        write(
            &root,
            "packages/shared/package.json",
            r#"{"name":"shared","version":"1.0.0-beta.1","exports":"./index.js"}"#,
        );
        write(
            &root,
            "packages/shared/index.js",
            "export const value = 40;",
        );
        write(
            &root,
            "local/package.json",
            r#"{"name":"actual-offset","version":"1.0.0","exports":"./index.js"}"#,
        );
        write(&root, "local/index.js", "export const offset = 2;");
        let install = installer(&root);
        let mut lock = Lockfile::default();
        install
            .install_project(&root, &mut lock, false)
            .await
            .unwrap();
        assert_eq!(lock.importers.len(), 3);
        assert_eq!(lock.package.len(), 3);
        crate::project_editor_dependencies(&root, &lock).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("node_modules/app/node_modules/shared/index.js"))
                .unwrap(),
            "export const value = 40;"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("node_modules/offset/index.js")).unwrap(),
            "export const offset = 2;"
        );
        write(
            &root,
            "packages/shared/index.js",
            "export const value = 50;",
        );
        assert_eq!(
            std::fs::read_to_string(root.join("node_modules/app/node_modules/shared/index.js"))
                .unwrap(),
            "export const value = 50;"
        );
        let before = lock.clone();
        install
            .install_project(&root, &mut lock, true)
            .await
            .unwrap();
        assert_eq!(lock, before);
        let moved = container.path().join("moved");
        std::fs::rename(&root, &moved).unwrap();
        let install = installer(&moved);
        install
            .install_project(&moved, &mut lock, true)
            .await
            .unwrap();
        crate::project_editor_dependencies(&moved, &lock).unwrap();
        assert!(moved
            .join("node_modules/app/node_modules/shared/index.js")
            .is_file());
        write(
            &moved,
            "packages/shared/package.json",
            r#"{"name":"shared","version":"1.0.0-beta.2","exports":"./index.js"}"#,
        );
        assert!(install
            .install_project(&moved, &mut lock, true)
            .await
            .unwrap_err()
            .to_string()
            .contains("frozen local manifest changed"));
        assert_eq!(lock, before);
    }
    #[tokio::test]
    async fn duplicate_names_and_escaping_file_links_fail_before_publication() {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "package.json",
            r#"{"workspaces":["packages/*"]}"#,
        );
        for name in ["a", "b"] {
            write(
                root.path(),
                &format!("packages/{name}/package.json"),
                r#"{"name":"duplicate","version":"1.0.0"}"#,
            );
        }
        let mut lock = Lockfile::default();
        let before = lock.clone();
        let install = installer(root.path());
        assert!(install
            .install_project(root.path(), &mut lock, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("duplicate workspace"));
        assert_eq!(lock, before);
        write(
            root.path(),
            "package.json",
            r#"{"dependencies":{"escape":"file:../outside"}}"#,
        );
        assert!(install
            .install_project(root.path(), &mut lock, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("escapes project"));
        assert_eq!(lock, before);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn local_manifest_and_workspace_directory_symlinks_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut lock = Lockfile::default();
        write(
            root.path(),
            "package.json",
            r#"{"workspaces":["packages/*"]}"#,
        );
        write(
            root.path(),
            "real/package.json",
            r#"{"name":"linked","version":"1.0.0"}"#,
        );
        std::fs::create_dir(root.path().join("packages")).unwrap();
        std::os::unix::fs::symlink("../real", root.path().join("packages/linked")).unwrap();
        assert!(installer(root.path())
            .install_project(root.path(), &mut lock, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot be symlinks"));
        write(
            root.path(),
            "package.json",
            r#"{"dependencies":{"linked":"file:source"}}"#,
        );
        std::fs::create_dir(root.path().join("source")).unwrap();
        std::os::unix::fs::symlink(
            "../real/package.json",
            root.path().join("source/package.json"),
        )
        .unwrap();
        assert!(installer(root.path())
            .install_project(root.path(), &mut lock, false)
            .await
            .unwrap_err()
            .to_string()
            .contains("ordinary file"));
        assert!(lock.package.is_empty());
    }
}
