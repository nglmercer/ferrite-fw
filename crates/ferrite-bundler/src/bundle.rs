//! Bundler request/response types.

use ferrite_config::ResolvedConfig;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_graph::ModuleGraph;
use ferrite_manifest::BuildManifest;
use ferrite_manifest::SsrManifest;
use ferrite_plugin::OutputBundle;
use std::collections::HashMap;
use std::path::PathBuf;

/// A chunk (§38). v0.1 maps one module → one chunk file.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Chunk id (module id).
    pub id: String,
    /// Modules in this chunk.
    pub modules: Vec<ModuleId>,
    /// Static chunk imports (chunk ids).
    pub imports: Vec<String>,
    /// Dynamic chunk imports (chunk ids).
    pub dynamic_imports: Vec<String>,
    /// Exported names.
    pub exports: Vec<String>,
    /// True for entry chunks.
    pub entry: bool,
    /// Chunk name (for file naming).
    pub name: String,
    /// Rendered code.
    pub code: String,
    /// Source map JSON.
    pub map: Option<String>,
    /// Output file name (after hashing).
    pub file_name: Option<String>,
}

/// Bundler input.
#[derive(Debug, Clone)]
pub struct BundleRequest {
    /// Entry module ids.
    pub entries: Vec<ModuleId>,
    /// Environment (`client`/`ssr`).
    pub env: String,
    /// True when minifying.
    pub minify: bool,
    /// True when emitting source maps.
    pub sourcemap: bool,
    /// Append `sourceMappingURL` comments (`false` = hidden maps, §41).
    pub map_comment: bool,
    /// Drop unused export statements, then re-minify (§38).
    pub treeshake: bool,
    /// Compiler engine for shake re-minification (`oxc` / `swc`).
    pub engine: String,
    /// Scope-hoist each entry closure into one file (§91).
    pub scope_hoist: bool,
}

impl BundleRequest {
    /// Request with comments enabled (external maps).
    #[must_use]
    pub fn with_maps(entries: Vec<ModuleId>, env: &str) -> Self {
        Self {
            entries,
            env: env.to_string(),
            minify: false,
            sourcemap: true,
            map_comment: true,
            treeshake: false,
            engine: "oxc".to_string(),
            scope_hoist: false,
        }
    }
}

/// Bundler output.
#[derive(Debug)]
pub struct BundleOutput {
    /// Chunks by id.
    pub chunks: HashMap<String, Chunk>,
    /// Files ready to write (chunk files + maps + assets).
    pub bundle: OutputBundle,
    /// Client manifest.
    pub manifest: BuildManifest,
    /// SSR manifest (module → files).
    pub ssr_manifest: SsrManifest,
    /// Tree-shaking stats.
    pub stats: BundleStats,
}

/// Bundle statistics.
#[derive(Debug, Clone, Default)]
pub struct BundleStats {
    /// Modules visited.
    pub modules_visited: usize,
    /// Modules emitted.
    pub modules_emitted: usize,
    /// Modules dropped as unreachable/side-effect-free.
    pub modules_dropped: usize,
    /// Total bytes emitted (code only).
    pub bytes: usize,
}

/// The bundler trait (§37).
#[async_trait::async_trait]
pub trait Bundler: Send + Sync {
    /// Bundle entries into an output bundle.
    async fn bundle(
        &self,
        graph: &ModuleGraph,
        config: &BuildBundleConfig,
        request: BundleRequest,
    ) -> Result<BundleOutput>;
}

/// Build configuration subset used by the bundler.
#[derive(Debug, Clone)]
pub struct BuildBundleConfig {
    /// Output dir (absolute).
    pub out_dir: PathBuf,
    /// Asset file pattern.
    pub asset_pattern: String,
    /// Chunk file pattern.
    pub chunk_pattern: String,
    /// Extracted CSS file pattern.
    pub css_pattern: String,
}

impl BuildBundleConfig {
    /// Derive from resolved config.
    #[must_use]
    pub fn from_resolved(config: &ResolvedConfig) -> Self {
        Self {
            out_dir: config.out_dir(),
            asset_pattern: "assets/[name]-[hash][ext]".to_string(),
            chunk_pattern: "assets/[name]-[hash].js".to_string(),
            css_pattern: "assets/[name]-[hash].css".to_string(),
        }
    }
}
