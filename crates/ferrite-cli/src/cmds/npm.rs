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
    let user = ferrite::load_user_config(root).unwrap_or_default();
    let resolved = ferrite::resolve_config(
        user,
        Some(root.to_path_buf()),
        ferrite::CliOverrides::default(),
    )?;
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

pub(crate) async fn add(args: AddArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    for spec in &args.specs {
        let (name, range) = ferrite::npm::parse_spec(spec);
        let version = installer.install(&name, &range, &mut lock, false).await?;
        println!("added {name}@{version}");
        if args.dev {
            pkg.dev_dependencies.insert(name, format!("^{version}"));
        } else {
            pkg.dependencies.insert(name, format!("^{version}"));
        }
    }
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    Ok(())
}

pub(crate) async fn remove(args: RemoveArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    for name in &args.names {
        if lock.remove(name) {
            println!("removed {name}");
        } else {
            println!("{name} is not installed");
        }
        pkg.dependencies.remove(name);
        pkg.dev_dependencies.remove(name);
        // Best-effort store cleanup.
        if let Ok(entries) = std::fs::read_dir(&installer.store) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().into_owned();
                if file_name == *name || file_name.starts_with(&format!("{name}@")) {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
    }
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    Ok(())
}

pub(crate) async fn update(args: UpdateArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let mut pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    let targets: Vec<(String, String)> = if args.specs.is_empty() {
        pkg.dependencies
            .iter()
            .chain(pkg.dev_dependencies.iter())
            .map(|(name, range)| (name.clone(), range.clone()))
            .collect()
    } else {
        args.specs
            .iter()
            .map(|spec| ferrite::npm::parse_spec(spec))
            .collect()
    };
    for (name, range) in targets {
        lock.remove(&name); // force re-resolution
        let version = installer.install(&name, &range, &mut lock, false).await?;
        println!("updated {name}@{version}");
        if let Some(entry) = pkg.dependencies.get_mut(&name) {
            *entry = format!("^{version}");
        } else if let Some(entry) = pkg.dev_dependencies.get_mut(&name) {
            *entry = format!("^{version}");
        }
    }
    lock.write(&lock_path)?;
    pkg.write(&pkg_path)?;
    Ok(())
}

pub(crate) async fn install(args: InstallArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    let deps: Vec<(String, String)> = pkg
        .dependencies
        .iter()
        .chain(pkg.dev_dependencies.iter())
        .map(|(name, range)| (name.clone(), range.clone()))
        .collect();
    if deps.is_empty() && !lock.package.is_empty() {
        // No package.json: reinstall locked versions.
        for locked in lock.package.clone() {
            installer
                .install(&locked.name, &locked.version, &mut lock, false)
                .await?;
            println!("installed {}@{}", locked.name, locked.version);
        }
    } else {
        for (name, range) in deps {
            let version = installer.install(&name, &range, &mut lock, false).await?;
            println!("installed {name}@{version}");
        }
    }
    lock.write(&lock_path)?;
    Ok(())
}
