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
    let user = ferrite::load_user_config(&root)?;
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
        for name in &profile.enabled {
            plugins.retain(|plugin| plugin != &format!("ferrite:{name}"));
            plugins.push(format!("ferrite:{name}-official"));
        }
    }
    let lock = ferrite::npm::Lockfile::read(&resolved.lockfile())?;
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
