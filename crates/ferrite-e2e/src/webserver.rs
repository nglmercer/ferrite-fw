//! Web-server lifecycle for e2e runs (boot, reuse, readiness wait).

use std::time::Duration;

use crate::error::{E2eError, E2eResult};

/// Poll `url` until it answers 2xx/3xx or the timeout expires.
pub async fn wait_for_url(url: &str, timeout: Duration) -> E2eResult<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| E2eError::WebServer(error.to_string()))?;
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        match client.get(url).send().await {
            Ok(response)
                if response.status().is_success() || response.status().is_redirection() =>
            {
                return Ok(())
            }
            _ => {}
        }
        if tokio::time::Instant::now() > deadline {
            return Err(E2eError::WebServer(format!(
                "{url} did not answer within {}ms",
                timeout.as_millis()
            )));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// A running (or reused) web server for tests.
pub struct RunningWebServer {
    /// Base URL tests should use.
    pub url: String,
    child: Option<tokio::process::Child>,
}

impl RunningWebServer {
    /// Stop the server when this instance booted it.
    pub async fn shutdown(mut self) -> E2eResult<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        }
        Ok(())
    }

    /// True when this instance spawned the process.
    #[must_use]
    pub fn spawned(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for RunningWebServer {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
    }
}

/// Boots or reuses the configured web server.
pub struct WebServer {
    command: Option<String>,
    url: String,
    timeout: Duration,
    reuse_existing: bool,
}

impl WebServer {
    /// Build from e2e config with CLI overrides.
    pub fn from_config(
        config: &ferrite_config::E2eConfig,
        url_override: Option<String>,
        command_override: Option<String>,
    ) -> Self {
        let server = config.web_server.clone().unwrap_or_default();
        let url = url_override
            .or(server.url)
            .or_else(|| config.base_url.clone())
            .unwrap_or_else(|| "http://127.0.0.1:5190/".to_string());
        Self {
            command: command_override.or(server.command),
            url,
            timeout: Duration::from_millis(server.timeout_ms.max(1_000)),
            reuse_existing: server.reuse_existing,
        }
    }

    /// Target URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Ensure the server answers: reuse it, or spawn `command` and wait.
    ///
    /// With no command, only readiness is checked (the caller — e.g.
    /// `ferrite e2e` with its in-process dev server — owns the boot).
    pub async fn ensure_running(&self) -> E2eResult<RunningWebServer> {
        if self.reuse_existing
            && wait_for_url(&self.url, Duration::from_secs(1))
                .await
                .is_ok()
        {
            tracing::info!(url = %self.url, "reusing running web server");
            return Ok(RunningWebServer {
                url: self.url.clone(),
                child: None,
            });
        }
        let Some(command) = &self.command else {
            wait_for_url(&self.url, self.timeout).await?;
            return Ok(RunningWebServer {
                url: self.url.clone(),
                child: None,
            });
        };
        tracing::info!(%command, url = %self.url, "starting web server");
        let mut child = spawn_shell(command)?;
        match wait_for_url(&self.url, self.timeout).await {
            Ok(()) => Ok(RunningWebServer {
                url: self.url.clone(),
                child: Some(child),
            }),
            Err(error) => {
                let _ = child.kill().await;
                Err(error)
            }
        }
    }
}

fn spawn_shell(command: &str) -> E2eResult<tokio::process::Child> {
    #[cfg(windows)]
    let mut cmd = {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(["/c", command]);
        cmd
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    cmd.spawn()
        .map_err(|error| E2eError::WebServer(format!("spawn {command:?}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn readiness_detects_local_server() {
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "ok" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await });
        let url = format!("http://{addr}/");
        wait_for_url(&url, Duration::from_secs(5)).await.unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn readiness_times_out_loudly() {
        let error = wait_for_url("http://127.0.0.1:1/", Duration::from_millis(400))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("did not answer"), "{error}");
    }

    #[test]
    fn from_config_prefers_overrides() {
        let config = ferrite_config::E2eConfig {
            base_url: Some("http://base/".to_string()),
            ..ferrite_config::E2eConfig::default()
        };
        let server = WebServer::from_config(&config, Some("http://over/".to_string()), None);
        assert_eq!(server.url(), "http://over/");
        let server = WebServer::from_config(&config, None, None);
        assert_eq!(server.url(), "http://base/");
    }
}
