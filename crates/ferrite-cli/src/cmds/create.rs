//! Transactional project generation through shared profiles and Ferrite installation.
use crate::cli::*;
use ferrite::frameworks::scaffold::{self, CreationTarget};
use std::path::Path;

pub(crate) async fn create(args: CreateArgs) -> ferrite::Result<()> {
    if args.list_templates {
        if args.name.is_some() || args.template.is_some() || args.framework.is_some() {
            return Err(ferrite::FerriteError::Config(
                "--list-templates cannot be combined with a destination or framework selection"
                    .into(),
            ));
        }
        for profile in ferrite::frameworks::registry::FRAMEWORKS
            .iter()
            .flat_map(|descriptor| descriptor.template_variants)
        {
            println!(
                "{}/{}/{} (compiler host: {})",
                profile.framework, profile.language, profile.rendering, profile.compiler_host
            );
        }
        return Ok(());
    }
    if args.framework.is_some() && args.template.is_some() && args.framework != args.template {
        return Err(ferrite::FerriteError::Config(
            "--framework and legacy --template select different owners".into(),
        ));
    }
    let framework = args
        .framework
        .as_deref()
        .or(args.template.as_deref())
        .unwrap_or("vanilla");
    let profile = scaffold::select(framework, &args.language, &args.rendering)?;
    let name = args.name.ok_or_else(|| {
        ferrite::FerriteError::Config(
            "create requires an app directory; use --list-templates to inspect profiles".into(),
        )
    })?;
    let target = CreationTarget::new(Path::new(&name))?;
    let package_name = target
        .path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let files = scaffold::files(profile, &package_name)?;
    if args.dry_run {
        println!(
            "would create {} ({framework}/{}/{})",
            target.path.display(),
            args.language,
            args.rendering
        );
        for file in files.keys() {
            println!("  {file}");
        }
        if !args.no_install {
            println!("  ferrite.lock (resolved through Ferrite)");
        }
        return Ok(());
    }
    let stage = target.stage()?;
    for (relative, contents) in files {
        let file = stage.path().join(relative);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, contents)?;
    }
    if !args.no_install {
        let (_, installer, lock_path, package_path) = super::npm::npm_context(stage.path())?;
        let package = ferrite::npm::JsPackageJson::read(&package_path)?;
        let mut lock = ferrite::npm::Lockfile::default();
        installer
            .install_manifest(&package, &mut lock, false)
            .await?;
        lock.write(&lock_path)?;
    }
    target.publish(&stage)?;
    println!(
        "created {} ({framework}/{}/{})",
        target.path.display(),
        args.language,
        args.rendering
    );
    println!("  cd {}", target.path.display());
    if args.no_install {
        println!("  ferrite install");
    }
    println!("  ferrite dev");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(path: &Path) -> CreateArgs {
        CreateArgs {
            name: Some(path.to_string_lossy().into_owned()),
            template: None,
            framework: None,
            language: "ts".into(),
            rendering: "client".into(),
            list_templates: false,
            no_install: false,
            dry_run: false,
        }
    }
    #[tokio::test]
    async fn dry_run_invalid_profiles_and_existing_destinations_never_write() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("app");
        let mut request = args(&path);
        request.dry_run = true;
        create(request).await.unwrap();
        assert!(!path.exists());
        let mut request = args(&path);
        request.framework = Some("vue".into());
        assert!(create(request).await.is_err());
        assert!(!path.exists());
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("owned"), "preserve").unwrap();
        assert!(create(args(&path)).await.is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("owned")).unwrap(),
            "preserve"
        );
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn create_resolves_lock_and_no_install_retains_a_real_manifest() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("installed");
        create(args(&path)).await.unwrap();
        let lock = ferrite::npm::Lockfile::read(&path.join("ferrite.lock")).unwrap();
        assert_eq!(lock.version, 2);
        assert!(lock.importers.contains_key("."));
        assert!(path.join("src/main.ts").exists());
        let other = root.path().join("uninstalled");
        let mut request = args(&other);
        request.no_install = true;
        request.language = "js".into();
        create(request).await.unwrap();
        assert!(!other.join("ferrite.lock").exists());
        assert!(other.join("src/main.js").exists());
        assert!(!other.join("tsconfig.json").exists());
        let package = ferrite::npm::JsPackageJson::read(&other.join("package.json")).unwrap();
        assert_eq!(package.rest["private"], true);
        assert!(std::fs::read_dir(root.path()).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ferrite-create-")));
    }
}
