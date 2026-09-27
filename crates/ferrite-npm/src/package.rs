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
    let requirement = semver::VersionReq::parse(range)
        .map_err(|error| FerriteError::Npm(format!("invalid version range `{range}`: {error}")))?;
    let mut best: Option<semver::Version> = None;
    for version in metadata.versions.keys() {
        let Ok(parsed) = semver::Version::parse(version) else {
            continue;
        };
        // Stable ranges skip prereleases unless explicitly requested.
        if !parsed.pre.is_empty() && !range.contains('-') {
            continue;
        }
        if requirement.matches(&parsed) && best.as_ref().is_none_or(|current| parsed > *current) {
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
        let relative = path.strip_prefix("package").unwrap_or(&path);
        if relative.as_os_str().is_empty() {
            continue;
        }
        // Refuse path traversal outside `dest`.
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(FerriteError::Npm(format!(
                "tarball entry escapes package dir: {}",
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
