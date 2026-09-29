//! Chromium discovery, launch, and browser-level target management.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::cdp::CdpConnection;
use crate::context::{BrowserContext, ContextOptions};
use crate::error::{E2eError, E2eResult};
use crate::page::Page;

/// Options for launching Chromium.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Launch headless (`--headless=new`).
    pub headless: bool,
    /// Explicit executable path (overrides auto-detection).
    pub executable_path: Option<PathBuf>,
    /// Extra Chromium CLI args.
    pub args: Vec<String>,
    /// Keep the user-data-dir after close (debugging).
    pub keep_profile: bool,
    /// Slow down each action by this long.
    pub slow_mo: Duration,
    /// Launch + protocol timeout.
    pub timeout: Duration,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            headless: true,
            executable_path: None,
            args: Vec::new(),
            keep_profile: false,
            slow_mo: Duration::ZERO,
            timeout: Duration::from_secs(30),
        }
    }
}

impl LaunchOptions {
    /// Build from resolved e2e config.
    #[must_use]
    pub fn from_config(config: &ferrite_config::E2eConfig) -> Self {
        Self {
            headless: config.headless,
            executable_path: config.executable_path.clone().map(PathBuf::from),
            args: config.args.clone(),
            keep_profile: false,
            slow_mo: Duration::from_millis(config.slow_mo_ms),
            timeout: Duration::from_millis(config.timeout_ms.max(1_000)),
        }
    }

    /// Override the executable path.
    #[must_use]
    pub fn executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_path = Some(path.into());
        self
    }

    /// Run with a visible window.
    #[must_use]
    pub fn headed(mut self) -> Self {
        self.headless = false;
        self
    }

    /// Append a Chromium CLI arg.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

/// Locate a Chromium executable.
///
/// Precedence: `FERRITE_CHROMIUM_PATH` / `CHROME_PATH` env, explicit hint,
/// well-known names on `PATH`, well-known install locations.
pub fn find_chromium(hint: Option<&Path>) -> Option<PathBuf> {
    if let Some(hint) = hint {
        if hint.is_file() {
            return Some(hint.to_path_buf());
        }
    }
    for env in ["FERRITE_CHROMIUM_PATH", "CHROME_PATH", "CHROMIUM_PATH"] {
        if let Ok(path) = std::env::var(env) {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    for name in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
        "headless_shell",
    ] {
        if let Some(path) = which(name) {
            return Some(path);
        }
    }
    for path in [
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/snap/bin/chromium",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
        "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    ] {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

fn free_port() -> E2eResult<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr().map(|addr| addr.port())?)
}

/// A launched Chromium instance.
pub struct Browser {
    child: Option<tokio::process::Child>,
    cdp: CdpConnection,
    _profile: Option<tempfile::TempDir>,
    debug_port: u16,
    slow_mo: Duration,
    timeout: Duration,
    base_url: Option<String>,
}

impl Browser {
    /// Launch with default options.
    pub async fn launch_default() -> E2eResult<Self> {
        Self::launch(LaunchOptions::default()).await
    }

    /// Launch Chromium and connect over CDP.
    pub async fn launch(options: LaunchOptions) -> E2eResult<Self> {
        let executable = options
            .executable_path
            .clone()
            .or_else(|| find_chromium(None))
            .ok_or_else(|| {
                E2eError::BrowserNotFound(
                    "no chromium executable found; set [e2e] executable_path or \
                     FERRITE_CHROMIUM_PATH"
                        .to_string(),
                )
            })?;
        let debug_port = free_port()?;
        let profile = tempfile::tempdir()?;
        let profile_arg = format!("--user-data-dir={}", profile.path().display());

        let mut cmd = tokio::process::Command::new(&executable);
        cmd.arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-dev-shm-usage")
            .arg(format!("--remote-debugging-port={debug_port}"))
            .arg("--remote-allow-origins=*")
            .arg(profile_arg)
            .arg("about:blank");
        if options.headless {
            cmd.arg("--headless=new").arg("--disable-gpu");
        }
        for arg in &options.args {
            cmd.arg(arg);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|error| {
            E2eError::Launch(format!("spawn {}: {error}", executable.display()))
        })?;

        // Poll /json/version until the DevTools endpoint answers.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| E2eError::Launch(error.to_string()))?;
        let deadline = tokio::time::Instant::now() + options.timeout;
        let version_url = format!("http://127.0.0.1:{debug_port}/json/version");
        let ws_url = loop {
            if tokio::time::Instant::now() > deadline {
                let _ = child.kill().await;
                return Err(E2eError::Launch(format!(
                    "chromium did not answer {version_url} within {:?}",
                    options.timeout
                )));
            }
            // Surface early crashes with stderr instead of hanging.
            if let Ok(Some(status)) = child.try_wait() {
                let stderr = drain_stderr(&mut child).await;
                return Err(E2eError::Launch(format!(
                    "chromium exited during launch ({status}): {stderr}"
                )));
            }
            match client.get(&version_url).send().await {
                Ok(response) => {
                    let body: Value = response
                        .json()
                        .await
                        .map_err(|error| E2eError::Launch(format!("bad /json/version: {error}")))?;
                    if let Some(ws) = body.get("webSocketDebuggerUrl").and_then(Value::as_str) {
                        break ws.to_string();
                    }
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        };

        let cdp = CdpConnection::connect(&ws_url).await?;
        // Fail loudly when the browser is unreachable over the protocol.
        let version: Value = cdp
            .call(None, "Browser.getVersion", Value::Null, options.timeout)
            .await?;
        tracing::info!(
            product = version.get("product").and_then(|v| v.as_str()),
            "chromium launched"
        );
        Ok(Self {
            child: Some(child),
            cdp,
            _profile: if options.keep_profile {
                None
            } else {
                // Leak the path when debugging; otherwise the tempdir owns it.
                Some(profile)
            },
            debug_port,
            slow_mo: options.slow_mo,
            timeout: options.timeout,
            base_url: None,
        })
    }

    /// Connect to an already-running Chromium (e.g. launched with
    /// `--remote-debugging-port`). The browser is not killed on close.
    pub async fn connect(debug_port: u16, timeout: Duration) -> E2eResult<Self> {
        let url = format!("http://127.0.0.1:{debug_port}/json/version");
        let body: Value = reqwest::get(&url)
            .await
            .map_err(|error| E2eError::Launch(format!("{url}: {error}")))?
            .json()
            .await
            .map_err(|error| E2eError::Launch(format!("bad /json/version: {error}")))?;
        let ws_url = body
            .get("webSocketDebuggerUrl")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::Launch("no webSocketDebuggerUrl".to_string()))?;
        let cdp = CdpConnection::connect(ws_url).await?;
        Ok(Self {
            child: None,
            cdp,
            _profile: None,
            debug_port,
            slow_mo: Duration::ZERO,
            timeout,
            base_url: None,
        })
    }

    /// Remote-debugging port.
    #[must_use]
    pub fn debug_port(&self) -> u16 {
        self.debug_port
    }

    /// Default protocol timeout.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Per-action slow-mo delay.
    #[must_use]
    pub fn slow_mo(&self) -> Duration {
        self.slow_mo
    }

    /// Set the base URL used to resolve relative navigations.
    pub fn set_base_url(&mut self, base_url: Option<String>) {
        self.base_url = base_url;
    }

    /// Base URL for relative navigations.
    #[must_use]
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// Raw CDP handle (browser session).
    #[must_use]
    pub fn cdp(&self) -> &CdpConnection {
        &self.cdp
    }

    /// Browser product version (`Browser.getVersion`).
    pub async fn version(&self) -> E2eResult<String> {
        let version = self
            .cdp
            .call(None, "Browser.getVersion", Value::Null, self.timeout)
            .await?;
        Ok(version
            .get("product")
            .and_then(Value::as_str)
            .unwrap_or("chromium")
            .to_string())
    }

    /// Create an isolated browser context (incognito-equivalent).
    pub async fn new_context(&self, options: ContextOptions) -> E2eResult<BrowserContext> {
        let params = serde_json::json!({
            "disposeOnDetach": true,
            "proxyServer": options.proxy_server,
        });
        let result = self
            .cdp
            .call(None, "Target.createBrowserContext", params, self.timeout)
            .await?;
        let id = result
            .get("browserContextId")
            .and_then(Value::as_str)
            .ok_or_else(|| E2eError::Launch("no browserContextId".to_string()))?;
        Ok(BrowserContext::new(
            self.cdp.clone(),
            Some(id.to_string()),
            options,
            self.slow_mo,
            self.timeout,
            self.base_url.clone(),
        ))
    }

    /// Default (shared) browser context.
    #[must_use]
    pub fn default_context(&self) -> BrowserContext {
        BrowserContext::new(
            self.cdp.clone(),
            None,
            ContextOptions::default(),
            self.slow_mo,
            self.timeout,
            self.base_url.clone(),
        )
    }

    /// Open a page in the default context.
    pub async fn new_page(&self) -> E2eResult<Page> {
        self.default_context().new_page().await
    }

    /// Close the browser (kills the child when this instance launched it).
    pub async fn close(mut self) -> E2eResult<()> {
        self.cdp.close();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        }
        Ok(())
    }
}

async fn drain_stderr(child: &mut tokio::process::Child) -> String {
    use tokio::io::AsyncReadExt as _;
    let mut text = String::new();
    if let Some(stderr) = child.stderr.as_mut() {
        let mut buf = vec![0u8; 4096];
        let Ok(n) = tokio::time::timeout(Duration::from_millis(500), stderr.read(&mut buf))
            .await
            .map(|r| r.unwrap_or(0))
        else {
            return String::new();
        };
        text.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    text.lines().take(5).collect::<Vec<_>>().join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_options_from_config() {
        let config = ferrite_config::E2eConfig {
            headless: false,
            slow_mo_ms: 50,
            ..ferrite_config::E2eConfig::default()
        };
        let options = LaunchOptions::from_config(&config);
        assert!(!options.headless);
        assert_eq!(options.slow_mo, Duration::from_millis(50));
    }

    #[test]
    fn chromium_hint_missing_is_ignored() {
        // A bogus hint must not itself be returned.
        let bogus = Path::new("/definitely/not/chrome");
        assert!(find_chromium(Some(bogus)).as_deref() != Some(bogus));
    }
}
