//! Auto-retrying assertions (`expect_*`), Playwright-style.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::error::{E2eError, E2eResult};
use crate::locator::Locator;
use crate::page::{Page, ScreenshotOptions};
use crate::snapshot::{
    assert_snapshot_png, compare_png, resolve_update, snap_path_for, SnapshotOptions,
    SnapshotUpdate,
};

/// Collect assertion failures without stopping at the first one
/// (Playwright `expect.soft` equivalent): feed each assertion result to
/// [`SoftAsserts::check`], then fail once via [`SoftAsserts::assert_all`].
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
/// `None` means "not yet"; `Err` is recorded as the last mismatch and
/// retried. Fails loudly with `E2eError::Expect` on timeout.
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
    use std::sync::Mutex;

    let found: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let slot = found.clone();
    let check = Arc::new(check);
    poll(timeout.into().duration(), description.into(), move || {
        let found = slot.clone();
        let check = check.clone();
        async move {
            match check().await {
                Ok(Some(value)) => {
                    *found.lock().unwrap_or_else(|e| e.into_inner()) = Some(value);
                    Ok(None)
                }
                Ok(None) => Ok(Some("pending".to_string())),
                Err(error) => Err(error),
            }
        }
    })
    .await?;
    let value = found.lock().unwrap_or_else(|e| e.into_inner()).take();
    value.ok_or_else(|| E2eError::Expect("poll finished without a value".to_string()))
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
    poll(timeout.into().duration(), description.into(), || async {
        check().await.map(|()| None)
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

    /// [`PageExpect::screenshot`] with explicit snapshot options.
    ///
    /// Missing snapshots and `update=all` are one-shot (write once, pass);
    /// otherwise captures are compared until they match or the timeout expires.
    pub async fn screenshot_with(&self, name: &str, opts: &SnapshotOptions) -> E2eResult<()> {
        self.page
            .auto_step(
                "expect.screenshot_with",
                crate::StepCategory::Assertion,
                async {
                    let mut effective = opts.clone();
                    if effective.dir.is_none() {
                        effective.dir = self.page.snapshot_dir.clone();
                    }
                    let opts = &effective;
                    let path = snap_path_for(name, "png", opts);
                    if !path.is_file() || resolve_update(opts.update) == SnapshotUpdate::All {
                        let actual = self.page.screenshot(ScreenshotOptions::default()).await?;
                        return assert_snapshot_png(name, &actual, opts);
                    }
                    let expected = std::fs::read(&path)?;
                    let page = self.page.clone();
                    let opts = opts.clone();
                    let name = name.to_string();
                    let negated = self.negated;
                    let description = format!("screenshot {name:?}{}", not_tag(negated));
                    let result = poll(self.timeout, description, || {
                        let page = page.clone();
                        let expected = expected.clone();
                        let opts = opts.clone();
                        async move {
                            let actual = match page.screenshot(ScreenshotOptions::default()).await {
                                Ok(bytes) => bytes,
                                Err(error) => return Ok(Some(error.to_string())),
                            };
                            match compare_png(&actual, &expected, opts.threshold) {
                                Ok(diff) if diff.passed(&opts) != negated => Ok(None),
                                Ok(diff) => Ok(Some(diff.summary())),
                                // Size mismatches count as different under negation.
                                Err(_) if negated => Ok(None),
                                Err(error) => Err(error),
                            }
                        }
                    })
                    .await;
                    match result {
                        Ok(()) => Ok(()),
                        Err(poll_error) => {
                            // Final capture for the `.actual.png` artifact + detailed message.
                            let actual = page.screenshot(ScreenshotOptions::default()).await?;
                            match assert_snapshot_png(&name, &actual, &opts) {
                                Err(rich) => Err(rich),
                                // Negated case: still matching at timeout.
                                Ok(()) => Err(poll_error),
                            }
                        }
                    }
                },
            )
            .await
    }

    /// Assert the accessibility snapshot equals `expected` exactly.
    pub async fn aria_snapshot(&self, expected: &str) -> E2eResult<()> {
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
                                let actual = match page.aria_snapshot().await {
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
            .page()
            .auto_step_local(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
                format!("expect.class {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.attribute("class", expected).await },
            )
            .await
    }

    /// Assert all selected values of a multiple select.
    pub async fn values(&self, expected: &[&str]) -> E2eResult<()> {
        self.locator
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
        self.locator
            .page()
            .auto_step(
                format!("expect.aria_snapshot {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.satisfies("ARIA snapshot", |locator| async move {
                        Ok(locator.aria_snapshot().await? == expected)
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
                format!("expect.checked {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.checked_state(true).await },
            )
            .await
    }

    /// Assert the element is unchecked.
    pub async fn unchecked(&self) -> E2eResult<()> {
        self.locator
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
                format!("expect.id {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async { self.attribute("id", expected).await },
            )
            .await
    }

    /// Assert a computed CSS property value (exact match).
    pub async fn css(&self, property: &str, expected: &str) -> E2eResult<()> {
        self.locator
            .page()
            .auto_step(
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
            .page()
            .auto_step_local(
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
        self.in_viewport_with(0.0).await
    }

    /// Assert the accessible name (exact match).
    pub async fn accessible_name(&self, expected: &str) -> E2eResult<()> {
        self.locator
            .page()
            .auto_step(
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
            .page()
            .auto_step(
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
            .page()
            .auto_step(
                format!("expect.screenshot {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    self.screenshot_with(name, &SnapshotOptions::default())
                        .await
                },
            )
            .await
    }

    /// [`LocatorExpect::screenshot`] with explicit snapshot options.
    ///
    /// Missing snapshots and `update=all` are one-shot (write once, pass);
    /// otherwise captures are compared until they match or the timeout expires.
    pub async fn screenshot_with(&self, name: &str, opts: &SnapshotOptions) -> E2eResult<()> {
        self.locator
            .page()
            .auto_step(
                format!("expect.screenshot_with {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                async {
                    let mut effective = opts.clone();
                    if effective.dir.is_none() {
                        effective.dir = self.locator.page().snapshot_dir.clone();
                    }
                    let opts = &effective;
                    let path = snap_path_for(name, "png", opts);
                    if !path.is_file() || resolve_update(opts.update) == SnapshotUpdate::All {
                        let actual = self.locator.screenshot().await?;
                        return assert_snapshot_png(name, &actual, opts);
                    }
                    let expected = std::fs::read(&path)?;
                    let locator = self.locator.clone();
                    let opts = opts.clone();
                    let name = name.to_string();
                    let negated = self.negated;
                    let description = format!(
                        "`{}` screenshot {name:?}{}",
                        locator.selector(),
                        not_tag(negated)
                    );
                    let result = poll(self.timeout, description, || {
                        let locator = locator.clone();
                        let expected = expected.clone();
                        let opts = opts.clone();
                        async move {
                            let actual = match locator.screenshot().await {
                                Ok(bytes) => bytes,
                                Err(error) => return Ok(Some(error.to_string())),
                            };
                            match compare_png(&actual, &expected, opts.threshold) {
                                Ok(diff) if diff.passed(&opts) != negated => Ok(None),
                                Ok(diff) => Ok(Some(diff.summary())),
                                // Size mismatches count as different under negation.
                                Err(_) if negated => Ok(None),
                                Err(error) => Err(error),
                            }
                        }
                    })
                    .await;
                    match result {
                        Ok(()) => Ok(()),
                        Err(poll_error) => {
                            // Final capture for the `.actual.png` artifact + detailed message.
                            let actual = locator.screenshot().await?;
                            match assert_snapshot_png(&name, &actual, &opts) {
                                Err(rich) => Err(rich),
                                // Negated case: still matching at timeout.
                                Ok(()) => Err(poll_error),
                            }
                        }
                    }
                },
            )
            .await
    }
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
        self.expect().visible().await
    }

    /// Assert hidden/absent (default window).
    pub async fn expect_hidden(&self) -> E2eResult<()> {
        self.expect().hidden().await
    }

    /// Assert exact trimmed text (default window).
    pub async fn expect_text(&self, expected: &str) -> E2eResult<()> {
        self.expect().text(expected).await
    }

    /// Assert the text contains a fragment (default window).
    pub async fn expect_contains_text(&self, fragment: &str) -> E2eResult<()> {
        self.expect().contains_text(fragment).await
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
