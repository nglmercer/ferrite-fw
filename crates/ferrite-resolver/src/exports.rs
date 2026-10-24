//! Checked package exports/imports selection, before filesystem resolution.
use ferrite_core::{FerriteError, Result};
use serde_json::Value;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MapKind {
    Exports,
    Imports,
}

/// Compatibility selector. Internal resolution uses the checked API so invalid
/// configuration and invalid targets remain actionable errors.
pub fn resolve_exports(exports: &Value, subpath: &str, conditions: &[String]) -> Option<String> {
    resolve_package_map(exports, subpath, conditions, MapKind::Exports)
        .ok()
        .flatten()
}
fn invalid(message: impl std::fmt::Display) -> FerriteError {
    FerriteError::Resolve(format!("invalid package map: {message}"))
}

/// Decode path segments before validation, rejecting encoded separators and
/// traversal rather than letting filesystem normalization erase the evidence.
pub(crate) fn decode_map_path(path: &str) -> Result<String> {
    let mut decoded = Vec::new();
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| invalid("incomplete percent escape"))?;
            let hex = std::str::from_utf8(hex).map_err(|_| invalid("invalid percent escape"))?;
            let byte =
                u8::from_str_radix(hex, 16).map_err(|_| invalid("invalid percent escape"))?;
            if matches!(byte, b'/' | b'\\') {
                return Err(invalid("encoded path separator"));
            }
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| invalid("path is not UTF-8"))?;
    if decoded.starts_with('/')
        || decoded.as_bytes().get(1) == Some(&b':')
        || decoded.contains(['\\', '\0', '?', '#'])
        || decoded.chars().any(char::is_control)
    {
        return Err(invalid(
            "unsupported path separator, control character or target query/fragment",
        ));
    }
    if decoded
        .split('/')
        .any(|part| matches!(part, "." | "..") || part.eq_ignore_ascii_case("node_modules"))
    {
        return Err(invalid("path contains traversal or node_modules segments"));
    }
    Ok(decoded)
}
fn target(
    target: &str,
    kind: MapKind,
    capture: Option<&str>,
) -> std::result::Result<String, String> {
    let target = capture.map_or_else(
        || target.to_string(),
        |capture| target.replace('*', capture),
    );
    if let Some(relative) = target.strip_prefix("./") {
        return decode_map_path(relative)
            .map(|path| format!("./{path}"))
            .map_err(|error| error.to_string());
    }
    if kind == MapKind::Imports
        && !target.starts_with(['/', '.', '#'])
        && !target.contains([':', '\\', '%', '?', '#'])
    {
        let (name, subpath) = crate::specifier::split_package(&target);
        ferrite_npm::validate_package_name(&name).map_err(|error| error.to_string())?;
        decode_map_path(subpath.trim_start_matches('/')).map_err(|error| error.to_string())?;
        return Ok(target);
    }
    Err(format!(
        "target `{target}` must start with ./{}",
        if kind == MapKind::Imports {
            " or name a dependency package"
        } else {
            ""
        }
    ))
}

enum Resolution {
    Target(String),
    Blocked,
    NoMatch,
    Invalid(String),
}
fn numeric_key(key: &str) -> bool {
    key.parse::<u32>()
        .is_ok_and(|n| n != u32::MAX && n.to_string() == key)
}
fn select(
    value: &Value,
    conditions: &[String],
    kind: MapKind,
    capture: Option<&str>,
    depth: usize,
) -> Result<Resolution> {
    if depth > 64 {
        return Err(invalid("target nesting exceeds 64 levels"));
    }
    Ok(match value {
        Value::Null => Resolution::Blocked,
        Value::String(value) => match target(value, kind, capture) {
            Ok(value) => Resolution::Target(value),
            Err(error) => Resolution::Invalid(error),
        },
        Value::Array(values) => {
            let mut last = Resolution::Blocked;
            for value in values {
                match select(value, conditions, kind, capture, depth + 1)? {
                    result @ Resolution::Target(_) => return Ok(result),
                    Resolution::NoMatch => (),
                    result => last = result,
                }
            }
            last
        }
        Value::Object(map) => {
            if map.keys().any(|key| numeric_key(key)) {
                return Err(invalid("numeric condition keys are not allowed"));
            }
            for (condition, value) in map {
                if condition == "default" || conditions.contains(condition) {
                    match select(value, conditions, kind, capture, depth + 1)? {
                        Resolution::NoMatch => (),
                        result => return Ok(result),
                    }
                }
            }
            Resolution::NoMatch
        }
        _ => Resolution::Invalid("targets must be strings, objects, arrays or null".into()),
    })
}
pub(crate) fn resolve_package_map(
    value: &Value,
    subpath: &str,
    conditions: &[String],
    kind: MapKind,
) -> Result<Option<String>> {
    let subpath_path = if kind == MapKind::Imports {
        subpath
            .strip_prefix('#')
            .filter(|path| !path.is_empty() && !path.starts_with('/'))
            .ok_or_else(|| invalid("imports keys must start with # followed by a name"))?
    } else if subpath == "." {
        ""
    } else {
        subpath
            .strip_prefix("./")
            .ok_or_else(|| invalid("exports subpaths must start with ./"))?
    };
    decode_map_path(subpath_path)?;
    let selected = if let Value::Object(map) = value {
        let subpath_map = if kind == MapKind::Imports {
            if map
                .keys()
                .any(|key| !key.starts_with('#') || key == "#" || key.starts_with("#/"))
            {
                return Err(invalid("imports keys must be valid # specifiers"));
            }
            true
        } else {
            let paths = map.keys().filter(|key| key.starts_with('.')).count();
            if paths != 0 && paths != map.len() {
                return Err(invalid("exports cannot mix subpath and condition keys"));
            }
            if paths != 0 && map.keys().any(|key| key != "." && !key.starts_with("./")) {
                return Err(invalid("invalid exports subpath key"));
            }
            paths != 0
        };
        if subpath_map {
            if let Some(value) = map.get(subpath) {
                select(value, conditions, kind, None, 0)?
            } else {
                let mut patterns: Vec<_> = map
                    .keys()
                    .filter(|key| key.matches('*').count() == 1)
                    .collect();
                patterns.sort_by_key(|key| std::cmp::Reverse((key.find('*').unwrap(), key.len())));
                let mut selected = Resolution::NoMatch;
                for pattern in patterns {
                    if let Some(capture) = match_pattern(pattern, subpath) {
                        decode_map_path(&capture)?;
                        selected = select(&map[pattern], conditions, kind, Some(&capture), 0)?;
                        break;
                    }
                }
                selected
            }
        } else if subpath == "." {
            select(value, conditions, kind, None, 0)?
        } else {
            Resolution::NoMatch
        }
    } else if kind == MapKind::Imports {
        return Err(invalid("imports must be an object"));
    } else if subpath == "." {
        select(value, conditions, kind, None, 0)?
    } else {
        Resolution::NoMatch
    };
    match selected {
        Resolution::Target(value) => Ok(Some(value)),
        Resolution::Invalid(error) => Err(invalid(error)),
        _ => Ok(None),
    }
}

/// Match one wildcard; retain package-map specificity order at the caller.
pub(crate) fn match_pattern(pattern: &str, subpath: &str) -> Option<String> {
    let (prefix, suffix) = pattern.split_once('*')?;
    if subpath.len() >= pattern.len() && subpath.starts_with(prefix) && subpath.ends_with(suffix) {
        let end = subpath.len() - suffix.len();
        (prefix.len() <= end).then(|| subpath[prefix.len()..end].to_string())
    } else {
        None
    }
}
