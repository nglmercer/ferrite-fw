//! Plugin container.

use crate::builtin::*;
use crate::plugin::*;
use crate::types::*;
use ferrite_config::ResolvedConfig;
use ferrite_config::UserConfig;
use ferrite_core::Result;
use ferrite_resolver::ResolvedId;
use std::sync::Arc;

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

    /// Run `configure_preview` hooks in order (after the legacy
    /// `configure_preview_server` hooks, which take a `&mut dyn ServerControl`).
    pub async fn hook_configure_preview(&self, preview: &mut PreviewControl) -> Result<()> {
        self.hook_configure_preview_server(preview as &mut dyn ServerControl)
            .await?;
        for plugin in &self.plugins {
            plugin
                .configure_preview(preview)
                .await
                .map_err(|error| wrap(plugin, "configure_preview", error))?;
        }
        Ok(())
    }

    /// Run `options` hooks in order.
    pub async fn hook_options(
        &self,
        ctx: &PluginContext<'_>,
        options: &mut BundleOptions,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .options(ctx, options)
                .await
                .map_err(|error| wrap(plugin, "options", error))?;
        }
        Ok(())
    }

    /// Run `output_options` hooks in order.
    pub async fn hook_output_options(
        &self,
        ctx: &PluginContext<'_>,
        options: &mut OutputOptions,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .output_options(ctx, options)
                .await
                .map_err(|error| wrap(plugin, "output_options", error))?;
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

    /// Run `hot_update` hooks, falling back to `handle_hot_update`.
    pub async fn hook_hot_update(
        &self,
        ctx: &PluginContext<'_>,
        event: HotUpdateEvent,
    ) -> Result<Option<HotUpdateResult>> {
        for plugin in &self.plugins {
            let result = plugin
                .hot_update(ctx, event.clone())
                .await
                .map_err(|error| wrap(plugin, "hot_update", error))?;
            if result.is_some() {
                return Ok(result);
            }
        }
        self.hook_handle_hot_update(ctx, event).await
    }

    /// Run `resolve_dynamic_import` hooks; first `Some` wins.
    pub async fn hook_resolve_dynamic_import(
        &self,
        ctx: &PluginContext<'_>,
        request: DynamicImportRequest,
    ) -> Result<Option<ResolvedId>> {
        for plugin in &self.plugins {
            let result = plugin
                .resolve_dynamic_import(ctx, request.clone())
                .await
                .map_err(|error| wrap(plugin, "resolve_dynamic_import", error))?;
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }

    /// Run `should_transform_cached_module` hooks; true when any hook
    /// forces a re-transform.
    pub async fn hook_should_transform_cached_module(
        &self,
        ctx: &PluginContext<'_>,
        module: CachedModuleInfo,
    ) -> Result<bool> {
        for plugin in &self.plugins {
            let result = plugin
                .should_transform_cached_module(ctx, module.clone())
                .await
                .map_err(|error| wrap(plugin, "should_transform_cached_module", error))?;
            if result == Some(true) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Run `watch_change` hooks in order.
    pub async fn hook_watch_change(
        &self,
        ctx: &PluginContext<'_>,
        event: WatchEvent,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .watch_change(ctx, event.clone())
                .await
                .map_err(|error| wrap(plugin, "watch_change", error))?;
        }
        Ok(())
    }

    /// Run `resolve_file_url` hooks; first `Some` wins.
    pub async fn hook_resolve_file_url(
        &self,
        ctx: &PluginContext<'_>,
        request: ResolveFileUrlRequest,
    ) -> Result<Option<String>> {
        for plugin in &self.plugins {
            let result = plugin
                .resolve_file_url(ctx, request.clone())
                .await
                .map_err(|error| wrap(plugin, "resolve_file_url", error))?;
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

    /// Run `render_start` hooks in order.
    pub async fn hook_render_start(
        &self,
        ctx: &PluginContext<'_>,
        start: RenderStart,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin
                .render_start(ctx, start.clone())
                .await
                .map_err(|error| wrap(plugin, "render_start", error))?;
        }
        Ok(())
    }

    /// Collect `banner` / `intro` / `outro` / `footer` pieces, joined in
    /// hook order per piece.
    pub async fn hook_chunk_wrapper(
        &self,
        ctx: &PluginContext<'_>,
        chunk: RenderChunk,
    ) -> Result<ChunkWrapper> {
        let mut banners = Vec::new();
        let mut intros = Vec::new();
        let mut outros = Vec::new();
        let mut footers = Vec::new();
        for plugin in &self.plugins {
            if let Some(text) = plugin
                .banner(ctx, chunk.clone())
                .await
                .map_err(|error| wrap(plugin, "banner", error))?
            {
                banners.push(text);
            }
            if let Some(text) = plugin
                .intro(ctx, chunk.clone())
                .await
                .map_err(|error| wrap(plugin, "intro", error))?
            {
                intros.push(text);
            }
            if let Some(text) = plugin
                .outro(ctx, chunk.clone())
                .await
                .map_err(|error| wrap(plugin, "outro", error))?
            {
                outros.push(text);
            }
            if let Some(text) = plugin
                .footer(ctx, chunk.clone())
                .await
                .map_err(|error| wrap(plugin, "footer", error))?
            {
                footers.push(text);
            }
        }
        let join = |parts: Vec<String>| {
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        };
        Ok(ChunkWrapper {
            banner: join(banners),
            intro: join(intros),
            outro: join(outros),
            footer: join(footers),
        })
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
