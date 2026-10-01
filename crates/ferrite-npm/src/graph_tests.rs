use crate::*;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

struct Fixture {
    dir: tempfile::TempDir,
    installer: Installer,
    metadata: BTreeMap<String, RegistryMetadata>,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let npm = dir.path().join(".ferrite/npm");
        let client = RegistryClient::new("http://127.0.0.1:9", npm.join("metadata")).unwrap();
        Self {
            dir,
            installer: Installer::new(client, npm),
            metadata: BTreeMap::new(),
        }
    }
    fn package(&mut self, name: &str, version: &str, deps: &[(&str, &str)]) {
        let json = serde_json::to_vec(&serde_json::json!({"name": name, "version": version, "scripts": {"postinstall": "echo forbidden > lifecycle-ran"}})).unwrap();
        let mut archive = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(json.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive
            .append_data(&mut header, "package/package.json", &json[..])
            .unwrap();
        archive.finish().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&archive.into_inner().unwrap()).unwrap();
        let bytes = gzip.finish().unwrap();
        use base64::Engine as _;
        use sha2::Digest as _;
        let integrity = format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(sha2::Sha512::digest(&bytes))
        );
        std::fs::create_dir_all(&self.installer.tarball_cache).unwrap();
        std::fs::write(
            self.installer
                .tarball_cache
                .join(format!("{}-{version}.tgz", name.replace('/', "__"))),
            bytes,
        )
        .unwrap();
        let info = RegistryVersion {
            name: name.into(),
            version: version.into(),
            dependencies: deps
                .iter()
                .map(|(name, range)| ((*name).into(), (*range).into()))
                .collect(),
            peer_dependencies: HashMap::new(),
            peer_dependencies_meta: HashMap::new(),
            optional_dependencies: HashMap::new(),
            dist: RegistryDist {
                tarball: format!("http://127.0.0.1:9/{name}-{version}.tgz"),
                integrity: Some(integrity),
                shasum: None,
            },
            os: None,
            cpu: None,
            binary: None,
            gypfile: None,
        };
        let meta = self
            .metadata
            .entry(name.into())
            .or_insert_with(|| RegistryMetadata {
                name: name.into(),
                dist_tags: HashMap::new(),
                versions: BTreeMap::new(),
            });
        meta.dist_tags.insert("latest".into(), version.into());
        meta.versions.insert(version.into(), info);
    }
    fn save(&self) {
        std::fs::create_dir_all(&self.installer.client.cache_dir).unwrap();
        for (name, meta) in &self.metadata {
            std::fs::write(
                self.installer
                    .client
                    .cache_dir
                    .join(format!("{}.json", name.replace('/', "__"))),
                serde_json::to_vec(meta).unwrap(),
            )
            .unwrap();
        }
    }
    fn manifest(deps: &[(&str, &str)]) -> JsPackageJson {
        JsPackageJson {
            dependencies: deps
                .iter()
                .map(|(name, range)| ((*name).into(), (*range).into()))
                .collect(),
            ..Default::default()
        }
    }
}

#[tokio::test]
async fn conflicting_versions_cycles_and_frozen_reinstall_are_concrete() {
    let mut fixture = Fixture::new();
    fixture.package("a", "1.0.0", &[("shared", "^1"), ("b", "1.0.0")]);
    fixture.package("b", "1.0.0", &[("shared", "^2"), ("a", "1.0.0")]);
    fixture.package("shared", "1.2.0", &[]);
    fixture.package("shared", "2.3.0", &[]);
    fixture.package("@scope/widget", "1.0.0", &[("shared", "^1")]);
    fixture.save();
    let manifest = Fixture::manifest(&[("a", "1.0.0"), ("@scope/widget", "1.0.0")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.package.len(), 5);
    assert_eq!(
        lock.package_by_id("a@1.0.0").unwrap().dependencies["shared"],
        "shared@1.2.0"
    );
    assert_eq!(
        lock.package_by_id("b@1.0.0").unwrap().dependencies["shared"],
        "shared@2.3.0"
    );
    assert_eq!(
        lock.package_by_id("b@1.0.0").unwrap().dependencies["a"],
        "a@1.0.0"
    );
    assert!(
        lock.find("shared").is_none(),
        "name lookup must not choose an arbitrary version"
    );
    assert!(fixture
        .installer
        .installed_dir("@scope/widget", "1.0.0")
        .join("package.json")
        .exists());
    assert!(!fixture.dir.path().join("lifecycle-ran").exists());
    assert!(lock.package.iter().all(|package| !fixture
        .installer
        .store
        .join(package.id())
        .join("lifecycle-ran")
        .exists()));
    let lock_path = fixture.dir.path().join("ferrite.lock");
    lock.write(&lock_path).unwrap();
    let original = std::fs::read(&lock_path).unwrap();
    std::fs::remove_dir_all(&fixture.installer.store).unwrap();
    std::fs::remove_dir_all(&fixture.installer.client.cache_dir).unwrap();
    let snapshot = lock.clone();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, true)
        .await
        .unwrap();
    assert_eq!(lock, snapshot, "frozen installs cannot mutate the graph");
    assert_eq!(std::fs::read(&lock_path).unwrap(), original);
    assert!(lock.package.iter().all(|package| fixture
        .installer
        .store
        .join(package.id())
        .join("package.json")
        .exists()));
    let changed = Fixture::manifest(&[("a", "^1")]);
    assert!(fixture
        .installer
        .install_manifest(&changed, &mut lock, true)
        .await
        .unwrap_err()
        .to_string()
        .contains("requests differ"));
    assert_eq!(lock, snapshot);
}

#[tokio::test]
async fn distinct_peer_contexts_coexist_and_incompatible_peers_fail_transactionally() {
    let mut fixture = Fixture::new();
    fixture.package(
        "legacy",
        "1.0.0",
        &[("runtime", "1.0.0"), ("consumer", "1.0.0")],
    );
    fixture.package(
        "modern",
        "1.0.0",
        &[("runtime", "2.0.0"), ("consumer", "1.0.0")],
    );
    fixture.package("runtime", "1.0.0", &[]);
    fixture.package("runtime", "2.0.0", &[]);
    fixture.package("consumer", "1.0.0", &[]);
    fixture
        .metadata
        .get_mut("consumer")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap()
        .peer_dependencies
        .insert("runtime".into(), "^1 || ^2".into());
    fixture.save();
    let mut lock = Lockfile::default();
    let manifest = Fixture::manifest(&[("legacy", "1.0.0"), ("modern", "1.0.0")]);
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    let consumers: Vec<_> = lock
        .package
        .iter()
        .filter(|package| package.name == "consumer")
        .collect();
    assert_eq!(consumers.len(), 2);
    assert_ne!(consumers[0].id(), consumers[1].id());
    assert_ne!(consumers[0].peers["runtime"], consumers[1].peers["runtime"]);
    for parent in ["legacy@1.0.0", "modern@1.0.0"] {
        let record = lock.package_by_id(parent).unwrap();
        let consumer = lock
            .package_by_id(&record.dependencies["consumer"])
            .unwrap();
        assert_eq!(
            consumer.dependencies["runtime"],
            record.dependencies["runtime"]
        );
    }
    let snapshot = lock.clone();
    fixture
        .metadata
        .get_mut("consumer")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap()
        .peer_dependencies
        .insert("runtime".into(), "^3".into());
    fixture.save();
    let error = fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("needs peer runtime@^3"), "{error}");
    assert_eq!(lock, snapshot);
}

#[tokio::test]
async fn optional_platform_dependencies_are_omitted_and_cached_integrity_is_verified() {
    let mut fixture = Fixture::new();
    fixture.package("app", "1.0.0", &[]);
    fixture.package("unsupported", "1.0.0", &[]);
    fixture
        .metadata
        .get_mut("app")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap()
        .optional_dependencies
        .insert("unsupported".into(), "1.0.0".into());
    fixture
        .metadata
        .get_mut("unsupported")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap()
        .os = Some(vec![format!(
        "!{}",
        if std::env::consts::OS == "macos" {
            "darwin"
        } else {
            std::env::consts::OS
        }
    )]);
    fixture.save();
    let manifest = Fixture::manifest(&[("app", "1.0.0")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.package.len(), 1);
    assert!(lock.package[0].dependencies.is_empty());
    std::fs::remove_dir_all(&fixture.installer.store).unwrap();
    std::fs::write(
        fixture.installer.tarball_cache.join("app-1.0.0.tgz"),
        "corrupt",
    )
    .unwrap();
    let snapshot = lock.clone();
    assert!(fixture
        .installer
        .install_manifest(&manifest, &mut lock, true)
        .await
        .unwrap_err()
        .to_string()
        .contains("integrity mismatch"));
    assert_eq!(lock, snapshot);
}

#[test]
fn legacy_lock_migrates_edges_but_never_invents_lost_versions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ferrite.lock");
    let text = "[[package]]\nname = 'a'\nversion = '1.0.0'\nsource = 'npm'\n[package.dependencies]\nb = '2.0.0'\n[[package]]\nname = 'b'\nversion = '2.0.0'\nsource = 'npm'\n";
    std::fs::write(&path, text).unwrap();
    let lock = Lockfile::read(&path).unwrap();
    assert_eq!(lock.version, 2);
    assert_eq!(
        lock.package_by_id("a@1.0.0").unwrap().dependencies["b"],
        "b@2.0.0"
    );
    assert!(lock.importers.is_empty());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        text,
        "read migration does not write"
    );
    std::fs::write(&path, text.replace("b = '2.0.0'", "b = '^1'")).unwrap();
    assert!(Lockfile::read(&path)
        .unwrap_err()
        .to_string()
        .contains("references missing b@^1"));
    std::fs::write(&path, "version = 999").unwrap();
    assert!(Lockfile::read(&path)
        .unwrap_err()
        .to_string()
        .contains("unsupported ferrite.lock version"));
}

#[test]
fn npm_ranges_and_package_paths_fail_closed() {
    for (version, range, expected) in [
        ("1.2.3", "1.2.3", true),
        ("1.2.4", "1.2.3", false),
        ("2.0.0", "^1 || ^2", true),
        ("1.8.0", ">=1 <2", true),
        ("2.4.0", "1.2.3 - 2.3", false),
        ("2.3.99", "1.2.3 - 2.3", true),
        ("0.3.0", "^0.2.0", false),
        ("1.3.0", "~1.2", false),
        ("1.2.3-beta.2", "^1.2.3-beta.1", true),
        ("1.3.0-beta.1", "^1.2.3-beta.1", false),
        ("1.0.0", "latest", false),
        ("1.0.0", "file:../outside", false),
    ] {
        assert_eq!(
            range_satisfied(version, range),
            expected,
            "{version} {range}"
        );
    }
    for name in [
        "../escape",
        "@scope/../../outside",
        "/absolute",
        "pkg\\escape",
        "@scope/",
        "",
    ] {
        assert!(validate_package_name(name).is_err(), "{name}");
    }
    assert!(validate_package_name("@scope/widget").is_ok());
}

#[tokio::test]
async fn explicit_update_selects_new_version_and_tags_stay_locked() {
    let mut fixture = Fixture::new();
    fixture.package("root", "1.0.0", &[("child", "latest")]);
    fixture.package("child", "1.0.0", &[]);
    fixture.save();
    let manifest = Fixture::manifest(&[("root", "^1")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    fixture.package("child", "2.0.0", &[]);
    fixture.package("root", "1.1.0", &[("child", "latest")]);
    fixture.save();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.importers["."].dependencies["root"], "root@1.0.0");
    assert_eq!(
        lock.package_by_id("root@1.0.0").unwrap().dependencies["child"],
        "child@1.0.0"
    );
    lock.importers
        .get_mut(".")
        .unwrap()
        .dependencies
        .remove("root");
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.importers["."].dependencies["root"], "root@1.1.0");
    assert_eq!(
        lock.package_by_id("root@1.1.0").unwrap().dependencies["child"],
        "child@2.0.0"
    );
}

#[test]
fn non_package_tarball_root_extracts_and_links_are_rejected() {
    fn archive(path: &str, link: bool) -> Vec<u8> {
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(if link { 0 } else { 2 });
        if link {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("../../outside").unwrap();
        }
        header.set_cksum();
        tar.append_data(&mut header, path, if link { &b""[..] } else { &b"{}"[..] })
            .unwrap();
        tar.finish().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&tar.into_inner().unwrap()).unwrap();
        gzip.finish().unwrap()
    }
    let dir = tempfile::tempdir().unwrap();
    extract_tarball(&archive("estree/package.json", false), dir.path()).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("package.json")).unwrap(),
        b"{}"
    );
    assert!(
        extract_tarball(&archive("package/escape", true), dir.path())
            .unwrap_err()
            .to_string()
            .contains("link/device")
    );
}

#[tokio::test]
async fn single_package_api_retains_roots_and_unsupported_fields_fail() {
    let mut fixture = Fixture::new();
    fixture.package("a", "1.0.0", &[("shared", "^1")]);
    fixture.package("b", "1.0.0", &[("shared", "^2")]);
    fixture.package("shared", "1.0.0", &[]);
    fixture.package("shared", "2.0.0", &[]);
    fixture.save();
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install("a", "1", &mut lock, false)
        .await
        .unwrap();
    fixture
        .installer
        .install("b", "1", &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.importers["."].dependencies.len(), 2);
    assert_eq!(
        lock.package_by_id("a@1.0.0").unwrap().dependencies["shared"],
        "shared@1.0.0"
    );
    assert_eq!(
        lock.package_by_id("b@1.0.0").unwrap().dependencies["shared"],
        "shared@2.0.0"
    );
    for (field, value) in [
        ("workspaces", serde_json::json!(["packages/*"])),
        ("optionalDependencies", serde_json::json!({"missing": "1"})),
    ] {
        let mut manifest = Fixture::manifest(&[("a", "1")]);
        manifest.rest.insert(field.into(), value);
        let before = lock.clone();
        assert!(fixture
            .installer
            .install_manifest(&manifest, &mut lock, false)
            .await
            .unwrap_err()
            .to_string()
            .contains(field));
        assert_eq!(before, lock);
    }
}
