//! Auto-retrying assertions (`expect_*`), Playwright-style.

use std::future::Future;
use std::time::Duration;

use serde::Serialize;

use crate::error::{E2eError, E2eResult};
use crate::locator::Locator;
use crate::page::{Page, ScreenshotOptions};
use crate::snapshot::{resolve_update, snap_path_for, SnapshotOptions, SnapshotUpdate};

/// Collect assertion failures without stopping at the first one
/// outside the runner: feed each assertion result to
/// [`SoftAsserts::check`], then fail once via [`SoftAsserts::assert_all`].
/// Use [`SoftAsserts::for_attempt`] for automatic runner-owned failure reporting.
#[derive(Debug, Default)]
pub struct SoftAsserts {
    failures: Vec<String>,
}

impl SoftAsserts {
    /// Empty collector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A collector bound to this attempt; failures automatically affect its result.
    #[must_use]
    pub fn for_attempt(info: &crate::TestInfo) -> crate::AttemptSoftAsserts {
        info.soft_asserts()
    }

    /// Record a standalone result with an optional contextual message.
    pub fn check_with_message(&mut self, result: E2eResult<()>, message: impl Into<String>) {
        if let Err(error) = result {
            let message = message.into();
            self.failures.push(if message.is_empty() {
                error.to_string()
            } else {
                format!("{message}: {error}")
            });
        }
    }

    /// Record an assertion result, keeping its failure message.
    pub fn check(&mut self, result: E2eResult<()>) {
        if let Err(error) = result {
            self.failures.push(error.to_string());
        }
    }

    /// Collected failure messages.
    #[must_use]
    pub fn failures(&self) -> &[String] {
        &self.failures
    }

    /// Fail with all collected messages (ok when empty).
    pub fn assert_all(self) -> E2eResult<()> {
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(E2eError::Expect(format!(
                "{} soft assertion(s) failed:\n{}",
                self.failures.len(),
                self.failures.join("\n")
            )))
        }
    }
}

/// Assertion retry window.
#[derive(Debug, Clone, Copy)]
pub struct Timeout(Duration);

impl Timeout {
    /// Retry for `ms` milliseconds.
    #[must_use]
    pub fn ms(ms: u64) -> Self {
        Self(Duration::from_millis(ms))
    }

    /// Retry for `secs` seconds.
    #[must_use]
    pub fn secs(secs: u64) -> Self {
        Self(Duration::from_secs(secs))
    }

    /// The window as a duration.
    #[must_use]
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for Timeout {
    fn default() -> Self {
        Self(Duration::from_secs(5))
    }
}

impl From<Duration> for Timeout {
    fn from(duration: Duration) -> Self {
        Self(duration)
    }
}

/// Controls for generic assertion polling. Probes run immediately, then wait
/// through the interval sequence; its final interval repeats until completion.
/// Zero timeout disables the local limit, but enclosing budgets still apply.
#[derive(Debug, Clone)]
pub struct PollingOptions {
    pub timeout: Duration,
    /// Must be nonempty and contain only positive durations.
    pub intervals: Vec<Duration>,
    pub message: Option<String>,
    pub cancellation: Option<crate::CancellationToken>,
}
impl Default for PollingOptions {
    fn default() -> Self {
        Self {
            timeout: Timeout::default().duration(),
            intervals: vec![Duration::from_millis(50)],
            message: None,
            cancellation: None,
        }
    }
}
impl PollingOptions {
    pub fn timeout(mut self, timeout: impl Into<Timeout>) -> Self {
        self.timeout = timeout.into().duration();
        self
    }
    pub fn intervals(mut self, intervals: impl IntoIterator<Item = Duration>) -> Self {
        self.intervals = intervals.into_iter().collect();
        self
    }
    pub fn message(mut self, message: impl Into<String>) -> Self {
        let message = message.into();
        self.message = (!message.is_empty()).then_some(message);
        self
    }
    pub fn cancellation(mut self, token: crate::CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
    fn validate(&self) -> E2eResult<()> {
        if self.intervals.is_empty() || self.intervals.iter().any(Duration::is_zero) {
            return Err(E2eError::Config(
                "polling intervals must be nonempty and positive".into(),
            ));
        }
        Ok(())
    }
}

async fn poll_value<T, F, Fut>(
    description: String,
    options: &PollingOptions,
    mut check: F,
) -> E2eResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Option<T>>>,
{
    let context = options
        .message
        .as_deref()
        .filter(|message| !message.is_empty())
        .map(|message| format!("{message}: {description}"))
        .unwrap_or(description);
    crate::report::automatic(
        None,
        format!("expect {context}"),
        crate::StepCategory::Assertion,
        async {
            options
                .validate()
                .map_err(|error| error.with_context(&context))?;
            let deadline = crate::operation::Deadline::new(options.timeout);
            let mut last = None;
            let mut last_images = None;
            // Keep probe failures inside a successful outer result. This separates
            // their Timeout errors from expiration of our one polling deadline.
            let work = deadline.run(context.clone(), async {
                let mut interval = 0;
                loop {
                    if deadline.expired() {
                        std::future::pending::<()>().await;
                    }
                    let (probe, images) =
                        crate::snapshot_artifacts::retry_probe(async { check().await }).await;
                    match probe {
                        Ok(Some(value)) => return Ok(Ok(value)),
                        Ok(None) => {
                            last = Some("pending".to_string());
                            last_images = None;
                        }
                        Err(error) if error.code() == "FERRITE_E2E_EXPECT" => {
                            last = Some(error.to_string());
                            last_images = images;
                        }
                        Err(error) => return Ok(Err(error)),
                    }
                    tokio::time::sleep(options.intervals[interval]).await;
                    interval = (interval + 1).min(options.intervals.len() - 1);
                }
            });
            let result = match &options.cancellation {
                Some(token) => token.run(work).await,
                None => work.await,
            };
            let result = match result {
                Ok(result) => result.map_err(|error| error.with_context(&context)),
                Err(E2eError::Timeout(..)) => Err(E2eError::Expect(format!(
                    "{context} (last: {})",
                    last.as_deref().unwrap_or("no data yet")
                ))),
                Err(error) => Err(error.with_context(&context)),
            };
            let finish = crate::snapshot_artifacts::finish(result, last_images);
            match &options.cancellation {
                Some(token) => token.run(finish).await,
                None => finish.await,
            }
        },
    )
    .await
}

/// Default assertion window in milliseconds.
pub(crate) const DEFAULT_EXPECT_MS: u64 = 5_000;

pub(crate) async fn poll<F, Fut>(timeout: Duration, description: String, check: F) -> E2eResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Option<String>>>,
{
    crate::report::automatic(
        None,
        format!("expect {description}"),
        crate::StepCategory::Assertion,
        poll_raw(timeout, description, check),
    )
    .await
}

async fn poll_raw<F, Fut>(timeout: Duration, description: String, mut check: F) -> E2eResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Option<String>>>,
{
    let mut last = None;
    let result = crate::operation::Deadline::new(timeout)
        .run(description.clone(), async {
            loop {
                match check().await {
                    Ok(None) => return Ok(()),
                    Ok(Some(mismatch)) => last = Some(mismatch),
                    Err(error @ (E2eError::Cancelled(_) | E2eError::Disconnected(_))) => {
                        return Err(error)
                    }
                    Err(error) => last = Some(error.to_string()),
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
    match result {
        Err(E2eError::Timeout(..)) => Err(E2eError::Expect(format!(
            "{description} (last: {})",
            last.as_deref().unwrap_or("no data yet")
        ))),
        result => result,
    }
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Description suffix for negated assertions.
fn not_tag(negated: bool) -> &'static str {
    if negated {
        " (not)"
    } else {
        ""
    }
}

/// Poll `check` until it returns `Some(value)` or the timeout hits.
///
/// `None` and assertion mismatches are retried every 50 ms; operational/control
/// errors propagate. Fails with `E2eError::Expect` when its polling window expires.
pub async fn expect_poll<T, F, Fut>(
    description: impl Into<String>,
    timeout: impl Into<Timeout>,
    check: F,
) -> E2eResult<T>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = E2eResult<Option<T>>> + Send,
    T: Send + 'static,
{
    expect_poll_with(
        description,
        &PollingOptions::default().timeout(timeout),
        check,
    )
    .await
}

/// Poll a value with custom intervals, contextual text and cancellation.
/// Only None/assertion mismatches retry. Supports borrowed, mutable and non-Send
/// probes/values; legacy expect_poll retains its original bounds and signature.
pub async fn expect_poll_with<T, F, Fut>(
    description: impl Into<String>,
    options: &PollingOptions,
    check: F,
) -> E2eResult<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Option<T>>>,
{
    poll_value(description.into(), options, check).await
}

/// Retry an arbitrary asynchronous assertion block until it succeeds.
pub async fn expect_to_pass<F, Fut>(
    description: impl Into<String>,
    timeout: impl Into<Timeout>,
    check: F,
) -> E2eResult<()>
where
    F: Fn() -> Fut,
    Fut: Future<Output = E2eResult<()>>,
{
    expect_to_pass_with(
        description,
        &PollingOptions::default().timeout(timeout),
        check,
    )
    .await
}

/// Retry an assertion block with custom polling controls. Intermediate soft
/// checks return their mismatch to retry; wrap this entire helper in soft.run
/// to collect only its final mismatch. Operational/control errors never soften.
/// Final screenshot diagnostics have one additional five-second file/rendering
/// budget, subject to caller cancellation and enclosing step/test deadlines.
pub async fn expect_to_pass_with<F, Fut>(
    description: impl Into<String>,
    options: &PollingOptions,
    mut check: F,
) -> E2eResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<()>>,
{
    poll_value(description.into(), options, || {
        let future = check();
        async move { future.await.map(|()| Some(())) }
    })
    .await
}

/// Page-level assertions.
pub struct PageExpect {
    page: Page,
    timeout: Duration,
    negated: bool,
}

impl PageExpect {
    pub(crate) fn new(page: Page) -> Self {
        Self {
            timeout: page.expect_timeout(),
            page,
            negated: false,
        }
    }

    /// Override the retry window.
    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<Timeout>) -> Self {
        self.timeout = timeout.into().duration();
        self
    }

    /// Negate the assertion (`not().not()` cancels out).
    #[must_use]
    #[allow(clippy::should_implement_trait)] // Playwright names it `not`.
    pub fn not(mut self) -> Self {
        self.negated = !self.negated;
        self
    }

    /// Assert the title with a Rust regular expression.
    pub async fn title_matches(&self, pattern: &str) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.title_matches",
                crate::StepCategory::Assertion,
                async {
                    let pattern =
                        regex::Regex::new(pattern).map_err(|e| E2eError::Config(e.to_string()))?;
                    poll(self.timeout, "title regex".into(), || async {
                        let actual = self.page.title().await?;
                        Ok(if pattern.is_match(&actual) != self.negated {
                            None
                        } else {
                            Some(actual)
                        })
                    })
                    .await
                },
            )
            .await
    }

    /// Assert using the shared exact/glob/regex/contains matcher.
    pub async fn url_matching(&self, matcher: &crate::UrlMatcher) -> E2eResult<()> {
        let matcher = matcher.resolved(|url| self.page.resolve_url(url))?;
        self.url_where(move |url| matcher.matches(url)).await
    }

    /// Retry a URL predicate; respects assertion negation and cancellation.
    pub async fn url_where<F>(&self, predicate: F) -> E2eResult<()>
    where
        F: Fn(&str) -> bool + Send + Sync,
    {
        self.page
            .auto_step(
                "expect.url_matching",
                crate::StepCategory::Assertion,
                async {
                    let scoped = self.page.with_timeout(self.timeout);
                    scoped
                        .run_operation(poll(
                            self.timeout,
                            "URL matcher/predicate".into(),
                            || async {
                                scoped.run_locator_handlers().await?;
                                let actual = scoped.url().await?;
                                Ok(if predicate(&actual) != self.negated {
                                    None
                                } else {
                                    Some(format!("URL was {actual}"))
                                })
                            },
                        ))
                        .await
                },
            )
            .await
    }

    /// Assert the URL with a Rust regular expression.
    pub async fn url_matches(&self, pattern: &str) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.url_matches",
                crate::StepCategory::Assertion,
                async {
                    let pattern =
                        regex::Regex::new(pattern).map_err(|e| E2eError::Config(e.to_string()))?;
                    poll(self.timeout, "URL regex".into(), || async {
                        let actual = self.page.url().await?;
                        Ok(if pattern.is_match(&actual) != self.negated {
                            None
                        } else {
                            Some(actual)
                        })
                    })
                    .await
                },
            )
            .await
    }

    /// Assert the exact title.
    pub async fn title(&self, expected: &str) -> E2eResult<()> {
        self.page
            .auto_step("expect.title", crate::StepCategory::Assertion, async {
                let page = self.page.clone();
                let expected = expected.to_string();
                let negated = self.negated;
                poll(
                    self.timeout,
                    format!("title == {expected:?}{}", not_tag(negated)),
                    || {
                        let page = page.clone();
                        let expected = expected.clone();
                        async move {
                            let title = match page.title().await {
                                Ok(title) => title,
                                Err(error) => return Ok(Some(error.to_string())),
                            };
                            if (title == expected) != negated {
                                Ok(None)
                            } else {
                                Ok(Some(format!("title was {title:?}")))
                            }
                        }
                    },
                )
                .await
            })
            .await
    }

    /// Assert the title contains a fragment.
    pub async fn title_contains(&self, fragment: &str) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.title_contains",
                crate::StepCategory::Assertion,
                async {
                    let page = self.page.clone();
                    let fragment = fragment.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("title contains {fragment:?}{}", not_tag(negated)),
                        || {
                            let page = page.clone();
                            let fragment = fragment.clone();
                            async move {
                                let title = match page.title().await {
                                    Ok(title) => title,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if title.contains(&fragment) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("title was {title:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the exact URL, resolving relative expectations against base_url.
    pub async fn url(&self, expected: &str) -> E2eResult<()> {
        self.page
            .auto_step("expect.url", crate::StepCategory::Assertion, async {
                let expected = self.page.resolve_url(expected)?;
                poll(self.timeout, format!("URL == {expected}"), || async {
                    self.page.run_locator_handlers().await?;
                    let actual = self.page.url().await?;
                    Ok(if (actual == expected) != self.negated {
                        None
                    } else {
                        Some(format!("URL was {actual}"))
                    })
                })
                .await
            })
            .await
    }

    /// Assert the URL contains a fragment.
    pub async fn url_contains(&self, fragment: &str) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.url_contains",
                crate::StepCategory::Assertion,
                async {
                    let page = self.page.clone();
                    let fragment = fragment.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("url contains {fragment:?}{}", not_tag(negated)),
                        || {
                            let page = page.clone();
                            let fragment = fragment.clone();
                            async move {
                                let url = match page.url().await {
                                    Ok(url) => url,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if url.contains(&fragment) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("url was {url:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert a viewport screenshot matches the named snapshot (retries until match).
    pub async fn screenshot(&self, name: &str) -> E2eResult<()> {
        self.page
            .auto_step("expect.screenshot", crate::StepCategory::Assertion, async {
                self.screenshot_with(name, &SnapshotOptions::default())
                    .await
            })
            .await
    }

    /// [`PageExpect::screenshot`] with capture and snapshot options.
    ///
    /// Accepts only successive stable captures within one assertion window,
    /// including generation under missing/all/changed. Operational errors retain
    /// their type; failure artifacts use the last completed capture. Final failure
    /// diagnostics share a separate five-second file/rendering budget, subject
    /// to page cancellation and enclosing step/test deadlines.
    pub async fn screenshot_with(&self, name: &str, opts: &SnapshotOptions) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.screenshot_with",
                crate::StepCategory::Assertion,
                async {
                    let page = self.page.with_timeout(Duration::ZERO);
                    screenshot_assertion(
                        &page,
                        name,
                        opts,
                        self.timeout,
                        self.negated,
                        |options, wait_for_fonts| {
                            let page = page.clone();
                            async move {
                                crate::screenshot::capture_with_font_wait(
                                    &page,
                                    options,
                                    crate::screenshot::Source::Page,
                                    wait_for_fonts,
                                )
                                .await
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the accessibility snapshot equals `expected` exactly.
    pub async fn aria_snapshot(&self, expected: &str) -> E2eResult<()> {
        self.aria_snapshot_options(expected, None).await
    }

    /// Assert exact bounded Ferrite ARIA text with the same options as capture.
    pub async fn aria_snapshot_with(
        &self,
        expected: &str,
        options: crate::AriaSnapshotOptions,
    ) -> E2eResult<()> {
        options.json()?;
        self.aria_snapshot_options(expected, Some(options)).await
    }

    async fn aria_snapshot_options(
        &self,
        expected: &str,
        options: Option<crate::AriaSnapshotOptions>,
    ) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.aria_snapshot",
                crate::StepCategory::Assertion,
                async {
                    let page = self.page.clone();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("aria snapshot == {expected:?}{}", not_tag(negated)),
                        || {
                            let page = page.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match match options {
                                    Some(options) => page.aria_snapshot_with(options).await,
                                    None => page.aria_snapshot().await,
                                } {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual == expected) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("aria snapshot was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }
}

/// Locator-level assertions.
pub struct LocatorExpect {
    pub(crate) locator: Locator,
    pub(crate) timeout: Duration,
    pub(crate) negated: bool,
}

impl LocatorExpect {
    pub(crate) fn new(locator: Locator) -> Self {
        Self {
            timeout: locator.page().expect_timeout(),
            locator,
            negated: false,
        }
    }

    /// Retry a custom element assertion using the configured expectation timeout.
    pub async fn satisfies<F, Fut>(&self, description: &str, predicate: F) -> E2eResult<()>
    where
        F: Fn(Locator) -> Fut,
        Fut: std::future::Future<Output = E2eResult<bool>>,
    {
        self.locator
            .diagnostic_step_local(
                format!("expect.satisfies {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    poll(self.timeout, description.to_string(), || async {
                        self.locator.page().run_locator_handlers().await?;
                        match predicate(self.locator.clone()).await {
                            Ok(value) if value != self.negated => Ok(None),
                            Ok(_) => Ok(Some(description.to_string())),
                            Err(error) => Err(error),
                        }
                    })
                    .await
                },
            )
            .await
    }

    /// Assert normalized text with a Rust regular expression.
    pub async fn text_matches(&self, pattern: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.text_matches {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let pattern =
                        regex::Regex::new(pattern).map_err(|e| E2eError::Config(e.to_string()))?;
                    self.satisfies("text regex", |locator| {
                        let pattern = pattern.clone();
                        async move { Ok(pattern.is_match(&locator.text().await?)) }
                    })
                    .await
                },
            )
            .await
    }

    /// Assert an attribute with a Rust regular expression.
    pub async fn attribute_matches(&self, name: &str, pattern: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.attribute_matches {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let pattern =
                        regex::Regex::new(pattern).map_err(|e| E2eError::Config(e.to_string()))?;
                    self.satisfies("attribute regex", |locator| {
                        let pattern = pattern.clone();
                        async move {
                            Ok(locator
                                .attribute(name)
                                .await?
                                .is_some_and(|value| pattern.is_match(&value)))
                        }
                    })
                    .await
                },
            )
            .await
    }

    /// Assert the exact class attribute.
    pub async fn class(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.class {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.attribute("class", expected).await },
            )
            .await
    }

    /// Assert all selected values of a multiple select.
    pub async fn values(&self, expected: &[&str]) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.values {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.satisfies("selected values", |locator| async move {
                        Ok(locator.selected_options().await? == expected)
                    })
                    .await
                },
            )
            .await
    }

    /// Assert normalized text contents for the complete ordered element list.
    pub async fn texts(&self, expected: &[&str]) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.texts {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let expected: Vec<String> = expected
                        .iter()
                        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                        .collect();
                    self.satisfies("element texts", |locator| {
                        let expected = expected.clone();
                        async move {
                            Ok(locator
                                .all_text_contents()
                                .await?
                                .iter()
                                .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
                                .collect::<Vec<_>>()
                                == expected)
                        }
                    })
                    .await
                },
            )
            .await
    }

    /// Assert the computed ARIA role.
    pub async fn role(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.role {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.satisfies("ARIA role", |locator| async move {
                        Ok(locator.role().await?.as_deref() == Some(expected))
                    })
                    .await
                },
            )
            .await
    }

    /// Assert the associated ARIA error message.
    pub async fn accessible_error_message(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!(
                    "expect.accessible_error_message {}",
                    self.locator.selector()
                ),
                crate::StepCategory::Assertion,
                async {
                    self.satisfies("accessible error message", |locator| async move {
                        Ok(locator.accessible_error_message().await?.as_deref() == Some(expected))
                    })
                    .await
                },
            )
            .await
    }

    /// Assert the element's indented accessibility tree.
    pub async fn aria_snapshot(&self, expected: &str) -> E2eResult<()> {
        self.aria_snapshot_options(expected, None).await
    }

    /// Assert exact bounded Ferrite ARIA text with the same options as capture.
    pub async fn aria_snapshot_with(
        &self,
        expected: &str,
        options: crate::AriaSnapshotOptions,
    ) -> E2eResult<()> {
        options.json()?;
        self.aria_snapshot_options(expected, Some(options)).await
    }

    async fn aria_snapshot_options(
        &self,
        expected: &str,
        options: Option<crate::AriaSnapshotOptions>,
    ) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.aria_snapshot {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.satisfies("ARIA snapshot", |locator| async move {
                        Ok(match options {
                            Some(options) => locator.aria_snapshot_with(options).await?,
                            None => locator.aria_snapshot().await?,
                        } == expected)
                    })
                    .await
                },
            )
            .await
    }

    /// Override the retry window.
    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<Timeout>) -> Self {
        self.timeout = timeout.into().duration();
        self
    }

    /// Negate the assertion (`not().not()` cancels out).
    #[must_use]
    #[allow(clippy::should_implement_trait)] // Playwright names it `not`.
    pub fn not(mut self) -> Self {
        self.negated = !self.negated;
        self
    }

    /// Assert the element is visible.
    pub async fn visible(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.visible {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` visible{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0 && state.visible) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!(
                                        "count={} visible={}",
                                        state.count, state.visible
                                    )))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is hidden or absent.
    pub async fn hidden(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.hidden {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` hidden{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count == 0 || !state.visible) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some("still visible".to_string()))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the exact trimmed text.
    pub async fn text(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.text {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` text == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let expected = expected.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (normalize_text(&state.text) == normalize_text(&expected))
                                    != negated
                                {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("text was {:?}", state.text)))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the text contains a fragment.
    pub async fn contains_text(&self, fragment: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.contains_text {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let fragment = fragment.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` contains {fragment:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let fragment = fragment.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if normalize_text(&state.text).contains(&normalize_text(&fragment))
                                    != negated
                                {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("text was {:?}", state.text)))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the form value.
    pub async fn value(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.value {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` value == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let expected = expected.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.value == expected) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("value was {:?}", state.value)))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the match count.
    pub async fn count(&self, expected: usize) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.count {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` count == {expected}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count == expected) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("count was {}", state.count)))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is checked.
    pub async fn checked(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.checked {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.checked_state(true).await },
            )
            .await
    }

    /// Assert the element is unchecked.
    pub async fn unchecked(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.unchecked {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.checked_state(false).await },
            )
            .await
    }

    async fn checked_state(&self, want: bool) -> E2eResult<()> {
        let locator = self.locator.clone();
        let negated = self.negated;
        poll(
            self.timeout,
            format!(
                "`{}` checked == {want}{}",
                locator.selector(),
                not_tag(negated)
            ),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if (state.checked == want) != negated {
                        Ok(None)
                    } else {
                        Ok(Some(format!("checked was {}", state.checked)))
                    }
                }
            },
        )
        .await
    }

    /// Assert the element is enabled.
    pub async fn enabled(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.enabled {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` enabled{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0 && state.enabled) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some("disabled or absent".to_string()))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is disabled.
    pub async fn disabled(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.disabled {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` disabled{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0 && !state.enabled) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some("enabled or absent".to_string()))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is editable (enabled input/textarea/select or contenteditable).
    pub async fn editable(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.editable {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` editable{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0 && state.editable) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!(
                                        "count={} editable={}",
                                        state.count, state.editable
                                    )))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is empty (no text and no form value).
    pub async fn empty(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.empty {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` empty{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                let empty = state.count > 0
                                    && state.text.is_empty()
                                    && state.value.is_empty();
                                if empty != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!(
                                        "text={:?} value={:?}",
                                        state.text, state.value
                                    )))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is focused.
    pub async fn focused(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.focused {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` focused{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0 && state.focused) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some("unfocused or absent".to_string()))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element is attached to the DOM.
    pub async fn attached(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.attached {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!("`{}` attached{}", locator.selector(), not_tag(negated)),
                        || {
                            let locator = locator.clone();
                            async move {
                                let state = match locator.state().await {
                                    Ok(state) => state,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (state.count > 0) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some("absent".to_string()))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert an attribute value (exact match).
    pub async fn attribute(&self, name: &str, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.attribute {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let name = name.to_string();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` attribute {name:?} == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let name = name.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match locator.attribute(&name).await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual.as_deref() == Some(expected.as_str())) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("attribute {name:?} was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element has a CSS class.
    pub async fn contains_class(&self, class: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.contains_class {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let class = class.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` has class {class:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let class = class.clone();
                            async move {
                                let actual = match locator.attribute("class").await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                let has = actual
                                    .as_deref()
                                    .unwrap_or_default()
                                    .split_whitespace()
                                    .any(|c| c == class);
                                if has != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("class was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element id (exact match).
    pub async fn id(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.id {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.attribute("id", expected).await },
            )
            .await
    }

    /// Assert a computed CSS property value (exact match).
    pub async fn css(&self, property: &str, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.css {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let property = property.to_string();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` css {property:?} == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let property = property.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match locator.css_value(&property).await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual.as_deref() == Some(expected.as_str())) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("css {property:?} was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert a DOM property value (JSON equality).
    pub async fn js_property<T: Serialize>(&self, name: &str, expected: &T) -> E2eResult<()> {
        self.locator
            .diagnostic_step_local(
                format!("expect.js_property {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let expected = serde_json::to_value(expected).map_err(E2eError::Json)?;
                    let locator = self.locator.clone();
                    let name = name.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` js property {name:?} == {expected}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let name = name.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match locator.js_property(&name).await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual.as_ref() == Some(&expected)) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("property {name:?} was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the element intersects the viewport.
    pub async fn in_viewport(&self) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.in_viewport {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.in_viewport_with(0.0).await },
            )
            .await
    }

    /// Assert the accessible name (exact match).
    pub async fn accessible_name(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.accessible_name {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` accessible name == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match locator.accessible_name().await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual.as_deref() == Some(expected.as_str())) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("accessible name was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert the accessible description (exact match).
    pub async fn accessible_description(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.accessible_description {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.clone();
                    let expected = expected.to_string();
                    let negated = self.negated;
                    poll(
                        self.timeout,
                        format!(
                            "`{}` accessible description == {expected:?}{}",
                            locator.selector(),
                            not_tag(negated)
                        ),
                        || {
                            let locator = locator.clone();
                            let expected = expected.clone();
                            async move {
                                let actual = match locator.accessible_description().await {
                                    Ok(actual) => actual,
                                    Err(error) => return Ok(Some(error.to_string())),
                                };
                                if (actual.as_deref() == Some(expected.as_str())) != negated {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("accessible description was {actual:?}")))
                                }
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }

    /// Assert an element screenshot matches the named snapshot (retries until match).
    pub async fn screenshot(&self, name: &str) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.screenshot {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.screenshot_with(name, &SnapshotOptions::default())
                        .await
                },
            )
            .await
    }

    /// [`LocatorExpect::screenshot`] with capture and snapshot options.
    /// Uses successive stable element captures within one assertion window.
    /// Final failure diagnostics share a separate five-second file/rendering
    /// budget, subject to page cancellation and enclosing step/test deadlines.
    pub async fn screenshot_with(&self, name: &str, opts: &SnapshotOptions) -> E2eResult<()> {
        self.locator
            .diagnostic_step(
                format!("expect.screenshot_with {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let locator = self.locator.with_timeout(Duration::ZERO);
                    let page = locator.page();
                    screenshot_assertion(
                        &page,
                        name,
                        opts,
                        self.timeout,
                        self.negated,
                        |options, wait_for_fonts| {
                            let locator = locator.clone();
                            async move {
                                crate::screenshot::capture_with_font_wait(
                                    &locator.page(),
                                    options,
                                    crate::screenshot::Source::Element(Box::new(locator.clone())),
                                    wait_for_fonts,
                                )
                                .await
                            }
                        },
                    )
                    .await
                },
            )
            .await
    }
}

async fn screenshot_assertion<F, Fut>(
    page: &Page,
    name: &str,
    options: &SnapshotOptions,
    timeout: Duration,
    negated: bool,
    mut capture: F,
) -> E2eResult<()>
where
    F: FnMut(ScreenshotOptions, bool) -> Fut,
    Fut: Future<Output = E2eResult<Vec<u8>>>,
{
    page.run_operation(async {
        let deadline = crate::operation::Deadline::new(timeout);
        let mut options = options.clone();
        options.validate()?;
        if options.dir.is_none() {
            options.dir = page.snapshot_dir.clone();
        }
        if options.update.is_none() {
            options.update = page.snapshot_update;
        }
        if options.path_template.is_none() {
            options.path_template = page.snapshot_path_template.clone();
        }
        if options.path_context.is_none() {
            options.path_context = Some(page.snapshot_path_context.clone());
        }
        if let Some(context) = &mut options.path_context {
            context.browser.get_or_insert(page.browser_kind());
        }
        let path = snap_path_for(name, crate::SnapshotKind::Screenshot, &options)?;
        let read = deadline
            .run("screenshot baseline read", async {
                Ok(crate::snapshot_work::read_baseline(path.clone()).await)
            })
            .await;
        let expected = match read {
            Ok(result) => result?,
            Err(E2eError::Timeout(..)) => {
                return Err(E2eError::Expect(format!(
                    "screenshot {name:?} expired while reading its baseline"
                )))
            }
            Err(error) => return Err(error),
        };
        let mode = resolve_update(options.update);
        if expected.is_none() && (negated || mode == SnapshotUpdate::None) {
            return Err(E2eError::Expect(format!(
                "no snapshot {name:?}{}",
                if negated {
                    "; negated assertions never write baselines"
                } else {
                    " (update=none)"
                }
            )));
        }
        let target = if !negated && matches!(mode, SnapshotUpdate::All | SnapshotUpdate::Changed) {
            None
        } else {
            expected.clone()
        };
        let capture_options = options.capture_options();
        let description = format!("screenshot {name:?}{}", not_tag(negated));
        let comparison = crate::snapshot_capture::compare_with_deadline(
            deadline,
            &description,
            target,
            &options,
            negated,
            || capture(capture_options.clone(), options.wait_for_fonts),
        )
        .await;
        let result = match comparison.result {
            Ok(()) if negated => Ok(()),
            Ok(()) => crate::snapshot_commit::finish(
                deadline,
                path.clone(),
                comparison
                    .actual
                    .as_ref()
                    .expect("successful comparison has a capture")
                    .clone(),
                expected.clone(),
                mode,
                &options,
            )
            .await
            .map_err(|error| error.with_context(&description)),
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                // Preserve typed control errors. Never take another capture after
                // exhaustion just to write an artifact, or update an unstable baseline.
                let error = match comparison.actual {
                    Some(actual) if error.code() == "FERRITE_E2E_EXPECT" => {
                        crate::snapshot_artifacts::record(
                            page,
                            crate::snapshot_artifacts::FailureImages {
                                name: name.to_owned(),
                                path,
                                expected,
                                actual,
                                previous: comparison.previous,
                                stable: comparison.stable,
                                threshold: options.threshold,
                            },
                            error,
                        )
                        .await
                    }
                    _ => error,
                };
                Err(error)
            }
        }
    })
    .await
}

impl Page {
    /// Page-level assertions with the default window.
    #[must_use]
    pub fn expect(&self) -> PageExpect {
        PageExpect::new(self.clone())
    }

    /// Assert the exact title (default window).
    pub async fn expect_title(&self, expected: &str) -> E2eResult<()> {
        self.expect().title(expected).await
    }

    /// Assert the URL contains a fragment (default window).
    pub async fn expect_url(&self, fragment: &str) -> E2eResult<()> {
        self.expect().url_contains(fragment).await
    }
}

impl Locator {
    /// Locator assertions with the default window.
    #[must_use]
    pub fn expect(&self) -> LocatorExpect {
        LocatorExpect::new(self.clone())
    }

    /// Assert visibility (default window).
    pub async fn expect_visible(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.expect_visible {}", self.selector()),
            crate::StepCategory::Assertion,
            async { self.expect().visible().await },
        )
        .await
    }

    /// Assert hidden/absent (default window).
    pub async fn expect_hidden(&self) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.expect_hidden {}", self.selector()),
            crate::StepCategory::Assertion,
            async { self.expect().hidden().await },
        )
        .await
    }

    /// Assert exact trimmed text (default window).
    pub async fn expect_text(&self, expected: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.expect_text {}", self.selector()),
            crate::StepCategory::Assertion,
            async { self.expect().text(expected).await },
        )
        .await
    }

    /// Assert the text contains a fragment (default window).
    pub async fn expect_contains_text(&self, fragment: &str) -> E2eResult<()> {
        self.diagnostic_step(
            format!("locator.expect_contains_text {}", self.selector()),
            crate::StepCategory::Assertion,
            async { self.expect().contains_text(fragment).await },
        )
        .await
    }
}

#[cfg(test)]
mod polling_options_tests {
    use super::*;
    use std::sync::Arc;
    use std::{
        cell::Cell,
        rc::Rc,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use tokio::time::Instant;

    #[tokio::test(start_paused = true)]
    async fn intervals_start_immediately_repeat_last_and_support_local_values() {
        let start = Instant::now();
        let mut times = Vec::new();
        let value = Rc::new("local value".to_string());
        let result = expect_poll_with(
            "cadence",
            &PollingOptions::default()
                .intervals([Duration::from_millis(20), Duration::from_millis(40)]),
            || {
                assert!(crate::report::in_retry_probe());
                times.push(start.elapsed().as_millis());
                std::future::ready(Ok((times.len() == 5).then(|| value.clone())))
            },
        )
        .await
        .unwrap();
        assert!(Rc::ptr_eq(&result, &value));
        assert_eq!(times, [0, 20, 60, 100, 140]);
        assert!(!crate::report::in_retry_probe());
        let times = std::cell::RefCell::new(Vec::new());
        let start = Instant::now();
        expect_to_pass("legacy cadence", Timeout::default(), || {
            let mut times = times.borrow_mut();
            times.push(start.elapsed().as_millis());
            std::future::ready(if times.len() == 3 {
                Ok(())
            } else {
                Err(E2eError::Expect("retry".into()))
            })
        })
        .await
        .unwrap();
        assert_eq!(*times.borrow(), [0, 50, 100]);
        assert_eq!(PollingOptions::default().timeout, Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn invalid_intervals_and_precancellation_never_construct_probes() {
        let called = Cell::new(0);
        for intervals in [
            vec![],
            vec![Duration::ZERO],
            vec![Duration::from_millis(10), Duration::ZERO],
        ] {
            let error = expect_to_pass_with(
                "validation",
                &PollingOptions::default()
                    .intervals(intervals)
                    .message("caller message"),
                || {
                    called.set(called.get() + 1);
                    std::future::ready(Ok(()))
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
            assert!(error.to_string().contains("caller message"));
        }
        let token = crate::CancellationToken::new();
        token.cancel_with_reason("already canceled");
        let error = expect_to_pass_with(
            "precancel",
            &PollingOptions::default().cancellation(token),
            || {
                called.set(called.get() + 1);
                std::future::ready(Ok(()))
            },
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, E2eError::Cancelled(message) if message.contains("already canceled"))
        );
        assert_eq!(called.get(), 0);
    }

    struct Released(Arc<AtomicUsize>);
    impl Drop for Released {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[tokio::test(start_paused = true)]
    async fn one_clock_bounds_probe_work_sleep_and_hung_future_release() {
        let calls = Cell::new(0);
        let start = Instant::now();
        let released = Arc::new(AtomicUsize::new(0));
        let error = expect_poll_with::<(), _, _>(
            "one clock",
            &PollingOptions::default()
                .timeout(Timeout::ms(100))
                .intervals([Duration::from_millis(40)]),
            || {
                let guard = Released(released.clone());
                calls.set(calls.get() + 1);
                let delay = if calls.get() == 1 { 30 } else { 50 };
                async move {
                    let _guard = guard;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    Ok(None)
                }
            },
        )
        .await
        .unwrap_err();
        assert_eq!(start.elapsed(), Duration::from_millis(100));
        assert_eq!(calls.get(), 2);
        assert_eq!(released.load(Ordering::SeqCst), 2);
        assert!(matches!(error, E2eError::Expect(message) if message.contains("last: pending")));
        let guard = Released(released.clone());
        let error = expect_to_pass_with(
            "hung",
            &PollingOptions::default().timeout(Timeout::ms(20)),
            || {
                let guard = Released(guard.0.clone());
                async move {
                    let _guard = guard;
                    std::future::pending::<E2eResult<()>>().await
                }
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(error, E2eError::Expect(message) if message.contains("no data yet")));
        assert_eq!(released.load(Ordering::SeqCst), 3);
        drop(guard);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_interrupts_long_waits_and_hung_probes_with_zero_timeout() {
        for hung in [false, true] {
            let token = crate::CancellationToken::new();
            let cancel = token.clone();
            let cancel_task = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                cancel.cancel_with_reason("stop now");
            });
            let options = PollingOptions::default()
                .timeout(Duration::ZERO)
                .intervals([Duration::from_secs(60)])
                .cancellation(token);
            let calls = Cell::new(0);
            let released = Arc::new(AtomicUsize::new(0));
            let start = Instant::now();
            let error = expect_poll_with::<(), _, _>("cancel probe/wait", &options, || {
                calls.set(calls.get() + 1);
                let guard = Released(released.clone());
                async move {
                    let _guard = guard;
                    if hung {
                        std::future::pending::<()>().await;
                    }
                    Ok(None)
                }
            })
            .await
            .unwrap_err();
            assert!(matches!(error,E2eError::Cancelled(message) if message.contains("stop now")));
            assert_eq!(start.elapsed(), Duration::from_millis(20));
            assert_eq!(calls.get(), 1);
            assert_eq!(released.load(Ordering::SeqCst), 1);
            cancel_task.await.unwrap();
        }
    }

    #[tokio::test(start_paused = true)]
    async fn zero_and_large_local_windows_cannot_extend_the_enclosing_clock() {
        for timeout in [Duration::ZERO, Duration::from_secs(5)] {
            let start = Instant::now();
            let calls = Cell::new(0);
            let error = crate::operation::Deadline::new(Duration::from_millis(35))
                .run(
                    "caller",
                    expect_to_pass_with(
                        "enclosed",
                        &PollingOptions::default()
                            .timeout(timeout)
                            .intervals([Duration::from_millis(20)]),
                        || {
                            calls.set(calls.get() + 1);
                            std::future::ready(Err(E2eError::Expect("not yet".into())))
                        },
                    ),
                )
                .await
                .unwrap_err();
            assert!(matches!(error,E2eError::Timeout(35,message) if message=="caller"));
            assert_eq!(start.elapsed(), Duration::from_millis(35));
            assert_eq!(calls.get(), 2);
            assert!(!crate::report::in_retry_probe());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn operational_errors_keep_their_type_and_never_retry_or_become_mismatches() {
        let errors = vec![
            E2eError::Timeout(7, "probe timeout".into()),
            E2eError::Cancelled("stop".into()),
            E2eError::Disconnected("transport".into()),
            E2eError::Skipped("skip".into()),
            E2eError::StepSkipped("step".into()),
            E2eError::Cdp {
                method: "method".into(),
                message: "native".into(),
            },
            E2eError::Config("bad input".into()),
            E2eError::Diagnostic {
                context: "label".into(),
                source: Box::new(E2eError::Timeout(9, "nested timeout".into())),
            },
            E2eError::Io(std::io::Error::from(std::io::ErrorKind::NotFound)),
        ];
        for original in errors {
            let code = original.code();
            let message = original.to_string();
            let mut error = Some(original);
            let calls = Cell::new(0);
            let actual = expect_to_pass_with(
                "typed",
                &PollingOptions::default().message("context"),
                || {
                    calls.set(calls.get() + 1);
                    std::future::ready(Err(error.take().unwrap()))
                },
            )
            .await
            .unwrap_err();
            assert_eq!(actual.code(), code);
            assert!(actual
                .to_string()
                .contains(message.rsplit(": ").next().unwrap()));
            assert_eq!(calls.get(), 1);
        }
        let error = expect_to_pass("legacy typed", Timeout::ms(20), || async {
            Err(E2eError::Timeout(7, "inner".into()))
        })
        .await
        .unwrap_err();
        assert!(matches!(error, E2eError::Timeout(7, _)));
    }

    #[tokio::test(start_paused = true)]
    async fn last_mismatch_message_and_nested_assertions_form_one_outer_step() {
        let options = PollingOptions::default()
            .timeout(Timeout::ms(90))
            .intervals([Duration::from_millis(40)])
            .message("eventually consistent");
        let mut calls = 0;
        let error = expect_to_pass_with("counter", &options, || {
            calls += 1;
            std::future::ready(Err(E2eError::Diagnostic {
                context: "inner label".into(),
                source: Box::new(E2eError::Expect(format!("probe {calls}"))),
            }))
        })
        .await
        .unwrap_err();
        assert_eq!(calls, 3);
        assert!(error.to_string().contains("eventually consistent: counter"));
        assert!(error.to_string().contains("probe 3"));
        assert!(error.to_string().contains("inner label"));
        let session = crate::report::StepSession::new(
            crate::report::ReporterHub::default(),
            crate::AttemptInfo {
                name: "one step".into(),
                file: "poll.rs".into(),
                line: 1,
                project: None,
                worker_index: 0,
                repeat_each_index: 0,
                retry: 0,
            },
        );
        let calls = Cell::new(0);
        session
            .scope(expect_to_pass_with(
                "outer",
                &PollingOptions::default().message("caller"),
                || {
                    calls.set(calls.get() + 1);
                    let done = calls.get() == 3;
                    crate::report::automatic(
                        None,
                        "nested",
                        crate::StepCategory::Assertion,
                        async move {
                            assert!(crate::report::in_retry_probe());
                            if done {
                                Ok(())
                            } else {
                                Err(E2eError::Expect("retry".into()))
                            }
                        },
                    )
                },
            ))
            .await
            .unwrap();
        let steps = session.finish_all();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].title, "expect caller: outer");
        assert_eq!(steps[0].status, crate::StepStatus::Passed);
        assert!(steps[0].steps.is_empty());
        assert!(!crate::report::in_retry_probe());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn polling_bounds_a_hung_check_and_does_not_swallow_cancellation() {
        let result = expect_to_pass("hung", Duration::from_millis(20), || {
            std::future::pending::<E2eResult<()>>()
        })
        .await;
        assert!(matches!(result, Err(E2eError::Expect(_))));
        let result = expect_poll::<u32, _, _>("cancel", Duration::ZERO, || async {
            Err(E2eError::Cancelled("stop".into()))
        })
        .await;
        assert!(matches!(result, Err(E2eError::Cancelled(_))));
        let checks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = checks.clone();
        expect_to_pass("zero", Duration::ZERO, move || {
            let count = count.clone();
            async move {
                if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Err(E2eError::Expect("retry".into()))
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();
    }

    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn soft_asserts_collect_and_report() {
        let mut soft = SoftAsserts::new();
        soft.check(Ok(()));
        soft.check(Err(E2eError::Expect("first".to_string())));
        soft.check(Err(E2eError::Expect("second".to_string())));
        assert_eq!(
            soft.failures(),
            &[
                E2eError::Expect("first".to_string()).to_string(),
                E2eError::Expect("second".to_string()).to_string()
            ]
        );
        let err = soft.assert_all().unwrap_err();
        assert!(err.to_string().contains("2 soft assertion(s)"), "{err}");
        assert!(err.to_string().contains("first"), "{err}");

        let mut clean = SoftAsserts::new();
        clean.check(Ok(()));
        assert!(clean.assert_all().is_ok());
    }
    use std::sync::Arc;

    #[tokio::test]
    async fn poll_retries_until_match() {
        let calls = Arc::new(AtomicUsize::new(0));
        let worker = calls.clone();
        poll(Duration::from_secs(5), "test".to_string(), || {
            let worker = worker.clone();
            async move {
                let n = worker.fetch_add(1, Ordering::SeqCst);
                if n >= 2 {
                    Ok(None)
                } else {
                    Ok(Some("not yet".to_string()))
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn poll_times_out_with_last_detail() {
        let error = poll(Duration::from_millis(120), "thing".to_string(), || async {
            Ok(Some("still wrong".to_string()))
        })
        .await
        .unwrap_err();
        let message = error.to_string();
        assert!(message.contains("thing"), "{message}");
        assert!(message.contains("still wrong"), "{message}");
    }

    #[test]
    fn timeout_conversions() {
        assert_eq!(Timeout::ms(250).duration(), Duration::from_millis(250));
        assert_eq!(Timeout::secs(2).duration(), Duration::from_secs(2));
        assert_eq!(
            Timeout::default().duration(),
            Duration::from_millis(DEFAULT_EXPECT_MS)
        );
    }

    #[tokio::test]
    async fn expect_poll_returns_first_value() {
        let calls = Arc::new(AtomicUsize::new(0));
        let worker = calls.clone();
        let value = expect_poll("counter", Timeout::ms(2000), move || {
            let worker = worker.clone();
            async move {
                let n = worker.fetch_add(1, Ordering::SeqCst);
                if n >= 2 {
                    Ok(Some(n))
                } else {
                    Ok(None)
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(value, 2);
    }

    #[tokio::test]
    async fn expect_poll_times_out_loudly() {
        let error = expect_poll("never", Timeout::ms(120), || async {
            Ok::<Option<u32>, E2eError>(None)
        })
        .await
        .unwrap_err();
        assert!(error.to_string().contains("never"), "{error}");
    }
}
