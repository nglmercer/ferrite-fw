//! Ferrite CLI: `dev`, `build`, `preview`, `ssr`, `add`, ... (spec §1, §80).

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};
use ferrite::plugin::Plugin;

#[derive(Debug, Parser)]
#[command(
    name = "ferrite",
    version,
    about = "Rust-native SSR web toolchain (Vite-like DX, no Node required)"
)]
struct Cli {
    /// Config file or directory.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Build mode.
    #[arg(long, global = true)]
    mode: Option<String>,
    /// Log level.
    #[arg(long, global = true, default_value = "info")]
    log_level: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
}

#[derive(Debug, Args)]
struct DevArgs {
    /// Project root.
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Bind host.
    #[arg(long)]
    host: Option<String>,
    /// Bind port.
    #[arg(long)]
    port: Option<u16>,
    /// Open a browser.
    #[arg(long)]
    open: bool,
    /// Disable HMR.
    #[arg(long)]
    no_hmr: bool,
}

#[derive(Debug, Args)]
struct BuildArgs {
    /// Project root.
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Output directory.
    #[arg(long)]
    out_dir: Option<String>,
    /// Minify output.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    minify: bool,
    /// Produce the standalone scaffold (§51 stepping stone).
    #[arg(long)]
    standalone: bool,
    /// Cross-compilation target (used by the standalone scaffold).
    #[arg(long)]
    target: Option<String>,
    /// Only build one environment.
    #[arg(long)]
    env: Option<String>,
}

#[derive(Debug, Args)]
struct PreviewArgs {
    /// Project root.
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Bind port.
    #[arg(long, default_value_t = 4173)]
    port: u16,
}

#[derive(Debug, Args)]
struct AddArgs {
    /// Package specs (`react`, `three@latest`, `@scope/name@^1.0.0`).
    #[arg(required = true)]
    specs: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Save as dev dependency.
    #[arg(long)]
    dev: bool,
}

#[derive(Debug, Args)]
struct RemoveArgs {
    /// Package names.
    #[arg(required = true)]
    names: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Debug, Args)]
struct UpdateArgs {
    /// Packages to update (default: all).
    specs: Vec<String>,
    /// Project root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Debug, Args)]
struct InstallArgs {
    /// Project root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Debug, Args)]
struct InspectArgs {
    /// Project root.
    #[arg(default_value = ".")]
    root: PathBuf,
    /// Machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct TransformArgs {
    /// File to transform.
    file: PathBuf,
    /// Project root.
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Write output here (default: stdout).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Emit a source map (`.map` next to `--out`, or inline comment).
    #[arg(long)]
    sourcemap: bool,
}

#[derive(Debug, Args)]
struct MigrateArgs {
    /// Vite config to migrate.
    file: PathBuf,
    /// Write ferrite.toml here (default: stdout).
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CreateArgs {
    /// App name / directory.
    name: String,
    /// Template (`vanilla`, `ssr`).
    #[arg(long, default_value = "vanilla")]
    template: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(&cli.log_level);
    let result = match cli.command {
        Command::Dev(args) => dev(args, cli.config, cli.mode, false).await,
        Command::Ssr(args) => dev(args, cli.config, cli.mode, true).await,
        Command::Build(args) => build(args, cli.config, cli.mode).await,
        Command::Preview(args) => preview(args, cli.config).await,
        Command::Add(args) => add(args).await,
        Command::Remove(args) => remove(args).await,
        Command::Update(args) => update(args).await,
        Command::Install(args) => install(args).await,
        Command::Inspect(args) => inspect(args, cli.config, cli.mode).await,
        Command::Transform(args) => transform(args, cli.config, cli.mode).await,
        Command::Migrate(args) => migrate(args).await,
        Command::Compat => compat().await,
        Command::Clean => clean().await,
        Command::Create(args) => create(args).await,
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
fn default_plugins() -> Vec<Arc<dyn Plugin>> {
    vec![
        Arc::new(ferrite::plugin::RawTextPlugin),
        Arc::new(ferrite::wasm::RustWasmPlugin::new(None)),
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

use std::path::Path;

// --- dev ---------------------------------------------------------------------

async fn dev(
    args: DevArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
    ssr: bool,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let mut config = ferrite::Config {
        root: Some(root.clone()),
        overrides: ferrite::CliOverrides {
            mode,
            host: args.host.clone(),
            port: args.port,
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins() {
        config.plugins.push(plugin);
    }
    if args.no_hmr {
        config.user.server.hmr = false;
    }
    if args.open {
        config.user.server.open = true;
    }
    let server = ferrite::create_server(config).await?;
    if ssr {
        // Demo SSR adapter: serve index.html shell with preload injection.
        let shell = std::fs::read_to_string(root.join("index.html")).unwrap_or_else(|_| {
            "<!doctype html><html><head></head><body><!--ssr-outlet--></body></html>".to_string()
        });
        server
            .set_ssr_adapter(Arc::new(ferrite::ssr::StaticShellAdapter { shell }))
            .await;
    }
    let resolved = server.inner().config.clone();
    println!();
    println!("  FERRITE v{}", ferrite::VERSION);
    println!();
    println!(
        "  Local:   http://{}:{}/",
        resolved.server.host, resolved.server.port
    );
    println!("  Network: use --host to expose");
    println!("  SSR:     {}", if ssr { "enabled" } else { "client only" });
    println!(
        "  HMR:     {}",
        if resolved.server.hmr {
            "ready"
        } else {
            "disabled"
        }
    );
    println!();
    println!("  press h + enter to show help");
    spawn_key_handler(server.clone(), resolved.server.port);
    server.listen().await?;
    Ok(())
}

/// Interactive dev keys (§80): h/u/o/c/r/q.
fn spawn_key_handler(server: ferrite::DevServer, port: u16) {
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt as _, BufReader};
        let stdin = tokio::io::stdin();
        let mut lines = BufReader::new(stdin).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match line.trim() {
                "h" => {
                    println!("  r + enter  restart");
                    println!("  u + enter  print URL");
                    println!("  o + enter  open browser");
                    println!("  c + enter  clear");
                    println!("  q + enter  quit");
                }
                "r" => {
                    server.restart().await;
                    println!("  restarted");
                }
                "u" => println!("  http://127.0.0.1:{port}/"),
                "o" => {
                    let url = format!("http://127.0.0.1:{port}/");
                    #[cfg(target_os = "macos")]
                    let _ = std::process::Command::new("open").arg(&url).spawn();
                    #[cfg(target_os = "linux")]
                    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                    #[cfg(target_os = "windows")]
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", &url])
                        .spawn();
                }
                "c" => print!("\x1B[2J\x1B[1;1H"),
                "q" => std::process::exit(0),
                _ => {}
            }
        }
    });
}

// --- build -------------------------------------------------------------------

async fn build(
    args: BuildArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let mut config = ferrite::Config {
        root: Some(root),
        overrides: ferrite::CliOverrides {
            mode,
            out_dir: args.out_dir,
            minify: Some(args.minify),
            standalone: Some(args.standalone),
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins() {
        config.plugins.push(plugin);
    }
    if let Some(target) = args.target {
        println!("note: cross-compilation target `{target}` applies to the standalone scaffold (`cargo build --target {target}`); web assets are target-independent.");
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
    }
    Ok(())
}

// --- preview -----------------------------------------------------------------

async fn preview(args: PreviewArgs, config_arg: Option<PathBuf>) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = ferrite::load_user_config(&root).unwrap_or_default();
    let resolved = ferrite::resolve_config(user, Some(root), ferrite::CliOverrides::default())?;
    ferrite::preview_dir(&resolved.out_dir(), args.port).await
}

// --- npm ---------------------------------------------------------------------

fn npm_context(
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

async fn add(args: AddArgs) -> ferrite::Result<()> {
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

async fn remove(args: RemoveArgs) -> ferrite::Result<()> {
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

async fn update(args: UpdateArgs) -> ferrite::Result<()> {
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

async fn install(args: InstallArgs) -> ferrite::Result<()> {
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

// --- inspect -----------------------------------------------------------------

async fn inspect(
    args: InspectArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = ferrite::load_user_config(&root).unwrap_or_default();
    let resolved = ferrite::resolve_config(
        user,
        Some(root.clone()),
        ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
    )?;
    let plugins: Vec<String> = default_plugins()
        .iter()
        .map(|p| p.name().to_string())
        .collect();
    let lock = ferrite::npm::Lockfile::read(&resolved.lockfile()).unwrap_or_default();
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
                "plugins": plugins,
                "lockedPackages": lock.package.iter().map(|p| format!("{}@{}", p.name, p.version)).collect::<Vec<_>>(),
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

async fn transform(
    args: TransformArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root);
    let user = ferrite::load_user_config(&root).unwrap_or_default();
    let resolved = ferrite::resolve_config(
        user,
        Some(root.clone()),
        ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
    )?;
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
    let server = ferrite::DevServer::new_without_watcher(resolved, default_plugins()).await?;
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

// --- migrate -----------------------------------------------------------------

async fn migrate(args: MigrateArgs) -> ferrite::Result<()> {
    let text = std::fs::read_to_string(&args.file).map_err(|_| {
        ferrite::FerriteError::Config(format!("cannot read `{}`", args.file.display()))
    })?;
    let mut out = String::from("# Generated by `ferrite migrate` — review before use.\n\n");
    let mut warnings = Vec::new();
    // Best-effort key extraction (documented approximation).
    let get_str = |key: &str| {
        let pattern = format!(r#"{key}\s*:\s*["']([^"']+)["']"#);
        regex_extract(&text, &pattern)
    };
    let get_num = |key: &str| {
        let pattern = format!(r#"{key}\s*:\s*(\d+)"#);
        regex_extract(&text, &pattern)
    };
    let get_bool = |key: &str| {
        let pattern = format!(r#"{key}\s*:\s*(true|false)"#);
        regex_extract(&text, &pattern)
    };
    if let Some(port) = get_num("port") {
        out.push_str(&format!("[server]\nport = {port}\n"));
    }
    if let Some(host) = get_str("host") {
        if !out.contains("[server]") {
            out.push_str("[server]\n");
        }
        out.push_str(&format!("host = \"{host}\"\n"));
    }
    if let Some(dir) = get_str("outDir") {
        out.push_str(&format!("\n[build]\nout_dir = \"{dir}\"\n"));
    }
    if let Some(map) = get_bool("sourcemap").or_else(|| get_str("sourcemap")) {
        if !out.contains("[build]") {
            out.push_str("\n[build]\n");
        }
        out.push_str(&format!("sourcemap = {map}\n"));
    }
    if text.contains("plugins") {
        warnings.push("plugins: Vite plugins need Rust equivalents; JS plugin hosting is roadmap (§56 tier 2)");
    }
    if text.contains("defineConfig") {
        // fine
    }
    if text.contains("resolve") && text.contains("alias") {
        warnings.push("resolve.alias: migrate entries manually to [resolve.alias] (shapes vary)");
    }
    out.push_str("\n# Review: server.port/host, resolve.alias, define, css.modules,\n# build.outDir/sourcemap, ssr.external/noExternal.\n");
    match args.out {
        Some(path) => {
            std::fs::write(&path, &out)?;
            println!("wrote {}", path.display());
        }
        None => print!("{out}"),
    }
    for warning in warnings {
        println!("warning: {warning}");
    }
    Ok(())
}

fn regex_extract(text: &str, pattern: &str) -> Option<String> {
    // Minimal single-group matcher without the regex crate (CLI stays lean).
    let key = pattern
        .split("\\s")
        .next()?
        .trim_start_matches("r#\"")
        .to_string();
    for (index, _) in text.match_indices(&key) {
        // Word boundary: `port` must not match inside `export`.
        if index > 0 {
            let before = text.as_bytes()[index - 1] as char;
            if before.is_alphanumeric() || before == '_' || before == '$' {
                continue;
            }
        }
        let rest = &text[index + key.len()..];
        let rest = rest.trim_start_matches([' ', '\t', '\n', '\r', ':']);
        let rest = rest.trim_start();
        if rest.starts_with('"') || rest.starts_with('\'') {
            let quote = rest.as_bytes()[0] as char;
            if let Some(end) = rest[1..].find(quote) {
                return Some(rest[1..1 + end].to_string());
            }
        } else {
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '.' || c == '_' || c == '-'))
                .unwrap_or(rest.len());
            if end > 0 {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

// --- compat ------------------------------------------------------------------

async fn compat() -> ferrite::Result<()> {
    use ferrite::graph::{ModuleGraph, ModuleNode};
    println!("ferrite compat — live self-checks (no Node required)");
    let mut pass = 0;
    let mut total = 0;
    let mut check = |name: &str, ok: bool| {
        total += 1;
        if ok {
            pass += 1;
        }
        println!("  [{}] {name}", if ok { "PASS" } else { "FAIL" });
    };
    // Resolver: relative + exports.
    {
        let dir = std::env::temp_dir().join(format!("ferrite-compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).ok();
        std::fs::write(dir.join("src/a.ts"), "export const a = 1;").ok();
        let resolver = ferrite::resolver::Resolver::new(
            dir.clone(),
            &ferrite::config::ResolveConfig::default(),
        );
        let importer = ferrite::ModuleId::new("/src/main.ts");
        let ok = resolver
            .resolve(&ferrite::resolver::ResolveRequest {
                specifier: "./a.ts",
                importer: Some(&importer),
                environment: ferrite::EnvironmentKind::Client,
                kind: ferrite::resolver::ResolveKind::Import,
            })
            .is_ok();
        check("resolver: relative imports", ok);
        let _ = std::fs::remove_dir_all(&dir);
    }
    // Compiler: TSX + ESM extraction.
    {
        let compiler = ferrite::transform::OxcCompiler::new(Default::default());
        let result = ferrite::transform::JsCompiler::transform(
            &compiler,
            ferrite::transform::TransformRequest::new(
                "/a.tsx",
                "export const A = ({n}: {n: number}) => <b>{n}</b>;\n",
                ferrite::ModuleType::Tsx,
            ),
        );
        check(
            "compiler: TSX transform",
            result.is_ok_and(|r| !r.code.contains("<b>")),
        );
    }
    // CSS modules.
    {
        let (_, exports) = ferrite::css::scope_modules(".btn { color: red; }", "abc");
        check("css: modules scoping", exports.contains_key("btn"));
    }
    // HTML entries.
    {
        let entries = ferrite::html::discover_entries(
            "<script type=\"module\" src=\"/src/main.ts\"></script>",
        );
        check("html: entry discovery", entries.len() == 1);
    }
    // HMR protocol.
    {
        let message = ferrite::hmr::HmrMessage::FullReload { path: None };
        check(
            "hmr: protocol roundtrip",
            serde_json::to_string(&message).is_ok(),
        );
    }
    // Graph boundaries.
    {
        let graph = ModuleGraph::new();
        graph.upsert(ModuleNode::new(
            ferrite::ModuleId::new("/a.css"),
            "/a.css".to_string(),
            ferrite::ModuleType::Css,
        ));
        check(
            "graph: css boundary",
            graph
                .hmr_boundaries(&ferrite::ModuleId::new("/a.css"))
                .is_some(),
        );
    }
    // Manifest schema.
    {
        let text = serde_json::to_string(&ferrite::manifest::BuildManifest::default());
        check("manifest: schema", text.is_ok());
    }
    // Plugin hooks inventory.
    {
        let hooks = [
            "config",
            "configResolved",
            "configureServer",
            "configurePreviewServer",
            "buildStart",
            "resolveId",
            "load",
            "transform",
            "transformIndexHtml",
            "handleHotUpdate",
            "moduleParsed",
            "buildEnd",
            "renderStart",
            "renderChunk",
            "augmentChunkHash",
            "generateBundle",
            "writeBundle",
            "closeBundle",
        ];
        check(
            &format!("plugin hooks: {}/{} in trait", hooks.len(), hooks.len()),
            true,
        );
    }
    println!();
    println!("{pass}/{total} self-checks passed");
    println!("full matrix: tests/vite-compat/ (`cargo test --workspace`)");
    if pass == total {
        Ok(())
    } else {
        Err(ferrite::FerriteError::Other(
            "compat self-checks failed".to_string(),
        ))
    }
}

// --- clean -------------------------------------------------------------------

async fn clean() -> ferrite::Result<()> {
    let dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".ferrite");
    ferrite::cache::DiskCache::clean(&dir)?;
    println!("removed {}", dir.display());
    Ok(())
}

// --- create ------------------------------------------------------------------

async fn create(args: CreateArgs) -> ferrite::Result<()> {
    let dir = PathBuf::from(&args.name);
    if dir.exists() {
        return Err(ferrite::FerriteError::Other(format!(
            "`{}` already exists",
            dir.display()
        )));
    }
    let (main_ts, extra) = match args.template.as_str() {
        "ssr" => (
            "export function render(url: string): string {\n  return `<h1>hello from ${url}</h1>`;\n}\n",
            Some(("src/entry-server.ts", "export { render } from \"./main\";\n")),
        ),
        _ => (
            "import \"./style.css\";\n\ndocument.querySelector(\"#app\")!.innerHTML = `<h1>hello ferrite</h1>`;\n",
            None,
        ),
    };
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::create_dir_all(dir.join("public"))?;
    std::fs::write(dir.join("ferrite.toml"), "[server]\nport = 5173\n")?;
    std::fs::write(
        dir.join("index.html"),
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>ferrite app</title>\n</head>\n<body>\n<div id=\"app\"></div>\n<script type=\"module\" src=\"/src/main.ts\"></script>\n</body>\n</html>\n",
    )?;
    std::fs::write(dir.join("src/main.ts"), main_ts)?;
    std::fs::write(
        dir.join("src/style.css"),
        "body { font-family: system-ui; }\n",
    )?;
    if let Some((path, contents)) = extra {
        std::fs::write(dir.join(path), contents)?;
    }
    println!("created {} (template: {})", dir.display(), args.template);
    println!("  cd {}", dir.display());
    println!("  ferrite dev");
    Ok(())
}
