//! Parallel test runner with retries, timeouts, and artifact capture.

use futures::FutureExt;
use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::ops::Deref;
use std::panic::Location;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::browser::{Browser, BrowserKind, LaunchOptions};
use crate::error::{E2eError, E2eResult};
use crate::page::{Page, ScreenshotOptions};
use crate::report::{
    Attachment, AttemptResult, AttemptStatus, SourceLocation, TestError, TestReport, TestResult,
    TestStatus,
};
use crate::video::{VideoMode, VideoOptions};
use crate::ContextOptions;

#[derive(Clone)]
struct RuntimeState {
    started: tokio::time::Instant,
    timeout: Duration,
    slow: bool,
    expected_fail: bool,
    skipped: Option<String>,
    status: Option<AttemptStatus>,
    errors: Vec<TestError>,
    annotations: Vec<(String, String)>,
}

#[derive(Clone)]
struct RuntimeControl {
    state: Arc<tokio::sync::watch::Sender<RuntimeState>>,
}
impl RuntimeControl {
    fn new(
        timeout: Duration,
        slow: bool,
        expected_fail: bool,
        annotations: Vec<(String, String)>,
    ) -> Self {
        Self {
            state: Arc::new(
                tokio::sync::watch::channel(RuntimeState {
                    started: tokio::time::Instant::now(),
                    timeout,
                    slow,
                    expected_fail,
                    skipped: None,
                    status: None,
                    errors: Vec::new(),
                    annotations,
                })
                .0,
            ),
        }
    }
    fn snapshot(&self) -> RuntimeState {
        self.state.borrow().clone()
    }
    fn restart(&self) {
        self.state
            .send_modify(|state| state.started = tokio::time::Instant::now());
    }
    async fn run<T>(&self, future: impl Future<Output = E2eResult<T>>) -> E2eResult<T> {
        let mut changes = self.state.subscribe();
        let future = future;
        tokio::pin!(future);
        loop {
            let state = changes.borrow_and_update().clone();
            if let Some(reason) = state.skipped {
                return Err(E2eError::Skipped(reason));
            }
            let end = if state.timeout.is_zero() {
                None
            } else {
                state.started.checked_add(state.timeout)
            };
            tokio::select! { biased;
                _=changes.changed()=>{},
                _=async { match end { Some(end)=>tokio::time::sleep_until(end).await, None=>std::future::pending().await } }=>
                    return Err(E2eError::Timeout(state.timeout.as_millis().min(u128::from(u64::MAX)) as u64,"test setup/body".into())),
                result=&mut future=>return result,
            }
        }
    }
}

/// A hook with declared fixture requirements and access to attempt resources.
#[derive(Clone)]
pub struct ContextHook {
    fixtures: Vec<TypeId>,
    func: TestContextFn,
}
impl ContextHook {
    pub fn new<F, Fut>(hook: F) -> Self
    where
        F: Fn(TestContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        Self {
            fixtures: Vec::new(),
            func: Arc::new(move |ctx| Box::pin(hook(ctx))),
        }
    }
    /// Request a lazy fixture before this hook executes.
    pub fn fixture<T: Send + Sync + 'static>(mut self) -> Self {
        self.fixtures.push(TypeId::of::<T>());
        self
    }
}

#[derive(Clone)]
enum EachHook {
    Page(HookFn),
    Context(ContextHook),
}

/// Metadata available before any test resources exist on a worker.
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    pub worker_index: usize,
    pub project: Option<String>,
}

/// Suite-wide hooks can use the browser and worker fixtures, but no test page.
#[derive(Clone)]
pub struct WorkerContext {
    pub browser: Arc<Browser>,
    pub info: WorkerInfo,
    fixtures: FixtureMap,
}
impl WorkerContext {
    pub fn require<T: Send + Sync + 'static>(&self) -> E2eResult<Arc<T>> {
        self.fixtures.require()
    }
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.fixtures.get()
    }
}

/// A suite hook with validated worker-scoped fixture requirements.
#[derive(Clone)]
pub struct WorkerHook {
    fixtures: Vec<TypeId>,
    func: Arc<dyn Fn(WorkerContext) -> BoxTestFuture + Send + Sync>,
}
impl WorkerHook {
    pub fn new<F, Fut>(hook: F) -> Self
    where
        F: Fn(WorkerContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        Self {
            fixtures: Vec::new(),
            func: Arc::new(move |ctx| Box::pin(hook(ctx))),
        }
    }
    pub fn fixture<T: Send + Sync + 'static>(mut self) -> Self {
        self.fixtures.push(TypeId::of::<T>());
        self
    }
}

#[derive(Clone)]
enum SuiteHook {
    Global(GlobalHook),
    Worker(WorkerHook),
}
impl SuiteHook {
    fn fixtures(&self) -> &[TypeId] {
        match self {
            Self::Global(_) => &[],
            Self::Worker(hook) => &hook.fixtures,
        }
    }
    async fn run(
        &self,
        runner: &Runner,
        state: &mut FixtureState,
        info: &WorkerInfo,
    ) -> E2eResult<()> {
        match self {
            Self::Global(hook) => hook().await,
            Self::Worker(hook) => {
                let fixtures = setup_fixtures(
                    &runner.fixtures,
                    &hook.fixtures,
                    state,
                    &mut FixtureState::default(),
                )
                .await?;
                (hook.func)(WorkerContext {
                    browser: fixtures.require()?,
                    info: info.clone(),
                    fixtures,
                })
                .await
            }
        }
    }
}
impl EachHook {
    fn fixtures(&self) -> &[TypeId] {
        match self {
            Self::Page(_) => &[],
            Self::Context(hook) => &hook.fixtures,
        }
    }
    async fn run(&self, context: TestContext) -> E2eResult<()> {
        match self {
            Self::Page(hook) => hook(context.page).await,
            Self::Context(hook) => (hook.func)(context).await,
        }
    }
}

/// A boxed test future.
pub type BoxTestFuture = Pin<Box<dyn Future<Output = E2eResult<()>> + Send + 'static>>;

/// A test body: receives a fresh [`Page`].
pub type TestFn = Arc<dyn Fn(Page) -> BoxTestFuture + Send + Sync>;

/// A test body: receives a [`TestContext`] (page + info + fixtures).
pub type TestContextFn = Arc<dyn Fn(TestContext) -> BoxTestFuture + Send + Sync>;

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
    /// Report skipped (known failure, tracked separately).
    Fixme,
    /// Expect failure: a failed test passes the run, a passing test fails it.
    Fail,
}

/// One named test.
#[derive(Clone)]
pub struct Test {
    /// Context options overriding suite/project/runner defaults.
    pub context_options: Option<ContextOptions>,
    /// Fixtures requested by this test (including their dependencies).
    pub required_fixtures: Vec<TypeId>,
    /// Enclosing suites, outermost first.
    pub suites: Vec<Arc<Suite>>,
    /// Named resources held exclusively while this test runs.
    pub locks: Vec<String>,
    /// Test name.
    pub name: String,
    /// Test body.
    pub func: TestFn,
    /// Context-aware body ([`test_with_context`]; wins over [`Test::func`]).
    pub ctx_func: Option<TestContextFn>,
    /// Tags for filtering.
    pub tags: Vec<String>,
    /// Annotations as (kind, description) pairs (reported, never filtered).
    pub annotations: Vec<(String, String)>,
    /// Source file of the `test()` call.
    pub file: String,
    /// Source line of the `test()` call.
    pub line: u32,
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
    /// Override context defaults for this test.
    pub fn context_options(mut self, options: ContextOptions) -> Self {
        self.context_options = Some(options);
        self
    }
    /// Request a lazily registered fixture and its dependency graph.
    pub fn fixture<T: Send + Sync + 'static>(mut self) -> Self {
        self.required_fixtures.push(TypeId::of::<T>());
        self
    }
    /// Prevent concurrent execution of tests sharing this resource name.
    pub fn lock(mut self, name: impl Into<String>) -> Self {
        self.locks.push(name.into());
        self
    }

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

    /// Expect failure (a failure passes the run, a pass fails it).
    #[must_use]
    pub fn fail(mut self) -> Self {
        self.mode = TestMode::Fail;
        self
    }

    /// Add a reported annotation (never filtered on).
    #[must_use]
    pub fn annotate(mut self, kind: impl Into<String>, description: impl Into<String>) -> Self {
        self.annotations.push((kind.into(), description.into()));
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
#[track_caller]
pub fn test<F, Fut>(name: impl Into<String>, func: F) -> Test
where
    F: Fn(Page) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = E2eResult<()>> + Send + 'static,
{
    let caller = Location::caller();
    Test {
        context_options: None,
        required_fixtures: Vec::new(),
        suites: Vec::new(),
        locks: Vec::new(),
        name: name.into(),
        func: Arc::new(move |page| Box::pin(func(page))),
        ctx_func: None,
        tags: Vec::new(),
        annotations: Vec::new(),
        file: caller.file().to_string(),
        line: caller.line(),
        mode: TestMode::Run,
        retries: None,
        timeout: None,
        slow: false,
    }
}

/// Define a test receiving a [`TestContext`] (page + info + fixtures).
///
/// The context derefs to [`Page`], so `ctx.goto(..)` keeps working.
#[track_caller]
pub fn test_with_context<F, Fut>(name: impl Into<String>, func: F) -> Test
where
    F: Fn(TestContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = E2eResult<()>> + Send + 'static,
{
    let mut test = test(name, |_page: Page| async { Ok(()) });
    test.ctx_func = Some(Arc::new(move |ctx| Box::pin(func(ctx))));
    test
}

/// Group tests under `name` (`"group > test"`); nests naturally.
#[must_use]
pub fn describe(name: &str, tests: Vec<Test>) -> Vec<Test> {
    Suite::new(name).tests(tests)
}

/// A nested suite with hooks scoped to its descendants.
/// `before_all`/`after_all` run once per worker and project that executes it.
#[derive(Clone, Default)]
pub struct Suite {
    context_options: Option<ContextOptions>,
    name: String,
    before_each: Vec<EachHook>,
    after_each: Vec<EachHook>,
    before_all: Vec<SuiteHook>,
    after_all: Vec<SuiteHook>,
    timeout: Option<Duration>,
    retries: Option<u32>,
    tags: Vec<String>,
    mode: Option<TestMode>,
    slow: bool,
}

impl Suite {
    /// Context defaults inherited by descendants without an override.
    pub fn context_options(mut self, options: ContextOptions) -> Self {
        self.context_options = Some(options);
        self
    }
    /// Run once per worker/project with worker fixtures (test-scoped roots are rejected).
    pub fn before_all_with_context(mut self, hook: WorkerHook) -> Self {
        self.before_all.push(SuiteHook::Worker(hook));
        self
    }
    /// Run before worker fixture teardown, including after failures.
    pub fn after_all_with_context(mut self, hook: WorkerHook) -> Self {
        self.after_all.push(SuiteHook::Worker(hook));
        self
    }
    /// Start a named suite; call [`Suite::tests`] to finish it.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Inherit a timeout when a descendant has no override.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    /// Inherit retries when a descendant has no override.
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }
    /// Add a tag to all descendants.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
    /// Skip all descendants.
    pub fn skip(mut self) -> Self {
        self.mode = Some(TestMode::Skip);
        self
    }
    /// Skip descendants as known failures.
    pub fn fixme(mut self) -> Self {
        self.mode = Some(TestMode::Fixme);
        self
    }
    /// Focus runnable descendants.
    pub fn only(mut self) -> Self {
        self.mode = Some(TestMode::Only);
        self
    }
    /// Triple descendant timeouts.
    pub fn slow(mut self) -> Self {
        self.slow = true;
        self
    }
    /// Run before each descendant attempt, outer suites first.
    pub fn before_each<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Page) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.before_each
            .push(EachHook::Page(Arc::new(move |page| Box::pin(hook(page)))));
        self
    }
    /// Run a hook with metadata, built-in resources and declared fixtures.
    pub fn before_each_with_context(mut self, hook: ContextHook) -> Self {
        self.before_each.push(EachHook::Context(hook));
        self
    }
    /// Resolve declared fixtures before cleanup; already-built values are reused.
    pub fn after_each_with_context(mut self, hook: ContextHook) -> Self {
        self.after_each.push(EachHook::Context(hook));
        self
    }
    /// Run after each descendant attempt, inner suites first.
    pub fn after_each<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Page) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_each
            .push(EachHook::Page(Arc::new(move |page| Box::pin(hook(page)))));
        self
    }
    /// Run once before this worker executes descendants in a project.
    pub fn before_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.before_all
            .push(SuiteHook::Global(Arc::new(move || Box::pin(hook()))));
        self
    }
    /// Run once at worker cleanup, inner suites first, even after setup failure.
    pub fn after_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_all
            .push(SuiteHook::Global(Arc::new(move || Box::pin(hook()))));
        self
    }
    /// Finish the suite, preserving nested scopes and descendant overrides.
    pub fn tests(self, tests: Vec<Test>) -> Vec<Test> {
        let suite = Arc::new(self);
        tests
            .into_iter()
            .map(|mut test| {
                test.context_options = test
                    .context_options
                    .or_else(|| suite.context_options.clone());
                test.name = format!("{} > {}", suite.name, test.name);
                test.timeout = test.timeout.or(suite.timeout);
                test.retries = test.retries.or(suite.retries);
                test.tags.extend(suite.tags.clone());
                test.slow |= suite.slow;
                if matches!(suite.mode, Some(TestMode::Skip | TestMode::Fixme))
                    || (suite.mode == Some(TestMode::Only)
                        && !matches!(test.mode, TestMode::Skip | TestMode::Fixme))
                {
                    test.mode = suite.mode.unwrap();
                }
                test.suites.insert(0, suite.clone());
                test
            })
            .collect()
    }
}

/// Metadata for one test execution (Playwright `TestInfo`).
#[derive(Clone)]
pub struct TestInfo {
    /// Test title.
    pub title: String,
    /// Source file of the `test()` call.
    pub file: String,
    /// Source line of the `test()` call.
    pub line: u32,
    /// Test tags.
    pub tags: Vec<String>,
    /// Current attempt (0-based).
    pub retry: u32,
    /// Worker index (`0..workers`).
    pub worker_index: usize,
    /// `repeat_each` index (`0` = single run).
    pub repeat_each_index: u32,
    /// Initial per-attempt timeout; use `effective_timeout()` for runtime updates.
    pub timeout: Duration,
    /// Artifact directory.
    pub output_dir: String,
    /// Project name, if any.
    pub project: Option<String>,
    /// Attachments shared across attempts.
    attachments: Arc<Mutex<Vec<Attachment>>>,
    runtime: RuntimeControl,
    reporters: crate::report::ReporterHub,
    attempt: crate::report::AttemptInfo,
    steps: Option<crate::report::StepSession>,
}

impl TestInfo {
    /// Current raw outcome. `None` while setup/body is still running.
    /// Published before `after_each`; cleanup failures update it immediately.
    pub fn status(&self) -> Option<AttemptStatus> {
        self.runtime.snapshot().status
    }
    /// The expected outcome, including runtime `fail` and `skip` modifiers.
    pub fn expected_status(&self) -> AttemptStatus {
        let state = self.runtime.snapshot();
        if state.skipped.is_some() || state.status == Some(AttemptStatus::Skipped) {
            AttemptStatus::Skipped
        } else if state.expected_fail {
            AttemptStatus::Failed
        } else {
            AttemptStatus::Passed
        }
    }
    /// All failures recorded so far, including setup/body and completed cleanup phases.
    pub fn errors(&self) -> Vec<TestError> {
        self.runtime.snapshot().errors
    }
    fn record_error(&self, error: &E2eError, phase: &str) {
        let status = match error {
            E2eError::Timeout(..) => AttemptStatus::TimedOut,
            E2eError::Cancelled(_) => AttemptStatus::Interrupted,
            E2eError::Skipped(_)
                if matches!(
                    phase,
                    "body" | "test setup" | "worker fixture setup" | "before_all"
                ) =>
            {
                AttemptStatus::Skipped
            }
            _ => AttemptStatus::Failed,
        };
        let location = Some(SourceLocation {
            file: self.file.clone(),
            line: self.line,
            column: 0,
        });
        self.runtime.state.send_modify(|state| {
            if !matches!(
                state.status,
                Some(AttemptStatus::TimedOut | AttemptStatus::Interrupted)
            ) {
                state.status = Some(status);
            }
            if status != AttemptStatus::Skipped {
                state.errors.push(TestError::new(error, phase, location));
            }
        });
    }
    fn body_outcome(&self, outcome: &E2eResult<()>, phase: &str) {
        match outcome {
            Ok(()) => self.runtime.state.send_modify(|state| {
                state.status = Some(if state.skipped.is_some() {
                    AttemptStatus::Skipped
                } else {
                    AttemptStatus::Passed
                })
            }),
            Err(error) => self.record_error(error, phase),
        }
    }
    /// Stop this attempt. Use `info.skip(reason)?` to stop the current closure
    /// immediately. Shared control also stops pending setup/body futures.
    pub fn skip(&self, reason: impl Into<String>) -> E2eResult<()> {
        let reason = reason.into();
        self.runtime.state.send_modify(|state| {
            if state.skipped.is_none() {
                state.skipped = Some(reason.clone());
                state.annotations.push(("skip".into(), reason.clone()));
            }
        });
        Err(E2eError::Skipped(reason))
    }
    /// Mark this attempt as an expected failure. Unexpected passes fail the run.
    pub fn fail(&self, reason: impl Into<String>) {
        let reason = reason.into();
        self.runtime.state.send_modify(|state| {
            state.expected_fail = true;
            state.annotations.push(("fail".into(), reason));
        });
    }
    /// Triple the current budget once, measured from the attempt's start.
    pub fn slow(&self, reason: impl Into<String>) {
        let reason = reason.into();
        self.runtime.state.send_modify(|state| {
            if !state.slow {
                state.timeout = state.timeout.saturating_mul(3);
                state.slow = true;
                state.annotations.push(("slow".into(), reason));
            }
        });
    }
    /// Change the total setup/body budget, including time already spent. Zero disables it.
    pub fn set_timeout(&self, timeout: Duration) {
        self.runtime
            .state
            .send_modify(|state| state.timeout = timeout);
    }
    pub fn effective_timeout(&self) -> Duration {
        self.runtime.snapshot().timeout
    }
    pub fn annotate(&self, kind: impl Into<String>, description: impl Into<String>) {
        let annotation = (kind.into(), description.into());
        self.runtime
            .state
            .send_modify(|state| state.annotations.push(annotation));
    }
    pub fn annotations(&self) -> Vec<(String, String)> {
        self.runtime.snapshot().annotations
    }
    /// Build a path inside this attempt's output directory, creating parents.
    pub fn output_path(&self, name: impl AsRef<std::path::Path>) -> E2eResult<std::path::PathBuf> {
        let name = name.as_ref();
        if name.components().any(|part| {
            !matches!(
                part,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        }) {
            return Err(E2eError::Config(
                "output_path must be relative and stay inside the test output directory".into(),
            ));
        }
        let path = std::path::Path::new(&self.output_dir).join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(path)
    }

    /// Attach bytes as a file under `<output_dir>/attachments/`.
    ///
    /// Returns the written path. The extension is derived from well-known
    /// content types (`.txt`, `.json`, `.png`, `.html`); anything else keeps
    /// the slugged name without an extension.
    pub fn attach(&self, name: &str, body: &[u8], content_type: &str) -> E2eResult<String> {
        let dir = std::path::Path::new(&self.output_dir).join("attachments");
        std::fs::create_dir_all(&dir)?;
        let mut attachments = self.attachments.lock().unwrap_or_else(|e| e.into_inner());
        let stem = format!("{}-{}", slug(&self.title), slug(name));
        let extension = attach_extension(content_type);
        let mut path = dir.join(format!("{stem}{extension}"));
        let mut suffix = 1;
        while path.exists() {
            path = dir.join(format!("{stem}-{suffix}{extension}"));
            suffix += 1;
        }
        std::fs::write(&path, body)?;
        let path = path.display().to_string();
        let attachment = Attachment {
            name: name.to_string(),
            path: path.clone(),
            content_type: content_type.to_string(),
        };
        attachments.push(attachment.clone());
        drop(attachments);
        if let Some(steps) = &self.steps {
            steps.attach(&attachment);
        }
        self.reporters
            .emit(|r| r.on_attachment(&self.attempt, &attachment));
        Ok(path)
    }

    /// Attachments recorded so far (across attempts).
    #[must_use]
    pub fn attachments(&self) -> Vec<Attachment> {
        self.attachments
            .lock()
            .map(|a| a.clone())
            .unwrap_or_default()
    }
}

/// Extension suffix for well-known attachment content types.
fn attach_extension(content_type: &str) -> &'static str {
    let mime = content_type.split(';').next().unwrap_or("").trim();
    match mime {
        "text/plain" => ".txt",
        "text/html" => ".html",
        "application/json" => ".json",
        "image/png" => ".png",
        _ => "",
    }
}

/// Values built by registered fixture setup closures, keyed by type.
#[derive(Clone, Default)]
pub struct FixtureMap {
    inner: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl FixtureMap {
    /// Read a required fixture, returning a configuration error if unavailable.
    pub fn require<T: Send + Sync + 'static>(&self) -> E2eResult<Arc<T>> {
        self.get().ok_or_else(|| {
            E2eError::Config(format!(
                "fixture {} is unavailable; declare its dependency/request",
                std::any::type_name::<T>()
            ))
        })
    }
    /// The fixture value of type `T`, if it has been set up in this scope.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.inner
            .get(&TypeId::of::<T>())?
            .clone()
            .downcast::<T>()
            .ok()
    }
}

/// What a test body receives under [`test_with_context`].
#[derive(Clone)]
pub struct TestContext {
    /// Fresh page for this attempt.
    pub page: Page,
    /// Fresh browser context for this attempt.
    pub context: crate::BrowserContext,
    /// Isolated HTTP client for this attempt, independent of browser cookies.
    pub request: crate::ApiClient,
    /// Execution metadata (attach via [`TestInfo::attach`]).
    pub info: TestInfo,
    /// Fixture values for this attempt.
    fixtures: FixtureMap,
}

impl Deref for TestContext {
    type Target = Page;

    fn deref(&self) -> &Page {
        &self.page
    }
}

impl TestContext {
    pub fn require<T: Send + Sync + 'static>(&self) -> E2eResult<Arc<T>> {
        self.fixtures.require()
    }
    /// The fixture value of type `T`, if it has been set up in this scope.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.fixtures.get()
    }
}

/// A boxed fixture setup future.
type SetupFuture =
    Pin<Box<dyn Future<Output = E2eResult<Arc<dyn Any + Send + Sync>>> + Send + 'static>>;

/// A boxed fixture teardown: receives the setup value.
type TeardownFn = Arc<dyn Fn(Arc<dyn Any + Send + Sync>) -> BoxTestFuture + Send + Sync>;

/// One registered fixture (setup + optional teardown).
#[derive(Clone)]
struct FixtureDef {
    type_id: TypeId,
    name: &'static str,
    dependencies: Vec<TypeId>,
    scope: FixtureScope,
    automatic: bool,
    setup: Arc<dyn Fn(FixtureMap) -> SetupFuture + Send + Sync>,
    teardown: Option<TeardownFn>,
}

/// Lifetime of a fixture value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FixtureScope {
    /// Fresh for each attempt, including retries.
    #[default]
    Test,
    /// Shared by tests on one worker in one project, until worker cleanup.
    Worker,
}

/// Typed fixture definition. Lazy by default; tests request it with [`Test::fixture`].
pub struct Fixture<T> {
    def: FixtureDef,
    marker: std::marker::PhantomData<fn() -> T>,
}

impl<T: Send + Sync + 'static> Fixture<T> {
    /// Setup receives the values of declared dependencies.
    pub fn new<F, Fut>(setup: F) -> Self
    where
        F: Fn(FixtureMap) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<T>> + Send + 'static,
    {
        Self {
            def: FixtureDef {
                type_id: TypeId::of::<T>(),
                name: std::any::type_name::<T>(),
                dependencies: Vec::new(),
                scope: FixtureScope::Test,
                automatic: false,
                setup: Arc::new(move |map| {
                    let future = setup(map);
                    Box::pin(async move {
                        future
                            .await
                            .map(|value| Arc::new(value) as Arc<dyn Any + Send + Sync>)
                    })
                }),
                teardown: None,
            },
            marker: std::marker::PhantomData,
        }
    }
    /// Declare a dependency; registration order does not matter.
    pub fn dependency<D: Send + Sync + 'static>(mut self) -> Self {
        self.def.dependencies.push(TypeId::of::<D>());
        self
    }
    /// Select test or worker lifetime. Worker fixtures may only depend on worker fixtures.
    pub fn scope(mut self, scope: FixtureScope) -> Self {
        self.def.scope = scope;
        self
    }
    /// Set up automatically even when no test explicitly requests this fixture.
    pub fn automatic(mut self, enabled: bool) -> Self {
        self.def.automatic = enabled;
        self
    }
    /// Cleanup runs after dependents, including after failures or cancellation.
    pub fn teardown<F, Fut>(mut self, teardown: F) -> Self
    where
        F: Fn(Arc<T>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        let teardown = Arc::new(teardown);
        self.def.teardown = Some(Arc::new(move |value| {
            let teardown = teardown.clone();
            Box::pin(async move {
                let value = value
                    .downcast::<T>()
                    .map_err(|_| E2eError::Config("fixture teardown type mismatch".into()))?;
                teardown(value).await
            })
        }));
        self
    }
}

#[derive(Default)]
struct FixtureState {
    values: FixtureMap,
    built: Vec<(usize, Arc<dyn Any + Send + Sync>)>,
}

// Setup errors are cached for later suite members. Preserve control-flow kinds
// instead of turning a timeout/cancellation into an ordinary configuration error.
#[derive(Clone)]
enum SetupFailure {
    Failed(String),
    TimedOut(u64, String),
    Interrupted(String),
    Skipped(String),
}
impl SetupFailure {
    fn new(error: E2eError, label: &str) -> Self {
        match error {
            E2eError::Timeout(ms, detail) => Self::TimedOut(ms, format!("{label}: {detail}")),
            E2eError::Cancelled(detail) => Self::Interrupted(format!("{label}: {detail}")),
            E2eError::Skipped(reason) => Self::Skipped(reason),
            error => Self::Failed(format!("{label}: {error}")),
        }
    }
    fn error(&self) -> E2eError {
        match self {
            Self::Failed(message) => E2eError::Config(message.clone()),
            Self::TimedOut(ms, message) => E2eError::Timeout(*ms, message.clone()),
            Self::Interrupted(message) => E2eError::Cancelled(message.clone()),
            Self::Skipped(reason) => E2eError::Skipped(reason.clone()),
        }
    }
}

#[derive(Default)]
struct SuiteState {
    worker: Option<WorkerInfo>,
    started: Vec<(Arc<Suite>, Option<SetupFailure>)>,
}

impl SuiteState {
    async fn setup(
        &mut self,
        test: &Test,
        runner: &Runner,
        fixtures: &mut FixtureState,
        info: WorkerInfo,
        deadline: crate::operation::Deadline,
        control: &crate::CancellationToken,
    ) -> E2eResult<()> {
        self.worker = Some(info.clone());
        for suite in &test.suites {
            if let Some((_, error)) = self
                .started
                .iter()
                .find(|(seen, _)| Arc::ptr_eq(seen, suite))
            {
                if let Some(error) = error {
                    return Err(error.error());
                }
                continue;
            }
            self.started.push((suite.clone(), None));
            for hook in &suite.before_all {
                if let Err(error) = bounded(deadline, Some(control), "suite before_all", async {
                    hook.run(runner, fixtures, &info).await
                })
                .await
                {
                    let error =
                        SetupFailure::new(error, &format!("suite {} before_all", suite.name));
                    self.started.last_mut().unwrap().1 = Some(error.clone());
                    return Err(error.error());
                }
            }
        }
        Ok(())
    }
    async fn cleanup(
        &mut self,
        runner: &Runner,
        fixtures: &mut FixtureState,
        project: Option<&str>,
    ) -> Vec<TestResult> {
        self.cleanup_finished(runner, fixtures, project, &[]).await
    }

    async fn cleanup_finished(
        &mut self,
        runner: &Runner,
        fixtures: &mut FixtureState,
        project: Option<&str>,
        remaining: &[Arc<Suite>],
    ) -> Vec<TestResult> {
        let mut results = Vec::new();
        let mut finished = Vec::new();
        self.started.retain(|(suite, error)| {
            if remaining.iter().any(|pending| Arc::ptr_eq(pending, suite)) {
                true
            } else {
                finished.push((suite.clone(), error.clone()));
                false
            }
        });
        for (suite, _) in finished.into_iter().rev() {
            for hook in &suite.after_all {
                if let Err(error) = bounded(
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "suite after_all",
                    async {
                        hook.run(
                            runner,
                            fixtures,
                            &self.worker.clone().unwrap_or(WorkerInfo {
                                worker_index: 0,
                                project: project.map(str::to_string),
                            }),
                        )
                        .await
                    },
                )
                .await
                {
                    results.push(runner.report_failure(
                        &display_name(project, &format!("{} > <after_all>", suite.name)),
                        error.to_string(),
                    ));
                }
            }
        }
        results
    }
}

async fn retire_worker_resources(
    runner: &Runner,
    suites: &mut SuiteState,
    fixtures: &mut FixtureState,
    project: Option<&str>,
) -> Vec<String> {
    let mut errors: Vec<_> = suites
        .cleanup(runner, fixtures, project)
        .await
        .into_iter()
        .filter_map(|result| result.error)
        .collect();
    if let Some(error) = teardown_fixtures(runner, &fixtures.built).await {
        errors.push(error);
    }
    *fixtures = FixtureState::default();
    errors
}

fn builtin_scope(id: TypeId) -> Option<FixtureScope> {
    if id == TypeId::of::<Browser>() || id == TypeId::of::<WorkerInfo>() {
        return Some(FixtureScope::Worker);
    }
    [
        TypeId::of::<Page>(),
        TypeId::of::<crate::BrowserContext>(),
        TypeId::of::<crate::ApiClient>(),
        TypeId::of::<TestInfo>(),
    ]
    .contains(&id)
    .then_some(FixtureScope::Test)
}

fn fixture_plan(defs: &[FixtureDef], roots: &[TypeId]) -> E2eResult<Vec<usize>> {
    fn visit(
        index: usize,
        defs: &[FixtureDef],
        marks: &mut [u8],
        out: &mut Vec<usize>,
    ) -> E2eResult<()> {
        if marks[index] == 2 {
            return Ok(());
        }
        if marks[index] == 1 {
            return Err(E2eError::Config(format!(
                "fixture dependency cycle at {}",
                defs[index].name
            )));
        }
        marks[index] = 1;
        for dependency in &defs[index].dependencies {
            if let Some(scope) = builtin_scope(*dependency) {
                if defs[index].scope == FixtureScope::Worker && scope == FixtureScope::Test {
                    return Err(E2eError::Config(format!(
                        "worker fixture {} cannot depend on a test-scoped built-in",
                        defs[index].name
                    )));
                }
                continue;
            }
            let dep = defs
                .iter()
                .position(|def| def.type_id == *dependency)
                .ok_or_else(|| {
                    E2eError::Config(format!(
                        "missing dependency for fixture {}",
                        defs[index].name
                    ))
                })?;
            if defs[index].scope == FixtureScope::Worker && defs[dep].scope == FixtureScope::Test {
                return Err(E2eError::Config(format!(
                    "worker fixture {} cannot depend on test fixture {}",
                    defs[index].name, defs[dep].name
                )));
            }
            visit(dep, defs, marks, out)?;
        }
        marks[index] = 2;
        out.push(index);
        Ok(())
    }
    let mut marks = vec![0; defs.len()];
    let mut out = Vec::new();
    for root in roots {
        if builtin_scope(*root).is_some() {
            continue;
        }
        let index = defs
            .iter()
            .position(|def| def.type_id == *root)
            .ok_or_else(|| E2eError::Config("requested fixture is not registered".into()))?;
        visit(index, defs, &mut marks, &mut out)?;
    }
    Ok(out)
}

async fn setup_fixtures(
    defs: &[FixtureDef],
    roots: &[TypeId],
    worker: &mut FixtureState,
    attempt: &mut FixtureState,
) -> E2eResult<FixtureMap> {
    for index in fixture_plan(defs, roots)? {
        let def = &defs[index];
        let target = if def.scope == FixtureScope::Worker {
            &mut *worker
        } else {
            &mut *attempt
        };
        if target.values.inner.contains_key(&def.type_id) {
            continue;
        }
        let mut dependencies = FixtureMap::default();
        for dependency in &def.dependencies {
            if let Some(value) = attempt
                .values
                .inner
                .get(dependency)
                .or_else(|| worker.values.inner.get(dependency))
            {
                dependencies.inner.insert(*dependency, value.clone());
            }
        }
        let value = crate::report::automatic(
            None,
            format!("fixture setup {}", def.name),
            crate::StepCategory::Fixture,
            async { (def.setup)(dependencies).await },
        )
        .await
        .map_err(|error| {
            SetupFailure::new(error, &format!("fixture {} setup", def.name)).error()
        })?;
        let target = if def.scope == FixtureScope::Worker {
            &mut *worker
        } else {
            &mut *attempt
        };
        target.values.inner.insert(def.type_id, value.clone());
        target.built.push((index, value));
    }
    let mut values = worker.values.clone();
    values.inner.extend(attempt.values.inner.clone());
    Ok(values)
}

/// A named group of tests with its own settings (Playwright projects).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Project {
    /// Optional engine override; other projects reuse the supplied browser.
    pub browser: Option<BrowserKind>,
    /// Explicit launch settings for a dedicated project browser.
    pub launch_options: Option<LaunchOptions>,
    /// Context configuration for every attempt in this project.
    pub context_options: Option<ContextOptions>,
    /// Project name (prefixes result names as `"name > test"`).
    pub name: String,
    /// Extra name-or-tag filter applied within this project.
    pub grep: Option<String>,
    /// Retry override for this project.
    pub retries: Option<u32>,
    /// Timeout override for this project.
    pub timeout: Option<Duration>,
}

impl Project {
    /// Select the engine for this project.
    pub fn browser(mut self, browser: BrowserKind) -> Self {
        self.browser = Some(browser);
        self
    }

    /// Launch a dedicated browser using these settings.
    pub fn launch_options(mut self, options: LaunchOptions) -> Self {
        self.browser = Some(options.browser);
        self.launch_options = Some(options);
        self
    }

    /// Configure contexts for this project.
    pub fn context_options(mut self, options: ContextOptions) -> Self {
        self.context_options = Some(options);
        self
    }

    /// A project with defaults (name only).
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Only run tests whose name or tags contain `grep` in this project.
    #[must_use]
    pub fn grep(mut self, grep: impl Into<String>) -> Self {
        self.grep = Some(grep.into());
        self
    }

    /// Retry override for this project.
    #[must_use]
    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    /// Timeout override for this project.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
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

/// True when `CI` is `"1"`/`"true"` (case-insensitive).
fn ci_truthy() -> bool {
    matches!(std::env::var("CI"), Ok(value) if value == "1" || value.eq_ignore_ascii_case("true"))
}

/// Display name (`"project > test"` when projected).
fn display_name(project: Option<&str>, test: &str) -> String {
    match project {
        Some(name) => format!("{name} > {test}"),
        None => test.to_string(),
    }
}

/// One runnable test with resolved settings.
#[derive(Clone)]
pub(crate) struct WorkItem {
    context_options: Option<ContextOptions>,
    test: Test,
    project: Option<String>,
    retries: u32,
    timeout: Duration,
    repeat_each_index: u32,
}

impl WorkItem {
    fn display_name(&self) -> String {
        display_name(self.project.as_deref(), &self.test.name)
    }
}

/// Resolve which projects run (`None` = the implicit unprefixed project).
///
/// Unknown wanted names fail loudly.
pub(crate) fn resolve_projects(
    all: &[Project],
    wanted: &[String],
) -> Result<Vec<Option<Project>>, String> {
    if wanted.is_empty() {
        if all.is_empty() {
            return Ok(vec![None]);
        }
        return Ok(all.iter().cloned().map(Some).collect());
    }
    let mut out = Vec::with_capacity(wanted.len());
    for name in wanted {
        match all.iter().find(|project| &project.name == name) {
            Some(project) => out.push(Some(project.clone())),
            None => {
                let have: Vec<&str> = all.iter().map(|p| p.name.as_str()).collect();
                return Err(format!(
                    "unknown project {name:?} (have: {})",
                    have.join(", ")
                ));
            }
        }
    }
    Ok(out)
}

/// Build the runnable and skipped lists: per-project selection (global
/// filters plus the project grep), `repeat_each` expansion, then one shard
/// over the combined display names.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_work_items(
    tests: &[Test],
    filter: Option<&str>,
    grep: Option<&str>,
    grep_invert: Option<&str>,
    projects: &[Option<Project>],
    runner_retries: u32,
    runner_timeout: Duration,
    repeat_each: u32,
    shard: Option<(usize, usize)>,
) -> (Vec<WorkItem>, Vec<(Test, Option<String>)>) {
    let mut runnable = Vec::new();
    let mut skipped = Vec::new();
    for project in projects {
        for test in select(tests, filter, grep, grep_invert, None) {
            if let Some(want) = project.as_ref().and_then(|p| p.grep.as_deref()) {
                if !(test.name.contains(want) || test.tags.iter().any(|tag| tag.contains(want))) {
                    continue;
                }
            }
            let name = project.clone().map(|p| p.name);
            match test.mode {
                TestMode::Skip | TestMode::Fixme => skipped.push(((*test).clone(), name)),
                _ => {
                    for repeat in 0..repeat_each.max(1) {
                        runnable.push(WorkItem {
                            context_options: test.context_options.clone().or_else(|| {
                                project.as_ref().and_then(|p| p.context_options.clone())
                            }),
                            test: (*test).clone(),
                            project: name.clone(),
                            retries: test
                                .retries
                                .or_else(|| project.as_ref().and_then(|p| p.retries))
                                .unwrap_or(runner_retries),
                            timeout: test
                                .timeout
                                .or_else(|| project.as_ref().and_then(|p| p.timeout))
                                .unwrap_or(runner_timeout),
                            repeat_each_index: repeat,
                        });
                    }
                }
            }
        }
    }
    if let Some((index, total)) = shard {
        runnable = take_shard(runnable, index, total);
    }
    (runnable, skipped)
}

/// Take one 1-based shard by display-name order (`repeat_each_index` breaks ties).
fn take_shard(mut items: Vec<WorkItem>, index: usize, total: usize) -> Vec<WorkItem> {
    items.sort_by(|a, b| {
        (a.display_name(), a.repeat_each_index).cmp(&(b.display_name(), b.repeat_each_index))
    });
    let total = total.max(1);
    let want = index.saturating_sub(1) % total;
    items
        .into_iter()
        .enumerate()
        .filter(|(position, _)| position % total == want)
        .map(|(_, item)| item)
        .collect()
}

/// A one-off failed result (`<global setup>`, `<join>`, ...).
fn failed_result(name: &str, error: String) -> TestResult {
    TestResult {
        attempt_results: Vec::new(),
        flaky: false,
        name: name.to_string(),
        status: TestStatus::Failed,
        attempts: 1,
        duration_ms: 0,
        error: Some(error),
        screenshots: vec![],
        trace: None,
        video: None,
        project: None,
        repeat_each_index: 0,
        annotations: Vec::new(),
        attachments: Vec::new(),
    }
}

/// Runs tests against a [`Browser`] with workers, retries, and artifacts.
/// A per-test hook (`Page` in, unit out).
pub type HookFn = Arc<dyn Fn(Page) -> BoxTestFuture + Send + Sync>;

/// A run-wide hook (no page).
pub type GlobalHook = Arc<dyn Fn() -> BoxTestFuture + Send + Sync>;

#[derive(Clone)]
pub struct Runner {
    firefox_lifecycle: Arc<tokio::sync::Mutex<()>>,
    context_options: ContextOptions,
    expect_timeout: Duration,
    reporter: String,
    reporters: crate::report::ReporterHub,
    resource_locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    workers: usize,
    retries: u32,
    test_timeout: Duration,
    global_timeout: Duration,
    cleanup_timeout: Duration,
    max_failures: usize,
    cancellation: crate::CancellationToken,
    filter: Option<String>,
    grep: Option<String>,
    grep_invert: Option<String>,
    shard: Option<(usize, usize)>,
    before_each: Vec<EachHook>,
    after_each: Vec<EachHook>,
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
    projects: Vec<Project>,
    repeat_each: u32,
    forbid_only: bool,
    fixtures: Vec<FixtureDef>,
}

impl Default for Runner {
    fn default() -> Self {
        Self::from_env().expect("invalid Ferrite E2E environment configuration")
    }
}

impl Runner {
    /// Read configuration exported by the Ferrite CLI.
    pub fn from_env() -> E2eResult<Self> {
        Ok(Self::from_config(&crate::config_from_env()?))
    }

    /// Add a live reporter alongside the configured file reporters.
    pub fn custom_reporter<R: crate::report::Reporter>(mut self, reporter: R) -> Self {
        self.reporters.0.push(Arc::new(reporter));
        self
    }
    /// Configure the fresh context created for each attempt.
    pub fn context_options(mut self, options: ContextOptions) -> Self {
        self.context_options = options;
        self
    }

    /// Configure default assertion retries on every test page.
    pub fn expect_timeout(mut self, timeout: Duration) -> Self {
        self.expect_timeout = timeout;
        self
    }

    /// Build from resolved e2e config.
    #[must_use]
    pub fn from_config(config: &ferrite_config::E2eConfig) -> Self {
        Self {
            firefox_lifecycle: Arc::new(tokio::sync::Mutex::new(())),
            context_options: ContextOptions {
                viewport: config.viewport.as_ref().map(|v| crate::Viewport {
                    width: v.width,
                    height: v.height,
                }),
                ..ContextOptions::default()
            },
            expect_timeout: Duration::from_millis(config.expect_timeout_ms),
            reporter: config.reporter.clone(),
            reporters: crate::report::ReporterHub::default(),
            resource_locks: Arc::new(Mutex::new(HashMap::new())),
            workers: config.workers.max(1),
            retries: config.retries,
            test_timeout: Duration::from_millis(config.timeout_ms),
            global_timeout: Duration::from_millis(config.global_timeout_ms),
            cleanup_timeout: Duration::from_millis(config.cleanup_timeout_ms),
            max_failures: config.max_failures,
            cancellation: crate::CancellationToken::new(),
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
            projects: Vec::new(),
            repeat_each: 1,
            forbid_only: false,
            fixtures: Vec::new(),
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

    /// Whole-run deadline; zero disables it. Cleanup has its own grace period.
    pub fn global_timeout(mut self, timeout: Duration) -> Self {
        self.global_timeout = timeout;
        self
    }
    /// Stop scheduling after this many final unexpected failures; zero disables it.
    /// Tests already running finish normally, and retries count as one test.
    pub fn max_failures(mut self, count: usize) -> Self {
        self.max_failures = count;
        self
    }
    /// Independent deadline for each cleanup callback or browser operation.
    pub fn cleanup_timeout(mut self, timeout: Duration) -> Self {
        self.cleanup_timeout = timeout;
        self
    }
    /// Interrupt this runner's work; teardown still runs.
    pub fn with_cancellation(mut self, token: crate::CancellationToken) -> Self {
        self.cancellation = token;
        self
    }
    fn report_failure(&self, name: &str, error: String) -> TestResult {
        self.reporters.emit(|r| r.on_error(None, &error));
        failed_result(name, error)
    }
    async fn lifecycle_guard(
        &self,
        browser: &Browser,
    ) -> E2eResult<Option<tokio::sync::OwnedMutexGuard<()>>> {
        Ok(if browser.kind() == BrowserKind::Firefox {
            Some(self.firefox_lifecycle.clone().lock_owned().await)
        } else {
            None
        })
    }
    async fn finish_run(&self, mut report: TestReport) -> TestReport {
        for (label, hooks) in [
            ("<after_all>", &self.after_all),
            ("<global teardown>", &self.global_teardown),
        ] {
            for hook in hooks {
                if let Err(error) = bounded(
                    crate::operation::Deadline::new(self.cleanup_timeout),
                    None,
                    label,
                    async { hook().await },
                )
                .await
                {
                    report
                        .results
                        .push(self.report_failure(label, error.to_string()));
                }
            }
        }
        report.run_steps = crate::report::current_session()
            .map(|s| s.finish_all())
            .unwrap_or_default();
        report.results.sort_by(|a, b| a.name.cmp(&b.name));
        self.write_artifacts(&report, &self.reporter);
        self.reporters.emit(|r| r.on_end(&report));
        report
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
            .push(EachHook::Page(Arc::new(move |page| Box::pin(hook(page)))));
        self
    }

    /// Run a hook with metadata, built-in resources and declared fixtures.
    pub fn before_each_with_context(mut self, hook: ContextHook) -> Self {
        self.before_each.push(EachHook::Context(hook));
        self
    }
    /// Resolve declared fixtures before cleanup; already-built values are reused.
    pub fn after_each_with_context(mut self, hook: ContextHook) -> Self {
        self.after_each.push(EachHook::Context(hook));
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
            .push(EachHook::Page(Arc::new(move |page| Box::pin(hook(page)))));
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

    /// Add a project (repeatable; result names gain a `"name > "` prefix).
    ///
    /// Use `--project` (`FERRITE_E2E_PROJECT`, comma-separated) to run a
    /// subset; unknown names fail the run loudly.
    #[must_use]
    pub fn project(mut self, project: Project) -> Self {
        self.projects.push(project);
        self
    }

    /// Run every test `n` times (distinguished by `repeat_each_index`).
    #[must_use]
    pub fn repeat_each(mut self, n: u32) -> Self {
        self.repeat_each = n.max(1);
        self
    }

    /// Fail the run when any test uses [`TestMode::Only`].
    ///
    /// Also enforced automatically when `CI` is `"1"`/`"true"`.
    #[must_use]
    pub fn forbid_only(mut self, forbid: bool) -> Self {
        self.forbid_only = forbid;
        self
    }

    /// Register a fixture value built fresh for every attempt.
    ///
    /// `setup` runs before each attempt (failures fail the attempt);
    /// read the value via [`TestContext::get`]. Later fixtures see nothing
    /// of earlier ones (no ordering dependency).
    #[must_use]
    pub fn fixture<T, F, Fut>(mut self, setup: F) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<T>> + Send + 'static,
    {
        let setup = Arc::new(move |_map: FixtureMap| {
            let fut = setup();
            Box::pin(async move {
                fut.await
                    .map(|value| Arc::new(value) as Arc<dyn Any + Send + Sync>)
            }) as SetupFuture
        });
        self.fixtures.push(FixtureDef {
            type_id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
            dependencies: Vec::new(),
            scope: FixtureScope::Test,
            automatic: true,
            setup,
            teardown: None,
        });
        self
    }

    /// Register a fixture with teardown (runs after every attempt, in reverse).
    ///
    /// Teardown failures fail the attempt; all teardowns still run.
    #[must_use]
    pub fn fixture_with_teardown<T, F, Fut, G, Fut2>(mut self, setup: F, teardown: G) -> Self
    where
        T: Send + Sync + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<T>> + Send + 'static,
        G: Fn(Arc<T>) -> Fut2 + Send + Sync + 'static,
        Fut2: Future<Output = E2eResult<()>> + Send + 'static,
    {
        let setup = Arc::new(move |_map: FixtureMap| {
            let fut = setup();
            Box::pin(async move {
                fut.await
                    .map(|value| Arc::new(value) as Arc<dyn Any + Send + Sync>)
            }) as SetupFuture
        });
        let teardown_fn = Arc::new(teardown);
        let teardown = Arc::new(move |any: Arc<dyn Any + Send + Sync>| {
            let teardown_fn = Arc::clone(&teardown_fn);
            Box::pin(async move {
                let typed: Arc<T> = any.downcast::<T>().map_err(|_| {
                    E2eError::Config("fixture type mismatch in teardown".to_string())
                })?;
                teardown_fn(typed).await
            }) as BoxTestFuture
        });
        self.fixtures.push(FixtureDef {
            type_id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
            dependencies: Vec::new(),
            scope: FixtureScope::Test,
            automatic: true,
            setup,
            teardown: Some(teardown),
        });
        self
    }

    /// Register a typed, lazy fixture with dependencies and scope.
    pub fn fixture_definition<T: Send + Sync + 'static>(mut self, fixture: Fixture<T>) -> Self {
        self.fixtures.push(fixture.def);
        self
    }

    /// Run tests to completion (never fails the call itself).
    ///
    /// Unset builder filters fall back to `FERRITE_E2E_FILTER`,
    /// `FERRITE_E2E_GREP`, `FERRITE_E2E_GREP_INVERT`, and `FERRITE_E2E_SHARD`
    /// (set by the CLI flags); `FERRITE_E2E_PROJECT` (comma-separated, from
    /// `--project`) selects which projects run. Snapshot assertions default to
    /// this runner's output directory without mutating process environment.
    /// Tests run on a worker pool
    /// (`worker_index` in [`TestInfo`]); progress prints in completion order.
    pub async fn run(&self, browser: &Browser, tests: Vec<Test>) -> TestReport {
        let session = crate::report::StepSession::new(
            crate::report::ReporterHub::default(),
            crate::AttemptInfo {
                name: "Run lifecycle".into(),
                file: "<run>".into(),
                line: 0,
                project: None,
                worker_index: 0,
                repeat_each_index: 0,
                retry: 0,
            },
        );
        session.scope(self.run_inner(browser, tests)).await
    }
    async fn run_inner(&self, browser: &Browser, tests: Vec<Test>) -> TestReport {
        self.reporters.emit(|r| r.on_begin(&tests));
        let mut report = TestReport::default();
        let control = crate::CancellationToken::new();
        if let Some(reason) = self.cancellation.reason() {
            control.cancel_with_reason(reason);
        }
        let timer_token = control.clone();
        let external = self.cancellation.clone();
        let deadline = crate::operation::Deadline::new(self.global_timeout);
        let _timer = AbortTask(tokio::spawn(async move {
            tokio::select! {
                ()=deadline.elapsed()=>timer_token.cancel_with_reason("global timeout exceeded"),
                reason=external.cancelled()=>timer_token.cancel_with_reason(reason),
            }
        }));
        let roots: Vec<_> = self.fixtures.iter().map(|def| def.type_id).collect();
        let mut seen = std::collections::HashSet::new();
        let validation = if self
            .fixtures
            .iter()
            .any(|def| builtin_scope(def.type_id).is_some() || !seen.insert(def.type_id))
        {
            Err(E2eError::Config(
                "duplicate or reserved built-in fixture type registration".into(),
            ))
        } else {
            fixture_plan(&self.fixtures, &roots).and_then(|_| {
                for test in &tests {
                    fixture_plan(&self.fixtures, &test.required_fixtures)?;
                    for hook in test
                        .suites
                        .iter()
                        .flat_map(|suite| suite.before_all.iter().chain(suite.after_all.iter()))
                    {
                        let plan = fixture_plan(&self.fixtures, hook.fixtures())?;
                        if hook
                            .fixtures()
                            .iter()
                            .any(|id| builtin_scope(*id) == Some(FixtureScope::Test))
                            || plan
                                .iter()
                                .any(|index| self.fixtures[*index].scope == FixtureScope::Test)
                        {
                            return Err(E2eError::Config(
                                "suite-wide hooks may only request worker-scoped fixtures".into(),
                            ));
                        }
                    }
                    for hook in self.before_each.iter().chain(self.after_each.iter()).chain(
                        test.suites.iter().flat_map(|suite| {
                            suite.before_each.iter().chain(suite.after_each.iter())
                        }),
                    ) {
                        fixture_plan(&self.fixtures, hook.fixtures())?;
                    }
                }
                Ok(())
            })
        };
        if let Err(error) = validation {
            report
                .results
                .push(self.report_failure("<fixtures>", error.to_string()));
            self.write_artifacts(&report, &self.reporter);
            self.reporters.emit(|r| r.on_end(&report));
            return report;
        }
        for setup in &self.global_setup {
            if let Err(error) = bounded(
                crate::operation::Deadline::new(self.test_timeout),
                Some(&control),
                "global setup",
                async { setup().await },
            )
            .await
            {
                report
                    .results
                    .push(self.report_failure("<global setup>", error.to_string()));
                return self.finish_run(report).await;
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
        let project_filter: Vec<String> = env_filter("FERRITE_E2E_PROJECT")
            .map(|raw| {
                raw.split(',')
                    .map(|name| name.trim().to_string())
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let projects = match resolve_projects(&self.projects, &project_filter) {
            Ok(projects) => projects,
            Err(error) => {
                report.results.push(self.report_failure("<project>", error));
                return self.finish_run(report).await;
            }
        };
        let (runnable, skipped) = build_work_items(
            &tests,
            filter.as_deref(),
            grep.as_deref(),
            grep_invert.as_deref(),
            &projects,
            self.retries,
            self.test_timeout,
            self.repeat_each,
            shard,
        );
        if (self.forbid_only || ci_truthy())
            && runnable.iter().any(|item| item.test.mode == TestMode::Only)
        {
            report.results.push(self.report_failure(
                "<forbid-only>",
                "test.only is forbidden (forbid_only/CI)".to_string(),
            ));
            return self.finish_run(report).await;
        }
        for (test, project) in &skipped {
            let name = display_name(project.as_deref(), &test.name);
            if self.list_progress {
                println!("[skip] {name}");
            }
            report.results.push(TestResult {
                attempt_results: Vec::new(),
                flaky: false,
                name,
                status: TestStatus::Skipped,
                attempts: 0,
                duration_ms: 0,
                error: None,
                screenshots: vec![],
                trace: None,
                video: None,
                project: project.clone(),
                repeat_each_index: 0,
                annotations: test.annotations.clone(),
                attachments: Vec::new(),
            });
        }
        for hook in &self.before_all {
            if let Err(error) = bounded(
                crate::operation::Deadline::new(self.test_timeout),
                Some(&control),
                "before_all",
                async { hook().await },
            )
            .await
            {
                report
                    .results
                    .push(self.report_failure("<before_all>", error.to_string()));
                return self.finish_run(report).await;
            }
        }
        let mut project_browsers = HashMap::new();
        let mut owned_browsers = Vec::new();
        let mut rejected_projects = Vec::new();
        for project in projects.iter().flatten() {
            let kind = project.browser.unwrap_or(browser.kind());
            if project.launch_options.is_some() || kind != browser.kind() {
                let options = project
                    .launch_options
                    .clone()
                    .unwrap_or_else(|| LaunchOptions::default().browser(kind));
                match bounded(
                    crate::operation::Deadline::new(self.test_timeout),
                    Some(&control),
                    "browser launch",
                    Browser::launch(options),
                )
                .await
                {
                    Ok(mut launched) => {
                        launched.set_base_url(browser.base_url().map(str::to_string));
                        let launched = Arc::new(launched);
                        owned_browsers.push(launched.clone());
                        project_browsers.insert(project.name.clone(), launched);
                    }
                    Err(error) => {
                        rejected_projects.push(project.name.clone());
                        report.results.push(self.report_failure(
                            &format!("{} > <browser launch>", project.name),
                            error.to_string(),
                        ));
                    }
                }
            }
        }
        let runnable = runnable
            .into_iter()
            .filter(|item| {
                !item
                    .project
                    .as_ref()
                    .is_some_and(|name| rejected_projects.contains(name))
            })
            .collect::<Vec<_>>();
        let project_browsers = Arc::new(project_browsers);
        // Worker pool: stable worker index; isolated context per attempt.
        let queue = Arc::new(Mutex::new(VecDeque::from(runnable)));
        let browser = Arc::new(browser.worker_handle());
        let failures = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut workers = tokio::task::JoinSet::new();
        for worker_index in 0..self.workers {
            let queue = Arc::clone(&queue);
            let runner = self.clone();
            let browser = Arc::clone(&browser);
            let project_browsers = project_browsers.clone();
            let control = control.clone();
            let failures = failures.clone();
            let run_session = crate::report::current_session().expect("runner scope");
            workers.spawn(async move {
                run_session
                    .scope(crate::report::automatic(
                        None,
                        format!("worker {worker_index}"),
                        crate::StepCategory::Hook,
                        async move {
                            let mut results = Vec::new();
                            let mut fixture_states: HashMap<Option<String>, FixtureState> =
                                HashMap::new();
                            let mut suite_states: HashMap<Option<String>, SuiteState> =
                                HashMap::new();
                            loop {
                                let item = queue
                                    .lock()
                                    .map(|mut q| {
                                        if control.is_cancelled()
                                            || (runner.max_failures > 0
                                                && failures
                                                    .load(std::sync::atomic::Ordering::SeqCst)
                                                    >= runner.max_failures)
                                        {
                                            None
                                        } else {
                                            q.pop_front()
                                        }
                                    })
                                    .unwrap_or(None);
                                match item {
                                    Some(item) => {
                                        let selected = item
                                            .project
                                            .as_ref()
                                            .and_then(|name| project_browsers.get(name))
                                            .unwrap_or(&browser);
                                        let result = run_one(
                                            &runner,
                                            selected,
                                            &item,
                                            worker_index,
                                            &control,
                                            fixture_states.entry(item.project.clone()).or_default(),
                                            suite_states.entry(item.project.clone()).or_default(),
                                        )
                                        .await;
                                        if result.status == TestStatus::Failed {
                                            failures
                                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                            let errors = retire_worker_resources(
                                                &runner,
                                                suite_states
                                                    .entry(item.project.clone())
                                                    .or_default(),
                                                fixture_states
                                                    .entry(item.project.clone())
                                                    .or_default(),
                                                item.project.as_deref(),
                                            )
                                            .await;
                                            if !errors.is_empty() {
                                                results.push(runner.report_failure(
                                                    &display_name(
                                                        item.project.as_deref(),
                                                        "<worker cleanup>",
                                                    ),
                                                    errors.join("; "),
                                                ));
                                            }
                                        }
                                        results.push(result);
                                        let remaining: Vec<_> = queue
                                            .lock()
                                            .unwrap_or_else(|e| e.into_inner())
                                            .iter()
                                            .filter(|pending| pending.project == item.project)
                                            .flat_map(|pending| pending.test.suites.iter().cloned())
                                            .collect();
                                        let cleanup = suite_states
                                            .entry(item.project.clone())
                                            .or_default()
                                            .cleanup_finished(
                                                &runner,
                                                fixture_states
                                                    .entry(item.project.clone())
                                                    .or_default(),
                                                item.project.as_deref(),
                                                &remaining,
                                            )
                                            .await;
                                        failures.fetch_add(
                                            cleanup.len(),
                                            std::sync::atomic::Ordering::SeqCst,
                                        );
                                        if !cleanup.is_empty() {
                                            let errors = retire_worker_resources(
                                                &runner,
                                                suite_states
                                                    .entry(item.project.clone())
                                                    .or_default(),
                                                fixture_states
                                                    .entry(item.project.clone())
                                                    .or_default(),
                                                item.project.as_deref(),
                                            )
                                            .await;
                                            if !errors.is_empty() {
                                                failures.fetch_add(
                                                    1,
                                                    std::sync::atomic::Ordering::SeqCst,
                                                );
                                                results.push(runner.report_failure(
                                                    &display_name(
                                                        item.project.as_deref(),
                                                        "<worker cleanup>",
                                                    ),
                                                    errors.join("; "),
                                                ));
                                            }
                                        }
                                        results.extend(cleanup);
                                    }
                                    None => break,
                                }
                            }
                            for (project, state) in &mut suite_states {
                                results.extend(
                                    state
                                        .cleanup(
                                            &runner,
                                            fixture_states.entry(project.clone()).or_default(),
                                            project.as_deref(),
                                        )
                                        .await,
                                );
                            }
                            for (project, state) in &fixture_states {
                                if let Some(error) = teardown_fixtures(&runner, &state.built).await
                                {
                                    results.push(runner.report_failure(
                                        &display_name(project.as_deref(), "<worker fixtures>"),
                                        error,
                                    ));
                                }
                            }
                            Ok(results)
                        },
                    ))
                    .await
                    .expect("worker scope returns outcomes")
            });
        }
        while let Some(joined) = workers.join_next().await {
            match joined {
                Ok(results) => {
                    for result in results {
                        if self.list_progress {
                            let mark = match result.status {
                                TestStatus::Passed => "ok",
                                TestStatus::Failed => "FAIL",
                                TestStatus::Skipped => "skip",
                                TestStatus::FailedExpected => "expected",
                            };
                            println!("[{mark}] {}", result.name);
                            if let Some(error) = &result.error {
                                println!("       {error}");
                            }
                        }
                        report.results.push(result);
                    }
                }
                Err(error) => report
                    .results
                    .push(self.report_failure("<join>", error.to_string())),
            }
        }
        if let Ok(mut pending) = queue.lock() {
            for item in pending.drain(..) {
                let mut result = self.report_failure(
                    &item.display_name(),
                    control
                        .reason()
                        .unwrap_or_else(|| "max_failures reached".into()),
                );
                result.status = TestStatus::Skipped;
                result.attempts = 0;
                result.project = item.project;
                result.repeat_each_index = item.repeat_each_index;
                result.annotations = item.test.annotations;
                report.results.push(result);
            }
        }
        if let Some(reason) = control.reason() {
            report
                .results
                .push(self.report_failure("<run interrupted>", reason));
        }
        drop(project_browsers);
        for browser in owned_browsers {
            if let Ok(browser) = Arc::try_unwrap(browser) {
                if let Err(error) = bounded(
                    crate::operation::Deadline::new(self.cleanup_timeout),
                    None,
                    "browser close",
                    browser.close(),
                )
                .await
                {
                    report
                        .results
                        .push(self.report_failure("<browser close>", error.to_string()));
                }
            }
        }
        self.finish_run(report).await
    }

    /// Write file artifacts for the reporters in `spec` (comma-separated).
    ///
    /// `html` exports a portable bundle with relative artifact links and companion
    /// JSON/JUnit files. Selected JSON/JUnit reporters reuse those files; without
    /// HTML they retain source paths. Bundle failures emit `Reporter::on_error`.
    /// `list` and `dot` are printed by the caller via [`TestReport::to_list`] /
    /// [`TestReport::to_dot`].
    pub fn write_artifacts(&self, report: &TestReport, spec: &str) -> Vec<String> {
        let mut written = Vec::new();
        std::fs::create_dir_all(&self.output_dir).ok();
        let formats: Vec<_> = spec.split(',').map(str::trim).collect();
        let bundle = if formats.contains(&"html") {
            match report.write_bundle(&self.output_dir) {
                Ok(bundle) => Some(bundle),
                Err(error) => {
                    self.reporters
                        .emit(|r| r.on_error(None, &format!("report bundle: {error}")));
                    None
                }
            }
        } else {
            None
        };
        for format in formats {
            let (path, data) = match format {
                "html" => {
                    if let Some(bundle) = &bundle {
                        written.push(bundle.html.display().to_string());
                    }
                    continue;
                }
                "json" => (
                    format!("{}/results.json", self.output_dir),
                    report.to_json(),
                ),
                "junit" => (format!("{}/junit.xml", self.output_dir), report.to_junit()),
                _ => continue,
            };
            if bundle.is_some() || std::fs::write(&path, data).is_ok() {
                written.push(path);
            }
        }
        written
    }
}

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn bounded_in<T>(
    session: Option<&crate::report::StepSession>,
    deadline: crate::operation::Deadline,
    token: Option<&crate::CancellationToken>,
    label: &str,
    future: impl Future<Output = E2eResult<T>>,
) -> E2eResult<T> {
    match session {
        Some(session) => session.scope(bounded(deadline, token, label, future)).await,
        None => bounded(deadline, token, label, future).await,
    }
}
async fn bounded<T>(
    deadline: crate::operation::Deadline,
    token: Option<&crate::CancellationToken>,
    label: &str,
    future: impl Future<Output = E2eResult<T>>,
) -> E2eResult<T> {
    let work = async {
        let category = if label.contains("fixture") {
            crate::StepCategory::Fixture
        } else {
            crate::StepCategory::Hook
        };
        let instrument =
            label.contains("before") || label.contains("after") || label.contains("global");
        let work = async {
            if instrument {
                crate::report::automatic(None, label, category, future).await
            } else {
                future.await
            }
        };
        match std::panic::AssertUnwindSafe(work).catch_unwind().await {
            Ok(result) => result,
            Err(panic) => Err(E2eError::Config(format!(
                "{label} panicked: {}",
                panic
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| panic.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic")
            ))),
        }
    };
    let work = deadline.run(label, work);
    match token {
        Some(token) => token.run(work).await,
        None => work.await,
    }
}

fn attempt_history(history: &Mutex<Vec<AttemptResult>>) -> Vec<AttemptResult> {
    history.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

struct AttemptGuard {
    hub: crate::report::ReporterHub,
    info: TestInfo,
    result: TestResult,
    started: Instant,
    start_time_ms: u64,
    attachments_start: usize,
    history: Arc<Mutex<Vec<AttemptResult>>>,
}
impl AttemptGuard {
    fn new(
        hub: crate::report::ReporterHub,
        info: TestInfo,
        history: Arc<Mutex<Vec<AttemptResult>>>,
    ) -> Self {
        let started = Instant::now();
        let start_time_ms = crate::driver::now_ms();
        hub.emit(|r| r.on_test_begin(&info.attempt));
        let mut result = failed_result(
            &info.attempt.name,
            "attempt interrupted before completion".into(),
        );
        result.attempts = info.retry + 1;
        result.project = info.project.clone();
        result.repeat_each_index = info.repeat_each_index;
        let attachments_start = info.attachments().len();
        Self {
            hub,
            info,
            result,
            started,
            start_time_ms,
            attachments_start,
            history,
        }
    }
    fn outcome(&mut self, status: TestStatus, error: Option<String>) {
        self.result.status = status;
        self.result.error = error;
    }
}
impl Drop for AttemptGuard {
    fn drop(&mut self) {
        if self.info.status().is_none() {
            self.info.record_error(
                &E2eError::Cancelled("attempt interrupted before completion".into()),
                "attempt",
            );
        }
        self.result.duration_ms =
            self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        self.result.annotations = self.info.annotations();
        self.result.attachments = self
            .info
            .attachments()
            .into_iter()
            .skip(self.attachments_start)
            .collect();
        let attempt = AttemptResult {
            info: self.info.attempt.clone(),
            status: self.info.status().unwrap(),
            expected_status: self.info.expected_status(),
            is_expected: self.result.status != TestStatus::Failed,
            start_time_ms: self.start_time_ms,
            duration_ms: self.result.duration_ms,
            errors: self.info.errors(),
            annotations: self.result.annotations.clone(),
            steps: self
                .info
                .steps
                .as_ref()
                .map(|s| s.finish_all())
                .unwrap_or_default(),
            attachments: self.result.attachments.clone(),
            screenshots: self.result.screenshots.clone(),
            trace: self.result.trace.clone(),
            video: self.result.video.clone(),
        };
        self.history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(attempt.clone());
        self.result.attempt_results = vec![attempt];
        if let Some(error) = &self.result.error {
            self.hub
                .emit(|r| r.on_error(Some(&self.info.attempt), error));
        }
        self.hub
            .emit(|r| r.on_test_end(&self.info.attempt, &self.result));
    }
}

async fn run_one(
    runner: &Runner,
    browser: &Browser,
    item: &WorkItem,
    worker_index: usize,
    control: &crate::CancellationToken,
    worker_fixtures: &mut FixtureState,
    suites: &mut SuiteState,
) -> TestResult {
    let test = &item.test;
    let mut names = test.locks.clone();
    names.sort();
    names.dedup();
    let locks: Vec<_> = {
        let mut registry = runner
            .resource_locks
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        names
            .iter()
            .map(|name| {
                Arc::clone(
                    registry
                        .entry(name.clone())
                        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))),
                )
            })
            .collect()
    };
    let mut guards = Vec::new();
    for lock in locks {
        match control.run(async { Ok(lock.lock_owned().await) }).await {
            Ok(guard) => guards.push(guard),
            Err(error) => return failed_result(&item.display_name(), error.to_string()),
        }
    }
    let started = Instant::now();
    let mut attempts = 0;
    let mut last_error = String::new();
    let mut screenshots = Vec::new();
    let mut trace_path = None;
    let mut video_path = None;
    let attachments = Arc::new(Mutex::new(Vec::new()));
    let history = Arc::new(Mutex::new(Vec::<AttemptResult>::new()));
    let name = item.display_name();
    let slug = if item.repeat_each_index == 0 {
        slug(&name)
    } else {
        format!("{}-r{}", slug(&name), item.repeat_each_index)
    };
    let mut timeout = item.timeout;
    if test.slow {
        timeout = timeout.saturating_mul(3);
    }
    let expected_fail = test.mode == TestMode::Fail;
    let mut expected_failure_observed = false;
    let mut annotations = test.annotations.clone();

    for _ in 0..=item.retries {
        if control.is_cancelled() {
            last_error = control.reason().unwrap();
            break;
        }
        attempts += 1;
        expected_failure_observed = false;
        let runtime =
            RuntimeControl::new(timeout, test.slow, expected_fail, test.annotations.clone());
        let mut info = TestInfo {
            title: test.name.clone(),
            file: test.file.clone(),
            line: test.line,
            tags: test.tags.clone(),
            retry: attempts - 1,
            worker_index,
            repeat_each_index: item.repeat_each_index,
            timeout,
            output_dir: std::path::Path::new(&runner.output_dir)
                .join(format!("{slug}-attempt{attempts}"))
                .display()
                .to_string(),
            project: item.project.clone(),
            attachments: Arc::clone(&attachments),
            steps: None,
            runtime: runtime.clone(),
            reporters: runner.reporters.clone(),
            attempt: crate::report::AttemptInfo {
                name: name.clone(),
                file: test.file.clone(),
                line: test.line,
                project: item.project.clone(),
                worker_index,
                repeat_each_index: item.repeat_each_index,
                retry: attempts - 1,
            },
        };
        info.steps = Some(crate::report::StepSession::new(
            runner.reporters.clone(),
            info.attempt.clone(),
        ));
        let mut attempt_report =
            AttemptGuard::new(runner.reporters.clone(), info.clone(), Arc::clone(&history));
        worker_fixtures.values.inner.insert(
            TypeId::of::<WorkerInfo>(),
            Arc::new(WorkerInfo {
                worker_index,
                project: item.project.clone(),
            }),
        );
        worker_fixtures
            .values
            .inner
            .insert(TypeId::of::<Browser>(), Arc::new(browser.worker_handle()));
        let deadline = crate::operation::Deadline::new(timeout);
        let automatic_worker: Vec<_> = runner
            .fixtures
            .iter()
            .filter(|def| def.automatic && def.scope == FixtureScope::Worker)
            .map(|def| def.type_id)
            .collect();
        let mut empty_attempt = FixtureState::default();
        if let Err(error) = bounded_in(
            info.steps.as_ref(),
            deadline,
            Some(control),
            "automatic worker fixture setup",
            setup_fixtures(
                &runner.fixtures,
                &automatic_worker,
                worker_fixtures,
                &mut empty_attempt,
            ),
        )
        .await
        {
            info.record_error(&error, "worker fixture setup");
            let cleanup = info
                .steps
                .as_ref()
                .unwrap()
                .scope(retire_worker_resources(
                    runner,
                    suites,
                    worker_fixtures,
                    item.project.as_deref(),
                ))
                .await;
            last_error = format!(
                "{error}{}",
                if cleanup.is_empty() {
                    String::new()
                } else {
                    format!("; worker cleanup: {}", cleanup.join("; "))
                }
            );
            if !cleanup.is_empty() {
                info.record_error(&E2eError::Config(cleanup.join("; ")), "worker cleanup");
            }
            attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
            annotations = info.annotations();
            continue;
        }
        let deadline = crate::operation::Deadline::new(timeout);
        if let Err(error) = info
            .steps
            .as_ref()
            .unwrap()
            .scope(suites.setup(
                test,
                runner,
                worker_fixtures,
                WorkerInfo {
                    worker_index,
                    project: item.project.clone(),
                },
                deadline,
                control,
            ))
            .await
        {
            info.record_error(&error, "before_all");
            last_error = error.to_string();
            if attempts <= item.retries && !control.is_cancelled() {
                let cleanup = info
                    .steps
                    .as_ref()
                    .unwrap()
                    .scope(retire_worker_resources(
                        runner,
                        suites,
                        worker_fixtures,
                        item.project.as_deref(),
                    ))
                    .await;
                if !cleanup.is_empty() {
                    info.record_error(&E2eError::Config(cleanup.join("; ")), "worker cleanup");
                    last_error.push_str(&format!("; worker cleanup: {}", cleanup.join("; ")));
                }
                attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
                annotations = info.annotations();
                continue;
            }
            attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
            annotations = info.annotations();
            break;
        }
        // Worker fixtures and beforeAll have independent setup budgets.
        // Firefox can discard a new tab when another worker closes its window.
        // Serialize lifecycle operations; bodies and hooks still run concurrently.
        let lifecycle = match bounded_in(
            info.steps.as_ref(),
            crate::operation::Deadline::new(timeout),
            Some(control),
            "context scheduling",
            runner.lifecycle_guard(browser),
        )
        .await
        {
            Ok(guard) => guard,
            Err(error) => {
                info.record_error(&error, "context scheduling");
                last_error = error.to_string();
                attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
                break;
            }
        };
        runtime.restart();
        let deadline = crate::operation::Deadline::new(timeout);
        let context = match bounded_in(
            info.steps.as_ref(),
            deadline,
            Some(control),
            "context setup",
            browser.new_context(
                item.context_options
                    .clone()
                    .unwrap_or_else(|| runner.context_options.clone()),
            ),
        )
        .await
        {
            Ok(context) => context,
            Err(error) => {
                info.record_error(&error, "context setup");
                last_error = error.to_string();
                attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
                annotations = info.annotations();
                break;
            }
        };
        let mut page = match bounded_in(
            info.steps.as_ref(),
            deadline,
            Some(control),
            "page setup",
            context.new_page(),
        )
        .await
        {
            Ok(page) => page,
            Err(error) => {
                info.record_error(&error, "page setup");
                last_error = error.to_string();
                let _ = bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "context close",
                    context.close(),
                )
                .await;
                attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
                annotations = info.annotations();
                continue;
            }
        };
        page.set_expect_timeout(runner.expect_timeout);
        drop(lifecycle);
        page.snapshot_dir = Some(
            std::env::var("FERRITE_SNAPSHOT_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::Path::new(&runner.output_dir).join("snapshots")),
        );
        page.reporter = info.steps.clone();
        let request = match crate::ApiClient::with_options(context.api_options()) {
            Ok(request) => request,
            Err(error) => {
                info.record_error(&error, "request setup");
                last_error = error.to_string();
                let _ = bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "context close",
                    context.close(),
                )
                .await;
                attempt_report.outcome(TestStatus::Failed, Some(last_error.clone()));
                annotations = info.annotations();
                break;
            }
        };
        let mut attempt_fixtures = FixtureState::default();
        attempt_fixtures
            .values
            .inner
            .insert(TypeId::of::<Page>(), Arc::new(page.clone()));
        attempt_fixtures.values.inner.insert(
            TypeId::of::<crate::BrowserContext>(),
            Arc::new(context.clone()),
        );
        attempt_fixtures
            .values
            .inner
            .insert(TypeId::of::<crate::ApiClient>(), Arc::new(request.clone()));
        attempt_fixtures
            .values
            .inner
            .insert(TypeId::of::<TestInfo>(), Arc::new(info.clone()));
        let mut recording = false;
        let mut body_started = false;

        // One budget covers beforeEach, fixture setup, recording setup and body.
        let outcome = bounded_in(
            info.steps.as_ref(),
            crate::operation::Deadline::new(Duration::ZERO),
            Some(control),
            "test setup/body",
            runtime.run(async {
                let roots: Vec<_> = runner
                    .fixtures
                    .iter()
                    .filter(|def| def.automatic)
                    .map(|def| def.type_id)
                    .collect();
                setup_fixtures(
                    &runner.fixtures,
                    &roots,
                    worker_fixtures,
                    &mut attempt_fixtures,
                )
                .await?;
                for hook in runner.before_each.iter().chain(
                    test.suites
                        .iter()
                        .flat_map(|suite| suite.before_each.iter()),
                ) {
                    crate::report::automatic(
                        None,
                        "before_each",
                        crate::StepCategory::Hook,
                        async {
                            let fixtures = setup_fixtures(
                                &runner.fixtures,
                                hook.fixtures(),
                                worker_fixtures,
                                &mut attempt_fixtures,
                            )
                            .await?;
                            hook.run(TestContext {
                                page: page.clone(),
                                context: context.clone(),
                                request: request.clone(),
                                info: info.clone(),
                                fixtures,
                            })
                            .await
                        },
                    )
                    .await?;
                }
                let fixtures = setup_fixtures(
                    &runner.fixtures,
                    &test.required_fixtures,
                    worker_fixtures,
                    &mut attempt_fixtures,
                )
                .await?;
                if runner.video.records() {
                    page.start_video(VideoOptions {
                        dir: std::path::PathBuf::from(&runner.output_dir),
                        fps: runner.video_fps,
                        ..VideoOptions::default()
                    })
                    .await?;
                    recording = true;
                }
                body_started = true;
                match &test.ctx_func {
                    Some(func) => {
                        func(TestContext {
                            page: page.clone(),
                            context: context.clone(),
                            request: request.clone(),
                            info: info.clone(),
                            fixtures,
                        })
                        .await
                    }
                    None => (test.func)(page.clone()).await,
                }
            }),
        )
        .await;
        info.body_outcome(&outcome, if body_started { "body" } else { "test setup" });
        let state = runtime.snapshot();
        let skipped = matches!(&outcome, Err(E2eError::Skipped(_)))
            || (state.skipped.is_some() && outcome.is_ok());
        let expected_fail = state.expected_fail;
        let failure_can_be_expected = matches!(&outcome, Err(error) if !matches!(error, E2eError::Timeout(..) | E2eError::Cancelled(_) | E2eError::Skipped(_)));
        let mut failed = if skipped {
            None
        } else {
            outcome.err().map(|e| e.to_string())
        };
        expected_failure_observed =
            expected_fail && body_started && failure_can_be_expected && !control.is_cancelled();
        for hook in test
            .suites
            .iter()
            .rev()
            .flat_map(|suite| suite.after_each.iter())
            .chain(runner.after_each.iter())
        {
            if let Err(error) = bounded_in(
                info.steps.as_ref(),
                crate::operation::Deadline::new(runner.cleanup_timeout),
                None,
                "after_each",
                async {
                    let fixtures = setup_fixtures(
                        &runner.fixtures,
                        hook.fixtures(),
                        worker_fixtures,
                        &mut attempt_fixtures,
                    )
                    .await?;
                    hook.run(TestContext {
                        page: page.clone(),
                        context: context.clone(),
                        request: request.clone(),
                        info: info.clone(),
                        fixtures,
                    })
                    .await
                },
            )
            .await
            {
                expected_failure_observed = false;
                info.record_error(&error, "after_each");
                let note = format!("after_each: {error}");
                failed = Some(match failed {
                    Some(prior) => format!("{prior} ({note})"),
                    None => note,
                });
            }
        }
        if let Some(note) =
            teardown_fixtures_with_info(runner, &attempt_fixtures.built, Some(&info)).await
        {
            expected_failure_observed = false;
            failed = Some(match failed {
                Some(prior) => format!("{prior} ({note})"),
                None => note,
            });
        }
        let unexpected_pass = failed.is_none() && expected_fail && !skipped;
        if unexpected_pass {
            let error = TestError {
                message: "expected to fail, but passed".into(),
                code: "unexpected_pass".into(),
                phase: "expectation".into(),
                location: Some(SourceLocation {
                    file: info.file.clone(),
                    line: info.line,
                    column: 0,
                }),
            };
            info.runtime
                .state
                .send_modify(|state| state.errors.push(error));
            failed = Some("expected to fail, but passed".to_string());
        }
        if recording {
            let keep = runner.video == VideoMode::On
                || (runner.video == VideoMode::OnlyOnFailure && failed.is_some());
            if keep {
                let path = std::path::Path::new(&runner.output_dir)
                    .join(format!("{slug}-attempt{attempts}.webm"));
                match bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "stop video",
                    page.stop_video(&path),
                )
                .await
                {
                    Ok(done) => video_path = Some(done.display().to_string()),
                    Err(error) => {
                        expected_failure_observed = false;
                        info.record_error(&error, "stop video");
                        let note = format!("stop video: {error}");
                        failed = Some(match failed {
                            Some(prior) => format!("{prior} ({note})"),
                            None => note,
                        });
                    }
                }
            } else {
                let _ = bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "cancel video",
                    async {
                        page.cancel_video().await;
                        Ok(())
                    },
                )
                .await;
            }
        }
        let take_shot =
            runner.screenshot_always || (failed.is_some() && runner.screenshot_on_failure);
        if take_shot {
            let path = std::path::Path::new(&runner.output_dir)
                .join(format!("{slug}-attempt{attempts}.png"));
            if bounded_in(
                info.steps.as_ref(),
                crate::operation::Deadline::new(runner.cleanup_timeout),
                None,
                "screenshot",
                page.save_screenshot(&path, ScreenshotOptions::default()),
            )
            .await
            .is_ok()
            {
                screenshots.push(path.display().to_string());
            }
        }
        if runner.write_trace {
            let path = std::path::Path::new(&runner.output_dir)
                .join(format!("{slug}-attempt{attempts}.json"));
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let payload = serde_json::json!({
                "test": name,
                "attempt": attempts,
                "worker": worker_index,
                "repeat": item.repeat_each_index,
                "console": page.console_messages(),
                "trace": page.trace(),
            });
            let data = serde_json::to_string_pretty(&payload).unwrap_or_default();
            if std::fs::write(&path, &data).is_ok() {
                // Preserve the original latest-attempt filename for existing consumers.
                let _ = std::fs::write(
                    std::path::Path::new(&runner.output_dir).join(format!("{slug}.json")),
                    &data,
                );
                trace_path = Some(path.display().to_string());
            }
        }
        request.dispose();
        let lifecycle = match bounded_in(
            info.steps.as_ref(),
            crate::operation::Deadline::new(runner.cleanup_timeout),
            None,
            "context cleanup scheduling",
            runner.lifecycle_guard(browser),
        )
        .await
        {
            Ok(guard) => guard,
            Err(error) => {
                expected_failure_observed = false;
                info.record_error(&error, "context cleanup scheduling");
                let note = format!("context cleanup scheduling: {error}");
                failed = Some(
                    failed
                        .map(|prior| format!("{prior}; {note}"))
                        .unwrap_or(note),
                );
                None
            }
        };
        for (label, result) in [
            (
                "page close",
                bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "page close",
                    page.close(),
                )
                .await,
            ),
            (
                "context close",
                bounded_in(
                    info.steps.as_ref(),
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "context close",
                    context.close(),
                )
                .await,
            ),
        ] {
            if let Err(error) = result {
                expected_failure_observed = false;
                info.record_error(&error, label);
                let note = format!("{label}: {error}");
                failed = Some(
                    failed
                        .map(|prior| format!("{prior}; {note}"))
                        .unwrap_or(note),
                );
            }
        }
        drop(lifecycle);
        // Retire logical worker resources after an unexpected failure, so
        // retries and subsequent tests cannot inherit failed fixture/suite state.
        if failed.is_some() && !expected_failure_observed {
            let notes = info
                .steps
                .as_ref()
                .unwrap()
                .scope(retire_worker_resources(
                    runner,
                    suites,
                    worker_fixtures,
                    item.project.as_deref(),
                ))
                .await;
            if !notes.is_empty() {
                info.record_error(&E2eError::Config(notes.join("; ")), "worker cleanup");
                failed = Some(format!(
                    "{}; worker cleanup: {}",
                    failed.unwrap(),
                    notes.join("; ")
                ));
            }
        }
        annotations = info.annotations();
        attempt_report.result.screenshots = screenshots
            .iter()
            .filter(|path| path.contains(&format!("attempt{attempts}.")))
            .cloned()
            .collect();
        attempt_report.result.trace = trace_path
            .clone()
            .filter(|path| path.ends_with(&format!("{slug}-attempt{attempts}.json")));
        attempt_report.result.video = video_path
            .clone()
            .filter(|path| path.ends_with(&format!("{slug}-attempt{attempts}.webm")));
        attempt_report.outcome(
            if failed.is_none() {
                if skipped {
                    TestStatus::Skipped
                } else {
                    TestStatus::Passed
                }
            } else if expected_failure_observed {
                TestStatus::FailedExpected
            } else {
                TestStatus::Failed
            },
            failed.clone(),
        );
        drop(attempt_report);
        // Unexpected passes fail immediately (no retry can redeem a pass).
        if unexpected_pass {
            return TestResult {
                attempt_results: attempt_history(&history),
                flaky: false,
                name: name.clone(),
                status: TestStatus::Failed,
                attempts,
                duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                error: failed,
                screenshots,
                trace: trace_path,
                video: video_path,
                project: item.project.clone(),
                repeat_each_index: item.repeat_each_index,
                annotations: annotations.clone(),
                attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
            };
        }
        match failed {
            None => {
                return TestResult {
                    attempt_results: attempt_history(&history),
                    flaky: !skipped && attempt_history(&history).iter().any(|a| !a.is_expected),
                    name: name.clone(),
                    status: if skipped {
                        TestStatus::Skipped
                    } else {
                        TestStatus::Passed
                    },
                    attempts,
                    duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    error: None,
                    screenshots,
                    trace: trace_path,
                    video: video_path,
                    project: item.project.clone(),
                    repeat_each_index: item.repeat_each_index,
                    annotations: annotations.clone(),
                    attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
                };
            }
            Some(error) => {
                last_error = error;
                if expected_failure_observed {
                    break;
                }
            }
        }
    }
    TestResult {
        attempt_results: attempt_history(&history),
        flaky: false,
        name: name.clone(),
        status: if expected_failure_observed {
            TestStatus::FailedExpected
        } else {
            TestStatus::Failed
        },
        attempts,
        duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        error: Some(last_error),
        screenshots,
        trace: trace_path,
        video: video_path,
        project: item.project.clone(),
        repeat_each_index: item.repeat_each_index,
        annotations: annotations.clone(),
        attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
    }
}

async fn teardown_fixtures(
    runner: &Runner,
    built: &[(usize, Arc<dyn Any + Send + Sync>)],
) -> Option<String> {
    teardown_fixtures_with_info(runner, built, None).await
}

async fn teardown_fixtures_with_info(
    runner: &Runner,
    built: &[(usize, Arc<dyn Any + Send + Sync>)],
    info: Option<&TestInfo>,
) -> Option<String> {
    let mut errors = Vec::new();
    for (index, value) in built.iter().rev() {
        let def = &runner.fixtures[*index];
        if let Some(teardown) = &def.teardown {
            if let Err(error) = bounded(
                crate::operation::Deadline::new(runner.cleanup_timeout),
                None,
                "fixture teardown",
                crate::report::automatic(
                    info.and_then(|i| i.steps.clone()),
                    format!("fixture teardown {}", def.name),
                    crate::StepCategory::Fixture,
                    async { teardown(Arc::clone(value)).await },
                ),
            )
            .await
            {
                if let Some(info) = info {
                    info.record_error(&error, "fixture teardown");
                }
                errors.push(format!("fixture teardown: {error}"));
            }
        }
    }
    (!errors.is_empty()).then(|| errors.join("; "))
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
            run_steps: Vec::new(),
            results: vec![TestResult {
                attempt_results: Vec::new(),
                flaky: false,
                name: "a".to_string(),
                status: TestStatus::Passed,
                attempts: 1,
                duration_ms: 5,
                error: None,
                screenshots: vec![],
                trace: None,
                video: None,
                project: None,
                repeat_each_index: 0,
                annotations: Vec::new(),
                attachments: Vec::new(),
            }],
        };
        let written = runner.write_artifacts(&report, "list,json,junit");
        assert_eq!(written.len(), 2);
        assert!(dir.join("results.json").is_file());
        assert!(dir.join("junit.xml").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn projects_resolve() {
        let all = vec![Project::new("shop"), Project::new("blog").grep("post")];
        // No filter: implicit single project when unconfigured...
        assert_eq!(resolve_projects(&[], &[]).unwrap(), vec![None]);
        // ...all projects when configured.
        let resolved = resolve_projects(&all, &[]).unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].as_ref().unwrap().name, "shop");
        // Exact-name subset.
        let resolved = resolve_projects(&all, &["blog".to_string()]).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].as_ref().unwrap().grep.as_deref(), Some("post"));
        // Unknown names fail loudly.
        let error = resolve_projects(&all, &["nope".to_string()]).unwrap_err();
        assert!(error.contains("unknown project"), "{error}");
        assert!(error.contains("shop"), "{error}");
    }

    #[test]
    fn work_items_partition_expand_and_shard() {
        let tests = vec![
            named("home renders"),
            named("auth > login").tag("fast"),
            named("flaky thing").skip(),
        ];
        let projects = vec![None];
        let (runnable, skipped) = build_work_items(
            &tests,
            None,
            None,
            None,
            &projects,
            2,
            Duration::from_secs(9),
            1,
            None,
        );
        assert_eq!(runnable.len(), 2);
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0].0.name, "flaky thing");
        assert_eq!(runnable[0].retries, 2);
        assert_eq!(runnable[0].timeout, Duration::from_secs(9));
        assert_eq!(runnable[0].repeat_each_index, 0);
        assert!(runnable[0].project.is_none());

        // Project grep + overrides + repeats.
        let projects = vec![Some(Project::new("fast").grep("fast").retries(5))];
        let (runnable, _) = build_work_items(
            &tests,
            None,
            None,
            None,
            &projects,
            0,
            Duration::from_secs(30),
            3,
            None,
        );
        assert_eq!(runnable.len(), 3);
        assert_eq!(runnable[0].project.as_deref(), Some("fast"));
        assert_eq!(runnable[0].retries, 5);
        assert_eq!(runnable[0].timeout, Duration::from_secs(30));
        assert_eq!(runnable[0].display_name(), "fast > auth > login");
        assert_eq!(runnable[2].repeat_each_index, 2);
        // Test-level overrides win over project-level ones.
        let tests = vec![named("auth > login").tag("fast").retries(1)];
        let (runnable, _) = build_work_items(
            &tests,
            None,
            None,
            None,
            &projects,
            0,
            Duration::from_secs(30),
            1,
            None,
        );
        assert_eq!(runnable[0].retries, 1);

        // One shard spans the combined, display-name-ordered list.
        let projects = vec![None];
        let (runnable, _) = build_work_items(
            &tests,
            None,
            None,
            None,
            &projects,
            0,
            Duration::from_secs(30),
            4,
            Some((2, 2)),
        );
        assert_eq!(runnable.len(), 2);
        assert_eq!(runnable[0].repeat_each_index, 1);
        assert_eq!(runnable[1].repeat_each_index, 3);
    }

    #[test]
    fn ci_env_triggers_forbid() {
        let key = "CI";
        let saved = std::env::var(key).ok();
        std::env::remove_var(key);
        assert!(!ci_truthy());
        std::env::set_var(key, "true");
        assert!(ci_truthy());
        std::env::set_var(key, "1");
        assert!(ci_truthy());
        std::env::set_var(key, "false");
        assert!(!ci_truthy());
        match saved {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn test_info_attaches_files() {
        let dir = std::env::temp_dir().join(format!("ferrite-w6-attach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let info = TestInfo {
            title: "home renders!".to_string(),
            file: "suite.rs".to_string(),
            line: 42,
            tags: Vec::new(),
            retry: 0,
            worker_index: 0,
            repeat_each_index: 0,
            timeout: Duration::from_secs(1),
            output_dir: dir.display().to_string(),
            project: None,
            attachments: Arc::new(Mutex::new(Vec::new())),
            steps: None,
            runtime: RuntimeControl::new(Duration::from_secs(1), false, false, Vec::new()),
            reporters: crate::report::ReporterHub::default(),
            attempt: crate::report::AttemptInfo {
                name: "home renders!".into(),
                file: "suite.rs".into(),
                line: 42,
                project: None,
                worker_index: 0,
                repeat_each_index: 0,
                retry: 0,
            },
        };
        let path = info.attach("console log", b"hello", "text/plain").unwrap();
        assert!(path.ends_with("home-renders-console-log.txt"), "{path}");
        assert!(std::path::Path::new(&path).is_file());
        let repeated = info.attach("console log", b"second", "text/plain").unwrap();
        assert_ne!(path, repeated);
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        let path = info.attach("data", b"{}", "application/json").unwrap();
        assert!(path.ends_with(".json"), "{path}");
        let path = info
            .attach("blob", b"x", "application/octet-stream")
            .unwrap();
        assert!(!path.ends_with(".txt"), "{path}");
        assert_eq!(info.attachments().len(), 4);
        assert_eq!(info.attachments()[0].content_type, "text/plain");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixture_map_types() {
        let map = FixtureMap::default();
        assert!(map.get::<String>().is_none());
        let mut map = FixtureMap::default();
        map.inner.insert(
            TypeId::of::<String>(),
            Arc::new("value".to_string()) as Arc<dyn Any + Send + Sync>,
        );
        assert_eq!(map.get::<String>().unwrap().as_str(), "value");
        assert!(map.get::<u32>().is_none());
    }

    #[test]
    fn fail_annotate_and_context_constructors() {
        let failing = named("boom").fail().annotate("issue", "123");
        assert_eq!(failing.mode, TestMode::Fail);
        assert_eq!(
            failing.annotations,
            vec![("issue".to_string(), "123".to_string())]
        );
        let ctx = test_with_context("with ctx", |_| async { Ok(()) });
        assert!(ctx.ctx_func.is_some());
        assert!(!ctx.file.is_empty());
        assert!(ctx.line > 0);
        assert!(named("plain").ctx_func.is_none());
    }

    #[test]
    fn artifacts_write_html() {
        let dir = std::env::temp_dir().join(format!("ferrite-e2e-rep-html-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let runner = Runner::default().output_dir(dir.display().to_string());
        let report = TestReport {
            run_steps: Vec::new(),
            results: vec![TestResult {
                attempt_results: Vec::new(),
                flaky: false,
                name: "a".to_string(),
                status: TestStatus::Failed,
                attempts: 2,
                duration_ms: 5,
                error: Some("boom".to_string()),
                screenshots: vec![],
                trace: None,
                video: None,
                project: None,
                repeat_each_index: 0,
                annotations: Vec::new(),
                attachments: Vec::new(),
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

#[cfg(test)]
mod fixture_scope_tests {
    use super::*;
    struct A;
    struct B;
    struct C;
    #[test]
    fn rejects_cycles_missing_requests_dependencies_and_scope_inversion() {
        let a = Fixture::<A>::new(|_| async { Ok(A) }).dependency::<B>();
        let b = Fixture::<B>::new(|_| async { Ok(B) }).dependency::<A>();
        let error = fixture_plan(&[a.def.clone(), b.def], &[TypeId::of::<A>()]).unwrap_err();
        assert!(error.to_string().contains("cycle"));
        assert!(fixture_plan(&[a.def], &[TypeId::of::<A>()])
            .unwrap_err()
            .to_string()
            .contains("missing dependency"));
        assert!(fixture_plan(&[], &[TypeId::of::<A>()])
            .unwrap_err()
            .to_string()
            .contains("not registered"));
        let a = Fixture::<A>::new(|_| async { Ok(A) });
        let b = Fixture::<B>::new(|_| async { Ok(B) })
            .dependency::<A>()
            .scope(FixtureScope::Worker);
        assert!(fixture_plan(&[a.def, b.def], &[TypeId::of::<B>()])
            .unwrap_err()
            .to_string()
            .contains("cannot depend"));
    }
    #[tokio::test]
    async fn partial_setup_and_dependency_teardown_survive_failure() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let a = log.clone();
        let b = log.clone();
        let runner = Runner::default()
            .fixture_definition(
                Fixture::<C>::new(|map| async move {
                    map.require::<B>()?;
                    Err(E2eError::Expect("setup failed".into()))
                })
                .dependency::<B>(),
            )
            .fixture_definition(
                Fixture::<B>::new(|map| async move {
                    map.require::<A>()?;
                    Ok(B)
                })
                .dependency::<A>()
                .teardown(move |_| {
                    let log = b.clone();
                    async move {
                        log.lock().unwrap().push("b");
                        Err(E2eError::Expect("cleanup failed".into()))
                    }
                }),
            )
            .fixture_definition(Fixture::<A>::new(|_| async { Ok(A) }).teardown(move |_| {
                let log = a.clone();
                async move {
                    log.lock().unwrap().push("a");
                    Ok(())
                }
            }));
        let mut worker = FixtureState::default();
        let mut attempt = FixtureState::default();
        assert!(setup_fixtures(
            &runner.fixtures,
            &[TypeId::of::<C>()],
            &mut worker,
            &mut attempt
        )
        .await
        .is_err());
        let error = teardown_fixtures(&runner, &attempt.built).await.unwrap();
        assert!(error.contains("cleanup failed"));
        assert_eq!(*log.lock().unwrap(), ["b", "a"]);
        assert_eq!(attempt.built.len(), 2);
    }
    #[test]
    fn nested_settings_prefer_descendants_and_focused_suites_preserve_skips() {
        let child = Suite::new("inner")
            .timeout(Duration::from_secs(2))
            .retries(3)
            .tests(vec![test("leaf", |_| async { Ok(()) })
                .context_options(ContextOptions::default().viewport(320, 240))]);
        let tests = Suite::new("outer")
            .timeout(Duration::from_secs(4))
            .retries(5)
            .context_options(ContextOptions::default().viewport(640, 480))
            .tests(child);
        assert_eq!(tests[0].timeout, Some(Duration::from_secs(2)));
        assert_eq!(tests[0].retries, Some(3));
        assert_eq!(
            tests[0]
                .context_options
                .as_ref()
                .unwrap()
                .viewport
                .unwrap()
                .width,
            320
        );
        assert_eq!(tests[0].suites.len(), 2);
        let tests = Suite::new("focused").only().tests(vec![
            test("skip", |_| async { Ok(()) }).skip(),
            test("run", |_| async { Ok(()) }),
        ]);
        assert_eq!(tests[0].mode, TestMode::Skip);
        assert_eq!(tests[1].mode, TestMode::Only);
    }
}

#[cfg(test)]
mod suite_lifecycle_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn suite_setup_timeout_and_cleanup_failures_are_bounded_and_continue() {
        let cleaned = Arc::new(AtomicUsize::new(0));
        let outer_cleanup = cleaned.clone();
        let inner = Suite::new("inner")
            .before_all(|| async { std::future::pending::<E2eResult<()>>().await })
            .after_all(|| async { std::future::pending::<E2eResult<()>>().await })
            .tests(vec![test("body", |_| async { Ok(()) })]);
        let tests = Suite::new("outer")
            .after_all(move || {
                let cleaned = outer_cleanup.clone();
                async move {
                    cleaned.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
            .tests(inner);
        let mut state = SuiteState::default();
        let token = crate::CancellationToken::new();
        let start = Instant::now();
        assert!(state
            .setup(
                &tests[0],
                &Runner::default(),
                &mut FixtureState::default(),
                WorkerInfo {
                    worker_index: 0,
                    project: None
                },
                crate::operation::Deadline::new(Duration::from_millis(20)),
                &token
            )
            .await
            .is_err());
        assert_eq!(state.started.len(), 2);
        let results = state
            .cleanup(
                &Runner::default().cleanup_timeout(Duration::from_millis(20)),
                &mut FixtureState::default(),
                None,
            )
            .await;
        assert_eq!(results.len(), 1);
        assert!(results[0].name.contains("inner"));
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(state.started.is_empty());
        assert!(start.elapsed() < Duration::from_secs(1));
    }
    #[tokio::test]
    async fn worker_cleanup_timeout_does_not_skip_dependencies_or_keep_cached_values() {
        let cleaned = Arc::new(AtomicUsize::new(0));
        let count = cleaned.clone();
        let runner = Runner::default()
            .cleanup_timeout(Duration::from_millis(20))
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(1) })
                    .scope(FixtureScope::Worker)
                    .teardown(move |_| {
                        let count = count.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }
                    }),
            )
            .fixture_definition(
                Fixture::<u64>::new(|map| async move {
                    map.require::<u32>()?;
                    Ok(2)
                })
                .scope(FixtureScope::Worker)
                .dependency::<u32>()
                .teardown(|_| async { std::future::pending::<E2eResult<()>>().await }),
            );
        let mut worker = FixtureState::default();
        let mut attempt = FixtureState::default();
        setup_fixtures(
            &runner.fixtures,
            &[TypeId::of::<u64>()],
            &mut worker,
            &mut attempt,
        )
        .await
        .unwrap();
        let errors =
            retire_worker_resources(&runner, &mut SuiteState::default(), &mut worker, None).await;
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("timed out"));
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(worker.values.inner.is_empty());
        assert!(worker.built.is_empty());
    }
}

#[cfg(test)]
mod fixture_cancellation_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[tokio::test]
    async fn cancellation_reclaims_completed_worker_dependencies_after_partial_setup() {
        let ready = Arc::new(tokio::sync::Notify::new());
        let setup_ready = ready.clone();
        let cleaned = Arc::new(AtomicUsize::new(0));
        let count = cleaned.clone();
        let runner = Runner::default()
            .fixture_definition(
                Fixture::<u32>::new(|_| async { Ok(1) })
                    .scope(FixtureScope::Worker)
                    .teardown(move |_| {
                        let count = count.clone();
                        async move {
                            count.fetch_add(1, Ordering::SeqCst);
                            Ok(())
                        }
                    }),
            )
            .fixture_definition(
                Fixture::<u64>::new(move |map| {
                    let ready = setup_ready.clone();
                    async move {
                        map.require::<u32>()?;
                        ready.notify_one();
                        std::future::pending::<E2eResult<u64>>().await
                    }
                })
                .dependency::<u32>()
                .scope(FixtureScope::Worker),
            );
        let mut worker = FixtureState::default();
        let mut attempt = FixtureState::default();
        let token = crate::CancellationToken::new();
        let roots = [TypeId::of::<u64>()];
        let (result, ()) = tokio::join!(
            token.run(setup_fixtures(
                &runner.fixtures,
                &roots,
                &mut worker,
                &mut attempt
            )),
            async {
                ready.notified().await;
                token.cancel();
            }
        );
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        assert_eq!(worker.built.len(), 1);
        assert!(
            retire_worker_resources(&runner, &mut SuiteState::default(), &mut worker, None)
                .await
                .is_empty()
        );
        assert_eq!(cleaned.load(Ordering::SeqCst), 1);
        assert!(worker.built.is_empty());
    }
    fn runtime_info(timeout: Duration) -> TestInfo {
        TestInfo {
            title: "runtime".into(),
            file: "runtime.rs".into(),
            line: 1,
            tags: Vec::new(),
            retry: 0,
            worker_index: 0,
            repeat_each_index: 0,
            timeout,
            output_dir: String::new(),
            project: None,
            attachments: Arc::default(),
            steps: None,
            runtime: RuntimeControl::new(timeout, false, false, Vec::new()),
            reporters: crate::report::ReporterHub::default(),
            attempt: crate::report::AttemptInfo {
                name: "runtime".into(),
                file: "runtime.rs".into(),
                line: 1,
                project: None,
                worker_index: 0,
                repeat_each_index: 0,
                retry: 0,
            },
        }
    }
    #[test]
    fn outcome_metadata_is_shared_and_cleanup_skip_cannot_erase_failure() {
        let info = runtime_info(Duration::from_secs(1));
        let cleanup = info.clone();
        assert_eq!(cleanup.status(), None);
        info.body_outcome(&Ok(()), "body");
        assert_eq!(cleanup.status(), Some(AttemptStatus::Passed));
        info.record_error(&E2eError::Skipped("cleanup skip".into()), "after_each");
        assert_eq!(cleanup.status(), Some(AttemptStatus::Failed));
        assert_eq!(cleanup.errors()[0].code, "FERRITE_E2E_SKIPPED");
        let mut snapshot = cleanup.errors();
        snapshot.clear();
        assert_eq!(info.errors().len(), 1);
        info.record_error(&E2eError::Timeout(10, "fixture".into()), "fixture teardown");
        info.record_error(&E2eError::Expect("later".into()), "fixture teardown");
        assert_eq!(cleanup.status(), Some(AttemptStatus::TimedOut));
        assert_eq!(cleanup.errors().len(), 3);
    }

    #[tokio::test]
    async fn runtime_timeout_updates_wake_active_waits_and_include_elapsed_time() {
        let info = runtime_info(Duration::from_millis(60));
        let change = info.clone();
        info.runtime
            .run(async move {
                change.set_timeout(Duration::from_millis(400));
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(())
            })
            .await
            .unwrap();
        let info = runtime_info(Duration::from_secs(10));
        let change = info.clone();
        let update = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            change.set_timeout(Duration::from_millis(20));
        });
        let started = Instant::now();
        assert!(matches!(
            info.runtime
                .run(std::future::pending::<E2eResult<()>>())
                .await,
            Err(E2eError::Timeout(20, _))
        ));
        update.await.unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        let info = runtime_info(Duration::from_millis(20));
        let change = info.clone();
        info.runtime
            .run(async move {
                change.set_timeout(Duration::ZERO);
                tokio::time::sleep(Duration::from_millis(80)).await;
                Ok(())
            })
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn runtime_slow_is_idempotent_and_skip_interrupts_pending_body() {
        let info = runtime_info(Duration::from_millis(60));
        let change = info.clone();
        info.runtime
            .run(async move {
                change.slow("slow machine");
                change.slow("again");
                assert_eq!(change.effective_timeout(), Duration::from_millis(180));
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(info.annotations().len(), 1);
        let change = info.clone();
        let update = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = change.skip("unsupported");
        });
        assert!(
            matches!(info.runtime.run(std::future::pending::<E2eResult<()>>()).await, Err(E2eError::Skipped(reason)) if reason == "unsupported")
        );
        update.await.unwrap();
    }
    #[test]
    fn worker_fixture_cannot_depend_on_test_builtins() {
        let def = Fixture::<String>::new(|_| async { Ok(String::new()) })
            .dependency::<Page>()
            .scope(FixtureScope::Worker);
        assert!(fixture_plan(&[def.def], &[TypeId::of::<String>()])
            .unwrap_err()
            .to_string()
            .contains("test-scoped built-in"));
    }
}
