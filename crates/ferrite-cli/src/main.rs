//! Ferrite CLI: `dev`, `build`, `preview`, `ssr`, `add`, ... (spec §1, §80).

mod cli;
mod cmds;

use cli::{Cli, Command};

use clap::Parser;
use ferrite::plugin::Plugin;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(&cli.log_level);
    let result = match cli.command {
        Command::Dev(args) => cmds::dev::dev(args, cli.config, cli.mode, false).await,
        Command::Ssr(args) => cmds::dev::dev(args, cli.config, cli.mode, true).await,
        Command::Build(args) => cmds::build::build(args, cli.config, cli.mode).await,
        Command::Preview(args) => cmds::preview::preview(args, cli.config).await,
        Command::Add(args) => cmds::npm::add(args, cli.config, cli.mode).await,
        Command::Remove(args) => cmds::npm::remove(args, cli.config, cli.mode).await,
        Command::Update(args) => cmds::npm::update(args, cli.config, cli.mode).await,
        Command::Install(args) => cmds::npm::install(args, cli.config, cli.mode).await,
        Command::Doctor(args) => cmds::inspect::doctor(args, cli.config, cli.mode).await,
        Command::Inspect(args) => cmds::inspect::inspect(args, cli.config, cli.mode).await,
        Command::Transform(args) => cmds::inspect::transform(args, cli.config, cli.mode).await,
        Command::Migrate(args) => cmds::migrate::migrate(args).await,
        Command::Compat => cmds::compat::compat().await,
        Command::Clean => cmds::compat::clean().await,
        Command::Create(args) => cmds::create::create(args).await,
        Command::E2e(args) => cmds::e2e::e2e(args, cli.config, cli.mode).await,
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn init_logging(level: &str) {
    let filter = format!("ferrite={level},ferrite_cli={level}");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .try_init();
}

/// Default plugin set for CLI-driven runs.
fn default_plugins(root: &std::path::Path) -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(ferrite::plugin::RawTextPlugin),
        Arc::new(ferrite::wasm::RustWasmPlugin::new(None)),
        Arc::new(ferrite::frameworks::ReactPlugin::new()),
        Arc::new(ferrite::frameworks::VuePlugin::new(root.to_path_buf())),
        Arc::new(ferrite::frameworks::SveltePlugin::new(root.to_path_buf())),
        Arc::new(ferrite::docs::MarkdownPlugin::new(root.to_path_buf())),
        Arc::new(ferrite::tailwind::TailwindPlugin::new(root.to_path_buf())),
    ]
}

fn root_of(config: &Option<PathBuf>, root: PathBuf) -> PathBuf {
    if let Some(path) = config {
        if path.is_file() {
            return path
                .parent()
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        }
        return path.clone();
    }
    root
}
