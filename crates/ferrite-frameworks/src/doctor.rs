//! Read-only capability diagnostics. Locating a host never establishes execution support.
use ferrite_config::ResolvedConfig;
use ferrite_core::Result;
use ferrite_npm::{JsPackageJson, Lockfile};
use serde::Serialize;
use std::path::{Path, PathBuf};

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
    pub node_executable: Option<PathBuf>,
    pub node_probe: &'static str,
    pub editor_view: &'static str,
    pub frameworks: Vec<FrameworkReport>,
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
        frameworks.push(FrameworkReport {
            framework: name, active, selection: if explicit.is_some() { "configuration" } else { "manifest" },
            version: package.map(|package| package.version.clone()), identity: package.map(|package| package.id()),
            compiler_host: host, compiler_support: if available { "experimental" } else { "unavailable" },
            client: if available && name != "react" { descriptor.client.label() } else { "unavailable" }, ssr: descriptor.ssr.label(),
            updates: if !available { "unavailable" } else if name == "react" { "experimental-refresh-incomplete" } else { "full-reload" }, checker: "unavailable",
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
        node_probe: if node_executable.is_some() {
            "located-not-executed"
        } else {
            "not-located"
        },
        node_executable,
        editor_view,
        frameworks,
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
