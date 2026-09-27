//! Dev server command.

use crate::cli::*;
use crate::default_plugins;
use crate::root_of;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) async fn dev(
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
            runtime: args.runtime,
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins(&root) {
        config.plugins.push(plugin);
    }
    if args.no_hmr {
        config.user.server.hmr = false;
    }
    if args.open {
        config.user.server.open = true;
    }
    let server = ferrite::create_server(config).await?;
    let resolved = server.inner().config.clone();
    let mut ssr_mode = String::from("client only");
    if ssr {
        // SSR adapter: napi-vm entry-server when that backend is selected,
        // else the index.html shell with preload injection.
        let shell = std::fs::read_to_string(root.join("index.html")).unwrap_or_else(|_| {
            "<!doctype html><html><head></head><body><!--ssr-outlet--></body></html>".to_string()
        });
        if resolved.runtime.backend == "napi-vm" {
            // Resolved root is absolute; embedded runtimes cannot `import`
            // relative module ids.
            match js_ssr_adapter(&server, &resolved, &resolved.root, &shell) {
                Ok(adapter) => {
                    server.set_ssr_adapter(adapter).await;
                    ssr_mode = String::from("enabled (napi-vm entry-server)");
                }
                Err(note) => {
                    println!("note: {note}; using static shell");
                    server
                        .set_ssr_adapter(Arc::new(ferrite::ssr::StaticShellAdapter { shell }))
                        .await;
                    ssr_mode = String::from("enabled (static shell)");
                }
            }
        } else {
            server
                .set_ssr_adapter(Arc::new(ferrite::ssr::StaticShellAdapter { shell }))
                .await;
            ssr_mode = String::from("enabled (static shell)");
        }
    }
    println!();
    println!("  FERRITE v{}", ferrite::VERSION);
    println!();
    println!(
        "  Local:   http://{}:{}/",
        resolved.server.host, resolved.server.port
    );
    println!("  Network: use --host to expose");
    println!("  SSR:     {ssr_mode}");
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
pub(crate) fn spawn_key_handler(server: ferrite::DevServer, port: u16) {
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

/// Build a napi-vm SSR adapter from `src/entry-server.*` (§46).
///
/// Returns a human-readable reason (not a hard error) when no entry exists
/// or cannot load, so the caller can fall back to the static shell.
pub(crate) fn js_ssr_adapter(
    server: &ferrite::DevServer,
    resolved: &ferrite::ResolvedConfig,
    root: &std::path::Path,
    shell: &str,
) -> Result<Arc<dyn ferrite::ssr::SsrAdapter>, String> {
    const CANDIDATES: [&str; 8] = ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
    let mut found = None;
    for ext in CANDIDATES {
        let path = root.join(format!("src/entry-server.{ext}"));
        if path.is_file() {
            found = Some(path);
            break;
        }
    }
    let path = found.ok_or_else(|| "no src/entry-server.* found".to_string())?;
    let code = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let ssr_env = resolved.ssr_env();
    let result = server
        .inner()
        .compiler
        .transform(ferrite::transform::TransformRequest {
            id: path.to_string_lossy().into_owned(),
            code,
            module_type: ferrite::ModuleType::from_path(&path),
            environment: ferrite::EnvironmentKind::Ssr,
            ssr: true,
            target: ssr_env.target.clone(),
            minify: false,
            sourcemap: false,
            define: ssr_env.define.clone(),
            jsx_runtime: resolved.react.runtime.clone(),
            development: !resolved.is_production,
        })
        .map_err(|error| format!("cannot transform {}: {error}", path.display()))?;
    let module = ferrite::runtime::CompiledModule {
        id: path.to_string_lossy().into_owned(),
        code: result.code,
        url: None,
    };
    Ok(Arc::new(
        ferrite::ssr::JsSsrAdapter::from_resolved(resolved, module).with_shell(shell.to_string()),
    ))
}
