//! Portable lock activation without registry metadata.
use crate::Lockfile;
use ferrite_core::{FerriteError, Result};
use std::collections::{BTreeMap, BTreeSet};
/// npm's target names; explicit values permit deterministic cross-platform replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlatform {
    pub os: String,
    pub cpu: String,
    pub libc: Option<String>,
}
impl InstallPlatform {
    pub fn current() -> Self {
        Self {
            os: match std::env::consts::OS {
                "macos" => "darwin",
                "windows" => "win32",
                value => value,
            }
            .into(),
            cpu: match std::env::consts::ARCH {
                "x86_64" => "x64",
                "aarch64" => "arm64",
                "x86" => "ia32",
                value => value,
            }
            .into(),
            libc: if cfg!(target_os = "linux") {
                Some(
                    if cfg!(target_env = "musl") {
                        "musl"
                    } else {
                        "glibc"
                    }
                    .into(),
                )
            } else {
                None
            },
        }
    }
}
/// Reachable compatible packages and filtered concrete importer/package edges.
#[derive(Debug, Default)]
pub struct ActiveGraph {
    pub packages: BTreeSet<String>,
    pub importers: BTreeMap<String, BTreeMap<String, String>>,
    pub dependencies: BTreeMap<String, BTreeMap<String, String>>,
}
fn matches(list: &Option<Vec<String>>, value: Option<&str>) -> bool {
    let Some(list) = list else {
        return true;
    };
    if list.len() == 1 && list[0] == "any" {
        return true;
    }
    let mut negated = 0;
    let mut matched = false;
    for entry in list {
        if let Some(denied) = entry.strip_prefix('!') {
            negated += 1;
            if Some(denied) == value {
                return false;
            }
        } else {
            matched |= Some(entry.as_str()) == value;
        }
    }
    matched || negated == list.len()
}
impl Lockfile {
    pub fn active_graph(&self, platform: &InstallPlatform) -> Result<ActiveGraph> {
        self.validate()?;
        let mut incompatible: BTreeSet<_> = self
            .package
            .iter()
            .filter(|package| {
                !matches(&package.os, Some(&platform.os))
                    || !matches(&package.cpu, Some(&platform.cpu))
                    || package.libc.is_some()
                        && (platform.libc.is_none()
                            || !matches(&package.libc, platform.libc.as_deref()))
            })
            .map(|package| package.id())
            .collect();
        let packages: BTreeMap<_, _> = self
            .package
            .iter()
            .map(|package| (package.id(), package))
            .collect();
        let mut required_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, package) in &packages {
            for (name, target) in &package.dependencies {
                if !package.optional_dependencies.contains(name) {
                    required_by
                        .entry(target.clone())
                        .or_default()
                        .push(id.clone());
                }
            }
        }
        let mut failed: Vec<_> = incompatible.iter().cloned().collect();
        while let Some(id) = failed.pop() {
            for parent in required_by.get(&id).into_iter().flatten() {
                if incompatible.insert(parent.clone()) {
                    failed.push(parent.clone());
                }
            }
        }
        let mut active = ActiveGraph::default();
        let mut pending = Vec::new();
        for (path, importer) in &self.importers {
            let mut edges = BTreeMap::new();
            for (name, id) in &importer.dependencies {
                if incompatible.contains(id) {
                    if importer.optional_dependencies.contains(name) {
                        continue;
                    }
                    return Err(FerriteError::Npm(format!(
                        "required dependency {path} -> {name} is incompatible with {}/{}/{:?}",
                        platform.os, platform.cpu, platform.libc
                    )));
                }
                edges.insert(name.clone(), id.clone());
                pending.push(id.clone());
            }
            active.importers.insert(path.clone(), edges);
        }
        for package in self
            .package
            .iter()
            .filter(|package| package.source == "workspace")
        {
            if incompatible.contains(&package.id()) {
                return Err(FerriteError::Npm(format!(
                    "workspace {} is incompatible with this platform",
                    package.name
                )));
            }
            pending.push(package.id());
        }
        while let Some(id) = pending.pop() {
            if !active.packages.insert(id.clone()) {
                continue;
            }
            let package = packages[&id];
            let edges: BTreeMap<_, _> = package
                .dependencies
                .iter()
                .filter(|(_, target)| !incompatible.contains(*target))
                .map(|(name, target)| (name.clone(), target.clone()))
                .collect();
            pending.extend(edges.values().cloned());
            active.dependencies.insert(id, edges);
        }
        Ok(active)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npm_platform_lists_match_negation_and_single_any_semantics() {
        let list = |values: &[&str]| Some(values.iter().map(|value| value.to_string()).collect());
        assert!(matches(&list(&["any"]), Some("linux")));
        assert!(!matches(&list(&["any", "darwin"]), Some("linux")));
        assert!(matches(&list(&["!darwin"]), Some("linux")));
        assert!(!matches(&list(&["linux", "!linux"]), Some("linux")));
        assert!(matches(&list(&["!any"]), Some("linux")));
    }
}
