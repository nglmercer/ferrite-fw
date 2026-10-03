//! Module resolution (spec §15, §73).
//!
//! Handles relative/absolute imports, aliases, bare npm imports, package
//! `exports`/`imports`, conditions, directory indexes, symlinks, CSS/URL
//! imports, and virtual modules.

mod exports;
#[cfg(test)]
mod lock_tests;
mod node;
mod resolver;
mod side_effects;
mod specifier;
mod types;
mod util;

pub use exports::resolve_exports;
pub use resolver::Resolver;
pub use types::{PackageJson, ResolveKind, ResolveRequest, ResolvedId, SideEffects};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exports::match_pattern;
    use crate::side_effects::glob_match;
    use crate::specifier::split_package;
    use ferrite_config::ResolveConfig;
    use ferrite_core::{EnvironmentKind, ModuleId};
    use std::path::PathBuf;

    fn test_resolver(root: PathBuf) -> Resolver {
        Resolver::new(
            root,
            &ResolveConfig {
                alias: [("@".to_string(), "/src".to_string())]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        )
    }

    #[test]
    fn package_split() {
        assert_eq!(split_package("react"), ("react".to_string(), String::new()));
        assert_eq!(
            split_package("pkg/sub/path.js"),
            ("pkg".to_string(), "/sub/path.js".to_string())
        );
        assert_eq!(
            split_package("@scope/name/deep"),
            ("@scope/name".to_string(), "/deep".to_string())
        );
    }

    #[test]
    fn exports_string() {
        let exports = serde_json::json!("./index.js");
        assert_eq!(
            resolve_exports(&exports, ".", &["import".to_string()]),
            Some("./index.js".to_string())
        );
        assert_eq!(resolve_exports(&exports, "./other", &[]), None);
    }

    #[test]
    fn exports_conditional() {
        let exports = serde_json::json!({
            ".": {
                "browser": "./browser.js",
                "import": "./esm.js",
                "default": "./cjs.js"
            }
        });
        assert_eq!(
            resolve_exports(&exports, ".", &["browser".to_string()]),
            Some("./browser.js".to_string())
        );
        assert_eq!(
            resolve_exports(&exports, ".", &["import".to_string()]),
            Some("./esm.js".to_string())
        );
    }

    #[test]
    fn exports_pattern() {
        let exports = serde_json::json!({
            "./features/*.js": "./src/features/*.js"
        });
        assert_eq!(
            resolve_exports(&exports, "./features/a.js", &[]),
            Some("./src/features/a.js".to_string())
        );
    }

    #[test]
    fn virtual_passthrough() {
        let resolver = test_resolver(PathBuf::from("/tmp/x"));
        let request = ResolveRequest {
            specifier: "virtual:ferrite/env",
            importer: None,
            environment: EnvironmentKind::Client,
            kind: ResolveKind::Import,
        };
        let resolved = resolver.resolve(&request).unwrap();
        assert_eq!(resolved.id.0, "\0virtual:ferrite/env");
    }

    #[test]
    fn node_builtin_external() {
        let resolver = test_resolver(PathBuf::from("/tmp/x"));
        let request = ResolveRequest {
            specifier: "node:path",
            importer: None,
            environment: EnvironmentKind::Client,
            kind: ResolveKind::Import,
        };
        let resolved = resolver.resolve(&request).unwrap();
        assert!(resolved.external);
    }

    #[test]
    fn relative_file_resolution() {
        let dir = std::env::temp_dir().join(format!("ferrite-resolve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/a.ts"), "export const a = 1;").unwrap();
        std::fs::write(dir.join("src/b.ts"), "export const b = 1;").unwrap();
        let resolver = test_resolver(dir.clone());
        let importer = ModuleId::new("/src/a.ts");
        let request = ResolveRequest {
            specifier: "./b.ts",
            importer: Some(&importer),
            environment: EnvironmentKind::Client,
            kind: ResolveKind::Import,
        };
        let resolved = resolver.resolve(&request).unwrap();
        assert_eq!(resolved.id.0, "/src/b.ts");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn glob_matching() {
        assert!(glob_match("*.css", "style.css"));
        assert!(glob_match("./src/**", "src/a/b.ts"));
        assert!(!glob_match("*.css", "style.ts"));
    }

    #[test]
    fn pattern_overlap_does_not_panic() {
        // `abc` both starts with `abc` and ends with `abc`; the capture
        // range would be `3..0` without the overlap guard.
        assert_eq!(match_pattern("abc*abc", "abc"), None);
        assert_eq!(
            match_pattern("./features/*.js", "./features/a.js"),
            Some("a".to_string())
        );
    }

    #[test]
    fn package_imports_walk_past_bare_package_json() {
        let dir = std::env::temp_dir().join(format!("ferrite-imports-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r##"{"imports": {"#dep": "./dep.js"}}"##,
        )
        .unwrap();
        std::fs::write(dir.join("nested/package.json"), r#"{"name": "inner"}"#).unwrap();
        std::fs::write(dir.join("dep.js"), "export const x = 1;").unwrap();
        std::fs::write(dir.join("nested/a.js"), "import '#dep';").unwrap();
        let resolver = test_resolver(dir.clone());
        let importer = ModuleId::new("/nested/a.js");
        let request = ResolveRequest {
            specifier: "#dep",
            importer: Some(&importer),
            environment: EnvironmentKind::Client,
            kind: ResolveKind::Import,
        };
        let resolved = resolver.resolve(&request).unwrap();
        assert_eq!(resolved.id.0, "/dep.js");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod missing_candidate_tests {
    #[test]
    fn candidates_share_file_probe_order_aliases_and_query_handling() {
        let root = std::path::PathBuf::from("/project");
        let mut config = ferrite_config::ResolveConfig {
            extensions: vec![".js".into(), ".ts".into()],
            ..Default::default()
        };
        config.alias.insert("@child".into(), "./src/child".into());
        let resolver = super::Resolver::new(root.clone(), &config);
        let importer = ferrite_core::ModuleId::new("/src/entry.js");
        let expected: Vec<_> = [
            "src/child",
            "src/child.js",
            "src/child.ts",
            "src/child/index.js",
            "src/child/index.ts",
            "src/child/main.js",
            "src/child/main.ts",
        ]
        .into_iter()
        .map(|path| root.join(path))
        .collect();
        for specifier in ["./child?raw", "@child", "/src/child"] {
            assert_eq!(
                resolver.unresolved_file_candidates(specifier, &importer),
                expected
            );
        }
        for specifier in [
            "react",
            "node:fs",
            "https://example.com/a.js",
            "virtual:test",
            "../../../outside",
        ] {
            assert!(resolver
                .unresolved_file_candidates(specifier, &importer)
                .is_empty());
        }
    }
}
