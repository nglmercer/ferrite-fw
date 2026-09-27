//! Render hooks for the production bundler.
//!
//! [`BundleHooks`] is the bundler side of the plugin render pipeline
//! (`renderStart` / `renderChunk` / `augmentChunkHash` / `banner` / `intro` /
//! `outro` / `footer`). The builder in the `ferrite` crate implements it over
//! a [`ferrite_plugin::PluginContainer`]; the bundler itself stays
//! plugin-agnostic so unit tests can drive it with [`NoHooks`] or fakes.

use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_plugin::ChunkWrapper;

/// Render-pipeline hooks consulted while bundling.
#[async_trait::async_trait]
pub trait BundleHooks: Send + Sync {
    /// Rendering starts (entries known, chunks not yet planned).
    async fn render_start(&self, _entries: &[ModuleId]) -> Result<()> {
        Ok(())
    }

    /// Transform one chunk's code before hashing and import rewriting.
    ///
    /// Returning `Some` replaces the chunk code (the replacement is what
    /// gets hashed and rewritten). Unlike Rollup — which runs `renderChunk`
    /// on fully rewritten code — Ferrite runs it pre-rewrite so the content
    /// hash covers the hooked code without a circular dependency on file
    /// names; hooks therefore see module-resolved (dev-URL) specifiers.
    async fn render_chunk(
        &self,
        _id: &str,
        _code: String,
        _is_entry: bool,
    ) -> Result<Option<String>> {
        Ok(None)
    }

    /// Extra hash input for a chunk id (empty by default).
    async fn chunk_hash_extra(&self, _chunk_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Banner/intro/outro/footer wrapper for a chunk (empty by default).
    ///
    /// The wrapper text is folded into the chunk hash and applied after
    /// import rewriting, so wrappers never see rewritten specifiers and
    /// never break them.
    async fn chunk_wrapper(
        &self,
        _id: &str,
        _code: &str,
        _is_entry: bool,
    ) -> Result<ChunkWrapper> {
        Ok(ChunkWrapper::default())
    }
}

/// No-op hooks (plain bundling).
pub struct NoHooks;

#[async_trait::async_trait]
impl BundleHooks for NoHooks {}
