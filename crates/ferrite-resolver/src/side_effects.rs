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
            for pattern in globs {
                // Invalid patterns cannot establish absence of side effects.
                match match_glob(pattern, &relative) {
                    Some(true) => return Some(true),
                    Some(false) => (),
                    None => return None,
                }
            }
            Some(false)
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

/// A slashless package pattern applies to a basename at any depth. A single
/// star never crosses a path separator; globstars, alternatives and character
/// classes follow the same glob engine used elsewhere in Ferrite.
fn match_glob(pattern: &str, path: &str) -> Option<bool> {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    let pattern = if pattern.contains('/') {
        pattern.to_string()
    } else {
        format!("**/{pattern}")
    };
    globset::GlobBuilder::new(&pattern)
        .literal_separator(true)
        .build()
        .ok()
        .map(|glob| glob.compile_matcher().is_match(path))
}

#[cfg(test)]
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    match_glob(pattern, path).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_globs_are_path_anchored_and_invalid_patterns_are_conservative() {
        for (pattern, path) in [
            ("*.css", "style.css"),
            ("*.css", "deep/style.css"),
            ("**/*.css", "style.css"),
            ("./src/**/*.css", "src/deep/style.css"),
            ("./{src,lib}/polyfill[0-9].js", "lib/polyfill2.js"),
        ] {
            assert_eq!(match_glob(pattern, path), Some(true), "{pattern}: {path}");
        }
        for (pattern, path) in [
            ("**/*.css", "style.css/nested.js"),
            ("src/*.js", "src/deep/file.js"),
            ("src/**/*.css", "other/src/style.css"),
            ("*.css", "style.css.js"),
        ] {
            assert_eq!(match_glob(pattern, path), Some(false), "{pattern}: {path}");
        }
        let package: PackageJson =
            serde_json::from_value(serde_json::json!({"sideEffects":["[invalid"]})).unwrap();
        assert_eq!(
            side_effects_for(
                &package,
                Path::new("/package"),
                Path::new("/package/main.js")
            ),
            None
        );
    }
}
