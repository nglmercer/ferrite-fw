//! Environment-aware module graph (spec §7, §34, §60).
//!
//! The graph is the center of the system: every served, transformed, bundled,
//! or hot-updated module is tracked here with per-environment data.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

use dashmap::DashMap;
use ferrite_core::{Hash, ModuleId, ModuleType};

/// Import edge kinds (§7).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportKind {
    /// Static `import ... from`.
    Static,
    /// Dynamic `import()`.
    Dynamic,
    /// CSS `@import` / CSS import.
    Css,
    /// `?url` / `new URL(..., import.meta.url)`.
    Url,
    /// `?worker` / worker constructor.
    Worker,
    /// `?wasm` / `.wasm` import.
    Wasm,
}

/// An edge from a module to one of its imports.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportEdge {
    /// Raw specifier text.
    pub specifier: String,
    /// Resolved module id.
    pub resolved: ModuleId,
    /// Import kind.
    pub kind: ImportKind,
}

/// Per-environment module data.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ModuleEnvironmentData {
    /// Transformed code for this environment.
    pub code: Option<String>,
    /// Content hash of the transformed code.
    pub hash: Option<String>,
    /// True when the cached transform is stale.
    pub invalidated: bool,
}

/// HMR metadata for a module (§32–§34).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HmrMetadata {
    /// Module accepts its own updates (`import.meta.hot.accept()`).
    pub self_accepting: bool,
    /// Dependencies this module accepts updates for.
    pub accepted_deps: HashSet<String>,
    /// Timestamp of the last HMR update.
    pub last_update: Option<u64>,
}

/// A node in the module graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModuleNode {
    /// Module id.
    pub id: ModuleId,
    /// Served URL.
    pub url: String,
    /// Source file, if any (virtual modules have none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Module type.
    pub module_type: ModuleType,
    /// Outgoing import edges.
    pub imports: Vec<ImportEdge>,
    /// Importer ids (reverse edges).
    pub importers: Vec<ModuleId>,
    /// Hash of the last transform inputs.
    pub transform_hash: String,
    /// Last invalidation time (unix millis).
    pub last_invalidated: Option<u64>,
    /// SSR environment data.
    pub ssr: ModuleEnvironmentData,
    /// Client environment data.
    pub client: ModuleEnvironmentData,
    /// Extra environments (`worker`, `test`, custom).
    #[serde(default)]
    pub extra_envs: HashMap<String, ModuleEnvironmentData>,
    /// HMR metadata.
    pub hmr: HmrMetadata,
}

impl ModuleNode {
    /// Create a node with empty edges.
    #[must_use]
    pub fn new(id: ModuleId, url: String, module_type: ModuleType) -> Self {
        Self {
            id,
            url,
            file: None,
            module_type,
            imports: Vec::new(),
            importers: Vec::new(),
            transform_hash: String::new(),
            last_invalidated: None,
            ssr: ModuleEnvironmentData::default(),
            client: ModuleEnvironmentData::default(),
            extra_envs: HashMap::new(),
            hmr: HmrMetadata::default(),
        }
    }

    /// Environment data accessor (`client`/`ssr`/custom).
    ///
    /// Returns `None` for unknown custom environments instead of aliasing
    /// `client`, so reads agree with what [`ModuleNode::env_mut`] wrote.
    #[must_use]
    pub fn env(&self, name: &str) -> Option<&ModuleEnvironmentData> {
        match name {
            "client" => Some(&self.client),
            "ssr" => Some(&self.ssr),
            other => self.extra_envs.get(other),
        }
    }

    /// Mutable environment data accessor.
    pub fn env_mut(&mut self, name: &str) -> &mut ModuleEnvironmentData {
        match name {
            "client" => &mut self.client,
            "ssr" => &mut self.ssr,
            other => self.extra_envs.entry(other.to_string()).or_default(),
        }
    }
}

/// The module graph.
#[derive(Debug, Default)]
pub struct ModuleGraph {
    /// All modules by id.
    modules: DashMap<ModuleId, ModuleNode>,
}

impl ModuleGraph {
    /// Create an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a node.
    pub fn upsert(&self, node: ModuleNode) {
        self.modules.insert(node.id.clone(), node);
    }

    /// Get a snapshot of a node.
    #[must_use]
    pub fn get(&self, id: &ModuleId) -> Option<ModuleNode> {
        self.modules.get(id).map(|entry| entry.value().clone())
    }

    /// True when the graph contains `id`.
    #[must_use]
    pub fn contains(&self, id: &ModuleId) -> bool {
        self.modules.contains_key(id)
    }

    /// Number of modules.
    #[must_use]
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// All module ids.
    #[must_use]
    pub fn module_ids(&self) -> Vec<ModuleId> {
        self.modules
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Ensure a node exists, creating a stub when missing.
    pub fn ensure(&self, id: &ModuleId, module_type: ModuleType) {
        self.modules
            .entry(id.clone())
            .or_insert_with(|| ModuleNode::new(id.clone(), id.0.clone(), module_type));
    }

    /// Record an import edge (and the reverse importer edge).
    pub fn add_edge(&self, from: &ModuleId, edge: ImportEdge) {
        self.ensure(from, ModuleType::Js);
        self.ensure(&edge.resolved, ModuleType::Js);
        if let Some(mut node) = self.modules.get_mut(from) {
            if !node.imports.iter().any(|e| e.resolved == edge.resolved) {
                node.imports.push(edge.clone());
            }
        }
        if let Some(mut node) = self.modules.get_mut(&edge.resolved) {
            if !node.importers.iter().any(|id| id == from) {
                node.importers.push(from.clone());
            }
        }
    }

    /// Replace the import list of `id` (used after re-transform).
    pub fn set_imports(&self, id: &ModuleId, edges: Vec<ImportEdge>) {
        // Remove stale reverse edges first.
        let old: Vec<ModuleId> = self
            .get(id)
            .map(|node| node.imports.into_iter().map(|e| e.resolved).collect())
            .unwrap_or_default();
        for dep in &old {
            if !edges.iter().any(|e| &e.resolved == dep) {
                if let Some(mut node) = self.modules.get_mut(dep) {
                    node.importers.retain(|importer| importer != id);
                }
            }
        }
        self.ensure(id, ModuleType::Js);
        if let Some(mut node) = self.modules.get_mut(id) {
            node.imports = edges.clone();
        }
        for edge in &edges {
            self.ensure(&edge.resolved, ModuleType::Js);
            if let Some(mut node) = self.modules.get_mut(&edge.resolved) {
                if !node.importers.iter().any(|importer| importer == id) {
                    node.importers.push(id.clone());
                }
            }
        }
    }

    /// Update per-environment code + transform hash.
    pub fn set_transformed(&self, id: &ModuleId, env: &str, code: String, hash: &Hash) {
        self.ensure(id, ModuleType::Js);
        if let Some(mut node) = self.modules.get_mut(id) {
            let data = node.env_mut(env);
            data.code = Some(code);
            data.hash = Some(hash.0.clone());
            data.invalidated = false;
            node.transform_hash = hash.0.clone();
        }
    }

    /// Invalidate a module (§60): mark environments stale and stamp the time.
    pub fn invalidate(&self, id: &ModuleId) {
        if let Some(mut node) = self.modules.get_mut(id) {
            node.client.invalidated = true;
            node.ssr.invalidated = true;
            for data in node.extra_envs.values_mut() {
                data.invalidated = true;
            }
            node.last_invalidated = Some(now_millis());
        }
    }

    /// Invalidate a module and every transitive importer.
    pub fn invalidate_tree(&self, id: &ModuleId) -> Vec<ModuleId> {
        let mut invalidated = Vec::new();
        let mut queue = VecDeque::from([id.clone()]);
        let mut seen = HashSet::new();
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            self.invalidate(&current);
            invalidated.push(current.clone());
            if let Some(node) = self.get(&current) {
                for importer in node.importers {
                    queue.push_back(importer);
                }
            }
        }
        invalidated
    }

    /// Find the nearest HMR acceptance boundary (§34).
    ///
    /// Walks importers from `changed`; returns the update chain when a
    /// self-accepting module (or accepting importer) is found, or `None`
    /// when a full reload is required.
    #[must_use]
    pub fn hmr_boundaries(&self, changed: &ModuleId) -> Option<Vec<ModuleId>> {
        // Changed module accepts itself.
        if let Some(node) = self.get(changed) {
            if node.hmr.self_accepting || node.module_type == ModuleType::Css {
                return Some(vec![changed.clone()]);
            }
            // Direct importer accepts this dep.
            for importer_id in &node.importers {
                if let Some(importer) = self.get(importer_id) {
                    if importer.hmr.accepted_deps.contains(&changed.0)
                        || importer.hmr.self_accepting
                    {
                        return Some(vec![importer_id.clone(), changed.clone()]);
                    }
                }
            }
        }
        // Breadth-first search for the nearest boundary.
        let mut queue = VecDeque::new();
        let mut seen = HashSet::new();
        if let Some(node) = self.get(changed) {
            for importer in node.importers {
                queue.push_back((importer, vec![changed.clone()]));
            }
        }
        while let Some((current, mut chain)) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            chain.insert(0, current.clone());
            let Some(node) = self.get(&current) else {
                continue;
            };
            if node.hmr.self_accepting {
                return Some(chain);
            }
            if node.importers.is_empty() {
                // Dead-end branch: keep searching the remaining branches for
                // the nearest boundary instead of aborting the whole walk.
                continue;
            }
            for importer in node.importers {
                queue.push_back((importer, chain.clone()));
            }
        }
        None
    }

    /// Collect every accepting boundary. An unaccepted root forces a reload.
    /// Unlike the legacy nearest-chain query, this checks all importer branches.
    pub fn hmr_accepting_boundaries(&self, changed: &ModuleId) -> Option<Vec<ModuleId>> {
        let mut queue = VecDeque::from([changed.clone()]);
        let mut seen = HashSet::new();
        let mut boundaries = HashSet::new();
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let node = self.get(&current)?;
            if node.hmr.self_accepting || node.module_type == ModuleType::Css {
                boundaries.insert(current);
                continue;
            }
            if node.importers.is_empty() {
                return None;
            }
            for importer_id in node.importers {
                let importer = self.get(&importer_id)?;
                if importer.hmr.accepted_deps.contains(&current.0) {
                    boundaries.insert(importer_id);
                } else {
                    queue.push_back(importer_id);
                }
            }
        }
        if boundaries.is_empty() {
            return None;
        }
        let mut boundaries: Vec<_> = boundaries.into_iter().collect();
        boundaries.sort_by(|a, b| a.0.cmp(&b.0));
        Some(boundaries)
    }

    /// Remove a module and detach its edges.
    pub fn remove(&self, id: &ModuleId) -> Option<ModuleNode> {
        let (_, node) = self.modules.remove(id)?;
        for edge in &node.imports {
            if let Some(mut dep) = self.modules.get_mut(&edge.resolved) {
                dep.importers.retain(|importer| importer != id);
            }
        }
        for importer in &node.importers {
            if let Some(mut node) = self.modules.get_mut(importer) {
                node.imports.retain(|edge| edge.resolved != *id);
            }
        }
        Some(node)
    }
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Format an instant for diagnostics.
#[must_use]
pub fn instant_age(instant: &Instant) -> std::time::Duration {
    instant.elapsed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(resolved: &str) -> ImportEdge {
        ImportEdge {
            specifier: resolved.to_string(),
            resolved: ModuleId::new(resolved),
            kind: ImportKind::Static,
        }
    }

    #[test]
    fn edges_maintain_importers() {
        let graph = ModuleGraph::new();
        graph.add_edge(&ModuleId::new("/a.ts"), edge("/b.ts"));
        let b = graph.get(&ModuleId::new("/b.ts")).unwrap();
        assert_eq!(b.importers, vec![ModuleId::new("/a.ts")]);
    }

    #[test]
    fn hmr_boundary_found() {
        let graph = ModuleGraph::new();
        let mut boundary = ModuleNode::new(
            ModuleId::new("/app.tsx"),
            "/app.tsx".to_string(),
            ModuleType::Tsx,
        );
        boundary.hmr.self_accepting = true;
        graph.upsert(boundary);
        graph.add_edge(&ModuleId::new("/app.tsx"), edge("/leaf.ts"));
        let chain = graph.hmr_boundaries(&ModuleId::new("/leaf.ts")).unwrap();
        assert_eq!(chain.first(), Some(&ModuleId::new("/app.tsx")));
    }

    #[test]
    fn hmr_full_reload_when_no_boundary() {
        let graph = ModuleGraph::new();
        graph.add_edge(&ModuleId::new("/entry.ts"), edge("/leaf.ts"));
        assert!(graph.hmr_boundaries(&ModuleId::new("/leaf.ts")).is_none());
    }

    #[test]
    fn hmr_dead_end_branch_does_not_hide_boundary() {
        let graph = ModuleGraph::new();
        let mut boundary = ModuleNode::new(
            ModuleId::new("/boundary.ts"),
            "/boundary.ts".to_string(),
            ModuleType::Ts,
        );
        boundary.hmr.self_accepting = true;
        graph.upsert(boundary);
        // The boundary sits one level up so the direct-importer fast path
        // does not trigger; the dead-end branch is queued first, so the
        // BFS must skip it and still find the boundary.
        graph.add_edge(&ModuleId::new("/entry.ts"), edge("/leaf.ts"));
        graph.add_edge(&ModuleId::new("/mid.ts"), edge("/leaf.ts"));
        graph.add_edge(&ModuleId::new("/boundary.ts"), edge("/mid.ts"));
        let chain = graph.hmr_boundaries(&ModuleId::new("/leaf.ts")).unwrap();
        assert_eq!(chain.first(), Some(&ModuleId::new("/boundary.ts")));
    }

    #[test]
    fn all_hmr_branches_must_accept_and_boundaries_are_deterministic() {
        let graph = ModuleGraph::new();
        for name in ["/b.ts", "/a.ts"] {
            graph.add_edge(&ModuleId::new(name), edge("/leaf.ts"));
            let mut node = graph.get(&ModuleId::new(name)).unwrap();
            node.hmr.self_accepting = true;
            graph.upsert(node);
        }
        assert_eq!(
            graph
                .hmr_accepting_boundaries(&ModuleId::new("/leaf.ts"))
                .unwrap(),
            vec![ModuleId::new("/a.ts"), ModuleId::new("/b.ts")]
        );
        graph.add_edge(&ModuleId::new("/unaccepted-root.ts"), edge("/leaf.ts"));
        assert!(graph
            .hmr_accepting_boundaries(&ModuleId::new("/leaf.ts"))
            .is_none());
        let cycle = ModuleGraph::new();
        cycle.add_edge(&ModuleId::new("/a.ts"), edge("/b.ts"));
        cycle.add_edge(&ModuleId::new("/b.ts"), edge("/a.ts"));
        assert!(cycle
            .hmr_accepting_boundaries(&ModuleId::new("/a.ts"))
            .is_none());
    }

    #[test]
    fn env_read_write_consistency() {
        let mut node = ModuleNode::new(ModuleId::new("/a.ts"), "/a.ts".to_string(), ModuleType::Ts);
        assert!(node.env("worker").is_none());
        node.env_mut("worker").code = Some("worker code".to_string());
        assert_eq!(
            node.env("worker").and_then(|data| data.code.clone()),
            Some("worker code".to_string())
        );
        // Custom-env writes must not leak into `client`, and unknown envs
        // must not alias it either.
        assert!(node
            .env("client")
            .and_then(|data| data.code.clone())
            .is_none());
        assert!(node.env("typo-env").is_none());
    }

    #[test]
    fn invalidate_tree_walks_importers() {
        let graph = ModuleGraph::new();
        graph.add_edge(&ModuleId::new("/a.ts"), edge("/b.ts"));
        graph.add_edge(&ModuleId::new("/b.ts"), edge("/c.ts"));
        let invalidated = graph.invalidate_tree(&ModuleId::new("/c.ts"));
        assert_eq!(invalidated.len(), 3);
    }
}
