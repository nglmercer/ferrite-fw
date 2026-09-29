//! Parallel test runner with retries, timeouts, and artifact capture.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::browser::Browser;
use crate::error::E2eResult;
use crate::page::{Page, ScreenshotOptions};
use crate::report::{TestReport, TestResult, TestStatus};

/// A boxed test future.
pub type BoxTestFuture = Pin<Box<dyn Future<Output = E2eResult<()>> + Send + 'static>>;

/// A test body: receives a fresh [`Page`].
pub type TestFn = Arc<dyn Fn(Page) -> BoxTestFuture + Send + Sync>;

/// One named test.
#[derive(Clone)]
pub struct Test {
    /// Test name.
    pub name: String,
    /// Test body.
    pub func: TestFn,
}

/// Define a test.
pub fn test<F, Fut>(name: impl Into<String>, func: F) -> Test
where
    F: Fn(Page) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = E2eResult<()>> + Send + 'static,
{
    Test {
        name: name.into(),
        func: Arc::new(move |page| Box::pin(func(page))),
    }
}

/// Runs tests against a [`Browser`] with workers, retries, and artifacts.
#[derive(Debug, Clone)]
pub struct Runner {
    workers: usize,
    retries: u32,
    test_timeout: Duration,
    filter: Option<String>,
    output_dir: String,
    screenshot_on_failure: bool,
    screenshot_always: bool,
    write_trace: bool,
    list_progress: bool,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            workers: 4,
            retries: 0,
            test_timeout: Duration::from_secs(30),
            filter: None,
            output_dir: "test-results".to_string(),
            screenshot_on_failure: true,
            screenshot_always: false,
            write_trace: true,
            list_progress: true,
        }
    }
}

impl Runner {
    /// Build from resolved e2e config.
    #[must_use]
    pub fn from_config(config: &ferrite_config::E2eConfig) -> Self {
        Self {
            workers: config.workers.max(1),
            retries: config.retries,
            test_timeout: Duration::from_millis(config.timeout_ms.max(1)),
            filter: None,
            output_dir: config.output_dir.clone(),
            screenshot_on_failure: config.screenshot_on_failure(),
            screenshot_always: config.screenshot_always(),
            write_trace: true,
            list_progress: true,
        }
    }

    /// Parallel workers.
    #[must_use]
    pub fn workers(mut self, workers: usize) -> Self {
        self.workers = workers.max(1);
        self
    }

    /// Retries after the first attempt.
    #[must_use]
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Per-attempt timeout.
    #[must_use]
    pub fn test_timeout(mut self, timeout: Duration) -> Self {
        self.test_timeout = timeout;
        self
    }

    /// Only run tests whose name contains `filter`.
    #[must_use]
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Artifact directory.
    #[must_use]
    pub fn output_dir(mut self, dir: impl Into<String>) -> Self {
        self.output_dir = dir.into();
        self
    }

    /// Print `list` progress lines while running.
    #[must_use]
    pub fn list_progress(mut self, enabled: bool) -> Self {
        self.list_progress = enabled;
        self
    }

    /// Run tests to completion (never fails the call itself).
    pub async fn run(&self, browser: &Browser, tests: Vec<Test>) -> TestReport {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.workers));
        let mut handles = Vec::new();
        for test in tests {
            if let Some(filter) = &self.filter {
                if !test.name.contains(filter) {
                    continue;
                }
            }
            let permit = semaphore.clone().acquire_owned().await.expect("semaphore");
            let runner = self.clone();
            let cdp = browser.cdp().clone();
            let slow_mo = browser.slow_mo();
            let timeout = browser.timeout();
            let base_url = browser.base_url().map(str::to_string);
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                run_one(&runner, &cdp, slow_mo, timeout, base_url.as_deref(), &test).await
            }));
        }
        let mut report = TestReport::default();
        for handle in handles {
            match handle.await {
                Ok(result) => {
                    if self.list_progress {
                        let mark = match result.status {
                            TestStatus::Passed => "ok",
                            TestStatus::Failed => "FAIL",
                            TestStatus::Skipped => "skip",
                        };
                        println!("[{mark}] {}", result.name);
                        if let Some(error) = &result.error {
                            println!("       {error}");
                        }
                    }
                    report.results.push(result);
                }
                Err(error) => report.results.push(TestResult {
                    name: "<join>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 1,
                    duration_ms: 0,
                    error: Some(error.to_string()),
                    screenshots: vec![],
                    trace: None,
                }),
            }
        }
        report.results.sort_by(|a, b| a.name.cmp(&b.name));
        report
    }

    /// Write `json`/`junit` artifacts for the reporters in `spec`
    /// (comma-separated `list`, `json`, `junit`).
    pub fn write_artifacts(&self, report: &TestReport, spec: &str) -> Vec<String> {
        let mut written = Vec::new();
        std::fs::create_dir_all(&self.output_dir).ok();
        for reporter in spec.split(',').map(str::trim) {
            match reporter {
                "json" => {
                    let path = format!("{}/results.json", self.output_dir);
                    if std::fs::write(&path, report.to_json()).is_ok() {
                        written.push(path);
                    }
                }
                "junit" => {
                    let path = format!("{}/junit.xml", self.output_dir);
                    if std::fs::write(&path, report.to_junit()).is_ok() {
                        written.push(path);
                    }
                }
                _ => {}
            }
        }
        written
    }
}

async fn run_one(
    runner: &Runner,
    cdp: &crate::cdp::CdpConnection,
    slow_mo: Duration,
    browser_timeout: Duration,
    base_url: Option<&str>,
    test: &Test,
) -> TestResult {
    let started = Instant::now();
    let mut attempts = 0;
    let mut last_error = String::new();
    let mut screenshots = Vec::new();
    let mut trace_path = None;
    let slug = slug(&test.name);

    for _ in 0..=runner.retries {
        attempts += 1;
        let page = match open_page(cdp, slow_mo, browser_timeout, base_url).await {
            Ok(page) => page,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        let outcome = tokio::time::timeout(runner.test_timeout, (test.func)(page.clone())).await;
        let failed = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some(format!(
                "test timed out after {}ms",
                runner.test_timeout.as_millis()
            )),
        };
        let take_shot =
            runner.screenshot_always || (failed.is_some() && runner.screenshot_on_failure);
        if take_shot {
            let path = std::path::Path::new(&runner.output_dir)
                .join(format!("{slug}-attempt{attempts}.png"));
            if page
                .save_screenshot(&path, ScreenshotOptions::default())
                .await
                .is_ok()
            {
                screenshots.push(path.display().to_string());
            }
        }
        if runner.write_trace {
            let path = std::path::Path::new(&runner.output_dir).join(format!("{slug}.json"));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let payload = serde_json::json!({
                "test": test.name,
                "attempt": attempts,
                "console": page.console_messages(),
                "trace": page.trace(),
            });
            if std::fs::write(
                &path,
                serde_json::to_string_pretty(&payload).unwrap_or_default(),
            )
            .is_ok()
            {
                trace_path = Some(path.display().to_string());
            }
        }
        page.close().await.ok();
        match failed {
            None => {
                return TestResult {
                    name: test.name.clone(),
                    status: TestStatus::Passed,
                    attempts,
                    duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    error: None,
                    screenshots,
                    trace: trace_path,
                };
            }
            Some(error) => last_error = error,
        }
    }
    TestResult {
        name: test.name.clone(),
        status: TestStatus::Failed,
        attempts,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        error: Some(last_error),
        screenshots,
        trace: trace_path,
    }
}

async fn open_page(
    cdp: &crate::cdp::CdpConnection,
    slow_mo: Duration,
    timeout: Duration,
    base_url: Option<&str>,
) -> E2eResult<Page> {
    let target = cdp
        .call(
            None,
            "Target.createTarget",
            serde_json::json!({ "url": "about:blank" }),
            timeout,
        )
        .await?;
    let target_id = target
        .get("targetId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| crate::error::E2eError::Launch("no targetId".to_string()))?
        .to_string();
    let attached = cdp
        .call(
            None,
            "Target.attachToTarget",
            serde_json::json!({ "targetId": target_id, "flatten": true }),
            timeout,
        )
        .await?;
    let session = attached
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| crate::error::E2eError::Launch("no sessionId".to_string()))?
        .to_string();
    Page::new(
        cdp.clone(),
        session,
        target_id,
        slow_mo,
        timeout,
        base_url.map(str::to_string),
    )
    .await
}

pub(crate) fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "test".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_sanitizes() {
        assert_eq!(slug("home renders!"), "home-renders");
        assert_eq!(slug("  A/B: c  "), "a-b-c");
        assert_eq!(slug("!!!"), "test");
    }

    #[test]
    fn runner_from_config() {
        let config = ferrite_config::E2eConfig {
            workers: 8,
            retries: 2,
            output_dir: "out".to_string(),
            ..ferrite_config::E2eConfig::default()
        };
        let runner = Runner::from_config(&config);
        assert_eq!(runner.workers, 8);
        assert_eq!(runner.retries, 2);
        assert_eq!(runner.output_dir, "out");
    }

    #[test]
    fn artifacts_write_json_and_junit() {
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-rep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let runner = Runner::default().output_dir(dir.display().to_string());
        let report = TestReport {
            results: vec![TestResult {
                name: "a".to_string(),
                status: TestStatus::Passed,
                attempts: 1,
                duration_ms: 5,
                error: None,
                screenshots: vec![],
                trace: None,
            }],
        };
        let written = runner.write_artifacts(&report, "list,json,junit");
        assert_eq!(written.len(), 2);
        assert!(dir.join("results.json").is_file());
        assert!(dir.join("junit.xml").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
