//! npm package commands.

use crate::cli::*;
use std::path::Path;
use std::path::PathBuf;

pub(crate) fn npm_context(
    root: &Path,
) -> ferrite::Result<(
    ferrite::npm::RegistryClient,
    ferrite::npm::Installer,
    PathBuf,
    PathBuf,
)> {
    npm_context_selected(root, None, None)
}

fn npm_context_selected(
    root: &Path,
    config_path: Option<&Path>,
    mode: Option<String>,
) -> ferrite::Result<(
    ferrite::npm::RegistryClient,
    ferrite::npm::Installer,
    PathBuf,
    PathBuf,
)> {
    let user = match config_path {
        Some(path) => ferrite::load_user_config_path(path)?,
        None => ferrite::load_user_config(root)?,
    };
    let resolved = ferrite::resolve_config(
        user,
        Some(root.to_path_buf()),
        ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
    )?;
    ferrite::npm::validate_editor_destination(root)?;
    let npm_root = root.join(".ferrite/npm");
    let cache_dir = npm_root.join("metadata");
    let client = ferrite::npm::RegistryClient::new(resolved.npm.registry.clone(), cache_dir)?;
    let installer = ferrite::npm::Installer::new(client.clone(), npm_root);
    Ok((
        client,
        installer,
        resolved.lockfile(),
        root.join("package.json"),
    ))
}

pub(crate) async fn add(
    mut args: AddArgs,
    config_path: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    args.root = crate::root_of(&config_path, args.root);
    let (_, installer, lock_path, pkg_path) =
        npm_context_selected(&args.root, config_path.as_deref(), mode)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    let mut added = Vec::new();
    for spec in &args.specs {
        let (name, range) = ferrite::npm::parse_spec(spec);
        if args.dev {
            pkg.dependencies.remove(&name);
            pkg.dev_dependencies.insert(name.clone(), range.clone());
        } else {
            pkg.dev_dependencies.remove(&name);
            pkg.dependencies.insert(name.clone(), range.clone());
        }
        added.push((name, range));
    }
    installer.install_manifest(&pkg, &mut lock, false).await?;
    for (name, range) in added {
        let version = lock.find(&name).expect("installed root").version.clone();
        if range == "latest" {
            let saved = format!("^{version}");
            if args.dev {
                pkg.dev_dependencies.insert(name.clone(), saved.clone());
            } else {
                pkg.dependencies.insert(name.clone(), saved.clone());
            }
            lock.importers
                .get_mut(".")
                .expect("root importer")
                .specifiers
                .insert(name.clone(), saved);
        }
        println!("added {name}@{version}");
    }
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    ferrite::npm::project_editor_dependencies(&args.root, &lock)?;
    Ok(())
}

pub(crate) async fn remove(
    mut args: RemoveArgs,
    config_path: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    args.root = crate::root_of(&config_path, args.root);
    let (_, installer, lock_path, pkg_path) =
        npm_context_selected(&args.root, config_path.as_deref(), mode)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    for name in &args.names {
        pkg.dependencies.remove(name);
        pkg.dev_dependencies.remove(name);
    }
    // Recompute reachability; removed roots may still be transitive dependencies.
    installer.install_manifest(&pkg, &mut lock, false).await?;
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    ferrite::npm::project_editor_dependencies(&args.root, &lock)?;
    for name in &args.names {
        println!("removed root dependency {name}");
    }
    Ok(())
}

pub(crate) async fn update(
    mut args: UpdateArgs,
    config_path: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    args.root = crate::root_of(&config_path, args.root);
    let (client, installer, lock_path, pkg_path) =
        npm_context_selected(&args.root, config_path.as_deref(), mode)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    let targets: Vec<_> = if args.specs.is_empty() {
        ferrite::npm::manifest_requests(&pkg).into_iter().collect()
    } else {
        args.specs
            .iter()
            .map(|spec| ferrite::npm::parse_spec(spec))
            .collect()
    };
    for (name, range) in &targets {
        if let Some(root) = lock.importers.get_mut(".") {
            root.dependencies.remove(name);
        }
        client.invalidate_metadata(name)?;
        if let Some(entry) = pkg.dependencies.get_mut(name) {
            if range != "latest" {
                *entry = range.clone();
            }
        } else if let Some(entry) = pkg.dev_dependencies.get_mut(name) {
            if range != "latest" {
                *entry = range.clone();
            }
        } else {
            return Err(ferrite::FerriteError::Npm(format!(
                "cannot update undeclared root {name}; run ferrite add {name}"
            )));
        }
    }
    installer.install_manifest(&pkg, &mut lock, false).await?;
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    ferrite::npm::project_editor_dependencies(&args.root, &lock)?;
    for (name, _) in targets {
        println!(
            "updated {name}@{}",
            lock.find(&name).expect("installed root").version
        );
    }
    Ok(())
}

pub(crate) async fn install(
    mut args: InstallArgs,
    config_path: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    args.root = crate::root_of(&config_path, args.root);
    let (_, installer, lock_path, pkg_path) =
        npm_context_selected(&args.root, config_path.as_deref(), mode)?;
    if args.frozen_lockfile && !lock_path.exists() {
        return Err(ferrite::FerriteError::Npm(format!(
            "frozen install requires {}; run ferrite install first",
            lock_path.display()
        )));
    }
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    installer
        .install_manifest(&pkg, &mut lock, args.frozen_lockfile)
        .await?;
    if !args.frozen_lockfile {
        lock.write(&lock_path)?;
    }
    ferrite::npm::project_editor_dependencies(&args.root, &lock)?;
    println!("installed {} concrete packages", lock.package.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn selected_config_install_and_frozen_replay_share_root_and_lock() {
        let project = tempfile::tempdir().unwrap();
        let unrelated = tempfile::tempdir().unwrap();
        let selected = project.path().join("packages.toml");
        std::fs::write(project.path().join("ferrite.toml"), "invalid default TOML!").unwrap();
        std::fs::write(project.path().join("package.json"), "{}").unwrap();
        std::fs::write(
            &selected,
            "[npm]\nlockfile = 'selected.lock'\nregistry = 'http://127.0.0.1:1'\n[framework]\nenabled = ['vue']\ncompiler_host = 'node'\nnode = 'missing-node'\n",
        )
        .unwrap();
        let (client, installer, lock, manifest) =
            npm_context_selected(project.path(), Some(&selected), None).unwrap();
        assert_eq!(client.registry, "http://127.0.0.1:1");
        assert_eq!(
            client.cache_dir,
            project.path().join(".ferrite/npm/metadata")
        );
        assert_eq!(
            installer.store,
            project.path().join(".ferrite/npm/packages")
        );
        assert_eq!(lock, project.path().join("selected.lock"));
        assert_eq!(manifest, project.path().join("package.json"));
        let args = |frozen| InstallArgs {
            root: unrelated.path().to_path_buf(),
            frozen_lockfile: frozen,
        };
        install(
            args(false),
            Some(selected.clone()),
            Some("production".into()),
        )
        .await
        .unwrap();
        let lock_path = project.path().join("selected.lock");
        let before = std::fs::read(&lock_path).unwrap();
        install(args(true), Some(selected.clone()), None)
            .await
            .unwrap();
        assert_eq!(std::fs::read(&lock_path).unwrap(), before);
        assert!(!project.path().join("ferrite.lock").exists());
        assert!(!unrelated.path().join(".ferrite").exists());
        assert!(project.path().join("node_modules").is_dir());
        let metadata_before = std::fs::read_dir(project.path()).unwrap().count();
        std::fs::write(&selected, "invalid selected TOML!").unwrap();
        assert!(install(args(false), Some(selected), None).await.is_err());
        assert_eq!(std::fs::read(&lock_path).unwrap(), before);
        assert_eq!(
            std::fs::read_dir(project.path()).unwrap().count(),
            metadata_before
        );
    }
}
