//! Module resolver.

use crate::exports::*;
use crate::node::*;
use crate::side_effects::*;
use crate::specifier::*;
use crate::types::*;
use crate::util::*;
use ferrite_config::ResolveConfig;
use ferrite_core::EnvironmentKind;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// The resolver.
#[derive(Debug, Clone)]
pub struct Resolver {
    /// Project root.
    pub root: PathBuf,
    /// Active export conditions; package key order defines priority.
    pub conditions: Vec<String>,
    /// Extensions to probe.
    pub extensions: Vec<String>,
    /// Import aliases.
    pub alias: HashMap<String, String>,
    /// Preserve symlinks.
    pub preserve_symlinks: bool,
    /// npm store root (`.ferrite/npm/packages`).
    pub npm_store: PathBuf,
    /// Concrete package graph used for importer-specific npm resolution.
    pub lockfile: PathBuf,
}

impl Resolver {
    /// Build a resolver from config.
    #[must_use]
    pub fn new(root: PathBuf, config: &ResolveConfig) -> Self {
        Self {
            npm_store: root.join(".ferrite/npm/packages"),
            lockfile: root.join("ferrite.lock"),
            root,
            conditions: config.conditions.clone(),
            extensions: config.extensions.clone(),
            alias: config.alias.clone(),
            preserve_symlinks: config.preserve_symlinks,
        }
    }

    /// Build a resolver with environment-specific default conditions.
    #[must_use]
    pub fn for_environment(root: PathBuf, config: &ResolveConfig, env: &EnvironmentKind) -> Self {
        let mut resolver = Self::new(root, config);
        if config.conditions == ResolveConfig::default().conditions {
            resolver.conditions = env.default_conditions();
        }
        resolver
    }

    /// Resolve a specifier.
    pub fn resolve(&self, request: &ResolveRequest<'_>) -> Result<ResolvedId> {
        let desired = if request.kind == ResolveKind::Require {
            "require"
        } else {
            "import"
        };
        let unwanted = if request.kind == ResolveKind::Require {
            "import"
        } else {
            "require"
        };
        if !self.conditions.iter().any(|condition| condition == desired)
            || self
                .conditions
                .iter()
                .any(|condition| condition == unwanted)
            || (request.kind == ResolveKind::Require
                && self
                    .conditions
                    .iter()
                    .any(|condition| condition == "module"))
        {
            let mut selected = self.clone();
            selected.conditions.retain(|condition| {
                condition != unwanted
                    && !(request.kind == ResolveKind::Require && condition == "module")
            });
            if !selected
                .conditions
                .iter()
                .any(|condition| condition == desired)
            {
                selected.conditions.push(desired.into());
            }
            return selected.resolve(request);
        }
        let (specifier, query) = split_query(request.specifier);

        // 1. Already-internal virtual modules pass through.
        if specifier.starts_with('\0') {
            return Ok(ResolvedId::new(request.specifier));
        }
        // 2. `virtual:*` maps to the internal `\0` id (§14).
        if let Some(rest) = specifier.strip_prefix("virtual:") {
            let id = format!("\0virtual:{rest}");
            return Ok(ResolvedId::new(attach_query(&id, query)));
        }
        // 3. Node builtins (§19).
        if let Some(name) = specifier.strip_prefix("node:") {
            return Ok(ResolvedId::external(format!("node:{name}"))
                .with_meta(serde_json::json!({"node_builtin": name})));
        }
        if is_bare_node_builtin(specifier) {
            return Ok(ResolvedId::external(specifier)
                .with_meta(serde_json::json!({"node_builtin": specifier})));
        }
        // 4. Remote / data URLs (§72).
        if specifier.starts_with("https://") || specifier.starts_with("http://") {
            return Ok(ResolvedId::external(request.specifier)
                .with_meta(serde_json::json!({"remote": true})));
        }
        if specifier.starts_with("data:") || specifier.starts_with("blob:") {
            return Ok(ResolvedId::external(request.specifier));
        }
        // 5. `rust:` packages (§74) resolve to a virtual module handled by the
        //    Rust/WASM plugin.
        if let Some(name) = specifier.strip_prefix("rust:") {
            return Ok(ResolvedId::new(format!("\0rust:{name}"))
                .with_meta(serde_json::json!({"rust_package": name})));
        }
        // 6. Aliases.
        if let Some(mapped) = self.apply_alias(specifier) {
            let mapped_owned = attach_query(&mapped, query);
            let nested = ResolveRequest {
                specifier: &mapped_owned,
                importer: request.importer,
                environment: request.environment.clone(),
                kind: request.kind,
            };
            return self.resolve(&nested);
        }
        // 7. Absolute paths (root-relative).
        if specifier.starts_with('/') && !specifier.starts_with("/@npm/") {
            let file = self.root.join(specifier.trim_start_matches('/'));
            return self.finalize_file(&file, query, None);
        }
        // 8. Already-resolved npm URLs pass through.
        if let Some(rest) = specifier.strip_prefix("/@npm/") {
            let file = self.npm_store.join(rest);
            return self.finalize_npm_file(&file, specifier, query);
        }
        // 9. Relative imports.
        if specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier == "."
            || specifier == ".."
        {
            let base_dir = request
                .importer
                .map(|importer| self.importer_dir(importer))
                .unwrap_or_else(|| self.root.clone());
            let file = base_dir.join(specifier);
            if is_under(&file, &self.npm_store) {
                let url = self.npm_path_to_url(&file);
                return self.finalize_npm_file(&file, &url, query);
            }
            return self.finalize_file(&file, query, None);
        }
        // 10. `#` imports (package `imports` field).
        if let Some(rest) = specifier.strip_prefix('#') {
            let base_dir = request
                .importer
                .map(|importer| self.importer_dir(importer))
                .unwrap_or_else(|| self.root.clone());
            if let Some(resolved) = self.resolve_package_imports(&base_dir, rest, query)? {
                return Ok(resolved);
            }
            return Err(FerriteError::Resolve(format!(
                "cannot resolve package import `#{rest}` from {}",
                request.importer.map_or("<root>", |id| id.0.as_str())
            )));
        }
        // 11. Bare npm imports.
        self.resolve_bare(specifier, request, query)
    }

    /// Async wrapper (resolution is fast/local; kept async for API symmetry).
    pub async fn resolve_async(&self, request: &ResolveRequest<'_>) -> Result<ResolvedId> {
        self.resolve(request)
    }

    /// Directory containing the importer.
    pub(crate) fn importer_dir(&self, importer: &ModuleId) -> PathBuf {
        let (path, _) = importer.split_query();
        let path = path.trim_start_matches('\0');
        if let Some(rest) = path.strip_prefix("/@npm/") {
            self.npm_store
                .join(rest)
                .parent()
                .map_or_else(|| self.npm_store.clone(), std::path::Path::to_path_buf)
        } else if let Some(file) = path.strip_prefix("/@fs/") {
            PathBuf::from(format!("/{file}"))
                .parent()
                .map_or_else(|| self.root.clone(), Path::to_path_buf)
        } else if path.starts_with('/') {
            self.root
                .join(path.trim_start_matches('/'))
                .parent()
                .map_or_else(|| self.root.clone(), std::path::Path::to_path_buf)
        } else {
            self.root.clone()
        }
    }

    /// Apply the longest matching alias, if any.
    pub(crate) fn apply_alias(&self, specifier: &str) -> Option<String> {
        let mut best: Option<(&str, &str)> = None;
        for (from, to) in &self.alias {
            let matches = specifier == from
                || specifier
                    .strip_prefix(from)
                    .is_some_and(|rest| rest.starts_with('/'));
            if matches && best.is_none_or(|(prev, _)| from.len() > prev.len()) {
                best = Some((from, to));
            }
        }
        best.map(|(from, to)| {
            let rest = specifier.strip_prefix(from).unwrap_or("");
            if to.starts_with("./") || to.starts_with("../") {
                format!(
                    "/{}",
                    self.root
                        .join(to)
                        .strip_prefix(&self.root)
                        .unwrap_or_else(|_| Path::new(to))
                        .to_string_lossy()
                        .replace('\\', "/")
                )
                .trim_end_matches('/')
                .to_string()
                    + rest
            } else {
                format!("{to}{rest}")
            }
        })
    }

    /// Resolve a bare package specifier.
    pub(crate) fn resolve_bare(
        &self,
        specifier: &str,
        request: &ResolveRequest<'_>,
        query: Option<&str>,
    ) -> Result<ResolvedId> {
        let (name, subpath) = split_package(specifier);
        ferrite_npm::validate_package_name(&name)?;
        if self.lockfile.exists() {
            let lock = ferrite_npm::Lockfile::read(&self.lockfile)?;
            let base = request
                .importer
                .map(|importer| self.importer_dir(importer))
                .unwrap_or_else(|| self.root.clone());
            let owner = lock
                .package
                .iter()
                .find(|package| base.starts_with(self.npm_store.join(package.id())));
            let target = if let Some(owner) = owner {
                if owner.name == name {
                    Some(owner.id())
                } else {
                    owner.dependencies.get(&name).cloned()
                }
            } else {
                let importer = lock
                    .importers
                    .iter()
                    .filter(|(path, _)| base.starts_with(self.root.join(path)))
                    .max_by_key(|(path, _)| path.len());
                importer.and_then(|(_, root)| root.dependencies.get(&name).cloned())
            };
            let id = target.ok_or_else(|| FerriteError::Resolve(format!(
                "no locked dependency edge for `{name}` from {}; run ferrite install to record importer dependencies",
                base.display()
            )))?;
            let dir = self.npm_store.join(&id);
            if !dir.join("package.json").exists() {
                return Err(FerriteError::Resolve(format!(
                    "locked package {id} is missing; run ferrite install --frozen-lockfile"
                )));
            }
            return self.resolve_in_package(&dir, &name, &subpath, query, true);
        }
        // Legacy manually populated stores remain usable only when unambiguous.
        if let Some(dir) = self.find_store_package(&name)? {
            return self.resolve_in_package(&dir, &name, &subpath, query, true);
        }
        // node_modules fallback (compatibility).
        let base_dir = request
            .importer
            .map(|importer| self.importer_dir(importer))
            .unwrap_or_else(|| self.root.clone());
        if let Some(dir) = find_node_modules(&base_dir, &name) {
            return self.resolve_in_package(&dir, &name, &subpath, query, false);
        }
        Err(FerriteError::Resolve(format!(
            "cannot resolve `{specifier}` (package `{name}` is not installed; run `ferrite add {name}`)"
        )))
    }

    /// Resolve inside a package directory via exports/main/module.
    pub(crate) fn resolve_in_package(
        &self,
        dir: &Path,
        name: &str,
        subpath: &str,
        query: Option<&str>,
        in_store: bool,
    ) -> Result<ResolvedId> {
        let pkg = read_package_json(dir);
        let export_subpath = if subpath.is_empty() {
            "."
        } else {
            &format!(".{subpath}")
        };
        // 1. `exports` field.
        if let Some(exports) = pkg.as_ref().and_then(|pkg| pkg.exports.clone()) {
            if let Some(target) = resolve_exports(&exports, export_subpath, &self.conditions) {
                let file = dir.join(target.trim_start_matches("./"));
                return self.finalize_package_file(dir, &file, query, pkg.as_ref(), in_store);
            }
            return Err(FerriteError::Resolve(format!(
                "package `{name}` does not export `{export_subpath}`"
            )));
        }
        // 2. Subpath file.
        if !subpath.is_empty() {
            let file = dir.join(subpath.trim_start_matches('/'));
            return self.finalize_package_file(dir, &file, query, pkg.as_ref(), in_store);
        }
        // 3. `module`/`main` fields, then index.
        if let Some(pkg) = pkg.as_ref() {
            for entry in if self
                .conditions
                .iter()
                .any(|condition| condition == "require")
            {
                [&pkg.main, &None]
            } else {
                [&pkg.module, &pkg.main]
            }
            .into_iter()
            .flatten()
            {
                let file = dir.join(entry.trim_start_matches("./"));
                if let Ok(resolved) =
                    self.finalize_package_file(dir, &file, query, Some(pkg), in_store)
                {
                    return Ok(resolved);
                }
            }
        }
        let file = dir.join("index.js");
        self.finalize_package_file(dir, &file, query, pkg.as_ref(), in_store)
    }

    /// Resolve `#` imports via the nearest package.json `imports` field.
    pub(crate) fn resolve_package_imports(
        &self,
        base_dir: &Path,
        name: &str,
        query: Option<&str>,
    ) -> Result<Option<ResolvedId>> {
        let mut dir = base_dir.to_path_buf();
        loop {
            // Like Node, keep walking up past a package.json that lacks a
            // matching `imports` entry instead of stopping at the first one.
            let candidate = dir.join("package.json");
            if candidate.exists() {
                if let Ok(text) = std::fs::read_to_string(&candidate) {
                    if let Ok(pkg) = serde_json::from_str::<PackageJson>(&text) {
                        if let Some(imports) = pkg.imports {
                            let key = format!("#{name}");
                            if let Some(target) = resolve_exports(&imports, &key, &self.conditions)
                            {
                                let file = dir.join(target.trim_start_matches("./"));
                                return self.finalize_file(&file, query, None).map(Some);
                            }
                        }
                    }
                }
            }
            if !dir.pop() {
                return Ok(None);
            }
        }
    }

    /// Find an installed package in the Ferrite store.
    pub(crate) fn find_store_package(&self, name: &str) -> Result<Option<PathBuf>> {
        let (namespace, leaf) = name
            .rsplit_once('/')
            .map_or((self.npm_store.clone(), name), |(scope, leaf)| {
                (self.npm_store.join(scope), leaf)
            });
        let Ok(entries) = std::fs::read_dir(namespace) else {
            return Ok(None);
        };
        let mut matches: Vec<_> = entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(&format!("{leaf}@"))
                    && entry.path().join("package.json").is_file()
            })
            .map(|entry| entry.path())
            .collect();
        matches.sort();
        match matches.len() {
            0 => Ok(None),
            1 => Ok(matches.pop()),
            _ => Err(FerriteError::Resolve(format!("multiple versions of {name} exist without an importer lock edge; run ferrite install instead of selecting an arbitrary version"))),
        }
    }

    /// Finalize a project file resolution.
    pub(crate) fn finalize_file(
        &self,
        file: &Path,
        query: Option<&str>,
        side_effects: Option<bool>,
    ) -> Result<ResolvedId> {
        let probed = probe_file(file, &self.extensions)
            .ok_or_else(|| FerriteError::Resolve(format!("cannot resolve `{}`", file.display())))?;
        let canonical = if self.preserve_symlinks {
            probed.clone()
        } else {
            probed.canonicalize().unwrap_or(probed.clone())
        };
        let url = if is_under(&canonical, &self.root) {
            format!(
                "/{}",
                canonical
                    .strip_prefix(&self.root)
                    .unwrap_or(&canonical)
                    .to_string_lossy()
                    .replace('\\', "/")
            )
        } else {
            // Outside root (linked package): serve via /@fs/ prefix (Vite-style).
            format!("/@fs{}", canonical.to_string_lossy().replace('\\', "/"))
        };
        // If the file wasn't canonicalized (symlinks preserved), prefer the
        // root-relative form when possible.
        let url = if url.starts_with("/@fs") {
            if let Ok(relative) = probed.strip_prefix(&self.root) {
                format!("/{}", relative.to_string_lossy().replace('\\', "/"))
            } else {
                url
            }
        } else {
            url
        };
        let module_type = ModuleType::from_path(&probed);
        let mut resolved = ResolvedId::new(attach_query(&url, query));
        resolved.module_type = Some(module_type);
        resolved.side_effects = side_effects;
        Ok(resolved)
    }

    /// Finalize an npm-store file resolution.
    pub(crate) fn finalize_npm_file(
        &self,
        file: &Path,
        url_hint: &str,
        query: Option<&str>,
    ) -> Result<ResolvedId> {
        let probed = probe_file(file, &self.extensions)
            .ok_or_else(|| FerriteError::Resolve(format!("cannot resolve `{url_hint}`")))?;
        let url = self.npm_path_to_url(&probed);
        let module_type = ModuleType::from_path(&probed);
        let side_effects = package_side_effects(&probed);
        let mut resolved = ResolvedId::new(attach_query(&url, query));
        resolved.module_type = Some(module_type);
        resolved.side_effects = side_effects;
        Ok(resolved)
    }

    /// Finalize a file inside a package directory.
    pub(crate) fn finalize_package_file(
        &self,
        dir: &Path,
        file: &Path,
        query: Option<&str>,
        pkg: Option<&PackageJson>,
        in_store: bool,
    ) -> Result<ResolvedId> {
        let probed = probe_file(file, &self.extensions)
            .ok_or_else(|| FerriteError::Resolve(format!("cannot resolve `{}`", file.display())))?;
        let side_effects = pkg.and_then(|pkg| side_effects_for(pkg, dir, &probed));
        let module_type = ModuleType::from_path(&probed);
        if in_store {
            let url = self.npm_path_to_url(&probed);
            let mut resolved = ResolvedId::new(attach_query(&url, query));
            resolved.module_type = Some(module_type);
            resolved.side_effects = side_effects;
            resolved.meta = serde_json::json!({
                "package": pkg.and_then(|p| p.name.clone()),
                "version": pkg.and_then(|p| p.version.clone()),
            });
            Ok(resolved)
        } else {
            // node_modules fallback: still serve through the npm URL space when
            // the package has a name, otherwise as a file URL.
            if let Some(name) = pkg.and_then(|p| p.name.clone()) {
                let version = pkg
                    .and_then(|p| p.version.clone())
                    .unwrap_or_else(|| "0.0.0".to_string());
                let relative = probed.strip_prefix(dir).unwrap_or(&probed);
                let url = format!(
                    "/@npm/{name}@{version}/{}",
                    relative.to_string_lossy().replace('\\', "/")
                );
                let mut resolved = ResolvedId::new(attach_query(&url, query));
                resolved.module_type = Some(module_type);
                resolved.side_effects = side_effects;
                return Ok(resolved);
            }
            self.finalize_file(&probed, query, side_effects)
        }
    }

    /// Convert a store path to its `/@npm/` URL.
    pub(crate) fn npm_path_to_url(&self, path: &Path) -> String {
        let relative = path.strip_prefix(&self.npm_store).unwrap_or(path);
        format!("/@npm/{}", relative.to_string_lossy().replace('\\', "/"))
    }

    /// Convert an `/@npm/` URL to a store path.
    #[must_use]
    pub fn npm_url_to_path(&self, url: &str) -> PathBuf {
        let trimmed = url.trim_start_matches("/@npm/");
        self.npm_store.join(trimmed)
    }
}
