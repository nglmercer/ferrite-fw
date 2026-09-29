//! Auto-retrying assertions (`expect_*`), Playwright-style.

use std::future::Future;
use std::time::Duration;

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

/// Page-level assertions.
pub struct PageExpect {
    page: Page,
    timeout: Duration,
}

impl PageExpect {
    pub(crate) fn new(page: Page) -> Self {
        Self {
            page,
            timeout: Duration::from_millis(DEFAULT_EXPECT_MS),
        }
    }

    /// Override the retry window.
    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<Timeout>) -> Self {
        self.timeout = timeout.into().duration();
        self
    }

    /// Assert the exact title.
    pub async fn title(&self, expected: &str) -> E2eResult<()> {
        let page = self.page.clone();
        let expected = expected.to_string();
        poll(self.timeout, format!("title == {expected:?}"), || {
            let page = page.clone();
            let expected = expected.clone();
            async move {
                let title = match page.title().await {
                    Ok(title) => title,
                    Err(error) => return Ok(Some(error.to_string())),
                };
                if title == expected {
                    Ok(None)
                } else {
                    Ok(Some(format!("title was {title:?}")))
                }
            }
        })
        .await
    }

    /// Assert the title contains a fragment.
    pub async fn title_contains(&self, fragment: &str) -> E2eResult<()> {
        let page = self.page.clone();
        let fragment = fragment.to_string();
        poll(self.timeout, format!("title contains {fragment:?}"), || {
            let page = page.clone();
            let fragment = fragment.clone();
            async move {
                let title = match page.title().await {
                    Ok(title) => title,
                    Err(error) => return Ok(Some(error.to_string())),
                };
                if title.contains(&fragment) {
                    Ok(None)
                } else {
                    Ok(Some(format!("title was {title:?}")))
                }
            }
        })
        .await
    }

    /// Assert the URL contains a fragment.
    pub async fn url_contains(&self, fragment: &str) -> E2eResult<()> {
        let page = self.page.clone();
        let fragment = fragment.to_string();
        poll(self.timeout, format!("url contains {fragment:?}"), || {
            let page = page.clone();
            let fragment = fragment.clone();
            async move {
                let url = match page.url().await {
                    Ok(url) => url,
                    Err(error) => return Ok(Some(error.to_string())),
                };
                if url.contains(&fragment) {
                    Ok(None)
                } else {
                    Ok(Some(format!("url was {url:?}")))
                }
            }
        })
        .await
    }
}

/// Locator-level assertions.
pub struct LocatorExpect {
    locator: Locator,
    timeout: Duration,
}

impl LocatorExpect {
    pub(crate) fn new(locator: Locator) -> Self {
        Self {
            locator,
            timeout: Duration::from_millis(DEFAULT_EXPECT_MS),
        }
    }

    /// Override the retry window.
    #[must_use]
    pub fn timeout(mut self, timeout: impl Into<Timeout>) -> Self {
        self.timeout = timeout.into().duration();
        self
    }

    /// Assert the element is visible.
    pub async fn visible(&self) -> E2eResult<()> {
        let locator = self.locator.clone();
        poll(
            self.timeout,
            format!("`{}` visible", locator.selector()),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.count > 0 && state.visible {
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
        poll(
            self.timeout,
            format!("`{}` hidden", locator.selector()),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.count == 0 || !state.visible {
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
        poll(
            self.timeout,
            format!("`{}` text == {expected:?}", locator.selector()),
            || {
                let locator = locator.clone();
                let expected = expected.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.text == expected {
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
        poll(
            self.timeout,
            format!("`{}` contains {fragment:?}", locator.selector()),
            || {
                let locator = locator.clone();
                let fragment = fragment.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.text.contains(&fragment) {
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
        poll(
            self.timeout,
            format!("`{}` value == {expected:?}", locator.selector()),
            || {
                let locator = locator.clone();
                let expected = expected.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.value == expected {
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
        poll(
            self.timeout,
            format!("`{}` count == {expected}", locator.selector()),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.count == expected {
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
        poll(
            self.timeout,
            format!("`{}` checked == {want}", locator.selector()),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.checked == want {
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
        poll(
            self.timeout,
            format!("`{}` enabled", locator.selector()),
            || {
                let locator = locator.clone();
                async move {
                    let state = match locator.state().await {
                        Ok(state) => state,
                        Err(error) => return Ok(Some(error.to_string())),
                    };
                    if state.count > 0 && state.enabled {
                        Ok(None)
                    } else {
                        Ok(Some("disabled or absent".to_string()))
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
}
