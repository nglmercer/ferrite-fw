//! Node-style file probing.

use crate::types::*;
use std::path::Path;
use std::path::PathBuf;

/// Probe exact path, `+extensions`, and directory `index.*`.
pub(crate) fn probe_file(file: &Path, extensions: &[String]) -> Option<PathBuf> {
    if file.is_file() {
        return Some(file.to_path_buf());
    }
    for candidate in file_probe_candidates(file, extensions).into_iter().skip(1) {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Extension-less file that exists (e.g. LICENSE-style or extensionless bin).
    if file.exists() && !file.is_dir() {
        return Some(file.to_path_buf());
    }
    None
}

/// Ordered paths used by local file resolution, including not-yet-created paths.
pub(crate) fn file_probe_candidates(file: &Path, extensions: &[String]) -> Vec<PathBuf> {
    let mut candidates = vec![file.to_path_buf()];
    candidates.extend(
        extensions
            .iter()
            .map(|ext| PathBuf::from(format!("{}{ext}", file.to_string_lossy()))),
    );
    for index in ["index", "main"] {
        candidates.extend(
            extensions
                .iter()
                .map(|ext| file.join(format!("{index}{ext}"))),
        );
    }
    candidates
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

/// Missing manifests permit legacy index resolution; unreadable or malformed
/// manifests must not silently disable package encapsulation.
pub(crate) fn read_package_json(dir: &Path) -> ferrite_core::Result<Option<PackageJson>> {
    let path = dir.join("package.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ferrite_core::FerriteError::Resolve(format!(
                "cannot read package manifest {}: {error}",
                path.display()
            )))
        }
    };
    serde_json::from_str(&text).map(Some).map_err(|error| {
        ferrite_core::FerriteError::Resolve(format!(
            "invalid package manifest {}: {error}",
            path.display()
        ))
    })
}
