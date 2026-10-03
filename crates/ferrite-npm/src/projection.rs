//! Node/editor views of the resolved graph, without a second dependency graph.
use crate::{validate_package_name, Lockfile};
use ferrite_core::{FerriteError, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
const MARKER: &str = ".ferrite-projection.json";

/// Project exact dependency edges as symlinks for Node compiler hosts. No npm
/// resolver or lifecycle scripts run. Existing unmanaged node_modules fail closed.
pub fn project_node_modules(npm_root: &Path, lock: &Lockfile) -> Result<PathBuf> {
    lock.validate()?;
    let absolute = if npm_root.is_absolute() {
        npm_root.to_path_buf()
    } else {
        std::env::current_dir()?.join(npm_root)
    };
    validate_directory(&absolute)?;
    let root = absolute.canonicalize()?;
    let store = root.join("packages");
    let mut plans = Vec::new();
    for package in &lock.package {
        let directory = store.join(package.id());
        validate_directory(&directory)?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("package.json"))?)?;
        if manifest["name"] != package.name || manifest["version"] != package.version {
            return Err(FerriteError::Npm(format!(
                "projection identity mismatch for {}; run ferrite install --frozen-lockfile",
                package.id()
            )));
        }
        plans.push((directory.join("node_modules"), package.dependencies.clone()));
    }
    let importer = lock.importers.get(".").ok_or_else(|| {
        FerriteError::Npm(
            "compiler projection requires a concrete root importer; run ferrite install".into(),
        )
    })?;
    let root_modules = root.join("node_modules");
    plans.push((root_modules.clone(), importer.dependencies.clone()));
    // Check every existing view before writing any of them.
    for (destination, links) in &plans {
        for (name, target) in links {
            validate_package_name(name)?;
            validate_directory(&store.join(target))?;
        }
        validate_existing(destination, &store)?;
    }
    for (destination, links) in plans {
        let parent = destination.parent().expect("projection has a parent");
        let stage = tempfile::tempdir_in(parent)?;
        for (name, target) in &links {
            let link = stage.path().join(name);
            std::fs::create_dir_all(link.parent().expect("link parent"))?;
            let relative = relative_target(
                destination.join(name).parent().unwrap(),
                &store.join(target),
            )?;
            link_directory(&relative, &link)?;
        }
        std::fs::write(stage.path().join(MARKER), serde_json::to_vec(&links)?)?;
        if destination.exists() {
            let existing: BTreeMap<String, String> =
                serde_json::from_slice(&std::fs::read(destination.join(MARKER))?)?;
            if existing == links
                && links.iter().all(|(name, target)| {
                    let path = destination.join(name);
                    relative_target(path.parent().unwrap(), &store.join(target)).is_ok_and(
                        |expected| std::fs::read_link(path).is_ok_and(|actual| actual == expected),
                    )
                })
            {
                continue;
            }
            let backup = tempfile::tempdir_in(parent)?;
            let old = backup.path().join("old");
            std::fs::rename(&destination, &old)?;
            if let Err(error) = std::fs::rename(stage.path(), &destination) {
                std::fs::rename(&old, &destination)?;
                return Err(error.into());
            }
            // Only the verified owned view is removed; symlink targets survive.
        } else {
            std::fs::rename(stage.path(), &destination)?;
        }
    }
    Ok(root_modules)
}
/// Expose the same concrete graph to editors and Node resolution at the project
/// root. The only new link is relative and portable; no dependency resolution runs.
pub fn project_editor_dependencies(project_root: &Path, lock: &Lockfile) -> Result<PathBuf> {
    let absolute = if project_root.is_absolute() {
        project_root.to_path_buf()
    } else {
        std::env::current_dir()?.join(project_root)
    };
    validate_directory(&absolute)?;
    let project = absolute.canonicalize()?;
    let destination = project.join("node_modules");
    let target = Path::new(".ferrite/npm/node_modules");
    // Reject unmanaged editor views before mutating any package projections.
    validate_editor_link(&destination, target)?;
    lock.validate()?;
    // Empty manifests still need an owned view; ensure each level separately
    // and reject existing symlinks instead of following them through mkdir_all.
    for directory in [project.join(".ferrite"), project.join(".ferrite/npm")] {
        match std::fs::create_dir(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        validate_directory(&directory)?;
    }
    project_node_modules(&project.join(".ferrite/npm"), lock)?;
    if !validate_editor_link(&destination, target)? {
        // Symlink creation itself refuses an existing destination, including a
        // destination introduced after validation.
        link_directory(target, &destination)?;
    }
    Ok(destination)
}

/// Preflight installation's editor destination without creating any graph view.
pub fn validate_editor_destination(project_root: &Path) -> Result<()> {
    validate_editor_link(
        &project_root.join("node_modules"),
        Path::new(".ferrite/npm/node_modules"),
    )?;
    Ok(())
}
fn validate_editor_link(destination: &Path, target: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_symlink() && std::fs::read_link(destination)? == target => Ok(true),
        Ok(_) => Err(FerriteError::Npm(format!("unmanaged editor node_modules at {}; Ferrite will not replace it; move it aside before projecting the locked graph", destination.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn relative_target(from: &Path, target: &Path) -> Result<PathBuf> {
    let from: Vec<_> = from.components().collect();
    let target: Vec<_> = target.components().collect();
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return Err(FerriteError::Npm(
            "projection links require a shared filesystem root".into(),
        ));
    }
    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for component in &target[common..] {
        result.push(component.as_os_str());
    }
    Ok(result)
}

fn validate_directory(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(FerriteError::Npm(format!(
                "compiler projection refuses symlink/non-directory {}",
                ancestor.display()
            )));
        }
    }
    Ok(())
}
fn validate_existing(destination: &Path, store: &Path) -> Result<()> {
    match std::fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(FerriteError::Npm(format!(
                "unmanaged compiler projection {}; move it aside before enabling the compiler host",
                destination.display()
            )))
        }
        _ => (),
    }
    let links: BTreeMap<String, String> = std::fs::read(destination.join(MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| {
            FerriteError::Npm(format!(
                "unmanaged node_modules at {}; Ferrite will not replace it",
                destination.display()
            ))
        })?;
    let mut allowed = std::collections::BTreeSet::from([MARKER.to_string()]);
    for (name, target) in &links {
        validate_package_name(name)?;
        if Path::new(target).components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(FerriteError::Npm(
                "unsafe compiler projection marker".into(),
            ));
        }
        let path = destination.join(name);
        let actual = std::fs::read_link(&path)?;
        let relative = relative_target(path.parent().unwrap(), &store.join(target))?;
        if actual != store.join(target) && actual != relative {
            return Err(FerriteError::Npm(format!(
                "modified compiler link {}; refusing to overwrite",
                path.display()
            )));
        }
        allowed.insert(name.split('/').next().unwrap().into());
        if let Some((scope, leaf)) = name.split_once('/') {
            validate_directory(&destination.join(scope))?;
            let expected: std::collections::BTreeSet<_> = links
                .keys()
                .filter_map(|name| name.strip_prefix(&format!("{scope}/")))
                .collect();
            for entry in std::fs::read_dir(destination.join(scope))? {
                let name = entry?.file_name().to_string_lossy().into_owned();
                if !expected.contains(name.as_str()) {
                    return Err(FerriteError::Npm(format!(
                        "unmanaged scoped projection entry {scope}/{name}"
                    )));
                }
            }
            let _ = leaf;
        }
    }
    for entry in std::fs::read_dir(destination)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if !allowed.contains(&name) {
            return Err(FerriteError::Npm(format!(
                "unmanaged projection entry {name}"
            )));
        }
    }
    Ok(())
}
#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}
#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, link).map_err(|error| {
        FerriteError::Npm(format!(
            "compiler projection needs directory symlinks (enable Windows Developer Mode): {error}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LockedImporter, LockedPackage};
    fn fixture() -> (tempfile::TempDir, Lockfile) {
        let root = tempfile::tempdir().unwrap();
        let mut a = LockedPackage {
            name: "a".into(),
            version: "1.0.0".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let one = LockedPackage {
            name: "@scope/shared".into(),
            version: "1.0.0".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let two = LockedPackage {
            version: "2.0.0".into(),
            ..one.clone()
        };
        a.dependencies.insert(one.name.clone(), one.id());
        let mut lock = Lockfile {
            package: vec![a.clone(), one, two.clone()],
            ..Default::default()
        };
        lock.importers.insert(
            ".".into(),
            LockedImporter {
                dependencies: BTreeMap::from([
                    (a.name.clone(), a.id()),
                    (two.name.clone(), two.id()),
                ]),
                ..Default::default()
            },
        );
        for package in &lock.package {
            let dir = root.path().join("packages").join(package.id());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("package.json"),
                serde_json::to_vec(
                    &serde_json::json!({"name":package.name,"version":package.version}),
                )
                .unwrap(),
            )
            .unwrap();
        }
        (root, lock)
    }
    #[test]
    fn projects_concrete_versions_and_updates_owned_views() {
        let (root, mut lock) = fixture();
        let view = project_node_modules(root.path(), &lock).unwrap();
        let store = root.path().join("packages");
        assert_eq!(
            std::fs::read_link(view.join("@scope/shared")).unwrap(),
            PathBuf::from("../../packages/@scope/shared@2.0.0")
        );
        assert_eq!(
            std::fs::read_link(store.join("a@1.0.0/node_modules/@scope/shared")).unwrap(),
            PathBuf::from("../../../@scope/shared@1.0.0")
        );
        project_node_modules(root.path(), &lock).unwrap();
        lock.importers
            .get_mut(".")
            .unwrap()
            .dependencies
            .insert("@scope/shared".into(), "@scope/shared@1.0.0".into());
        project_node_modules(root.path(), &lock).unwrap();
        assert_eq!(
            std::fs::read_link(view.join("@scope/shared")).unwrap(),
            PathBuf::from("../../packages/@scope/shared@1.0.0")
        );
        assert!(store.join("@scope/shared@2.0.0/package.json").is_file());
    }
    #[test]
    fn upgrades_owned_absolute_links_even_when_edges_are_unchanged() {
        let (root, lock) = fixture();
        let view = project_node_modules(root.path(), &lock).unwrap();
        let path = view.join("a");
        std::fs::remove_file(&path).unwrap();
        link_directory(&root.path().join("packages/a@1.0.0"), &path).unwrap();
        project_node_modules(root.path(), &lock).unwrap();
        assert_eq!(
            std::fs::read_link(path).unwrap(),
            PathBuf::from("../packages/a@1.0.0")
        );
    }
    #[test]
    fn editor_view_survives_project_relocation_and_retains_importer_versions() {
        let (store, lock) = fixture();
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir(project.path().join(".ferrite")).unwrap();
        std::fs::rename(store.path(), project.path().join(".ferrite/npm")).unwrap();
        project_editor_dependencies(project.path(), &lock).unwrap();
        assert_eq!(
            std::fs::read_link(project.path().join("node_modules")).unwrap(),
            PathBuf::from(".ferrite/npm/node_modules")
        );
        let relocated = tempfile::tempdir().unwrap();
        let root = relocated.path().join("app");
        std::fs::rename(project.path(), &root).unwrap();
        for (package, version) in [
            ("@scope/shared", "2.0.0"),
            ("a/node_modules/@scope/shared", "1.0.0"),
        ] {
            let manifest: serde_json::Value = serde_json::from_slice(
                &std::fs::read(root.join("node_modules").join(package).join("package.json"))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(manifest["version"], version);
        }
        project_editor_dependencies(&root, &lock).unwrap();
    }
    #[test]
    fn unmanaged_editor_views_fail_before_any_package_projection_is_created() {
        let (store, lock) = fixture();
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir(project.path().join(".ferrite")).unwrap();
        std::fs::rename(store.path(), project.path().join(".ferrite/npm")).unwrap();
        let view = project.path().join("node_modules");
        std::fs::create_dir(&view).unwrap();
        std::fs::write(view.join("keep"), "unrelated").unwrap();
        assert!(project_editor_dependencies(project.path(), &lock)
            .unwrap_err()
            .to_string()
            .contains("unmanaged editor"));
        assert_eq!(
            std::fs::read_to_string(view.join("keep")).unwrap(),
            "unrelated"
        );
        assert!(!project
            .path()
            .join(".ferrite/npm/packages/a@1.0.0/node_modules")
            .exists());
    }

    #[test]
    fn refuses_unmanaged_views_before_writing_any_links() {
        let (root, lock) = fixture();
        let unmanaged = root.path().join("node_modules");
        std::fs::create_dir(&unmanaged).unwrap();
        std::fs::write(unmanaged.join("keep.txt"), "keep").unwrap();
        assert!(project_node_modules(root.path(), &lock)
            .unwrap_err()
            .to_string()
            .contains("unmanaged"));
        assert_eq!(
            std::fs::read_to_string(unmanaged.join("keep.txt")).unwrap(),
            "keep"
        );
        assert!(!root.path().join("packages/a@1.0.0/node_modules").exists());
    }
    #[test]
    fn refuses_modified_owned_links() {
        let (root, lock) = fixture();
        let view = project_node_modules(root.path(), &lock).unwrap();
        let path = view.join("@scope/shared");
        std::fs::remove_file(&path).unwrap();
        link_directory(&root.path().join("packages/@scope/shared@1.0.0"), &path).unwrap();
        assert!(project_node_modules(root.path(), &lock)
            .unwrap_err()
            .to_string()
            .contains("modified compiler link"));
    }
}
