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

/// How a test runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestMode {
    /// Run normally.
    #[default]
    Run,
    /// Report skipped without running.
    Skip,
    /// Restrict the run to `Only` tests.
    Only,
    /// Report skipped (expected to fail, tracked separately).
    Fixme,
}

/// One named test.
#[derive(Clone)]
pub struct Test {
    /// Test name.
    pub name: String,
    /// Test body.
    pub func: TestFn,
    /// Tags for filtering.
    pub tags: Vec<String>,
    /// Run mode.
    pub mode: TestMode,
    /// Retry override (runner default when unset).
    pub retries: Option<u32>,
    /// Timeout override (runner default when unset).
    pub timeout: Option<Duration>,
    /// Triple the effective timeout.
    pub slow: bool,
}

impl Test {
    /// Add a tag for filtering.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Skip without running (reported as skipped).
    #[must_use]
    pub fn skip(mut self) -> Self {
        self.mode = TestMode::Skip;
        self
    }

    /// Run only `Only` tests (reported as skipped otherwise).
    #[must_use]
    pub fn only(mut self) -> Self {
        self.mode = TestMode::Only;
        self
    }

    /// Skip as a known failure (reported as skipped).
    #[must_use]
    pub fn fixme(mut self) -> Self {
        self.mode = TestMode::Fixme;
        self
    }

    /// Triple the effective timeout.
    #[must_use]
    pub fn slow(mut self) -> Self {
        self.slow = true;
        self
    }

    /// Override the retry count.
    #[must_use]
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Override the timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
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
        mode: TestMode::Run,
        retries: None,
        timeout: None,
        slow: false,
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

/// Select the tests to run: `Only` restriction, name-or-tag substring
/// filters, inverted grep, then one shard.
///
/// `shard` is 1-based (`shard(1, 3)` runs the first third by name order).
#[allow(clippy::too_many_arguments)]
pub(crate) fn select<'a>(
    tests: &'a [Test],
    filter: Option<&str>,
    grep: Option<&str>,
    grep_invert: Option<&str>,
    shard: Option<(usize, usize)>,
) -> Vec<&'a Test> {
    let mut selected: Vec<&Test> = tests.iter().collect();
    if selected.iter().any(|test| test.mode == TestMode::Only) {
        selected.retain(|test| test.mode == TestMode::Only);
    }
    for needle in filter.into_iter().chain(grep) {
        selected.retain(|test| {
            test.name.contains(needle) || test.tags.iter().any(|tag| tag.contains(needle))
        });
    }
    if let Some(needle) = grep_invert {
        selected.retain(|test| {
            !(test.name.contains(needle) || test.tags.iter().any(|tag| tag.contains(needle)))
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
/// A per-test hook (`Page` in, unit out).
pub type HookFn = Arc<dyn Fn(Page) -> BoxTestFuture + Send + Sync>;

/// A run-wide hook (no page).
pub type GlobalHook = Arc<dyn Fn() -> BoxTestFuture + Send + Sync>;

#[derive(Clone)]
pub struct Runner {
    workers: usize,
    retries: u32,
    test_timeout: Duration,
    filter: Option<String>,
    grep: Option<String>,
    grep_invert: Option<String>,
    shard: Option<(usize, usize)>,
    before_each: Vec<HookFn>,
    after_each: Vec<HookFn>,
    before_all: Vec<GlobalHook>,
    after_all: Vec<GlobalHook>,
    global_setup: Vec<GlobalHook>,
    global_teardown: Vec<GlobalHook>,
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
            grep_invert: None,
            shard: None,
            before_each: Vec::new(),
            after_each: Vec::new(),
            before_all: Vec::new(),
            after_all: Vec::new(),
            global_setup: Vec::new(),
            global_teardown: Vec::new(),
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
            grep_invert: None,
            shard: None,
            before_each: Vec::new(),
            after_each: Vec::new(),
            before_all: Vec::new(),
            after_all: Vec::new(),
            global_setup: Vec::new(),
            global_teardown: Vec::new(),
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

    /// Skip tests whose name or tags contain `grep_invert`.
    ///
    /// Builder values win over `FERRITE_E2E_GREP_INVERT` (set by `--grep-invert`).
    #[must_use]
    pub fn grep_invert(mut self, grep_invert: impl Into<String>) -> Self {
        self.grep_invert = Some(grep_invert.into());
        self
    }

    /// Run `hook` before every attempt (failures fail the attempt).
    #[must_use]
    pub fn before_each<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Page) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.before_each
            .push(Arc::new(move |page| Box::pin(hook(page))));
        self
    }

    /// Run `hook` after every attempt (failures fail the attempt).
    #[must_use]
    pub fn after_each<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Page) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_each
            .push(Arc::new(move |page| Box::pin(hook(page))));
        self
    }

    /// Run `hook` once after test selection, before any test runs.
    ///
    /// Runs after [`Runner::global_setup`]; failures abort with one failed
    /// result (unlike `before_each`, there is no page yet).
    #[must_use]
    pub fn before_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.before_all.push(Arc::new(move || Box::pin(hook())));
        self
    }

    /// Run `hook` once after all tests complete (failures append one result).
    ///
    /// Runs before [`Runner::global_teardown`].
    #[must_use]
    pub fn after_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_all.push(Arc::new(move || Box::pin(hook())));
        self
    }

    /// Run `hook` once before the run (failures abort with one failed result).
    #[must_use]
    pub fn global_setup<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.global_setup.push(Arc::new(move || Box::pin(hook())));
        self
    }

    /// Run `hook` once after the run (failures append one failed result).
    #[must_use]
    pub fn global_teardown<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.global_teardown
            .push(Arc::new(move || Box::pin(hook())));
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
    /// `FERRITE_E2E_GREP`, `FERRITE_E2E_GREP_INVERT`, and `FERRITE_E2E_SHARD`
    /// (set by the CLI flags).
    pub async fn run(&self, browser: &Browser, tests: Vec<Test>) -> TestReport {
        let mut report = TestReport::default();
        for setup in &self.global_setup {
            if let Err(error) = setup().await {
                report.results.push(TestResult {
                    name: "<global setup>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 1,
                    duration_ms: 0,
                    error: Some(error.to_string()),
                    screenshots: vec![],
                    trace: None,
                    video: None,
                });
                return report;
            }
        }
        let filter = self
            .filter
            .clone()
            .or_else(|| env_filter("FERRITE_E2E_FILTER"));
        let grep = self.grep.clone().or_else(|| env_filter("FERRITE_E2E_GREP"));
        let grep_invert = self
            .grep_invert
            .clone()
            .or_else(|| env_filter("FERRITE_E2E_GREP_INVERT"));
        let shard = self.shard.or_else(shard_from_env);
        let selected: Vec<Test> = select(
            &tests,
            filter.as_deref(),
            grep.as_deref(),
            grep_invert.as_deref(),
            shard,
        )
        .into_iter()
        .cloned()
        .collect();
        let mut runnable = Vec::new();
        for test in selected {
            match test.mode {
                TestMode::Skip | TestMode::Fixme => {
                    if self.list_progress {
                        println!("[skip] {}", test.name);
                    }
                    report.results.push(TestResult {
                        name: test.name.clone(),
                        status: TestStatus::Skipped,
                        attempts: 0,
                        duration_ms: 0,
                        error: None,
                        screenshots: vec![],
                        trace: None,
                        video: None,
                    });
                }
                TestMode::Run | TestMode::Only => runnable.push(test),
            }
        }
        for hook in &self.before_all {
            if let Err(error) = hook().await {
                report.results.push(TestResult {
                    name: "<before_all>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 1,
                    duration_ms: 0,
                    error: Some(error.to_string()),
                    screenshots: vec![],
                    trace: None,
                    video: None,
                });
                return report;
            }
        }
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.workers));
        let mut handles = Vec::new();
        for test in runnable {
            let permit = semaphore.clone().acquire_owned().await.expect("semaphore");
            let runner = self.clone();
            let context = browser.default_context();
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                run_one(&runner, &context, &test).await
            }));
        }
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
        for hook in &self.after_all {
            if let Err(error) = hook().await {
                report.results.push(TestResult {
                    name: "<after_all>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 1,
                    duration_ms: 0,
                    error: Some(error.to_string()),
                    screenshots: vec![],
                    trace: None,
                    video: None,
                });
            }
        }
        for teardown in &self.global_teardown {
            if let Err(error) = teardown().await {
                report.results.push(TestResult {
                    name: "<global teardown>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 1,
                    duration_ms: 0,
                    error: Some(error.to_string()),
                    screenshots: vec![],
                    trace: None,
                    video: None,
                });
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
                "html" => {
                    let path = format!("{}/report.html", self.output_dir);
                    if std::fs::write(&path, report.to_html()).is_ok() {
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
    let retries = test.retries.unwrap_or(runner.retries);
    let mut timeout = test.timeout.unwrap_or(runner.test_timeout);
    if test.slow {
        timeout = timeout.saturating_mul(3);
    }

    for _ in 0..=retries {
        attempts += 1;
        let page = match context.new_page().await {
            Ok(page) => page,
            Err(error) => {
                last_error = error.to_string();
                continue;
            }
        };
        let mut hooked = None;
        for hook in &runner.before_each {
            if let Err(error) = hook(page.clone()).await {
                hooked = Some(format!("before_each: {error}"));
                break;
            }
        }
        if let Some(error) = hooked {
            last_error = error;
            page.close().await.ok();
            continue;
        }
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
        let outcome = tokio::time::timeout(timeout, (test.func)(page.clone())).await;
        let mut failed = match outcome {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(error.to_string()),
            Err(_) => Some(format!("test timed out after {}ms", timeout.as_millis())),
        };
        for hook in &runner.after_each {
            if let Err(error) = hook(page.clone()).await {
                let note = format!("after_each: {error}");
                failed = Some(match failed {
                    Some(prior) => format!("{prior} ({note})"),
                    None => note,
                });
            }
        }
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
        assert_eq!(names(select(&tests, None, None, None, None)).len(), 3);
        assert_eq!(
            names(select(&tests, Some("auth"), None, None, None)),
            vec!["auth > login".to_string(), "auth > logout".to_string()]
        );
        assert_eq!(
            names(select(&tests, Some("fast"), None, None, None)),
            vec!["auth > login".to_string()]
        );
        // grep ANDs with filter.
        assert_eq!(
            names(select(&tests, Some("auth"), Some("slow"), None, None)),
            vec!["auth > logout".to_string()]
        );
        assert_eq!(
            select(&tests, Some("auth"), Some("fast"), None, None).len(),
            1
        );
        assert!(select(&tests, Some("zzz"), None, None, None).is_empty());
    }

    #[test]
    fn select_inverts_grep() {
        let tests = vec![
            named("home renders"),
            named("auth > login").tag("fast"),
            named("auth > logout").tag("slow"),
        ];
        let selected = select(&tests, Some("auth"), None, Some("slow"), None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "auth > login");
        let selected = select(&tests, None, None, Some("auth"), None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "home renders");
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
            names(select(&tests, None, None, None, Some((1, 2)))),
            vec!["a".to_string(), "c".to_string()]
        );
        assert_eq!(
            names(select(&tests, None, None, None, Some((2, 2)))),
            vec!["b".to_string(), "d".to_string()]
        );
        assert_eq!(
            names(select(&tests, None, None, None, Some((3, 3)))),
            vec!["c".to_string()]
        );
        // Filters apply before sharding.
        assert_eq!(
            names(select(&tests, Some("a"), None, None, Some((1, 2)))),
            vec!["a".to_string()]
        );
    }

    #[test]
    fn select_restricts_to_only() {
        let tests = vec![named("a"), named("b").only(), named("c")];
        let selected = select(&tests, None, None, None, None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "b");
    }

    #[test]
    fn modes_and_overrides_build() {
        let test = named("a")
            .skip()
            .retries(2)
            .timeout(Duration::from_secs(5))
            .slow();
        assert_eq!(test.mode, TestMode::Skip);
        assert_eq!(test.retries, Some(2));
        assert_eq!(test.timeout, Some(Duration::from_secs(5)));
        assert!(test.slow);
        assert_eq!(named("b").fixme().mode, TestMode::Fixme);
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

    #[test]
    fn artifacts_write_html() {
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-rep-html-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let runner = Runner::default().output_dir(dir.display().to_string());
        let report = TestReport {
            results: vec![TestResult {
                name: "a".to_string(),
                status: TestStatus::Failed,
                attempts: 2,
                duration_ms: 5,
                error: Some("boom".to_string()),
                screenshots: vec![],
                trace: None,
                video: None,
            }],
        };
        let written = runner.write_artifacts(&report, "html");
        assert_eq!(written.len(), 1);
        let html = std::fs::read_to_string(dir.join("report.html")).unwrap();
        assert!(html.contains(">a<"), "{html}");
        assert!(html.contains("boom"), "{html}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
