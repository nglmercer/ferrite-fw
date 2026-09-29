//! CLI argument definitions.

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "ferrite",
    version,
    about = "Rust-native SSR web toolchain (Vite-like DX, no Node required)"
)]
pub(crate) struct Cli {
    /// Config file or directory.
    #[arg(long, global = true)]
    pub(crate) config: Option<PathBuf>,
    /// Build mode.
    #[arg(long, global = true)]
    pub(crate) mode: Option<String>,
    /// Log level.
    #[arg(long, global = true, default_value = "info")]
    pub(crate) log_level: String,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)] // Parsed once at startup; size is irrelevant.
pub(crate) enum Command {
    /// Start the dev server.
    Dev(DevArgs),
    /// Production build.
    Build(BuildArgs),
    /// Preview a production build.
    Preview(PreviewArgs),
    /// Run with SSR enabled.
    Ssr(DevArgs),
    /// Add npm packages (no Node required).
    Add(AddArgs),
    /// Remove npm packages.
    Remove(RemoveArgs),
    /// Update npm packages.
    Update(UpdateArgs),
    /// Install locked/declared npm packages.
    Install(InstallArgs),
    /// Inspect resolved config, plugins, and graph.
    Inspect(InspectArgs),
    /// One-shot file transform.
    Transform(TransformArgs),
    /// Migrate a Vite config to ferrite.toml.
    Migrate(MigrateArgs),
    /// Report compatibility self-checks.
    Compat,
    /// Remove .ferrite/ caches.
    Clean,
    /// Scaffold a new app.
    Create(CreateArgs),
    /// Run end-to-end tests (Chromium, Rust-native, no Node required).
    E2e(E2eArgs),
}

#[derive(Debug, Args)]
pub(crate) struct DevArgs {
    /// Project root.
    #[arg(default_value = ".")]
    pub(crate) root: PathBuf,
    /// Bind host.
    #[arg(long)]
    pub(crate) host: Option<String>,
    /// Bind port.
    #[arg(long)]
    pub(crate) port: Option<u16>,
    /// Open a browser.
    #[arg(long)]
    pub(crate) open: bool,
    /// Disable HMR.
    #[arg(long)]
    pub(crate) no_hmr: bool,
    /// Embedded runtime backend (`auto`, `none`, `napi-vm`).
    #[arg(long)]
    pub(crate) runtime: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct BuildArgs {
    /// Project root.
    #[arg(default_value = ".")]
    pub(crate) root: PathBuf,
    /// Output directory.
    #[arg(long)]
    pub(crate) out_dir: Option<String>,
    /// Minify output.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub(crate) minify: bool,
    /// Produce the standalone scaffold (§51 stepping stone).
    #[arg(long)]
    pub(crate) standalone: bool,
    /// Cross-compilation target (used by the standalone scaffold).
    #[arg(long)]
    pub(crate) target: Option<String>,
    /// Only build one environment.
    #[arg(long)]
    pub(crate) env: Option<String>,
    /// Scope-hoist each entry closure into one file.
    #[arg(long)]
    pub(crate) scope_hoist: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PreviewArgs {
    /// Project root.
    #[arg(default_value = ".")]
    pub(crate) root: PathBuf,
    /// Bind port.
    #[arg(long, default_value_t = 4173)]
    pub(crate) port: u16,
}

#[derive(Debug, Args)]
pub(crate) struct AddArgs {
    /// Package specs (`react`, `three@latest`, `@scope/name@^1.0.0`).
    #[arg(required = true)]
    pub(crate) specs: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Save as dev dependency.
    #[arg(long)]
    pub(crate) dev: bool,
}

#[derive(Debug, Args)]
pub(crate) struct RemoveArgs {
    /// Package names.
    #[arg(required = true)]
    pub(crate) names: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    /// Packages to update (default: all).
    pub(crate) specs: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    /// Project root.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct InspectArgs {
    /// Project root.
    #[arg(default_value = ".")]
    pub(crate) root: PathBuf,
    /// Machine-readable JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct TransformArgs {
    /// File to transform.
    pub(crate) file: PathBuf,
    /// Project root.
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Write output here (default: stdout).
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
    /// Emit a source map (`.map` next to `--out`, or inline comment).
    #[arg(long)]
    pub(crate) sourcemap: bool,
}

#[derive(Debug, Args)]
pub(crate) struct MigrateArgs {
    /// Vite config to migrate.
    pub(crate) file: PathBuf,
    /// Write ferrite.toml here (default: stdout).
    #[arg(long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct CreateArgs {
    /// App name / directory.
    pub(crate) name: String,
    /// Template (`vanilla`, `ssr`).
    #[arg(long, default_value = "vanilla")]
    pub(crate) template: String,
}

/// Parse `--shard 1/3` into a 1-based (index, total).
fn parse_shard(raw: &str) -> Result<(usize, usize), String> {
    let (index, total) = raw
        .split_once('/')
        .ok_or_else(|| format!("invalid --shard {raw:?} (want 1-based index/total like 1/3)"))?;
    let index: usize = index
        .trim()
        .parse()
        .map_err(|_| format!("invalid --shard {raw:?} (index is not a number)"))?;
    let total: usize = total
        .trim()
        .parse()
        .map_err(|_| format!("invalid --shard {raw:?} (total is not a number)"))?;
    if index < 1 || index > total {
        return Err(format!(
            "invalid --shard {raw:?} (want a 1-based index within the total)"
        ));
    }
    Ok((index, total))
}

#[derive(Debug, Args)]
pub(crate) struct E2eArgs {
    /// Project root.
    #[arg(default_value = ".")]
    pub(crate) root: PathBuf,
    /// Run with a visible browser window.
    #[arg(long)]
    pub(crate) headed: bool,
    /// Browser engine (`chromium` or `firefox`).
    #[arg(long)]
    pub(crate) engine: Option<String>,
    /// Browser executable path.
    #[arg(long)]
    pub(crate) browser: Option<PathBuf>,
    /// Base URL for tests (overrides config).
    #[arg(long)]
    pub(crate) base_url: Option<String>,
    /// Use an already-running server at URL (boots nothing).
    #[arg(long)]
    pub(crate) url: Option<String>,
    /// Web server command to boot (overrides config).
    #[arg(long)]
    pub(crate) web_server: Option<String>,
    /// Reporter spec (`list`, `json`, `junit`, `html`, comma-separated).
    #[arg(long)]
    pub(crate) reporter: Option<String>,
    /// Retries per test.
    #[arg(long)]
    pub(crate) retries: Option<u32>,
    /// Parallel workers.
    #[arg(long)]
    pub(crate) workers: Option<usize>,
    /// Only run tests whose name contains this.
    #[arg(long)]
    pub(crate) filter: Option<String>,
    /// Only run tests whose name or tags contain this (ANDed with `--filter`).
    #[arg(long)]
    pub(crate) grep: Option<String>,
    /// Skip tests whose name or tags contain this.
    #[arg(long)]
    pub(crate) grep_invert: Option<String>,
    /// Run one shard (`1/3` = first third by name order).
    #[arg(long, value_parser = parse_shard)]
    pub(crate) shard: Option<(usize, usize)>,
    /// Video policy (`on`, `off`, `only-on-failure`).
    #[arg(long)]
    pub(crate) video: Option<String>,
    /// Snapshot update mode (`missing`, `all`, `none`).
    #[arg(long)]
    pub(crate) update_snapshots: Option<String>,
    /// Only verify the browser launches, then exit.
    #[arg(long)]
    pub(crate) check: bool,
    /// Test command to run (default: `cargo test --test e2e`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) command: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_parses_index_and_total() {
        assert_eq!(parse_shard("1/3"), Ok((1, 3)));
        assert_eq!(parse_shard("3/3"), Ok((3, 3)));
        assert!(parse_shard("shard").is_err());
        assert!(parse_shard("0/2").is_err());
        assert!(parse_shard("3/2").is_err());
        assert!(parse_shard("1/0").is_err());
        assert!(parse_shard("x/y").is_err());
    }
}
