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
