//! Plugin trait.

use crate::types::*;
use ferrite_config::ResolvedConfig;
use ferrite_config::UserConfig;
use ferrite_core::Result;
use ferrite_resolver::ResolvedId;

/// The plugin trait (spec §11 + §12).
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin name.
    fn name(&self) -> &'static str;

    /// Hook ordering.
    fn enforce(&self) -> Enforce {
        Enforce::Normal
    }

    /// Pre plugins see source syntax; normal/post plugins see lowered JavaScript.
    fn transform_phase(&self) -> TransformPhase {
        if self.enforce() == Enforce::Pre {
            TransformPhase::BeforeLowering
        } else {
            TransformPhase::AfterLowering
        }
    }

    /// Stable compiler/options identity used to isolate transform caches.
    fn cache_key(&self) -> String {
        self.name().to_string()
    }

    /// Framework compile-time defaults. Explicit environment/user defines win.
    fn compiler_defines(
        &self,
        _environment: &ferrite_core::Environment,
    ) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
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

    /// Configure the preview server (legacy control surface).
    async fn configure_preview_server(&self, _server: &mut dyn ServerControl) -> Result<()> {
        Ok(())
    }

    /// Configure the preview server: extra headers, static mounts, proxies.
    ///
    /// Runs after [`Plugin::configure_preview_server`]; new plugins should
    /// implement this hook instead.
    async fn configure_preview(&self, _preview: &mut PreviewControl) -> Result<()> {
        Ok(())
    }

    /// Mutate input options before the build starts (Rollup `options`).
    async fn options(&self, _ctx: &PluginContext, _options: &mut BundleOptions) -> Result<()> {
        Ok(())
    }

    /// Mutate output options before chunks are named (Rollup `outputOptions`).
    async fn output_options(
        &self,
        _ctx: &PluginContext,
        _options: &mut OutputOptions,
    ) -> Result<()> {
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

    /// Resolve a dynamic `import()` specifier (Rollup `resolveDynamicImport`).
    ///
    /// Falls back to [`Plugin::resolve_id`] when every hook returns `None`.
    async fn resolve_dynamic_import(
        &self,
        _ctx: &PluginContext,
        _request: DynamicImportRequest,
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

    /// Custom hot-update handling (Vite `handleHotUpdate`, legacy).
    async fn handle_hot_update(
        &self,
        _ctx: &PluginContext,
        _event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        Ok(None)
    }

    /// Custom hot-update handling (Vite 6 `hotUpdate`).
    ///
    /// Runs before [`Plugin::handle_hot_update`]; the first `Some` across
    /// both hooks wins.
    async fn hot_update(
        &self,
        _ctx: &PluginContext,
        _event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        Ok(None)
    }

    /// Decide whether a cached module must be re-transformed (Vite
    /// `shouldTransformCachedModule`). `Some(true)` forces a re-transform;
    /// `None`/`Some(false)` keeps the cached transform.
    async fn should_transform_cached_module(
        &self,
        _ctx: &PluginContext,
        _module: CachedModuleInfo,
    ) -> Result<Option<bool>> {
        Ok(None)
    }

    /// Observe a file-watcher event (Vite `watchChange`).
    async fn watch_change(&self, _ctx: &PluginContext, _event: WatchEvent) -> Result<()> {
        Ok(())
    }

    /// Map an emitted file to its public URL (Vite `resolveFileUrl`).
    /// First `Some` wins; `None` keeps the default URL.
    async fn resolve_file_url(
        &self,
        _ctx: &PluginContext,
        _request: ResolveFileUrlRequest,
    ) -> Result<Option<String>> {
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

    /// Prepend a banner to a chunk (Rollup `banner`).
    async fn banner(&self, _ctx: &PluginContext, _chunk: RenderChunk) -> Result<Option<String>> {
        Ok(None)
    }

    /// Prepend an intro inside a chunk (Rollup `intro`).
    async fn intro(&self, _ctx: &PluginContext, _chunk: RenderChunk) -> Result<Option<String>> {
        Ok(None)
    }

    /// Append an outro inside a chunk (Rollup `outro`).
    async fn outro(&self, _ctx: &PluginContext, _chunk: RenderChunk) -> Result<Option<String>> {
        Ok(None)
    }

    /// Append a footer to a chunk (Rollup `footer`).
    async fn footer(&self, _ctx: &PluginContext, _chunk: RenderChunk) -> Result<Option<String>> {
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
