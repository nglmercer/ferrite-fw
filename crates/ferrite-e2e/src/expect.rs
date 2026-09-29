//! Auto-retrying assertions (`expect_*`), Playwright-style.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;

use crate::error::{E2eError, E2eResult};
use crate::locator::Locator;
use crate::page::Page;

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

async fn poll<F, Fut>(timeout: Duration, description: String, mut check: F) -> E2eResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = E2eResult<Option<String>>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last: Option<String>;
    loop {
        match check().await {
            Ok(None) => return Ok(()),
            Ok(Some(mismatch)) => last = Some(mismatch),
            Err(error) => last = Some(error.to_string()),
        }
        if tokio::time::Instant::now() > deadline {
            return Err(E2eError::Expect(format!(
                "{description} (last: {})",
                last.as_deref().unwrap_or("no data yet")
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
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
                Err(error) => Ok(Some(error.to_string())),
            }
        }
    })
    .await?;
    let value = found.lock().unwrap_or_else(|e| e.into_inner()).take();
    value.ok_or_else(|| E2eError::Expect("poll finished without a value".to_string()))
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
            page,
            timeout: Duration::from_millis(DEFAULT_EXPECT_MS),
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

    /// Assert the exact title.
    pub async fn title(&self, expected: &str) -> E2eResult<()> {
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
    }

    /// Assert the title contains a fragment.
    pub async fn title_contains(&self, fragment: &str) -> E2eResult<()> {
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
    }

    /// Assert the URL contains a fragment.
    pub async fn url_contains(&self, fragment: &str) -> E2eResult<()> {
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
    }
}

/// Locator-level assertions.
pub struct LocatorExpect {
    locator: Locator,
    timeout: Duration,
    negated: bool,
}

impl LocatorExpect {
    pub(crate) fn new(locator: Locator) -> Self {
        Self {
            locator,
            timeout: Duration::from_millis(DEFAULT_EXPECT_MS),
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

    /// Assert the element is visible.
    pub async fn visible(&self) -> E2eResult<()> {
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
    }

    /// Assert the element is hidden or absent.
    pub async fn hidden(&self) -> E2eResult<()> {
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
    }

    /// Assert the exact trimmed text.
    pub async fn text(&self, expected: &str) -> E2eResult<()> {
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
                    if (state.text == expected) != negated {
                        Ok(None)
                    } else {
                        Ok(Some(format!("text was {:?}", state.text)))
                    }
                }
            },
        )
        .await
    }

    /// Assert the text contains a fragment.
    pub async fn contains_text(&self, fragment: &str) -> E2eResult<()> {
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
                    if state.text.contains(&fragment) != negated {
                        Ok(None)
                    } else {
                        Ok(Some(format!("text was {:?}", state.text)))
                    }
                }
            },
        )
        .await
    }

    /// Assert the form value.
    pub async fn value(&self, expected: &str) -> E2eResult<()> {
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
    }

    /// Assert the match count.
    pub async fn count(&self, expected: usize) -> E2eResult<()> {
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
    }

    /// Assert the element is checked.
    pub async fn checked(&self) -> E2eResult<()> {
        self.checked_state(true).await
    }

    /// Assert the element is unchecked.
    pub async fn unchecked(&self) -> E2eResult<()> {
        self.checked_state(false).await
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
    }

    /// Assert the element is disabled.
    pub async fn disabled(&self) -> E2eResult<()> {
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
    }

    /// Assert the element is editable (enabled input/textarea/select or contenteditable).
    pub async fn editable(&self) -> E2eResult<()> {
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
    }

    /// Assert the element is empty (no text and no form value).
    pub async fn empty(&self) -> E2eResult<()> {
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
                    let empty = state.count > 0 && state.text.is_empty() && state.value.is_empty();
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
    }

    /// Assert the element is focused.
    pub async fn focused(&self) -> E2eResult<()> {
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
    }

    /// Assert the element is attached to the DOM.
    pub async fn attached(&self) -> E2eResult<()> {
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
    }

    /// Assert an attribute value (exact match).
    pub async fn attribute(&self, name: &str, expected: &str) -> E2eResult<()> {
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
    }

    /// Assert the element has a CSS class.
    pub async fn contains_class(&self, class: &str) -> E2eResult<()> {
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
    }

    /// Assert the element id (exact match).
    pub async fn id(&self, expected: &str) -> E2eResult<()> {
        self.attribute("id", expected).await
    }

    /// Assert a computed CSS property value (exact match).
    pub async fn css(&self, property: &str, expected: &str) -> E2eResult<()> {
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
    }

    /// Assert a DOM property value (JSON equality).
    pub async fn js_property<T: Serialize>(&self, name: &str, expected: &T) -> E2eResult<()> {
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
    }

    /// Assert the element intersects the viewport.
    pub async fn in_viewport(&self) -> E2eResult<()> {
        let locator = self.locator.clone();
        let negated = self.negated;
        poll(
            self.timeout,
            format!("`{}` in viewport{}", locator.selector(), not_tag(negated)),
            || {
                let locator = locator.clone();
                async move {
                    let inside = match locator.in_viewport().await {
                        Ok(inside) => inside,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if inside != negated {
                        Ok(None)
                    } else {
                        Ok(Some("outside the viewport or absent".to_string()))
                    }
                }
            },
        )
        .await
    }

    /// Assert the accessible name (exact match).
    pub async fn accessible_name(&self, expected: &str) -> E2eResult<()> {
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
    }

    /// Assert the accessible description (exact match).
    pub async fn accessible_description(&self, expected: &str) -> E2eResult<()> {
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
    use std::sync::atomic::{AtomicUsize, Ordering};
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
