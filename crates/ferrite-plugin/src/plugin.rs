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
