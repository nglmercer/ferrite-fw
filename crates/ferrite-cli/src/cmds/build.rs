//! Build command.

use crate::cli::*;
use crate::default_plugins;
use crate::root_of;
use std::path::PathBuf;

pub(crate) async fn build(
    args: BuildArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let mut config = ferrite::Config {
        root: Some(root.clone()),
        config_path: config_arg.clone(),
        overrides: ferrite::CliOverrides {
            mode,
            out_dir: args.out_dir,
            minify: Some(args.minify),
            standalone: Some(args.standalone),
            target: args.target.clone(),
            scope_hoist: args.scope_hoist.then_some(true),
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins(&root) {
        config.plugins.push(plugin);
    }
    if args.target.is_some() && !args.standalone {
        println!("note: `--target` only affects `--standalone` builds; web assets are target-independent.");
    }
    let builder = ferrite::create_builder(config).await?;
    let reports = match args.env.as_deref() {
        Some(env) => vec![builder.build(env).await?],
        None => builder.build_app().await?,
    };
    for report in reports {
        println!(
            "built {}: {} files, {} modules ({} dropped), {} bytes → {}",
            report.env,
            report.chunks,
            report.modules,
            report.dropped,
            report.bytes,
            report.out_dir.display()
        );
        println!("  entries: {}", report.entries.join(", "));
        if let Some(binary) = &report.standalone_binary {
            println!("  standalone: {}", binary.display());
        }
    }
    Ok(())
}
