//! Read-only capability diagnostics. Locating a host never establishes execution support.
use ferrite_config::ResolvedConfig;
use ferrite_core::Result;
use ferrite_npm::{JsPackageJson, Lockfile};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const JSX_SCAN_SCOPE: &str = "ownership only for project JSX/TSX files; excludes symlink sources and generated/public directories; package/host execution and custom plugin lowering are unverified";

#[derive(Debug, Serialize)]
pub struct JsxOwnershipReport {
    pub manifest: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub manifest_error: Option<String>,
    #[serde(flatten)]
    pub ownership: crate::registry::JsxOwnership,
}

/// Read source-file ownership without loading any compiler or executing plugins.
pub fn inspect_jsx_ownership(config: &ResolvedConfig) -> Result<Vec<JsxOwnershipReport>> {
    let mut queue = vec![config.root.clone()];
    let output = config.root.join(&config.build.out_dir);
    let mut groups: BTreeMap<Option<PathBuf>, Vec<PathBuf>> = BTreeMap::new();
    while let Some(directory) = queue.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if path == output
                    || matches!(
                        entry.file_name().to_str(),
                        Some(".git" | ".ferrite" | "node_modules" | "target" | "dist" | "public")
                    )
                {
                    continue;
                }
                queue.push(path);
            } else if kind.is_file()
                && matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("jsx" | "tsx")
                )
            {
                let manifest = crate::registry::jsx_manifest_candidates(&config.root, &path)
                    .into_iter()
                    .find(|manifest| manifest.exists());
                groups.entry(manifest).or_default().push(
                    path.strip_prefix(&config.root)
                        .unwrap_or(&path)
                        .to_path_buf(),
                );
            }
        }
    }
    let mut reports = Vec::new();
    for (manifest, mut files) in groups {
        files.sort();
        let mut manifest_error = None;
        let value = match &manifest {
            Some(path) => {
                let parsed = std::fs::read_to_string(path)
                    .map_err(ferrite_core::FerriteError::from)
                    .and_then(|source| {
                        serde_json::from_str(&source).map_err(|error| {
                            ferrite_core::FerriteError::Config(format!(
                                "cannot inspect JSX owner in {}: {error}",
                                path.display()
                            ))
                        })
                    });
                match parsed {
                    Ok(value) => value,
                    Err(error)
                        if config.framework.is_some()
                            || config.react.import_source.is_some()
                            || config.react.factory.is_some() =>
                    {
                        manifest_error = Some(error.to_string());
                        serde_json::Value::Null
                    }
                    Err(error) => return Err(error),
                }
            }
            None => serde_json::Value::Null,
        };
        reports.push(JsxOwnershipReport {
            manifest,
            files,
            manifest_error,
            ownership: crate::registry::jsx_ownership(config, &value),
        });
    }
    Ok(reports)
}

#[derive(Debug, Serialize)]
pub struct Issue {
    pub severity: &'static str,
    pub message: String,
    pub action: String,
}
#[derive(Debug, Serialize)]
pub struct FrameworkReport {
    pub framework: &'static str,
    pub active: bool,
    pub selection: &'static str,
    pub version: Option<String>,
    pub identity: Option<String>,
    pub compiler_host: Option<String>,
    pub compiler_support: &'static str,
    pub client: &'static str,
    pub ssr: &'static str,
    pub updates: &'static str,
    pub checker: &'static str,
    pub compiler_profiles: Vec<serde_json::Value>,
}
#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub schema_version: u32,
    pub root: PathBuf,
    pub compiler: String,
    pub compiler_version: Option<String>,
    pub ssr_runtime: String,
    pub runtime_config: ferrite_config::RuntimeConfig,
    pub node_executable: Option<PathBuf>,
    pub node_probe: &'static str,
    pub editor_view: &'static str,
    pub frameworks: Vec<FrameworkReport>,
    pub foreign_plugins: Vec<serde_json::Value>,
    pub jsx_ownership_scope: &'static str,
    pub jsx_ownership: Vec<JsxOwnershipReport>,
    pub issues: Vec<Issue>,
}

pub fn inspect(config: &ResolvedConfig) -> Result<DoctorReport> {
    let lock = Lockfile::read(&config.lockfile())?;
    lock.validate()?;
    let manifest = JsPackageJson::read(&config.root.join("package.json"))?;
    let explicit = config.framework.as_ref();
    let configured: Vec<&str> = explicit
        .map(|profile| profile.enabled.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let declared = |name: &str| {
        manifest.dependencies.contains_key(name) || manifest.dev_dependencies.contains_key(name)
    };
    let node_executable = locate_node(
        &config.root,
        explicit.and_then(|profile| profile.node.as_deref()),
    );
    let mut issues = Vec::new();
    let jsx_ownership = inspect_jsx_ownership(config)?;
    for report in &jsx_ownership {
        if let Some(error) = &report.manifest_error {
            issues.push(Issue { severity: "warning", message: format!("JSX owner manifest could not be inspected: {error}; explicit selection governs lowering"), action: "correct the manifest before dependency installation; explicit JSX selection does not validate dependency manifests".into() });
        }
        if matches!(
            report.ownership.status,
            "unowned" | "ambiguous" | "unavailable"
        ) {
            issues.push(Issue {
                severity: if config.foreign_plugins.iter().flatten().next().is_some() { "warning" } else { "error" },
                message: format!("{} native JSX ownership for {} source files in {} ({})", report.ownership.status, report.files.len(), report.manifest.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| config.root.display().to_string()), report.ownership.declared_owners.join(", ")),
                action: "select framework.enabled = [\"react\"] with compiler_host = \"native\", or provide and execute a framework plugin that lowers JSX to JavaScript; custom lowering is not validated by doctor".into(),
            });
        }
    }
    let mut foreign_plugins = Vec::new();
    for profile in config.foreign_plugins.iter().flatten() {
        let entry = config.root.join(&profile.entry);
        let node = locate_node(&config.root, profile.node.as_deref());
        if !entry.is_file() || node.is_none() {
            issues.push(Issue { severity: "error", message: format!("foreign plugin {} entry or explicit Node executable is missing", profile.name), action: "check foreign_plugins.entry/node; run dev/build to validate the hook contract after locating both".into() });
        }
        foreign_plugins.push(serde_json::json!({"name":profile.name, "entry":entry, "entryExists":entry.is_file(), "host":profile.host, "nodeExecutable":node, "probe": if node.is_some() {"located-not-executed"} else {"not-located"}, "support":"experimental", "execution":"unverified"}));
    }
    if !config.lockfile().exists() {
        issues.push(Issue {
            severity: "warning",
            message: "resolved lockfile is missing".into(),
            action: "run ferrite install".into(),
        });
    }
    let compiler = ferrite_transform::compiler_for_engine(&config.compiler.engine);
    if let Err(error) = &compiler {
        issues.push(Issue {
            severity: "error",
            message: error.to_string(),
            action: "select an available compiler.engine and required feature flags".into(),
        });
    }
    let compiler_version = compiler
        .as_ref()
        .ok()
        .map(|compiler| compiler.version().to_string());
    let mut frameworks = Vec::new();
    let has_component_owner = crate::registry::FRAMEWORKS.iter().any(|descriptor| {
        descriptor.name != "vanilla"
            && if explicit.is_some() {
                configured.contains(&descriptor.name)
            } else {
                declared(descriptor.name)
            }
    });
    for descriptor in crate::registry::FRAMEWORKS {
        let name = descriptor.name;
        let active = if name == "vanilla" {
            !has_component_owner
        } else if explicit.is_some() {
            configured.contains(&name)
        } else {
            declared(name)
        };
        let package = lock
            .importers
            .get(".")
            .and_then(|root| root.dependencies.get(name))
            .and_then(|id| lock.package_by_id(id));
        let host = if name == "vanilla" || name == "react" {
            Some("native".to_string())
        } else {
            explicit.and_then(|profile| profile.compiler_host.clone())
        };
        let mut available = active && compiler.is_ok();
        if active && name != "vanilla" {
            match package {
                None => {
                    available = false;
                    issues.push(Issue {
                        severity: "error",
                        message: format!("{name} has no concrete root dependency edge"),
                        action: format!("declare {name} in package.json and run ferrite install"),
                    });
                }
                Some(package) => {
                    let path = config
                        .root
                        .join(".ferrite/npm/packages")
                        .join(package.id())
                        .join("package.json");
                    let installed = std::fs::read(&path)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
                    if installed.as_ref().is_none_or(|manifest| {
                        manifest["name"] != package.name || manifest["version"] != package.version
                    }) {
                        available = false;
                        issues.push(Issue {
                            severity: "error",
                            message: format!(
                                "{} is missing or its stored identity differs",
                                package.id()
                            ),
                            action: "run ferrite install --frozen-lockfile".into(),
                        });
                    }
                    if !descriptor.compiler_profiles.is_empty()
                        && !descriptor.compiler_profiles.iter().any(|profile| {
                            Some(profile.host) == host.as_deref()
                                && profile.framework_version == package.version
                        })
                    {
                        available = false;
                        issues.push(Issue { severity: "error", message: format!("{name}@{} has no validated selected compiler profile", package.version), action: "select a version/host reported in compiler_profiles; configure framework.compiler_host explicitly".into() });
                    }
                }
            }
            if !descriptor.compiler_profiles.is_empty() && host.as_deref() != Some("node") {
                available = false;
                issues.push(Issue { severity: "error", message: format!("{name} compiler host is not enabled"), action: format!("configure framework.enabled = ['{name}'] and framework.compiler_host = 'node'; SSR runtime is separate") });
            } else if host.as_deref() == Some("node") && node_executable.is_none() {
                available = false;
                issues.push(Issue {
                    severity: "error",
                    message: "selected Node compiler executable was not located".into(),
                    action: "install Node or configure framework.node to an executable path".into(),
                });
            }
        }
        if active {
            issues.push(Issue { severity: "warning", message: format!("{name} checker integration is unavailable; transpilation does not type-check"), action: "use the official framework checker separately until Ferrite checker execution is implemented".into() });
        }
        if active && name == "react" {
            let mut required = vec![
                ("react", crate::scaffold::REACT_VERSION),
                ("react-dom", crate::scaffold::REACT_VERSION),
            ];
            if config.react.refresh {
                required.push(("react-refresh", crate::scaffold::REACT_REFRESH_VERSION));
            }
            for (dependency, expected) in required {
                let selected = lock
                    .importers
                    .get(".")
                    .and_then(|root| root.dependencies.get(dependency))
                    .and_then(|id| lock.package_by_id(id));
                let installed = selected.and_then(|package| {
                    std::fs::read(
                        config
                            .root
                            .join(".ferrite/npm/packages")
                            .join(package.id())
                            .join("package.json"),
                    )
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                });
                if selected.is_none_or(|package| package.version != expected)
                    || installed.as_ref().is_none_or(|manifest| {
                        manifest["name"] != dependency || manifest["version"] != expected
                    })
                {
                    available = false;
                    issues.push(Issue { severity: "error", message: format!("React experimental client profile requires installed {dependency}@{expected}"), action: format!("declare {dependency} = {expected} in package.json and run ferrite install; other versions have no validated client profile") });
                }
            }
        }
        frameworks.push(FrameworkReport {
            framework: name, active, selection: if explicit.is_some() { "configuration" } else { "manifest" },
            version: package.map(|package| package.version.clone()), identity: package.map(|package| package.id()),
            compiler_host: host, compiler_support: if available { "experimental" } else { "unavailable" },
            client: if available { descriptor.client.label() } else { "unavailable" }, ssr: descriptor.ssr.label(),
            updates: if !available { "unavailable" } else if name == "react" && config.react.refresh { "experimental-refresh-incomplete" } else { "full-reload" }, checker: "unavailable",
            compiler_profiles: descriptor.compiler_profiles.iter().map(|profile| serde_json::json!({
                "host": profile.host, "framework_version": profile.framework_version,
                "client_compilation": profile.client.label(), "server_compilation": profile.server.label(),
                "fixture": profile.conformance_fixture, "requires_explicit_enable": profile.requires_explicit_enable
            })).collect(),
        });
    }
    let view = config.root.join(".ferrite/npm/node_modules");
    let recorded: Option<std::collections::BTreeMap<String, String>> =
        std::fs::read(view.join(".ferrite-projection.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let editor_view = if std::fs::read_link(config.root.join("node_modules"))
        .is_ok_and(|link| link == Path::new(".ferrite/npm/node_modules"))
    {
        if lock.importers.get(".").is_some_and(|root| {
            recorded.as_ref() == Some(&root.dependencies)
                && root.dependencies.iter().all(|(name, id)| {
                    match (
                        config.root.join("node_modules").join(name).canonicalize(),
                        config
                            .root
                            .join(".ferrite/npm/packages")
                            .join(id)
                            .canonicalize(),
                    ) {
                        (Ok(actual), Ok(expected)) => actual == expected,
                        _ => false,
                    }
                })
        }) {
            "locked-root-view"
        } else {
            "missing-or-mismatched"
        }
    } else {
        "missing-or-unmanaged"
    };
    if editor_view != "locked-root-view" {
        issues.push(Issue {
            severity: "warning",
            message: "editor root dependency view is missing or does not match the lockfile".into(),
            action: "run ferrite install (with --frozen-lockfile for an existing lock); move unmanaged node_modules aside explicitly first".into(),
        });
    }
    Ok(DoctorReport {
        schema_version: crate::registry::SCHEMA_VERSION,
        root: config.root.clone(),
        compiler: config.compiler.engine.clone(),
        compiler_version,
        ssr_runtime: config.runtime.backend.clone(),
        runtime_config: config.runtime.clone(),
        node_probe: if node_executable.is_some() {
            "located-not-executed"
        } else {
            "not-located"
        },
        node_executable,
        editor_view,
        frameworks,
        foreign_plugins,
        jsx_ownership_scope: JSX_SCAN_SCOPE,
        jsx_ownership,
        issues,
    })
}
fn locate_node(root: &Path, configured: Option<&Path>) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = if let Some(path) = configured {
        vec![if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        }]
    } else {
        std::env::var_os("PATH")
            .map(|path| {
                std::env::split_paths(&path)
                    .map(|dir| dir.join(if cfg!(windows) { "node.exe" } else { "node" }))
                    .collect()
            })
            .unwrap_or_default()
    };
    candidates.into_iter().find(|path| {
        let Ok(metadata) = std::fs::metadata(path) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_config::{resolve_config, CliOverrides, FrameworkConfig, UserConfig};
    fn config(root: &Path, explicit: bool) -> ResolvedConfig {
        let mut user = UserConfig::default();
        if explicit {
            user.framework = Some(FrameworkConfig {
                enabled: vec!["vue".into()],
                compiler_host: Some("node".into()),
                node: Some(root.join("missing-node")),
                ..Default::default()
            });
        }
        resolve_config(user, Some(root.to_path_buf()), CliOverrides::default()).unwrap()
    }
    #[test]
    fn jsx_scan_reports_nearest_packages_and_explicit_overrides() {
        let root = tempfile::tempdir().unwrap();
        for file in [
            "src/b.tsx",
            "src/a.jsx",
            "packages/widget/view.tsx",
            "node_modules/fake/view.jsx",
            "dist/view.jsx",
            "public/view.jsx",
            "release/view.jsx",
        ] {
            let file = root.path().join(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "export const View = () => <div />;").unwrap();
        }
        std::fs::write(
            root.path().join("package.json"),
            r#"{"dependencies":{"react":"19","solid-js":"1"}}"#,
        )
        .unwrap();
        let widget = root.path().join("packages/widget/package.json");
        std::fs::write(&widget, r#"{"peerDependencies":{"preact":"10"}}"#).unwrap();
        let mut resolved = config(root.path(), false);
        resolved.build.out_dir = "release".into();
        let reports = inspect_jsx_ownership(&resolved).unwrap();
        assert_eq!(reports.len(), 2);
        let project = reports
            .iter()
            .find(|row| row.manifest == Some(root.path().join("package.json")))
            .unwrap();
        assert_eq!(
            project.files,
            [PathBuf::from("src/a.jsx"), PathBuf::from("src/b.tsx")]
        );
        assert_eq!(project.ownership.status, "ambiguous");
        let nested = reports
            .iter()
            .find(|row| row.manifest == Some(widget.clone()))
            .unwrap();
        assert_eq!(nested.ownership.status, "unavailable");
        assert_eq!(nested.ownership.declared_owners, ["preact"]);
        let report = inspect(&resolved).unwrap();
        assert!(report.issues.iter().any(|issue| issue.severity == "error"
            && issue.message.contains("ambiguous native JSX ownership")));
        assert!(report.issues.iter().any(|issue| issue.severity == "error"
            && issue.message.contains("unavailable native JSX ownership")));
        resolved.framework = Some(FrameworkConfig {
            enabled: vec!["react".into()],
            compiler_host: Some("native".into()),
            ..Default::default()
        });
        assert!(inspect_jsx_ownership(&resolved)
            .unwrap()
            .iter()
            .all(|row| row.ownership.status == "selected"
                && row.ownership.selection == "explicit-framework"));
        // Explicit ownership must not fall back to inference on a malformed nested manifest.
        std::fs::write(&widget, "{").unwrap();
        let reports = inspect_jsx_ownership(&resolved).unwrap();
        assert!(reports
            .iter()
            .any(|row| row.manifest_error.is_some() && row.ownership.status == "selected"));
        resolved.framework = Some(FrameworkConfig::default());
        assert!(inspect_jsx_ownership(&resolved)
            .unwrap()
            .iter()
            .all(|row| row.ownership.status == "unowned"));
        resolved.framework = None;
        resolved.react.import_source = Some("custom-runtime".into());
        assert!(inspect_jsx_ownership(&resolved)
            .unwrap()
            .iter()
            .all(|row| row.ownership.status == "configured-lowering-unverified"));
        resolved.react.import_source = None;
        assert!(inspect_jsx_ownership(&resolved)
            .unwrap_err()
            .to_string()
            .contains("packages/widget/package.json"));
    }

    #[cfg(unix)]
    #[test]
    fn jsx_scan_does_not_follow_source_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("view.jsx"), "export default <div />;").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("linked")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("view.jsx"),
            root.path().join("view.jsx"),
        )
        .unwrap();
        assert!(inspect_jsx_ownership(&config(root.path(), false))
            .unwrap()
            .is_empty());
    }
    #[test]
    fn detected_components_require_explicit_host_and_do_not_spawn_or_write() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"dependencies":{"vue":"3.5.22"}}"#,
        )
        .unwrap();
        let report = inspect(&config(root.path(), false)).unwrap();
        let vue = report
            .frameworks
            .iter()
            .find(|row| row.framework == "vue")
            .unwrap();
        assert!(vue.active);
        assert_eq!(vue.selection, "manifest");
        assert_eq!(vue.compiler_host, None);
        assert_eq!(vue.client, "unavailable");
        assert_eq!(vue.ssr, "unavailable");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("compiler host is not enabled")));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        let report = inspect(&config(root.path(), true)).unwrap();
        assert_eq!(report.node_probe, "not-located");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn react_client_requires_concrete_runtime_and_refresh_dependencies() {
        use ferrite_npm::{LockedImporter, LockedPackage};
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("package.json"), r#"{"dependencies":{"react":"19.2.0","react-dom":"19.2.0"},"devDependencies":{"react-refresh":"0.17.0"}}"#).unwrap();
        let user = UserConfig {
            framework: Some(FrameworkConfig {
                enabled: vec!["react".into()],
                compiler_host: Some("native".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let resolved =
            resolve_config(user, Some(root.path().into()), CliOverrides::default()).unwrap();
        let mut lock = Lockfile::default();
        let mut importer = LockedImporter::default();
        for (name, version) in [
            ("react", "19.2.0"),
            ("react-dom", "19.2.0"),
            ("react-refresh", "0.17.0"),
        ] {
            let package = LockedPackage {
                name: name.into(),
                version: version.into(),
                source: "npm".into(),
                ..Default::default()
            };
            importer.specifiers.insert(name.into(), version.into());
            importer.dependencies.insert(name.into(), package.id());
            let directory = root.path().join(".ferrite/npm/packages").join(package.id());
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("package.json"),
                serde_json::json!({"name":name,"version":version}).to_string(),
            )
            .unwrap();
            lock.package.push(package);
        }
        lock.importers.insert(".".into(), importer);
        lock.write(&resolved.lockfile()).unwrap();
        let report = inspect(&resolved).unwrap();
        let react = report
            .frameworks
            .iter()
            .find(|row| row.framework == "react")
            .unwrap();
        assert_eq!(react.client, "experimental");
        assert_eq!(react.compiler_host.as_deref(), Some("native"));
        assert_eq!(react.ssr, "unavailable");
        let refresh = lock
            .package
            .iter()
            .find(|package| package.name == "react-refresh")
            .unwrap();
        std::fs::remove_file(
            root.path()
                .join(".ferrite/npm/packages")
                .join(refresh.id())
                .join("package.json"),
        )
        .unwrap();
        let report = inspect(&resolved).unwrap();
        assert_eq!(
            report
                .frameworks
                .iter()
                .find(|row| row.framework == "react")
                .unwrap()
                .client,
            "unavailable"
        );
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("installed react-refresh@0.17.0")));
        let mut without_refresh = resolved.clone();
        without_refresh.react.refresh = false;
        let report = inspect(&without_refresh).unwrap();
        let react = report
            .frameworks
            .iter()
            .find(|row| row.framework == "react")
            .unwrap();
        assert_eq!(react.client, "experimental");
        assert_eq!(react.updates, "full-reload");
        assert!(!report
            .issues
            .iter()
            .any(|issue| issue.message.contains("installed react-refresh@0.17.0")));
    }

    #[cfg(unix)]
    #[test]
    fn concrete_root_version_wins_and_located_host_is_never_executed() {
        use ferrite_npm::{LockedImporter, LockedPackage};
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"dependencies":{"vue":"3.5.22"}}"#,
        )
        .unwrap();
        let selected = LockedPackage {
            name: "vue".into(),
            version: "3.5.22".into(),
            source: "npm".into(),
            ..Default::default()
        };
        let other = LockedPackage {
            version: "3.4.0".into(),
            ..selected.clone()
        };
        let mut lock = Lockfile::default();
        lock.importers.insert(
            ".".into(),
            LockedImporter {
                specifiers: [("vue".into(), "3.5.22".into())].into(),
                dependencies: [("vue".into(), selected.id())].into(),
            },
        );
        lock.package = vec![other, selected.clone()];
        let resolved = config(root.path(), true);
        lock.write(&resolved.lockfile()).unwrap();
        let package = root
            .path()
            .join(".ferrite/npm/packages")
            .join(selected.id());
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"vue","version":"3.5.22"}"#,
        )
        .unwrap();
        let executable = root.path().join("missing-node");
        std::fs::write(&executable, "#!/bin/sh\nexit 99\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let report = inspect(&resolved).unwrap();
        let vue = report
            .frameworks
            .iter()
            .find(|row| row.framework == "vue")
            .unwrap();
        assert_eq!(vue.version.as_deref(), Some("3.5.22"));
        assert_eq!(vue.identity, Some(selected.id()));
        assert_eq!(vue.client, "experimental");
        assert_eq!(report.node_probe, "located-not-executed");
        assert!(!root.path().join(".ferrite/npm/node_modules").exists());
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"vue","version":"3.4.0"}"#,
        )
        .unwrap();
        let report = inspect(&resolved).unwrap();
        assert_eq!(
            report
                .frameworks
                .iter()
                .find(|row| row.framework == "vue")
                .unwrap()
                .client,
            "unavailable"
        );
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("stored identity differs")));
    }
    #[test]
    fn explicit_disabling_overrides_detection_and_dangling_views_are_not_healthy() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("package.json"),
            r#"{"dependencies":{"vue":"3.5.22"}}"#,
        )
        .unwrap();
        let user = UserConfig {
            framework: Some(FrameworkConfig::default()),
            ..Default::default()
        };
        let resolved = resolve_config(
            user,
            Some(root.path().to_path_buf()),
            CliOverrides::default(),
        )
        .unwrap();
        let report = inspect(&resolved).unwrap();
        assert!(
            !report
                .frameworks
                .iter()
                .find(|row| row.framework == "vue")
                .unwrap()
                .active
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                ".ferrite/npm/node_modules",
                root.path().join("node_modules"),
            )
            .unwrap();
            assert_eq!(
                inspect(&resolved).unwrap().editor_view,
                "missing-or-mismatched"
            );
        }
    }
}

#[cfg(test)]
mod foreign_profile_tests {
    #[test]
    fn configured_foreign_plugins_are_reported_without_execution() {
        let root = tempfile::tempdir().unwrap();
        let entry = root.path().join("plugin.mjs");
        std::fs::write(&entry, "throw new Error('doctor must not execute');").unwrap();
        let user: ferrite_config::UserConfig = serde_json::from_value(serde_json::json!({"foreign_plugins":[{"name":"fixture", "entry":"plugin.mjs", "host":"node", "node":"missing-node"}]})).unwrap();
        let config =
            ferrite_config::resolve_config(user, Some(root.path().into()), Default::default())
                .unwrap();
        let report = super::inspect(&config).unwrap();
        assert_eq!(report.foreign_plugins[0]["name"], "fixture");
        assert_eq!(report.foreign_plugins[0]["entryExists"], true);
        assert_eq!(report.foreign_plugins[0]["probe"], "not-located");
        assert_eq!(report.foreign_plugins[0]["execution"], "unverified");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.severity == "error"
                && issue.message.contains("foreign plugin fixture")));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
