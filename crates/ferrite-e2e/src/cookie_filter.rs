use crate::TextMatcher;

/// Cookie filters are ANDed; each field supports exact strings or Rust regex.
/// Matching is case-sensitive and uses native strings without normalization.
/// An empty filter retains the existing clear-all contract.
#[derive(Debug, Clone, Default)]
pub struct CookieFilter {
    pub name: Option<TextMatcher>,
    pub domain: Option<TextMatcher>,
    pub path: Option<TextMatcher>,
}
impl CookieFilter {
    pub fn name(mut self, value: impl Into<TextMatcher>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn domain(mut self, value: impl Into<TextMatcher>) -> Self {
        self.domain = Some(value.into());
        self
    }
    pub fn path(mut self, value: impl Into<TextMatcher>) -> Self {
        self.path = Some(value.into());
        self
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.name.is_none() && self.domain.is_none() && self.path.is_none()
    }
    pub(crate) fn matches(&self, name: &str, domain: &str, path: &str) -> bool {
        [&self.name, &self.domain, &self.path]
            .into_iter()
            .zip([name, domain, path])
            .all(|(matcher, actual)| match matcher {
                None => true,
                Some(TextMatcher::Exact(expected)) => actual == expected,
                Some(TextMatcher::Regex(regex)) => regex.is_match(actual),
            })
    }
}
