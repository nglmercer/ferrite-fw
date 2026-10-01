//! Package exports resolution.

/// Resolve a package `exports`/`imports` value for `subpath` under `conditions`.
///
/// Returns the target path (e.g. `./dist/index.js`) when matched.
pub fn resolve_exports(
    exports: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
) -> Option<String> {
    match resolve_target(exports, subpath, conditions, true) {
        Resolution::Target(target) => Some(target),
        Resolution::Blocked | Resolution::NoMatch => None,
    }
}

// A selected null target must stop conditional fallback. An unmatched
// condition is different: the enclosing object can try its next key.
enum Resolution {
    Target(String),
    Blocked,
    NoMatch,
}

fn resolve_target(
    value: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
    top: bool,
) -> Resolution {
    match value {
        serde_json::Value::Null => Resolution::Blocked,
        serde_json::Value::String(target) => {
            if !top || subpath == "." || subpath.is_empty() {
                Resolution::Target(target.clone())
            } else {
                Resolution::NoMatch
            }
        }
        serde_json::Value::Array(items) => {
            let mut last = if items.is_empty() {
                Resolution::Blocked
            } else {
                Resolution::NoMatch
            };
            for item in items {
                match resolve_target(item, subpath, conditions, false) {
                    target @ Resolution::Target(_) => return target,
                    Resolution::Blocked => last = Resolution::Blocked,
                    Resolution::NoMatch => (),
                }
            }
            last
        }
        serde_json::Value::Object(map) => {
            let has_subpath_keys = map
                .keys()
                .any(|key| key.starts_with('.') || key.starts_with('#'));
            if top && has_subpath_keys {
                if let Some(target) = map.get(subpath) {
                    return resolve_target(target, subpath, conditions, false);
                }
                let mut patterns: Vec<&String> = map
                    .keys()
                    .filter(|key| key.matches('*').count() == 1)
                    .collect();
                patterns.sort_by_key(|key| std::cmp::Reverse((key.find('*').unwrap(), key.len())));
                for pattern in patterns {
                    if let Some(captured) = match_pattern(pattern, subpath) {
                        return match resolve_target(&map[pattern], ".", conditions, false) {
                            Resolution::Target(target) => {
                                Resolution::Target(target.replace('*', &captured))
                            }
                            result => result,
                        };
                    }
                }
                Resolution::NoMatch
            } else {
                if top && subpath != "." && !subpath.is_empty() {
                    return Resolution::NoMatch;
                }
                // Package key insertion order defines priority. Active
                // conditions are a set, not a separate priority ordering.
                for (condition, target) in map {
                    if condition == "default" || conditions.contains(condition) {
                        match resolve_target(target, subpath, conditions, false) {
                            Resolution::NoMatch => (),
                            selected => return selected,
                        }
                    }
                }
                Resolution::NoMatch
            }
        }
        _ => Resolution::NoMatch,
    }
}

/// Match a `*` subpath pattern, returning the captured text.
pub(crate) fn match_pattern(pattern: &str, subpath: &str) -> Option<String> {
    let (prefix, suffix) = pattern.split_once('*')?;
    if subpath.len() >= pattern.len() && subpath.starts_with(prefix) && subpath.ends_with(suffix) {
        let end = subpath.len() - suffix.len();
        // Prefix and suffix may overlap (e.g. `abc*abc` vs `abc`), which
        // would make the capture range `prefix.len()..end` inverted.
        if prefix.len() > end {
            return None;
        }
        Some(subpath[prefix.len()..end].to_string())
    } else {
        None
    }
}
