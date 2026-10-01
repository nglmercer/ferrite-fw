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
    let user = ferrite::load_user_config(root)?;
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
    Ok(())
}

pub(crate) async fn remove(args: RemoveArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
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
    for name in &args.names {
        println!("removed root dependency {name}");
    }
    Ok(())
}

pub(crate) async fn update(args: UpdateArgs) -> ferrite::Result<()> {
    let (client, installer, lock_path, pkg_path) = npm_context(&args.root)?;
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
    for (name, _) in targets {
        println!(
            "updated {name}@{}",
            lock.find(&name).expect("installed root").version
        );
    }
    Ok(())
}

pub(crate) async fn install(args: InstallArgs) -> ferrite::Result<()> {
    let (_, installer, lock_path, pkg_path) = npm_context(&args.root)?;
    if args.frozen_lockfile && !lock_path.exists() {
        return Err(ferrite::FerriteError::Npm(
            "frozen install requires ferrite.lock; run ferrite install first".into(),
        ));
    }
    let mut lock = ferrite::npm::Lockfile::read(&lock_path)?;
    let pkg = ferrite::npm::JsPackageJson::read(&pkg_path)?;
    installer
        .install_manifest(&pkg, &mut lock, args.frozen_lockfile)
        .await?;
    if !args.frozen_lockfile {
        lock.write(&lock_path)?;
    }
    println!("installed {} concrete packages", lock.package.len());
    Ok(())
}
