//! Package version resolution and extraction.

use crate::metadata::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::path::Path;

/// Resolve the best version for `range` (tag, exact, or semver range).
pub fn resolve_version(metadata: &RegistryMetadata, range: &str) -> Result<String> {
    // Dist-tag (latest, next, ...).
    if let Some(tagged) = metadata.dist_tags.get(range) {
        return Ok(tagged.clone());
    }
    // Exact version.
    if metadata.versions.contains_key(range) {
        return Ok(range.to_string());
    }
    let requirement = node_semver::Range::parse(range)
        .map_err(|error| FerriteError::Npm(format!("invalid version range `{range}`: {error}")))?;
    let mut best: Option<node_semver::Version> = None;
    for version in metadata.versions.keys() {
        let Ok(parsed) = node_semver::Version::parse(version) else {
            continue;
        };
        if requirement.satisfies(&parsed) && best.as_ref().is_none_or(|current| parsed > *current) {
            best = Some(parsed);
        }
    }
    best.map(|version| version.to_string()).ok_or_else(|| {
        FerriteError::Npm(format!(
            "no version of `{}` satisfies `{range}`",
            metadata.name
        ))
    })
}

/// Verify `integrity` (`sha512-<base64>` / `sha1-<base64>`) for `bytes`.
pub fn verify_integrity(bytes: &[u8], integrity: Option<&str>) -> Result<()> {
    let Some(integrity) = integrity else {
        return Ok(());
    };
    let Some((algorithm, expected)) = integrity.split_once('-') else {
        return Err(FerriteError::Npm(format!(
            "bad integrity value `{integrity}`"
        )));
    };
    use base64::Engine as _;
    let expected = base64::engine::general_purpose::STANDARD
        .decode(expected.trim())
        .map_err(|error| FerriteError::Npm(format!("bad integrity base64: {error}")))?;
    let actual: Vec<u8> = match algorithm {
        "sha512" => {
            use sha2::Digest as _;
            sha2::Sha512::digest(bytes).to_vec()
        }
        "sha384" => {
            use sha2::Digest as _;
            sha2::Sha384::digest(bytes).to_vec()
        }
        "sha256" => {
            use sha2::Digest as _;
            sha2::Sha256::digest(bytes).to_vec()
        }
        other => {
            return Err(FerriteError::Npm(format!(
                "unsupported integrity algorithm `{other}`"
            )));
        }
    };
    if actual != expected {
        return Err(FerriteError::Npm("tarball integrity mismatch".to_string()));
    }
    Ok(())
}

/// Extract a gzip tarball into `dest`, stripping the `package/` prefix.
///
/// Parent directories are created explicitly: real-world tarballs (e.g.
/// `left-pad`) often omit directory entries, and `Entry::unpack` does not
/// create missing parents.
pub fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    std::fs::create_dir_all(dest)?;
    for entry in archive
        .entries()
        .map_err(|error| FerriteError::Npm(error.to_string()))?
    {
        let mut entry = entry.map_err(|error| FerriteError::Npm(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| FerriteError::Npm(error.to_string()))?
            .into_owned();
        // npm tarballs have a single top-level directory, whose name is
        // not necessarily `package` (DefinitelyTyped packages use their name).
        let mut components = path.components();
        let Some(std::path::Component::Normal(_)) = components.next() else {
            return Err(FerriteError::Npm(format!(
                "invalid tarball root: {}",
                path.display()
            )));
        };
        let relative = components.as_path();
        if relative.as_os_str().is_empty() {
            continue;
        }
        // Refuse path traversal outside `dest`.
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(FerriteError::Npm(format!(
                "tarball entry escapes package dir: {}",
                relative.display()
            )));
        }
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() && !entry_type.is_dir() {
            return Err(FerriteError::Npm(format!(
                "unsupported tarball link/device entry {}; publish regular files instead",
                relative.display()
            )));
        }
        let target = dest.join(relative);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&target)
            .map_err(|error| FerriteError::Npm(format!("unpack {}: {error}", target.display())))?;
    }
    Ok(())
}

/// npm-compatible range matching. Tags must be matched to a recorded specifier,
/// never treated as a wildcard for arbitrary installed packages.
pub fn range_satisfied(version: &str, range: &str) -> bool {
    match (
        node_semver::Range::parse(range),
        node_semver::Version::parse(version),
    ) {
        (Ok(range), Ok(version)) => range.satisfies(&version),
        _ => false,
    }
}
