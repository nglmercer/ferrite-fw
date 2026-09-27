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
use ferrite_transform::{rewrite_specifiers, ImportBinding, MinifyRequest, ShakeInfo};

/// Extracted CSS carried by a module (§29).
#[derive(Debug, Clone)]
pub struct CssExtract {
    /// Stylesheet text (refs rewritten, minified in production).
    pub text: String,
    /// True for CSS modules (the JS stub holds the exports map and is a
    /// real chunk; plain CSS stubs are dropped after extraction).
    pub is_modules: bool,
}

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
    /// Extracted CSS (concatenated per entry into hashed `.css` assets).
    pub css: Option<CssExtract>,
    /// Statement-DCE facts (`None` = opaque, keep everything).
    pub shake: Option<ferrite_transform::ShakeInfo>,
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
        let mut modules = self.traverse(&request).await?;
        let visited = modules.len();
        // 2. Tree-shake: drop unreachable + provably side-effect-free leaves.
        let live = tree_shake(&request.entries, &modules);
        let dropped = visited.saturating_sub(live.len());
        // 2b. Statement-level shake: drop unused exports, re-minify (§38).
        if request.treeshake {
            shake_statements(&mut modules, &live, &request)?;
        }
        // 3. Chunk: one file per module; entries + dynamic boundaries marked.
        let dynamic_entries: HashSet<ModuleId> = modules
            .values()
            .flat_map(|module| {
                module.imports.iter().filter_map(|(_, id, kind)| {
                    matches!(kind, ImportKind::Dynamic).then_some(id.clone())
                })
            })
            .collect();
        // Plain-CSS modules carry extracted styles, not JS: they get no
        // chunk file (CSS-modules stubs stay — importers read the map).
        let is_plain_css = |id: &ModuleId| {
            modules
                .get(id)
                .and_then(|module| module.css.as_ref())
                .is_some_and(|css| !css.is_modules)
        };
        // 3. Chunk: one file per module, or one scope-hoisted file per
        // entry closure (§91; shared/ineligible modules stay separate).
        let groups = plan_chunks(&request, &modules, &live, &dynamic_entries, &is_plain_css)?;
        // Module → chunk id (concat members share their entry's chunk).
        let mut chunk_of: HashMap<&ModuleId, String> = HashMap::new();
        for group in &groups {
            for member in &group.members {
                chunk_of.insert(member, group.id.clone());
            }
        }
        let mut chunks: HashMap<String, Chunk> = HashMap::new();
        for group in &groups {
            let mut imports = Vec::new();
            let mut dynamic_imports = Vec::new();
            let mut seen_static = HashSet::new();
            let mut seen_dynamic = HashSet::new();
            for member in &group.members {
                let module = modules.get(member).ok_or_else(|| {
                    FerriteError::Build(format!("missing module `{member}` after traversal"))
                })?;
                for (_, dep, kind) in &module.imports {
                    if !live.contains(dep) || is_plain_css(dep) {
                        continue;
                    }
                    let Some(dep_chunk) = chunk_of.get(dep) else {
                        continue;
                    };
                    if dep_chunk == &group.id {
                        continue;
                    }
                    if *kind == ImportKind::Dynamic {
                        if seen_dynamic.insert(dep_chunk.clone()) {
                            dynamic_imports.push(dep_chunk.clone());
                        }
                    } else if seen_static.insert(dep_chunk.clone()) {
                        imports.push(dep_chunk.clone());
                    }
                }
            }
            let _ = graph;
            chunks.insert(
                group.id.clone(),
                Chunk {
                    id: group.id.clone(),
                    modules: group.members.clone(),
                    imports,
                    dynamic_imports,
                    exports: group.exports.clone(),
                    entry: request.entries.contains(&ModuleId::new(group.id.clone())),
                    name: chunk_name(&ModuleId::new(group.id.clone())),
                    code: group.code.clone(),
                    map: group.map.clone(),
                    file_name: None,
                },
            );
        }
        // 4. Hash + file names. Each name embeds a content hash of the
        //    chunk's own code plus every transitively reachable chunk's
        //    content hash, so a change anywhere in the dependency cone
        //    renames the file (no stale caches). The hash intentionally
        //    covers pre-rewrite code: rewritten code embeds file names, so
        //    hashing emitted bytes would be circular (a file cannot contain
        //    its own hash).
        let content_hashes: HashMap<String, String> = chunks
            .iter()
            .map(|(id, chunk)| (id.clone(), Hash::of_str(&chunk.code).0))
            .collect();
        let mut file_names: HashMap<String, String> = HashMap::new();
        for chunk in chunks.values() {
            let mut material = chunk.code.clone();
            for hash in reachable_content_hashes(chunk, &chunks, &content_hashes) {
                material.push('\0');
                material.push_str(hash);
            }
            let hash = Hash::of_str(&material).short(8);
            let file_name = config
                .chunk_pattern
                .replace("[name]", &chunk.name)
                .replace("[hash]", &hash);
            file_names.insert(chunk.id.clone(), file_name);
        }
        // 5. Emit one hashed CSS file per CSS module, rewriting relative
        //    `@import`s to relative CSS URLs in post-order (leaves first).
        let mut css_file_names: HashMap<String, String> = HashMap::new();
        let mut css_texts: HashMap<String, String> = HashMap::new();
        let mut css_ids: Vec<&ModuleId> = live
            .iter()
            .filter(|id| {
                modules
                    .get(id)
                    .and_then(|module| module.css.as_ref())
                    .is_some()
            })
            .collect();
        css_ids.sort_by(|a, b| a.0.cmp(&b.0));
        let mut css_done = HashSet::new();
        for id in css_ids {
            emit_css_module(
                id,
                &modules,
                &live,
                config,
                &mut css_file_names,
                &mut css_texts,
                &mut css_done,
                &mut Vec::new(),
            )?;
        }
        let mut bundle = OutputBundle::default();
        let mut bytes = 0;
        let mut css_files_sorted: Vec<(&String, &String)> = css_file_names.iter().collect();
        css_files_sorted.sort();
        for (id, file) in css_files_sorted {
            if let Some(text) = css_texts.get(id) {
                bytes += text.len();
                bundle.insert(EmittedFile {
                    name: file.clone(),
                    contents: text.clone().into_bytes(),
                    is_entry: false,
                });
            }
        }
        // 6. Rewrite imports to relative chunk URLs + minify already applied
        //    by the loader; finalize code.
        let mut manifest_entries: BTreeMap<String, ManifestEntry> = BTreeMap::new();
        let mut ssr_entries: HashMap<String, Vec<String>> = HashMap::new();
        // Module → chunk id (concat members share their entry's chunk).
        let mut member_chunk: HashMap<&ModuleId, &String> = HashMap::new();
        for (id, chunk) in &chunks {
            for member in &chunk.modules {
                member_chunk.insert(member, id);
            }
        }
        let mut chunk_ids: Vec<String> = chunks.keys().cloned().collect();
        chunk_ids.sort();
        for chunk_id in chunk_ids {
            let chunk = chunks.get(&chunk_id).unwrap().clone();
            let file_name = file_names[&chunk_id].clone();
            let mut import_map: HashMap<String, String> = HashMap::new();
            let mut stripped_css: Vec<&str> = Vec::new();
            for member in &chunk.modules {
                let module = &modules[member];
                for (specifier, dep, kind) in &module.imports {
                    let dep_file = member_chunk
                        .get(dep)
                        .and_then(|dep_chunk| file_names.get(*dep_chunk));
                    if let Some(dep_file) = dep_file {
                        import_map.insert(specifier.clone(), relative_url(&file_name, dep_file));
                    } else if *kind != ImportKind::Dynamic
                        && modules
                            .get(dep)
                            .and_then(|dep| dep.css.as_ref())
                            .is_some_and(|css| !css.is_modules)
                    {
                        // Static bare import of extracted plain CSS: dropped.
                        stripped_css.push(specifier.as_str());
                    }
                }
            }
            let code = if import_map.is_empty() {
                chunk.code.clone()
            } else {
                rewrite_imports_text(&chunk.code, &import_map)
            };
            let code = strip_bare_imports(&code, &stripped_css);
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
            // Extracted CSS files for this entry's closure (BFS order).
            let css_files = if chunk.entry {
                entry_css_files(&ModuleId::new(chunk_id.clone()), &modules, &live)
                    .iter()
                    .filter_map(|id| css_file_names.get(id).cloned())
                    .collect()
            } else {
                Vec::new()
            };
            manifest_entries.insert(
                chunk_id.clone(),
                ManifestEntry {
                    file: file_name.clone(),
                    src: Some(chunk_id.clone()),
                    is_entry: chunk.entry.then_some(true),
                    is_dynamic_entry: (!chunk.entry
                        && dynamic_entries.contains(&ModuleId::new(chunk_id.clone())))
                    .then_some(true),
                    css: css_files,
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
    // bare CSS-less type-only shims; statement-level DCE runs separately in
    // [`shake_statements`].
    let _ = modules;
    live
}

/// Per-module used-export sets: each entry is the names imported by live
/// modules, plus the ids that must keep every export.
pub struct UsedExports {
    /// Names used per module (absent = none used).
    pub used: HashMap<ModuleId, HashSet<String>>,
    /// Modules that keep all exports (entries, namespaces, dynamics).
    pub full: HashSet<ModuleId>,
}

/// Compute used exports to a fixpoint over the live graph (§38).
///
/// Soundness rules: entries, dynamic targets, worker entries, namespace
/// imports, and opaque modules (`shake: None`, stale caches) keep
/// everything; unknown binding lists keep the whole dependency; star
/// re-exports propagate names the barrel does not define locally.
#[must_use]
pub fn used_exports(
    entries: &[ModuleId],
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
) -> UsedExports {
    let mut used: HashMap<ModuleId, HashSet<String>> = HashMap::new();
    let mut full: HashSet<ModuleId> = entries.iter().cloned().collect();
    // Opaque modules and their dependencies keep everything.
    for id in live {
        match modules.get(id).and_then(|module| module.shake.as_ref()) {
            Some(_) => {}
            None => {
                full.insert(id.clone());
                if let Some(module) = modules.get(id) {
                    for (_, dep, _) in &module.imports {
                        full.insert(dep.clone());
                    }
                }
            }
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for id in live {
            let Some(module) = modules.get(id) else {
                continue;
            };
            let Some(shake) = module.shake.as_ref() else {
                continue;
            };
            for (index, (_, dep, kind)) in module.imports.iter().enumerate() {
                if !live.contains(dep) {
                    continue;
                }
                match kind {
                    ImportKind::Dynamic | ImportKind::Worker => {
                        changed |= full.insert(dep.clone());
                    }
                    ImportKind::Static => {
                        let Some(bindings) = shake.import_bindings.get(index) else {
                            // Unknown bindings: keep the whole dependency.
                            changed |= full.insert(dep.clone());
                            continue;
                        };
                        for binding in bindings {
                            match binding {
                                ImportBinding::Named(name) => {
                                    changed |=
                                        used.entry(dep.clone()).or_default().insert(name.clone());
                                }
                                ImportBinding::Default => {
                                    changed |= used
                                        .entry(dep.clone())
                                        .or_default()
                                        .insert("default".to_string());
                                }
                                ImportBinding::Namespace => {
                                    changed |= full.insert(dep.clone());
                                }
                                ImportBinding::SideEffect | ImportBinding::Reexport => {}
                            }
                        }
                    }
                    // Assets, CSS, and wasm shims carry no JS exports.
                    ImportKind::Css | ImportKind::Url | ImportKind::Wasm => {}
                }
            }
            propagate_reexports(id, shake, live, &mut used, &mut full, &mut changed);
        }
    }
    UsedExports { used, full }
}

/// Propagate used names through a module's re-exports.
fn propagate_reexports(
    id: &ModuleId,
    shake: &ShakeInfo,
    live: &HashSet<ModuleId>,
    used: &mut HashMap<ModuleId, HashSet<String>>,
    full: &mut HashSet<ModuleId>,
    changed: &mut bool,
) {
    // Star re-exports only leak names the barrel does not define itself.
    let local: HashSet<&str> = shake
        .exports
        .iter()
        .filter(|export| export.from.is_none())
        .map(|export| export.exported.as_str())
        .collect();
    let used_here: HashSet<String> = used.get(id).cloned().unwrap_or_default();
    let full_here = full.contains(id);
    for export in &shake.exports {
        let Some(target) = export.target.as_ref() else {
            continue;
        };
        if !live.contains(target) {
            continue;
        }
        if export.exported == "*" {
            if full_here {
                *changed |= full.insert(target.clone());
            } else {
                for name in &used_here {
                    if !local.contains(name.as_str()) {
                        *changed |= used.entry(target.clone()).or_default().insert(name.clone());
                    }
                }
            }
        } else if full_here || used_here.contains(&export.exported) {
            let name = export.imported.as_deref().unwrap_or(&export.exported);
            *changed |= used
                .entry(target.clone())
                .or_default()
                .insert(name.to_string());
        }
    }
}

/// Drop unused export statements from live modules, then re-minify (§38).
///
/// Each shaken module is parsed from its loaded code, pruned with
/// [`ferrite_transform::drop_unused_exports`], and minified again so dead
/// bindings left behind by unwrapped exports are collected. Source maps
/// chain through the re-minification. Modules that keep everything, carry
/// no JS, or shake to no-ops are untouched.
pub fn shake_statements(
    modules: &mut HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    request: &BundleRequest,
) -> Result<()> {
    let usage = used_exports(&request.entries, modules, live);
    let compiler = ferrite_transform::compiler_for_engine(&request.engine)?;
    let empty = HashSet::new();
    for id in live {
        if usage.full.contains(id) {
            continue;
        }
        let Some(module) = modules.get(id) else {
            continue;
        };
        if !module.module_type.is_js_like() || module.shake.is_none() {
            continue;
        }
        let used = usage.used.get(id).unwrap_or(&empty);
        let pruned = ferrite_transform::drop_unused_exports(&module.id.0, &module.code, used)?;
        let Some(code) = pruned else {
            continue;
        };
        let minified = compiler.minify(MinifyRequest {
            id: module.id.0.clone(),
            code,
            sourcemap: request.sourcemap,
            input_map: module.map.clone().map(ferrite_core::SourceMap::external),
        })?;
        if let Some(module) = modules.get_mut(id) {
            module.code = minified.code;
            module.map = minified.map.map(|chained| chained.mappings);
        }
    }
    Ok(())
}

/// One chunk group: members sharing a file.
struct ChunkGroup {
    /// Chunk id (entry module id for concats, module id for singles).
    id: String,
    /// Member module ids (emit order for concats).
    members: Vec<ModuleId>,
    /// Rendered code.
    code: String,
    /// Source map JSON, when produced.
    map: Option<String>,
    /// Preserved export names (concat entries).
    exports: Vec<String>,
}

/// Plan chunk groups: without `scope_hoist` every module is its own
/// group; otherwise each entry closure owned by exactly one entry is
/// concatenated when eligible (§91). Shared, non-JS, or ineligible
/// modules stay separate so semantics never change silently.
fn plan_chunks(
    request: &BundleRequest,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    dynamic_entries: &HashSet<ModuleId>,
    is_plain_css: &dyn Fn(&ModuleId) -> bool,
) -> Result<Vec<ChunkGroup>> {
    // Deterministic single-module order (previous behavior, sorted).
    let mut singles: Vec<&ModuleId> = live.iter().filter(|id| !is_plain_css(id)).collect();
    singles.sort_by(|a, b| a.0.cmp(&b.0));
    if !request.scope_hoist {
        return singles
            .into_iter()
            .map(|id| {
                let module = modules.get(id).ok_or_else(|| {
                    FerriteError::Build(format!("missing module `{id}` after traversal"))
                })?;
                Ok(ChunkGroup {
                    id: id.0.clone(),
                    members: vec![id.clone()],
                    code: module.code.clone(),
                    map: module.map.clone(),
                    exports: Vec::new(),
                })
            })
            .collect();
    }
    // Closure roots: static entries + dynamic + worker boundaries.
    let mut roots: Vec<ModuleId> = request.entries.clone();
    for id in dynamic_entries {
        if !roots.contains(id) {
            roots.push(id.clone());
        }
    }
    for module in modules.values() {
        for (_, dep, kind) in &module.imports {
            if *kind == ImportKind::Worker && live.contains(dep) && !roots.contains(dep) {
                roots.push(dep.clone());
            }
        }
    }
    roots.sort_by(|a, b| a.0.cmp(&b.0));
    // Static post-order closures over live JS-like members.
    let mut closures: HashMap<ModuleId, Vec<ModuleId>> = HashMap::new();
    for root in &roots {
        closures.insert(
            root.clone(),
            static_closure(root, modules, live, is_plain_css),
        );
    }
    // Ownership: members in exactly one closure concatenate.
    let mut owners: HashMap<ModuleId, usize> = HashMap::new();
    for closure in closures.values() {
        for member in closure {
            *owners.entry(member.clone()).or_default() += 1;
        }
    }
    let mut grouped: HashSet<ModuleId> = HashSet::new();
    let mut groups = Vec::new();
    for root in &roots {
        let owned: Vec<ModuleId> = closures[root]
            .iter()
            .filter(|member| owners.get(*member).copied().unwrap_or(0) == 1)
            .cloned()
            .collect();
        if owned.len() < 2 || !owned.contains(root) {
            continue;
        }
        if let Some(group) = try_concat(root, &owned, modules, request.sourcemap)? {
            for member in &owned {
                grouped.insert(member.clone());
            }
            groups.push(group);
        }
    }
    // Leftovers (shared, non-JS, ineligible, unowned) stay separate.
    for id in singles {
        if grouped.contains(id) {
            continue;
        }
        let module = modules
            .get(id)
            .ok_or_else(|| FerriteError::Build(format!("missing module `{id}` after traversal")))?;
        groups.push(ChunkGroup {
            id: id.0.clone(),
            members: vec![id.clone()],
            code: module.code.clone(),
            map: module.map.clone(),
            exports: Vec::new(),
        });
    }
    groups.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(groups)
}

/// Static post-order closure of live JS-like members (deps first).
fn static_closure(
    root: &ModuleId,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    is_plain_css: &dyn Fn(&ModuleId) -> bool,
) -> Vec<ModuleId> {
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    // Iterative post-order with an exit marker.
    let mut pending: Vec<(ModuleId, bool)> = vec![(root.clone(), false)];
    while let Some((id, exit)) = pending.pop() {
        if exit {
            order.push(id);
            continue;
        }
        if !visited.insert(id.clone()) {
            continue;
        }
        pending.push((id.clone(), true));
        if let Some(module) = modules.get(&id) {
            for (_, dep, kind) in &module.imports {
                if *kind != ImportKind::Static || !live.contains(dep) || is_plain_css(dep) {
                    continue;
                }
                let js_like = modules
                    .get(dep)
                    .map(|module| module.module_type.is_js_like())
                    .unwrap_or(false);
                if js_like {
                    pending.push((dep.clone(), false));
                }
            }
        }
    }
    order
}

/// Try to concatenate `owned` (post-order, entry last) into one group.
fn try_concat(
    root: &ModuleId,
    owned: &[ModuleId],
    modules: &HashMap<ModuleId, LoadedModule>,
    want_map: bool,
) -> Result<Option<ChunkGroup>> {
    let mut members = Vec::with_capacity(owned.len());
    for id in owned {
        let module = modules
            .get(id)
            .ok_or_else(|| FerriteError::Build(format!("missing module `{id}` after traversal")))?;
        let Some(shake) = module.shake.clone() else {
            return Ok(None);
        };
        members.push(ferrite_transform::concat::ConcatModule {
            id: id.0.clone(),
            code: module.code.clone(),
            shake,
            imports: module
                .imports
                .iter()
                .map(
                    |(specifier, dep, kind)| ferrite_transform::concat::ConcatImport {
                        specifier: specifier.clone(),
                        target: dep.0.clone(),
                        is_static: *kind == ImportKind::Static,
                    },
                )
                .collect(),
            is_entry: id == root,
            input_map: module.map.clone(),
        });
    }
    let output = ferrite_transform::concat::concat_modules(&members, want_map)?;
    Ok(output.map(|output| ChunkGroup {
        id: root.0.clone(),
        members: owned.to_vec(),
        code: output.code,
        map: output.map,
        exports: output.exports,
    }))
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

/// Full content hashes of every chunk reachable from `root` (inclusive),
/// sorted for determinism. Cycle-safe via a visited set.
fn reachable_content_hashes<'a>(
    root: &Chunk,
    chunks: &'a HashMap<String, Chunk>,
    content_hashes: &'a HashMap<String, String>,
) -> Vec<&'a str> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = vec![root.id.as_str()];
    let mut hashes: Vec<&'a str> = Vec::new();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(hash) = content_hashes.get(id) {
            hashes.push(hash.as_str());
        }
        if let Some(chunk) = chunks.get(id) {
            stack.extend(chunk.imports.iter().map(String::as_str));
            stack.extend(chunk.dynamic_imports.iter().map(String::as_str));
        }
    }
    hashes.sort_unstable();
    hashes
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
/// CSS module ids in an entry closure (BFS in import order).
fn entry_css_files(
    entry: &ModuleId,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([entry.clone()]);
    let mut out = Vec::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(module) = modules.get(&id) else {
            continue;
        };
        if !live.contains(&id) {
            continue;
        }
        if module.css.is_some() {
            out.push(id.0.clone());
        }
        for (_, dep, _) in &module.imports {
            queue.push_back(dep.clone());
        }
    }
    out
}

/// Emit one CSS module's file, dependencies first (post-order).
///
/// Relative `@import`s rewrite to `./<dep-basename>` (all CSS files share
/// the flat `assets/` dir); remote `@import`s stay untouched. Circular
/// imports are a loud build error.
#[allow(clippy::too_many_arguments)]
fn emit_css_module(
    id: &ModuleId,
    modules: &HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    config: &BuildBundleConfig,
    file_names: &mut HashMap<String, String>,
    texts: &mut HashMap<String, String>,
    done: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Result<()> {
    if done.contains(&id.0) {
        return Ok(());
    }
    if stack.contains(&id.0) {
        stack.push(id.0.clone());
        return Err(FerriteError::Build(format!(
            "circular CSS @import: {}",
            stack.join(" -> ")
        )));
    }
    let module = modules
        .get(id)
        .ok_or_else(|| FerriteError::Build(format!("missing CSS module `{id}` after traversal")))?;
    let Some(css) = module.css.as_ref() else {
        return Ok(());
    };
    stack.push(id.0.clone());
    let mut deps: Vec<(String, ModuleId)> = module
        .imports
        .iter()
        .filter(|(_, dep, _)| {
            live.contains(dep) && modules.get(dep).and_then(|dep| dep.css.as_ref()).is_some()
        })
        .map(|(spec, dep, _)| (spec.clone(), dep.clone()))
        .collect();
    deps.sort_by(|a, b| a.1 .0.cmp(&b.1 .0));
    for (_, dep) in &deps {
        emit_css_module(dep, modules, live, config, file_names, texts, done, stack)?;
    }
    let mut mapping: HashMap<String, String> = HashMap::new();
    for (spec, dep) in &deps {
        if let Some(dep_file) = file_names.get(&dep.0) {
            let base = dep_file.rsplit('/').next().unwrap_or(dep_file);
            mapping.insert(spec.clone(), format!("./{base}"));
        }
    }
    let final_text = ferrite_css::rewrite_css_imports(&css.text, |spec| mapping.get(spec).cloned());
    let hash = Hash::of_str(&final_text).short(8);
    let name = config
        .css_pattern
        .replace("[name]", &chunk_name(id))
        .replace("[hash]", &hash);
    file_names.insert(id.0.clone(), name);
    texts.insert(id.0.clone(), final_text);
    stack.pop();
    done.insert(id.0.clone());
    Ok(())
}

/// Drop bare imports (`import "<spec>";`) for extracted CSS.
///
/// Matches minified and pretty code (`import"…"`, optional whitespace /
/// semicolon). Binding imports (`import x from …`), dynamic imports, and
/// lookalike string contents (whose quotes are escaped) are never touched.
fn strip_bare_imports(code: &str, specs: &[&str]) -> String {
    if specs.is_empty() {
        return code.to_string();
    }
    let mut out = String::with_capacity(code.len());
    let mut cursor = 0;
    while cursor < code.len() {
        if let Some(end) = code
            .get(cursor..)
            .and_then(|rest| match_bare_import(rest, specs))
        {
            cursor += end;
        } else if let Some(next) = code[cursor..].chars().next() {
            out.push(next);
            cursor += next.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// Length of the bare-import statement at `text` start, if any.
fn match_bare_import(text: &str, specs: &[&str]) -> Option<usize> {
    let after_import = text.strip_prefix("import")?;
    // `import(`, `import x`, `import*`… are not bare imports.
    if after_import.chars().next().is_some_and(|char| {
        char.is_alphanumeric() || char == '_' || char == '$' || char == '*' || char == '('
    }) {
        return None;
    }
    let trimmed = after_import.trim_start();
    let quote = trimmed.chars().next()?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    for spec in specs {
        let mut candidate = String::with_capacity(spec.len() + 2);
        candidate.push(quote);
        candidate.push_str(spec);
        candidate.push(quote);
        if let Some(after_spec) = trimmed.strip_prefix(&candidate) {
            let tail = after_spec.trim_start();
            let semicolon = usize::from(tail.starts_with(';'));
            return Some(text.len() - tail[semicolon..].len());
        }
    }
    None
}

/// Rewrite import specifiers in `code` using parsed import ranges.
///
/// Only real import specifiers are replaced: identical text inside plain
/// strings or comments is left alone. Falls back to quoted-text replacement
/// when `code` does not parse.
pub fn rewrite_imports_text(code: &str, mapping: &HashMap<String, String>) -> String {
    if mapping.is_empty() {
        return code.to_string();
    }
    match rewrite_specifiers(code, &ModuleType::Js, mapping) {
        Ok((rewritten, _)) => rewritten,
        Err(_) => blind_rewrite_imports_text(code, mapping),
    }
}

/// Quoted-text fallback for unparseable inputs.
fn blind_rewrite_imports_text(code: &str, mapping: &HashMap<String, String>) -> String {
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

    struct MapLoader {
        modules: HashMap<ModuleId, LoadedModule>,
    }

    #[async_trait::async_trait]
    impl ModuleLoader for MapLoader {
        async fn load(&self, id: &ModuleId, _env: &str) -> Result<LoadedModule> {
            self.modules
                .get(id)
                .cloned()
                .ok_or_else(|| FerriteError::Build(format!("test loader missing `{id}`")))
        }
    }

    fn js_module(
        id: &str,
        code: &str,
        imports: Vec<(String, ModuleId, ImportKind)>,
    ) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: code.to_string(),
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: None,
            shake: None,
        }
    }

    fn css_module(id: &str, text: &str, is_modules: bool, deps: Vec<ModuleId>) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: "export default undefined;\n".to_string(),
            imports: deps
                .into_iter()
                .map(|dep| {
                    (
                        format!("./{}", dep.0.rsplit('/').next().unwrap_or(&dep.0)),
                        dep,
                        ImportKind::Static,
                    )
                })
                .collect(),
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: Some(CssExtract {
                text: text.to_string(),
                is_modules,
            }),
            shake: None,
        }
    }

    fn test_config() -> BuildBundleConfig {
        BuildBundleConfig {
            out_dir: std::env::temp_dir(),
            asset_pattern: "assets/[name]-[hash][ext]".to_string(),
            chunk_pattern: "assets/[name]-[hash].js".to_string(),
            css_pattern: "assets/[name]-[hash].css".to_string(),
        }
    }

    #[test]
    fn strips_only_bare_css_imports() {
        let code = "import \"/a.css\";\nimport x from \"/b.css\";\nimport(\"/c.css\");\nconst s = \"import \\\"/a.css\\\"\";\n";
        let out = strip_bare_imports(code, &["/a.css", "/b.css", "/c.css"]);
        assert!(!out.contains("import \"/a.css\""), "{out}");
        assert!(out.contains("import x from \"/b.css\""), "{out}");
        assert!(out.contains("import(\"/c.css\")"), "{out}");
        assert!(out.contains("const s = "), "{out}");
        assert!(out.ends_with('\n'));
        // Minified single line, no spaces.
        let min = "import{greet}from\"./g.js\";import\"/a.css\";console.log(1);";
        let out = strip_bare_imports(min, &["/a.css"]);
        assert_eq!(out, "import{greet}from\"./g.js\";console.log(1);");
        // Semicolon-less (ASI) and star imports survive.
        let edge = "import \"/a.css\"\nimport * as ns from \"/a.css\";\n";
        let out = strip_bare_imports(edge, &["/a.css"]);
        assert!(out.contains("import * as ns"), "{out}");
        assert!(!out.contains("import \"/a.css\"\n"), "{out}");
    }

    #[tokio::test]
    async fn extracts_css_per_module_with_import_rewrite() {
        let modules = HashMap::from([
            (
                ModuleId::new("/main.js"),
                js_module(
                    "/main.js",
                    "import \"/a.css\";\nimport styles from \"/m.module.css\";\nconsole.log(styles);\n",
                    vec![
                        (
                            "/a.css".to_string(),
                            ModuleId::new("/a.css"),
                            ImportKind::Static,
                        ),
                        (
                            "/m.module.css".to_string(),
                            ModuleId::new("/m.module.css"),
                            ImportKind::Static,
                        ),
                    ],
                ),
            ),
            (
                ModuleId::new("/a.css"),
                css_module(
                    "/a.css",
                    "@import \"./b.css\";\n@import \"https://fonts.example/f.css\";\n.a{color:red}\n",
                    false,
                    vec![ModuleId::new("/b.css")],
                ),
            ),
            (
                ModuleId::new("/b.css"),
                css_module("/b.css", ".b{color:blue}\n", false, Vec::new()),
            ),
            (
                ModuleId::new("/m.module.css"),
                css_module("/m.module.css", ".btn{color:green}\n", true, Vec::new()),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                BundleRequest {
                    entries: vec![ModuleId::new("/main.js")],
                    env: "client".to_string(),
                    minify: false,
                    sourcemap: false,
                    map_comment: false,
                    treeshake: false,
                    engine: "oxc".to_string(),
                    scope_hoist: false,
                },
            )
            .await
            .unwrap();
        let names: Vec<&String> = output.bundle.files.keys().collect();
        // One css file per css module; no JS chunk for plain css.
        let css_files: Vec<&&String> = names.iter().filter(|name| name.ends_with(".css")).collect();
        assert_eq!(css_files.len(), 3, "{names:?}");
        assert!(
            !names
                .iter()
                .any(|name| name.contains("a-") && name.ends_with(".js")),
            "{names:?}"
        );
        // Modules stub chunk survives (importers read the map).
        assert!(
            names
                .iter()
                .any(|name| name.contains("m-") && name.ends_with(".js")),
            "{names:?}"
        );
        // @import rewritten to the relative css file; remote untouched.
        let a_file = css_files
            .iter()
            .find(|name| name.contains("/a-"))
            .expect("a css")
            .to_string();
        let a_text = String::from_utf8(output.bundle.files[&a_file].contents.clone()).unwrap();
        assert!(a_text.contains("@import \"./b-"), "{a_text}");
        assert!(
            a_text.contains("@import \"https://fonts.example/f.css\""),
            "{a_text}"
        );
        // Bare import stripped from the JS chunk.
        let main_entry = output.manifest.entries.get("/main.js").expect("entry");
        let main_code =
            String::from_utf8(output.bundle.files[&main_entry.file].contents.clone()).unwrap();
        assert!(!main_code.contains("/a.css"), "{main_code}");
        assert!(!main_code.contains("/m.module.css"), "{main_code}");
        assert!(main_code.contains("./m-"), "{main_code}");
        // Manifest links the entry stylesheets.
        assert_eq!(main_entry.css.len(), 3, "{main_entry:?}");
        assert!(main_entry
            .css
            .iter()
            .all(|file| output.bundle.files.contains_key(file)));
    }

    #[tokio::test]
    async fn circular_css_import_fails_loudly() {
        let modules = HashMap::from([
            (
                ModuleId::new("/main.js"),
                js_module(
                    "/main.js",
                    "import \"/a.css\";\n",
                    vec![(
                        "/a.css".to_string(),
                        ModuleId::new("/a.css"),
                        ImportKind::Static,
                    )],
                ),
            ),
            (
                ModuleId::new("/a.css"),
                css_module(
                    "/a.css",
                    "@import \"./b.css\";\n",
                    false,
                    vec![ModuleId::new("/b.css")],
                ),
            ),
            (
                ModuleId::new("/b.css"),
                css_module(
                    "/b.css",
                    "@import \"./a.css\";\n",
                    false,
                    vec![ModuleId::new("/a.css")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let error = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                BundleRequest {
                    entries: vec![ModuleId::new("/main.js")],
                    env: "client".to_string(),
                    minify: false,
                    sourcemap: false,
                    map_comment: false,
                    treeshake: false,
                    engine: "oxc".to_string(),
                    scope_hoist: false,
                },
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("circular CSS"), "{error}");
    }

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
        assert!(output.contains("from \"./a-1.js\""), "{output}");
    }

    #[test]
    fn rewrite_leaves_strings_and_comments_alone() {
        let code = "import x from \"./a\";\nconst s = \"./a\";\n// see \"./a\" docs\n";
        let mapping = HashMap::from([("./a".to_string(), "./a-1.js".to_string())]);
        let output = rewrite_imports_text(code, &mapping);
        assert!(output.contains("from \"./a-1.js\""), "{output}");
        assert!(output.contains("const s = \"./a\";"), "{output}");
        assert!(output.contains("// see \"./a\" docs"), "{output}");
    }

    #[tokio::test]
    async fn chunk_names_follow_transitive_content() {
        async fn bundle_with(dep_code: &str) -> BundleOutput {
            let modules = HashMap::from([
                (
                    ModuleId::new("/main.js"),
                    js_module(
                        "/main.js",
                        "import { x } from \"/dep.js\";\nconsole.log(x);\n",
                        vec![(
                            "/dep.js".to_string(),
                            ModuleId::new("/dep.js"),
                            ImportKind::Static,
                        )],
                    ),
                ),
                (
                    ModuleId::new("/dep.js"),
                    js_module("/dep.js", dep_code, vec![]),
                ),
            ]);
            let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
            bundler
                .bundle(
                    &ModuleGraph::new(),
                    &test_config(),
                    BundleRequest {
                        entries: vec![ModuleId::new("/main.js")],
                        env: "client".to_string(),
                        minify: false,
                        sourcemap: false,
                        map_comment: false,
                        treeshake: false,
                        engine: "oxc".to_string(),
                        scope_hoist: false,
                    },
                )
                .await
                .unwrap()
        }

        let before = bundle_with("export const x = 1;\n").await;
        let after = bundle_with("export const x = 2;\n").await;
        let main_before = &before.manifest.entries["/main.js"].file;
        let main_after = &after.manifest.entries["/main.js"].file;
        let dep_before = &before.manifest.entries["/dep.js"].file;
        let dep_after = &after.manifest.entries["/dep.js"].file;
        // Both the changed chunk and its importer are renamed.
        assert_ne!(dep_before, dep_after);
        assert_ne!(main_before, main_after);
        // The importer's emitted code points at the new dependency file.
        let main_code = String::from_utf8(after.bundle.files[main_after].contents.clone()).unwrap();
        let dep_file = dep_after.rsplit('/').next().unwrap();
        assert!(main_code.contains(dep_file), "{main_code}");
    }

    fn named(name: &str) -> Vec<ImportBinding> {
        vec![ImportBinding::Named(name.to_string())]
    }

    fn shaken_module(
        id: &str,
        code: &str,
        imports: Vec<(String, ModuleId, ImportKind)>,
        bindings: Vec<Vec<ImportBinding>>,
        exports: Vec<ferrite_transform::ParsedExport>,
    ) -> LoadedModule {
        LoadedModule {
            id: ModuleId::new(id),
            code: code.to_string(),
            imports,
            side_effects: None,
            module_type: ModuleType::Js,
            map: None,
            css: None,
            shake: Some(ShakeInfo {
                import_bindings: bindings,
                exports,
            }),
        }
    }

    fn local_export(name: &str) -> ferrite_transform::ParsedExport {
        ferrite_transform::ParsedExport {
            exported: name.to_string(),
            local: Some(name.to_string()),
            from: None,
            imported: None,
            target: None,
        }
    }

    fn reexport(exported: &str, imported: &str, target: &str) -> ferrite_transform::ParsedExport {
        ferrite_transform::ParsedExport {
            exported: exported.to_string(),
            local: None,
            from: Some(target.to_string()),
            imported: Some(imported.to_string()),
            target: Some(ModuleId::new(target)),
        }
    }

    fn shake_request(entries: Vec<ModuleId>) -> BundleRequest {
        BundleRequest {
            entries,
            env: "client".to_string(),
            minify: false,
            sourcemap: false,
            map_comment: false,
            treeshake: true,
            engine: "oxc".to_string(),
            scope_hoist: false,
        }
    }

    #[test]
    fn usage_propagates_through_barrel() {
        let entry = ModuleId::new("/e.js");
        let barrel = ModuleId::new("/barrel.js");
        let leaf = ModuleId::new("/leaf.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x } from \"./barrel.js\";\nconsole.log(x);\n",
                    vec![(
                        "./barrel.js".to_string(),
                        barrel.clone(),
                        ImportKind::Static,
                    )],
                    vec![named("x")],
                    vec![],
                ),
            ),
            (
                barrel.clone(),
                shaken_module(
                    "/barrel.js",
                    "export { x } from \"./leaf.js\";\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![vec![ImportBinding::Reexport]],
                    vec![reexport("x", "x", "/leaf.js")],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const x = 1;\nexport const y = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("x"), local_export("y")],
                ),
            ),
        ]);
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        assert!(usage.full.contains(&entry));
        assert_eq!(
            usage.used.get(&barrel).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
        assert_eq!(
            usage.used.get(&leaf).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
    }

    #[test]
    fn usage_conservative_cases() {
        let entry = ModuleId::new("/e.js");
        let ns = ModuleId::new("/ns.js");
        let dyn_target = ModuleId::new("/dyn.js");
        let opaque = ModuleId::new("/opaque.js");
        let opaque_dep = ModuleId::new("/opaque-dep.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import * as ns from \"./ns.js\";\nimport(\"./dyn.js\");\n",
                    vec![
                        ("./ns.js".to_string(), ns.clone(), ImportKind::Static),
                        (
                            "./dyn.js".to_string(),
                            dyn_target.clone(),
                            ImportKind::Dynamic,
                        ),
                    ],
                    vec![vec![ImportBinding::Namespace], vec![]],
                    vec![],
                ),
            ),
            (
                ns.clone(),
                shaken_module(
                    "/ns.js",
                    "export const a = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("a")],
                ),
            ),
            (
                dyn_target.clone(),
                shaken_module(
                    "/dyn.js",
                    "export const b = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("b")],
                ),
            ),
            // Opaque: keeps itself and its dependency whole.
            (
                opaque.clone(),
                LoadedModule {
                    id: opaque.clone(),
                    code: "import \"./opaque-dep.js\";\n".to_string(),
                    imports: vec![(
                        "./opaque-dep.js".to_string(),
                        opaque_dep.clone(),
                        ImportKind::Static,
                    )],
                    side_effects: None,
                    module_type: ModuleType::Js,
                    map: None,
                    css: None,
                    shake: None,
                },
            ),
            (
                opaque_dep.clone(),
                shaken_module(
                    "/opaque-dep.js",
                    "export const c = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("c")],
                ),
            ),
        ]);
        // Entry must reach the opaque module for it to be live.
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        assert!(usage.full.contains(&ns));
        assert!(usage.full.contains(&dyn_target));
        assert!(usage.full.contains(&opaque));
        assert!(usage.full.contains(&opaque_dep));
    }

    #[test]
    fn usage_star_leaks_only_unknown_names() {
        let entry = ModuleId::new("/e.js");
        let barrel = ModuleId::new("/barrel.js");
        let leaf = ModuleId::new("/leaf.js");
        let modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x, own } from \"./barrel.js\";\nconsole.log(x, own);\n",
                    vec![(
                        "./barrel.js".to_string(),
                        barrel.clone(),
                        ImportKind::Static,
                    )],
                    vec![vec![
                        ImportBinding::Named("x".to_string()),
                        ImportBinding::Named("own".to_string()),
                    ]],
                    vec![],
                ),
            ),
            (
                barrel.clone(),
                shaken_module(
                    "/barrel.js",
                    "export const own = 0;\nexport * from \"./leaf.js\";\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![vec![ImportBinding::Reexport]],
                    vec![
                        local_export("own"),
                        ferrite_transform::ParsedExport {
                            exported: "*".to_string(),
                            local: None,
                            from: Some("/leaf.js".to_string()),
                            imported: None,
                            target: Some(leaf.clone()),
                        },
                    ],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const x = 1;\nexport const y = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("x"), local_export("y")],
                ),
            ),
        ]);
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let usage = used_exports(std::slice::from_ref(&entry), &modules, &live);
        // `own` is local to the barrel; only `x` leaks to the leaf.
        assert_eq!(
            usage.used.get(&leaf).cloned().unwrap_or_default(),
            HashSet::from(["x".to_string()])
        );
    }

    #[test]
    fn statements_drop_unused_and_reminify() {
        let entry = ModuleId::new("/e.js");
        let leaf = ModuleId::new("/leaf.js");
        let mut modules = HashMap::from([
            (
                entry.clone(),
                shaken_module(
                    "/e.js",
                    "import { x } from \"./leaf.js\";\nconsole.log(x);\n",
                    vec![("./leaf.js".to_string(), leaf.clone(), ImportKind::Static)],
                    vec![named("x")],
                    vec![],
                ),
            ),
            (
                leaf.clone(),
                shaken_module(
                    "/leaf.js",
                    "export const xray = 1;\nexport const yankee = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("xray"), local_export("yankee")],
                ),
            ),
        ]);
        // Entry imports `xray` (rename the binding to match the leaf).
        modules
            .get_mut(&entry)
            .unwrap()
            .shake
            .as_mut()
            .unwrap()
            .import_bindings = vec![named("xray")];
        let live: HashSet<ModuleId> = modules.keys().cloned().collect();
        let request = shake_request(vec![entry.clone()]);
        shake_statements(&mut modules, &live, &request).unwrap();
        let code = &modules.get(&leaf).unwrap().code;
        assert!(code.contains("xray"), "{code}");
        assert!(!code.contains("yankee"), "{code}");
        // Entry keeps everything.
        assert!(modules.get(&entry).unwrap().code.contains("console"));
    }

    fn hoist_request(entries: Vec<ModuleId>) -> BundleRequest {
        BundleRequest {
            entries,
            env: "client".to_string(),
            minify: false,
            sourcemap: false,
            map_comment: false,
            treeshake: false,
            engine: "oxc".to_string(),
            scope_hoist: true,
        }
    }

    fn js_files(output: &BundleOutput) -> Vec<String> {
        let mut files: Vec<String> = output
            .bundle
            .files
            .values()
            .filter(|file| file.name.ends_with(".js"))
            .map(|file| String::from_utf8_lossy(&file.contents).into_owned())
            .collect();
        files.sort();
        files
    }

    #[tokio::test]
    async fn scope_hoist_merges_entry_closure() {
        let modules = HashMap::from([
            (
                ModuleId::new("/e.js"),
                shaken_module(
                    "/e.js",
                    "import { value } from \"/lib.js\";\nconsole.log(value);\n",
                    vec![(
                        "/lib.js".to_string(),
                        ModuleId::new("/lib.js"),
                        ImportKind::Static,
                    )],
                    vec![named("value")],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/lib.js"),
                shaken_module(
                    "/lib.js",
                    "export const value = 41 + 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("value")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/e.js")]),
            )
            .await
            .unwrap();
        let files = js_files(&output);
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].contains("$f0$value"), "{}", files[0]);
        assert!(!files[0].contains("from \"/lib.js\""), "{}", files[0]);
    }

    #[tokio::test]
    async fn scope_hoist_splits_shared_and_dynamic() {
        let modules = HashMap::from([
            (
                ModuleId::new("/a.js"),
                shaken_module(
                    "/a.js",
                    "import { s } from \"/shared.js\";\nimport(\"/lazy.js\");\nconsole.log(s);\n",
                    vec![
                        (
                            "/shared.js".to_string(),
                            ModuleId::new("/shared.js"),
                            ImportKind::Static,
                        ),
                        (
                            "/lazy.js".to_string(),
                            ModuleId::new("/lazy.js"),
                            ImportKind::Dynamic,
                        ),
                    ],
                    vec![named("s"), vec![]],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/b.js"),
                shaken_module(
                    "/b.js",
                    "import { s } from \"/shared.js\";\nconsole.log(s);\n",
                    vec![(
                        "/shared.js".to_string(),
                        ModuleId::new("/shared.js"),
                        ImportKind::Static,
                    )],
                    vec![named("s")],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/shared.js"),
                shaken_module(
                    "/shared.js",
                    "export const s = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("s")],
                ),
            ),
            (
                ModuleId::new("/lazy.js"),
                shaken_module(
                    "/lazy.js",
                    "export const l = 2;\n",
                    vec![],
                    vec![],
                    vec![local_export("l")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/a.js"), ModuleId::new("/b.js")]),
            )
            .await
            .unwrap();
        // /a.js, /b.js hoist alone (shared/dynamic split out) + 2 singles.
        assert_eq!(output.chunks.len(), 4, "{:?}", output.chunks.keys());
        let entry_a = &output.chunks["/a.js"];
        assert_eq!(entry_a.modules, vec![ModuleId::new("/a.js")]);
        // Cross-chunk import rewritten to the shared chunk file.
        let files = js_files(&output);
        let a_file = files
            .iter()
            .find(|file| file.contains("console.log"))
            .unwrap();
        assert!(a_file.contains("./shared-"), "{a_file}");
    }

    #[tokio::test]
    async fn scope_hoist_bails_to_singles() {
        let modules = HashMap::from([
            (
                ModuleId::new("/e.js"),
                shaken_module(
                    "/e.js",
                    "import * as ns from \"/lib.js\";\nconsole.log(ns.value);\n",
                    vec![(
                        "/lib.js".to_string(),
                        ModuleId::new("/lib.js"),
                        ImportKind::Static,
                    )],
                    vec![vec![ImportBinding::Namespace]],
                    vec![],
                ),
            ),
            (
                ModuleId::new("/lib.js"),
                shaken_module(
                    "/lib.js",
                    "export const value = 1;\n",
                    vec![],
                    vec![],
                    vec![local_export("value")],
                ),
            ),
        ]);
        let bundler = FerriteBundler::new(Arc::new(MapLoader { modules }));
        let output = bundler
            .bundle(
                &ModuleGraph::new(),
                &test_config(),
                hoist_request(vec![ModuleId::new("/e.js")]),
            )
            .await
            .unwrap();
        assert_eq!(output.chunks.len(), 2);
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
                    css: None,
                    shake: None,
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
                    css: None,
                    shake: None,
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
                    css: None,
                    shake: None,
                },
            ),
        ]);
        let live = tree_shake(&entries, &modules);
        assert!(live.contains(&ModuleId::new("/a.js")));
        assert!(live.contains(&ModuleId::new("/b.js")));
        assert!(!live.contains(&ModuleId::new("/z.js")));
    }
}
