//! Rust-native plugin API with Vite/Rollup semantics (spec §11–§14).
//!
//! [`Plugin`] mirrors the Rollup/Vite hook surface; [`PluginContainer`]
//! orders hooks by [`Enforce`] and filters by [`Apply`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ferrite_config::{ResolvedConfig, UserConfig};
use ferrite_core::{Environment, EnvironmentKind, ModuleId, ModuleType, Result, SourceMap};
use ferrite_graph::{ModuleGraph, ModuleNode};
use ferrite_resolver::{ResolveKind, ResolvedId, Resolver};

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

fn regex_match(pattern: &str, text: &str) -> bool {
    regex::Regex::new(pattern).is_ok_and(|regex| regex.is_match(text))
}

/// The plugin trait (spec §11 + §12).
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name.
    fn name(&self) -> &'static str;

    /// Hook ordering.
    fn enforce(&self) -> Enforce {
        Enforce::Normal
    }

    /// Serve/build applicability.
    fn apply(&self) -> Apply {
        Apply::All
    }

    /// Optional `transform` hook filter (§12).
    fn transform_filter(&self) -> Option<HookFilter> {
        None
    }

    /// Mutate user config before resolution.
    async fn config(&self, _config: &mut UserConfig) -> Result<()> {
        Ok(())
    }

    /// Observe the resolved config.
    async fn config_resolved(&self, _config: &ResolvedConfig) -> Result<()> {
        Ok(())
    }

    /// Configure the dev server.
    async fn configure_server(&self, _server: &mut dyn ServerControl) -> Result<()> {
        Ok(())
    }

    /// Configure the preview server.
    async fn configure_preview_server(&self, _server: &mut dyn ServerControl) -> Result<()> {
        Ok(())
    }

    /// Build start.
    async fn build_start(&self, _ctx: &PluginContext) -> Result<()> {
        Ok(())
    }

    /// Resolve a specifier to a module id.
    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        _request: ResolveHookRequest<'_>,
    ) -> Result<Option<ResolvedId>> {
        Ok(None)
    }

    /// Load a module's source.
    async fn load(
        &self,
        _ctx: &PluginContext,
        _request: LoadRequest,
    ) -> Result<Option<LoadResult>> {
        Ok(None)
    }

    /// Transform a module's code.
    async fn transform(
        &self,
        _ctx: &PluginContext,
        _request: TransformRequest,
    ) -> Result<Option<TransformResult>> {
        Ok(None)
    }

    /// Transform `index.html`.
    async fn transform_index_html(
        &self,
        _ctx: &PluginContext,
        _html: HtmlTransformContext,
    ) -> Result<Option<HtmlTransformResult>> {
        Ok(None)
    }

    /// Custom hot-update handling.
    async fn handle_hot_update(
        &self,
        _ctx: &PluginContext,
        _event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        Ok(None)
    }

    /// A module finished parsing.
    async fn module_parsed(&self, _ctx: &PluginContext, _module: ModuleParsed) -> Result<()> {
        Ok(())
    }

    /// Build end (called with the error, if any).
    async fn build_end(&self, _ctx: &PluginContext, _end: BuildEnd) -> Result<()> {
        Ok(())
    }

    /// Render start.
    async fn render_start(&self, _ctx: &PluginContext, _start: RenderStart) -> Result<()> {
        Ok(())
    }

    /// Render a chunk.
    async fn render_chunk(
        &self,
        _ctx: &PluginContext,
        _chunk: RenderChunk,
    ) -> Result<Option<RenderChunkResult>> {
        Ok(None)
    }

    /// Contribute extra chunk-hash input.
    async fn augment_chunk_hash(
        &self,
        _ctx: &PluginContext,
        _chunk_id: &str,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    /// Mutate the output bundle before writing.
    async fn generate_bundle(
        &self,
        _ctx: &PluginContext,
        _bundle: &mut OutputBundle,
    ) -> Result<()> {
        Ok(())
    }

    /// Observe the written bundle.
    async fn write_bundle(&self, _ctx: &PluginContext, _bundle: &OutputBundle) -> Result<()> {
        Ok(())
    }

    /// Bundle closed.
    async fn close_bundle(&self) -> Result<()> {
        Ok(())
    }
}

/// Ordered, mode-filtered plugin set.
pub struct PluginContainer {
    /// Plugins in hook order (Pre → Normal → Post, stable).
    plugins: Vec<Arc<dyn Plugin>>,
    /// Current mode.
    mode: Apply,
}

impl PluginContainer {
    /// Build a container, sorting by [`Enforce`] and filtering by `mode`.
    #[must_use]
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, mode: Apply) -> Self {
        let mut plugins: Vec<Arc<dyn Plugin>> = plugins
            .into_iter()
            .filter(|plugin| plugin.apply().applies(mode))
            .collect();
        plugins.sort_by_key(|plugin| plugin.enforce());
        Self { plugins, mode }
    }

    /// Current mode.
    #[must_use]
    pub fn mode(&self) -> Apply {
        self.mode
    }

    /// Plugin names in hook order.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|plugin| plugin.name()).collect()
    }

    /// Number of plugins.
    #[must_use]
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Run `config` hooks in order.
    pub async fn hook_config(&self, config: &mut UserConfig) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .config(config)
                .await
                .map_err(|error| wrap(plugin, "config", error))?;
        }
        Ok(())
    }

    /// Run `configure_server` hooks in order.
    pub async fn hook_configure_server(&self, server: &mut dyn ServerControl) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .configure_server(server)
                .await
                .map_err(|error| wrap(plugin, "configure_server", error))?;
        }
        Ok(())
    }

    /// Run `configure_preview_server` hooks in order.
    pub async fn hook_configure_preview_server(
        &self,
        server: &mut dyn ServerControl,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .configure_preview_server(server)
                .await
                .map_err(|error| wrap(plugin, "configure_preview_server", error))?;
        }
        Ok(())
    }

    /// Run `config_resolved` hooks in order.
    pub async fn hook_config_resolved(&self, config: &ResolvedConfig) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .config_resolved(config)
                .await
                .map_err(|error| wrap(plugin, "config_resolved", error))?;
        }
        Ok(())
    }

    /// Run `build_start` hooks in order.
    pub async fn hook_build_start(&self, ctx: &PluginContext<'_>) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .build_start(ctx)
                .await
                .map_err(|error| wrap(plugin, "build_start", error))?;
        }
        Ok(())
    }

    /// Run `resolve_id` hooks; first `Some` wins.
    pub async fn hook_resolve_id(
        &self,
        ctx: &PluginContext<'_>,
        request: ResolveHookRequest<'_>,
    ) -> Result<Option<ResolvedId>> {
        for plugin in &self.plugins {
            let result = plugin
                .resolve_id(ctx, request.clone())
                .await
                .map_err(|error| wrap(plugin, "resolve_id", error))?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }

    /// Run `load` hooks; first `Some` wins.
    pub async fn hook_load(
        &self,
        ctx: &PluginContext<'_>,
        request: LoadRequest,
    ) -> Result<Option<LoadResult>> {
        for plugin in &self.plugins {
            let result = plugin
                .load(ctx, request.clone())
                .await
                .map_err(|error| wrap(plugin, "load", error))?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }

    /// Run `transform` hooks in sequence (output feeds the next hook).
    pub async fn hook_transform(
        &self,
        ctx: &PluginContext<'_>,
        request: TransformRequest,
    ) -> Result<TransformResult> {
        let mut code = request.code.clone();
        let mut map = None;
        let mut dependencies = Vec::new();
        for plugin in &self.plugins {
            if let Some(filter) = plugin.transform_filter() {
                if !filter.matches(&request.id, Some(&code)) {
                    continue;
                }
            }
            let hook_request = TransformRequest {
                id: request.id.clone(),
                code: code.clone(),
                module_type: request.module_type.clone(),
                environment: request.environment.clone(),
                ssr: request.ssr,
            };
            if let Some(result) = plugin
                .transform(ctx, hook_request)
                .await
                .map_err(|error| wrap(plugin, "transform", error))?
            {
                code = result.code;
                if result.map.is_some() {
                    map = result.map;
                }
                dependencies.extend(result.dependencies);
            }
        }
        Ok(TransformResult {
            code,
            map,
            dependencies,
        })
    }

    /// Run `transform_index_html` hooks in sequence.
    pub async fn hook_transform_index_html(
        &self,
        ctx: &PluginContext<'_>,
        html: HtmlTransformContext,
    ) -> Result<HtmlTransformResult> {
        let mut current = html.html.clone();
        let mut tags = Vec::new();
        for plugin in &self.plugins {
            let hook_html = HtmlTransformContext {
                html: current.clone(),
                path: html.path.clone(),
                ssr: html.ssr,
            };
            if let Some(result) = plugin
                .transform_index_html(ctx, hook_html)
                .await
                .map_err(|error| wrap(plugin, "transform_index_html", error))?
            {
                if let Some(replaced) = result.html {
                    current = replaced;
                }
                tags.extend(result.tags);
            }
        }
        Ok(HtmlTransformResult {
            html: Some(current),
            tags,
        })
    }

    /// Run `handle_hot_update` hooks; first `Some` wins.
    pub async fn hook_handle_hot_update(
        &self,
        ctx: &PluginContext<'_>,
        event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        for plugin in &self.plugins {
            let result = plugin
                .handle_hot_update(ctx, event.clone())
                .await
                .map_err(|error| wrap(plugin, "handle_hot_update", error))?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }

    /// Run `generate_bundle` hooks in order.
    pub async fn hook_generate_bundle(
        &self,
        ctx: &PluginContext<'_>,
        bundle: &mut OutputBundle,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .generate_bundle(ctx, bundle)
                .await
                .map_err(|error| wrap(plugin, "generate_bundle", error))?;
        }
        Ok(())
    }

    /// Run `write_bundle` hooks in order.
    pub async fn hook_write_bundle(
        &self,
        ctx: &PluginContext<'_>,
        bundle: &OutputBundle,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .write_bundle(ctx, bundle)
                .await
                .map_err(|error| wrap(plugin, "write_bundle", error))?;
        }
        Ok(())
    }

    /// Run `close_bundle` hooks in order.
    pub async fn hook_close_bundle(&self) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .close_bundle()
                .await
                .map_err(|error| wrap(plugin, "close_bundle", error))?;
        }
        Ok(())
    }

    /// Run `module_parsed` hooks in order.
    pub async fn hook_module_parsed(
        &self,
        ctx: &PluginContext<'_>,
        module: ModuleParsed,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .module_parsed(ctx, module.clone())
                .await
                .map_err(|error| wrap(plugin, "module_parsed", error))?;
        }
        Ok(())
    }

    /// Run `build_end` hooks in order.
    pub async fn hook_build_end(&self, ctx: &PluginContext<'_>, end: BuildEnd) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .build_end(ctx, end.clone())
                .await
                .map_err(|error| wrap(plugin, "build_end", error))?;
        }
        Ok(())
    }

    /// Run `render_chunk` hooks in sequence.
    pub async fn hook_render_chunk(
        &self,
        ctx: &PluginContext<'_>,
        chunk: RenderChunk,
    ) -> Result<RenderChunkResult> {
        let mut code = chunk.code.clone();
        let mut map = None;
        for plugin in &self.plugins {
            let hook_chunk = RenderChunk {
                id: chunk.id.clone(),
                code: code.clone(),
                is_entry: chunk.is_entry,
            };
            if let Some(result) = plugin
                .render_chunk(ctx, hook_chunk)
                .await
                .map_err(|error| wrap(plugin, "render_chunk", error))?
            {
                if let Some(replacement) = result.code {
                    code = replacement;
                }
                if result.map.is_some() {
                    map = result.map;
                }
            }
        }
        Ok(RenderChunkResult {
            code: Some(code),
            map,
        })
    }

    /// Collect `augment_chunk_hash` contributions.
    pub async fn hook_augment_chunk_hash(
        &self,
        ctx: &PluginContext<'_>,
        chunk_id: &str,
    ) -> Result<Vec<String>> {
        let mut parts = Vec::new();
        for plugin in &self.plugins {
            if let Some(part) = plugin
                .augment_chunk_hash(ctx, chunk_id)
                .await
                .map_err(|error| wrap(plugin, "augment_chunk_hash", error))?
            {
                parts.push(part);
            }
        }
        Ok(parts)
    }
}

fn wrap(
    plugin: &Arc<dyn Plugin>,
    hook: &str,
    error: ferrite_core::FerriteError,
) -> ferrite_core::FerriteError {
    ferrite_core::FerriteError::Plugin {
        plugin: plugin.name().to_string(),
        hook: hook.to_string(),
        message: error.to_string(),
    }
}

/// Reference plugin from spec §68: serves `*.txt?raw` as ESM strings.
pub struct RawTextPlugin;

#[async_trait::async_trait]
impl Plugin for RawTextPlugin {
    fn name(&self) -> &'static str {
        "raw-text"
    }

    async fn load(&self, _ctx: &PluginContext, request: LoadRequest) -> Result<Option<LoadResult>> {
        if let Some(path) = request.id.strip_suffix("?raw") {
            // Only handle text-like files; binary assets use `?url`.
            if path.ends_with(".txt") || path.ends_with(".md") {
                return Ok(Some(LoadResult {
                    code: format!("export default {path:?};\n"),
                    module_type: ModuleType::Js,
                    dependencies: vec![path.to_string()],
                }));
            }
        }
        Ok(None)
    }
}

/// Handle to a foreign (JS/Node-hosted) plugin (§57).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PluginHandle {
    /// Plugin name.
    pub name: String,
    /// Host that owns it (`embedded-js`, `node-adapter`).
    pub host: String,
}

/// A hook invocation on a foreign plugin host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HookName {
    /// `resolveId`.
    ResolveId,
    /// `load`.
    Load,
    /// `transform`.
    Transform,
    /// `transformIndexHtml`.
    TransformIndexHtml,
    /// `handleHotUpdate`.
    HandleHotUpdate,
    /// `generateBundle`.
    GenerateBundle,
}

/// Foreign plugin host: embedded JS runtime or Node adapter (§56–§57).
///
/// Tier-2/3 compatibility surface. The embedded runtime exposes only this
/// narrow JSON bridge — never unrestricted filesystem or memory access.
#[async_trait::async_trait]
pub trait ForeignPluginHost: Send + Sync {
    /// Host name.
    fn name(&self) -> &'static str;

    /// Call one hook on one foreign plugin.
    async fn call_hook(
        &self,
        plugin: &PluginHandle,
        hook: HookName,
        input: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

/// True for internal (`\0`-prefixed) virtual ids (§14).
#[must_use]
pub fn is_virtual_id(id: &str) -> bool {
    id.starts_with('\0')
}

/// Convert a user-facing `virtual:*` specifier to its internal id.
#[must_use]
pub fn to_virtual_id(specifier: &str) -> String {
    if let Some(rest) = specifier.strip_prefix("virtual:") {
        format!("\0virtual:{rest}")
    } else {
        specifier.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OrderPlugin {
        name: &'static str,
        enforce: Enforce,
    }

    #[async_trait::async_trait]
    impl Plugin for OrderPlugin {
        fn name(&self) -> &'static str {
            self.name
        }

        fn enforce(&self) -> Enforce {
            self.enforce
        }
    }

    #[test]
    fn container_orders_by_enforce() {
        let container = PluginContainer::new(
            vec![
                Arc::new(OrderPlugin {
                    name: "b",
                    enforce: Enforce::Post,
                }),
                Arc::new(OrderPlugin {
                    name: "a",
                    enforce: Enforce::Pre,
                }),
            ],
            Apply::All,
        );
        assert_eq!(container.names(), vec!["a", "b"]);
    }

    #[test]
    fn hook_filter_matches() {
        let filter = HookFilter {
            id: Some(r"\.tsx?$".to_string()),
            code: None,
            query: None,
        };
        assert!(filter.matches("/src/a.ts", None));
        assert!(!filter.matches("/src/a.css", None));
    }
}
