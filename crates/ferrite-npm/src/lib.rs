//! npm without Node.js (spec §16, §17, §81).
//!
//! Registry client, semver resolution, integrity-checked tarball installs
//! into `.ferrite/npm/packages/`, `ferrite.lock` lockfiles, and
//! `package.json` compatibility (read/write, never requires npm itself).

mod client;
#[cfg(test)]
mod graph_tests;
mod installer;
mod lockfile;
mod manifest;
mod metadata;
mod package;
mod package_json;
mod projection;
pub use projection::{
    project_editor_dependencies, project_node_modules, validate_editor_destination,
};
mod spec;

pub use client::RegistryClient;
pub use installer::{manifest_requests, Installer};
pub use lockfile::{
    validate_package_name, LockedImporter, LockedPackage, Lockfile, LOCKFILE_VERSION,
};
pub use manifest::{classify_package, PackageManifest, PackageRuntime, DEFAULT_REGISTRY};
pub use metadata::{PeerDependencyMeta, RegistryDist, RegistryMetadata, RegistryVersion};
pub use package::{extract_tarball, range_satisfied, resolve_version, verify_integrity};
pub use package_json::JsPackageJson;
pub use spec::parse_spec;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, HashMap};

    fn metadata() -> RegistryMetadata {
        RegistryMetadata {
            name: "pkg".to_string(),
            dist_tags: HashMap::from([("latest".to_string(), "2.0.0".to_string())]),
            versions: ["1.0.0", "1.2.3", "2.0.0"]
                .into_iter()
                .map(|version| {
                    (
                        version.to_string(),
                        RegistryVersion {
                            name: "pkg".to_string(),
                            version: version.to_string(),
                            dependencies: HashMap::new(),
                            peer_dependencies: HashMap::new(),
                            peer_dependencies_meta: HashMap::new(),
                            optional_dependencies: HashMap::new(),
                            dist: RegistryDist {
                                tarball: String::new(),
                                integrity: None,
                                shasum: None,
                            },
                            os: None,
                            cpu: None,
                            binary: None,
                            gypfile: None,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn resolves_tags_and_ranges() {
        let meta = metadata();
        assert_eq!(resolve_version(&meta, "latest").unwrap(), "2.0.0");
        assert_eq!(resolve_version(&meta, "^1.0.0").unwrap(), "1.2.3");
        assert_eq!(resolve_version(&meta, "1.0.0").unwrap(), "1.0.0");
        assert!(resolve_version(&meta, "^3.0.0").is_err());
    }

    #[test]
    fn parses_specs() {
        assert_eq!(
            parse_spec("react"),
            ("react".to_string(), "latest".to_string())
        );
        assert_eq!(
            parse_spec("three@^0.180.0"),
            ("three".to_string(), "^0.180.0".to_string())
        );
        assert_eq!(
            parse_spec("@scope/name@1.2.3"),
            ("@scope/name".to_string(), "1.2.3".to_string())
        );
    }

    #[test]
    fn lockfile_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ferrite-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut lock = Lockfile::default();
        lock.upsert(LockedPackage {
            name: "react".to_string(),
            version: "19.1.0".to_string(),
            source: "npm".to_string(),
            integrity: Some("sha512-abc".to_string()),
            dependencies: BTreeMap::new(),
            ..Default::default()
        });
        lock.write(&dir.join("ferrite.lock")).unwrap();
        let loaded = Lockfile::read(&dir.join("ferrite.lock")).unwrap();
        assert_eq!(loaded.find("react").unwrap().version, "19.1.0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn integrity_checks() {
        use sha2::Digest as _;
        let bytes = b"hello";
        let digest = sha2::Sha512::digest(bytes);
        use base64::Engine as _;
        let integrity = format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        );
        assert!(verify_integrity(bytes, Some(&integrity)).is_ok());
        assert!(verify_integrity(b"other", Some(&integrity)).is_err());
    }

    #[test]
    fn extracts_tarball_without_dir_entries() {
        // Regression: real tarballs (left-pad) omit `perf/` dir entries.
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let contents = b"console.log(1);";
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "package/perf/deep.js", &contents[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let mut gz = Vec::new();
        {
            use std::io::Write as _;
            let mut encoder = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::fast());
            encoder.write_all(&tar_bytes).unwrap();
            encoder.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("ferrite-tar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        extract_tarball(&gz, &dir).unwrap();
        assert!(dir.join("perf/deep.js").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn classifies_packages() {
        let esm = PackageManifest {
            package_type: Some("module".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_package(&esm), PackageRuntime::BrowserEsm);
        let native = PackageManifest {
            gypfile: Some("binding.gyp".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_package(&native), PackageRuntime::NodeRequired);
    }
}
