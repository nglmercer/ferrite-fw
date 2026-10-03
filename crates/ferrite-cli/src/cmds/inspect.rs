//! Inspect and transform commands.

use crate::cli::*;
use crate::default_plugins;
use crate::root_of;
use std::path::PathBuf;

pub(crate) async fn inspect(
    args: InspectArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = match &config_arg {
        Some(path) => ferrite::load_user_config_path(path)?,
        None => ferrite::load_user_config(&root)?,
    };
    let resolved = ferrite::resolve_config(
        user,
        Some(root.clone()),
        ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
    )?;
    let mut plugins: Vec<String> = default_plugins(&root)
        .iter()
        .map(|p| p.name().to_string())
        .collect();
    if let Some(profile) = &resolved.framework {
        plugins.retain(|plugin| plugin != ferrite::frameworks::ReactPlugin::NAME);
        for name in &profile.enabled {
            if name == "react" {
                plugins.push(ferrite::frameworks::ReactPlugin::NAME.into());
                continue;
            }
            plugins.retain(|plugin| plugin != &format!("ferrite:{name}"));
            plugins.push(format!("ferrite:{name}-official"));
        }
    }
    let lock = ferrite::npm::Lockfile::read(&resolved.lockfile())?;
    for _ in resolved.foreign_plugins.iter().flatten() {
        plugins.push("ferrite:foreign-hook".into());
    }
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "version": ferrite::VERSION,
                "root": resolved.root,
                "mode": resolved.mode,
                "base": resolved.base,
                "server": { "host": resolved.server.host, "port": resolved.server.port },
                "build": { "outDir": resolved.build.out_dir, "minify": resolved.build.minify, "target": resolved.build.target },
                "compiler": resolved.compiler.engine,
                "framework": resolved.framework,
                "foreignPlugins": resolved.foreign_plugins.iter().flatten().map(|profile| serde_json::json!({"name":profile.name, "entry":profile.entry, "host":profile.host, "node":profile.node, "timeoutMs":profile.timeout_ms.unwrap_or(10000), "support":"experimental", "executed":false})).collect::<Vec<_>>(),
                "react": resolved.react,
                "ssrRuntime": resolved.runtime.backend,
                "plugins": plugins,
                "lockfileVersion": lock.version,
                "importers": lock.importers,
                "lockedPackages": lock.package.iter().map(|p| p.id()).collect::<Vec<_>>(),
            })
        );
    } else {
        println!("ferrite v{}", ferrite::VERSION);
        println!("root:     {}", resolved.root.display());
        println!("mode:     {}", resolved.mode);
        println!(
            "server:   {}:{}",
            resolved.server.host, resolved.server.port
        );
        println!("out_dir:  {}", resolved.build.out_dir);
        println!("compiler: {}", resolved.compiler.engine);
        println!("plugins:  {}", plugins.join(", "));
        println!("framework compiler: {:?}", resolved.framework);
        println!("SSR runtime: {}", resolved.runtime.backend);
        println!("locked:   {} packages", lock.package.len());
        println!();
        println!(
            "live inspector: ferrite dev → http://127.0.0.1:{}/@ferrite/inspect",
            resolved.server.port
        );
    }
    Ok(())
}

// --- transform ---------------------------------------------------------------

pub(crate) async fn transform(
    args: TransformArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let (resolved, plugins) = ferrite::Config {
        root: Some(root.clone()),
        config_path: config_arg.clone(),
        plugins: default_plugins(&root),
        overrides: ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
        ..Default::default()
    }
    .resolve()
    .await?;
    let file = if args.file.is_absolute() {
        args.file.clone()
    } else {
        root.join(&args.file)
    };
    if !file.exists() {
        return Err(ferrite::FerriteError::Resolve(format!(
            "cannot read `{}`",
            file.display()
        )));
    }
    let url = ferrite::core::file_to_url(&root, &file);
    let server = ferrite::DevServer::new_without_watcher(resolved, plugins).await?;
    let module = server
        .pipeline_module(&ferrite::ModuleId::new(url), None, "client")
        .await?;
    if module.is_raw_bytes {
        println!(
            "note: binary asset ({} bytes on disk)",
            std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0)
        );
        return Ok(());
    }
    let mut output = module.code;
    if args.sourcemap {
        if let Some(map) = module.map {
            output.push_str(&ferrite::core::SourceMap::inline_comment_json(&map));
        }
    }
    match args.out {
        Some(path) => {
            std::fs::write(&path, output)?;
            println!("wrote {}", path.display());
        }
        None => print!("{output}"),
    }
    Ok(())
}

pub(crate) async fn doctor(
    args: InspectArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = match &config_arg {
        Some(path) => ferrite::load_user_config_path(path)?,
        None => ferrite::load_user_config(&root)?,
    };
    let config = ferrite::resolve_config(
        user,
        Some(root),
        ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
    )?;
    let report = ferrite::frameworks::doctor::inspect(&config)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("framework registry schema {}", report.schema_version);
        println!(
            "compiler: {}; SSR runtime: {}",
            report.compiler, report.ssr_runtime
        );
        println!(
            "Node: {} (not a compiler execution test)",
            report.node_probe
        );
        println!("editor packages: {}", report.editor_view);
        for profile in &report.foreign_plugins {
            println!("foreign plugin: {profile}");
        }
        for framework in &report.frameworks {
            if framework.active {
                println!(
                    "{}@{}: host {}, client {}, SSR {}, checker {}, updates {}",
                    framework.framework,
                    framework.version.as_deref().unwrap_or("unresolved/builtin"),
                    framework.compiler_host.as_deref().unwrap_or("disabled"),
                    framework.client,
                    framework.ssr,
                    framework.checker,
                    framework.updates
                );
            }
        }
        for issue in &report.issues {
            println!("{}: {}; {}", issue.severity, issue.message, issue.action);
        }
    }
    if report.issues.iter().any(|issue| issue.severity == "error") {
        return Err(ferrite::FerriteError::Config(
            "doctor found unavailable project capabilities; follow the reported actions".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod config_tests {
    #[tokio::test]
    async fn selected_file_controls_library_pipeline_without_default_discovery() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ferrite.toml"), "invalid TOML!").unwrap();
        let selected = dir.path().join("selected.toml");
        std::fs::write(
            &selected,
            "[server]\nport = 4321\n[framework]\nenabled = []\n[define]\nSELECTED = '42'\n",
        )
        .unwrap();
        let (resolved, _) = ferrite::Config {
            config_path: Some(selected.clone()),
            ..Default::default()
        }
        .resolve()
        .await
        .unwrap();
        assert_eq!(resolved.root, dir.path());
        assert_eq!(resolved.server.port, 4321);
        assert_eq!(resolved.define["SELECTED"], "42");
        assert!(resolved.framework.unwrap().enabled.is_empty());
        std::fs::write(&selected, "invalid selected TOML!").unwrap();
        assert!(ferrite::Config {
            config_path: Some(selected),
            ..Default::default()
        }
        .resolve()
        .await
        .is_err());
    }
}
