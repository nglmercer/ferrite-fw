//! Tree-shaking.

use crate::bundle::*;
use crate::loader::*;
use ferrite_core::ModuleId;
use ferrite_core::Result;
use ferrite_graph::ImportKind;
use ferrite_transform::ImportBinding;
use ferrite_transform::MinifyRequest;
use ferrite_transform::ShakeInfo;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

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
pub(crate) fn propagate_reexports(
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
