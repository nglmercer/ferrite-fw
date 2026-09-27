//! Scope-hoisted concatenation (§91): merge a chunk's modules into one
//! scope with renamed top-level bindings.
//!
//! [`concat_modules`] takes an ordered module closure (post-order DFS from
//! the chunk entry, computed by the bundler) and produces a single ESM
//! document: inside imports/exports dissolve into direct references to
//! prefixed bindings, outside imports/exports survive as real chunk edges,
//! and the entry's export surface is preserved. Source maps chain through
//! each member's input map and concatenate with line offsets.
//!
//! Soundness is by bail-out: anything the pass cannot prove safe —
//! namespace imports from inside, inside re-exports (except entry
//! barrels, which expand), direct `eval`, `with`, `import.meta` — returns
//! `Ok(None)` and the caller falls back to one chunk per module.

mod analyze;
mod emit;
mod maps;
mod rename;
mod types;

use crate::concat::analyze::*;
use crate::concat::emit::*;
use crate::concat::maps::*;
use crate::concat::rename::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use std::collections::HashMap;
pub use types::{ConcatImport, ConcatModule, ConcatOutput};

/// Concatenate `modules` (emit order) into one scope.
///
/// Returns `Ok(None)` when the closure is ineligible (caller falls back
/// to per-module chunks); `Err` only on programmer errors (no entry) or
/// unparseable member code.
pub fn concat_modules(modules: &[ConcatModule], want_map: bool) -> Result<Option<ConcatOutput>> {
    let entry_count = modules.iter().filter(|module| module.is_entry).count();
    if entry_count != 1 {
        return Err(FerriteError::Build(format!(
            "concat needs exactly one entry, got {entry_count}"
        )));
    }
    if modules
        .iter()
        .any(|module| module.code.contains("import.meta"))
    {
        return Ok(None);
    }
    let index_of: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| (module.id.as_str(), index))
        .collect();
    // Per-module analysis: parse, top-level bindings, import locals.
    let mut parsed = Vec::with_capacity(modules.len());
    for module in modules {
        let Some(analyzed) = analyze(module)? else {
            return Ok(None);
        };
        parsed.push(analyzed);
    }
    // Resolve inside aliases: local name → renamed target binding.
    // `aliases[i]` maps an imported local to its final name.
    let mut aliases: Vec<HashMap<String, String>> = Vec::with_capacity(modules.len());
    for (index, module) in modules.iter().enumerate() {
        let Some(map) = resolve_aliases(module, index, modules, &parsed, &index_of)? else {
            return Ok(None);
        };
        aliases.push(map);
    }
    // Emit each member: rename, dissolve inside edges, convert exports.
    let mut pieces = Vec::with_capacity(modules.len());
    let mut default_names: Vec<Option<String>> = vec![None; modules.len()];
    for (index, module) in modules.iter().enumerate() {
        let emit = emit_module(module, index, modules, &parsed, &aliases, &index_of)?;
        let Some(emit) = emit else {
            return Ok(None);
        };
        default_names[index] = emit.default_name.clone();
        pieces.push(emit);
    }
    // Entry export block (entry barrels expand inside re-exports).
    let entry_index = modules
        .iter()
        .position(|module| module.is_entry)
        .unwrap_or(0);
    let Some(block) = entry_export_block(
        &modules[entry_index],
        entry_index,
        modules,
        &parsed,
        &default_names,
        &index_of,
    )?
    else {
        return Ok(None);
    };
    // Join + maps.
    let mut code = String::new();
    let mut line_starts = Vec::with_capacity(pieces.len());
    for (index, piece) in pieces.iter().enumerate() {
        line_starts.push(code.lines().count() as u32);
        code.push_str(&format!("// @ferrite-concat {}\n", modules[index].id));
        code.push_str(&piece.code);
        if !piece.code.ends_with('\n') {
            code.push('\n');
        }
    }
    code.push_str(&block.code);
    let exports = block.exports;
    let map = if want_map {
        Some(concat_maps(&pieces, &line_starts, modules)?)
    } else {
        None
    };
    Ok(Some(ConcatOutput { code, map, exports }))
}

#[cfg(test)]
mod tests {
    use ferrite_core::ModuleType;

    use super::*;
    use crate::parse::parse_module;
    use crate::ShakeInfo;

    fn member(id: &str, code: &str, imports: Vec<(&str, &str)>, is_entry: bool) -> ConcatModule {
        let parsed = parse_module(id, code, &ModuleType::Js).unwrap();
        ConcatModule {
            id: id.to_string(),
            code: code.to_string(),
            shake: ShakeInfo {
                import_bindings: parsed
                    .imports
                    .iter()
                    .map(|import| import.bindings.clone())
                    .collect(),
                exports: parsed.export_details,
            },
            imports: imports
                .into_iter()
                .map(|(specifier, target)| ConcatImport {
                    specifier: specifier.to_string(),
                    target: target.to_string(),
                    is_static: true,
                })
                .collect(),
            is_entry,
            input_map: None,
        }
    }

    #[test]
    fn merges_named_and_default_without_collisions() {
        let lib = member(
            "/lib.js",
            "const x = 1;\nexport const value = x + 1;\nexport default function main() { return x; }\n",
            vec![],
            false,
        );
        let entry = member(
            "/entry.js",
            "import def, { value } from \"/lib.js\";\nconst x = 100;\nexport const result = def() + value + x;\n",
            vec![("/lib.js", "/lib.js")],
            true,
        );
        let out = concat_modules(&[lib, entry], false)
            .unwrap()
            .expect("concat");
        // No surviving inside import; both `x` bindings prefixed apart.
        assert!(!out.code.contains("from \"/lib.js\""), "{}", out.code);
        assert!(out.code.contains("$f0$x"), "{}", out.code);
        assert!(out.code.contains("$f1$x"), "{}", out.code);
        assert!(out.code.contains("$f0$value"), "{}", out.code);
        assert!(out.code.contains("$f0$main"), "{}", out.code);
        // Entry surface preserved.
        assert!(
            out.code.contains("export { $f1$result as result }"),
            "{}",
            out.code
        );
        assert_eq!(out.exports, vec!["result".to_string()]);
    }

    #[test]
    fn converts_anonymous_default_and_keeps_outside_edges() {
        let lib = member("/lib.js", "export default 40 + 2;\n", vec![], false);
        let entry = member(
            "/entry.js",
            "import answer from \"/lib.js\";\nimport { ext } from \"/outside.js\";\nconsole.log(answer, ext);\n",
            vec![("/lib.js", "/lib.js"), ("/outside.js", "/outside.js")],
            true,
        );
        let out = concat_modules(&[lib, entry], false)
            .unwrap()
            .expect("concat");
        assert!(
            out.code.contains("const $f0$$default = 40 + 2"),
            "{}",
            out.code
        );
        assert!(
            out.code.contains("console.log($f0$$default, $f1$ext)"),
            "{}",
            out.code
        );
        // Outside import survives with prefixed local.
        assert!(out.code.contains("from \"/outside.js\""), "{}", out.code);
        assert!(out.code.contains("$f1$ext"), "{}", out.code);
    }

    #[test]
    fn entry_barrel_expands_inside_reexports() {
        let leaf = member("/leaf.js", "export const via = 1;\n", vec![], false);
        let entry = member(
            "/entry.js",
            "export { via } from \"/leaf.js\";\nexport * from \"/leaf.js\";\n",
            vec![("/leaf.js", "/leaf.js")],
            true,
        );
        let out = concat_modules(&[leaf, entry], false)
            .unwrap()
            .expect("concat");
        assert!(out.code.contains("$f0$via as via"), "{}", out.code);
        assert!(!out.code.contains("from \"/leaf.js\""), "{}", out.code);
    }

    #[test]
    fn inner_shadowing_survives_rename() {
        let lib = member("/lib.js", "export const value = 1;\n", vec![], false);
        let entry = member(
            "/entry.js",
            "import { value } from \"/lib.js\";\nfunction f(value) { return value * 2; }\nexport const out = f(value);\n",
            vec![("/lib.js", "/lib.js")],
            true,
        );
        let out = concat_modules(&[lib, entry], false)
            .unwrap()
            .expect("concat");
        // Top-level `f` renamed, parameter shadows: untouched.
        assert!(out.code.contains("function $f1$f(value)"), "{}", out.code);
        assert!(out.code.contains("$f1$f($f0$value)"), "{}", out.code);
    }

    #[test]
    fn bails_on_namespace_inside_reexport_eval_and_meta() {
        // Namespace from inside.
        let lib = member("/lib.js", "export const a = 1;\n", vec![], false);
        let entry = member(
            "/entry.js",
            "import * as ns from \"/lib.js\";\nconsole.log(ns.a);\n",
            vec![("/lib.js", "/lib.js")],
            true,
        );
        assert!(concat_modules(&[lib, entry], false).unwrap().is_none());
        // Non-entry inside re-export.
        let leaf = member("/leaf.js", "export const a = 1;\n", vec![], false);
        let mid = member(
            "/mid.js",
            "export { a } from \"/leaf.js\";\n",
            vec![("/leaf.js", "/leaf.js")],
            false,
        );
        let entry = member(
            "/entry.js",
            "import { a } from \"/mid.js\";\nconsole.log(a);\n",
            vec![("/mid.js", "/mid.js")],
            true,
        );
        assert!(concat_modules(&[leaf, mid, entry], false)
            .unwrap()
            .is_none());
        // Direct eval.
        let evil = member("/evil.js", "const x = 1;\neval(\"x\");\n", vec![], false);
        let entry = member(
            "/entry.js",
            "import \"/evil.js\";\nconsole.log(1);\n",
            vec![("/evil.js", "/evil.js")],
            true,
        );
        assert!(concat_modules(&[evil, entry], false).unwrap().is_none());
        // import.meta.
        let meta = member(
            "/meta.js",
            "export const url = import.meta.url;\n",
            vec![],
            false,
        );
        let entry = member(
            "/entry.js",
            "import { url } from \"/meta.js\";\nconsole.log(url);\n",
            vec![("/meta.js", "/meta.js")],
            true,
        );
        assert!(concat_modules(&[meta, entry], false).unwrap().is_none());
    }

    #[test]
    fn concatenates_maps_with_offsets() {
        let lib = member("/lib.js", "export const value = 1;\n", vec![], false);
        let entry = member(
            "/entry.js",
            "import { value } from \"/lib.js\";\nexport const out = value;\n",
            vec![("/lib.js", "/lib.js")],
            true,
        );
        let out = concat_modules(&[lib, entry], true)
            .unwrap()
            .expect("concat");
        let map = out.map.expect("map");
        let decoded = oxc_sourcemap::SourceMap::from_json_string(&map).unwrap();
        let sources: Vec<&str> = decoded.get_sources().collect();
        assert!(sources.contains(&"/lib.js"), "{sources:?}");
        assert!(sources.contains(&"/entry.js"), "{sources:?}");
        // Both pieces contribute mappings on distinct output lines.
        let mut by_source: HashMap<&str, Vec<u32>> = HashMap::new();
        for token in decoded.get_tokens() {
            if let Some(source_id) = token.get_source_id() {
                let source = decoded.get_source(source_id).unwrap_or("?");
                by_source
                    .entry(source)
                    .or_default()
                    .push(token.get_dst_line());
            }
        }
        let lib_lines = by_source.get("/lib.js").cloned().unwrap_or_default();
        let entry_lines = by_source.get("/entry.js").cloned().unwrap_or_default();
        assert!(!lib_lines.is_empty(), "{by_source:?}");
        assert!(!entry_lines.is_empty(), "{by_source:?}");
        assert!(
            entry_lines.iter().min() > lib_lines.iter().max(),
            "{by_source:?}"
        );
    }
}
