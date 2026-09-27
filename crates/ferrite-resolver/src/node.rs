//! Node-style file probing.

use crate::types::*;
use std::path::Path;
use std::path::PathBuf;

/// Probe exact path, `+extensions`, and directory `index.*`.
pub(crate) fn probe_file(file: &Path, extensions: &[String]) -> Option<PathBuf> {
    if file.is_file() {
        return Some(file.to_path_buf());
    }
    let with_ext: Option<PathBuf> = extensions.iter().find_map(|ext| {
        let candidate = PathBuf::from(format!("{}{ext}", file.to_string_lossy()));
        candidate.is_file().then_some(candidate)
    });
    if with_ext.is_some() {
        return with_ext;
    }
    if file.is_dir() {
        for index in ["index", "main"] {
            for ext in extensions {
                let candidate = file.join(format!("{index}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    // Extension-less file that exists (e.g. LICENSE-style or extensionless bin).
    if file.exists() && !file.is_dir() {
        return Some(file.to_path_buf());
    }
    None
}

/// Walk up looking for `node_modules/<name>` (compatibility fallback).
pub(crate) fn find_node_modules(base: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = base.to_path_buf();
    loop {
        let candidate = dir.join("node_modules").join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Read and parse a package.json (best effort).
pub(crate) fn read_package_json(dir: &Path) -> Option<PackageJson> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    serde_json::from_str(&text).ok()
}
