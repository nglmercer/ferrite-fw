//! Package exports resolution.

/// Resolve a package `exports`/`imports` value for `subpath` under `conditions`.
///
/// Returns the target path (e.g. `./dist/index.js`) when matched.
pub fn resolve_exports(
    exports: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
) -> Option<String> {
    resolve_exports_inner(exports, subpath, conditions, true)
}

pub(crate) fn resolve_exports_inner(
    value: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
    top: bool,
) -> Option<String> {
    match value {
        serde_json::Value::String(target) => {
            if !top || subpath == "." || subpath.is_empty() {
                Some(target.clone())
            } else {
                None
            }
        }
        serde_json::Value::Array(items) => {
            // First matching entry wins; `null` entries are skipped.
            items.iter().find_map(|item| {
                if item.is_null() {
                    None
                } else {
                    resolve_exports_inner(item, subpath, conditions, false)
                }
            })
        }
        serde_json::Value::Object(map) => {
            // `#`-prefixed keys are `imports` subpath keys; condition names
            // (`import`, `default`, ...) never start with `.` or `#`.
            let has_subpath_keys = map
                .keys()
                .any(|key| key.starts_with('.') || key.starts_with('#'));
            if top && has_subpath_keys {
                // Subpath map: exact match, then `*` patterns (longest first).
                if let Some(target) = map.get(subpath) {
                    if target.is_null() {
                        return None;
                    }
                    return resolve_exports_inner(target, subpath, conditions, false);
                }
                let mut patterns: Vec<&String> =
                    map.keys().filter(|key| key.contains('*')).collect();
                patterns.sort_by_key(|key| std::cmp::Reverse(key.len()));
                for pattern in patterns {
                    if let Some(captured) = match_pattern(pattern, subpath) {
                        let target = &map[pattern];
                        if target.is_null() {
                            return None;
                        }
                        return resolve_target_pattern(target, &captured, conditions);
                    }
                }
                None
            } else {
                // Conditional map: first matching condition wins.
                for condition in conditions
                    .iter()
                    .chain(std::iter::once(&"default".to_string()))
                {
                    if let Some(target) = map.get(condition) {
                        if target.is_null() {
                            return None;
                        }
                        if let Some(resolved) =
                            resolve_exports_inner(target, subpath, conditions, false)
                        {
                            return Some(resolved);
                        }
                    }
                }
                None
            }
        }
        _ => None,
    }
}

/// Match a `*` subpath pattern, returning the captured text.
pub(crate) fn match_pattern(pattern: &str, subpath: &str) -> Option<String> {
    let (prefix, suffix) = pattern.split_once('*')?;
    if subpath.starts_with(prefix) && subpath.ends_with(suffix) {
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

/// Substitute `*` captures into a target value.
pub(crate) fn resolve_target_pattern(
    target: &serde_json::Value,
    captured: &str,
    conditions: &[String],
) -> Option<String> {
    match target {
        serde_json::Value::String(template) => Some(template.replace('*', captured)),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_target_pattern(item, captured, conditions)),
        serde_json::Value::Object(_) => {
            // Conditional wrapping a pattern target.
            for condition in conditions
                .iter()
                .chain(std::iter::once(&"default".to_string()))
            {
                if let Some(nested) = target.get(condition) {
                    if let Some(resolved) = resolve_target_pattern(nested, captured, conditions) {
                        return Some(resolved);
                    }
                }
            }
            None
        }
        _ => None,
    }
}
