//! npm without Node.js (spec §16, §17, §81).
//!
//! Registry client, semver resolution, integrity-checked tarball installs
//! into `.ferrite/npm/packages/`, `ferrite.lock` lockfiles, and
//! `package.json` compatibility (read/write, never requires npm itself).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use ferrite_core::{FerriteError, Result};

/// Default registry.
pub const DEFAULT_REGISTRY: &str = "https://registry.npmjs.org";

/// Package runtime classification (§17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PackageRuntime {
    /// Browser-first ESM.
    BrowserEsm,
    /// Browser-compatible CJS (convertible).
    BrowserCjs,
    /// Universal (works in browser + SSR).
    Universal,
    /// Usable in SSR with shims.
    NodeCompatible,
    /// Requires real Node.js (native addons, `node:*` internals).
    NodeRequired,
    /// Build-time only (never shipped to the browser).
    BuildTimeOnly,
}

/// Classify a package from its manifest.
#[must_use]
pub fn classify_package(manifest: &PackageManifest) -> PackageRuntime {
    let has_browser = manifest.browser.is_some();
    let is_module = manifest.package_type.as_deref() == Some("module") || manifest.module.is_some();
    let has_exports = manifest.exports.is_some();
    if manifest.has_native_binding() {
        return PackageRuntime::NodeRequired;
    }
    if manifest.requires_node_core() {
        return PackageRuntime::NodeCompatible;
    }
    match (
        has_browser || is_module || has_exports,
        manifest.main.is_some(),
    ) {
        (true, true) => PackageRuntime::Universal,
        (true, false) => PackageRuntime::BrowserEsm,
        (false, true) => PackageRuntime::BrowserCjs,
        (false, false) => PackageRuntime::BuildTimeOnly,
    }
}

/// Installed/registry package manifest (superset of resolver's view).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct PackageManifest {
    /// Package name.
    pub name: Option<String>,
    /// Package version.
    pub version: Option<String>,
    /// CJS entry.
    pub main: Option<String>,
    /// ESM entry.
    pub module: Option<String>,
    /// Browser override.
    pub browser: Option<serde_json::Value>,
    /// Export map.
    pub exports: Option<serde_json::Value>,
    /// Package type (`module`/`commonjs`).
    #[serde(rename = "type")]
    pub package_type: Option<String>,
    /// Dependencies.
    pub dependencies: HashMap<String, String>,
    /// Peer dependencies.
    #[serde(rename = "peerDependencies")]
    pub peer_dependencies: HashMap<String, String>,
    /// Optional dependencies.
    #[serde(rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, String>,
    /// OS allowlist.
    pub os: Option<Vec<String>>,
    /// CPU allowlist.
    pub cpu: Option<Vec<String>>,
    /// Native binding hints (`binary`, `gypfile`).
    pub binary: Option<serde_json::Value>,
    /// `gypfile` presence indicates a native addon.
    pub gypfile: Option<String>,
}

impl PackageManifest {
    /// Heuristic: native `.node` binding required (§88).
    #[must_use]
    pub fn has_native_binding(&self) -> bool {
        self.gypfile.is_some() || self.binary.is_some()
    }

    /// Heuristic: depends on known server-only packages.
    #[must_use]
    pub fn requires_node_core(&self) -> bool {
        self.dependencies.keys().any(|name| {
            matches!(
                name.as_str(),
                "node-gyp" | "fsevents" | "sharp" | "esbuild" | "workerd"
            )
        })
    }
}

/// Registry version metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryVersion {
    /// Package name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Dependencies.
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    /// Peer dependencies.
    #[serde(default, rename = "peerDependencies")]
    pub peer_dependencies: HashMap<String, String>,
    /// Optional dependencies.
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: HashMap<String, String>,
    /// Distribution info.
    pub dist: RegistryDist,
    /// OS allowlist.
    pub os: Option<Vec<String>>,
    /// CPU allowlist.
    pub cpu: Option<Vec<String>>,
    /// Native binding metadata (`node-gyp` packages, §88).
    #[serde(default)]
    pub binary: Option<serde_json::Value>,
    /// `binding.gyp` presence flag (native addon, §88).
    #[serde(default)]
    pub gypfile: Option<bool>,
}

/// Registry dist info.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryDist {
    /// Tarball URL.
    pub tarball: String,
    /// Subresource integrity (`sha512-...`).
    pub integrity: Option<String>,
    /// Legacy shasum.
    pub shasum: Option<String>,
}

/// Registry packument (abbreviated).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegistryMetadata {
    /// Package name.
    pub name: String,
    /// Dist tags.
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: HashMap<String, String>,
    /// Versions.
    #[serde(default)]
    pub versions: BTreeMap<String, RegistryVersion>,
}

/// Resolve the best version for `range` (tag, exact, or semver range).
pub fn resolve_version(metadata: &RegistryMetadata, range: &str) -> Result<String> {
    // Dist-tag (latest, next, ...).
    if let Some(tagged) = metadata.dist_tags.get(range) {
        return Ok(tagged.clone());
    }
    // Exact version.
    if metadata.versions.contains_key(range) {
        return Ok(range.to_string());
    }
    let requirement = semver::VersionReq::parse(range)
        .map_err(|error| FerriteError::Npm(format!("invalid version range `{range}`: {error}")))?;
    let mut best: Option<semver::Version> = None;
    for version in metadata.versions.keys() {
        let Ok(parsed) = semver::Version::parse(version) else {
            continue;
        };
        // Stable ranges skip prereleases unless explicitly requested.
        if !parsed.pre.is_empty() && !range.contains('-') {
            continue;
        }
        if requirement.matches(&parsed) && best.as_ref().is_none_or(|current| parsed > *current) {
            best = Some(parsed);
        }
    }
    best.map(|version| version.to_string()).ok_or_else(|| {
        FerriteError::Npm(format!(
            "no version of `{}` satisfies `{range}`",
            metadata.name
        ))
    })
}

/// Verify `integrity` (`sha512-<base64>` / `sha1-<base64>`) for `bytes`.
pub fn verify_integrity(bytes: &[u8], integrity: Option<&str>) -> Result<()> {
    let Some(integrity) = integrity else {
        return Ok(());
    };
    let Some((algorithm, expected)) = integrity.split_once('-') else {
        return Err(FerriteError::Npm(format!(
            "bad integrity value `{integrity}`"
        )));
    };
    use base64::Engine as _;
    let expected = base64::engine::general_purpose::STANDARD
        .decode(expected.trim())
        .map_err(|error| FerriteError::Npm(format!("bad integrity base64: {error}")))?;
    let actual: Vec<u8> = match algorithm {
        "sha512" => {
            use sha2::Digest as _;
            sha2::Sha512::digest(bytes).to_vec()
        }
        "sha384" => {
            use sha2::Digest as _;
            sha2::Sha384::digest(bytes).to_vec()
        }
        "sha256" => {
            use sha2::Digest as _;
            sha2::Sha256::digest(bytes).to_vec()
        }
        other => {
            return Err(FerriteError::Npm(format!(
                "unsupported integrity algorithm `{other}`"
            )));
        }
    };
    if actual != expected {
        return Err(FerriteError::Npm("tarball integrity mismatch".to_string()));
    }
    Ok(())
}

/// Extract a gzip tarball into `dest`, stripping the `package/` prefix.
///
/// Parent directories are created explicitly: real-world tarballs (e.g.
/// `left-pad`) often omit directory entries, and `Entry::unpack` does not
/// create missing parents.
pub fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    std::fs::create_dir_all(dest)?;
    for entry in archive
        .entries()
        .map_err(|error| FerriteError::Npm(error.to_string()))?
    {
        let mut entry = entry.map_err(|error| FerriteError::Npm(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| FerriteError::Npm(error.to_string()))?
            .into_owned();
        let relative = path.strip_prefix("package").unwrap_or(&path);
        if relative.as_os_str().is_empty() {
            continue;
        }
        // Refuse path traversal outside `dest`.
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(FerriteError::Npm(format!(
                "tarball entry escapes package dir: {}",
                relative.display()
            )));
        }
        let target = dest.join(relative);
        if entry.header().entry_type().is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&target)
            .map_err(|error| FerriteError::Npm(format!("unpack {}: {error}", target.display())))?;
    }
    Ok(())
}

/// A locked package (§81).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    /// Package name.
    pub name: String,
    /// Locked version.
    pub version: String,
    /// Source (`npm`).
    pub source: String,
    /// Integrity hash.
    pub integrity: Option<String>,
    /// Locked dependencies (`name` → `version`).
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

/// `ferrite.lock` (§81; npm packages only — `Cargo.lock` owns Rust).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Lockfile {
    /// Locked packages.
    #[serde(default)]
    pub package: Vec<LockedPackage>,
}

impl Lockfile {
    /// Read a lockfile (missing file → empty).
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        toml::from_str(&text).map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))
    }

    /// Write a lockfile (sorted for stable diffs).
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut sorted = self.clone();
        sorted
            .package
            .sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
        let text = toml::to_string_pretty(&sorted)
            .map_err(|error| FerriteError::Npm(format!("lockfile: {error}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Find a locked package.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&LockedPackage> {
        self.package.iter().find(|package| package.name == name)
    }

    /// Insert or replace a locked package.
    pub fn upsert(&mut self, package: LockedPackage) {
        if let Some(existing) = self.package.iter_mut().find(|p| p.name == package.name) {
            *existing = package;
        } else {
            self.package.push(package);
        }
    }

    /// Remove a locked package. Returns true when removed.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.package.len();
        self.package.retain(|package| package.name != name);
        self.package.len() != before
    }
}

/// npm registry HTTP client with on-disk metadata caching.
#[derive(Debug, Clone)]
pub struct RegistryClient {
    /// HTTP client.
    client: reqwest::Client,
    /// Registry base URL.
    pub registry: String,
    /// Metadata cache directory.
    pub cache_dir: PathBuf,
}

impl RegistryClient {
    /// Create a client.
    pub fn new(registry: impl Into<String>, cache_dir: PathBuf) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(format!("ferrite/{}", ferrite_core::VERSION))
            .build()
            .map_err(|error| FerriteError::Npm(error.to_string()))?;
        Ok(Self {
            client,
            registry: registry.into().trim_end_matches('/').to_string(),
            cache_dir,
        })
    }

    /// Fetch (and cache) package metadata.
    pub async fn metadata(&self, name: &str) -> Result<RegistryMetadata> {
        let cache_path = self
            .cache_dir
            .join(format!("{}.json", name.replace('/', "__")));
        if let Ok(text) = std::fs::read_to_string(&cache_path) {
            if let Ok(cached) = serde_json::from_str(&text) {
                return Ok(cached);
            }
        }
        let url = format!("{}/{name}", self.registry);
        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.npm.install-v1+json")
            .send()
            .await
            .map_err(|error| FerriteError::Npm(format!("registry request failed: {error}")))?;
        if !response.status().is_success() {
            return Err(FerriteError::Npm(format!(
                "registry returned {} for `{name}`",
                response.status()
            )));
        }
        let metadata: RegistryMetadata = response
            .json()
            .await
            .map_err(|error| FerriteError::Npm(format!("bad registry JSON: {error}")))?;
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(
            &cache_path,
            serde_json::to_string(&metadata).unwrap_or_default(),
        );
        Ok(metadata)
    }

    /// Download a tarball.
    pub async fn tarball(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| FerriteError::Npm(format!("tarball download failed: {error}")))?
            .bytes()
            .await
            .map_err(|error| FerriteError::Npm(format!("tarball read failed: {error}")))?;
        Ok(bytes.to_vec())
    }
}

/// Installer: resolves, downloads, verifies, and extracts packages.
pub struct Installer {
    /// Registry client.
    pub client: RegistryClient,
    /// Install root (`.ferrite/npm/packages`).
    pub store: PathBuf,
    /// Tarball cache.
    pub tarball_cache: PathBuf,
}

impl Installer {
    /// Create an installer rooted at `project_root/.ferrite/npm`.
    pub fn new(client: RegistryClient, npm_root: PathBuf) -> Self {
        Self {
            client,
            store: npm_root.join("packages"),
            tarball_cache: npm_root.join("tarballs"),
        }
    }

    /// Install `name@range` and its dependencies, updating `lockfile`.
    /// Returns the locked version.
    pub async fn install(
        &self,
        name: &str,
        range: &str,
        lockfile: &mut Lockfile,
        optional_ok: bool,
    ) -> Result<String> {
        let mut visiting = Vec::new();
        self.install_inner(
            name.to_string(),
            range.to_string(),
            lockfile,
            &mut visiting,
            optional_ok,
        )
        .await
    }

    /// Recursive installer (boxed: async recursion).
    #[allow(clippy::too_many_lines)]
    fn install_inner<'a>(
        &'a self,
        name: String,
        range: String,
        lockfile: &'a mut Lockfile,
        visiting: &'a mut Vec<String>,
        optional_ok: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            if visiting.contains(&name) {
                return Ok(range); // cycle: keep the requested range
            }
            // Reuse the lock when it still satisfies the range.
            if let Some(locked) = lockfile.find(&name) {
                if range_satisfied(&locked.version, &range)
                    && self.installed_dir(&name, &locked.version).exists()
                {
                    return Ok(locked.version.clone());
                }
            }
            visiting.push(name.clone());
            let metadata = self.client.metadata(&name).await?;
            let version = resolve_version(&metadata, &range)?;
            let info = metadata.versions.get(&version).ok_or_else(|| {
                FerriteError::Npm(format!("version `{version}` of `{name}` disappeared"))
            })?;
            // Platform filter (§16 OS/CPU filtering): skip mismatched optional deps.
            if !platform_matches(&info.os, &info.cpu) {
                visiting.pop();
                if optional_ok {
                    return Ok(version);
                }
                return Err(FerriteError::Npm(format!(
                    "`{name}@{version}` does not support this platform"
                )));
            }
            // Native addons cannot run in the embedded runtime (§88).
            if info.binary.is_some() || info.gypfile == Some(true) {
                tracing::warn!(
                    "native Node addon detected: `{name}@{version}` ships `.node` binaries; \
                     use a Rust-native replacement, a WASM alternative, or the Node \
                     compatibility host"
                );
            }
            self.fetch_extract(&name, &version, &info.dist).await?;
            let dependencies: Vec<(String, String)> = info
                .dependencies
                .iter()
                .map(|(dep, dep_range)| (dep.clone(), dep_range.clone()))
                .collect();
            let mut locked_deps = BTreeMap::new();
            for (dep, dep_range) in dependencies {
                match self
                    .install_inner(
                        dep.clone(),
                        dep_range,
                        &mut *lockfile,
                        &mut *visiting,
                        false,
                    )
                    .await
                {
                    Ok(locked) => {
                        locked_deps.insert(dep, locked);
                    }
                    Err(error) => {
                        visiting.pop();
                        return Err(error);
                    }
                }
            }
            // Optional deps: best effort, never fatal.
            let optional: Vec<(String, String)> = info
                .optional_dependencies
                .iter()
                .map(|(dep, dep_range)| (dep.clone(), dep_range.clone()))
                .collect();
            for (dep, dep_range) in optional {
                if let Ok(locked) = self
                    .install_inner(dep.clone(), dep_range, &mut *lockfile, &mut *visiting, true)
                    .await
                {
                    locked_deps.insert(dep, locked);
                }
            }
            // Peer deps: warn when missing (recording them as locked
            // dependencies would be wrong, so only warn here).
            for (peer, peer_range) in &info.peer_dependencies {
                if lockfile.find(peer).is_none() {
                    tracing::warn!(
                        "`{name}@{version}` wants peer `{peer}@{peer_range}` (not installed)"
                    );
                }
            }
            lockfile.upsert(LockedPackage {
                name: name.clone(),
                version: version.clone(),
                source: "npm".to_string(),
                integrity: info.dist.integrity.clone(),
                dependencies: locked_deps,
            });
            visiting.pop();
            Ok(version)
        })
    }

    /// Download (cached), verify, and extract one tarball.
    async fn fetch_extract(&self, name: &str, version: &str, dist: &RegistryDist) -> Result<()> {
        let dest = self.installed_dir(name, version);
        if dest.join("package.json").exists() {
            return Ok(());
        }
        let cached = self
            .tarball_cache
            .join(format!("{name}-{version}.tgz").replace('/', "__"));
        let bytes = if cached.exists() {
            std::fs::read(&cached)?
        } else {
            let bytes = self.client.tarball(&dist.tarball).await?;
            verify_integrity(&bytes, dist.integrity.as_deref())?;
            if let Some(parent) = cached.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&cached, &bytes);
            bytes
        };
        verify_integrity(&bytes, dist.integrity.as_deref())?;
        let _ = std::fs::remove_dir_all(&dest);
        extract_tarball(&bytes, &dest)?;
        Ok(())
    }

    /// Installed directory for `name@version`.
    #[must_use]
    pub fn installed_dir(&self, name: &str, version: &str) -> PathBuf {
        self.store.join(format!("{name}@{version}"))
    }
}

/// True when `version` satisfies `range` (or range is a tag/unknown).
fn range_satisfied(version: &str, range: &str) -> bool {
    if version == range {
        return true;
    }
    let (Ok(requirement), Ok(parsed)) = (
        semver::VersionReq::parse(range),
        semver::Version::parse(version),
    ) else {
        return true; // tags: assume satisfied (re-resolve on `ferrite update`)
    };
    requirement.matches(&parsed)
}

/// True when the current platform matches `os`/`cpu` allowlists.
fn platform_matches(os: &Option<Vec<String>>, cpu: &Option<Vec<String>>) -> bool {
    if let Some(list) = os {
        let current = std::env::consts::OS;
        if !list.iter().any(|entry| entry == current || entry == "any") {
            return false;
        }
    }
    if let Some(list) = cpu {
        let current = std::env::consts::ARCH;
        if !list.iter().any(|entry| entry == current || entry == "any") {
            return false;
        }
    }
    true
}

/// Parse `spec` (`name`, `name@range`, `@scope/name@range`) into parts.
#[must_use]
pub fn parse_spec(spec: &str) -> (String, String) {
    if let Some(rest) = spec.strip_prefix('@') {
        // Scoped: split at the *second* `@`.
        match rest.find('@') {
            Some(pos) => {
                let name = format!("@{}", &rest[..pos]);
                let range = rest[pos + 1..].to_string();
                (
                    name,
                    if range.is_empty() {
                        "latest".to_string()
                    } else {
                        range
                    },
                )
            }
            None => (spec.to_string(), "latest".to_string()),
        }
    } else {
        match spec.split_once('@') {
            Some((name, range)) => (
                name.to_string(),
                if range.is_empty() {
                    "latest".to_string()
                } else {
                    range.to_string()
                },
            ),
            None => (spec.to_string(), "latest".to_string()),
        }
    }
}

/// Minimal `package.json` dependency view (compatibility).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct JsPackageJson {
    /// Dependencies.
    pub dependencies: HashMap<String, String>,
    /// Dev dependencies.
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: HashMap<String, String>,
    /// Rest of the file, preserved verbatim.
    #[serde(flatten)]
    pub rest: HashMap<String, serde_json::Value>,
}

impl JsPackageJson {
    /// Read (missing → empty).
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(FerriteError::Json)
    }

    /// Write back, preserving unknown fields.
    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).map_err(FerriteError::Json)?;
        std::fs::write(path, format!("{text}\n"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata() -> RegistryMetadata {
        RegistryMetadata {
            name: "pkg".to_string(),
            dist_tags: HashMap::from([("latest".to_string(), "2.0.0".to_string())]),
            versions: ["1.0.0", "1.2.3", "2.0.0"]
                .into_iter()
                .map(|version| {
                    (
                        version.to_string(),
                        RegistryVersion {
                            name: "pkg".to_string(),
                            version: version.to_string(),
                            dependencies: HashMap::new(),
                            peer_dependencies: HashMap::new(),
                            optional_dependencies: HashMap::new(),
                            dist: RegistryDist {
                                tarball: String::new(),
                                integrity: None,
                                shasum: None,
                            },
                            os: None,
                            cpu: None,
                            binary: None,
                            gypfile: None,
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn resolves_tags_and_ranges() {
        let meta = metadata();
        assert_eq!(resolve_version(&meta, "latest").unwrap(), "2.0.0");
        assert_eq!(resolve_version(&meta, "^1.0.0").unwrap(), "1.2.3");
        assert_eq!(resolve_version(&meta, "1.0.0").unwrap(), "1.0.0");
        assert!(resolve_version(&meta, "^3.0.0").is_err());
    }

    #[test]
    fn parses_specs() {
        assert_eq!(
            parse_spec("react"),
            ("react".to_string(), "latest".to_string())
        );
        assert_eq!(
            parse_spec("three@^0.180.0"),
            ("three".to_string(), "^0.180.0".to_string())
        );
        assert_eq!(
            parse_spec("@scope/name@1.2.3"),
            ("@scope/name".to_string(), "1.2.3".to_string())
        );
    }

    #[test]
    fn lockfile_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ferrite-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut lock = Lockfile::default();
        lock.upsert(LockedPackage {
            name: "react".to_string(),
            version: "19.1.0".to_string(),
            source: "npm".to_string(),
            integrity: Some("sha512-abc".to_string()),
            dependencies: BTreeMap::new(),
        });
        lock.write(&dir.join("ferrite.lock")).unwrap();
        let loaded = Lockfile::read(&dir.join("ferrite.lock")).unwrap();
        assert_eq!(loaded.find("react").unwrap().version, "19.1.0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn integrity_checks() {
        use sha2::Digest as _;
        let bytes = b"hello";
        let digest = sha2::Sha512::digest(bytes);
        use base64::Engine as _;
        let integrity = format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(digest)
        );
        assert!(verify_integrity(bytes, Some(&integrity)).is_ok());
        assert!(verify_integrity(b"other", Some(&integrity)).is_err());
    }

    #[test]
    fn extracts_tarball_without_dir_entries() {
        // Regression: real tarballs (left-pad) omit `perf/` dir entries.
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            let contents = b"console.log(1);";
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, "package/perf/deep.js", &contents[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let mut gz = Vec::new();
        {
            use std::io::Write as _;
            let mut encoder = flate2::write::GzEncoder::new(&mut gz, flate2::Compression::fast());
            encoder.write_all(&tar_bytes).unwrap();
            encoder.finish().unwrap();
        }
        let dir = std::env::temp_dir().join(format!("ferrite-tar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        extract_tarball(&gz, &dir).unwrap();
        assert!(dir.join("perf/deep.js").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn classifies_packages() {
        let esm = PackageManifest {
            package_type: Some("module".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_package(&esm), PackageRuntime::BrowserEsm);
        let native = PackageManifest {
            gypfile: Some("binding.gyp".to_string()),
            ..Default::default()
        };
        assert_eq!(classify_package(&native), PackageRuntime::NodeRequired);
    }
}
