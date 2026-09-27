//! Package side-effects.

use crate::types::*;
use std::path::Path;

/// Look up `sideEffects` for a file inside a package.
pub(crate) fn side_effects_for(pkg: &PackageJson, dir: &Path, file: &Path) -> Option<bool> {
    match &pkg.side_effects {
        None => None,
        Some(SideEffects::Bool(value)) => Some(*value),
        Some(SideEffects::Globs(globs)) => {
            let relative = file
                .strip_prefix(dir)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            // Conservative: side effects unless a `!`-negated glob matches...
            // Simplified: true when any positive glob matches the extension set.
            let matched = globs.iter().any(|glob| glob_match(glob, &relative));
            Some(matched)
        }
    }
}

/// Best-effort `sideEffects` lookup by walking up to the owning package.
pub(crate) fn package_side_effects(file: &Path) -> Option<bool> {
    let mut dir = file.parent()?.to_path_buf();
    loop {
        let candidate = dir.join("package.json");
        if candidate.exists() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if let Ok(pkg) = serde_json::from_str::<PackageJson>(&text) {
                    return side_effects_for(&pkg, &dir, file);
                }
            }
            return None;
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Tiny glob matcher (`*`, `**`, `?`, trailing `/...`).
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches("./");
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        let mut rest = path;
        for (i, part) in parts.iter().enumerate() {
            let part = part.trim_matches('/');
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                if !segment_match(part, rest) {
                    // prefix piece must match at the start (up to `/`)
                    if let Some(pos) = rest.find('/') {
                        if !segment_match(part, &rest[..pos]) {
                            return false;
                        }
                        rest = &rest[pos + 1..];
                    } else {
                        return segment_match(part, rest);
                    }
                } else if let Some(pos) = rest.find('/') {
                    rest = &rest[pos + 1..];
                }
            } else if i == parts.len() - 1 {
                return rest.split('/').any(|segment| segment_match(part, segment))
                    || segment_match(part, rest);
            }
        }
        return true;
    }
    if pattern.contains('/') {
        segment_match(pattern, path)
    } else {
        path.split('/')
            .any(|segment| segment_match(pattern, segment))
    }
}

pub(crate) fn segment_match(pattern: &str, text: &str) -> bool {
    let (mut px, mut tx) = (pattern.as_bytes(), text.as_bytes());
    let (mut star, mut mark): (Option<&[u8]>, &[u8]) = (None, tx);
    while !tx.is_empty() {
        match px.first() {
            Some(b'*') => {
                star = Some(&px[1..]);
                px = &px[1..];
                mark = tx;
            }
            Some(b'?') | Some(_) if px.first() == Some(&tx[0]) || px.first() == Some(&b'?') => {
                px = &px[1..];
                tx = &tx[1..];
            }
            _ => {
                if let Some(saved) = star {
                    px = saved;
                    mark = &mark[1..];
                    tx = mark;
                    if mark.is_empty() && px.is_empty() {
                        return true;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    while px.first() == Some(&b'*') {
        px = &px[1..];
    }
    px.is_empty()
}
