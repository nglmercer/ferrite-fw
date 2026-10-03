use crate::{ResolveKind, ResolveRequest, Resolver};
use ferrite_core::{EnvironmentKind, ModuleId};
use ferrite_npm::{LockedImporter, LockedPackage, Lockfile};
use std::collections::BTreeMap;

fn package(name: &str, version: &str, deps: &[(&str, &str)]) -> LockedPackage {
    LockedPackage {
        name: name.into(),
        version: version.into(),
        source: "npm".into(),
        dependencies: deps
            .iter()
            .map(|(name, id)| ((*name).into(), (*id).into()))
            .collect(),
        ..Default::default()
    }
}
fn install_files(root: &std::path::Path, package: &LockedPackage) {
    let path = root.join(".ferrite/npm/packages").join(package.id());
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("package.json"), serde_json::to_vec(&serde_json::json!({"name": package.name, "version": package.version, "type": "module", "exports": {".": "./index.js"}})).unwrap()).unwrap();
    std::fs::write(path.join("index.js"), "export const value = 1;").unwrap();
}
fn resolve(
    resolver: &Resolver,
    name: &str,
    importer: Option<&ModuleId>,
) -> ferrite_core::Result<crate::ResolvedId> {
    resolver.resolve(&ResolveRequest {
        specifier: name,
        importer,
        environment: EnvironmentKind::Client,
        kind: ResolveKind::Import,
    })
}

#[test]
fn resolver_follows_importer_edges_instead_of_highest_store_version() {
    let dir = tempfile::tempdir().unwrap();
    let mut lock = Lockfile::default();
    for record in [
        package("a", "1.0.0", &[("shared", "shared@1.0.0")]),
        package("b", "1.0.0", &[("shared", "shared@2.0.0")]),
        package("shared", "1.0.0", &[]),
        package("shared", "2.0.0", &[]),
        package("@scope/component", "1.0.0", &[("shared", "shared@1.0.0")]),
    ] {
        install_files(dir.path(), &record);
        lock.upsert(record);
    }
    lock.importers.insert(
        ".".into(),
        LockedImporter {
            specifiers: BTreeMap::from([
                ("a".into(), "1.0.0".into()),
                ("b".into(), "1.0.0".into()),
                ("@scope/component".into(), "1.0.0".into()),
            ]),
            dependencies: BTreeMap::from([
                ("a".into(), "a@1.0.0".into()),
                ("b".into(), "b@1.0.0".into()),
                ("@scope/component".into(), "@scope/component@1.0.0".into()),
            ]),
        },
    );
    lock.write(&dir.path().join("ferrite.lock")).unwrap();
    let resolver = Resolver::new(dir.path().into(), &Default::default());
    let a = resolve(&resolver, "a", None).unwrap().id;
    let b = resolve(&resolver, "b", None).unwrap().id;
    let scoped = resolve(&resolver, "@scope/component", None).unwrap().id;
    assert_eq!(
        resolve(&resolver, "shared", Some(&a)).unwrap().id.0,
        "/@npm/shared@1.0.0/index.js"
    );
    assert_eq!(
        resolve(&resolver, "shared", Some(&b)).unwrap().id.0,
        "/@npm/shared@2.0.0/index.js"
    );
    assert_eq!(
        resolve(&resolver, "shared", Some(&scoped)).unwrap().id.0,
        "/@npm/shared@1.0.0/index.js"
    );
    assert!(resolve(&resolver, "shared", None)
        .unwrap_err()
        .to_string()
        .contains("no locked dependency edge"));
    // An unrelated, newer package in the store must never change resolution.
    install_files(dir.path(), &package("shared", "99.0.0", &[]));
    assert_eq!(
        resolve(&resolver, "shared", Some(&a)).unwrap().id.0,
        "/@npm/shared@1.0.0/index.js"
    );
    assert_eq!(
        resolve(&resolver, "a", Some(&a)).unwrap().id,
        a,
        "self import keeps the selected package instance"
    );
}

#[test]
fn missing_or_ambiguous_store_versions_fail_and_custom_locks_work() {
    let dir = tempfile::tempdir().unwrap();
    let one = package("pkg", "1.0.0", &[]);
    let two = package("pkg", "2.0.0", &[]);
    install_files(dir.path(), &one);
    install_files(dir.path(), &two);
    let mut resolver = Resolver::new(dir.path().into(), &Default::default());
    assert!(resolve(&resolver, "pkg", None)
        .unwrap_err()
        .to_string()
        .contains("multiple versions"));
    let mut lock = Lockfile::default();
    lock.upsert(one.clone());
    lock.importers.insert(
        ".".into(),
        LockedImporter {
            specifiers: BTreeMap::from([("pkg".into(), "1.0.0".into())]),
            dependencies: BTreeMap::from([("pkg".into(), one.id())]),
        },
    );
    resolver.lockfile = dir.path().join("custom.lock");
    lock.write(&resolver.lockfile).unwrap();
    assert_eq!(
        resolve(&resolver, "pkg", None).unwrap().id.0,
        "/@npm/pkg@1.0.0/index.js"
    );
    std::fs::remove_dir_all(resolver.npm_store.join(one.id())).unwrap();
    assert!(resolve(&resolver, "pkg", None)
        .unwrap_err()
        .to_string()
        .contains("frozen-lockfile"));
}

#[test]
fn require_conditions_and_manifest_order_select_the_correct_entry() {
    let dir = tempfile::tempdir().unwrap();
    let package = dir.path().join("node_modules/dual");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("package.json"), r#"{"name":"dual","exports":{".":{"require":"./require.cjs","browser":"./browser.js","import":"./import.js","default":"./fallback.js"}}}"#).unwrap();
    for file in [
        "require.cjs",
        "browser.js",
        "import.js",
        "fallback.js",
        "module.js",
        "main.cjs",
    ] {
        std::fs::write(package.join(file), "").unwrap();
    }
    let resolver = Resolver::new(dir.path().into(), &Default::default());
    for (kind, expected) in [
        (ResolveKind::Import, "browser.js"),
        (ResolveKind::Require, "require.cjs"),
    ] {
        let resolved = resolver
            .resolve(&ResolveRequest {
                specifier: "dual",
                importer: None,
                environment: EnvironmentKind::Client,
                kind,
            })
            .unwrap();
        assert!(resolved.id.0.ends_with(expected), "{}", resolved.id.0);
    }
    std::fs::write(
        package.join("package.json"),
        r#"{"name":"dual","module":"./module.js","main":"./main.cjs"}"#,
    )
    .unwrap();
    let resolved = resolver
        .resolve(&ResolveRequest {
            specifier: "dual",
            importer: None,
            environment: EnvironmentKind::Client,
            kind: ResolveKind::Require,
        })
        .unwrap();
    assert!(resolved.id.0.ends_with("main.cjs"));
    let exports: serde_json::Value =
        serde_json::from_str(r#"{"default":"./first.js","browser":"./second.js"}"#).unwrap();
    assert_eq!(
        crate::resolve_exports(&exports, ".", &["browser".into()]),
        Some("./first.js".into())
    );
}

#[test]
fn conditional_null_and_pattern_priority_do_not_fall_back_silently() {
    let parse = |json| serde_json::from_str::<serde_json::Value>(json).unwrap();
    let conditions = vec!["browser".into(), "require".into()];
    let blocked = parse(r#"{"browser":{"require":null},"default":"./fallback.js"}"#);
    assert_eq!(crate::resolve_exports(&blocked, ".", &conditions), None);
    let unmatched = parse(r#"{"browser":{"import":"./esm.js"},"default":"./fallback.js"}"#);
    assert_eq!(
        crate::resolve_exports(&unmatched, ".", &conditions),
        Some("./fallback.js".into())
    );
    assert_eq!(
        crate::resolve_exports(&unmatched, "./private", &conditions),
        None
    );
    let patterns = parse(
        r#"{"./a/*":{"default":"./first/*.js","browser":"./second/*.js"},"./*long-suffix":"./other/*.js"}"#,
    );
    assert_eq!(
        crate::resolve_exports(&patterns, "./a/blong-suffix", &conditions),
        Some("./first/blong-suffix.js".into())
    );
    let blocked_pattern =
        parse(r#"{"./a/*":{"browser":{"require":null},"default":"./fallback/*.js"}}"#);
    assert_eq!(
        crate::resolve_exports(&blocked_pattern, "./a/file", &conditions),
        None
    );
    let alternatives = parse(r#"[null,{"import":"./esm.js"},"./cjs.js"]"#);
    assert_eq!(
        crate::resolve_exports(&alternatives, ".", &conditions),
        Some("./cjs.js".into())
    );
}

#[test]
fn npm_cycles_queries_and_dot_segments_share_concrete_module_identity() {
    let root = tempfile::tempdir().unwrap();
    let package = package("@scope/cycle", "1.0.0", &[]);
    install_files(root.path(), &package);
    let dir = root.path().join(".ferrite/npm/packages").join(package.id());
    std::fs::create_dir_all(dir.join("src/nested")).unwrap();
    std::fs::write(
        dir.join("src/a.js"),
        "import './nested/../b.js'; export const a = 1;",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/b.js"),
        "import './nested/../a.js'; export const b = 1;",
    )
    .unwrap();
    for preserve_symlinks in [false, true] {
        let mut resolver = Resolver::new(root.path().into(), &Default::default());
        resolver.preserve_symlinks = preserve_symlinks;
        let initial = resolve(&resolver, "/@npm/@scope/cycle@1.0.0/src/./a.js", None)
            .unwrap()
            .id;
        assert_eq!(initial.0, "/@npm/@scope/cycle@1.0.0/src/a.js");
        let mut next = initial.clone();
        for _ in 0..12 {
            let b = resolve(&resolver, "./nested/../b.js", Some(&next))
                .unwrap()
                .id;
            assert_eq!(b.0, "/@npm/@scope/cycle@1.0.0/src/b.js");
            next = resolve(&resolver, "./nested/../a.js", Some(&b)).unwrap().id;
            assert_eq!(
                next, initial,
                "cycles must not grow new dot-segment identities"
            );
        }
        let query = resolve(
            &resolver,
            "./nested/../a.js?ferrite-cjs-factory",
            Some(&initial),
        )
        .unwrap()
        .id;
        assert_eq!(
            query.0,
            "/@npm/@scope/cycle@1.0.0/src/a.js?ferrite-cjs-factory"
        );
    }
}
