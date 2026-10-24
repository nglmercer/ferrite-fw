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

/// Reachability from entries, after import/export pruning has updated edges.
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
            if let Some(name) = export.imported.as_deref() {
                *changed |= used
                    .entry(target.clone())
                    .or_default()
                    .insert(name.to_string());
            } else {
                // `export * as ns from` exposes a namespace, not an export
                // literally named ns on the dependency.
                *changed |= full.insert(target.clone());
            }
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
        let parsed = compiler.parse(ferrite_transform::ParseRequest {
            id: module.id.0.clone(),
            code: minified.code.clone(),
            module_type: ferrite_core::ModuleType::Js,
        })?;
        if let Some(module) = modules.get_mut(id) {
            update_import_facts(module, parsed)?;
            module.code = minified.code;
            module.map = minified.map.map(|map| map.mappings);
        }
    }
    Ok(())
}

/// Remove static evaluation-only edges to explicitly effect-free JS modules.
/// Opaque facts, live bindings, dynamic imports, entries and CSS stay intact.
fn prune_effect_only_imports(
    modules: &mut HashMap<ModuleId, LoadedModule>,
    live: &HashSet<ModuleId>,
    request: &BundleRequest,
) -> Result<()> {
    let usage = used_exports(&request.entries, modules, live);
    let compiler = ferrite_transform::compiler_for_engine(&request.engine)?;
    let mut ids: Vec<_> = live.iter().cloned().collect();
    ids.sort_by(|a, b| a.0.cmp(&b.0));
    for id in ids {
        let Some(module) = modules.get(&id) else {
            continue;
        };
        let Some(shake) = &module.shake else { continue };
        if !module.module_type.is_js_like() {
            continue;
        }
        let mut specifiers = HashSet::new();
        let mut unused_reexports = HashSet::new();
        for (index, (specifier, dep, kind)) in module.imports.iter().enumerate() {
            let Some(target) = modules.get(dep) else {
                continue;
            };
            if *kind != ImportKind::Static
                || target.side_effects != Some(false)
                || target.css.is_some()
                || !target.module_type.is_js_like()
                || request.entries.contains(dep)
            {
                continue;
            }
            let Some(bindings) = shake.import_bindings.get(index) else {
                continue;
            };
            if bindings.is_empty() {
                continue;
            }
            let unused_exports =
                !usage.full.contains(dep) && usage.used.get(dep).is_none_or(HashSet::is_empty);
            if bindings.iter().all(|binding| match binding {
                ImportBinding::SideEffect => true,
                ImportBinding::Reexport => unused_exports,
                _ => false,
            }) {
                specifiers.insert(specifier.clone());
                if bindings.contains(&ImportBinding::Reexport) {
                    unused_reexports.insert(specifier.clone());
                }
            }
        }
        let Some(output) = ferrite_transform::remove_effect_only_imports(
            &id.0,
            &module.code,
            &specifiers,
            &unused_reexports,
            request.sourcemap,
            module.map.as_deref(),
        )?
        else {
            continue;
        };
        let parsed = compiler.parse(ferrite_transform::ParseRequest {
            id: id.0.clone(),
            code: output.code.clone(),
            module_type: ferrite_core::ModuleType::Js,
        })?;
        let module = modules.get_mut(&id).expect("live module");
        update_import_facts(module, parsed)?;
        module.code = output.code;
        module.map = output.map.map(|map| map.mappings);
    }
    Ok(())
}

fn update_import_facts(
    module: &mut LoadedModule,
    parsed: ferrite_transform::ParsedModule,
) -> Result<()> {
    let mut imports = Vec::new();
    let mut import_bindings = Vec::new();
    for import in parsed.imports {
        let dynamic = import.kind == ferrite_transform::ParsedImportKind::Dynamic;
        let edge = module
            .imports
            .iter()
            .find(|(specifier, _, kind)| {
                specifier == &import.specifier && (*kind == ImportKind::Dynamic) == dynamic
            })
            .ok_or_else(|| {
                ferrite_core::FerriteError::Build(format!(
                    "tree shaking introduced unresolved import `{}` in `{}`",
                    import.specifier, module.id.0
                ))
            })?;
        imports.push(edge.clone());
        import_bindings.push(import.bindings);
    }
    let mut exports = parsed.export_details;
    for export in &mut exports {
        if let Some(from) = &export.from {
            export.target = imports
                .iter()
                .find(|(specifier, _, kind)| specifier == from && *kind != ImportKind::Dynamic)
                .map(|(_, id, _)| id.clone());
        }
    }
    module.imports = imports;
    module.shake = Some(ShakeInfo {
        import_bindings,
        exports,
    });
    Ok(())
}

/// Reach a fixed point after removing effect-free edges and unused exports.
/// Facts only shrink; repeating allows dead barrels, cycles and their assets to
/// leave the reachable graph before chunk planning and manifest construction.
pub(crate) fn shake_modules(
    modules: &mut HashMap<ModuleId, LoadedModule>,
    request: &BundleRequest,
) -> Result<HashSet<ModuleId>> {
    let mut live = tree_shake(&request.entries, modules);
    if !request.treeshake {
        return Ok(live);
    }
    fn facts(
        modules: &HashMap<ModuleId, LoadedModule>,
        live: &HashSet<ModuleId>,
    ) -> (usize, usize, usize) {
        let mut counts = (live.len(), 0, 0);
        for id in live {
            if let Some(module) = modules.get(id) {
                counts.1 += module.imports.len();
                if let Some(shake) = &module.shake {
                    counts.2 += shake.exports.len()
                        + shake.import_bindings.iter().map(Vec::len).sum::<usize>();
                }
            }
        }
        counts
    }
    loop {
        let before = facts(modules, &live);
        shake_statements(modules, &live, request)?;
        prune_effect_only_imports(modules, &live, request)?;
        live = tree_shake(&request.entries, modules);
        if facts(modules, &live) == before {
            break;
        }
    }
    Ok(live)
}

#[cfg(test)]
mod effect_tests {
    use super::*;
    use ferrite_transform::JsCompiler;
    fn module(id: &str, code: &str, effects: Option<bool>) -> LoadedModule {
        let parsed = ferrite_transform::OxcCompiler::new(Default::default())
            .parse(ferrite_transform::ParseRequest {
                id: id.into(),
                code: code.into(),
                module_type: ferrite_core::ModuleType::Js,
            })
            .unwrap();
        let imports = parsed
            .imports
            .iter()
            .map(|import| {
                (
                    import.specifier.clone(),
                    ModuleId::new(&import.specifier),
                    if import.kind == ferrite_transform::ParsedImportKind::Dynamic {
                        ImportKind::Dynamic
                    } else {
                        ImportKind::Static
                    },
                )
            })
            .collect();
        let mut result = LoadedModule {
            id: ModuleId::new(id),
            code: code.into(),
            imports,
            side_effects: effects,
            module_type: ferrite_core::ModuleType::Js,
            map: None,
            css: None,
            shake: None,
        };
        update_import_facts(&mut result, parsed).unwrap();
        result
    }
    fn request() -> BundleRequest {
        BundleRequest {
            entries: vec![ModuleId::new("/entry.js")],
            env: "client".into(),
            minify: true,
            sourcemap: true,
            map_comment: true,
            treeshake: true,
            engine: "oxc".into(),
            scope_hoist: false,
        }
    }
    #[test]
    fn pruning_preserves_opaque_callers_entries_bindings_dynamic_and_css() {
        let mut entry = module("/entry.js", "import '/drop.js'; import '/unknown.js'; import '/true.js'; import '/style.js'; import '/opaque.js'; import '/other-entry.js'; import {kept} from '/used.js'; import * as ns from '/namespace.js'; export const result = [kept, ns]; export const load = () => import('/drop.js');", Some(false));
        let mut css = module("/style.js", "export default {};", Some(false));
        css.css = Some(crate::CssExtract {
            text: "body{color:red}".into(),
            is_modules: true,
            keep_js: true,
        });
        let mut opaque = module("/opaque.js", "import '/opaque-child.js';", None);
        opaque.shake = None;
        // Provide an input map so edge removal must chain rather than discard it.
        let parsed = ferrite_transform::remove_effect_only_imports(
            "/entry.js",
            &format!("import '/removed.js';\n{}", entry.code),
            &HashSet::from(["/removed.js".into()]),
            &HashSet::new(),
            true,
            None,
        )
        .unwrap()
        .unwrap();
        entry.code = parsed.code;
        entry.map = parsed.map.map(|map| map.mappings);
        let mut modules: HashMap<_, _> = [
            entry,
            css,
            opaque,
            module("/drop.js", "export const value = 1;", Some(false)),
            module("/unknown.js", "console.log('unknown');", None),
            module("/true.js", "console.log('true');", Some(true)),
            module("/other-entry.js", "console.log('entry');", Some(false)),
            module(
                "/opaque-child.js",
                "console.log('opaque child');",
                Some(false),
            ),
            module("/used.js", "export const kept = 1;", Some(false)),
            module("/namespace.js", "export const item = 2;", Some(false)),
        ]
        .into_iter()
        .map(|module| (module.id.clone(), module))
        .collect();
        let mut request = request();
        request.entries.push(ModuleId::new("/other-entry.js"));
        let live = shake_modules(&mut modules, &request).unwrap();
        assert_eq!(live.len(), modules.len());
        let entry = &modules[&ModuleId::new("/entry.js")];
        assert!(
            !entry.code.contains("import \"/drop.js\""),
            "{}",
            entry.code
        );
        assert!(
            entry.code.contains("import(\"/drop.js\")"),
            "{}",
            entry.code
        );
        assert!(entry.map.is_some());
        assert!(entry
            .imports
            .iter()
            .any(|(_, id, kind)| id.0 == "/drop.js" && *kind == ImportKind::Dynamic));
        assert!(!entry
            .imports
            .iter()
            .any(|(_, id, kind)| id.0 == "/drop.js" && *kind == ImportKind::Static));
    }
    #[test]
    fn dead_cycles_and_unused_barrels_leave_the_graph_without_affecting_shared_modules() {
        let mut modules: HashMap<_,_> = [
            module("/entry.js", "import '/cycle-a.js'; import '/barrel.js'; import {value} from '/shared.js'; console.log(value);", None),
            module("/cycle-a.js", "import '/cycle-b.js';", Some(false)),
            module("/cycle-b.js", "import '/cycle-a.js'; import '/shared.js';", Some(false)),
            module("/barrel.js", "export * from '/shared.js';", Some(false)),
            module("/shared.js", "export const value = 3;", Some(false)),
        ].into_iter().map(|module| (module.id.clone(), module)).collect();
        let original = modules.clone();
        let mut disabled = request();
        disabled.treeshake = false;
        assert_eq!(shake_modules(&mut modules, &disabled).unwrap().len(), 5);
        assert_eq!(
            modules[&ModuleId::new("/entry.js")].code,
            original[&ModuleId::new("/entry.js")].code
        );
        assert_eq!(
            shake_modules(&mut modules, &request()).unwrap(),
            HashSet::from([ModuleId::new("/entry.js"), ModuleId::new("/shared.js")])
        );
    }
}
