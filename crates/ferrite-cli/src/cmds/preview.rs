//! Preview command.

use crate::cli::*;
use crate::default_plugins;
use crate::root_of;
use std::path::PathBuf;

pub(crate) async fn preview(args: PreviewArgs, config_arg: Option<PathBuf>) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let mut config = ferrite::Config {
        root: Some(root.clone()),
        overrides: ferrite::CliOverrides {
            port: Some(args.port),
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins(&root) {
        config.plugins.push(plugin);
    }
    ferrite::preview(config).await
}
