use crate::{E2eError, E2eResult, LocatorExpect};
use std::future::Future;

/// Expected string or Rust regular expression. Strings use assertion-specific
/// normalization; regular expressions always inspect the unnormalized value.
#[derive(Debug, Clone)]
pub enum TextMatcher {
    Exact(String),
    Regex(regex::Regex),
}
impl TextMatcher {
    pub fn exact(value: impl Into<String>) -> Self {
        Self::Exact(value.into())
    }
    pub fn regex(pattern: &str) -> E2eResult<Self> {
        regex::Regex::new(pattern)
            .map(Self::Regex)
            .map_err(|error| E2eError::Config(format!("invalid assertion regex: {error}")))
    }
    fn prepared(&self, options: MatchOptions) -> E2eResult<Self> {
        match (self, options.ignore_case) {
            (Self::Regex(pattern), Some(ignore_case)) => regex::RegexBuilder::new(pattern.as_str())
                .case_insensitive(ignore_case)
                .build()
                .map(Self::Regex)
                .map_err(|error| E2eError::Config(format!("invalid assertion regex: {error}"))),
            _ => Ok(self.clone()),
        }
    }
    fn matches(
        &self,
        actual: &str,
        contains: bool,
        normalize: bool,
        options: MatchOptions,
    ) -> bool {
        match self {
            Self::Regex(pattern) => pattern.is_match(actual),
            Self::Exact(expected) => {
                let mut actual = if normalize {
                    normalize_string(actual)
                } else {
                    actual.into()
                };
                let mut expected = if normalize {
                    normalize_string(expected)
                } else {
                    expected.clone()
                };
                if options.ignore_case == Some(true) {
                    actual = actual.to_lowercase();
                    expected = expected.to_lowercase();
                }
                if contains {
                    actual.contains(&expected)
                } else {
                    actual == expected
                }
            }
        }
    }
    fn matches_class(&self, actual: &str, options: MatchOptions) -> bool {
        self.matches(actual, false, false, options)
    }
}
impl From<&str> for TextMatcher {
    fn from(value: &str) -> Self {
        Self::exact(value)
    }
}
impl From<String> for TextMatcher {
    fn from(value: String) -> Self {
        Self::Exact(value)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MatchOptions {
    /// None retains regex flags; Some overrides the builder's case setting.
    pub ignore_case: Option<bool>,
}
impl MatchOptions {
    pub fn ignore_case(mut self, value: bool) -> Self {
        self.ignore_case = Some(value);
        self
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct TextAssertionOptions {
    pub matching: MatchOptions,
    pub use_inner_text: bool,
}
impl TextAssertionOptions {
    pub fn ignore_case(mut self, value: bool) -> Self {
        self.matching.ignore_case = Some(value);
        self
    }
    pub fn use_inner_text(mut self, value: bool) -> Self {
        self.use_inner_text = value;
        self
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckedOptions {
    pub checked: Option<bool>,
    pub indeterminate: Option<bool>,
}
impl CheckedOptions {
    pub fn checked(mut self, value: bool) -> Self {
        self.checked = Some(value);
        self
    }
    pub fn indeterminate(mut self, value: bool) -> Self {
        self.indeterminate = Some(value);
        self
    }
}
#[derive(Debug, Clone, Copy)]
pub enum StateAssertion {
    Attached,
    Visible,
    Hidden,
    Enabled,
    Disabled,
    Editable,
    Focused,
    Empty,
}

// JavaScript whitespace used by the pinned Playwright string assertions.
fn normalize_string(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|c| !matches!(c, '\u{200b}' | '\u{ad}'))
        .collect();
    value
        .split(is_js_whitespace)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_js_whitespace(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

impl LocatorExpect {
    async fn observe<F, Fut>(&self, label: String, check: F) -> E2eResult<()>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = E2eResult<Option<String>>>,
    {
        self.locator
            .page()
            .auto_step_local(
                format!("expect {label} {}", self.locator.selector()),
                crate::StepCategory::Assertion,
                self.locator.page().run_operation(crate::expect::poll(
                    self.timeout,
                    label,
                    || async {
                        self.locator.page().run_locator_handlers().await?;
                        check().await
                    },
                )),
            )
            .await
    }
    fn mismatch(&self, matched: bool, actual: impl std::fmt::Debug) -> Option<String> {
        if matched != self.negated {
            None
        } else {
            Some(format!("actual: {actual:?}"))
        }
    }
    async fn match_text(
        &self,
        expected: &TextMatcher,
        options: TextAssertionOptions,
        contains: bool,
    ) -> E2eResult<()> {
        let expected = expected.prepared(options.matching)?;
        self.observe(
            format!(
                "text {}{expected:?}",
                if contains { "contains " } else { "equals " }
            ),
            || async {
                let actual = if options.use_inner_text {
                    self.locator.inner_text().await?
                } else {
                    self.locator.text_content().await?
                };
                Ok(match actual {
                    Some(actual) => self.mismatch(
                        expected.matches(&actual, contains, true, options.matching),
                        actual,
                    ),
                    None => Some("element has no text value".into()),
                })
            },
        )
        .await
    }
    pub async fn text_with(
        &self,
        expected: &TextMatcher,
        options: TextAssertionOptions,
    ) -> E2eResult<()> {
        self.match_text(expected, options, false).await
    }
    pub async fn contains_text_with(
        &self,
        expected: &TextMatcher,
        options: TextAssertionOptions,
    ) -> E2eResult<()> {
        self.match_text(expected, options, true).await
    }
    async fn match_texts(
        &self,
        expected: &[TextMatcher],
        options: TextAssertionOptions,
        subset: bool,
    ) -> E2eResult<()> {
        let expected: Vec<_> = expected
            .iter()
            .map(|m| m.prepared(options.matching))
            .collect::<E2eResult<_>>()?;
        self.observe(
            format!(
                "ordered {}texts {expected:?}",
                if subset { "subset " } else { "" }
            ),
            || async {
                let actual = if options.use_inner_text {
                    self.locator.all_inner_texts().await?
                } else {
                    self.locator.all_text_contents().await?
                };
                let matched = if subset {
                    let mut found = 0;
                    for item in &actual {
                        if found < expected.len()
                            && expected[found].matches(item, true, true, options.matching)
                        {
                            found += 1;
                        }
                    }
                    found == expected.len()
                } else {
                    actual.len() == expected.len()
                        && actual
                            .iter()
                            .zip(&expected)
                            .all(|(a, e)| e.matches(a, false, true, options.matching))
                };
                Ok(self.mismatch(matched, actual))
            },
        )
        .await
    }
    pub async fn texts_with(
        &self,
        expected: &[TextMatcher],
        options: TextAssertionOptions,
    ) -> E2eResult<()> {
        self.match_texts(expected, options, false).await
    }
    pub async fn contains_texts_with(
        &self,
        expected: &[TextMatcher],
        options: TextAssertionOptions,
    ) -> E2eResult<()> {
        self.match_texts(expected, options, true).await
    }
    pub async fn class_with(&self, expected: &TextMatcher, options: MatchOptions) -> E2eResult<()> {
        let expected = expected.prepared(options)?;
        self.observe(format!("class {expected:?}"), || async {
            let actual = self.locator.attribute("class").await?.unwrap_or_default();
            Ok(self.mismatch(expected.matches_class(&actual, options), actual))
        })
        .await
    }
    pub async fn classes_with(
        &self,
        expected: &[TextMatcher],
        options: MatchOptions,
    ) -> E2eResult<()> {
        let expected: Vec<_> = expected
            .iter()
            .map(|m| m.prepared(options))
            .collect::<E2eResult<_>>()?;
        self.observe(format!("element classes {expected:?}"), || async {
            let actual: Vec<String> = self
                .locator
                .evaluate_all("els => els.map(el => el.getAttribute('class') || '')")
                .await?;
            let matched = actual.len() == expected.len()
                && actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, e)| e.matches_class(a, options));
            Ok(self.mismatch(matched, actual))
        })
        .await
    }
    pub async fn values_with(
        &self,
        expected: &[TextMatcher],
        options: MatchOptions,
    ) -> E2eResult<()> {
        let expected: Vec<_> = expected
            .iter()
            .map(|m| m.prepared(options))
            .collect::<E2eResult<_>>()?;
        self.observe(format!("selected values {expected:?}"), || async {
            let actual: Vec<String> = self.locator.evaluate("el => { if (el.tagName !== 'SELECT' || !el.multiple) throw new Error('values assertion requires a multiple select'); return Array.from(el.selectedOptions).map(o => o.value); }").await?;
            let matched = actual.len() == expected.len() && actual.iter().zip(&expected).all(|(a,e)| e.matches(a, false, false, options));
            Ok(self.mismatch(matched, actual))
        }).await
    }

    /// Require every expected class token on a single element; order is ignored.
    pub async fn contains_class_tokens(&self, expected: &[&str]) -> E2eResult<()> {
        self.observe(format!("class tokens {expected:?}"), || async {
            let actual: Vec<String> = self
                .locator
                .evaluate("el => Array.from(el.classList)")
                .await?;
            let matched = expected
                .iter()
                .flat_map(|value| value.split(is_js_whitespace))
                .filter(|token| !token.is_empty())
                .all(|token| actual.iter().any(|class| class == token));
            Ok(self.mismatch(matched, actual))
        })
        .await
    }

    /// Require class tokens for the complete ordered element list.
    pub async fn contains_class_tokens_list(&self, expected: &[&str]) -> E2eResult<()> {
        self.observe(format!("element class tokens {expected:?}"), || async {
            let actual: Vec<Vec<String>> = self
                .locator
                .evaluate_all("els => els.map(el => Array.from(el.classList))")
                .await?;
            let matched = actual.len() == expected.len()
                && actual.iter().zip(expected).all(|(tokens, expected)| {
                    expected
                        .split(is_js_whitespace)
                        .filter(|token| !token.is_empty())
                        .all(|token| tokens.iter().any(|class| class == token))
                });
            Ok(self.mismatch(matched, actual))
        })
        .await
    }
    pub async fn checked_with(&self, options: CheckedOptions) -> E2eResult<()> {
        if options.checked.is_some() && options.indeterminate.is_some() {
            return Err(E2eError::Config(
                "checked and indeterminate options cannot be combined".into(),
            ));
        }
        self.observe(format!("checked state {options:?}"), || async {
            let actual: serde_json::Value = self.locator.evaluate("el => { if (el.tagName !== 'INPUT' || !['checkbox','radio'].includes(el.type)) throw new Error('checked assertion requires a checkbox or radio input'); return { checked: el.checked, indeterminate: !!el.indeterminate }; }").await?;
            let matched = if let Some(value) = options.indeterminate { actual["indeterminate"] == value }
                else { actual["checked"] == options.checked.unwrap_or(true) };
            Ok(self.mismatch(matched, actual))
        }).await
    }
    pub async fn state_with(&self, state: StateAssertion, expected: bool) -> E2eResult<()> {
        self.observe(format!("{state:?} == {expected}"), || async {
            let observed = self.locator.state().await?;
            if observed.count > 1 { return Err(E2eError::Locator { selector: self.locator.selector().into(), message: "strict mode violation: multiple elements match".into() }); }
            let actual = match state {
                StateAssertion::Attached => observed.count == 1,
                StateAssertion::Visible => observed.count == 1 && observed.visible,
                StateAssertion::Hidden => observed.count == 0 || !observed.visible,
                _ if observed.count == 0 => return Ok(Some("element is absent".into())),
                StateAssertion::Enabled => observed.enabled,
                StateAssertion::Disabled => !observed.enabled,
                StateAssertion::Editable => observed.editable,
                StateAssertion::Focused => observed.focused,
                StateAssertion::Empty => self.locator.evaluate::<bool>("el => ['INPUT','TEXTAREA'].includes(el.tagName) ? el.value === '' : !el.textContent.trim()").await?,
            };
            Ok(self.mismatch(actual == expected, actual))
        }).await
    }
    pub async fn in_viewport_with(&self, ratio: f64) -> E2eResult<()> {
        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
            return Err(E2eError::Config(
                "viewport ratio must be finite and between 0 and 1".into(),
            ));
        }
        self.observe(format!("viewport intersection >= {ratio}"), || async {
            let count = self.locator.state().await?.count;
            if count > 1 {
                return Err(E2eError::Locator {
                    selector: self.locator.selector().into(),
                    message: "strict mode violation: multiple elements match".into(),
                });
            }
            let actual = if count == 0 {
                0.0
            } else {
                self.locator.intersection_ratio().await?
            };
            Ok(self.mismatch(actual > 0.0 && actual + 1e-9 >= ratio, actual))
        })
        .await
    }
    async fn accessible_with(
        &self,
        field: &str,
        expected: &TextMatcher,
        options: MatchOptions,
    ) -> E2eResult<()> {
        let expected = expected.prepared(options)?;
        self.observe(format!("accessible {field} {expected:?}"), || async {
            let actual = match field {
                "name" => self.locator.accessible_name().await?,
                "description" => self.locator.accessible_description().await?,
                _ => self.locator.accessible_error_message().await?,
            };
            Ok(match actual {
                Some(actual) => {
                    self.mismatch(expected.matches(&actual, false, true, options), actual)
                }
                None => Some(format!("element has no accessible {field}")),
            })
        })
        .await
    }
    pub async fn accessible_name_with(
        &self,
        expected: &TextMatcher,
        options: MatchOptions,
    ) -> E2eResult<()> {
        self.accessible_with("name", expected, options).await
    }
    pub async fn accessible_description_with(
        &self,
        expected: &TextMatcher,
        options: MatchOptions,
    ) -> E2eResult<()> {
        self.accessible_with("description", expected, options).await
    }
    pub async fn accessible_error_message_with(
        &self,
        expected: &TextMatcher,
        options: MatchOptions,
    ) -> E2eResult<()> {
        self.accessible_with("error message", expected, options)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_preserves_raw_regex_and_class_tokens() {
        let options = MatchOptions::default();
        let text = "\u{feff} A\u{200b}\u{ad}\n\u{a0}B  ";
        assert!(TextMatcher::exact("A B").matches(text, false, true, options));
        assert!(!TextMatcher::regex("^A B$")
            .unwrap()
            .matches(text, false, true, options));
        assert!(TextMatcher::regex("\\n")
            .unwrap()
            .matches(text, false, true, options));
        assert!(!TextMatcher::exact("two one").matches_class("one two", options));
        assert!(!TextMatcher::exact("one").matches_class("one two", options));
        assert!(!TextMatcher::exact("one one").matches_class("one", options));
    }
}
