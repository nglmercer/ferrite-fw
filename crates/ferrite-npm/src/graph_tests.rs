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
            libc: None,
            local: None,
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
        .os = Some(vec![format!("!{}", InstallPlatform::current().os)]);
    fixture.save();
    let manifest = Fixture::manifest(&[("app", "1.0.0")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.package.len(), 2);
    let active = lock.active_graph(&InstallPlatform::current()).unwrap();
    assert_eq!(active.packages.len(), 1);
    assert!(active.dependencies["app@1.0.0"].is_empty());
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
    assert_eq!(lock.version, LOCKFILE_VERSION);
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

#[tokio::test]
async fn registry_aliases_preserve_edges_projection_and_frozen_identity() {
    let mut fixture = Fixture::new();
    fixture.package("actual", "1.0.0", &[]);
    fixture.package("actual", "2.0.0", &[]);
    fixture.package("@scope/library", "1.0.0", &[("nested", "npm:actual@^2")]);
    fixture.save();
    let manifest = Fixture::manifest(&[
        ("legacy", "npm:actual@^1"),
        ("modern", "npm:actual@^2"),
        ("scoped", "npm:@scope/library@1"),
    ]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert_eq!(lock.importers["."].dependencies["legacy"], "actual@1.0.0");
    assert_eq!(lock.importers["."].dependencies["modern"], "actual@2.0.0");
    assert_eq!(
        lock.find("scoped").unwrap().dependencies["nested"],
        "actual@2.0.0"
    );
    assert_eq!(lock.package.len(), 3);
    project_editor_dependencies(fixture.dir.path(), &lock).unwrap();
    let alias: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture.dir.path().join("node_modules/legacy/package.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(alias["name"], "actual");
    let snapshot = lock.clone();
    std::fs::remove_dir_all(&fixture.installer.store).unwrap();
    std::fs::remove_dir_all(&fixture.installer.client.cache_dir).unwrap();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, true)
        .await
        .unwrap();
    assert_eq!(lock, snapshot);
    let mut corrupt = lock.clone();
    corrupt
        .importers
        .get_mut(".")
        .unwrap()
        .specifiers
        .insert("legacy".into(), "npm:unrelated@1".into());
    assert!(
        corrupt.validate().is_err(),
        "alias edges must validate their real target name"
    );
}

#[tokio::test]
async fn shasum_only_registry_packages_replay_with_locked_integrity() {
    let mut fixture = Fixture::new();
    fixture.package("legacy-shasum", "1.0.0", &[]);
    let bytes = std::fs::read(
        fixture
            .installer
            .tarball_cache
            .join("legacy-shasum-1.0.0.tgz"),
    )
    .unwrap();
    use sha2::Digest as _;
    let shasum: String = sha1::Sha1::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let info = fixture
        .metadata
        .get_mut("legacy-shasum")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap();
    info.dist.integrity = None;
    info.dist.shasum = Some(shasum);
    fixture.save();
    let manifest = Fixture::manifest(&[("legacy-shasum", "1")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    assert!(lock
        .find("legacy-shasum")
        .unwrap()
        .integrity
        .as_ref()
        .unwrap()
        .starts_with("sha1-"));
    let snapshot = lock.clone();
    std::fs::remove_dir_all(&fixture.installer.store).unwrap();
    std::fs::remove_dir_all(&fixture.installer.client.cache_dir).unwrap();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, true)
        .await
        .unwrap();
    assert_eq!(lock, snapshot);
    lock.package[0].integrity = None;
    assert!(fixture
        .installer
        .install_manifest(&manifest, &mut lock, true)
        .await
        .unwrap_err()
        .to_string()
        .contains("lacks locked integrity"));
}

#[tokio::test]
async fn portable_optional_variants_replay_without_metadata() {
    let mut fixture = Fixture::new();
    for name in ["linux-gnu", "linux-musl", "windows", "optional-parent"] {
        fixture.package(name, "1.0.0", &[]);
    }
    for (name, os, libc) in [
        ("linux-gnu", "linux", Some("glibc")),
        ("linux-musl", "linux", Some("musl")),
        ("windows", "win32", None),
    ] {
        let info = fixture
            .metadata
            .get_mut(name)
            .unwrap()
            .versions
            .get_mut("1.0.0")
            .unwrap();
        info.os = Some(vec![os.into()]);
        info.cpu = Some(vec!["x64".into()]);
        info.libc = libc.map(|value| vec![value.into()]);
    }
    fixture
        .metadata
        .get_mut("optional-parent")
        .unwrap()
        .versions
        .get_mut("1.0.0")
        .unwrap()
        .dependencies
        .insert("linux-gnu".into(), "1".into());
    fixture.save();
    let mut manifest = Fixture::manifest(&[("windows", "missing-range-overridden")]);
    for name in ["linux-gnu", "linux-musl", "windows", "optional-parent"] {
        manifest
            .optional_dependencies
            .insert(name.into(), "1".into());
    }
    let linux = InstallPlatform {
        os: "linux".into(),
        cpu: "x64".into(),
        libc: Some("glibc".into()),
    };
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest_for_platform(&manifest, &mut lock, false, &linux)
        .await
        .unwrap();
    assert_eq!(lock.package.len(), 4);
    assert_eq!(lock.active_graph(&linux).unwrap().packages.len(), 2);
    let path = fixture.dir.path().join("ferrite.lock");
    lock.write(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_dir_all(&fixture.installer.client.cache_dir).unwrap();
    for (os, libc, name) in [
        ("linux", Some("musl"), "linux-musl"),
        ("win32", None, "windows"),
    ] {
        std::fs::remove_dir_all(&fixture.installer.store).unwrap();
        let platform = InstallPlatform {
            os: os.into(),
            cpu: "x64".into(),
            libc: libc.map(str::to_string),
        };
        let before = lock.clone();
        fixture
            .installer
            .install_manifest_for_platform(&manifest, &mut lock, true, &platform)
            .await
            .unwrap();
        assert_eq!(lock, before);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            lock.active_graph(&platform).unwrap().packages,
            std::collections::BTreeSet::from([format!("{name}@1.0.0")])
        );
        assert!(fixture
            .installer
            .store
            .join(format!("{name}@1.0.0/package.json"))
            .is_file());
        assert!(!fixture
            .installer
            .store
            .join("optional-parent@1.0.0")
            .exists());
    }
}

#[tokio::test]
async fn transitive_peer_contexts_follow_contextual_providers_through_cycles() {
    let mut fixture = Fixture::new();
    fixture.package(
        "legacy",
        "1.0.0",
        &[("runtime", "1"), ("tool", "1"), ("bridge", "1")],
    );
    fixture.package(
        "modern",
        "1.0.0",
        &[("runtime", "2"), ("tool", "1"), ("bridge", "1")],
    );
    fixture.package("runtime", "1.0.0", &[]);
    fixture.package("runtime", "2.0.0", &[]);
    fixture.package("tool", "1.0.0", &[]);
    fixture.package("bridge", "1.0.0", &[("runtime", "2"), ("mid", "1")]);
    fixture.package("mid", "1.0.0", &[("bridge", "1"), ("consumer", "1")]);
    fixture.package("consumer", "1.0.0", &[]);
    for (name, peer, range) in [("tool", "runtime", "*"), ("consumer", "tool", "1")] {
        fixture
            .metadata
            .get_mut(name)
            .unwrap()
            .versions
            .get_mut("1.0.0")
            .unwrap()
            .peer_dependencies
            .insert(peer.into(), range.into());
    }
    fixture.save();
    let manifest = Fixture::manifest(&[("legacy", "1"), ("modern", "1")]);
    let mut lock = Lockfile::default();
    fixture
        .installer
        .install_manifest(&manifest, &mut lock, false)
        .await
        .unwrap();
    for name in ["bridge", "mid", "tool", "consumer"] {
        assert_eq!(
            lock.package
                .iter()
                .filter(|package| package.name == name)
                .count(),
            2,
            "{name}"
        );
    }
    for (root, version) in [("legacy", "1.0.0"), ("modern", "2.0.0")] {
        let root = lock.find(root).unwrap();
        let bridge = lock.package_by_id(&root.dependencies["bridge"]).unwrap();
        let mid = lock.package_by_id(&bridge.dependencies["mid"]).unwrap();
        assert_eq!(mid.dependencies["bridge"], bridge.id());
        let consumer = lock.package_by_id(&mid.dependencies["consumer"]).unwrap();
        let tool = lock.package_by_id(&consumer.peers["tool"]).unwrap();
        assert_eq!(tool.id(), root.dependencies["tool"]);
        assert_eq!(
            lock.package_by_id(&tool.peers["runtime"]).unwrap().version,
            version
        );
        assert_eq!(
            lock.package_by_id(&bridge.dependencies["runtime"])
                .unwrap()
                .version,
            "2.0.0"
        );
    }
    let ids: std::collections::BTreeSet<_> = lock.package.iter().map(LockedPackage::id).collect();
    fixture.package("unrelated", "1.0.0", &[]);
    fixture.save();
    let mut extended = manifest.clone();
    extended.dependencies.insert("unrelated".into(), "1".into());
    fixture
        .installer
        .install_manifest(&extended, &mut lock, false)
        .await
        .unwrap();
    assert!(
        ids.is_subset(&lock.package.iter().map(LockedPackage::id).collect()),
        "old: {ids:?} new: {:?}",
        lock.package
    );
    std::fs::remove_dir_all(&fixture.installer.client.cache_dir).unwrap();
    let before = lock.clone();
    fixture
        .installer
        .install_manifest(&extended, &mut lock, true)
        .await
        .unwrap();
    assert_eq!(lock, before);
    project_node_modules(fixture.installer.store.parent().unwrap(), &lock).unwrap();
}

#[tokio::test]
async fn conflicting_live_local_peer_views_fail_without_publishing_a_lock() {
    let mut fixture = Fixture::new();
    fixture.package("runtime", "1.0.0", &[]);
    fixture.package("runtime", "2.0.0", &[]);
    fixture.save();
    fn write(root: &std::path::Path, path: &str, value: serde_json::Value) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    }
    write(
        fixture.dir.path(),
        "package.json",
        serde_json::json!({"workspaces":["packages/*"],"dependencies":{"runtime":"1"}}),
    );
    for (name, version) in [("legacy", "1"), ("modern", "2")] {
        write(
            fixture.dir.path(),
            &format!("packages/{name}/package.json"),
            serde_json::json!({"name":name,"version":"1.0.0","dependencies":{"runtime":version,"shared":"workspace:*"}}),
        );
    }
    write(
        fixture.dir.path(),
        "packages/shared/package.json",
        serde_json::json!({"name":"shared","version":"1.0.0","peerDependencies":{"runtime":"*"}}),
    );
    let mut lock = Lockfile::default();
    let before = lock.clone();
    let error = fixture
        .installer
        .install_project(fixture.dir.path(), &mut lock, false)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("conflicting peer contexts"),
        "{error}"
    );
    assert_eq!(lock, before);
    assert!(!fixture
        .dir
        .path()
        .join("packages/shared/node_modules")
        .exists());
}

#[test]
fn required_platform_incompatibility_propagates_through_optional_cycles() {
    let package = |name: &str, edges: &[(&str, &str)]| LockedPackage {
        name: name.into(),
        version: "1.0.0".into(),
        source: "npm".into(),
        dependencies: edges
            .iter()
            .map(|(name, target)| (name.to_string(), format!("{target}@1.0.0")))
            .collect(),
        dependency_specifiers: edges
            .iter()
            .map(|(name, _)| (name.to_string(), "1".into()))
            .collect(),
        ..Default::default()
    };
    let a = package("a", &[("b", "b")]);
    let mut b = package("b", &[("a", "a")]);
    b.os = Some(vec!["win32".into()]);
    let mut lock = Lockfile {
        portable: true,
        package: vec![a, b],
        importers: BTreeMap::from([(
            ".".into(),
            LockedImporter {
                dependencies: BTreeMap::from([("a".into(), "a@1.0.0".into())]),
                specifiers: BTreeMap::from([("a".into(), "1".into())]),
                optional_dependencies: vec!["a".into()],
            },
        )]),
        ..Default::default()
    };
    let platform = InstallPlatform {
        os: "linux".into(),
        cpu: "x64".into(),
        libc: Some("glibc".into()),
    };
    assert!(lock.active_graph(&platform).unwrap().packages.is_empty());
    lock.importers
        .get_mut(".")
        .unwrap()
        .dependencies
        .insert("b".into(), "b@1.0.0".into());
    assert!(lock
        .active_graph(&platform)
        .unwrap_err()
        .to_string()
        .contains("required dependency"));
}
