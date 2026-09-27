//! Module resolution (spec §15, §73).
//!
//! Handles relative/absolute imports, aliases, bare npm imports, package
//! `exports`/`imports`, conditions, directory indexes, symlinks, CSS/URL
//! imports, and virtual modules.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ferrite_config::ResolveConfig;
use ferrite_core::{EnvironmentKind, FerriteError, ModuleId, ModuleType, Result};

/// What kind of import triggered resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveKind {
    /// Static ESM import / re-export.
    Import,
    /// CommonJS `require()`.
    Require,
    /// Dynamic `import()`.
    DynamicImport,
    /// CSS `@import` / CSS file import.
    Css,
    /// `?url` / `new URL(..., import.meta.url)`.
    Url,
    /// Worker import.
    Worker,
    /// WASM import.
    Wasm,
}

/// A resolution request (§15).
#[derive(Debug, Clone)]
pub struct ResolveRequest<'a> {
    /// Raw specifier text.
    pub specifier: &'a str,
    /// Importing module, if any.
    pub importer: Option<&'a ModuleId>,
    /// Target environment.
    pub environment: EnvironmentKind,
    /// Import kind.
    pub kind: ResolveKind,
}

/// A resolved module id (§15).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResolvedId {
    /// Resolved module id.
    pub id: ModuleId,
    /// True when the module must not be bundled/transformed.
    pub external: bool,
    /// Package `sideEffects` flag, when known.
    pub side_effects: Option<bool>,
    /// Detected module type, when known.
    pub module_type: Option<ModuleType>,
    /// Extra metadata (`node_builtin`, `remote`, `package`, ...).
    pub meta: serde_json::Value,
}

impl ResolvedId {
    /// Create an internal resolved id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: ModuleId::new(id.into()),
            external: false,
            side_effects: None,
            module_type: None,
            meta: serde_json::Value::Null,
        }
    }

    /// Create an external resolved id.
    #[must_use]
    pub fn external(id: impl Into<String>) -> Self {
        let mut resolved = Self::new(id);
        resolved.external = true;
        resolved
    }

    /// Attach metadata.
    #[must_use]
    pub fn with_meta(mut self, meta: serde_json::Value) -> Self {
        self.meta = meta;
        self
    }
}

/// Minimal `package.json` view used by the resolver.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PackageJson {
    /// Package name.
    pub name: Option<String>,
    /// Package version.
    pub version: Option<String>,
    /// CJS entry.
    pub main: Option<String>,
    /// ESM entry.
    pub module: Option<String>,
    /// Export map.
    pub exports: Option<serde_json::Value>,
    /// Import map (`#` specifiers).
    pub imports: Option<serde_json::Value>,
    /// Side-effects flag.
    #[serde(rename = "sideEffects")]
    pub side_effects: Option<SideEffects>,
    /// Whether the package is ESM (`"type": "module"`).
    #[serde(rename = "type")]
    pub package_type: Option<String>,
}

/// `sideEffects` field shapes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum SideEffects {
    /// Boolean form.
    Bool(bool),
    /// Glob list form.
    Globs(Vec<String>),
}

/// The resolver.
#[derive(Debug, Clone)]
pub struct Resolver {
    /// Project root.
    pub root: PathBuf,
    /// Export conditions (ordered).
    pub conditions: Vec<String>,
    /// Extensions to probe.
    pub extensions: Vec<String>,
    /// Import aliases.
    pub alias: HashMap<String, String>,
    /// Preserve symlinks.
    pub preserve_symlinks: bool,
    /// npm store root (`.ferrite/npm/packages`).
    pub npm_store: PathBuf,
}

impl Resolver {
    /// Build a resolver from config.
    #[must_use]
    pub fn new(root: PathBuf, config: &ResolveConfig) -> Self {
        Self {
            npm_store: root.join(".ferrite/npm/packages"),
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
    fn importer_dir(&self, importer: &ModuleId) -> PathBuf {
        let (path, _) = importer.split_query();
        let path = path.trim_start_matches('\0');
        if let Some(rest) = path.strip_prefix("/@npm/") {
            self.npm_store
                .join(rest)
                .parent()
                .map_or_else(|| self.npm_store.clone(), std::path::Path::to_path_buf)
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
    fn apply_alias(&self, specifier: &str) -> Option<String> {
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
    fn resolve_bare(
        &self,
        specifier: &str,
        request: &ResolveRequest<'_>,
        query: Option<&str>,
    ) -> Result<ResolvedId> {
        let (name, subpath) = split_package(specifier);
        // Installed Ferrite store first (no node_modules required).
        if let Some(dir) = self.find_store_package(&name) {
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
    fn resolve_in_package(
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
            for entry in [&pkg.module, &pkg.main].into_iter().flatten() {
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
    fn resolve_package_imports(
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
    fn find_store_package(&self, name: &str) -> Option<PathBuf> {
        // Layout: `.ferrite/npm/packages/<name>@<version>/`.
        let entries = std::fs::read_dir(&self.npm_store).ok()?;
        let mut best: Option<(SemverLikeKey, PathBuf)> = None;
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if let Some(rest) = file_name.strip_prefix(&format!("{name}@")) {
                let key = semver_like_key(rest);
                if best.as_ref().is_none_or(|(prev, _)| key > *prev) {
                    best = Some((key, entry.path()));
                }
            }
        }
        best.map(|(_, path)| path)
    }

    /// Finalize a project file resolution.
    fn finalize_file(
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
    fn finalize_npm_file(
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
    fn finalize_package_file(
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
    fn npm_path_to_url(&self, path: &Path) -> String {
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

/// Split `specifier?query`.
fn split_query(specifier: &str) -> (&str, Option<&str>) {
    match specifier.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (specifier, None),
    }
}

/// Re-attach a query string.
fn attach_query(id: &str, query: Option<&str>) -> String {
    match query {
        Some(query) => format!("{id}?{query}"),
        None => id.to_string(),
    }
}

/// Split a bare specifier into `(package_name, /subpath)`.
fn split_package(specifier: &str) -> (String, String) {
    if let Some(rest) = specifier.strip_prefix('@') {
        // Scoped: `@scope/name/...`.
        let mut parts = rest.splitn(3, '/');
        let scope = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        let tail = parts.next().unwrap_or("");
        let package = format!("@{scope}/{name}");
        if tail.is_empty() {
            (package, String::new())
        } else {
            (package, format!("/{tail}"))
        }
    } else {
        match specifier.split_once('/') {
            Some((name, tail)) => (name.to_string(), format!("/{tail}")),
            None => (specifier.to_string(), String::new()),
        }
    }
}

/// Probe exact path, `+extensions`, and directory `index.*`.
fn probe_file(file: &Path, extensions: &[String]) -> Option<PathBuf> {
    if file.is_file() {
        return Some(file.to_path_buf());
    }
    let with_ext: Option<PathBuf> = extensions.iter().find_map(|ext| {
        let candidate = PathBuf::from(format!("{}{ext}", file.to_string_lossy()));
        candidate.is_file().then_some(candidate)
    });
    if with_ext.is_some() {
        return with_ext;
    }
    if file.is_dir() {
        for index in ["index", "main"] {
            for ext in extensions {
                let candidate = file.join(format!("{index}{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    // Extension-less file that exists (e.g. LICENSE-style or extensionless bin).
    if file.exists() && !file.is_dir() {
        return Some(file.to_path_buf());
    }
    None
}

/// Walk up looking for `node_modules/<name>` (compatibility fallback).
fn find_node_modules(base: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = base.to_path_buf();
    loop {
        let candidate = dir.join("node_modules").join(name);
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Read and parse a package.json (best effort).
fn read_package_json(dir: &Path) -> Option<PackageJson> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Look up `sideEffects` for a file inside a package.
fn side_effects_for(pkg: &PackageJson, dir: &Path, file: &Path) -> Option<bool> {
    match &pkg.side_effects {
        None => None,
        Some(SideEffects::Bool(value)) => Some(*value),
        Some(SideEffects::Globs(globs)) => {
            let relative = file
                .strip_prefix(dir)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            // Conservative: side effects unless a `!`-negated glob matches...
            // Simplified: true when any positive glob matches the extension set.
            let matched = globs.iter().any(|glob| glob_match(glob, &relative));
            Some(matched)
        }
    }
}

/// Best-effort `sideEffects` lookup by walking up to the owning package.
fn package_side_effects(file: &Path) -> Option<bool> {
    let mut dir = file.parent()?.to_path_buf();
    loop {
        let candidate = dir.join("package.json");
        if candidate.exists() {
            if let Ok(text) = std::fs::read_to_string(&candidate) {
                if let Ok(pkg) = serde_json::from_str::<PackageJson>(&text) {
                    return side_effects_for(&pkg, &dir, file);
                }
            }
            return None;
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Tiny glob matcher (`*`, `**`, `?`, trailing `/...`).
fn glob_match(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_start_matches("./");
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        let mut rest = path;
        for (i, part) in parts.iter().enumerate() {
            let part = part.trim_matches('/');
            if part.is_empty() {
                continue;
            }
            if i == 0 {
                if !segment_match(part, rest) {
                    // prefix piece must match at the start (up to `/`)
                    if let Some(pos) = rest.find('/') {
                        if !segment_match(part, &rest[..pos]) {
                            return false;
                        }
                        rest = &rest[pos + 1..];
                    } else {
                        return segment_match(part, rest);
                    }
                } else if let Some(pos) = rest.find('/') {
                    rest = &rest[pos + 1..];
                }
            } else if i == parts.len() - 1 {
                return rest.split('/').any(|segment| segment_match(part, segment))
                    || segment_match(part, rest);
            }
        }
        return true;
    }
    if pattern.contains('/') {
        segment_match(pattern, path)
    } else {
        path.split('/')
            .any(|segment| segment_match(pattern, segment))
    }
}

fn segment_match(pattern: &str, text: &str) -> bool {
    let (mut px, mut tx) = (pattern.as_bytes(), text.as_bytes());
    let (mut star, mut mark): (Option<&[u8]>, &[u8]) = (None, tx);
    while !tx.is_empty() {
        match px.first() {
            Some(b'*') => {
                star = Some(&px[1..]);
                px = &px[1..];
                mark = tx;
            }
            Some(b'?') | Some(_) if px.first() == Some(&tx[0]) || px.first() == Some(&b'?') => {
                px = &px[1..];
                tx = &tx[1..];
            }
            _ => {
                if let Some(saved) = star {
                    px = saved;
                    mark = &mark[1..];
                    tx = mark;
                    if mark.is_empty() && px.is_empty() {
                        return true;
                    }
                } else {
                    return false;
                }
            }
        }
    }
    while px.first() == Some(&b'*') {
        px = &px[1..];
    }
    px.is_empty()
}

/// Resolve a package `exports`/`imports` value for `subpath` under `conditions`.
///
/// Returns the target path (e.g. `./dist/index.js`) when matched.
pub fn resolve_exports(
    exports: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
) -> Option<String> {
    resolve_exports_inner(exports, subpath, conditions, true)
}

fn resolve_exports_inner(
    value: &serde_json::Value,
    subpath: &str,
    conditions: &[String],
    top: bool,
) -> Option<String> {
    match value {
        serde_json::Value::String(target) => {
            if !top || subpath == "." || subpath.is_empty() {
                Some(target.clone())
            } else {
                None
            }
        }
        serde_json::Value::Array(items) => {
            // First matching entry wins; `null` entries are skipped.
            items.iter().find_map(|item| {
                if item.is_null() {
                    None
                } else {
                    resolve_exports_inner(item, subpath, conditions, false)
                }
            })
        }
        serde_json::Value::Object(map) => {
            // `#`-prefixed keys are `imports` subpath keys; condition names
            // (`import`, `default`, ...) never start with `.` or `#`.
            let has_subpath_keys = map
                .keys()
                .any(|key| key.starts_with('.') || key.starts_with('#'));
            if top && has_subpath_keys {
                // Subpath map: exact match, then `*` patterns (longest first).
                if let Some(target) = map.get(subpath) {
                    if target.is_null() {
                        return None;
                    }
                    return resolve_exports_inner(target, subpath, conditions, false);
                }
                let mut patterns: Vec<&String> =
                    map.keys().filter(|key| key.contains('*')).collect();
                patterns.sort_by_key(|key| std::cmp::Reverse(key.len()));
                for pattern in patterns {
                    if let Some(captured) = match_pattern(pattern, subpath) {
                        let target = &map[pattern];
                        if target.is_null() {
                            return None;
                        }
                        return resolve_target_pattern(target, &captured, conditions);
                    }
                }
                None
            } else {
                // Conditional map: first matching condition wins.
                for condition in conditions
                    .iter()
                    .chain(std::iter::once(&"default".to_string()))
                {
                    if let Some(target) = map.get(condition) {
                        if target.is_null() {
                            return None;
                        }
                        if let Some(resolved) =
                            resolve_exports_inner(target, subpath, conditions, false)
                        {
                            return Some(resolved);
                        }
                    }
                }
                None
            }
        }
        _ => None,
    }
}

/// Match a `*` subpath pattern, returning the captured text.
fn match_pattern(pattern: &str, subpath: &str) -> Option<String> {
    let (prefix, suffix) = pattern.split_once('*')?;
    if subpath.starts_with(prefix) && subpath.ends_with(suffix) {
        let end = subpath.len() - suffix.len();
        // Prefix and suffix may overlap (e.g. `abc*abc` vs `abc`), which
        // would make the capture range `prefix.len()..end` inverted.
        if prefix.len() > end {
            return None;
        }
        Some(subpath[prefix.len()..end].to_string())
    } else {
        None
    }
}

/// Substitute `*` captures into a target value.
fn resolve_target_pattern(
    target: &serde_json::Value,
    captured: &str,
    conditions: &[String],
) -> Option<String> {
    match target {
        serde_json::Value::String(template) => Some(template.replace('*', captured)),
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| resolve_target_pattern(item, captured, conditions)),
        serde_json::Value::Object(_) => {
            // Conditional wrapping a pattern target.
            for condition in conditions
                .iter()
                .chain(std::iter::once(&"default".to_string()))
            {
                if let Some(nested) = target.get(condition) {
                    if let Some(resolved) = resolve_target_pattern(nested, captured, conditions) {
                        return Some(resolved);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// True when `path` is under `dir` (lexically).
fn is_under(path: &Path, dir: &Path) -> bool {
    path.strip_prefix(dir).is_ok()
}

/// Bare (unprefixed) Node builtins that must not be treated as packages.
fn is_bare_node_builtin(specifier: &str) -> bool {
    matches!(
        specifier,
        "assert"
            | "buffer"
            | "child_process"
            | "cluster"
            | "crypto"
            | "dgram"
            | "dns"
            | "events"
            | "fs"
            | "http"
            | "https"
            | "net"
            | "os"
            | "path"
            | "process"
            | "querystring"
            | "stream"
            | "string_decoder"
            | "timers"
            | "tls"
            | "tty"
            | "url"
            | "util"
            | "v8"
            | "vm"
            | "worker_threads"
            | "zlib"
    )
}

type SemverLikeKey = (u64, u64, u64, String);

fn semver_like_key(version: &str) -> SemverLikeKey {
    let mut numbers = version
        .split(['.', '-', '+'])
        .filter_map(|part| part.parse::<u64>().ok());
    (
        numbers.next().unwrap_or(0),
        numbers.next().unwrap_or(0),
        numbers.next().unwrap_or(0),
        version.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
