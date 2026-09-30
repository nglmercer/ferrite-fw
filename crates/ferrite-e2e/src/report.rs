//! Test results and reporters (`list`, `dot`, `json`, `junit`, `html`).

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Identity of one attempt. Retries and repetitions have separate identities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptInfo {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub project: Option<String>,
    pub worker_index: usize,
    pub repeat_each_index: u32,
    pub retry: u32,
}

/// Raw execution status, independent of whether failure was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttemptStatus {
    Passed,
    Failed,
    TimedOut,
    Skipped,
    Interrupted,
}

/// Source position of a test or user step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}
impl SourceLocation {
    pub(crate) fn caller(location: &'static std::panic::Location<'static>) -> Self {
        Self {
            file: location.file().into(),
            line: location.line(),
            column: location.column(),
        }
    }
}

/// A structured failure with its runner phase and stable error code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestError {
    pub message: String,
    pub code: String,
    pub phase: String,
    pub location: Option<SourceLocation>,
}
impl TestError {
    pub(crate) fn new(
        error: &crate::E2eError,
        phase: &str,
        location: Option<SourceLocation>,
    ) -> Self {
        Self {
            message: error.to_string(),
            code: error.code().into(),
            phase: phase.into(),
            location,
        }
    }
}

/// One attempt-owned soft mismatch, including its collection source and step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftAssertionFailure {
    /// Contextual assertion failure, also present in the attempt errors.
    pub error: TestError,
    /// Optional caller-supplied assertion message.
    pub message: Option<String>,
    /// Owning assertion or user step, when collection occurs inside one.
    pub step_id: Option<u64>,
    /// Owning step path, or the test identity when no step is active.
    pub title_path: Vec<String>,
}

/// Category of a recorded step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepCategory {
    #[default]
    User,
    Action,
    Assertion,
    Hook,
    Fixture,
}
/// Execution outcome of a step, independent of the enclosing test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Running,
    #[default]
    Passed,
    Failed,
    Skipped,
    TimedOut,
    Interrupted,
}
/// A step annotation with optional Rust source metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepAnnotation {
    pub kind: String,
    pub description: String,
    pub location: Option<SourceLocation>,
}
/// Controls for a user step. Zero timeout inherits the enclosing test budget.
#[derive(Debug, Clone, Default)]
pub struct StepOptions {
    pub timeout: std::time::Duration,
    /// Record a skipped step without constructing or polling its body.
    pub skip: Option<String>,
    pub annotations: Vec<StepAnnotation>,
}
impl StepOptions {
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }
    pub fn skip(mut self, reason: impl Into<String>) -> Self {
        self.skip = Some(reason.into());
        self
    }
    pub fn annotate(mut self, kind: impl Into<String>, description: impl Into<String>) -> Self {
        self.annotations.push(StepAnnotation {
            kind: kind.into(),
            description: description.into(),
            location: None,
        });
        self
    }
}
/// A controlled step may finish without producing a value when skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome<T> {
    Completed(T),
    Skipped(String),
}
/// Live control and metadata for a user step. Clones share annotations/skip state.
#[derive(Clone)]
pub struct StepContext {
    session: StepSession,
    id: u64,
    skip: crate::CancellationToken,
}
impl StepContext {
    /// Abort this step; `skip(reason)?` also exits the current closure immediately.
    /// Skipping a step does not skip its enclosing test or parent step.
    #[track_caller]
    pub fn skip(&self, reason: impl Into<String>) -> crate::E2eResult<()> {
        let reason = reason.into();
        // Serialize the annotation and cancellation across shared contexts so
        // simultaneous skip requests retain one reason and one annotation.
        let mut records = self
            .session
            .records
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !self.skip.is_cancelled() {
            if !records.sealed {
                if let Some((step, _, false)) = records.nodes.iter_mut().find(|n| n.0.id == self.id)
                {
                    step.annotations.push(StepAnnotation {
                        kind: "skip".into(),
                        description: reason.clone(),
                        location: Some(SourceLocation::caller(std::panic::Location::caller())),
                    });
                }
            }
            self.skip.cancel_with_reason(reason.clone());
        }
        Err(crate::E2eError::StepSkipped(reason))
    }
    #[track_caller]
    pub fn annotate(&self, kind: impl Into<String>, description: impl Into<String>) {
        let annotation = StepAnnotation {
            kind: kind.into(),
            description: description.into(),
            location: Some(SourceLocation::caller(std::panic::Location::caller())),
        };
        let mut records = self
            .session
            .records
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !records.sealed {
            if let Some((step, _, false)) = records.nodes.iter_mut().find(|n| n.0.id == self.id) {
                step.annotations.push(annotation);
            }
        }
    }
    pub fn annotations(&self) -> Vec<StepAnnotation> {
        self.session
            .records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .nodes
            .iter()
            .find(|n| n.0.id == self.id)
            .map(|n| n.0.annotations.clone())
            .unwrap_or_default()
    }
    pub fn title_path(&self) -> Vec<String> {
        self.session
            .records
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .nodes
            .iter()
            .find(|n| n.0.id == self.id)
            .map(|n| n.0.title_path.clone())
            .unwrap_or_default()
    }
}

/// One user, action, assertion or lifecycle step with children and attachments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInfo {
    #[serde(default)]
    pub category: StepCategory,
    #[serde(default)]
    pub status: StepStatus,
    #[serde(default)]
    pub annotations: Vec<StepAnnotation>,
    #[serde(default)]
    pub title_path: Vec<String>,
    pub id: u64,
    pub parent_id: Option<u64>,
    pub title: String,
    pub location: SourceLocation,
    /// Unix epoch milliseconds.
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub interrupted: bool,
    pub error: Option<TestError>,
    pub steps: Vec<StepInfo>,
    pub attachments: Vec<Attachment>,
}

impl StepInfo {
    pub fn title_path(&self) -> Vec<String> {
        self.title_path.clone()
    }
}

/// Complete diagnostics and artifacts for a single attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptResult {
    /// Console output and JavaScript errors across all pages of this attempt.
    #[serde(default)]
    pub console: Vec<crate::ConsoleMessage>,
    /// Popup startup observations, including failed or immediately closed adoption.
    #[serde(default)]
    pub popup_diagnostics: crate::PopupDiagnosticsHistory,
    /// Attempt-owned soft mismatches, isolated from other retries and tests.
    #[serde(default)]
    pub soft_assertions: Vec<SoftAssertionFailure>,
    /// Effective attempt settings, including its final runtime timeout.
    #[serde(default)]
    pub settings: Option<crate::ResolvedTestSettings>,
    pub info: AttemptInfo,
    pub status: AttemptStatus,
    pub expected_status: AttemptStatus,
    /// Includes cleanup and expectation checks, not just the body outcome.
    pub is_expected: bool,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub errors: Vec<TestError>,
    pub annotations: Vec<(String, String)>,
    pub steps: Vec<StepInfo>,
    pub attachments: Vec<Attachment>,
    pub screenshots: Vec<String>,
    pub trace: Option<String>,
    pub video: Option<String>,
}

/// Live runner callbacks, in lifecycle order within each attempt.
/// Callbacks are synchronous and may run concurrently on different workers.
/// Keep them short (enqueue slow uploads yourself). Callback panics are contained.
/// Aggregate file reporters remain available alongside these callbacks.
pub trait Reporter: Send + Sync + 'static {
    /// Discovered tests, before filtering or project expansion.
    fn on_begin(&self, _tests: &[crate::Test]) {}
    /// Resolved run configuration after project browser startup, before workers.
    /// When startup aborts early, emits its planned snapshot before cleanup instead.
    /// Dedicated versions remain None for browsers that were never launched.
    fn on_configuration(&self, _config: &crate::ResolvedRunConfig) {}
    /// Initial attempt settings, emitted immediately before on_test_begin.
    fn on_test_configuration(
        &self,
        _attempt: &AttemptInfo,
        _settings: &crate::ResolvedTestSettings,
    ) {
    }
    fn on_test_begin(&self, _attempt: &AttemptInfo) {}
    /// The result covers this attempt only; `attempts` is its one-based ordinal.
    fn on_test_end(&self, _attempt: &AttemptInfo, _result: &TestResult) {}
    fn on_step_begin(&self, _attempt: &AttemptInfo, _step: &StepInfo) {}
    fn on_step_end(&self, _attempt: &AttemptInfo, _step: &StepInfo) {}
    fn on_attachment(&self, _attempt: &AttemptInfo, _attachment: &Attachment) {}
    /// `None` denotes a run/worker error rather than an attempt error.
    fn on_error(&self, _attempt: Option<&AttemptInfo>, _error: &str) {}
    fn on_end(&self, _report: &TestReport) {}
}

#[derive(Clone, Default)]
pub(crate) struct ReporterHub(pub(crate) Vec<Arc<dyn Reporter>>);
impl ReporterHub {
    pub(crate) fn emit(&self, callback: impl Fn(&dyn Reporter)) {
        for reporter in &self.0 {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback(reporter.as_ref());
            }));
        }
    }
}

tokio::task_local! {
    static CURRENT_STEP: (u64, u64);
    static CURRENT_SESSION: StepSession;
    static SUPPRESS_ACTIONS: bool;
}
#[derive(Default)]
struct StepRecords {
    nodes: Vec<(StepInfo, std::time::Instant, bool)>,
    sealed: bool,
}
pub(crate) fn current_session() -> Option<StepSession> {
    CURRENT_SESSION.try_with(Clone::clone).ok()
}
/// Record one public operation, suppressing implementation calls/poll attempts.
/// Hook/fixture scopes intentionally keep their nested actions visible.
pub(crate) fn automatic<'a, T: 'a>(
    session: Option<StepSession>,
    title: impl Into<String>,
    category: StepCategory,
    future: impl std::future::Future<Output = crate::E2eResult<T>> + 'a,
) -> impl std::future::Future<Output = crate::E2eResult<T>> + 'a {
    let future = Box::pin(future);
    let title = title.into();
    async move {
        if SUPPRESS_ACTIONS.try_with(|value| *value).unwrap_or(false) {
            return future.await;
        }
        match session.or_else(current_session) {
            None => future.await,
            Some(session) => {
                let location = session.default_location();
                session
                    .run_kind(&title, location, category, Vec::new(), future, |out| {
                        out.as_ref()
                            .err()
                            .map(|e| TestError::new(e, &format!("{category:?}"), None))
                    })
                    .await
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct StepSession {
    id: u64,
    hub: ReporterHub,
    attempt: AttemptInfo,
    records: Arc<Mutex<StepRecords>>,
}
impl StepSession {
    pub(crate) fn new(hub: ReporterHub, attempt: AttemptInfo) -> Self {
        Self {
            id: next_id(),
            hub,
            attempt,
            records: Arc::default(),
        }
    }
    pub(crate) async fn run<F: std::future::Future>(
        &self,
        title: &str,
        location: SourceLocation,
        future: F,
        error: impl FnOnce(&F::Output) -> Option<TestError>,
    ) -> F::Output {
        self.run_kind(
            title,
            location,
            StepCategory::User,
            Vec::new(),
            future,
            error,
        )
        .await
    }
    pub(crate) async fn scope<F: std::future::Future>(&self, future: F) -> F::Output {
        CURRENT_SESSION.scope(self.clone(), future).await
    }
    pub(crate) fn context(&self) -> (Option<u64>, Vec<String>) {
        let id = CURRENT_STEP
            .try_with(|(session, id)| (*session == self.id).then_some(*id))
            .ok()
            .flatten();
        let records = self.records.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((step, _, _)) = records
            .nodes
            .iter()
            .find(|(step, _, _)| Some(step.id) == id)
        {
            return (id, step.title_path.clone());
        }
        (
            None,
            vec![self.attempt.file.clone(), self.attempt.name.clone()],
        )
    }
    pub(crate) async fn assertion<F: std::future::Future<Output = crate::E2eResult<()>>>(
        &self,
        title: &str,
        location: SourceLocation,
        future: F,
    ) -> crate::E2eResult<()> {
        self.run_kind(
            title,
            location.clone(),
            StepCategory::Assertion,
            Vec::new(),
            future,
            |result| {
                result
                    .as_ref()
                    .err()
                    .map(|error| TestError::new(error, "soft assertion", Some(location)))
            },
        )
        .await
    }
    fn default_location(&self) -> SourceLocation {
        SourceLocation {
            file: self.attempt.file.clone(),
            line: self.attempt.line,
            column: 0,
        }
    }
    async fn run_kind<F: std::future::Future>(
        &self,
        title: &str,
        location: SourceLocation,
        category: StepCategory,
        annotations: Vec<StepAnnotation>,
        future: F,
        error: impl FnOnce(&F::Output) -> Option<TestError>,
    ) -> F::Output {
        use futures::FutureExt;
        let mut guard = self.start_kind(title, location, category, annotations);
        let result = CURRENT_STEP
            .scope(
                (self.id, guard.id),
                self.scope(SUPPRESS_ACTIONS.scope(
                    matches!(category, StepCategory::Action | StepCategory::Assertion),
                    std::panic::AssertUnwindSafe(future).catch_unwind(),
                )),
            )
            .await;
        match result {
            Ok(value) => {
                guard.finish(error(&value), false);
                value
            }
            Err(panic) => {
                let message = panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic");
                guard.finish(
                    Some(TestError {
                        message: message.into(),
                        code: "panic".into(),
                        phase: "step".into(),
                        location: None,
                    }),
                    false,
                );
                std::panic::resume_unwind(panic)
            }
        }
    }
    pub(crate) async fn controlled<T, F, Fut>(
        &self,
        title: &str,
        location: SourceLocation,
        options: StepOptions,
        body: F,
    ) -> crate::E2eResult<StepOutcome<T>>
    where
        F: FnOnce(StepContext) -> Fut,
        Fut: std::future::Future<Output = crate::E2eResult<T>>,
    {
        use futures::FutureExt;
        let mut guard = self.start_kind(title, location, StepCategory::User, options.annotations);
        let context = StepContext {
            session: self.clone(),
            id: guard.id,
            skip: crate::CancellationToken::new(),
        };
        if let Some(reason) = options.skip {
            let _ = context.skip(reason.clone());
            guard.finished = true;
            self.finish_status(guard.id, None, StepStatus::Skipped);
            return Ok(StepOutcome::Skipped(reason));
        }
        let skip = context.skip.clone();
        let work = async {
            tokio::select! { biased;
                reason=skip.cancelled()=>Ok(StepOutcome::Skipped(reason)),
                result=crate::operation::Deadline::new(options.timeout).run(format!("step {title}"), async {
                    body(context).await
                }) => {
                    if let Some(reason) = skip.reason() { Ok(StepOutcome::Skipped(reason)) }
                    else { result.map(StepOutcome::Completed) }
                },
            }
        };
        let result = CURRENT_STEP
            .scope(
                (self.id, guard.id),
                self.scope(
                    SUPPRESS_ACTIONS
                        .scope(false, std::panic::AssertUnwindSafe(work).catch_unwind()),
                ),
            )
            .await;
        match result {
            Ok(result) => {
                if matches!(&result, Ok(StepOutcome::Skipped(_))) {
                    guard.finished = true;
                    self.finish_status(guard.id, None, StepStatus::Skipped);
                } else {
                    guard.finish(
                        result
                            .as_ref()
                            .err()
                            .map(|e| TestError::new(e, "step", None)),
                        false,
                    );
                }
                result
            }
            Err(panic) => {
                guard.finish(Some(panic_error(&panic)), false);
                std::panic::resume_unwind(panic)
            }
        }
    }
    #[cfg(test)]
    fn start(&self, title: &str, location: SourceLocation) -> StepGuard {
        self.start_kind(title, location, StepCategory::User, Vec::new())
    }
    fn start_kind(
        &self,
        title: &str,
        location: SourceLocation,
        category: StepCategory,
        annotations: Vec<StepAnnotation>,
    ) -> StepGuard {
        let parent_id = CURRENT_STEP
            .try_with(|(session, step)| (*session == self.id).then_some(*step))
            .ok()
            .flatten();
        let mut title_path = vec![self.attempt.file.clone(), self.attempt.name.clone()];
        if let Some(parent) = parent_id {
            if let Some((step, _, _)) = self
                .records
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .nodes
                .iter()
                .find(|n| n.0.id == parent)
            {
                title_path = step.title_path.clone();
            }
        }
        title_path.push(title.into());
        let step = StepInfo {
            category,
            status: StepStatus::Running,
            annotations,
            title_path,
            id: next_id(),
            parent_id,
            title: title.into(),
            location,
            start_time_ms: crate::driver::now_ms(),
            duration_ms: 0,
            interrupted: false,
            error: None,
            steps: Vec::new(),
            attachments: Vec::new(),
        };
        let id = step.id;
        let active = {
            let mut records = self.records.lock().unwrap_or_else(|e| e.into_inner());
            if records.sealed {
                false
            } else {
                records
                    .nodes
                    .push((step.clone(), std::time::Instant::now(), false));
                true
            }
        };
        if active {
            self.hub.emit(|r| r.on_step_begin(&self.attempt, &step));
        }
        StepGuard {
            session: self.clone(),
            id,
            finished: !active,
        }
    }
    fn finish(&self, id: u64, error: Option<TestError>, interrupted: bool) {
        let status = if interrupted {
            StepStatus::Interrupted
        } else if error
            .as_ref()
            .is_some_and(|e| e.code == "FERRITE_E2E_TIMEOUT")
        {
            StepStatus::TimedOut
        } else if error.is_some() {
            StepStatus::Failed
        } else {
            StepStatus::Passed
        };
        self.finish_status(id, error, status);
    }
    fn finish_status(&self, id: u64, error: Option<TestError>, status: StepStatus) {
        let step = {
            let mut records = self.records.lock().unwrap_or_else(|e| e.into_inner());
            let Some((step, started, finished)) = records.nodes.iter_mut().find(|n| n.0.id == id)
            else {
                return;
            };
            if *finished {
                return;
            }
            *finished = true;
            step.duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            step.interrupted = status == StepStatus::Interrupted;
            step.status = status;
            step.error = error.map(|mut error| {
                if error.location.is_none() {
                    error.location = Some(step.location.clone());
                }
                error
            });
            tree(&records, id)
        };
        self.hub.emit(|r| r.on_step_end(&self.attempt, &step));
    }
    pub(crate) fn attach(&self, attachment: &Attachment) {
        if let Ok((session, id)) = CURRENT_STEP.try_with(|id| *id) {
            if session == self.id {
                let mut records = self.records.lock().unwrap_or_else(|e| e.into_inner());
                if !records.sealed {
                    if let Some((step, _, _)) = records.nodes.iter_mut().find(|n| n.0.id == id) {
                        step.attachments.push(attachment.clone());
                    }
                }
            }
        }
    }
    pub(crate) fn finish_all(&self) -> Vec<StepInfo> {
        let pending = {
            let mut records = self.records.lock().unwrap_or_else(|e| e.into_inner());
            records.sealed = true;
            records
                .nodes
                .iter()
                .rev()
                .filter(|n| !n.2)
                .map(|n| n.0.id)
                .collect::<Vec<_>>()
        };
        for id in pending {
            self.finish(id, None, true);
        }
        let records = self.records.lock().unwrap_or_else(|e| e.into_inner());
        records
            .nodes
            .iter()
            .filter(|n| n.0.parent_id.is_none())
            .map(|n| tree(&records, n.0.id))
            .collect()
    }
}
fn panic_error(panic: &Box<dyn std::any::Any + Send>) -> TestError {
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("non-string panic");
    TestError {
        message: message.into(),
        code: "panic".into(),
        phase: "step".into(),
        location: None,
    }
}

fn tree(records: &StepRecords, id: u64) -> StepInfo {
    let mut step = records
        .nodes
        .iter()
        .find(|n| n.0.id == id)
        .unwrap()
        .0
        .clone();
    step.steps = records
        .nodes
        .iter()
        .filter(|n| n.0.parent_id == Some(id))
        .map(|n| tree(records, n.0.id))
        .collect();
    step
}
fn next_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
struct StepGuard {
    session: StepSession,
    id: u64,
    finished: bool,
}
impl StepGuard {
    fn finish(&mut self, error: Option<TestError>, interrupted: bool) {
        self.finished = true;
        self.session.finish(self.id, error, interrupted);
    }
}
impl Drop for StepGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.session.finish(self.id, None, true);
        }
    }
}

/// Outcome of one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    /// Passed (possibly after retries).
    Passed,
    /// Failed all attempts.
    Failed,
    /// Skipped by the filter.
    Skipped,
    /// Failed as expected (`Test::fail`); does not fail the run.
    #[serde(rename = "expected")]
    FailedExpected,
}

/// One file attached to a test via [`TestInfo`](crate::runner::TestInfo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Attachment name.
    pub name: String,
    /// File path (under the output dir).
    pub path: String,
    /// MIME type.
    pub content_type: String,
}

/// Result of one test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Full history, including attempts that failed before a successful retry.
    #[serde(default)]
    pub attempt_results: Vec<AttemptResult>,
    /// Passed after at least one unexpected unsuccessful attempt.
    #[serde(default)]
    pub flaky: bool,
    /// Test name.
    pub name: String,
    /// Outcome.
    pub status: TestStatus,
    /// Attempts used (1 + retries).
    pub attempts: u32,
    /// Total wall time in milliseconds.
    pub duration_ms: u64,
    /// Failure message, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Screenshot artifact paths.
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// Trace artifact path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// Video artifact path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
    /// Project name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// `repeat_each` index (0 = single run).
    #[serde(default)]
    pub repeat_each_index: u32,
    /// Annotations as (kind, description) pairs.
    #[serde(default)]
    pub annotations: Vec<(String, String)>,
    /// Attached files.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
}

/// Aggregate report for a run.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TestReport {
    /// Effective configuration used by this run; absent in historical reports.
    #[serde(default)]
    pub configuration: Option<crate::ResolvedRunConfig>,
    /// Global hooks and shared worker lifecycle that are outside an attempt.
    #[serde(default)]
    pub run_steps: Vec<StepInfo>,
    /// Per-test results in completion order.
    pub results: Vec<TestResult>,
}

// Derived run policy is serialized alongside the unchanged per-test outcomes.
// Historical reports omit configuration and retain their original success rule.
impl Serialize for TestReport {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut report = serializer.serialize_struct("TestReport", 6)?;
        report.serialize_field("configuration", &self.configuration)?;
        report.serialize_field("run_steps", &self.run_steps)?;
        report.serialize_field("results", &self.results)?;
        report.serialize_field("status", if self.ok() { "passed" } else { "failed" })?;
        report.serialize_field("exit_code", &self.exit_code())?;
        report.serialize_field("flaky_policy_failed", &self.flaky_policy_failed())?;
        report.end()
    }
}

impl TestReport {
    /// Number of passed tests.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count()
    }

    /// Number of tests recovered by retries.
    #[must_use]
    pub fn flaky(&self) -> usize {
        self.results.iter().filter(|r| r.flaky).count()
    }

    /// Number of failed tests.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count()
    }

    /// Whether the configured flaky-test policy rejects this run.
    #[must_use]
    pub fn flaky_policy_failed(&self) -> bool {
        self.configuration
            .as_ref()
            .is_some_and(|config| config.fail_on_flaky_tests)
            && self.flaky() > 0
    }

    /// True when no test failed and the aggregate flaky policy is satisfied.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.failed() == 0 && !self.flaky_policy_failed()
    }

    /// Process exit code (0 = all passed).
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.ok() {
            0
        } else {
            1
        }
    }

    /// Number of expected failures.
    #[must_use]
    pub fn expected_failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::FailedExpected)
            .count()
    }

    /// One-line summary.
    #[must_use]
    pub fn summary(&self) -> String {
        let skipped = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count();
        let expected = self.expected_failed();
        let mut out = format!(
            "{} passed, {} failed, {} skipped",
            self.passed(),
            self.failed(),
            skipped,
        );
        if expected > 0 {
            out.push_str(&format!(", {expected} expected-failed"));
        }
        if self.flaky() > 0 {
            out.push_str(&format!(", {} flaky", self.flaky()));
        }
        out.push_str(&format!(" ({} total)", self.results.len()));
        if self.flaky_policy_failed() {
            out.push_str("; run failed: fail_on_flaky_tests");
        }
        out
    }

    /// Render the `dot` reporter output (one char per test + summary).
    ///
    /// `.` passed, `F` failed, `s` skipped, `E` failed-as-expected.
    #[must_use]
    pub fn to_dot(&self) -> String {
        let mut out = String::new();
        for result in &self.results {
            out.push(match result.status {
                TestStatus::Passed => '.',
                TestStatus::Failed => 'F',
                TestStatus::Skipped => 's',
                TestStatus::FailedExpected => 'E',
            });
        }
        out.push('\n');
        out.push_str(&self.summary());
        out.push('\n');
        out
    }

    /// Render the `list` reporter output.
    #[must_use]
    pub fn to_list(&self) -> String {
        let mut out = String::new();
        for result in &self.results {
            let mark = match result.status {
                TestStatus::Passed => "ok",
                TestStatus::Failed => "FAIL",
                TestStatus::Skipped => "skip",
                TestStatus::FailedExpected => "expected",
            };
            out.push_str(&format!(
                "[{mark}] {} ({}ms, {} attempt{})\n",
                result.name,
                result.duration_ms,
                result.attempts,
                if result.attempts == 1 { "" } else { "s" },
            ));
            if let Some(error) = &result.error {
                for line in error.lines() {
                    out.push_str(&format!("       {line}\n"));
                }
            }
            for shot in &result.screenshots {
                out.push_str(&format!("       screenshot: {shot}\n"));
            }
            if let Some(trace) = &result.trace {
                out.push_str(&format!("       trace: {trace}\n"));
            }
            if let Some(video) = &result.video {
                out.push_str(&format!("       video: {video}\n"));
            }
            if let Some(project) = &result.project {
                out.push_str(&format!("       project: {project}\n"));
            }
            for (kind, description) in &result.annotations {
                out.push_str(&format!("       annotation: {kind}={description}\n"));
            }
            for attachment in &result.attachments {
                out.push_str(&format!("       attachment: {}\n", attachment.path));
            }
        }
        out.push_str(&self.summary());
        out.push('\n');
        out
    }

    /// Render the `json` reporter output.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render the `junit` reporter output. With fail_on_flaky_tests enabled,
    /// recovered cases carry a FlakyTestPolicy failure for CI consumers; their
    /// original final status/flakiness remain in properties and Rust/JSON results.
    #[must_use]
    pub fn to_junit(&self) -> String {
        let flaky_policy_failed = self.flaky_policy_failed();
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str(&format!(
            "<testsuite name=\"ferrite-e2e\" tests=\"{}\" failures=\"{}\">\n",
            self.results.len(),
            self.results
                .iter()
                .filter(|result| result.status == TestStatus::Failed
                    || (flaky_policy_failed && result.flaky))
                .count()
        ));
        out.push_str(&format!(
            "  <properties><property name=\"ferrite.run.status\" value=\"{}\"/>\
             <property name=\"ferrite.fail_on_flaky_tests\" value=\"{}\"/></properties>\n",
            if self.ok() { "passed" } else { "failed" },
            self.configuration
                .as_ref()
                .is_some_and(|config| config.fail_on_flaky_tests)
        ));
        for result in &self.results {
            out.push_str(&format!(
                "  <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">\n",
                xml_escape(&result.name),
                xml_escape(result.project.as_deref().unwrap_or("ferrite-e2e")),
                result.duration_ms as f64 / 1000.0
            ));
            if result.status == TestStatus::Skipped {
                out.push_str("    <skipped/>\n");
            }
            // Expected failures keep their message but never fail the suite.
            if result.status == TestStatus::Failed {
                out.push_str(&format!(
                    "    <failure message=\"{}\"/>\n",
                    xml_escape(&one_line(result.error.as_deref().unwrap_or("test failed")))
                ));
            } else if flaky_policy_failed && result.flaky {
                out.push_str("    <failure type=\"FlakyTestPolicy\" message=\"fail_on_flaky_tests: test recovered after an unexpected attempt\"/>\n");
            }
            out.push_str(&format!(
                "    <properties><property name=\"ferrite.final_status\" value=\"{}\"/>\
                 <property name=\"ferrite.flaky\" value=\"{}\"/>\
                 <property name=\"ferrite.attempts\" value=\"{}\"/>",
                match result.status {
                    TestStatus::Passed => "passed",
                    TestStatus::Failed => "failed",
                    TestStatus::Skipped => "skipped",
                    TestStatus::FailedExpected => "failedexpected",
                },
                result.flaky,
                result.attempts
            ));
            if let Some(video) = &result.video {
                out.push_str(&format!(
                    "<property name=\"video\" value=\"{}\"/>",
                    xml_escape(video)
                ));
            }
            out.push_str("</properties>\n");
            out.push_str("  </testcase>\n");
        }
        out.push_str("</testsuite>\n");
        out
    }

    /// Render HTML with inline styles and links to the recorded artifact paths.
    /// Use [`TestReport::write_bundle`] to export portable artifact links.
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut out = String::from(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <title>ferrite e2e report</title><style>\
             body{font-family:sans-serif;margin:2em;color:#222}\
             table{border-collapse:collapse;width:100%}\
             th,td{border:1px solid #ccc;padding:.4em .6em;text-align:left;vertical-align:top}\
             th{background:#f0f0f0}pre{background:#f6f6f6;padding:.4em;white-space:pre-wrap}\
             .pill{display:inline-block;padding:.1em .6em;border-radius:1em;color:#fff;font-size:.85em}\
             .pass{background:#2a7}.fail{background:#c33}.skip{background:#888}\
             .exp{background:#b96}\
             </style></head><body>",
        );
        out.push_str(&format!(
            "<h1>ferrite e2e</h1><p><span class=\"pill {}\">run {}</span> {}</p>",
            if self.ok() { "pass" } else { "fail" },
            if self.ok() { "passed" } else { "failed" },
            xml_escape(&self.summary())
        ));
        out.push_str(
            "<table><thead><tr><th>status</th><th>test</th><th>time</th>\
             <th>attempts</th><th>details</th></tr></thead><tbody>",
        );
        for result in &self.results {
            let (label, class) = match result.status {
                TestStatus::Passed if result.flaky => ("flaky", "exp"),
                TestStatus::Passed => ("passed", "pass"),
                TestStatus::Failed => ("failed", "fail"),
                TestStatus::Skipped => ("skipped", "skip"),
                TestStatus::FailedExpected => ("expected-failed", "exp"),
            };
            out.push_str(&format!(
                "<tr><td><span class=\"pill {class}\">{label}</span></td><td>{}</td>\
                 <td>{}ms</td><td>{}</td><td>",
                xml_escape(&result.name),
                result.duration_ms,
                result.attempts
            ));
            if let Some(project) = &result.project {
                out.push_str(&format!("<div>project: {}</div>", xml_escape(project)));
            }
            for (kind, description) in &result.annotations {
                out.push_str(&format!(
                    "<div>{}={}</div>",
                    xml_escape(kind),
                    xml_escape(description)
                ));
            }
            if let Some(error) = &result.error {
                out.push_str(&format!("<pre>{}</pre>", xml_escape(error)));
            }
            for shot in &result.screenshots {
                let href = xml_escape(shot);
                out.push_str(&format!("<a href=\"{href}\">screenshot</a> "));
            }
            if let Some(trace) = &result.trace {
                let href = xml_escape(trace);
                out.push_str(&format!("<a href=\"{href}\">trace</a> "));
            }
            if let Some(video) = &result.video {
                let href = xml_escape(video);
                out.push_str(&format!("<a href=\"{href}\">video</a> "));
            }
            for attachment in &result.attachments {
                let href = xml_escape(&attachment.path);
                out.push_str(&format!(
                    "<a href=\"{href}\">{}</a> ",
                    xml_escape(&attachment.name)
                ));
            }
            for attempt in &result.attempt_results {
                out.push_str(&format!(
                    "<details><summary>Attempt {}: {:?} (expected {:?}), {}ms</summary>",
                    attempt.info.retry + 1,
                    attempt.status,
                    attempt.expected_status,
                    attempt.duration_ms
                ));
                out.push_str(&format!(
                    "<div>Started: {} · worker {} · repeat {}</div>",
                    crate::har::iso8601(attempt.start_time_ms),
                    attempt.info.worker_index,
                    attempt.info.repeat_each_index
                ));
                if let Some(settings) = &attempt.settings {
                    out.push_str("<details><summary>Effective settings</summary><pre>");
                    out.push_str(&xml_escape(
                        &serde_json::to_string_pretty(settings).unwrap_or_default(),
                    ));
                    out.push_str("</pre></details>");
                }
                for error in &attempt.errors {
                    render_error(&mut out, error);
                }
                for (kind, value) in &attempt.annotations {
                    out.push_str(&format!(
                        "<div>{}: {}</div>",
                        xml_escape(kind),
                        xml_escape(value)
                    ));
                }
                if !attempt.soft_assertions.is_empty() {
                    out.push_str("<details><summary>Soft assertions</summary><ul>");
                    for failure in &attempt.soft_assertions {
                        out.push_str("<li>");
                        out.push_str(&format!(
                            "<div>{}</div>",
                            xml_escape(&failure.title_path.join(" > "))
                        ));
                        render_error(&mut out, &failure.error);
                        out.push_str("</li>");
                    }
                    out.push_str("</ul></details>");
                }
                render_steps(&mut out, &attempt.steps);
                render_console(&mut out, &attempt.console);
                render_popup_diagnostics(&mut out, &attempt.popup_diagnostics);
                for path in &attempt.screenshots {
                    render_link(&mut out, path, "screenshot");
                }
                if let Some(path) = &attempt.trace {
                    render_link(&mut out, path, "trace");
                }
                if let Some(path) = &attempt.video {
                    render_link(&mut out, path, "video");
                }
                for attachment in &attempt.attachments {
                    render_link(&mut out, &attachment.path, &attachment.name);
                }
                out.push_str("</details>");
            }
            out.push_str("</td></tr>");
        }
        out.push_str("</tbody></table>");
        if let Some(config) = &self.configuration {
            out.push_str("<details><summary>Effective configuration</summary><pre>");
            out.push_str(&xml_escape(
                &serde_json::to_string_pretty(config).unwrap_or_default(),
            ));
            out.push_str("</pre></details>");
        }

        if !self.run_steps.is_empty() {
            out.push_str("<h2>Run lifecycle</h2>");
            render_steps(&mut out, &self.run_steps);
        }
        out.push_str("</body></html>");
        out
    }
}

fn render_link(out: &mut String, path: &str, name: &str) {
    out.push_str(&format!(
        "<a href=\"{}\">{}</a> ",
        xml_escape(path),
        xml_escape(name)
    ));
}
fn render_error(out: &mut String, error: &TestError) {
    out.push_str(&format!(
        "<pre>{}: {} [{}]</pre>",
        xml_escape(&error.phase),
        xml_escape(&error.message),
        xml_escape(&error.code)
    ));
    if let Some(location) = &error.location {
        out.push_str(&format!(
            "<div>{}:{}:{}</div>",
            xml_escape(&location.file),
            location.line,
            location.column
        ));
    }
}
fn render_popup_diagnostics(out: &mut String, history: &crate::PopupDiagnosticsHistory) {
    if history.entries.is_empty() && history.dropped_popups == 0 {
        return;
    }
    out.push_str(&format!(
        "<details><summary>Popup startup diagnostics: {} ({} evicted)</summary>",
        history.entries.len(),
        history.dropped_popups
    ));
    for popup in &history.entries {
        out.push_str(&format!("<details><summary>{}: {:?}</summary><div>Opener: {} · native closed: {} · document replaced: {} · truncated: {} · dropped events: {}</div>", xml_escape(&popup.page_id), popup.adoption, xml_escape(&popup.opener_id), popup.closed, popup.initial_document_replaced, popup.truncated, popup.dropped_events));
        if let Some(error) = &popup.error {
            out.push_str(&format!("<pre>{}</pre>", xml_escape(error)));
        }
        for request in &popup.requests {
            out.push_str(&format!(
                "<div>{} {} · {} · {}</div>",
                xml_escape(&request.recorded.method),
                xml_escape(&request.recorded.url),
                request.recorded.status,
                xml_escape(&format!("{:?}", request.completion))
            ));
        }
        out.push_str("</details>");
    }
    out.push_str("</details>");
}
fn render_console(out: &mut String, messages: &[crate::ConsoleMessage]) {
    if messages.is_empty() {
        return;
    }
    out.push_str("<details><summary>Console and page errors</summary><ul>");
    for message in messages {
        out.push_str(&format!(
            "<li><strong>{}</strong><pre>{}</pre>",
            xml_escape(&message.kind),
            xml_escape(&message.text)
        ));
        if let Some(location) = &message.location {
            out.push_str(&format!(
                "<div>{}:{}:{}</div>",
                xml_escape(&location.url),
                location.line,
                location.column
            ));
        }
        if let Some(timestamp) = message.timestamp_ms {
            out.push_str(&format!("<div>{}</div>", crate::har::iso8601(timestamp)));
        }
        if let Some(page) = &message.page_id {
            out.push_str(&format!("<div>page: {}</div>", xml_escape(page)));
        }
        if let Some(error) = &message.error {
            out.push_str("<details><summary>Structured page error</summary>");
            for (label, value) in [
                ("Name", &error.name),
                ("Message", &error.message),
                ("Constructor", &error.class_name),
            ] {
                if let Some(value) = value {
                    out.push_str(&format!("<div>{label}: {}</div>", xml_escape(value)));
                }
            }
            if let Some(stack) = &error.stack {
                out.push_str(&format!("<pre>{}</pre>", xml_escape(stack)));
            }
            for frame in &error.frames {
                out.push_str(&format!(
                    "<div>{}{} · {}:{}:{}{}</div>",
                    if frame.async_stack { "async " } else { "" },
                    xml_escape(frame.function_name.as_deref().unwrap_or("unknown function")),
                    xml_escape(frame.url.as_deref().unwrap_or("unknown source")),
                    frame
                        .line
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".into()),
                    frame
                        .column
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "?".into()),
                    frame
                        .async_description
                        .as_ref()
                        .map(|v| format!(" · {}", xml_escape(v)))
                        .unwrap_or_default()
                ));
            }
            if let Some(thrown) = &error.thrown {
                out.push_str(&format!(
                    "<pre>Thrown value: {}</pre>",
                    xml_escape(&serde_json::to_string(thrown).unwrap_or_default())
                ));
            }
            if error.truncated {
                out.push_str("<div>Error metadata truncated</div>");
            }
            out.push_str("</details>");
        }
        if let Some(arguments) = &message.arguments {
            out.push_str(&format!(
                "<details><summary>Console arguments: {} ({} omitted{})</summary><ol>",
                arguments.values.len(),
                arguments.dropped_arguments,
                if arguments.truncated {
                    "; truncated"
                } else {
                    ""
                }
            ));
            for argument in &arguments.values {
                out.push_str(&format!(
                    "<li><pre>{}</pre></li>",
                    xml_escape(&serde_json::to_string_pretty(argument).unwrap_or_default())
                ));
            }
            out.push_str("</ol></details>");
        }
        out.push_str("</li>");
    }
    out.push_str("</ul></details>");
}

fn render_steps(out: &mut String, steps: &[StepInfo]) {
    if steps.is_empty() {
        return;
    }
    out.push_str("<ul>");
    for step in steps {
        out.push_str(&format!(
            "<li><details open><summary>{} · {:?} · {:?} ({}ms{})</summary><div>{}:{}:{} · {}</div>",
            xml_escape(&step.title),
            step.category,
            step.status,
            step.duration_ms,
            if step.interrupted {
                ", interrupted"
            } else {
                ""
            },
            xml_escape(&step.location.file),
            step.location.line,
            step.location.column,
            crate::har::iso8601(step.start_time_ms)
        ));
        for annotation in &step.annotations {
            out.push_str(&format!(
                "<div>{}: {}</div>",
                xml_escape(&annotation.kind),
                xml_escape(&annotation.description)
            ));
        }
        if let Some(error) = &step.error {
            render_error(out, error);
        }
        for attachment in &step.attachments {
            render_link(out, &attachment.path, &attachment.name);
        }
        render_steps(out, &step.steps);
        out.push_str("</details></li>");
    }
    out.push_str("</ul>");
}

fn one_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> StepSession {
        StepSession::new(
            ReporterHub::default(),
            AttemptInfo {
                name: "steps".into(),
                file: "test.rs".into(),
                line: 1,
                project: None,
                worker_index: 0,
                repeat_each_index: 0,
                retry: 0,
            },
        )
    }
    fn source() -> SourceLocation {
        SourceLocation {
            file: "test.rs".into(),
            line: 12,
            column: 3,
        }
    }

    #[tokio::test]
    async fn concurrent_nested_steps_keep_parentage_and_handled_errors() {
        let session = session();
        session
            .run(
                "outer",
                source(),
                async {
                    let (_, handled) = tokio::join!(
                        session.run(
                            "left",
                            source(),
                            async {
                                tokio::task::yield_now().await;
                                session.attach(&Attachment {
                                    name: "left data".into(),
                                    path: "data.txt".into(),
                                    content_type: "text/plain".into(),
                                });
                                session
                                    .run("grandchild", source(), async { 7 }, |_| None)
                                    .await
                            },
                            |_| None
                        ),
                        session.run(
                            "right",
                            source(),
                            async { Err::<(), _>(crate::E2eError::Expect("handled".into())) },
                            |result| result
                                .as_ref()
                                .err()
                                .map(|e| TestError::new(e, "step", None))
                        )
                    );
                    assert!(handled.is_err());
                },
                |_| None,
            )
            .await;
        let roots = session.finish_all();
        assert_eq!(roots.len(), 1);
        let outer = &roots[0];
        assert!(!outer.interrupted);
        assert_eq!(outer.steps.len(), 2);
        let left = &outer.steps[0];
        let right = &outer.steps[1];
        assert_eq!(left.parent_id, Some(outer.id));
        assert_eq!(left.steps[0].parent_id, Some(left.id));
        assert_eq!(left.attachments.len(), 1);
        assert!(right.attachments.is_empty());
        assert_eq!(right.error.as_ref().unwrap().code, "FERRITE_E2E_EXPECT");
        assert_eq!(
            right
                .error
                .as_ref()
                .unwrap()
                .location
                .as_ref()
                .unwrap()
                .line,
            12
        );
        assert!(outer.start_time_ms > 0);
    }

    #[tokio::test]
    async fn cancelled_steps_finish_children_and_parent_once() {
        #[derive(Clone)]
        struct Events(Arc<Mutex<Vec<StepInfo>>>);
        impl Reporter for Events {
            fn on_step_end(&self, _: &AttemptInfo, step: &StepInfo) {
                self.0.lock().unwrap().push(step.clone());
            }
        }
        let events = Events(Arc::default());
        let mut session = session();
        session.hub.0.push(Arc::new(events.clone()));
        let future = session.run(
            "outer",
            source(),
            async {
                session
                    .run("child", source(), std::future::pending::<()>(), |_| None)
                    .await
            },
            |_| None,
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), future)
                .await
                .is_err()
        );
        let roots = session.finish_all();
        assert!(roots[0].interrupted);
        assert!(roots[0].steps[0].interrupted);
        let ended = events.0.lock().unwrap();
        assert_eq!(ended.len(), 2);
        assert_eq!(ended[0].title, "child");
        assert_eq!(ended[1].steps.len(), 1);
        drop(ended);
        session.finish_all();
        assert_eq!(events.0.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn panics_record_failures_and_resume_unwinding() {
        use futures::FutureExt;
        let session = session();
        let result = std::panic::AssertUnwindSafe(session.run(
            "panic",
            source(),
            async { panic!("boom") },
            |_: &()| None,
        ))
        .catch_unwind()
        .await;
        assert!(result.is_err());
        let steps = session.finish_all();
        assert!(!steps[0].interrupted);
        assert_eq!(steps[0].error.as_ref().unwrap().code, "panic");
        assert_eq!(steps[0].error.as_ref().unwrap().message, "boom");
    }

    #[tokio::test]
    async fn sealed_attempt_finishes_detached_steps_without_late_events() {
        let session = session();
        let guard = session.start("detached", source());
        assert!(session.finish_all()[0].interrupted);
        drop(guard);
        session.run("late", source(), async {}, |_| None).await;
        assert_eq!(session.finish_all().len(), 1);
    }

    #[test]
    fn legacy_json_without_attempt_history_still_loads() {
        let mut value = serde_json::to_value(sample_result("old", TestStatus::Passed)).unwrap();
        value.as_object_mut().unwrap().remove("attempt_results");
        value.as_object_mut().unwrap().remove("flaky");
        let result: TestResult = serde_json::from_value(value).unwrap();
        assert!(result.attempt_results.is_empty());
        assert!(!result.flaky);
    }

    #[tokio::test]
    async fn html_and_json_retain_attempt_diagnostics_and_escape_text() {
        let session = session();
        session
            .run("<script>step</script>", source(), async {}, |_| {
                Some(TestError {
                    message: "<bad>".into(),
                    phase: "step".into(),
                    code: "error".into(),
                    location: None,
                })
            })
            .await;
        let attempt = AttemptResult {
            console: Vec::new(),
            popup_diagnostics: Default::default(),
            soft_assertions: Vec::new(),
            settings: None,
            info: session.attempt.clone(),
            status: AttemptStatus::Failed,
            expected_status: AttemptStatus::Passed,
            is_expected: false,
            start_time_ms: 1,
            duration_ms: 20,
            errors: vec![TestError {
                message: "first failure".into(),
                code: "error".into(),
                phase: "body".into(),
                location: Some(source()),
            }],
            annotations: Vec::new(),
            steps: session.finish_all(),
            attachments: vec![Attachment {
                name: "<attachment>".into(),
                path: "a\".txt".into(),
                content_type: "text/plain".into(),
            }],
            screenshots: vec!["first.png".into()],
            trace: Some("first.json".into()),
            video: Some("first.webm".into()),
        };
        let mut second = attempt.clone();
        second.info.retry = 1;
        second.status = AttemptStatus::Passed;
        second.is_expected = true;
        let mut result = sample_result("recovered", TestStatus::Passed);
        result.flaky = true;
        result.attempts = 2;
        result.attempt_results = vec![attempt, second];
        let report = TestReport {
            configuration: None,
            run_steps: Vec::new(),
            results: vec![result],
        };
        assert_eq!(report.flaky(), 1);
        let html = report.to_html();
        for expected in [
            "Attempt 1",
            "Attempt 2",
            "first failure",
            "first.png",
            "first.webm",
            "test.rs:12:3",
            "flaky",
            "&lt;script&gt;",
            "a&quot;.txt",
        ] {
            assert!(html.contains(expected), "missing {expected}");
        }
        assert!(!html.contains("<script>"));
        let round_trip: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(round_trip.results[0].attempt_results.len(), 2);
        assert_eq!(round_trip.results[0].attempt_results[0].steps.len(), 1);
    }

    #[tokio::test]
    async fn controlled_skips_are_local_and_never_construct_preset_bodies() {
        let session = session();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let preset = session
            .controlled(
                "preset",
                source(),
                StepOptions::default().skip("unsupported"),
                |_| {
                    calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    async { Ok(3) }
                },
            )
            .await
            .unwrap();
        assert_eq!(preset, StepOutcome::Skipped("unsupported".into()));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        let session_ref = &session;
        let outer = session
            .controlled(
                "outer",
                source(),
                StepOptions::default(),
                |context| async move {
                    context.annotate("ticket", "123");
                    assert_eq!(context.title_path(), ["test.rs", "steps", "outer"]);
                    let nested = session_ref
                        .controlled(
                            "nested",
                            source(),
                            StepOptions::default(),
                            |step| async move {
                                assert_eq!(
                                    step.title_path(),
                                    ["test.rs", "steps", "outer", "nested"]
                                );
                                step.skip("later")?;
                                panic!("skip must stop the closure");
                                #[allow(unreachable_code)]
                                Ok::<(), crate::E2eError>(())
                            },
                        )
                        .await?;
                    assert_eq!(nested, StepOutcome::Skipped("later".into()));
                    Ok(42)
                },
            )
            .await
            .unwrap();
        assert_eq!(outer, StepOutcome::Completed(42));
        let steps = session.finish_all();
        assert_eq!(steps[0].status, StepStatus::Skipped);
        assert_eq!(steps[1].status, StepStatus::Passed);
        assert_eq!(steps[1].annotations[0].kind, "ticket");
        assert_eq!(steps[1].steps[0].status, StepStatus::Skipped);
        assert!(steps[1].steps[0].error.is_none());
    }
    #[tokio::test]
    async fn shared_step_skip_cancels_children_and_annotations_stop_at_completion() {
        let session = session();
        let session_ref = &session;
        let saved = Arc::new(Mutex::new(None));
        let saved_context = saved.clone();
        let result = session
            .controlled(
                "parent",
                source(),
                StepOptions::default(),
                |context| async move {
                    *saved_context.lock().unwrap() = Some(context.clone());
                    let abort = context.clone();
                    let task = tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        let _ = abort.skip("cancel this step");
                        abort
                    });
                    session_ref
                        .run(
                            "pending child",
                            source(),
                            std::future::pending::<()>(),
                            |_| None,
                        )
                        .await;
                    task.await.unwrap();
                    Ok::<(), crate::E2eError>(())
                },
            )
            .await
            .unwrap();
        assert_eq!(result, StepOutcome::Skipped("cancel this step".into()));
        let completed = saved.lock().unwrap().take().unwrap();
        completed.annotate("late", "ignored");
        assert_eq!(completed.annotations().len(), 1);
        let steps = session.finish_all();
        assert_eq!(steps[0].status, StepStatus::Skipped);
        assert_eq!(steps[0].steps[0].status, StepStatus::Interrupted);
        assert_eq!(steps[0].annotations[0].description, "cancel this step");
    }
    #[tokio::test]
    async fn step_deadlines_return_errors_and_zero_leaves_the_outer_budget_in_charge() {
        let session = session();
        let result = session
            .controlled(
                "short",
                source(),
                StepOptions::default().timeout(std::time::Duration::from_millis(10)),
                |_| std::future::pending::<crate::E2eResult<()>>(),
            )
            .await;
        assert!(matches!(result, Err(crate::E2eError::Timeout(10, _))));
        session
            .controlled("zero", source(), StepOptions::default(), |_| async {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                Ok(())
            })
            .await
            .unwrap();
        let steps = session.finish_all();
        assert_eq!(steps[0].status, StepStatus::TimedOut);
        assert_eq!(steps[0].error.as_ref().unwrap().code, "FERRITE_E2E_TIMEOUT");
        assert_eq!(steps[1].status, StepStatus::Passed);
    }
    #[tokio::test]
    async fn automatic_wrappers_hide_implementation_calls_but_keep_user_scopes() {
        let session = session();
        session
            .scope(automatic(None, "before_each", StepCategory::Hook, async {
                automatic(None, "click", StepCategory::Action, async {
                    automatic(None, "mouse down", StepCategory::Action, async { Ok(()) }).await?;
                    session
                        .run(
                            "custom",
                            source(),
                            automatic(None, "read", StepCategory::Action, async { Ok(()) }),
                            |_| None,
                        )
                        .await
                })
                .await
            }))
            .await
            .unwrap();
        let steps = session.finish_all();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].category, StepCategory::Hook);
        assert_eq!(steps[0].steps[0].title, "click");
        assert_eq!(steps[0].steps[0].steps.len(), 1);
        assert_eq!(steps[0].steps[0].steps[0].title, "custom");
        assert_eq!(steps[0].steps[0].steps[0].steps[0].title, "read");
    }

    fn sample_result(name: &str, status: TestStatus) -> TestResult {
        TestResult {
            attempt_results: Vec::new(),
            flaky: false,
            name: name.to_string(),
            status,
            attempts: 1,
            duration_ms: 120,
            error: None,
            screenshots: vec![],
            trace: None,
            video: None,
            project: None,
            repeat_each_index: 0,
            annotations: Vec::new(),
            attachments: Vec::new(),
        }
    }

    fn sample() -> TestReport {
        TestReport {
            configuration: None,
            run_steps: Vec::new(),
            results: vec![
                sample_result("passes", TestStatus::Passed),
                TestResult {
                    attempt_results: Vec::new(),
                    flaky: false,
                    name: "fails <bad>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 3,
                    duration_ms: 4500,
                    error: Some("expect failed: title".to_string()),
                    screenshots: vec!["test-results/fails.png".to_string()],
                    trace: Some("test-results/fails.json".to_string()),
                    video: Some("test-results/fails.webm".to_string()),
                    project: Some("shop".to_string()),
                    repeat_each_index: 0,
                    annotations: vec![("flaky".to_string(), "retry".to_string())],
                    attachments: vec![Attachment {
                        name: "console".to_string(),
                        path: "test-results/console.txt".to_string(),
                        content_type: "text/plain".to_string(),
                    }],
                },
            ],
        }
    }

    #[test]
    fn summary_counts() {
        let report = sample();
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert!(!report.ok());
        assert_eq!(report.exit_code(), 1);
        assert!(report.summary().contains("1 passed, 1 failed"));
    }

    #[test]
    fn list_marks_results() {
        let list = sample().to_list();
        assert!(list.contains("[ok] passes"), "{list}");
        assert!(list.contains("[FAIL] fails <bad>"), "{list}");
        assert!(
            list.contains("screenshot: test-results/fails.png"),
            "{list}"
        );
        assert!(list.contains("video: test-results/fails.webm"), "{list}");
    }

    #[test]
    fn json_round_trips() {
        let report = sample();
        let parsed: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(parsed.results.len(), 2);
    }

    #[test]
    fn flaky_policy_changes_aggregate_serialization_without_fabricating_results() {
        let mut result = sample_result("recovers <&>", TestStatus::Passed);
        result.flaky = true;
        result.attempts = 2;
        let mut report = TestReport {
            results: vec![result],
            ..Default::default()
        };
        assert!(report.ok());
        report.configuration = Some(crate::ResolvedRunConfig {
            fail_on_flaky_tests: true,
            ..Default::default()
        });
        assert!(!report.ok());
        assert_eq!(report.exit_code(), 1);
        assert_eq!(
            (report.passed(), report.failed(), report.flaky()),
            (1, 0, 1)
        );
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["status"], "failed");
        assert_eq!(value["exit_code"], 1);
        assert_eq!(value["flaky_policy_failed"], true);
        assert_eq!(value["results"][0]["status"], "passed");
        let parsed: TestReport = serde_json::from_value(value).unwrap();
        assert!(!parsed.ok());
        assert_eq!(parsed.results.len(), 1);
        assert_eq!(parsed.results[0].attempts, 2);
        for text in [report.to_list(), report.to_dot(), report.to_html()] {
            assert!(text.contains("run failed: fail_on_flaky_tests"));
        }
        let junit = report.to_junit();
        assert!(junit.contains("tests=\"1\" failures=\"1\""));
        assert!(junit.contains("type=\"FlakyTestPolicy\""));
        assert!(junit.contains("name=\"ferrite.final_status\" value=\"passed\""));
        assert!(junit.contains("recovers &lt;&amp;&gt;"));
        report.configuration.as_mut().unwrap().fail_on_flaky_tests = false;
        assert!(report.ok());
        assert!(!report.to_junit().contains("<failure"));
        let mut historical = serde_json::to_value(&report).unwrap();
        historical.as_object_mut().unwrap().remove("configuration");
        assert!(serde_json::from_value::<TestReport>(historical)
            .unwrap()
            .ok());
        report.results[0].flaky = false;
        report.configuration.as_mut().unwrap().fail_on_flaky_tests = true;
        assert!(report.ok());
        report.results[0].status = TestStatus::Failed;
        report.results[0].error = None;
        assert!(!report.ok());
        assert!(report
            .to_junit()
            .contains("<failure message=\"test failed\""));
    }

    #[test]
    fn html_renders_report() {
        let html = sample().to_html();
        assert!(html.contains("<title>ferrite e2e report</title>"), "{html}");
        assert!(html.contains("1 passed, 1 failed"), "{html}");
        assert!(html.contains("fails &lt;bad&gt;"), "{html}");
        assert!(html.contains("pill fail"), "{html}");
        assert!(html.contains("expect failed: title"), "{html}");
        assert!(html.contains("href=\"test-results/fails.png\""), "{html}");
        assert!(html.contains("href=\"test-results/fails.webm\""), "{html}");
    }

    #[test]
    fn junit_escapes_names() {
        let junit = sample().to_junit();
        assert!(junit.contains("tests=\"2\" failures=\"1\""), "{junit}");
        assert!(junit.contains("fails &lt;bad&gt;"), "{junit}");
        assert!(junit.contains("<failure"), "{junit}");
        assert!(junit.contains("classname=\"shop\""), "{junit}");
        assert!(
            junit.contains("<property name=\"video\" value=\"test-results/fails.webm\"/>"),
            "{junit}"
        );
    }

    #[test]
    fn dot_marks_statuses() {
        let report = TestReport {
            configuration: None,
            run_steps: Vec::new(),
            results: vec![
                sample_result("a", TestStatus::Passed),
                sample_result("b", TestStatus::Failed),
                sample_result("c", TestStatus::Skipped),
                sample_result("d", TestStatus::FailedExpected),
            ],
        };
        let dot = report.to_dot();
        assert!(dot.starts_with(".FsE\n"), "{dot}");
        assert!(dot.contains("1 expected-failed"), "{dot}");
    }

    #[test]
    fn expected_failures_do_not_fail_the_run() {
        let mut expected = sample_result("flaky", TestStatus::FailedExpected);
        expected.error = Some("timed out".to_string());
        let report = TestReport {
            configuration: None,
            run_steps: Vec::new(),
            results: vec![expected],
        };
        assert_eq!(report.expected_failed(), 1);
        assert!(report.ok());
        assert_eq!(report.exit_code(), 0);
        assert!(
            !report.to_junit().contains("<failure"),
            "must not fail suite"
        );
        assert!(report.to_list().contains("[expected] flaky"));
        assert!(report.to_html().contains("pill exp"));
    }

    #[test]
    fn list_shows_project_annotations_attachments() {
        let list = sample().to_list();
        assert!(list.contains("project: shop"), "{list}");
        assert!(list.contains("annotation: flaky=retry"), "{list}");
        assert!(
            list.contains("attachment: test-results/console.txt"),
            "{list}"
        );
    }
}

#[cfg(test)]
mod live_reporter_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct PanicReporter;
    impl Reporter for PanicReporter {
        fn on_begin(&self, _: &[crate::Test]) {
            panic!("reporter panic");
        }
    }
    struct Counter(Arc<AtomicUsize>);
    impl Reporter for Counter {
        fn on_begin(&self, _: &[crate::Test]) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn callback_panic_does_not_block_other_reporters() {
        let count = Arc::new(AtomicUsize::new(0));
        let hub = ReporterHub(vec![
            Arc::new(PanicReporter),
            Arc::new(Counter(count.clone())),
        ]);
        hub.emit(|r| r.on_begin(&[]));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
