//! Concat binding renames.

use crate::concat::analyze::*;
use crate::concat::types::*;
use ferrite_core::Result;
use std::collections::HashMap;

/// Binding prefix for member `index`.
pub(crate) fn prefix(index: usize) -> String {
    format!("$f{index}$")
}

/// Final renamed name of member `target`'s default export.
pub(crate) fn default_final_name(target: usize, parsed: &[Analyzed]) -> String {
    match parsed[target].default_decl.as_deref() {
        Some(decl) => format!("{}{decl}", prefix(target)),
        None => format!("{}$default", prefix(target)),
    }
}

/// Resolve inside import aliases for member `index`: imported local →
/// final renamed target binding. `None` = ineligible.
pub(crate) fn resolve_aliases(
    module: &ConcatModule,
    index: usize,
    modules: &[ConcatModule],
    parsed: &[Analyzed],
    index_of: &HashMap<&str, usize>,
) -> Result<Option<HashMap<String, String>>> {
    // Edge lookup: specifier text → target id.
    let edge_of = |source: &str| {
        module
            .imports
            .iter()
            .find(|edge| edge.specifier == source)
            .map(|edge| (edge.target.as_str(), edge.is_static))
    };
    let mut map = HashMap::new();
    for import in &parsed[index].imports {
        let (target_id, is_static) = match edge_of(&import.source) {
            Some(edge) => edge,
            // Unresolved import (kept verbatim by the pipeline): outside.
            None => continue,
        };
        let inside = is_static && index_of.contains_key(target_id);
        if !inside {
            // Outside edge: kept as a real import; prefix the locals.
            for spec in &import.specs {
                let local = match spec {
                    StmtSpec::Named { local, .. }
                    | StmtSpec::Default { local }
                    | StmtSpec::Namespace { local } => local,
                };
                map.insert(local.clone(), format!("{}{local}", prefix(index)));
            }
            continue;
        }
        let target = index_of[target_id];
        for spec in &import.specs {
            match spec {
                StmtSpec::Namespace { .. } => return Ok(None),
                StmtSpec::Default { local } => {
                    if !parsed[target].has_default_export {
                        return Ok(None);
                    }
                    map.insert(local.clone(), default_final_name(target, parsed));
                }
                StmtSpec::Named { local, imported } => {
                    let Some(decl) = local_decl_of(&modules[target], &parsed[target], imported)
                    else {
                        return Ok(None);
                    };
                    map.insert(local.clone(), format!("{}{decl}", prefix(target)));
                }
            }
        }
    }
    // Non-entry inside re-exports cannot dissolve: bail. (Entry barrels
    // expand in `entry_export_block`; outside re-exports are kept.)
    if !module.is_entry {
        for reexport in &parsed[index].reexports {
            if let Some((target_id, is_static)) = edge_of(&reexport.source) {
                if is_static && index_of.contains_key(target_id) {
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(map))
}

/// Declared local backing `exported` in `module` (`None` when the name
/// comes from a re-export, is missing, or is not a plain declaration).
pub(crate) fn local_decl_of(
    module: &ConcatModule,
    parsed: &Analyzed,
    exported: &str,
) -> Option<String> {
    let detail = module
        .shake
        .exports
        .iter()
        .find(|export| export.from.is_none() && export.exported == exported)?;
    let local = detail.local.as_deref()?;
    parsed.declared.contains(local).then(|| local.to_string())
}
