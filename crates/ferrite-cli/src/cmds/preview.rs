//! Preview command.

use crate::cli::*;
use crate::root_of;
use std::path::PathBuf;

pub(crate) async fn preview(args: PreviewArgs, config_arg: Option<PathBuf>) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = ferrite::load_user_config(&root).unwrap_or_default();
    let resolved = ferrite::resolve_config(user, Some(root), ferrite::CliOverrides::default())?;
    ferrite::preview_dir(&resolved.out_dir(), args.port).await
}
