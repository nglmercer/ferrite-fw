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
        config_path: config_arg.clone(),
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
        if resolved.runtime.backend != "napi-vm" {
            return Err(ferrite::FerriteError::Other(format!(
                "SSR requires a renderer-capable runtime; selected backend `{}` cannot execute entry-server. Select --runtime napi-vm with the napi-vm feature, or omit --ssr for client serving",
                resolved.runtime.backend
            )));
        }
        let shell = server.transform_index_html("/index.html").await?;
        let adapter = js_ssr_adapter(&server, &resolved, &shell)
            .await
            .map_err(|note| {
                ferrite::FerriteError::Other(format!("SSR initialization failed: {note}"))
            })?;
        server.set_ssr_adapter(adapter).await;
        ssr_mode = String::from("enabled (napi-vm entry-server)");
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
    if let Some(profile) = &resolved.framework {
        if !profile.enabled.is_empty() {
            println!(
                "  Compiler host: {} ({}, experimental)",
                profile.compiler_host.as_deref().unwrap_or("unavailable"),
                profile.enabled.join(", ")
            );
        }
    }
    println!("  SSR runtime: {}", resolved.runtime.backend);

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

/// Build a napi-vm SSR adapter from the shared configured server-entry selection.
/// The explicit SSR caller propagates selection/compilation failures.
pub(crate) async fn js_ssr_adapter(
    server: &ferrite::DevServer,
    resolved: &ferrite::ResolvedConfig,
    shell: &str,
) -> Result<Arc<dyn ferrite::ssr::SsrAdapter>, String> {
    ferrite::create_dev_ssr_adapter(server, resolved, shell)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(all(test, feature = "napi-vm"))]
mod ssr_tests {
    use super::*;

    #[tokio::test]
    async fn development_ssr_delivers_scoped_styles_and_updates_css() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/entry-server.js"), "import classes from './style.module.css'; export function render() { return `<h1 class=\"${classes.title}\">styled</h1>`; }").unwrap();
        let css_file = root.path().join("src/style.module.css");
        std::fs::write(&css_file, ".title { color: red; }").unwrap();
        let mut resolved = ferrite::config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        resolved.runtime.backend = "napi-vm".into();
        resolved.server.host = "127.0.0.1".into();
        resolved.server.port = 0;
        let server = ferrite::DevServer::new_without_watcher(resolved.clone(), Vec::new())
            .await
            .unwrap();
        let adapter = js_ssr_adapter(
            &server,
            &resolved,
            "<html><head></head><body><!--ssr-outlet--></body></html>",
        )
        .await
        .unwrap();
        server.set_ssr_adapter(adapter).await;
        let listening = server.clone();
        let serving = tokio::spawn(async move { listening.listen().await });
        let address = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Some(address) = *server.inner().bound_addr.lock().unwrap() {
                    break address;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("server must bind");
        let origin = format!("http://{address}");
        for color in ["red", "blue"] {
            std::fs::write(&css_file, format!(".title {{ color: {color}; }}")).unwrap();
            let response = server
                .inner()
                .http_client
                .get(&origin)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            let html = response.text().await.unwrap();
            assert!(html.contains("rel=\"stylesheet\""), "{html}");
            let href = html
                .split("href=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            assert_eq!(href, "/src/style.module.css?direct");
            let class = html
                .split("class=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let response = server
                .inner()
                .http_client
                .get(format!("{origin}{href}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200);
            assert!(response.headers()["content-type"]
                .to_str()
                .unwrap()
                .contains("text/css"));
            let css = response.text().await.unwrap();
            assert!(css.contains(class) && css.contains(color), "{html} / {css}");
            assert!(
                !css.contains("document") && !css.contains("import.meta.hot"),
                "{css}"
            );
        }
        serving.abort();
        let _ = serving.await;
        server.close();
    }

    #[tokio::test]
    async fn shared_ssr_compiles_typescript_dependencies_and_reports_errors() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/entry-server.ts"),
            "import { label } from './label.ts'; export function render(url: string, request: { method: string; body: number[] }) { if (request.method === 'POST') return { html: `<h1>${label} byte=${request.body[0]}</h1>`, status: 202, headers: [['x-renderer', 'compiled']] }; return `<h1>${label} ${url}</h1>`; }").unwrap();
        let dependency = root.path().join("src/label.ts");
        std::fs::write(&dependency, "export const label: string = 'compiled';").unwrap();
        let mut resolved = ferrite::config::resolve_config(
            Default::default(),
            Some(root.path().into()),
            Default::default(),
        )
        .unwrap();
        resolved.runtime.backend = "napi-vm".into();
        resolved.server.host = "127.0.0.1".into();
        resolved.server.port = 0;
        let server = ferrite::DevServer::new_without_watcher(resolved.clone(), Vec::new())
            .await
            .unwrap();
        let adapter = js_ssr_adapter(&server, &resolved, "<!--ssr-outlet-->")
            .await
            .unwrap();
        let response = adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/about".into(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                Default::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.into_string().await.unwrap(),
            "<h1>compiled /about</h1>"
        );
        server.set_ssr_adapter(adapter.clone()).await;
        let listening = server.clone();
        let serving = tokio::spawn(async move { listening.listen().await });
        let address = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if let Some(address) = *server.inner().bound_addr.lock().unwrap() {
                    break address;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("server must bind");
        let url = format!("http://{address}/");
        let response = server.inner().http_client.get(&url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(response.text().await.unwrap(), "<h1>compiled /</h1>");
        let response = server
            .inner()
            .http_client
            .post(&url)
            .body(vec![255, 0, 128])
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 202);
        assert_eq!(response.headers()["x-renderer"], "compiled");
        assert_eq!(response.text().await.unwrap(), "<h1>compiled byte=255</h1>");
        std::fs::write(&dependency, "export const label: string = ;").unwrap();
        let response = server.inner().http_client.get(&url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 500);
        assert!(response.text().await.unwrap().contains("label.ts"));
        let error = adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/".into(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                Default::default(),
            )
            .await
            .expect_err("broken dependency must fail")
            .to_string();
        assert!(error.contains("label.ts"), "{error}");
        std::fs::write(&dependency, "export const label: string = 'recovered';").unwrap();
        let response = adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/".into(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                Default::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.into_string().await.unwrap(),
            "<h1>recovered /</h1>"
        );
        let response = server.inner().http_client.get(&url).send().await.unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(response.text().await.unwrap(), "<h1>recovered /</h1>");
        serving.abort();
        let _ = serving.await;
        drop(server);
        let error = adapter
            .render(
                ferrite::ssr::SsrHttpRequest {
                    method: "GET".into(),
                    uri: "/".into(),
                    headers: Vec::new(),
                    body: Vec::new(),
                },
                Default::default(),
            )
            .await
            .expect_err("adapter must not keep server alive");
        assert!(error.to_string().contains("closed"), "{error}");
    }
}
