use super::*;
use crate::exports::{resolve_package_map, MapKind};
use ferrite_core::{EnvironmentKind, ModuleId};
use serde_json::json;

#[test]
fn checked_maps_reject_invalid_targets_and_preserve_array_and_condition_fallback() {
    for target in [
        "../outside.js",
        "/outside.js",
        ".//outside.js",
        "./../outside.js",
        "./%2e%2e/outside.js",
        "./nested/%2foutside.js",
        "./node_modules/dep/index.js",
        "./NODE_MODULES/dep.js",
        "./nested/./file.js",
        "./a\\b.js",
        "./bad%xy.js",
        "./file.js?unsupported",
    ] {
        assert!(
            resolve_package_map(&json!(target), ".", &[], MapKind::Exports).is_err(),
            "{target}"
        );
    }
    for map in [
        json!({".":"./ok.js","import":"./ok.js"}),
        json!({"0":"./ok.js","default":"./ok.js"}),
        json!({"#":"./ok.js"}),
    ] {
        let kind = if map.get("#").is_some() {
            MapKind::Imports
        } else {
            MapKind::Exports
        };
        assert!(resolve_package_map(
            &map,
            if kind == MapKind::Imports { "#x" } else { "." },
            &[],
            kind
        )
        .is_err());
    }
    assert_eq!(
        resolve_package_map(
            &json!(["../bad.js", null, "./ok.js"]),
            ".",
            &[],
            MapKind::Exports
        )
        .unwrap(),
        Some("./ok.js".into())
    );
    let map: serde_json::Value =
        serde_json::from_str(r#"{"browser":{"import":null},"default":"./ok.js"}"#).unwrap();
    assert_eq!(
        resolve_package_map(
            &map,
            ".",
            &["browser".into(), "import".into()],
            MapKind::Exports
        )
        .unwrap(),
        None
    );
    assert_eq!(
        resolve_package_map(&map, ".", &["require".into()], MapKind::Exports).unwrap(),
        Some("./ok.js".into())
    );
    assert_eq!(
        resolve_package_map(&json!("./space%20name.js"), ".", &[], MapKind::Exports).unwrap(),
        Some("./space name.js".into())
    );
    let map = json!({"./*":"./src/*"});
    assert!(resolve_package_map(&map, "./%2e%2e/outside", &[], MapKind::Exports).is_err());
}

fn write(root: &std::path::Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}
fn resolve(
    resolver: &Resolver,
    importer: &str,
    specifier: &str,
    kind: ResolveKind,
) -> ferrite_core::Result<ResolvedId> {
    resolver.resolve(&ResolveRequest {
        specifier,
        importer: Some(&ModuleId::new(importer)),
        environment: EnvironmentKind::Ssr,
        kind,
    })
}

#[test]
fn imports_use_nearest_scope_conditions_exact_files_and_circular_alias_bounds() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "package.json",
        r##"{"imports":{"#local":{"import":"./esm.js","require":"./cjs.cjs"},"#pattern/*":"./src/*.js","#missing":"./missing","#directory":"./src","#dependency":"dep","#blocked":null}}"##,
    );
    for file in [
        "esm.js",
        "cjs.cjs",
        "src/file.js",
        "src/index.js",
        "missing.js",
    ] {
        write(root.path(), file, "export const value = 42;");
    }
    write(
        root.path(),
        "node_modules/dep/package.json",
        r#"{"exports":{".":{"import":"./esm.js","require":"./cjs.cjs"}}}"#,
    );
    write(
        root.path(),
        "node_modules/dep/esm.js",
        "export const value = 1;",
    );
    write(
        root.path(),
        "node_modules/dep/cjs.cjs",
        "module.exports = 2;",
    );
    write(root.path(), "nested/package.json", r#"{"name":"nested"}"#);
    let mut resolver = Resolver::for_environment(
        root.path().into(),
        &Default::default(),
        &EnvironmentKind::Ssr,
    );
    assert_eq!(
        resolve(&resolver, "/entry.js", "#local", ResolveKind::Import)
            .unwrap()
            .id
            .0,
        "/esm.js"
    );
    assert_eq!(
        resolve(&resolver, "/entry.js", "#local?raw", ResolveKind::Require)
            .unwrap()
            .id
            .0,
        "/cjs.cjs?raw"
    );
    assert_eq!(
        resolve(&resolver, "/entry.js", "#pattern/file", ResolveKind::Import)
            .unwrap()
            .id
            .0,
        "/src/file.js"
    );
    assert!(
        resolve(&resolver, "/entry.js", "#dependency", ResolveKind::Import)
            .unwrap()
            .id
            .0
            .ends_with("node_modules/dep/esm.js")
    );
    assert!(
        resolve(&resolver, "/entry.js", "#dependency", ResolveKind::Require)
            .unwrap()
            .id
            .0
            .ends_with("node_modules/dep/cjs.cjs")
    );
    for specifier in ["#", "#/invalid", "#missing", "#directory", "#blocked"] {
        assert!(
            resolve(&resolver, "/entry.js", specifier, ResolveKind::Import).is_err(),
            "{specifier}"
        );
    }
    assert!(resolve(&resolver, "/nested/entry.js", "#local", ResolveKind::Import).is_err());
    resolver.alias.insert("dep".into(), "#dependency".into());
    assert!(
        resolve(&resolver, "/entry.js", "#dependency", ResolveKind::Import)
            .unwrap_err()
            .to_string()
            .contains("64 redirects")
    );
}

#[test]
fn mapped_dependencies_use_owning_locked_edges_and_store_local_identity() {
    let root = tempfile::tempdir().unwrap();
    let owner = ferrite_npm::LockedPackage {
        name: "owner".into(),
        version: "1.0.0".into(),
        source: "npm".into(),
        dependencies: [("dep".into(), "dep@2.0.0".into())].into(),
        ..Default::default()
    };
    let dep = |version: &str| ferrite_npm::LockedPackage {
        name: "dep".into(),
        version: version.into(),
        source: "npm".into(),
        ..Default::default()
    };
    let lock = ferrite_npm::Lockfile {
        importers: [(
            ".".into(),
            ferrite_npm::LockedImporter {
                optional_dependencies: Vec::new(),
                specifiers: [
                    ("owner".into(), "1.0.0".into()),
                    ("dep".into(), "1.0.0".into()),
                ]
                .into(),
                dependencies: [
                    ("owner".into(), owner.id()),
                    ("dep".into(), "dep@1.0.0".into()),
                ]
                .into(),
            },
        )]
        .into(),
        package: vec![owner, dep("1.0.0"), dep("2.0.0")],
        ..Default::default()
    };
    lock.write(&root.path().join("ferrite.lock")).unwrap();
    write(
        root.path(),
        ".ferrite/npm/packages/owner@1.0.0/package.json",
        r##"{"name":"owner","version":"1.0.0","imports":{"#dep":"dep","#local":"./src/local.js"}}"##,
    );
    write(
        root.path(),
        ".ferrite/npm/packages/owner@1.0.0/src/local.js",
        "export const local = 1;",
    );
    for version in ["1.0.0", "2.0.0"] {
        write(
            root.path(),
            &format!(".ferrite/npm/packages/dep@{version}/package.json"),
            &format!(r#"{{"name":"dep","version":"{version}","exports":"./main.js"}}"#),
        );
        write(
            root.path(),
            &format!(".ferrite/npm/packages/dep@{version}/main.js"),
            "export const value = 1;",
        );
    }
    let resolver = Resolver::for_environment(
        root.path().into(),
        &Default::default(),
        &EnvironmentKind::Ssr,
    );
    assert_eq!(
        resolve(
            &resolver,
            "/@npm/owner@1.0.0/src/entry.js",
            "#dep",
            ResolveKind::Import
        )
        .unwrap()
        .id
        .0,
        "/@npm/dep@2.0.0/main.js"
    );
    assert_eq!(
        resolve(
            &resolver,
            "/@npm/owner@1.0.0/src/entry.js",
            "#local",
            ResolveKind::Import
        )
        .unwrap()
        .id
        .0,
        "/@npm/owner@1.0.0/src/local.js"
    );
}

#[test]
#[ignore = "explicit Node reference executable for supported package-map contracts"]
fn node_reference_exports_selection_and_target_validation() {
    let node = std::env::var_os("FERRITE_NODE_PATH").unwrap_or_else(|| "node".into());
    for (map, specifier) in [
        (json!("./ok.js"), "pkg"),
        (json!(["../escape.js", "./ok.js"]), "pkg"),
        (json!([null, "./ok.js"]), "pkg"),
        (json!(["./absent.js", "./ok.js"]), "pkg"),
        (json!({"require":null,"default":"./ok.js"}), "pkg"),
        (json!({"import":"./missing.js","default":"./ok.js"}), "pkg"),
        (json!({".":"./ok.js","require":"./ok.js"}), "pkg"),
        (json!({"0":"./ok.js","default":"./ok.js"}), "pkg"),
        (json!("../escape.js"), "pkg"),
        (json!("./%2e%2e/escape.js"), "pkg"),
        (json!("./node_modules/dep/main.js"), "pkg"),
        (json!("./missing"), "pkg"),
        (json!("./dir"), "pkg"),
        (json!({"./feature/*":"./src/*.js"}), "pkg/feature/a"),
        (
            json!({"./feature/*":null,"./*":"./src/*.js"}),
            "pkg/feature/a",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "node_modules/pkg/package.json",
            &json!({"name":"pkg","version":"1.0.0","exports":map}).to_string(),
        );
        for file in [
            "ok.js",
            "missing.js",
            "dir/index.js",
            "src/a.js",
            "node_modules/dep/main.js",
        ] {
            write(
                root.path(),
                &format!("node_modules/pkg/{file}"),
                "module.exports = 42;",
            );
        }
        write(
            root.path(),
            "node_modules/escape.js",
            "module.exports = 'must not escape';",
        );
        let resolver = Resolver::for_environment(
            root.path().into(),
            &Default::default(),
            &EnvironmentKind::Ssr,
        );
        let actual = resolve(&resolver, "/main.cjs", specifier, ResolveKind::Require);
        let reference = std::process::Command::new(&node).args(["-e", "try { process.stdout.write(require.resolve(process.argv[1])); } catch (error) { process.stderr.write(error.code); process.exitCode=1; }", specifier]).current_dir(root.path()).output().expect("explicit Node reference executable");
        assert_eq!(
            actual.is_ok(),
            reference.status.success(),
            "map={map}, specifier={specifier}, ferrite={actual:?}, Node={}",
            String::from_utf8_lossy(&reference.stderr)
        );
        if let Ok(actual) = actual {
            let reference = String::from_utf8(reference.stdout).unwrap();
            assert_eq!(
                std::path::Path::new(&actual.id.0).file_name(),
                std::path::Path::new(&reference).file_name(),
                "map={map}"
            );
        }
    }
}

#[test]
fn malformed_package_manifests_do_not_fall_back_to_index() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "node_modules/pkg/package.json",
        "{invalid json",
    );
    write(
        root.path(),
        "node_modules/pkg/index.js",
        "export const hidden = 42;",
    );
    let resolver = Resolver::for_environment(
        root.path().into(),
        &Default::default(),
        &EnvironmentKind::Ssr,
    );
    assert!(resolve(&resolver, "/entry.js", "pkg", ResolveKind::Import)
        .unwrap_err()
        .to_string()
        .contains("invalid package manifest"));
}
