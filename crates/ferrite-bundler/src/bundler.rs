//! Default production bundler.

use crate::bundle::*;
use crate::chunks::*;
use crate::emit::*;
use crate::loader::*;
use crate::shake::*;
use ferrite_core::FerriteError;
use ferrite_core::Hash;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_graph::ImportKind;
use ferrite_graph::ModuleGraph;
use ferrite_manifest::BuildManifest;
use ferrite_manifest::ManifestEntry;
use ferrite_manifest::SsrManifest;
use ferrite_plugin::EmittedFile;
use ferrite_plugin::OutputBundle;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;

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
        hooks: &(dyn crate::BundleHooks + Send + Sync),
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
        // 2c. Render start (entries + live set known).
        hooks.render_start(&request.entries).await?;
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
        // 3b. `renderChunk` replacements apply before hashing so the
        //     content hash covers hooked code; wrappers are collected now
        //     (folded into the hash below) and applied at emit time.
        let mut wrappers: HashMap<String, ferrite_plugin::ChunkWrapper> = HashMap::new();
        for chunk in chunks.values_mut() {
            if let Some(code) = hooks
                .render_chunk(&chunk.id, chunk.code.clone(), chunk.entry)
                .await?
            {
                chunk.code = code;
            }
            wrappers.insert(
                chunk.id.clone(),
                hooks
                    .chunk_wrapper(&chunk.id, &chunk.code, chunk.entry)
                    .await?,
            );
        }
        // 4. Hash + file names. Each name embeds a content hash of the
        //    chunk's own code plus every transitively reachable chunk's
        //    content hash, so a change anywhere in the dependency cone
        //    renames the file (no stale caches). The hash intentionally
        //    covers pre-rewrite code: rewritten code embeds file names, so
        //    hashing emitted bytes would be circular (a file cannot contain
        //    its own hash). Hook contributions (`renderChunk` output,
        //    wrapper text, `augmentChunkHash` parts) are part of the
        //    material.
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
            if let Some(text) = wrappers
                .get(&chunk.id)
                .and_then(wrapper_hash_text)
            {
                material.push('\0');
                material.push_str(&text);
            }
            for extra in hooks.chunk_hash_extra(&chunk.id).await? {
                material.push('\0');
                material.push_str(&extra);
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
            let code = match wrappers.get(&chunk_id) {
                Some(wrapper) => wrapper.apply(&code),
                None => code,
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

/// Wrapper text folded into a chunk's hash material.
fn wrapper_hash_text(wrapper: &ferrite_plugin::ChunkWrapper) -> Option<String> {
    if wrapper.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for text in [
        &wrapper.banner,
        &wrapper.intro,
        &wrapper.outro,
        &wrapper.footer,
    ]
    .into_iter()
    .flatten()
    {
        parts.push(text.as_str());
    }
    Some(parts.join("\n"))
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
