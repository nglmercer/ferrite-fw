use crate::{E2eError, E2eResult};

/// Reusable full-URL matching for navigation and network waits.
/// Exact relative URLs resolve against the page's configured base URL.
#[derive(Debug, Clone)]
pub struct UrlMatcher(Kind);
#[derive(Debug, Clone)]
enum Kind {
    Exact(String),
    Contains(String),
    Pattern(regex::Regex),
}
impl UrlMatcher {
    pub fn exact(url: impl Into<String>) -> Self {
        Self(Kind::Exact(url.into()))
    }
    /// Explicit substring matching, including compatibility with legacy waits.
    pub fn contains(fragment: impl Into<String>) -> Self {
        Self(Kind::Contains(fragment.into()))
    }
    /// Full-URL glob: `*` excludes `/`, `**` includes it, `{a,b}` are alternatives,
    /// `?` is literal, and a backslash escapes the following character.
    pub fn glob(pattern: &str) -> E2eResult<Self> {
        let mut chars = pattern.chars().peekable();
        let mut expression = String::from("^");
        let mut depth = 0usize;
        let mut previous = None;
        while let Some(c) = chars.next() {
            let before = previous;
            previous = Some(c);
            match c {
                '\\' => {
                    let escaped = chars
                        .next()
                        .ok_or_else(|| E2eError::Config("URL glob ends with an escape".into()))?;
                    expression.push_str(&regex::escape(&escaped.to_string()));
                    previous = Some(escaped);
                }
                '*' => {
                    if chars.peek() == Some(&'*') {
                        while chars.peek() == Some(&'*') {
                            chars.next();
                        }
                        if chars.peek() == Some(&'/') {
                            chars.next();
                            previous = Some('/');
                            expression.push_str(if before == Some('/') {
                                "(?:.+/)?"
                            } else {
                                ".*/"
                            });
                        } else {
                            expression.push_str(".*");
                        }
                    } else {
                        expression.push_str("[^/]*");
                    }
                }
                '{' => {
                    if depth != 0 {
                        return Err(E2eError::Config(
                            "URL glob does not support nested '{'".into(),
                        ));
                    }
                    depth += 1;
                    expression.push_str("(?:");
                }
                '}' if depth > 0 => {
                    depth -= 1;
                    expression.push(')');
                }
                '}' => return Err(E2eError::Config("URL glob has an unmatched '}'".into())),
                ',' if depth > 0 => expression.push('|'),
                _ => expression.push_str(&regex::escape(&c.to_string())),
            }
        }
        if depth != 0 {
            return Err(E2eError::Config("URL glob has an unmatched '{'".into()));
        }
        expression.push('$');
        Self::regex(&expression)
    }
    /// Regex matching follows the supplied anchors; unanchored patterns search.
    pub fn regex(pattern: &str) -> E2eResult<Self> {
        regex::Regex::new(pattern)
            .map(|r| Self(Kind::Pattern(r)))
            .map_err(|e| E2eError::Config(format!("invalid URL regex: {e}")))
    }
    pub fn matches(&self, url: &str) -> bool {
        match &self.0 {
            Kind::Exact(expected) => url == expected,
            Kind::Contains(fragment) => url.contains(fragment),
            Kind::Pattern(pattern) => pattern.is_match(url),
        }
    }
    pub(crate) fn resolved(
        &self,
        resolve: impl FnOnce(&str) -> E2eResult<String>,
    ) -> E2eResult<Self> {
        match &self.0 {
            Kind::Exact(url) if reqwest::Url::parse(url).is_err() => Ok(Self::exact(resolve(url)?)),
            _ => Ok(self.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matching_preserves_url_glob_boundaries_and_literal_query_marks() {
        let pattern = UrlMatcher::glob("https://*.test/{api,v1}/users?active=*").unwrap();
        assert!(pattern.matches("https://app.test/api/users?active=yes"));
        for bad in [
            "https://app.test/api/usersXactive=yes",
            "https://app.test/api/users?active=x/y",
            "https://app.test/v2/users?active=yes",
            "prefixhttps://app.test/api/users?active=yes",
        ] {
            assert!(!pattern.matches(bad), "{bad}");
        }
        assert!(UrlMatcher::glob("**/api/**")
            .unwrap()
            .matches("https://app.test/api/users/1"));
        assert!(UrlMatcher::glob(r"**/literal\*\{x\}")
            .unwrap()
            .matches("https://app.test/literal*{x}"));
        assert!(
            !UrlMatcher::exact("https://app.test/users").matches("https://app.test/users/archive")
        );
        assert!(UrlMatcher::regex(r"/users/\d+$")
            .unwrap()
            .matches("https://app.test/users/42"));
    }
    #[test]
    fn invalid_patterns_and_relative_resolution_are_explicit() {
        for bad in ["{unfinished", "extra}", "escape\\"] {
            assert!(UrlMatcher::glob(bad).is_err());
        }
        assert!(UrlMatcher::regex("[").is_err());
        let relative = UrlMatcher::exact("/users")
            .resolved(|path| Ok(format!("https://app.test{path}")))
            .unwrap();
        assert!(relative.matches("https://app.test/users"));
        UrlMatcher::exact("https://app.test/users")
            .resolved(|_| panic!("absolute URL must not resolve"))
            .unwrap();
    }
}
