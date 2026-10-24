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

/// Verify an SRI set using the strongest supported algorithm present.
/// Multiple digests of that strength are alternatives; weaker matches never
/// override a mismatch at a stronger strength. Unknown algorithms are ignored
/// only when a supported digest is also present.
pub fn verify_integrity(bytes: &[u8], integrity: Option<&str>) -> Result<()> {
    let Some(integrity) = integrity else {
        return Err(FerriteError::Npm(
            "tarball has no integrity metadata; require registry integrity or a legacy shasum"
                .into(),
        ));
    };
    use base64::Engine as _;
    use sha2::Digest as _;
    let mut candidates = Vec::new();
    for token in integrity.split_ascii_whitespace() {
        let Some((algorithm, encoded)) = token.split_once('-') else {
            return Err(FerriteError::Npm(format!("bad integrity value `{token}`")));
        };
        let strength = match algorithm {
            "sha512" => 4,
            "sha384" => 3,
            "sha256" => 2,
            "sha1" => 1,
            _ => continue,
        };
        let encoded = encoded.split('?').next().unwrap_or(encoded);
        candidates.push((strength, encoded));
    }
    let strength = candidates
        .iter()
        .map(|(strength, _)| *strength)
        .max()
        .ok_or_else(|| {
            FerriteError::Npm("integrity contains no supported hash algorithm".into())
        })?;
    let actual = match strength {
        4 => sha2::Sha512::digest(bytes).to_vec(),
        3 => sha2::Sha384::digest(bytes).to_vec(),
        2 => sha2::Sha256::digest(bytes).to_vec(),
        _ => sha1::Sha1::digest(bytes).to_vec(),
    };
    for (_, encoded) in candidates
        .into_iter()
        .filter(|(candidate, _)| *candidate == strength)
    {
        // A malformed alternative cannot hide a later valid digest.
        if let Ok(expected) = base64::engine::general_purpose::STANDARD.decode(encoded) {
            if actual == expected {
                return Ok(());
            }
        }
    }
    Err(FerriteError::Npm(
        "tarball integrity mismatch or malformed digest".into(),
    ))
}

/// Normalize registry integrity, retaining legacy SHA-1 metadata in the lock.
pub(crate) fn distribution_integrity(dist: &RegistryDist) -> Result<String> {
    if let Some(integrity) = &dist.integrity {
        if !integrity.trim().is_empty() {
            return Ok(integrity.clone());
        }
    }
    let shasum = dist.shasum.as_deref().ok_or_else(|| {
        FerriteError::Npm("registry tarball has neither integrity nor shasum".into())
    })?;
    if shasum.len() != 40 || !shasum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(FerriteError::Npm(
            "registry tarball has a malformed SHA-1 shasum".into(),
        ));
    }
    let bytes: Vec<u8> = (0..40)
        .step_by(2)
        .map(|index| u8::from_str_radix(&shasum[index..index + 2], 16).expect("validated hex"))
        .collect();
    use base64::Engine as _;
    Ok(format!(
        "sha1-{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
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

#[cfg(test)]
mod integrity_tests {
    use super::*;
    use base64::Engine as _;
    use sha2::Digest as _;

    #[test]
    fn strongest_sri_alternatives_prevent_weaker_hash_downgrades() {
        let bytes = b"verified tarball";
        let encode = |bytes| base64::engine::general_purpose::STANDARD.encode(bytes);
        let sha1 = format!("sha1-{}", encode(sha1::Sha1::digest(bytes).to_vec()));
        let sha512 = format!("sha512-{}", encode(sha2::Sha512::digest(bytes).to_vec()));
        let wrong = format!(
            "sha512-{}",
            encode(sha2::Sha512::digest(b"different").to_vec())
        );
        assert!(verify_integrity(bytes, Some(&sha1)).is_ok());
        assert!(verify_integrity(bytes, Some(&format!("{sha1} {wrong}"))).is_err());
        assert!(verify_integrity(bytes, Some(&format!("{wrong} {sha512}"))).is_ok());
        assert!(verify_integrity(bytes, Some(&format!("sha999-unknown {sha512}?option"))).is_ok());
        assert!(verify_integrity(bytes, Some("sha999-unknown")).is_err());
        assert!(verify_integrity(bytes, Some("")).is_err());
        assert!(verify_integrity(bytes, None).is_err());
    }

    #[test]
    fn legacy_shasum_is_converted_and_verified() {
        let bytes = b"legacy tarball";
        let hex: String = sha1::Sha1::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let mut dist = RegistryDist {
            tarball: "unused".into(),
            integrity: None,
            shasum: Some(hex),
        };
        let normalized = distribution_integrity(&dist).unwrap();
        assert!(normalized.starts_with("sha1-"));
        assert!(verify_integrity(bytes, Some(&normalized)).is_ok());
        assert!(verify_integrity(b"tampered", Some(&normalized)).is_err());
        dist.shasum = Some("not hex".into());
        assert!(distribution_integrity(&dist).is_err());
        dist.shasum = None;
        assert!(distribution_integrity(&dist).is_err());
    }
}
