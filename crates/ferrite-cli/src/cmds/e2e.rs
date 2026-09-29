//! E2E command: boot the web server, run the test command.
//!
//! `ferrite e2e` owns the server lifecycle (in-process dev server by
//! default, a configured `command`, or an already-running `--url`) and then
//! runs the user's Rust test command with `FERRITE_E2E_BASE_URL` set. Test
//! bodies use the `ferrite_e2e` library (`Browser`, `Page`, `Runner`).

use crate::cli::E2eArgs;
use crate::{default_plugins, root_of};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) async fn e2e(
    args: E2eArgs,
    config_arg: Option<PathBuf>,
    mode: Option<String>,
) -> ferrite::Result<()> {
    let root = root_of(&config_arg, args.root.clone());
    let mut config = ferrite::Config {
        root: Some(root.clone()),
        overrides: ferrite::CliOverrides {
            mode,
            ..Default::default()
        },
        ..Default::default()
    };
    for plugin in default_plugins(&root) {
        config.plugins.push(plugin);
    }
    let (mut resolved, _) = config.resolve().await?;
    apply_flag_overrides(&mut resolved.e2e, &args);

    if !resolved.e2e.is_chromium() {
        return Err(ferrite::FerriteError::Other(format!(
            "unsupported [e2e] browser {:?}: v1 supports chromium only",
            resolved.e2e.browser
        )));
    }

    if args.check {
        return check_chromium(&resolved.e2e).await;
    }

    // Server lifecycle: explicit --url wins, then config, else boot.
    let booted = ensure_server(&root, &resolved, args.url.clone()).await?;
    println!("e2e server: {}", booted.url);

    let command = test_command(&root, &args.command)?;
    println!("e2e run: {}", command.join(" "));
    let status = std::process::Command::new(&command[0])
        .args(&command[1..])
        .current_dir(&root)
        .env("FERRITE_E2E_BASE_URL", &booted.url)
        .env("FERRITE_E2E_REPORTER", &resolved.e2e.reporter)
        .env("FERRITE_E2E_WORKERS", resolved.e2e.workers.to_string())
        .env("FERRITE_E2E_RETRIES", resolved.e2e.retries.to_string())
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|error| {
            ferrite::FerriteError::Other(format!("run {}: {error}", command.join(" ")))
        })?;
    booted.shutdown().await;
    let code = status.code().unwrap_or(1);
    std::process::exit(code);
}

fn apply_flag_overrides(e2e: &mut ferrite::config::E2eConfig, args: &E2eArgs) {
    if args.headed {
        e2e.headless = false;
    }
    if let Some(browser) = &args.browser {
        e2e.executable_path = Some(browser.display().to_string());
    }
    if let Some(base_url) = &args.base_url {
        e2e.base_url = Some(base_url.clone());
    }
    if let Some(reporter) = &args.reporter {
        e2e.reporter = reporter.clone();
    }
    if let Some(retries) = args.retries {
        e2e.retries = retries;
    }
    if let Some(workers) = args.workers {
        e2e.workers = workers;
    }
    if args.filter.is_some() {
        // The filter travels to the test process; `Runner::filter` applies it.
        std::env::set_var(
            "FERRITE_E2E_FILTER",
            args.filter.as_deref().unwrap_or_default(),
        );
    }
}

async fn check_chromium(e2e: &ferrite::config::E2eConfig) -> ferrite::Result<()> {
    let hint = e2e.executable_path.clone().map(PathBuf::from);
    let found = ferrite::e2e::find_chromium(hint.as_deref());
    let Some(path) = found else {
        return Err(ferrite::FerriteError::Other(
            "chromium not found; install chromium or google-chrome, or set \
             [e2e] executable_path / FERRITE_CHROMIUM_PATH"
                .to_string(),
        ));
    };
    println!("chromium: {}", path.display());
    let mut options = ferrite::e2e::LaunchOptions::from_config(e2e);
    options.executable_path = Some(path);
    let browser = ferrite::e2e::Browser::launch(options)
        .await
        .map_err(ferrite::FerriteError::from)?;
    println!(
        "version: {}",
        browser
            .version()
            .await
            .map_err(ferrite::FerriteError::from)?
    );
    browser.close().await.map_err(ferrite::FerriteError::from)?;
    println!("e2e check: ok");
    Ok(())
}

struct BootedServer {
    url: String,
    dev_task: Option<tokio::task::JoinHandle<()>>,
    spawned: Option<ferrite::e2e::RunningWebServer>,
}

impl BootedServer {
    async fn shutdown(self) {
        if let Some(spawned) = self.spawned {
            spawned.shutdown().await.ok();
        }
        if let Some(task) = self.dev_task {
            task.abort();
        }
    }
}

async fn ensure_server(
    root: &Path,
    resolved: &ferrite::ResolvedConfig,
    url_flag: Option<String>,
) -> ferrite::Result<BootedServer> {
    // 1. Explicit --url: never boot, only wait for readiness.
    if let Some(url) = url_flag {
        ferrite::e2e::wait_for_url(&url, Duration::from_secs(60))
            .await
            .map_err(ferrite::FerriteError::from)?;
        return Ok(BootedServer {
            url,
            dev_task: None,
            spawned: None,
        });
    }

    let server = ferrite::e2e::WebServer::from_config(&resolved.e2e, None, None);
    let url = server.url().to_string();
    // 2. Reuse a running server when allowed.
    if resolved
        .e2e
        .web_server
        .as_ref()
        .map(|s| s.reuse_existing)
        .unwrap_or(true)
        && ferrite::e2e::wait_for_url(&url, Duration::from_secs(1))
            .await
            .is_ok()
    {
        println!("reusing running server at {url}");
        return Ok(BootedServer {
            url,
            dev_task: None,
            spawned: None,
        });
    }
    // 3. Configured command: spawn it and wait.
    if resolved
        .e2e
        .web_server
        .as_ref()
        .and_then(|s| s.command.clone())
        .is_some()
    {
        let running = server
            .ensure_running()
            .await
            .map_err(ferrite::FerriteError::from)?;
        return Ok(BootedServer {
            url: running.url.clone(),
            dev_task: None,
            spawned: Some(running),
        });
    }
    // 4. Default: in-process dev server on the e2e URL port.
    boot_dev_server(root, resolved, &url).await
}

async fn boot_dev_server(
    root: &Path,
    _resolved: &ferrite::ResolvedConfig,
    url: &str,
) -> ferrite::Result<BootedServer> {
    let port = url_port(url).unwrap_or(5190);
    let mut config = ferrite::Config {
        root: Some(root.to_path_buf()),
        overrides: ferrite::CliOverrides {
            port: Some(port),
            ..Default::default()
        },
        ..Default::default()
    };
    config.user.server.strict_port = true;
    for plugin in default_plugins(root) {
        config.plugins.push(plugin);
    }
    let server = ferrite::create_server(config).await?;
    let task = tokio::spawn(async move {
        if let Err(error) = server.listen().await {
            tracing::warn!("e2e dev server ended: {error}");
        }
    });
    ferrite::e2e::wait_for_url(url, Duration::from_secs(60))
        .await
        .map_err(|error| {
            task.abort();
            ferrite::FerriteError::from(error)
        })?;
    Ok(BootedServer {
        url: url.to_string(),
        dev_task: Some(task),
        spawned: None,
    })
}

fn url_port(url: &str) -> Option<u16> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    authority.rsplit(':').next()?.parse().ok()
}

fn test_command(root: &Path, explicit: &[String]) -> ferrite::Result<Vec<String>> {
    if !explicit.is_empty() {
        return Ok(explicit.to_vec());
    }
    // Default: `cargo test --test e2e`, but fail loudly with guidance when
    // no such target exists instead of dumping cargo's error.
    let has_target = root.join("tests").join("e2e.rs").is_file()
        || root.join("tests").join("e2e").join("main.rs").is_file();
    if !has_target {
        return Err(ferrite::FerriteError::Other(
            "no tests/e2e.rs found; create one with the ferrite_e2e Runner, \
             or pass an explicit command: `ferrite e2e -- cargo test --test my_e2e`"
                .to_string(),
        ));
    }
    Ok(vec![
        "cargo".to_string(),
        "test".to_string(),
        "--test".to_string(),
        "e2e".to_string(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_parses() {
        assert_eq!(url_port("http://127.0.0.1:5190/"), Some(5190));
        assert_eq!(url_port("http://localhost:3000"), Some(3000));
        assert_eq!(url_port("http://example.com/"), None);
    }

    #[test]
    fn explicit_command_wins() {
        let dir = std::env::temp_dir();
        let command = test_command(
            &dir,
            &["npm".to_string(), "run".to_string(), "e2e".to_string()],
        )
        .unwrap();
        assert_eq!(command, vec!["npm", "run", "e2e"]);
    }

    #[test]
    fn missing_target_errors_loudly() {
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let error = test_command(&dir, &[]).unwrap_err();
        assert!(error.to_string().contains("tests/e2e.rs"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
