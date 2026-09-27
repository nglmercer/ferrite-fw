//! Plugin hook types.

use ferrite_config::ResolvedConfig;
use ferrite_core::Environment;
use ferrite_core::EnvironmentKind;
use ferrite_core::ModuleId;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_core::SourceMap;
use ferrite_graph::ModuleGraph;
use ferrite_graph::ModuleNode;
use ferrite_resolver::ResolveKind;
use ferrite_resolver::ResolvedId;
use ferrite_resolver::Resolver;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Hook ordering (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Enforce {
    /// Run before core plugins.
    Pre,
    /// Default order.
    Normal,
    /// Run after core plugins.
    Post,
}

/// When a plugin applies (§11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Apply {
    /// Dev server only.
    Serve,
    /// Build only.
    Build,
    /// Both.
    All,
}

impl Apply {
    /// True when the plugin applies to `mode`.
    #[must_use]
    pub fn applies(&self, mode: Apply) -> bool {
        matches!(self, Self::All) || *self == mode
    }
}

/// Narrow control surface a plugin can use during `configure_server`.
///
/// (The full `DevServer` type lives in `ferrite-server`, which depends on
/// this crate; the trait keeps the dependency one-directional.)
pub trait ServerControl: Send + Sync {
    /// Resolved configuration.
    fn resolved_config(&self) -> &ResolvedConfig;
    /// Project root.
    fn root(&self) -> &Path;
}

/// Plugin hook context (§13).
pub struct PluginContext<'a> {
    /// Module graph.
    pub graph: &'a ModuleGraph,
    /// Resolver.
    pub resolver: &'a Resolver,
    /// Current environment.
    pub environment: &'a Environment,
    /// Emitted files (shared with the build/dev pipeline).
    pub emitted: &'a Mutex<HashMap<String, EmittedFile>>,
    /// Extra watch files registered by plugins.
    pub watch_files: &'a Mutex<Vec<String>>,
    /// Collected warnings.
    pub warnings: &'a Mutex<Vec<String>>,
}

impl PluginContext<'_> {
    /// Resolve a specifier (plugin `resolveId` equivalent helper).
    pub fn resolve(&self, specifier: &str, importer: Option<&ModuleId>) -> Result<ResolvedId> {
        self.resolver.resolve(&ferrite_resolver::ResolveRequest {
            specifier,
            importer,
            environment: self.environment.kind.clone(),
            kind: ResolveKind::Import,
        })
    }

    /// Emit a file into the bundle / dev pipeline.
    pub fn emit_file(&self, file: EmittedFile) {
        if let Ok(mut emitted) = self.emitted.lock() {
            emitted.insert(file.name.clone(), file);
        }
    }

    /// Look up an emitted file name.
    #[must_use]
    pub fn get_file_name(&self, reference_id: &str) -> Option<String> {
        self.emitted
            .lock()
            .ok()
            .and_then(|emitted| emitted.get(reference_id).map(|file| file.name.clone()))
    }

    /// Module info for an id.
    #[must_use]
    pub fn get_module_info(&self, id: &ModuleId) -> Option<ModuleNode> {
        self.graph.get(id)
    }

    /// All known module ids.
    #[must_use]
    pub fn get_module_ids(&self) -> Vec<ModuleId> {
        self.graph.module_ids()
    }

    /// Register an extra watch file.
    pub fn add_watch_file(&self, file: &str) {
        if let Ok(mut watch) = self.watch_files.lock() {
            if !watch.iter().any(|entry| entry == file) {
                watch.push(file.to_string());
            }
        }
    }

    /// Record a warning.
    pub fn warn(&self, message: &str) {
        if let Ok(mut warnings) = self.warnings.lock() {
            warnings.push(message.to_string());
        }
    }
}

/// A file emitted into the output bundle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmittedFile {
    /// Output file name.
    pub name: String,
    /// File bytes.
    #[serde(skip)]
    pub contents: Vec<u8>,
    /// True for entry chunks.
    pub is_entry: bool,
}

/// Output bundle passed to `generate_bundle` / `write_bundle`.
#[derive(Debug, Default)]
pub struct OutputBundle {
    /// Files by output name.
    pub files: HashMap<String, EmittedFile>,
}

impl OutputBundle {
    /// Insert or replace a file.
    pub fn insert(&mut self, file: EmittedFile) {
        self.files.insert(file.name.clone(), file);
    }

    /// Remove a file.
    pub fn remove(&mut self, name: &str) -> Option<EmittedFile> {
        self.files.remove(name)
    }
}

/// `resolveId` hook request.
#[derive(Debug, Clone)]
pub struct ResolveHookRequest<'a> {
    /// Raw specifier.
    pub specifier: &'a str,
    /// Importer, if any.
    pub importer: Option<&'a ModuleId>,
    /// Environment.
    pub environment: EnvironmentKind,
    /// True for SSR resolution.
    pub ssr: bool,
}

/// `load` hook request (§68).
#[derive(Debug, Clone)]
pub struct LoadRequest {
    /// Resolved id to load.
    pub id: String,
    /// Environment.
    pub environment: EnvironmentKind,
}

/// `load` hook result (§68).
#[derive(Debug, Clone)]
pub struct LoadResult {
    /// Module source.
    pub code: String,
    /// Module type override.
    pub module_type: ModuleType,
    /// Extra dependencies.
    pub dependencies: Vec<String>,
}

impl Default for LoadResult {
    fn default() -> Self {
        Self {
            code: String::new(),
            module_type: ModuleType::Js,
            dependencies: Vec::new(),
        }
    }
}

/// `transform` hook request.
#[derive(Debug, Clone)]
pub struct TransformRequest {
    /// Module id.
    pub id: String,
    /// Current code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
    /// Environment.
    pub environment: EnvironmentKind,
    /// True for SSR transforms.
    pub ssr: bool,
}

/// `transform` hook result.
#[derive(Debug, Clone)]
pub struct TransformResult {
    /// Transformed code.
    pub code: String,
    /// Source map.
    pub map: Option<SourceMap>,
    /// Extra dependencies.
    pub dependencies: Vec<String>,
}

/// HTML transformation context (`transformIndexHtml`).
#[derive(Debug, Clone)]
pub struct HtmlTransformContext {
    /// Current HTML.
    pub html: String,
    /// Request path.
    pub path: String,
    /// True for SSR HTML.
    pub ssr: bool,
}

/// HTML transformation result.
#[derive(Debug, Clone, Default)]
pub struct HtmlTransformResult {
    /// Replacement HTML (when fully overridden).
    pub html: Option<String>,
    /// Tags to inject.
    pub tags: Vec<HtmlTag>,
}

/// An injected HTML tag.
#[derive(Debug, Clone)]
pub struct HtmlTag {
    /// Tag name (`script`, `link`, `meta`, ...).
    pub tag: String,
    /// Attributes.
    pub attrs: HashMap<String, String>,
    /// Inner HTML.
    pub children: Option<String>,
    /// Injection point.
    pub inject_to: HtmlInjectTo,
}

/// HTML injection points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HtmlInjectTo {
    /// `<head>` (default).
    #[default]
    Head,
    /// Start of `<head>`.
    HeadPrepend,
    /// `<body>`.
    Body,
    /// Start of `<body>`.
    BodyPrepend,
}

/// Hot-update event (`handleHotUpdate`).
#[derive(Debug, Clone)]
pub struct HotUpdateEvent {
    /// Changed file (root-relative URL).
    pub file: String,
    /// Affected modules.
    pub modules: Vec<ModuleId>,
    /// Timestamp (unix millis).
    pub timestamp: u64,
}

/// Hot-update result: custom boundary or full reload.
#[derive(Debug, Clone, Default)]
pub struct HotUpdateResult {
    /// Modules to send as the update boundary.
    pub modules: Vec<ModuleId>,
    /// Force a full reload.
    pub full_reload: bool,
}

/// `moduleParsed` hook payload.
#[derive(Debug, Clone)]
pub struct ModuleParsed {
    /// Parsed module id.
    pub id: ModuleId,
}

/// `buildEnd` hook payload.
#[derive(Debug, Clone, Default)]
pub struct BuildEnd {
    /// Error message when the build failed, if any.
    pub error: Option<String>,
}

/// `renderStart` hook payload.
#[derive(Debug, Clone)]
pub struct RenderStart {
    /// Entry ids.
    pub entries: Vec<ModuleId>,
}

/// `renderChunk` hook payload.
#[derive(Debug, Clone)]
pub struct RenderChunk {
    /// Chunk id.
    pub id: String,
    /// Current chunk code.
    pub code: String,
    /// True for entry chunks.
    pub is_entry: bool,
}

/// `renderChunk` hook result.
#[derive(Debug, Clone, Default)]
pub struct RenderChunkResult {
    /// Replacement code.
    pub code: Option<String>,
    /// Replacement map.
    pub map: Option<SourceMap>,
}

/// Hook filter (§12): skip hooks cheaply before invoking them.
#[derive(Debug, Clone, Default)]
pub struct HookFilter {
    /// Only match these ids (regex).
    pub id: Option<String>,
    /// Only match when code matches (regex).
    pub code: Option<String>,
    /// Only match these queries (regex over the `?query` part).
    pub query: Option<String>,
}

impl HookFilter {
    /// True when the hook should run for (`id`, `code`).
    #[must_use]
    pub fn matches(&self, id: &str, code: Option<&str>) -> bool {
        if let Some(pattern) = &self.id {
            if !regex_match(pattern, id) {
                return false;
            }
        }
        if let Some(pattern) = &self.query {
            let query = id.split_once('?').map_or("", |(_, query)| query);
            if !regex_match(pattern, query) {
                return false;
            }
        }
        if let Some(pattern) = &self.code {
            if !code.is_some_and(|code| regex_match(pattern, code)) {
                return false;
            }
        }
        true
    }
}

pub(crate) fn regex_match(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|regex| regex.is_match(text))
}
