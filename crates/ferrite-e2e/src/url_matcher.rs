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
    Glob {
        source: String,
        pattern: regex::Regex,
    },
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
        let compiled = regex::Regex::new(&expression)
            .map_err(|e| E2eError::Config(format!("invalid URL glob: {e}")))?;
        Ok(Self(Kind::Glob {
            source: pattern.to_owned(),
            pattern: compiled,
        }))
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
            Kind::Pattern(pattern) | Kind::Glob { pattern, .. } => pattern.is_match(url),
        }
    }
    pub(crate) fn resolved(
        &self,
        resolve: impl FnOnce(&str) -> E2eResult<String>,
    ) -> E2eResult<Self> {
        match &self.0 {
            Kind::Exact(url) => {
                let resolved = match reqwest::Url::parse(url) {
                    Ok(url) => url.to_string(),
                    Err(_) => resolve(url)?,
                };
                Ok(Self::exact(resolved))
            }
            Kind::Glob { source, .. }
                if !source.starts_with('*')
                    && !["about:", "data:", "chrome:", "edge:", "file:"]
                        .iter()
                        .any(|p| source.starts_with(p)) =>
            {
                let resolved = resolve_glob(source, resolve)?;
                Self::glob(&resolved)
            }
            _ => Ok(self.clone()),
        }
    }

    pub(crate) fn identity(&self) -> (u8, &str) {
        match &self.0 {
            Kind::Exact(value) => (0, value),
            Kind::Contains(value) => (1, value),
            Kind::Pattern(value) => (2, value.as_str()),
            Kind::Glob { source, .. } => (3, source),
        }
    }
    pub(crate) fn description(&self) -> String {
        let (kind, source) = self.identity();
        format!(
            "URL {}: {source}",
            ["exact", "contains", "regex", "glob"][usize::from(kind)]
        )
    }
}

impl PartialEq for UrlMatcher {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}
impl Eq for UrlMatcher {}

/// Protect glob syntax before URL resolution/normalization; restore it after
/// resolving relative paths. Host/scheme normalization remains case insensitive.
fn resolve_glob(
    source: &str,
    resolve: impl FnOnce(&str) -> E2eResult<String>,
) -> E2eResult<String> {
    let mut salt = 0;
    while source.contains(&format!("ferriteglob{salt}token")) {
        salt += 1;
    }
    let prefix = format!("ferriteglob{salt}token");
    let mut protected: Vec<(String, String, bool)> = Vec::new();
    let mut segments = Vec::new();
    for (index, token) in source.split('/').enumerate() {
        if index == 0 && token.ends_with(':') && (token.contains('*') || token.contains('{')) {
            protected.push(("http:".into(), token.into(), true));
            segments.push("http:".to_owned());
        } else if token
            .chars()
            .any(|c| matches!(c, '*' | '?' | '{' | '}' | '\\'))
        {
            let mut protect = |part: &str, question: bool| {
                if part.is_empty() {
                    return String::new();
                }
                let marker = format!("{}{}end", prefix, protected.len());
                let marker = if question {
                    format!("?{marker}")
                } else {
                    marker
                };
                protected.push((marker.clone(), part.to_owned(), false));
                marker
            };
            let token = match token.find('?') {
                Some(pos) => format!(
                    "{}{}",
                    protect(&token[..pos], false),
                    protect(&token[pos..], true)
                ),
                None => protect(token, false),
            };
            segments.push(token);
        } else {
            segments.push(token.to_owned());
        }
    }
    let candidate = segments.join("/");
    let resolved = match reqwest::Url::parse(&candidate) {
        Ok(url) => url.to_string(),
        Err(_) => resolve(&candidate)?,
    };
    let parsed = reqwest::Url::parse(&resolved)
        .map_err(|e| E2eError::Config(format!("URL glob resolution failed: {e}")))?;
    let host = parsed.host_str().unwrap_or_default().to_owned();
    let mut resolved = parsed.to_string();
    for (marker, original, scheme) in protected {
        if resolved.matches(&marker).count() != 1 {
            return Err(E2eError::Config(
                "URL glob placeholder collides with its base URL".into(),
            ));
        }
        let original = if scheme || host.contains(&marker) {
            original.to_lowercase()
        } else {
            original
        };
        resolved = resolved.replacen(&marker, &original, 1);
    }
    Ok(resolved)
}

pub(crate) fn legacy_glob(pattern: &str) -> E2eResult<globset::GlobMatcher> {
    globset::Glob::new(pattern)
        .map(|glob| glob.compile_matcher())
        .map_err(|e| E2eError::Config(format!("invalid legacy URL glob {pattern:?}: {e}")))
}
pub(crate) enum HarFilter {
    Shared(UrlMatcher),
    Legacy(globset::GlobMatcher),
}
impl HarFilter {
    pub(crate) fn matches(&self, url: &str) -> bool {
        match self {
            Self::Shared(matcher) => matcher.matches(url),
            Self::Legacy(matcher) => matcher.is_match(url),
        }
    }
}
pub(crate) fn har_filter(
    options: &crate::RouteFromHarOptions,
    resolve: impl FnOnce(&str) -> E2eResult<String>,
) -> E2eResult<Option<HarFilter>> {
    match (&options.url_filter, &options.url_matcher) {
        (Some(_), Some(_)) => Err(E2eError::Config(
            "HAR url_filter and url_matcher are mutually exclusive".into(),
        )),
        (Some(glob), None) => Ok(Some(HarFilter::Legacy(legacy_glob(glob)?))),
        (None, Some(matcher)) => Ok(Some(HarFilter::Shared(matcher.resolved(resolve)?))),
        (None, None) => Ok(None),
    }
}
pub(crate) fn prepare_rules(
    rules: Vec<crate::RouteRule>,
    resolve: impl Fn(&str) -> E2eResult<String>,
) -> E2eResult<Vec<crate::RouteRule>> {
    rules
        .into_iter()
        .map(|mut rule| {
            if let Some(matcher) = &rule.matcher {
                rule.matcher = Some(matcher.resolved(&resolve)?);
            } else {
                legacy_glob(&rule.pattern)?;
            }
            Ok(rule)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolved_matchers_match_pinned_playwright_url_cases() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../scripts/e2e-conformance/url-reference.json"
        ))
        .unwrap();
        assert_eq!(reference["playwright"], "1.63.0");
        let base = reqwest::Url::parse("http://127.0.0.1:34567/base/").unwrap();
        let origin = base.origin().ascii_serialization();
        for case in reference["cases"].as_array().unwrap() {
            let source = case["pattern"]
                .as_str()
                .unwrap()
                .replace("$ORIGIN", &origin);
            let matcher = match case["kind"].as_str().unwrap() {
                "glob" => UrlMatcher::glob(&source).unwrap(),
                "exact" => UrlMatcher::exact(&source),
                "regex" => UrlMatcher::regex(&source).unwrap(),
                kind => panic!("{kind}"),
            }
            .resolved(|source| Ok(base.join(source).unwrap().to_string()))
            .unwrap();
            let actual = base.join(case["path"].as_str().unwrap()).unwrap();
            assert_eq!(
                matcher.matches(actual.as_str()),
                case["matches"].as_bool().unwrap(),
                "{case}"
            );
        }
        for case in reference["invalid_globs"].as_array().unwrap() {
            assert!(case["rejected"].as_bool().unwrap());
            assert!(UrlMatcher::glob(case["pattern"].as_str().unwrap()).is_err());
        }
    }
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
        let absolute = UrlMatcher::glob("HTTP://EXAMPLE.COM/API/{one,two}*")
            .unwrap()
            .resolved(|_| panic!("absolute globs do not need a base URL"))
            .unwrap();
        assert!(absolute.matches("http://example.com/API/one"));
        assert!(!absolute.matches("http://example.com/api/one"));
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
