//! Ferrite core types shared by every subsystem.
//!
//! This crate defines the stability boundary from spec §92: config, module
//! identities, environments, diagnostics, and hashing. Compiler-specific AST
//! types must never leak through this API.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Crate/framework version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Framework-wide result type.
pub type Result<T> = std::result::Result<T, FerriteError>;

/// Framework-wide error model (spec §64).
#[derive(Debug, thiserror::Error)]
pub enum FerriteError {
    /// Module resolution failure (`FERRITE_RESOLVE_*`).
    #[error("resolve error: {0}")]
    Resolve(String),
    /// Parse failure (`FERRITE_PARSE_*`).
    #[error("parse error in {id}: {message}")]
    Parse {
        /// Module id being parsed.
        id: String,
        /// Human-readable message.
        message: String,
        /// Optional code frame.
        frame: Option<String>,
    },
    /// Transform failure (`FERRITE_TRANSFORM_*`).
    #[error("transform error in {id}: {message}")]
    Transform {
        /// Module id being transformed.
        id: String,
        /// Human-readable message.
        message: String,
    },
    /// Plugin failure (`FERRITE_PLUGIN_*`).
    #[error("plugin `{plugin}` failed in hook `{hook}`: {message}")]
    Plugin {
        /// Plugin name.
        plugin: String,
        /// Hook name.
        hook: String,
        /// Human-readable message.
        message: String,
    },
    /// JS runtime failure (`FERRITE_RUNTIME_*`).
    #[error("runtime error: {0}")]
    Runtime(String),
    /// Build failure (`FERRITE_BUILD_*`).
    #[error("build error: {0}")]
    Build(String),
    /// Configuration failure (`FERRITE_CONFIG_*`).
    #[error("config error: {0}")]
    Config(String),
    /// npm subsystem failure (`FERRITE_NPM_*`).
    #[error("npm error: {0}")]
    Npm(String),
    /// SSR failure (`FERRITE_SSR_*`).
    #[error("ssr error: {0}")]
    Ssr(String),
    /// HMR failure (`FERRITE_HMR_*`).
    #[error("hmr error: {0}")]
    Hmr(String),
    /// Cache failure (`FERRITE_CACHE_*`).
    #[error("cache error: {0}")]
    Cache(String),
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Generic failure.
    #[error("{0}")]
    Other(String),
}

impl FerriteError {
    /// Machine-readable error code for diagnostics.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Resolve(_) => "FERRITE_RESOLVE_001",
            Self::Parse { .. } => "FERRITE_PARSE_001",
            Self::Transform { .. } => "FERRITE_TRANSFORM_001",
            Self::Plugin { .. } => "FERRITE_PLUGIN_001",
            Self::Runtime(_) => "FERRITE_RUNTIME_001",
            Self::Build(_) => "FERRITE_BUILD_001",
            Self::Config(_) => "FERRITE_CONFIG_001",
            Self::Npm(_) => "FERRITE_NPM_001",
            Self::Ssr(_) => "FERRITE_SSR_001",
            Self::Hmr(_) => "FERRITE_HMR_001",
            Self::Cache(_) => "FERRITE_CACHE_001",
            Self::Io(_) => "FERRITE_IO_001",
            Self::Json(_) => "FERRITE_JSON_001",
            Self::Other(_) => "FERRITE_001",
        }
    }

    /// Convert into a serializable diagnostic for the error overlay (§35).
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        let (message, id, frame) = match self {
            Self::Parse { id, message, frame } => {
                (message.clone(), Some(id.clone()), frame.clone())
            }
            Self::Transform { id, message } => (message.clone(), Some(id.clone()), None),
            other => (other.to_string(), None, None),
        };
        Diagnostic {
            message,
            stack: format!("{self:?}"),
            id,
            frame,
            code: self.code().to_string(),
            plugin: match self {
                Self::Plugin { plugin, .. } => Some(plugin.clone()),
                _ => None,
            },
        }
    }
}

/// Serializable diagnostic payload (§35, `type: "error"`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    /// Human-readable message.
    pub message: String,
    /// Debug stack / cause chain.
    pub stack: String,
    /// Module id, when applicable.
    pub id: Option<String>,
    /// Code frame, when applicable.
    pub frame: Option<String>,
    /// Machine-readable code.
    pub code: String,
    /// Plugin name, when applicable.
    pub plugin: Option<String>,
}

/// Build a `line/column` code frame for `source` at byte `offset`.
#[must_use]
pub fn code_frame(source: &str, offset: usize, context_lines: usize) -> String {
    let offset = offset.min(source.len());
    let line_start = source[..offset].matches('\n').count();
    let lines: Vec<&str> = source.lines().collect();
    let lo = line_start.saturating_sub(context_lines);
    let hi = (line_start + context_lines + 1).min(lines.len().max(1));
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate().take(hi).skip(lo) {
        let marker = if i == line_start { ">" } else { " " };
        out.push_str(&format!("{} {:>4} │ {}\n", marker, i + 1, line));
        if i == line_start {
            let col = offset - source[..offset].rfind('\n').map_or(0, |p| p + 1);
            out.push_str(&format!("  {:>4} │ {:>width$}^\n", "", "", width = col));
        }
    }
    out
}

/// Unique module identity inside the graph.
///
/// For files this is the `/`-rooted URL (e.g. `/src/main.ts`); for virtual
/// modules it is the `\0`-prefixed id (e.g. `\0virtual:ferrite/env`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ModuleId(pub String);

impl ModuleId {
    /// Create a module id.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// True for `\0`-prefixed virtual modules (§14).
    #[must_use]
    pub fn is_virtual(&self) -> bool {
        self.0.starts_with('\0')
    }

    /// True for `/@npm/`-served dependency modules.
    #[must_use]
    pub fn is_npm(&self) -> bool {
        self.0.starts_with("/@npm/")
    }

    /// Split off the `?query` suffix, if any.
    #[must_use]
    pub fn split_query(&self) -> (&str, Option<&str>) {
        match self.0.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (self.0.as_str(), None),
        }
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Native module types (§6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModuleType {
    /// Plain JavaScript (`.js`, `.mjs`, `.cjs`).
    Js,
    /// JSX (`.jsx`).
    Jsx,
    /// TypeScript (`.ts`, `.mts`, `.cts`).
    Ts,
    /// TSX (`.tsx`).
    Tsx,
    /// JSON (`.json`).
    Json,
    /// CSS (`.css`, `.module.css`).
    Css,
    /// HTML (`.html`).
    Html,
    /// WebAssembly (`.wasm`).
    Wasm,
    /// Binary/static asset.
    Asset,
    /// Raw text (`?raw`).
    Text,
    /// Data URL (`?inline`, small assets).
    Data,
    /// Plugin-registered module type.
    Custom(String),
}

impl ModuleType {
    /// Infer the module type from a file path or module id.
    #[must_use]
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_string_lossy();
        let path = path.split('?').next().unwrap_or(&path);
        if path.ends_with(".module.css") || path.ends_with(".css") {
            Self::Css
        } else if path.ends_with(".tsx") {
            Self::Tsx
        } else if path.ends_with(".ts") || path.ends_with(".mts") || path.ends_with(".cts") {
            Self::Ts
        } else if path.ends_with(".jsx") {
            Self::Jsx
        } else if path.ends_with(".js") || path.ends_with(".mjs") || path.ends_with(".cjs") {
            Self::Js
        } else if path.ends_with(".json") {
            Self::Json
        } else if path.ends_with(".html") || path.ends_with(".htm") {
            Self::Html
        } else if path.ends_with(".wasm") {
            Self::Wasm
        } else if path.ends_with(".txt") || path.ends_with(".md") {
            Self::Text
        } else {
            Self::Asset
        }
    }

    /// True for JS-like modules handled by the JS compiler.
    #[must_use]
    pub fn is_js_like(&self) -> bool {
        matches!(self, Self::Js | Self::Jsx | Self::Ts | Self::Tsx)
    }
}

/// Compilation target.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Target {
    /// `es2015`.
    Es2015,
    /// `es2020`.
    Es2020,
    /// `es2022` (default).
    #[default]
    Es2022,
    /// `esnext`.
    EsNext,
    /// Custom target string.
    Custom(String),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Es2015 => f.write_str("es2015"),
            Self::Es2020 => f.write_str("es2020"),
            Self::Es2022 => f.write_str("es2022"),
            Self::EsNext => f.write_str("esnext"),
            Self::Custom(value) => f.write_str(value),
        }
    }
}

impl std::str::FromStr for Target {
    type Err = FerriteError;

    fn from_str(value: &str) -> Result<Self> {
        Ok(match value.to_ascii_lowercase().as_str() {
            "es2015" => Self::Es2015,
            "es2020" => Self::Es2020,
            "es2022" => Self::Es2022,
            "esnext" => Self::EsNext,
            other => Self::Custom(other.to_string()),
        })
    }
}

/// Build/dev environment kinds (§36).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentKind {
    /// Browser client bundle.
    #[default]
    Client,
    /// Server-side rendering.
    Ssr,
    /// Web worker.
    Worker,
    /// Test runner.
    Test,
    /// Plugin-defined environment.
    Custom(String),
}

impl EnvironmentKind {
    /// True for SSR-like environments.
    #[must_use]
    pub fn is_ssr(&self) -> bool {
        matches!(self, Self::Ssr)
    }

    /// Environment name used in the graph/cache keys.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Client => "client".to_string(),
            Self::Ssr => "ssr".to_string(),
            Self::Worker => "worker".to_string(),
            Self::Test => "test".to_string(),
            Self::Custom(name) => name.clone(),
        }
    }

    /// Default package-export conditions (§73).
    #[must_use]
    pub fn default_conditions(&self) -> Vec<String> {
        match self {
            Self::Client => ["browser", "development", "import", "module", "default"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            Self::Ssr => ["development", "import", "node-compatible", "default"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            Self::Worker => ["worker", "import", "browser", "default"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            Self::Test => ["development", "import", "default"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            Self::Custom(_) => ["import", "default"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }
}

impl fmt::Display for EnvironmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// An explicit build environment (§36).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Environment {
    /// Environment name (`client`, `ssr`, ...).
    pub name: String,
    /// Environment kind.
    pub kind: EnvironmentKind,
    /// Compilation target.
    pub target: Target,
    /// Package-export conditions.
    pub conditions: Vec<String>,
    /// Compile-time defines.
    pub define: std::collections::HashMap<String, String>,
}

impl Environment {
    /// Create an environment with default conditions for `kind`.
    #[must_use]
    pub fn new(name: impl Into<String>, kind: EnvironmentKind) -> Self {
        let conditions = kind.default_conditions();
        Self {
            name: name.into(),
            kind,
            target: Target::default(),
            conditions,
            define: std::collections::HashMap::new(),
        }
    }
}

/// Content hash (blake3 hex).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Hash(pub String);

impl Hash {
    /// Hash bytes with blake3.
    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(blake3::hash(bytes).to_hex().to_string())
    }

    /// Hash a string.
    #[must_use]
    pub fn of_str(value: &str) -> Self {
        Self::of_bytes(value.as_bytes())
    }

    /// Short (8-char) fingerprint for file names.
    #[must_use]
    pub fn short(&self, len: usize) -> String {
        self.0.chars().take(len).collect()
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque source map payload (§41).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SourceMap {
    /// Raw source-map JSON (or data URL for inline maps).
    pub mappings: String,
    /// True when the map is inlined in the emitted code.
    pub inline: bool,
}

impl SourceMap {
    /// Create an external source map from JSON.
    #[must_use]
    pub fn external(mappings: impl Into<String>) -> Self {
        Self {
            mappings: mappings.into(),
            inline: false,
        }
    }

    /// Render an inline `sourceMappingURL` comment.
    #[must_use]
    pub fn inline_comment_json(json: &str) -> String {
        use std::fmt::Write as _;
        // Base64 without extra deps: reuse a tiny encoder.
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bytes = json.as_bytes();
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let mut n: u32 = 0;
            for (i, byte) in chunk.iter().enumerate() {
                n |= (*byte as u32) << (16 - 8 * i);
            }
            let chars = [ALPHABET[(n >> 18 & 63) as usize] as char];
            let _ = write!(out, "{}", chars[0]);
            let _ = write!(out, "{}", ALPHABET[(n >> 12 & 63) as usize] as char);
            if chunk.len() > 1 {
                let _ = write!(out, "{}", ALPHABET[(n >> 6 & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                let _ = write!(out, "{}", ALPHABET[(n & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        format!("\n//# sourceMappingURL=data:application/json;base64,{out}")
    }
}

/// Canonicalize a root-relative URL path (resolve `.`/`..`, keep leading `/`).
#[must_use]
pub fn normalize_url_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

/// Join an importer URL with a relative specifier.
#[must_use]
pub fn join_url(importer: &str, specifier: &str) -> String {
    if specifier.starts_with('/') {
        return normalize_url_path(specifier);
    }
    let base = importer.rsplit_once('/').map_or("/", |(dir, _)| dir);
    normalize_url_path(&format!("{base}/{specifier}"))
}

/// Read a file relative to `root`, returning its UTF-8 contents.
pub fn read_to_string(root: &Path, file: &Path) -> Result<String> {
    let path = if file.is_absolute() {
        file.to_path_buf()
    } else {
        root.join(file)
    };
    std::fs::read_to_string(&path)
        .map_err(|source| FerriteError::Other(format!("cannot read {}: {source}", path.display())))
}

/// Convert a file path to a project URL or an external `/@fs` identity.
#[must_use]
pub fn file_to_url(root: &Path, file: &Path) -> String {
    let root = PathBuf::from(normalize_path(root));
    let file = PathBuf::from(normalize_path(file));
    if file.is_absolute() && !file.starts_with(&root) {
        return normalize_url_path(&format!(
            "/@fs{}",
            file.to_string_lossy().replace('\\', "/")
        ));
    }
    let relative = file.strip_prefix(&root).unwrap_or(&file);
    let mut url = String::from("/");
    url.push_str(&relative.to_string_lossy().replace('\\', "/"));
    normalize_url_path(&url)
}

/// Convert a project URL or external `/@fs` identity to a file path.
#[must_use]
pub fn url_to_file(root: &Path, url: &str) -> PathBuf {
    if let Some(file) = url.strip_prefix("/@fs") {
        if Path::new(file).is_absolute() {
            return PathBuf::from(file);
        }
    }
    let trimmed = url.trim_start_matches('/');
    root.join(trimmed)
}

/// Normalize a path the way Vite's `normalizePath` does: forward slashes,
/// lexically resolved `.`/`..`, no trailing slash (except the root itself).
#[must_use]
pub fn normalize_path(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let (prefix, rest) = match text.split_once("://") {
        Some((scheme, _)) => {
            let end = scheme.len() + 3;
            text.split_at(end)
        }
        None => ("", text.as_str()),
    };
    let absolute = rest.starts_with('/');
    // Preserve a Windows drive prefix (`C:/...`) or UNC root (`//host/...`).
    let (drive, rest) = match rest.split_once('/') {
        Some((head, tail)) if head.len() == 2 && head.ends_with(':') => (format!("{head}/"), tail),
        _ if rest.starts_with("//") => (String::from("//"), &rest[2..]),
        _ => (String::new(), rest),
    };
    let mut parts: Vec<&str> = Vec::new();
    for segment in rest.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() && !absolute && drive.is_empty() {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    let mut out = String::from(prefix);
    out.push_str(&drive);
    if absolute && drive.is_empty() {
        out.push('/');
    }
    out.push_str(&parts.join("/"));
    if out.is_empty() {
        return String::from(".");
    }
    out
}

/// Walk up from `start` looking for a workspace root, like Vite's
/// `searchForWorkspaceRoot`: the nearest ancestor (or self) containing
/// `pnpm-workspace.yaml`, `lerna.json`, a `.git` entry, or a `package.json`
/// with a `workspaces` field. Falls back to `start` itself.
#[must_use]
pub fn search_for_workspace_root(start: &Path) -> PathBuf {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(start)
    };
    if current.is_file() {
        current.pop();
    }
    loop {
        if current.join("pnpm-workspace.yaml").exists()
            || current.join("pnpm-workspace.yml").exists()
            || current.join("lerna.json").exists()
            || current.join(".git").exists()
            || has_package_workspaces(&current.join("package.json"))
        {
            return current;
        }
        if !current.pop() {
            return start.to_path_buf();
        }
    }
}

/// True when `package.json` exists and declares a `workspaces` field.
fn has_package_workspaces(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    // A full JSON parse is overkill for one probe; a quoted-key scan
    // tolerates comments/trailing commas in lenient manifests.
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    while let Some(char) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if char == '\\' {
                escaped = true;
            } else if char == '"' {
                in_string = false;
                if current == "workspaces" {
                    // The key must be followed by a colon.
                    for next in chars.by_ref() {
                        if next.is_whitespace() {
                            continue;
                        }
                        return next == ':';
                    }
                    return false;
                }
                current.clear();
            } else {
                current.push(char);
            }
        } else if char == '"' {
            in_string = true;
            current.clear();
        }
    }
    false
}

/// Monotonic instant helper (re-exported so all crates share one clock type).
#[must_use]
pub fn now() -> Instant {
    Instant::now()
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_files_have_distinct_urls_and_round_trip() {
        let project = std::env::temp_dir().join("ferrite-core-project");
        let outside = std::env::temp_dir().join("ferrite-core-external");
        let file = outside.join("input.js");
        let url = file_to_url(&project, &file);
        assert!(url.starts_with("/@fs"), "{url}");
        assert_eq!(url_to_file(&project, &url), file);
        assert_eq!(
            file_to_url(&project, &project.join("../ferrite-core-external/input.js")),
            url
        );
        assert_eq!(
            file_to_url(&project, &project.join("src/app.js")),
            "/src/app.js"
        );
        assert_eq!(
            file_to_url(&project, Path::new("src/app.js")),
            "/src/app.js"
        );
        assert_eq!(
            url_to_file(&project, "/src/app.js"),
            project.join("src/app.js")
        );
    }

    use super::*;

    #[test]
    fn module_type_from_path() {
        assert_eq!(ModuleType::from_path("a.ts"), ModuleType::Ts);
        assert_eq!(ModuleType::from_path("a.tsx"), ModuleType::Tsx);
        assert_eq!(ModuleType::from_path("a.module.css"), ModuleType::Css);
        assert_eq!(ModuleType::from_path("a.mjs"), ModuleType::Js);
        assert_eq!(ModuleType::from_path("a.wasm"), ModuleType::Wasm);
        assert_eq!(ModuleType::from_path("a.json"), ModuleType::Json);
    }

    #[test]
    fn url_joining() {
        assert_eq!(join_url("/src/a.ts", "./b.ts"), "/src/b.ts");
        assert_eq!(join_url("/src/a.ts", "../b.ts"), "/b.ts");
        assert_eq!(join_url("/src/a.ts", "/abs.ts"), "/abs.ts");
    }

    #[test]
    fn code_frame_marks_line() {
        let frame = code_frame("a\nb\nc", 2, 1);
        assert!(frame.contains('>'));
        assert!(frame.contains('^'));
    }

    #[test]
    fn normalize_path_shapes() {
        assert_eq!(normalize_path(Path::new("/a/b/../c/./d/")), "/a/c/d");
        assert_eq!(normalize_path(Path::new("a/./b")), "a/b");
        assert_eq!(normalize_path(Path::new("../a")), "../a");
        assert_eq!(normalize_path(Path::new("/../a")), "/a");
        assert_eq!(normalize_path(Path::new("")), ".");
        assert_eq!(normalize_path(Path::new("C:\\a\\b\\..\\c")), "C:/a/c");
        assert_eq!(normalize_path(Path::new("file:///a/./b")), "file:///a/b");
    }

    #[test]
    fn workspace_root_search() {
        let root = std::env::temp_dir().join(format!("ferrite-wsroot-{}", std::process::id()));
        let nested = root.join("packages/app/src");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&nested).unwrap();
        // Fallback: no markers anywhere up to / (or a real ancestor root).
        // With markers present, the nearest one wins.
        std::fs::write(
            root.join("pnpm-workspace.yaml"),
            "packages:\n  - packages/*\n",
        )
        .unwrap();
        assert_eq!(search_for_workspace_root(&nested), root);
        assert_eq!(search_for_workspace_root(&nested.join("index.ts")), root);
        std::fs::write(
            root.join("packages/app/package.json"),
            "{\"name\": \"app\", \"workspaces\": [\"x\"]}",
        )
        .unwrap();
        assert_eq!(
            search_for_workspace_root(&nested),
            root.join("packages/app")
        );
        // A package.json without workspaces is not a root.
        std::fs::remove_file(root.join("pnpm-workspace.yaml")).unwrap();
        std::fs::write(
            root.join("packages/app/package.json"),
            "{\"name\": \"app\"}",
        )
        .unwrap();
        let fallback = search_for_workspace_root(&nested);
        assert!(nested.starts_with(&fallback), "{fallback:?}");
        let _ = std::fs::remove_dir_all(&root);
    }
}
