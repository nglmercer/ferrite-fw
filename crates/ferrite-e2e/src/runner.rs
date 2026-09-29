//! Parallel test runner with retries, timeouts, and artifact capture.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::browser::Browser;
use crate::error::E2eResult;
use crate::page::{Page, ScreenshotOptions};
use crate::report::{TestReport, TestResult, TestStatus};
use crate::video::{VideoMode, VideoOptions};

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
    /// Tags for filtering.
    pub tags: Vec<String>,
}

impl Test {
    /// Add a tag for filtering.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
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
        tags: Vec::new(),
    }
}

/// Group tests under `name` (`"group > test"`); nests naturally.
#[must_use]
pub fn describe(name: &str, tests: Vec<Test>) -> Vec<Test> {
    tests
        .into_iter()
        .map(|mut test| {
            test.name = format!("{name} > {}", test.name);
            test
        })
        .collect()
}

/// Select the tests to run: name-or-tag substring filters, then one shard.
///
/// `shard` is 1-based (`shard(1, 3)` runs the first third by name order).
pub(crate) fn select<'a>(
    tests: &'a [Test],
    filter: Option<&str>,
    grep: Option<&str>,
    shard: Option<(usize, usize)>,
) -> Vec<&'a Test> {
    let mut selected: Vec<&Test> = tests.iter().collect();
    for needle in filter.into_iter().chain(grep) {
        selected.retain(|test| {
            test.name.contains(needle) || test.tags.iter().any(|tag| tag.contains(needle))
        });
    }
    if let Some((index, total)) = shard {
        let mut ordered = selected;
        ordered.sort_by(|a: &&Test, b: &&Test| a.name.cmp(&b.name));
        let total = total.max(1);
        let want = index.saturating_sub(1) % total;
        selected = ordered
            .into_iter()
            .enumerate()
            .filter(|(position, _)| position % total == want)
            .map(|(_, test)| test)
            .collect();
    }
    selected
}

/// Shard from `FERRITE_E2E_SHARD` (`"1/3"`); warns and ignores garbage.
fn shard_from_env() -> Option<(usize, usize)> {
    let raw = std::env::var("FERRITE_E2E_SHARD").ok()?;
    let (index, total) = raw.split_once('/')?;
    let index: usize = index.trim().parse().ok()?;
    let total: usize = total.trim().parse().ok()?;
    if index < 1 || index > total {
        eprintln!(
            "warning: ignoring invalid FERRITE_E2E_SHARD={raw:?} (want 1-based index within total)"
        );
        return None;
    }
    Some((index, total))
}

/// Non-empty env var, if set.
fn env_filter(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|value| !value.is_empty())
}

/// Runs tests against a [`Browser`] with workers, retries, and artifacts.
#[derive(Debug, Clone)]
pub struct Runner {
    workers: usize,
    retries: u32,
    test_timeout: Duration,
    filter: Option<String>,
    grep: Option<String>,
    shard: Option<(usize, usize)>,
    output_dir: String,
    screenshot_on_failure: bool,
    screenshot_always: bool,
    write_trace: bool,
    list_progress: bool,
    video: VideoMode,
    video_fps: u32,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            workers: 4,
            retries: 0,
            test_timeout: Duration::from_secs(30),
            filter: None,
            grep: None,
            shard: None,
            output_dir: "test-results".to_string(),
            screenshot_on_failure: true,
            screenshot_always: false,
            write_trace: true,
            list_progress: true,
            video: VideoMode::Off,
            video_fps: 10,
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
            grep: None,
            shard: None,
            output_dir: config.output_dir.clone(),
            screenshot_on_failure: config.screenshot_on_failure(),
            screenshot_always: config.screenshot_always(),
            write_trace: true,
            list_progress: true,
            video: VideoMode::parse(&config.video).unwrap_or(VideoMode::Off),
            video_fps: config.video_fps.max(1),
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

    /// Only run tests whose name or tags contain `filter`.
    ///
    /// Builder values win over `FERRITE_E2E_FILTER` (set by `--filter`).
    #[must_use]
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Only run tests whose name or tags contain `grep` (ANDed with `filter`).
    ///
    /// Builder values win over `FERRITE_E2E_GREP` (set by `--grep`).
    #[must_use]
    pub fn grep(mut self, grep: impl Into<String>) -> Self {
        self.grep = Some(grep.into());
        self
    }

    /// Run one shard: 1-based `index` of `total` by name order.
    ///
    /// Builder values win over `FERRITE_E2E_SHARD` (set by `--shard`).
    ///
    /// # Panics
    ///
    /// Panics on a zero index/total or an index past the total.
    #[must_use]
    pub fn shard(mut self, index: usize, total: usize) -> Self {
        assert!(
            index >= 1 && total >= 1 && index <= total,
            "invalid shard {index}/{total} (want a 1-based index within the total)"
        );
        self.shard = Some((index, total));
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

    /// Video recording mode.
    #[must_use]
    pub fn video_mode(mut self, mode: VideoMode) -> Self {
        self.video = mode;
        self
    }

    /// Recording frames per second.
    #[must_use]
    pub fn video_fps(mut self, fps: u32) -> Self {
        self.video_fps = fps.max(1);
        self
    }

    /// Run tests to completion (never fails the call itself).
    ///
    /// Unset builder filters fall back to `FERRITE_E2E_FILTER`,
    /// `FERRITE_E2E_GREP`, and `FERRITE_E2E_SHARD` (set by the CLI flags).
    pub async fn run(&self, browser: &Browser, tests: Vec<Test>) -> TestReport {
        let filter = self
            .filter
            .clone()
            .or_else(|| env_filter("FERRITE_E2E_FILTER"));
        let grep = self.grep.clone().or_else(|| env_filter("FERRITE_E2E_GREP"));
        let shard = self.shard.or_else(shard_from_env);
        let selected: Vec<Test> = select(&tests, filter.as_deref(), grep.as_deref(), shard)
            .into_iter()
            .cloned()
            .collect();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.workers));
        let mut handles = Vec::new();
        for test in selected {
            let permit = semaphore.clone().acquire_owned().await.expect("semaphore");
            let runner = self.clone();
            let context = browser.default_context();
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                run_one(&runner, &context, &test).await
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
                    video: None,
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
    context: &crate::context::BrowserContext,
    test: &Test,
) -> TestResult {
    let started = Instant::now();
    let mut attempts = 0;
    let mut last_error = String::new();
    let mut screenshots = Vec::new();
    let mut trace_path = None;
    let mut video_path = None;
    let slug = slug(&test.name);

    for _ in 0..=runner.retries {
        attempts += 1;
        let page = match context.new_page().await {
            Ok(page) => page,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        let recording = if runner.video.records() {
            match page
                .start_video(VideoOptions {
                    dir: std::path::PathBuf::from(&runner.output_dir),
                    fps: runner.video_fps,
                    ..VideoOptions::default()
                })
                .await
            {
                Ok(()) => true,
                Err(error) => {
                    last_error = format!("start video: {error}");
                    page.close().await.ok();
                    continue;
                }
            }
        } else {
            false
        };
        let outcome = tokio::time::timeout(runner.test_timeout, (test.func)(page.clone())).await;
        let mut failed = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some(format!(
                "test timed out after {}ms",
                runner.test_timeout.as_millis()
            )),
        };
        if recording {
            let keep = runner.video == VideoMode::On
                || (runner.video == VideoMode::OnlyOnFailure && failed.is_some());
            if keep {
                let path = std::path::Path::new(&runner.output_dir)
                    .join(format!("{slug}-attempt{attempts}.webm"));
                match page.stop_video(&path).await {
                    Ok(done) => video_path = Some(done.display().to_string()),
                    Err(error) => {
                        let note = format!("stop video: {error}");
                        failed = Some(match failed {
                            Some(prior) => format!("{prior} ({note})"),
                            None => note,
                        });
                    }
                }
            } else {
                page.cancel_video().await;
            }
        }
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
                    video: video_path,
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
        video: video_path,
    }
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

    fn named(name: &str) -> Test {
        test(name, |_| async { Ok(()) })
    }

    #[test]
    fn describe_prefixes_and_nests() {
        let tests = describe("auth", vec![named("login"), named("logout")]);
        assert_eq!(tests[0].name, "auth > login");
        assert_eq!(tests[1].name, "auth > logout");
        let nested = describe("app", tests);
        assert_eq!(nested[0].name, "app > auth > login");
    }

    #[test]
    fn select_matches_names_and_tags() {
        let tests = vec![
            named("home renders"),
            named("auth > login").tag("fast"),
            named("auth > logout").tag("slow"),
        ];
        let names = |selected: Vec<&Test>| {
            selected
                .iter()
                .map(|test| test.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(select(&tests, None, None, None)).len(), 3);
        assert_eq!(
            names(select(&tests, Some("auth"), None, None)),
            vec!["auth > login".to_string(), "auth > logout".to_string()]
        );
        assert_eq!(
            names(select(&tests, Some("fast"), None, None)),
            vec!["auth > login".to_string()]
        );
        // grep ANDs with filter.
        assert_eq!(
            names(select(&tests, Some("auth"), Some("slow"), None)),
            vec!["auth > logout".to_string()]
        );
        assert_eq!(select(&tests, Some("auth"), Some("fast"), None).len(), 1);
        assert!(select(&tests, Some("zzz"), None, None).is_empty());
    }

    #[test]
    fn select_shards_by_name_order() {
        let tests = vec![named("c"), named("a"), named("b"), named("d")];
        let names = |selected: Vec<&Test>| {
            selected
                .iter()
                .map(|test| test.name.clone())
                .collect::<Vec<_>>()
        };
        // Sorted a,b,c,d; shard 1/2 takes positions 0,2.
        assert_eq!(
            names(select(&tests, None, None, Some((1, 2)))),
            vec!["a".to_string(), "c".to_string()]
        );
        assert_eq!(
            names(select(&tests, None, None, Some((2, 2)))),
            vec!["b".to_string(), "d".to_string()]
        );
        assert_eq!(
            names(select(&tests, None, None, Some((3, 3)))),
            vec!["c".to_string()]
        );
        // Filters apply before sharding.
        assert_eq!(
            names(select(&tests, Some("a"), None, Some((1, 2)))),
            vec!["a".to_string()]
        );
    }

    #[test]
    #[should_panic(expected = "invalid shard 0/2")]
    fn shard_rejects_zero_index() {
        let _ = Runner::default().shard(0, 2);
    }

    #[test]
    #[should_panic(expected = "invalid shard 3/2")]
    fn shard_rejects_index_past_total() {
        let _ = Runner::default().shard(3, 2);
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
                video: None,
            }],
        };
        let written = runner.write_artifacts(&report, "list,json,junit");
        assert_eq!(written.len(), 2);
        assert!(dir.join("results.json").is_file());
        assert!(dir.join("junit.xml").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
