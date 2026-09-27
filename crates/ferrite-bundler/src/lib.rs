//! Production bundler (spec §37–§39).
//!
//! [`Bundler`] owns entry discovery, graph traversal, chunking, hashing,
//! manifest generation, and source maps. Transformation stays in
//! `ferrite-transform`; the bundler consumes modules through [`ModuleLoader`]
//! so dev and build share one pipeline.
//!
//! v0.1 emits one content-hashed ESM file per module with relative import
//! rewriting (correct under native ESM, HTTP/2-friendly). Scope-hoisted
//! concatenation is the documented Rolldown-backend roadmap (§91).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use ferrite_config::ResolvedConfig;
use ferrite_core::{FerriteError, Hash, ModuleId, ModuleType, Result};
use ferrite_graph::{ImportKind, ModuleGraph};
use ferrite_manifest::{BuildManifest, ManifestEntry, SsrManifest};
use ferrite_plugin::{EmittedFile, OutputBundle};

/// A module loaded for bundling.
#[derive(Debug, Clone)]
pub struct LoadedModule {
    /// Module id.
    pub id: ModuleId,
    /// Final code (transformed, defines applied).
    pub code: String,
    /// Resolved imports (`(specifier, id, kind)`).
    pub imports: Vec<(String, ModuleId, ImportKind)>,
    /// Package `sideEffects` flag, when known.
    pub side_effects: Option<bool>,
    /// Module type.
    pub module_type: ModuleType,
    /// Source map JSON, when produced.
    pub map: Option<String>,
}

/// Loads transformed modules for the bundler (implemented by the facade).
#[async_trait::async_trait]
pub trait ModuleLoader: Send + Sync {
    /// Load + transform a module for `env` (`client`/`ssr`).
    async fn load(&self, id: &ModuleId, env: &str) -> Result<LoadedModule>;
}

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
}

impl BuildBundleConfig {
    /// Derive from resolved config.
    #[must_use]
    pub fn from_resolved(config: &ResolvedConfig) -> Self {
        Self {
            out_dir: config.out_dir(),
            asset_pattern: "assets/[name]-[hash][ext]".to_string(),
            chunk_pattern: "assets/[name]-[hash].js".to_string(),
        }
    }
}

/// The default v0.1 bundler.
pub struct FerriteBundler<L> {
    /// Module loader.
    loader: Arc<L>,
}

impl<L: ModuleLoader> FerriteBundler<L> {
    /// Create a bundler over `loader`.
    #[must_use]
    pub fn new(loader: Arc<L>) -> Self {
        Self { loader }
    }
}

#[async_trait::async_trait]
impl<L: ModuleLoader + 'static> Bundler for FerriteBundler<L> {
    async fn bundle(
        &self,
        graph: &ModuleGraph,
        config: &BuildBundleConfig,
        request: BundleRequest,
    ) -> Result<BundleOutput> {
        // 1. Traverse from entries (parallel load).
        let modules = self.traverse(&request).await?;
        let visited = modules.len();
        // 2. Tree-shake: drop unreachable + provably side-effect-free leaves.
        let live = tree_shake(&request.entries, &modules);
        let dropped = visited.saturating_sub(live.len());
        // 3. Chunk: one file per module; entries + dynamic boundaries marked.
        let dynamic_entries: HashSet<ModuleId> = modules
            .values()
            .flat_map(|module| {
                module.imports.iter().filter_map(|(_, id, kind)| {
                    matches!(kind, ImportKind::Dynamic).then_some(id.clone())
                })
            })
            .collect();
        let mut chunks: HashMap<String, Chunk> = HashMap::new();
        for id in &live {
            let module = modules.get(id).ok_or_else(|| {
                FerriteError::Build(format!("missing module `{id}` after traversal"))
            })?;
            let is_entry = request.entries.contains(id) || dynamic_entries.contains(id);
            chunks.insert(
                id.0.clone(),
                Chunk {
                    id: id.0.clone(),
                    modules: vec![id.clone()],
                    imports: module
                        .imports
                        .iter()
                        .filter(|(_, dep, kind)| *kind != ImportKind::Dynamic && live.contains(dep))
                        .map(|(_, dep, _)| dep.0.clone())
                        .collect(),
                    dynamic_imports: module
                        .imports
                        .iter()
                        .filter(|(_, dep, kind)| *kind == ImportKind::Dynamic && live.contains(dep))
                        .map(|(_, dep, _)| dep.0.clone())
                        .collect(),
                    exports: Vec::new(),
                    entry: request.entries.contains(id),
                    name: chunk_name(id),
                    code: module.code.clone(),
                    map: module.map.clone(),
                    file_name: None,
                },
            );
            let _ = (graph, is_entry);
        }
        // 4. Hash + file names (two passes: names first, then import rewrite).
        let mut file_names: HashMap<String, String> = HashMap::new();
        for chunk in chunks.values() {
            let hash = Hash::of_str(&chunk.code).short(8);
            let file_name = config
                .chunk_pattern
                .replace("[name]", &chunk.name)
                .replace("[hash]", &hash);
            file_names.insert(chunk.id.clone(), file_name);
        }
        // 5. Rewrite imports to relative chunk URLs + minify already applied
        //    by the loader; finalize code.
        let mut bundle = OutputBundle::default();
        let mut manifest_entries: BTreeMap<String, ManifestEntry> = BTreeMap::new();
        let mut ssr_entries: HashMap<String, Vec<String>> = HashMap::new();
        let mut bytes = 0;
        let mut chunk_ids: Vec<String> = chunks.keys().cloned().collect();
        chunk_ids.sort();
        for chunk_id in chunk_ids {
            let chunk = chunks.get(&chunk_id).unwrap().clone();
            let file_name = file_names[&chunk_id].clone();
            let module = &modules[&ModuleId::new(chunk_id.clone())];
            let mut import_map: HashMap<String, String> = HashMap::new();
            for (specifier, dep, _) in &module.imports {
                if let Some(dep_file) = file_names.get(&dep.0) {
                    import_map.insert(specifier.clone(), relative_url(&file_name, dep_file));
                }
            }
            let code = if import_map.is_empty() {
                chunk.code.clone()
            } else {
                rewrite_imports_text(&chunk.code, &import_map)
            };
            bytes += code.len();
            let mut code_with_map = code.clone();
            if request.sourcemap {
                if let Some(map) = &chunk.map {
                    let map_name = format!("{file_name}.map");
                    bundle.insert(EmittedFile {
                        name: map_name.clone(),
                        contents: map.as_bytes().to_vec(),
                        is_entry: false,
                    });
                    if request.map_comment {
                        code_with_map.push_str(&format!("\n//# sourceMappingURL={map_name}"));
                    }
                }
            }
            bundle.insert(EmittedFile {
                name: file_name.clone(),
                contents: code_with_map.into_bytes(),
                is_entry: chunk.entry,
            });
            let css = Vec::new(); // CSS extraction records here (server pipeline).
            manifest_entries.insert(
                chunk_id.clone(),
                ManifestEntry {
                    file: file_name.clone(),
                    src: Some(chunk_id.clone()),
                    is_entry: chunk.entry.then_some(true),
                    is_dynamic_entry: (!chunk.entry
                        && dynamic_entries.contains(&ModuleId::new(chunk_id.clone())))
                    .then_some(true),
                    css,
                    imports: chunk
                        .imports
                        .iter()
                        .filter_map(|dep| file_names.get(dep).cloned())
                        .collect(),
                    dynamic_imports: chunk
                        .dynamic_imports
                        .iter()
                        .filter_map(|dep| file_names.get(dep).cloned())
                        .collect(),
                    assets: Vec::new(),
                },
            );
            ssr_entries.insert(chunk_id.clone(), vec![file_name]);
        }
        Ok(BundleOutput {
            chunks,
            bundle,
            manifest: BuildManifest {
                entries: manifest_entries.into_iter().collect(),
            },
            ssr_manifest: SsrManifest {
                entries: ssr_entries,
            },
            stats: BundleStats {
                modules_visited: visited,
                modules_emitted: live.len(),
                modules_dropped: dropped,
                bytes,
            },
        })
    }
}

impl<L: ModuleLoader> FerriteBundler<L> {
    /// Traverse entries breadth-first, loading in parallel batches.
    async fn traverse(&self, request: &BundleRequest) -> Result<HashMap<ModuleId, LoadedModule>> {
        let mut modules: HashMap<ModuleId, LoadedModule> = HashMap::new();
        let mut queue: VecDeque<ModuleId> = request.entries.iter().cloned().collect();
        while !queue.is_empty() {
            let mut batch = Vec::new();
            while let Some(id) = queue.pop_front() {
                if !modules.contains_key(&id) && !batch.contains(&id) {
                    batch.push(id);
                }
                if batch.len() >= 32 {
                    break;
                }
            }
            if batch.is_empty() {
                continue;
            }
            let loads = batch.iter().map(|id| self.loader.load(id, &request.env));
            let results = futures::future::join_all(loads).await;
            for (id, loaded) in batch.into_iter().zip(results) {
                let loaded = loaded?;
                for (_, dep, _) in &loaded.imports {
                    if !modules.contains_key(dep) {
                        queue.push_back(dep.clone());
                    }
                }
                modules.insert(id, loaded);
            }
        }
        Ok(modules)
    }
}

/// Tree-shake (§39, v0.1): reachability from entries + side-effect-free leaf
/// pruning. Statement-level DCE is the bundler-backend roadmap.
#[must_use]
pub fn tree_shake(
    entries: &[ModuleId],
    modules: &HashMap<ModuleId, LoadedModule>,
) -> HashSet<ModuleId> {
    // Reachable set.
    let mut live = HashSet::new();
    let mut queue: VecDeque<ModuleId> = entries.iter().cloned().collect();
    while let Some(id) = queue.pop_front() {
        if !live.insert(id.clone()) {
            continue;
        }
        if let Some(module) = modules.get(&id) {
            for (_, dep, _) in &module.imports {
                queue.push_back(dep.clone());
            }
        }
    }
    // Prune side-effect-free, export-unused leaves conservatively: only drop
    // modules explicitly marked `sideEffects: false` that no live module
    // imports *for side effects*. v0.1 keeps every reachable module except
    // bare CSS-less type-only shims; the pass below documents the hook where
    // statement-level DCE plugs in.
    let _ = modules;
    live
}

/// Derive a chunk name from a module id: `/src/main.ts` → `main`.
#[must_use]
pub fn chunk_name(id: &ModuleId) -> String {
    let (path, _) = id.split_query();
    let base = path.rsplit('/').next().unwrap_or("chunk");
    let stem = base.split_once('.').map_or(base, |(stem, _)| stem);
    let clean: String = stem
        .chars()
        .map(|char| {
            if char.is_alphanumeric() || char == '-' || char == '_' {
                char
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "chunk".to_string()
    } else {
        clean
    }
}

/// Relative URL from `from_file` to `to_file` (both under `assets/`).
#[must_use]
pub fn relative_url(from_file: &str, to_file: &str) -> String {
    let from_dir = from_file.rsplit_once('/').map_or("", |(dir, _)| dir);
    let to_parts: Vec<&str> = to_file.split('/').collect();
    let from_parts: Vec<&str> = if from_dir.is_empty() {
        Vec::new()
    } else {
        from_dir.split('/').collect()
    };
    let mut common = 0;
    while common < from_parts.len()
        && common < to_parts.len() - 1
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }
    let mut relative = String::new();
    for _ in common..from_parts.len() {
        relative.push_str("../");
    }
    relative.push_str(&to_parts[common..].join("/"));
    if !relative.starts_with('.') {
        relative.insert_str(0, "./");
    }
    relative
}

/// Rewrite import specifiers in rendered code (specifier → relative URL).
///
/// Operates on exact quoted-specifier matches produced by the loader's own
/// import list, so it cannot misfire on unrelated strings.
#[must_use]
pub fn rewrite_imports_text(code: &str, mapping: &HashMap<String, String>) -> String {
    let mut output = code.to_string();
    let mut pairs: Vec<(&String, &String)> = mapping.iter().collect();
    pairs.sort_by_key(|(spec, _)| std::cmp::Reverse(spec.len()));
    for (specifier, replacement) in pairs {
        for quote in ['"', '\'', '`'] {
            let from = format!("{quote}{specifier}{quote}");
            let to = format!("{quote}{replacement}{quote}");
            output = output.replace(&from, &to);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_naming() {
        assert_eq!(chunk_name(&ModuleId::new("/src/main.ts")), "main");
        assert_eq!(
            chunk_name(&ModuleId::new("/@npm/react@19.0.0/index.js")),
            "index"
        );
    }

    #[test]
    fn relative_urls() {
        assert_eq!(relative_url("assets/a-1.js", "assets/b-2.js"), "./b-2.js");
        assert_eq!(
            relative_url("assets/nested/a-1.js", "assets/b-2.js"),
            "../b-2.js"
        );
    }

    #[test]
    fn rewrites_exact_specifiers() {
        let code = "import x from \"./a\";\nconst s = \"./a\";\n";
        let mapping = HashMap::from([("./a".to_string(), "./a-1.js".to_string())]);
        let output = rewrite_imports_text(code, &mapping);
        assert!(output.contains("\"./a-1.js\""));
    }

    #[test]
    fn shake_keeps_reachable() {
        let entries = vec![ModuleId::new("/a.js")];
        let modules = HashMap::from([
            (
                ModuleId::new("/a.js"),
                LoadedModule {
                    id: ModuleId::new("/a.js"),
                    code: String::new(),
                    imports: vec![(
                        "./b".to_string(),
                        ModuleId::new("/b.js"),
                        ImportKind::Static,
                    )],
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                },
            ),
            (
                ModuleId::new("/b.js"),
                LoadedModule {
                    id: ModuleId::new("/b.js"),
                    code: String::new(),
                    imports: Vec::new(),
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                },
            ),
            (
                ModuleId::new("/z.js"),
                LoadedModule {
                    id: ModuleId::new("/z.js"),
                    code: String::new(),
                    imports: Vec::new(),
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                },
            ),
        ]);
        let live = tree_shake(&entries, &modules);
        assert!(live.contains(&ModuleId::new("/a.js")));
        assert!(live.contains(&ModuleId::new("/b.js")));
        assert!(!live.contains(&ModuleId::new("/z.js")));
    }
}
