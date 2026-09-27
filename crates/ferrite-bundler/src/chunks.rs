//! Chunk planning.

use crate::bundle::*;
use crate::loader::*;
use ferrite_core::FerriteError;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_graph::ImportKind;
use std::collections::HashMap;
use std::collections::HashSet;

/// One chunk group: members sharing a file.
pub(crate) struct ChunkGroup {
    /// Chunk id (entry module id for concats, module id for singles).
    pub(crate) id: String,
    /// Member module ids (emit order for concats).
    pub(crate) members: Vec<ModuleId>,
    /// Rendered code.
    pub(crate) code: String,
    /// Source map JSON, when produced.
    pub(crate) map: Option<String>,
    /// Preserved export names (concat entries).
    pub(crate) exports: Vec<String>,
}

/// Plan chunk groups: without `scope_hoist` every module is its own
/// group; otherwise each entry closure owned by exactly one entry is
/// concatenated when eligible (§91). Shared, non-JS, or ineligible
/// modules stay separate so semantics never change silently.
pub(crate) fn plan_chunks(
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
pub(crate) fn static_closure(
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
pub(crate) fn try_concat(
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
pub(crate) fn reachable_content_hashes<'a>(
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
