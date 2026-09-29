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
use crate::report::{Attachment, TestReport, TestResult, TestStatus};
use crate::video::{VideoMode, VideoOptions};
use crate::ContextOptions;

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
    before_each: Vec<HookFn>,
    after_each: Vec<HookFn>,
    before_all: Vec<GlobalHook>,
    after_all: Vec<GlobalHook>,
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
            .push(Arc::new(move |page| Box::pin(hook(page))));
        self
    }
    /// Run after each descendant attempt, inner suites first.
    pub fn after_each<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn(Page) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_each
            .push(Arc::new(move |page| Box::pin(hook(page))));
        self
    }
    /// Run once before this worker executes descendants in a project.
    pub fn before_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.before_all.push(Arc::new(move || Box::pin(hook())));
        self
    }
    /// Run once at worker cleanup, inner suites first, even after setup failure.
    pub fn after_all<F, Fut>(mut self, hook: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = E2eResult<()>> + Send + 'static,
    {
        self.after_all.push(Arc::new(move || Box::pin(hook())));
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
    /// Effective per-attempt timeout.
    pub timeout: Duration,
    /// Artifact directory.
    pub output_dir: String,
    /// Project name, if any.
    pub project: Option<String>,
    /// Attachments shared across attempts.
    attachments: Arc<Mutex<Vec<Attachment>>>,
}

impl TestInfo {
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
        let file = format!(
            "{}-{}{}",
            slug(&self.title),
            slug(name),
            attach_extension(content_type)
        );
        let path = dir.join(file);
        std::fs::write(&path, body)?;
        let path = path.display().to_string();
        self.attachments
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(Attachment {
                name: name.to_string(),
                path: path.clone(),
                content_type: content_type.to_string(),
            });
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

#[derive(Default)]
struct SuiteState {
    started: Vec<(Arc<Suite>, Option<String>)>,
}

impl SuiteState {
    async fn setup(
        &mut self,
        test: &Test,
        deadline: crate::operation::Deadline,
        control: &crate::CancellationToken,
    ) -> E2eResult<()> {
        for suite in &test.suites {
            if let Some((_, error)) = self
                .started
                .iter()
                .find(|(seen, _)| Arc::ptr_eq(seen, suite))
            {
                if let Some(error) = error {
                    return Err(E2eError::Config(error.clone()));
                }
                continue;
            }
            self.started.push((suite.clone(), None));
            for hook in &suite.before_all {
                if let Err(error) = bounded(deadline, Some(control), "suite before_all", async {
                    hook().await
                })
                .await
                {
                    let error = format!("suite {} before_all: {error}", suite.name);
                    self.started.last_mut().unwrap().1 = Some(error.clone());
                    return Err(E2eError::Config(error));
                }
            }
        }
        Ok(())
    }
    async fn cleanup(&mut self, runner: &Runner, project: Option<&str>) -> Vec<TestResult> {
        self.cleanup_finished(runner, project, &[]).await
    }

    async fn cleanup_finished(
        &mut self,
        runner: &Runner,
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
                    async { hook().await },
                )
                .await
                {
                    results.push(failed_result(
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
        .cleanup(runner, project)
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
        let value = (def.setup)(dependencies)
            .await
            .map_err(|error| E2eError::Config(format!("fixture {} setup: {error}", def.name)))?;
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
    context_options: ContextOptions,
    expect_timeout: Duration,
    reporter: String,
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
            context_options: ContextOptions {
                viewport: config.viewport.as_ref().map(|v| crate::Viewport {
                    width: v.width,
                    height: v.height,
                }),
                ..ContextOptions::default()
            },
            expect_timeout: Duration::from_millis(config.expect_timeout_ms),
            reporter: config.reporter.clone(),
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
                    report.results.push(failed_result(label, error.to_string()));
                }
            }
        }
        report.results.sort_by(|a, b| a.name.cmp(&b.name));
        self.write_artifacts(&report, &self.reporter);
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
        let validation = if self.fixtures.iter().any(|def| !seen.insert(def.type_id)) {
            Err(E2eError::Config(
                "duplicate fixture type registration".into(),
            ))
        } else {
            fixture_plan(&self.fixtures, &roots).and_then(|_| {
                for test in &tests {
                    fixture_plan(&self.fixtures, &test.required_fixtures)?;
                }
                Ok(())
            })
        };
        if let Err(error) = validation {
            report
                .results
                .push(failed_result("<fixtures>", error.to_string()));
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
                    .push(failed_result("<global setup>", error.to_string()));
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
                report.results.push(failed_result("<project>", error));
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
            report.results.push(failed_result(
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
                    .push(failed_result("<before_all>", error.to_string()));
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
                        report.results.push(failed_result(
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
            workers.spawn(async move {
                let mut results = Vec::new();
                let mut fixture_states: HashMap<Option<String>, FixtureState> = HashMap::new();
                let mut suite_states: HashMap<Option<String>, SuiteState> = HashMap::new();
                loop {
                    let item = queue
                        .lock()
                        .map(|mut q| {
                            if control.is_cancelled()
                                || (runner.max_failures > 0
                                    && failures.load(std::sync::atomic::Ordering::SeqCst)
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
                                failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                let errors = retire_worker_resources(
                                    &runner,
                                    suite_states.entry(item.project.clone()).or_default(),
                                    fixture_states.entry(item.project.clone()).or_default(),
                                    item.project.as_deref(),
                                )
                                .await;
                                if !errors.is_empty() {
                                    results.push(failed_result(
                                        &display_name(item.project.as_deref(), "<worker cleanup>"),
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
                                .cleanup_finished(&runner, item.project.as_deref(), &remaining)
                                .await;
                            failures.fetch_add(cleanup.len(), std::sync::atomic::Ordering::SeqCst);
                            if !cleanup.is_empty() {
                                let errors = retire_worker_resources(
                                    &runner,
                                    suite_states.entry(item.project.clone()).or_default(),
                                    fixture_states.entry(item.project.clone()).or_default(),
                                    item.project.as_deref(),
                                )
                                .await;
                                if !errors.is_empty() {
                                    failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    results.push(failed_result(
                                        &display_name(item.project.as_deref(), "<worker cleanup>"),
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
                    results.extend(state.cleanup(&runner, project.as_deref()).await);
                }
                for (project, state) in &fixture_states {
                    if let Some(error) = teardown_fixtures(&runner, &state.built).await {
                        results.push(failed_result(
                            &display_name(project.as_deref(), "<worker fixtures>"),
                            error,
                        ));
                    }
                }
                results
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
                    .push(failed_result("<join>", error.to_string())),
            }
        }
        if let Ok(mut pending) = queue.lock() {
            for item in pending.drain(..) {
                let mut result = failed_result(
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
                .push(failed_result("<run interrupted>", reason));
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
                        .push(failed_result("<browser close>", error.to_string()));
                }
            }
        }
        self.finish_run(report).await
    }

    /// Write file artifacts for the reporters in `spec` (comma-separated).
    ///
    /// `json`, `junit`, and `html` write files; `list` and `dot` are printed
    /// by the caller via [`TestReport::to_list`] / [`TestReport::to_dot`].
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

struct AbortTask(tokio::task::JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn bounded<T>(
    deadline: crate::operation::Deadline,
    token: Option<&crate::CancellationToken>,
    label: &str,
    future: impl Future<Output = E2eResult<T>>,
) -> E2eResult<T> {
    let work = async {
        match std::panic::AssertUnwindSafe(future).catch_unwind().await {
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

    for _ in 0..=item.retries {
        if control.is_cancelled() {
            last_error = control.reason().unwrap();
            break;
        }
        attempts += 1;
        expected_failure_observed = false;
        let deadline = crate::operation::Deadline::new(timeout);
        let automatic_worker: Vec<_> = runner
            .fixtures
            .iter()
            .filter(|def| def.automatic && def.scope == FixtureScope::Worker)
            .map(|def| def.type_id)
            .collect();
        let mut empty_attempt = FixtureState::default();
        if let Err(error) = bounded(
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
            let cleanup =
                retire_worker_resources(runner, suites, worker_fixtures, item.project.as_deref())
                    .await;
            last_error = format!(
                "{error}{}",
                if cleanup.is_empty() {
                    String::new()
                } else {
                    format!("; worker cleanup: {}", cleanup.join("; "))
                }
            );
            continue;
        }
        let deadline = crate::operation::Deadline::new(timeout);
        if let Err(error) = suites.setup(test, deadline, control).await {
            last_error = error.to_string();
            if attempts <= item.retries && !control.is_cancelled() {
                let cleanup = retire_worker_resources(
                    runner,
                    suites,
                    worker_fixtures,
                    item.project.as_deref(),
                )
                .await;
                if !cleanup.is_empty() {
                    last_error.push_str(&format!("; worker cleanup: {}", cleanup.join("; ")));
                }
                continue;
            }
            break;
        }
        let deadline = crate::operation::Deadline::new(timeout);
        let context = match bounded(
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
                last_error = error.to_string();
                break;
            }
        };
        let mut page =
            match bounded(deadline, Some(control), "page setup", context.new_page()).await {
                Ok(page) => page,
                Err(error) => {
                    last_error = error.to_string();
                    let _ = bounded(
                        crate::operation::Deadline::new(runner.cleanup_timeout),
                        None,
                        "context close",
                        context.close(),
                    )
                    .await;
                    continue;
                }
            };
        page.set_expect_timeout(runner.expect_timeout);
        page.snapshot_dir = Some(
            std::env::var("FERRITE_SNAPSHOT_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| std::path::Path::new(&runner.output_dir).join("snapshots")),
        );
        let mut attempt_fixtures = FixtureState::default();
        let mut recording = false;
        let mut body_started = false;
        let info = TestInfo {
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
        };
        // One budget covers beforeEach, fixture setup, recording setup and body.
        let outcome = bounded(deadline, Some(control), "test setup/body", async {
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
                hook(page.clone())
                    .await
                    .map_err(|e| E2eError::Config(format!("before_each: {e}")))?;
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
                        info,
                        fixtures,
                    })
                    .await
                }
                None => (test.func)(page.clone()).await,
            }
        })
        .await;
        let mut failed = outcome.err().map(|e| e.to_string());
        expected_failure_observed =
            expected_fail && body_started && failed.is_some() && !control.is_cancelled();
        for hook in test
            .suites
            .iter()
            .rev()
            .flat_map(|suite| suite.after_each.iter())
            .chain(runner.after_each.iter())
        {
            if let Err(error) = bounded(
                crate::operation::Deadline::new(runner.cleanup_timeout),
                None,
                "after_each",
                async { hook(page.clone()).await },
            )
            .await
            {
                expected_failure_observed = false;
                let note = format!("after_each: {error}");
                failed = Some(match failed {
                    Some(prior) => format!("{prior} ({note})"),
                    None => note,
                });
            }
        }
        if let Some(note) = teardown_fixtures(runner, &attempt_fixtures.built).await {
            expected_failure_observed = false;
            failed = Some(match failed {
                Some(prior) => format!("{prior} ({note})"),
                None => note,
            });
        }
        let unexpected_pass = failed.is_none() && expected_fail;
        if unexpected_pass {
            failed = Some("expected to fail, but passed".to_string());
        }
        if recording {
            let keep = runner.video == VideoMode::On
                || (runner.video == VideoMode::OnlyOnFailure && failed.is_some());
            if keep {
                let path = std::path::Path::new(&runner.output_dir)
                    .join(format!("{slug}-attempt{attempts}.webm"));
                match bounded(
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "stop video",
                    page.stop_video(&path),
                )
                .await
                {
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
                let _ = bounded(
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
            if bounded(
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
            let path = std::path::Path::new(&runner.output_dir).join(format!("{slug}.json"));
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
            if std::fs::write(
                &path,
                serde_json::to_string_pretty(&payload).unwrap_or_default(),
            )
            .is_ok()
            {
                trace_path = Some(path.display().to_string());
            }
        }
        for (label, result) in [
            (
                "page close",
                bounded(
                    crate::operation::Deadline::new(runner.cleanup_timeout),
                    None,
                    "page close",
                    page.close(),
                )
                .await,
            ),
            (
                "context close",
                bounded(
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
                let note = format!("{label}: {error}");
                failed = Some(
                    failed
                        .map(|prior| format!("{prior}; {note}"))
                        .unwrap_or(note),
                );
            }
        }
        // Retire logical worker resources after an unexpected failure, so
        // retries and subsequent tests cannot inherit failed fixture/suite state.
        if failed.is_some() && !expected_failure_observed {
            let notes =
                retire_worker_resources(runner, suites, worker_fixtures, item.project.as_deref())
                    .await;
            if !notes.is_empty() {
                failed = Some(format!(
                    "{}; worker cleanup: {}",
                    failed.unwrap(),
                    notes.join("; ")
                ));
            }
        }
        // Unexpected passes fail immediately (no retry can redeem a pass).
        if unexpected_pass {
            return TestResult {
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
                annotations: test.annotations.clone(),
                attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
            };
        }
        match failed {
            None => {
                return TestResult {
                    name: name.clone(),
                    status: TestStatus::Passed,
                    attempts,
                    duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    error: None,
                    screenshots,
                    trace: trace_path,
                    video: video_path,
                    project: item.project.clone(),
                    repeat_each_index: item.repeat_each_index,
                    annotations: test.annotations.clone(),
                    attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
                };
            }
            Some(error) => last_error = error,
        }
    }
    TestResult {
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
        annotations: test.annotations.clone(),
        attachments: attachments.lock().map(|a| a.clone()).unwrap_or_default(),
    }
}

async fn teardown_fixtures(
    runner: &Runner,
    built: &[(usize, Arc<dyn Any + Send + Sync>)],
) -> Option<String> {
    let mut errors = Vec::new();
    for (index, value) in built.iter().rev() {
        let def = &runner.fixtures[*index];
        if let Some(teardown) = &def.teardown {
            if let Err(error) = bounded(
                crate::operation::Deadline::new(runner.cleanup_timeout),
                None,
                "fixture teardown",
                async { teardown(Arc::clone(value)).await },
            )
            .await
            {
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
            results: vec![TestResult {
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
        };
        let path = info.attach("console log", b"hello", "text/plain").unwrap();
        assert!(path.ends_with("home-renders-console-log.txt"), "{path}");
        assert!(std::path::Path::new(&path).is_file());
        let path = info.attach("data", b"{}", "application/json").unwrap();
        assert!(path.ends_with(".json"), "{path}");
        let path = info
            .attach("blob", b"x", "application/octet-stream")
            .unwrap();
        assert!(!path.ends_with(".txt"), "{path}");
        assert_eq!(info.attachments().len(), 3);
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
            results: vec![TestResult {
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
                crate::operation::Deadline::new(Duration::from_millis(20)),
                &token
            )
            .await
            .is_err());
        assert_eq!(state.started.len(), 2);
        let results = state
            .cleanup(
                &Runner::default().cleanup_timeout(Duration::from_millis(20)),
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
}
