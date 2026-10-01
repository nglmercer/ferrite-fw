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

    ferrite::e2e::Runner::try_from_config(&resolved.e2e).map_err(ferrite::FerriteError::from)?;
    // Validate the engine early (loud on webkit/unknown).
    let kind = ferrite::e2e::BrowserKind::parse(&resolved.e2e.browser)
        .map_err(ferrite::FerriteError::from)?;
    // Validate the snapshot mode early (loud on typos).
    ferrite::e2e::SnapshotUpdate::parse(&resolved.e2e.update_snapshots)
        .map_err(ferrite::FerriteError::from)?;

    if args.check {
        return check_browser(&resolved.e2e, kind).await;
    }

    // Chromium recordings assemble via ffmpeg: fail fast with a hint.
    if resolved.e2e.video != "off"
        && kind == ferrite::e2e::BrowserKind::Chromium
        && ferrite::e2e::find_ffmpeg().is_none()
    {
        return Err(ferrite::FerriteError::Other(
            "video is enabled for chromium but ffmpeg was not found; install \
             ffmpeg or set FERRITE_FFMPEG_PATH (firefox records natively)"
                .to_string(),
        ));
    }

    // Server lifecycle: explicit --url wins, then config, else boot.
    let booted = ensure_server(&root, &resolved, args.url.clone()).await?;
    if resolved.e2e.base_url.is_none() {
        resolved.e2e.base_url = Some(booted.url.clone());
    }
    println!("e2e server: {}", booted.url);

    let command = test_command(&root, &args.command)?;
    println!("e2e run: {}", command.join(" "));
    let status = child_command(&root, &resolved.e2e, &command)?
        .status()
        .map_err(|error| {
            ferrite::FerriteError::Other(format!("run {}: {error}", command.join(" ")))
        })?;
    booted.shutdown().await;
    let code = status.code().unwrap_or(1);
    std::process::exit(code);
}

fn child_command(
    root: &Path,
    config: &ferrite::config::E2eConfig,
    command: &[String],
) -> ferrite::Result<std::process::Command> {
    let program = command
        .first()
        .ok_or_else(|| ferrite::FerriteError::Other("empty E2E test command".into()))?;
    let mut child = std::process::Command::new(program);
    child
        .args(&command[1..])
        .current_dir(root)
        .env("FERRITE_E2E_CONFIG", serde_json::to_string(config)?)
        .env("FERRITE_E2E_BROWSER", &config.browser)
        .env("FERRITE_E2E_VIDEO", &config.video)
        .env("FERRITE_E2E_REPORTER", &config.reporter)
        .env("FERRITE_E2E_WORKERS", config.workers.to_string())
        .env("FERRITE_E2E_RETRIES", config.retries.to_string())
        .env(
            "FERRITE_E2E_FAIL_ON_FLAKY_TESTS",
            config.fail_on_flaky_tests.to_string(),
        )
        .env("FERRITE_E2E_FORBID_ONLY", config.forbid_only.to_string())
        .env("FERRITE_E2E_REPEAT_EACH", config.repeat_each.to_string())
        .env("FERRITE_E2E_OUTPUT_DIR", &config.output_dir)
        .env("FERRITE_E2E_PRESERVE_OUTPUT", &config.preserve_output)
        .env("FERRITE_UPDATE_SNAPSHOTS", &config.update_snapshots)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    for (name, value) in [
        ("TIMEOUT_MS", config.timeout_ms),
        ("EXPECT_TIMEOUT_MS", config.expect_timeout_ms),
        ("GLOBAL_TIMEOUT_MS", config.global_timeout_ms),
        ("MAX_FAILURES", config.max_failures as u64),
        ("CLEANUP_TIMEOUT_MS", config.cleanup_timeout_ms),
        ("VIDEO_FPS", u64::from(config.video_fps)),
    ] {
        child.env(format!("FERRITE_E2E_{name}"), value.to_string());
    }
    for (name, value) in [
        ("FERRITE_E2E_BASE_URL", &config.base_url),
        ("FERRITE_E2E_FILTER", &config.filter),
        ("FERRITE_E2E_GREP", &config.grep),
        ("FERRITE_E2E_GREP_INVERT", &config.grep_invert),
        ("FERRITE_SNAPSHOT_DIR", &config.snapshot_dir),
        (
            "FERRITE_SNAPSHOT_PATH_TEMPLATE",
            &config.snapshot_path_template,
        ),
    ] {
        if let Some(value) = value {
            child.env(name, value);
        }
    }
    if let Some((index, total)) = config.shard {
        child.env("FERRITE_E2E_SHARD", format!("{index}/{total}"));
    }
    if !config.selected_projects.is_empty() {
        child.env("FERRITE_E2E_PROJECT", config.selected_projects.join(","));
    }
    Ok(child)
}

fn apply_flag_overrides(e2e: &mut ferrite::config::E2eConfig, args: &E2eArgs) {
    if let Some(value) = args.fail_on_flaky_tests {
        e2e.fail_on_flaky_tests = value;
    }
    if let Some(value) = args.forbid_only {
        e2e.forbid_only = value;
    }
    if let Some(engine) = &args.engine {
        e2e.browser = engine.clone();
    }
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
    if let Some(value) = args.global_timeout {
        e2e.global_timeout_ms = value;
    }
    if let Some(value) = args.max_failures {
        e2e.max_failures = value;
    }
    if let Some(value) = args.cleanup_timeout {
        e2e.cleanup_timeout_ms = value;
    }
    if let Some(workers) = args.workers {
        e2e.workers = workers;
    }
    if let Some(video) = &args.video {
        e2e.video = video.clone();
    }
    if let Some(update_snapshots) = &args.update_snapshots {
        e2e.update_snapshots = update_snapshots.clone();
    }
    if let Some(value) = args.repeat_each {
        e2e.repeat_each = value.max(1);
    }
    if let Some(value) = &args.output_dir {
        e2e.output_dir = value.display().to_string();
    }
    if let Some(value) = &args.snapshot_dir {
        e2e.snapshot_dir = Some(value.display().to_string());
    }
    if let Some(value) = &args.snapshot_path_template {
        e2e.snapshot_path_template = Some(value.clone());
    }
    if let Some(value) = &args.filter {
        e2e.filter = Some(value.clone());
    }
    if let Some(value) = &args.grep {
        e2e.grep = Some(value.clone());
    }
    if let Some(value) = &args.grep_invert {
        e2e.grep_invert = Some(value.clone());
    }
    if let Some(value) = args.shard {
        e2e.shard = Some(value);
    }
    if !args.project.is_empty() {
        e2e.selected_projects = args.project.clone();
    }
}

async fn check_browser(
    e2e: &ferrite::config::E2eConfig,
    kind: ferrite::e2e::BrowserKind,
) -> ferrite::Result<()> {
    use ferrite::e2e::BrowserKind;
    let hint = e2e.executable_path.clone().map(PathBuf::from);
    let (found, missing) = match kind {
        BrowserKind::Chromium => (
            ferrite::e2e::find_chromium(hint.as_deref()),
            "chromium not found; install chromium or google-chrome, or set \
             [e2e] executable_path / FERRITE_CHROMIUM_PATH",
        ),
        BrowserKind::Firefox => (
            ferrite::e2e::find_firefox(hint.as_deref()),
            "firefox not found; install firefox, or set [e2e] executable_path / \
             FERRITE_FIREFOX_PATH",
        ),
    };
    let Some(path) = found else {
        return Err(ferrite::FerriteError::Other(missing.to_string()));
    };
    println!("{}: {}", kind.name(), path.display());
    let mut options =
        ferrite::e2e::LaunchOptions::from_config(e2e).map_err(ferrite::FerriteError::from)?;
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
    fn policy_flags_support_enable_disable_and_omitted_config_values() {
        use clap::Parser;
        for (flags, expected) in [
            (vec![], true),
            (vec!["--fail-on-flaky-tests", "--forbid-only"], true),
            (
                vec!["--fail-on-flaky-tests=false", "--forbid-only=false"],
                false,
            ),
        ] {
            let arguments = [
                vec!["ferrite", "e2e", "."],
                flags,
                vec!["--", "cargo", "test"],
            ]
            .concat();
            let crate::cli::Command::E2e(args) = crate::cli::Cli::parse_from(arguments).command
            else {
                panic!("e2e args")
            };
            assert_eq!(args.command, ["cargo", "test"]);
            let mut config = ferrite::config::E2eConfig {
                fail_on_flaky_tests: true,
                forbid_only: true,
                ..Default::default()
            };
            apply_flag_overrides(&mut config, &args);
            assert_eq!(config.fail_on_flaky_tests, expected);
            assert_eq!(config.forbid_only, expected);
            let child = child_command(Path::new("."), &config, &["cargo".into()]).unwrap();
            for name in ["FERRITE_E2E_FAIL_ON_FLAKY_TESTS", "FERRITE_E2E_FORBID_ONLY"] {
                assert_eq!(
                    child
                        .get_envs()
                        .find(|(key, _)| key == &name)
                        .unwrap()
                        .1
                        .unwrap(),
                    expected.to_string().as_str()
                );
            }
        }
        assert!(crate::cli::Cli::try_parse_from([
            "ferrite",
            "e2e",
            "--fail-on-flaky-tests=invalid"
        ])
        .is_err());
    }

    #[test]
    fn project_flags_override_without_mutating_parent_environment() {
        use clap::Parser;
        let before = std::env::var_os("FERRITE_E2E_GREP");
        let crate::cli::Command::E2e(args) = crate::cli::Cli::parse_from([
            "ferrite",
            "e2e",
            "--repeat-each",
            "0",
            "--output-dir",
            "cli-output",
            "--snapshot-dir",
            "cli-baseline",
            "--snapshot-path-template",
            "{snapshotDir}/{browserName}/{arg}{ext}",
            "--filter",
            "",
            "--grep",
            "included",
            "--grep-invert",
            "excluded",
            "--shard",
            "2/3",
            "--project",
            "desktop",
        ])
        .command
        else {
            panic!("e2e args")
        };
        let mut config = ferrite::config::E2eConfig {
            repeat_each: 4,
            output_dir: "old-output".into(),
            grep: Some("old".into()),
            projects: vec![ferrite::config::E2eProjectConfig {
                name: "desktop".into(),
                repeat_each: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_flag_overrides(&mut config, &args);
        assert_eq!(config.repeat_each, 1);
        assert_eq!(config.output_dir, "cli-output");
        assert_eq!(config.snapshot_dir.as_deref(), Some("cli-baseline"));
        assert_eq!(
            config.snapshot_path_template.as_deref(),
            Some("{snapshotDir}/{browserName}/{arg}{ext}")
        );
        assert_eq!(config.filter.as_deref(), Some(""));
        assert_eq!(config.grep.as_deref(), Some("included"));
        assert_eq!(config.grep_invert.as_deref(), Some("excluded"));
        assert_eq!(config.shard, Some((2, 3)));
        assert_eq!(config.selected_projects, ["desktop"]);
        assert_eq!(config.projects[0].repeat_each, Some(2));
        assert_eq!(std::env::var_os("FERRITE_E2E_GREP"), before);
        ferrite::e2e::Runner::try_from_config(&config).unwrap();
    }

    #[test]
    fn child_environment_preserves_json_and_all_explicit_legacy_settings() {
        let config = ferrite::config::E2eConfig {
            timeout_ms: 811,
            expect_timeout_ms: 433,
            global_timeout_ms: 900,
            cleanup_timeout_ms: 199,
            max_failures: 2,
            video_fps: 17,
            repeat_each: 3,
            base_url: Some("http://localhost:7777".into()),
            snapshot_dir: Some("snapshots with spaces".into()),
            snapshot_path_template: Some("{snapshotDir}/{projectName}/{arg}{ext}".into()),
            filter: Some("".into()),
            grep: Some("good".into()),
            grep_invert: Some("bad".into()),
            shard: Some((1, 2)),
            selected_projects: vec!["desktop".into()],
            projects: vec![ferrite::config::E2eProjectConfig {
                name: "desktop".into(),
                output_dir: Some("project out".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let child =
            child_command(Path::new("."), &config, &["cargo".into(), "test".into()]).unwrap();
        let env = child
            .get_envs()
            .map(|(name, value)| {
                (
                    name.to_string_lossy().into_owned(),
                    value.unwrap().to_string_lossy().into_owned(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            env["FERRITE_SNAPSHOT_PATH_TEMPLATE"],
            "{snapshotDir}/{projectName}/{arg}{ext}"
        );
        let forwarded: ferrite::config::E2eConfig =
            serde_json::from_str(&env["FERRITE_E2E_CONFIG"]).unwrap();
        assert_eq!(
            serde_json::to_value(&forwarded).unwrap(),
            serde_json::to_value(&config).unwrap()
        );
        for (name, value) in [
            ("TIMEOUT_MS", "811"),
            ("EXPECT_TIMEOUT_MS", "433"),
            ("GLOBAL_TIMEOUT_MS", "900"),
            ("CLEANUP_TIMEOUT_MS", "199"),
            ("MAX_FAILURES", "2"),
            ("VIDEO_FPS", "17"),
            ("REPEAT_EACH", "3"),
            ("FILTER", ""),
            ("GREP", "good"),
            ("GREP_INVERT", "bad"),
            ("SHARD", "1/2"),
            ("PROJECT", "desktop"),
        ] {
            assert_eq!(env[&format!("FERRITE_E2E_{name}")], value);
        }
        assert_eq!(env["FERRITE_SNAPSHOT_DIR"], "snapshots with spaces");
        assert_eq!(child.get_args().next().unwrap(), "test");
        assert!(child_command(Path::new("."), &config, &[]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn actual_child_gets_forwarded_values_with_spaces() {
        let config = ferrite::config::E2eConfig {
            repeat_each: 3,
            timeout_ms: 811,
            snapshot_dir: Some("baseline space".into()),
            projects: vec![ferrite::config::E2eProjectConfig {
                name: "desktop".into(),
                repeat_each: Some(2),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut child = child_command(Path::new("."), &config, &["sh".into(),"-c".into(),
            "printf '%s\\n%s\\n%s\\n%s\\n' \"$FERRITE_E2E_CONFIG\" \"$FERRITE_E2E_REPEAT_EACH\" \"$FERRITE_E2E_TIMEOUT_MS\" \"$FERRITE_SNAPSHOT_DIR\"".into()]).unwrap();
        child.stdout(std::process::Stdio::piped());
        let output = child.output().unwrap();
        assert!(output.status.success());
        let output = String::from_utf8(output.stdout).unwrap();
        let mut lines = output.lines();
        let forwarded: ferrite::config::E2eConfig =
            serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(forwarded.projects[0].repeat_each, Some(2));
        assert_eq!(lines.collect::<Vec<_>>(), ["3", "811", "baseline space"]);
    }

    #[test]
    fn runner_limit_flags_override_config() {
        use clap::Parser;
        let crate::cli::Command::E2e(args) = crate::cli::Cli::parse_from([
            "ferrite",
            "e2e",
            "--global-timeout",
            "900",
            "--max-failures",
            "2",
            "--cleanup-timeout",
            "70",
        ])
        .command
        else {
            panic!("e2e args")
        };
        let mut config = ferrite::config::E2eConfig::default();
        apply_flag_overrides(&mut config, &args);
        assert_eq!(config.global_timeout_ms, 900);
        assert_eq!(config.max_failures, 2);
        assert_eq!(config.cleanup_timeout_ms, 70);
    }
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
