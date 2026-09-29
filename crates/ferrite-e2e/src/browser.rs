//! Browser discovery, launch, and browser-level management.
//!
//! Chromium is driven over CDP, Firefox over WebDriver BiDi. There is no
//! WebKit backend: Linux ships no stock WebKit browser with an automation
//! protocol (Playwright's WebKit is a custom download), so `webkit` is a
//! loud configuration error.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use crate::bidi::BidiConnection;
use crate::cdp::CdpConnection;
use crate::context::{BrowserContext, ContextOptions};
use crate::error::{E2eError, E2eResult};
use crate::page::Page;

/// Supported browser engines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserKind {
    /// Chromium over CDP.
    #[default]
    Chromium,
    /// Firefox over WebDriver BiDi.
    Firefox,
}

impl BrowserKind {
    /// Parse `[e2e] browser` / `--engine` (`chromium`, `chrome`, `firefox`, `ff`).
    pub fn parse(name: &str) -> E2eResult<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "chromium" | "chrome" => Ok(Self::Chromium),
            "firefox" | "ff" => Ok(Self::Firefox),
            "webkit" | "safari" => Err(E2eError::Config(
                "webkit is not supported: Linux ships no stock WebKit browser \
                 with an automation protocol"
                    .to_string(),
            )),
            other => Err(E2eError::Config(format!(
                "unknown browser {other:?}: expected \"chromium\" or \"firefox\""
            ))),
        }
    }

    /// Config-file name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Chromium => "chromium",
            Self::Firefox => "firefox",
        }
    }
}

/// Options for launching a browser.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Engine to launch.
    pub browser: BrowserKind,
    /// Launch headless.
    pub headless: bool,
    /// Explicit executable path (overrides auto-detection).
    pub executable_path: Option<PathBuf>,
    /// Extra browser CLI args.
    pub args: Vec<String>,
    /// Keep the profile dir after close (debugging).
    pub keep_profile: bool,
    /// Slow down each action by this long.
    pub slow_mo: Duration,
    /// Launch + protocol timeout.
    pub timeout: Duration,
    /// Browser-wide user agent (Chromium flag / Firefox profile pref).
    pub user_agent: Option<String>,
    /// Browser-wide proxy (`host:port`, `http(s)://…`, `socks5://…`).
    pub proxy_server: Option<String>,
    /// Accept insecure TLS certificates session-wide.
    pub ignore_https_errors: bool,
    /// Download directory (Chromium pref / Firefox profile pref).
    pub download_dir: Option<PathBuf>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            browser: BrowserKind::Chromium,
            headless: true,
            executable_path: None,
            args: Vec::new(),
            keep_profile: false,
            slow_mo: Duration::ZERO,
            timeout: Duration::from_secs(30),
            user_agent: None,
            proxy_server: None,
            ignore_https_errors: false,
            download_dir: None,
        }
    }
}

impl LaunchOptions {
    /// Build from resolved e2e config (validates the browser name).
    pub fn from_config(config: &ferrite_config::E2eConfig) -> E2eResult<Self> {
        Ok(Self {
            browser: BrowserKind::parse(&config.browser)?,
            headless: config.headless,
            executable_path: config.executable_path.clone().map(PathBuf::from),
            args: config.args.clone(),
            keep_profile: false,
            slow_mo: Duration::from_millis(config.slow_mo_ms),
            timeout: Duration::from_millis(config.timeout_ms.max(1_000)),
            user_agent: config.user_agent.clone(),
            proxy_server: config.proxy_server.clone(),
            ignore_https_errors: config.ignore_https_errors,
            download_dir: config.download_dir.clone().map(PathBuf::from),
        })
    }

    /// Select the engine.
    #[must_use]
    pub fn browser(mut self, browser: BrowserKind) -> Self {
        self.browser = browser;
        self
    }

    /// Override the executable path.
    #[must_use]
    pub fn executable(mut self, path: impl Into<PathBuf>) -> Self {
        self.executable_path = Some(path.into());
        self
    }

    /// Set the download directory.
    #[must_use]
    pub fn download_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.download_dir = Some(path.into());
        self
    }

    /// Run with a visible window.
    #[must_use]
    pub fn headed(mut self) -> Self {
        self.headless = false;
        self
    }

    /// Append a browser CLI arg.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

/// Locate a Chromium executable.
///
/// Precedence: explicit hint, `FERRITE_CHROMIUM_PATH` / `CHROME_PATH` env,
/// well-known names on `PATH`, well-known install locations.
pub fn find_chromium(hint: Option<&Path>) -> Option<PathBuf> {
    find_browser(
        hint,
        &["FERRITE_CHROMIUM_PATH", "CHROME_PATH", "CHROMIUM_PATH"],
        &[
            "chromium",
            "chromium-browser",
            "google-chrome",
            "google-chrome-stable",
            "chrome",
            "headless_shell",
        ],
        &[
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/snap/bin/chromium",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        ],
    )
}

/// Locate a Firefox executable.
///
/// Precedence: explicit hint, `FERRITE_FIREFOX_PATH` / `FIREFOX_PATH` env,
/// well-known names on `PATH`, well-known install locations.
pub fn find_firefox(hint: Option<&Path>) -> Option<PathBuf> {
    find_browser(
        hint,
        &["FERRITE_FIREFOX_PATH", "FIREFOX_PATH"],
        &["firefox", "firefox-esr"],
        &[
            "/usr/bin/firefox",
            "/usr/bin/firefox-esr",
            "/snap/bin/firefox",
            "/Applications/Firefox.app/Contents/MacOS/firefox",
            "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
            "C:\\Program Files (x86)\\Mozilla Firefox\\firefox.exe",
        ],
    )
}

fn find_browser(
    hint: Option<&Path>,
    envs: &[&str],
    names: &[&str],
    paths: &[&str],
) -> Option<PathBuf> {
    if let Some(hint) = hint {
        if hint.is_file() {
            return Some(hint.to_path_buf());
        }
    }
    for env in envs {
        if let Ok(path) = std::env::var(env) {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    for name in names {
        if let Some(path) = which(name) {
            return Some(path);
        }
    }
    for path in paths {
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

/// Protocol backend of a launched browser.
#[derive(Clone)]
pub(crate) enum Backend {
    /// Chromium CDP connection.
    Cdp(CdpConnection),
    /// Firefox BiDi connection + session.
    Bidi {
        /// BiDi connection (one session per process).
        conn: BidiConnection,
        /// Session accepts insecure certificates.
        insecure_certs: bool,
    },
}

/// A launched browser instance.
pub struct Browser {
    child: Option<tokio::process::Child>,
    backend: Backend,
    kind: BrowserKind,
    _profile: Option<tempfile::TempDir>,
    debug_port: u16,
    slow_mo: Duration,
    timeout: Duration,
    base_url: Option<String>,
    proxy_server: Option<String>,
    product: String,
    contexts: Arc<Mutex<Vec<BrowserContext>>>,
    default: OnceLock<BrowserContext>,
}

impl Browser {
    /// Launch with default options (Chromium).
    pub async fn launch_default() -> E2eResult<Self> {
        Self::launch(LaunchOptions::default()).await
    }

    /// Launch the configured engine and connect over its protocol.
    pub async fn launch(options: LaunchOptions) -> E2eResult<Self> {
        match options.browser {
            BrowserKind::Chromium => Self::launch_chromium(options).await,
            BrowserKind::Firefox => Self::launch_firefox(options).await,
        }
    }

    async fn launch_chromium(options: LaunchOptions) -> E2eResult<Self> {
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
        if let Some(dir) = &options.download_dir {
            write_chromium_download_pref(profile.path(), dir)?;
        }
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
        if let Some(user_agent) = &options.user_agent {
            cmd.arg(format!("--user-agent={user_agent}"));
        }
        if let Some(proxy) = &options.proxy_server {
            cmd.arg(format!("--proxy-server={proxy}"));
        }
        if options.ignore_https_errors {
            cmd.arg("--ignore-certificate-errors");
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
        let version: Value = cdp
            .call(None, "Browser.getVersion", Value::Null, options.timeout)
            .await?;
        let product = version
            .get("product")
            .and_then(Value::as_str)
            .unwrap_or("chromium")
            .to_string();
        tracing::info!(%product, "chromium launched");
        Ok(Self {
            child: Some(child),
            backend: Backend::Cdp(cdp),
            kind: BrowserKind::Chromium,
            _profile: if options.keep_profile {
                None
            } else {
                Some(profile)
            },
            debug_port,
            slow_mo: options.slow_mo,
            timeout: options.timeout,
            base_url: None,
            proxy_server: options.proxy_server,
            product,
            contexts: Arc::new(Mutex::new(Vec::new())),
            default: OnceLock::new(),
        })
    }

    async fn launch_firefox(options: LaunchOptions) -> E2eResult<Self> {
        let executable = options
            .executable_path
            .clone()
            .or_else(|| find_firefox(None))
            .ok_or_else(|| {
                E2eError::BrowserNotFound(
                    "no firefox executable found; set [e2e] executable_path or \
                     FERRITE_FIREFOX_PATH"
                        .to_string(),
                )
            })?;
        let debug_port = free_port()?;
        let profile = tempfile::tempdir()?;
        write_firefox_prefs(
            profile.path(),
            options.user_agent.as_deref(),
            options.download_dir.as_deref(),
        )?;
        if let Some(dir) = &options.download_dir {
            std::fs::create_dir_all(dir)?;
        }

        let mut cmd = tokio::process::Command::new(&executable);
        if options.headless {
            cmd.arg("--headless");
        }
        cmd.arg("--no-remote")
            .arg("--profile")
            .arg(profile.path())
            .arg("--remote-debugging-port")
            .arg(debug_port.to_string())
            .arg("about:blank");
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

        // Poll the Remote Agent HTTP root until it answers.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|error| E2eError::Launch(error.to_string()))?;
        let deadline = tokio::time::Instant::now() + options.timeout;
        let root_url = format!("http://127.0.0.1:{debug_port}/");
        loop {
            if tokio::time::Instant::now() > deadline {
                let _ = child.kill().await;
                return Err(E2eError::Launch(format!(
                    "firefox did not answer {root_url} within {:?}",
                    options.timeout
                )));
            }
            if let Ok(Some(status)) = child.try_wait() {
                let stderr = drain_stderr(&mut child).await;
                return Err(E2eError::Launch(format!(
                    "firefox exited during launch ({status}): {stderr}"
                )));
            }
            match client.get(&root_url).send().await {
                Ok(response) if response.status().is_success() => break,
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }

        // Exactly one BiDi session per Firefox process.
        let ws_url = format!("ws://127.0.0.1:{debug_port}/session");
        let bidi = BidiConnection::connect(&ws_url).await?;
        let mut capabilities = serde_json::Map::new();
        // Keep prompts open until handled: the default ("dismiss and notify")
        // auto-dismisses before `handleUserPrompt` arrives, making accept and
        // prompt text meaningless. Unhandled dialogs now block the page until
        // `Page::handle_dialogs` runs.
        capabilities.insert(
            "unhandledPromptBehavior".to_string(),
            Value::String("ignore".to_string()),
        );
        if options.ignore_https_errors {
            capabilities.insert("acceptInsecureCerts".to_string(), Value::Bool(true));
        }
        if let Some(proxy) = &options.proxy_server {
            capabilities.insert("proxy".to_string(), proxy_capabilities(proxy));
        }
        let session_params = serde_json::json!({ "capabilities": { "alwaysMatch": capabilities } });
        let session = bidi
            .call("session.new", session_params, options.timeout)
            .await?;
        if session.get("sessionId").and_then(Value::as_str).is_none() {
            return Err(E2eError::Launch(
                "BiDi session.new returned no sessionId".to_string(),
            ));
        }
        let product = session
            .get("capabilities")
            .and_then(|caps| {
                let name = caps.get("browserName").and_then(Value::as_str)?;
                let version = caps.get("browserVersion").and_then(Value::as_str)?;
                Some(format!("{name}/{version}"))
            })
            .unwrap_or_else(|| "firefox".to_string());
        bidi.call(
            "session.subscribe",
            serde_json::json!({
                "events": [
                    "log.entryAdded",
                    "network.beforeRequestSent",
                    "network.responseCompleted",
                    "network.fetchError",
                    "browsingContext.load",
                    "browsingContext.domContentLoaded",
                    "browsingContext.userPromptOpened",
                ],
            }),
            options.timeout,
        )
        .await?;
        tracing::info!(%product, "firefox launched");
        Ok(Self {
            child: Some(child),
            backend: Backend::Bidi {
                conn: bidi,
                insecure_certs: options.ignore_https_errors,
            },
            kind: BrowserKind::Firefox,
            _profile: if options.keep_profile {
                None
            } else {
                Some(profile)
            },
            debug_port,
            slow_mo: options.slow_mo,
            timeout: options.timeout,
            base_url: None,
            proxy_server: options.proxy_server,
            product,
            contexts: Arc::new(Mutex::new(Vec::new())),
            default: OnceLock::new(),
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
            backend: Backend::Cdp(cdp),
            kind: BrowserKind::Chromium,
            _profile: None,
            debug_port,
            slow_mo: Duration::ZERO,
            timeout,
            base_url: None,
            proxy_server: None,
            product: "chromium".to_string(),
            contexts: Arc::new(Mutex::new(Vec::new())),
            default: OnceLock::new(),
        })
    }

    /// Engine kind.
    #[must_use]
    pub fn kind(&self) -> BrowserKind {
        self.kind
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

    /// Raw CDP handle (`Some` on Chromium only).
    #[must_use]
    pub fn cdp(&self) -> Option<&CdpConnection> {
        match &self.backend {
            Backend::Cdp(cdp) => Some(cdp),
            Backend::Bidi { .. } => None,
        }
    }

    /// Raw BiDi handle (`Some` on Firefox only).
    #[must_use]
    pub fn bidi(&self) -> Option<&BidiConnection> {
        match &self.backend {
            Backend::Cdp(_) => None,
            Backend::Bidi { conn, .. } => Some(conn),
        }
    }

    /// Browser product version (captured at launch).
    pub async fn version(&self) -> E2eResult<String> {
        Ok(self.product.clone())
    }

    /// Create an isolated browser context (incognito-equivalent).
    pub async fn new_context(&self, options: ContextOptions) -> E2eResult<BrowserContext> {
        // Stock engines apply proxies browser-wide at launch; a context that
        // asks for a different proxy fails loudly instead of lying.
        if options.proxy_server.is_some() && options.proxy_server != self.proxy_server {
            return Err(E2eError::Config(
                "per-context proxy differs from the launch proxy (stock engines \
                 apply proxies browser-wide); set LaunchOptions::proxy_server or \
                 [e2e] proxy_server"
                    .to_string(),
            ));
        }
        let id = match &self.backend {
            Backend::Cdp(cdp) => {
                let result = cdp
                    .call(
                        None,
                        "Target.createBrowserContext",
                        serde_json::json!({ "disposeOnDetach": true }),
                        self.timeout,
                    )
                    .await?;
                Some(
                    result
                        .get("browserContextId")
                        .and_then(Value::as_str)
                        .ok_or_else(|| E2eError::Launch("no browserContextId".to_string()))?
                        .to_string(),
                )
            }
            Backend::Bidi { conn, .. } => {
                let result = conn
                    .call(
                        "browser.createUserContext",
                        serde_json::json!({}),
                        self.timeout,
                    )
                    .await?;
                Some(
                    result
                        .get("userContext")
                        .and_then(Value::as_str)
                        .ok_or_else(|| E2eError::Launch("no userContext".to_string()))?
                        .to_string(),
                )
            }
        };
        let context = BrowserContext::new(
            self.backend.clone(),
            id,
            options,
            self.slow_mo,
            self.timeout,
            self.base_url.clone(),
            Arc::downgrade(&self.contexts),
        );
        self.contexts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(context.clone());
        Ok(context)
    }

    /// Default (shared) browser context.
    #[must_use]
    pub fn default_context(&self) -> BrowserContext {
        self.default
            .get_or_init(|| {
                let context = BrowserContext::new(
                    self.backend.clone(),
                    None,
                    ContextOptions::default(),
                    self.slow_mo,
                    self.timeout,
                    self.base_url.clone(),
                    Arc::downgrade(&self.contexts),
                );
                self.contexts
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(context.clone());
                context
            })
            .clone()
    }

    /// Open contexts (default context included once used).
    #[must_use]
    pub fn contexts(&self) -> Vec<BrowserContext> {
        self.contexts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Open pages across all contexts.
    #[must_use]
    pub fn pages(&self) -> Vec<Page> {
        self.contexts()
            .iter()
            .flat_map(BrowserContext::pages)
            .collect()
    }

    /// Open a page in the default context.
    pub async fn new_page(&self) -> E2eResult<Page> {
        self.default_context().new_page().await
    }

    /// Close the browser (ends the BiDi session, kills the child when this
    /// instance launched it).
    pub async fn close(mut self) -> E2eResult<()> {
        match &self.backend {
            Backend::Cdp(cdp) => cdp.close(),
            Backend::Bidi { conn, .. } => {
                let _ = conn
                    .call("session.end", serde_json::json!({}), Duration::from_secs(5))
                    .await;
                conn.close();
            }
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        }
        Ok(())
    }
}

/// Firefox profile prefs (`user.js`): UA override plus automation defaults.
/// Seed a fresh Chromium profile with a download directory.
fn write_chromium_download_pref(profile: &Path, dir: &Path) -> E2eResult<()> {
    std::fs::create_dir_all(dir)?;
    let prefs = serde_json::json!({
        "download": {
            "default_directory": dir.to_string_lossy(),
            "prompt_for_download": false,
            "directory_upgrade": true,
        },
    });
    std::fs::write(
        profile.join("Preferences"),
        serde_json::to_string(&prefs).map_err(E2eError::Json)?,
    )?;
    Ok(())
}

fn write_firefox_prefs(
    profile: &Path,
    user_agent: Option<&str>,
    download_dir: Option<&Path>,
) -> E2eResult<()> {
    let mut prefs = String::from(
        "user_pref(\"browser.shell.checkDefaultBrowser\", false);\n\
         user_pref(\"browser.sessionstore.resume_from_crash\", false);\n\
         user_pref(\"browser.aboutwelcome.enabled\", false);\n",
    );
    if let Some(user_agent) = user_agent {
        prefs.push_str(&format!(
            "user_pref(\"general.useragent.override\", {});\n",
            serde_json::to_string(user_agent)?
        ));
    }
    if let Some(dir) = download_dir {
        prefs.push_str(&format!(
            "user_pref(\"browser.download.dir\", {});\n\
             user_pref(\"browser.download.folderList\", 2);\n\
             user_pref(\"browser.download.useDownloadDir\", true);\n\
             user_pref(\"browser.download.manager.showWhenStarting\", false);\n\
             user_pref(\"browser.helperApps.neverAsk.saveToDisk\", \
             \"text/plain,application/octet-stream,application/pdf,text/csv,\
             application/json,text/html,image/png,image/jpeg\");\n",
            serde_json::to_string(&dir.to_string_lossy())?
        ));
    }
    std::fs::write(profile.join("user.js"), prefs)?;
    Ok(())
}

/// Classic WebDriver proxy capabilities for `session.new`.
fn proxy_capabilities(proxy: &str) -> Value {
    let proxy = proxy.trim();
    if let Some(rest) = proxy
        .strip_prefix("socks5://")
        .or_else(|| proxy.strip_prefix("socks://"))
    {
        return serde_json::json!({
            "proxyType": "manual",
            "socksProxy": rest,
            "socksVersion": 5,
        });
    }
    let host = proxy
        .strip_prefix("http://")
        .or_else(|| proxy.strip_prefix("https://"))
        .unwrap_or(proxy);
    serde_json::json!({
        "proxyType": "manual",
        "httpProxy": host,
        "sslProxy": host,
    })
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
            browser: "firefox".to_string(),
            headless: false,
            slow_mo_ms: 50,
            ..ferrite_config::E2eConfig::default()
        };
        let options = LaunchOptions::from_config(&config).unwrap();
        assert_eq!(options.browser, BrowserKind::Firefox);
        assert!(!options.headless);
        assert_eq!(options.slow_mo, Duration::from_millis(50));
    }

    #[test]
    fn browser_names_parse() {
        assert_eq!(
            BrowserKind::parse("chromium").unwrap(),
            BrowserKind::Chromium
        );
        assert_eq!(BrowserKind::parse("Chrome").unwrap(), BrowserKind::Chromium);
        assert_eq!(BrowserKind::parse("firefox").unwrap(), BrowserKind::Firefox);
        assert_eq!(BrowserKind::parse("ff").unwrap(), BrowserKind::Firefox);
        assert!(BrowserKind::parse("webkit").is_err());
        assert!(BrowserKind::parse("netscape").is_err());
        assert!(LaunchOptions::from_config(&ferrite_config::E2eConfig {
            browser: "webkit".to_string(),
            ..ferrite_config::E2eConfig::default()
        })
        .is_err());
    }

    #[test]
    fn proxy_capabilities_shape() {
        assert_eq!(
            proxy_capabilities("127.0.0.1:8080"),
            serde_json::json!({
                "proxyType": "manual",
                "httpProxy": "127.0.0.1:8080",
                "sslProxy": "127.0.0.1:8080",
            })
        );
        assert_eq!(
            proxy_capabilities("socks5://127.0.0.1:1080")["socksVersion"],
            Value::from(5)
        );
    }

    #[test]
    fn firefox_prefs_write_user_js() {
        let dir = tempfile::tempdir().unwrap();
        write_firefox_prefs(dir.path(), Some("TestAgent/1.0"), None).unwrap();
        let prefs = std::fs::read_to_string(dir.path().join("user.js")).unwrap();
        assert!(prefs.contains("general.useragent.override"), "{prefs}");
        assert!(prefs.contains("TestAgent/1.0"), "{prefs}");
    }

    #[test]
    fn chromium_hint_missing_is_ignored() {
        // A bogus hint must not itself be returned.
        let bogus = Path::new("/definitely/not/chrome");
        assert!(find_chromium(Some(bogus)).as_deref() != Some(bogus));
        let bogus = Path::new("/definitely/not/firefox");
        assert!(find_firefox(Some(bogus)).as_deref() != Some(bogus));
    }
}
