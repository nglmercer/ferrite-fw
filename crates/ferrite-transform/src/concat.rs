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

use std::collections::{HashMap, HashSet};

use ferrite_core::{FerriteError, Result};
use oxc_allocator::{Allocator, FromIn as _, ReplaceWith as _};
use oxc_ast::ast::*;
use oxc_ast_visit::{Visit, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_semantic::{SemanticBuilder, SymbolId};
use oxc_span::SourceType;

use crate::ShakeInfo;

/// One import edge, aligned with `ShakeInfo.import_bindings`.
#[derive(Debug, Clone)]
pub struct ConcatImport {
    /// Specifier text as it appears in `code` (resolved URL).
    pub specifier: String,
    /// Resolved target module id.
    pub target: String,
    /// True for static imports (only these dissolve).
    pub is_static: bool,
}

/// One module of the concat closure, in emit order.
#[derive(Debug, Clone)]
pub struct ConcatModule {
    /// Module id.
    pub id: String,
    /// Loaded code (transformed, minified).
    pub code: String,
    /// Statement-DCE facts.
    pub shake: ShakeInfo,
    /// Import edges (aligned with `shake.import_bindings`).
    pub imports: Vec<ConcatImport>,
    /// True for the chunk entry (exactly one).
    pub is_entry: bool,
    /// Map JSON (`code` → original), when the loader produced one.
    pub input_map: Option<String>,
}

/// Concatenated chunk.
#[derive(Debug, Clone)]
pub struct ConcatOutput {
    /// Single-scope ESM code.
    pub code: String,
    /// Concatenated map JSON, when requested.
    pub map: Option<String>,
    /// Entry export names preserved.
    pub exports: Vec<String>,
}

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

/// Parsed member: top-level bindings, import locals.
struct Analyzed {
    /// Top-level declared names (excluding import locals).
    declared: HashSet<String>,
    /// Import local names (specifier locals).
    imported: HashSet<String>,
    /// Import statements (`source`, specifiers).
    imports: Vec<StmtImport>,
    /// Re-export statements (`export … from`).
    reexports: Vec<StmtReexport>,
    /// Declared name of a named default export, when present.
    default_decl: Option<String>,
    /// True when any default export exists.
    has_default_export: bool,
}

/// One import statement's specifiers.
struct StmtImport {
    /// Module source text.
    source: String,
    /// (`local`, specifier kind).
    specs: Vec<StmtSpec>,
}

/// Import specifier kinds.
enum StmtSpec {
    /// `import { imported as local }` (`imported == local` unaliased).
    Named { local: String, imported: String },
    /// `import local`.
    Default { local: String },
    /// `import * as local`.
    Namespace { local: String },
}

/// One re-export statement.
struct StmtReexport {
    /// Module source text.
    source: String,
    /// (`exported`, `imported`) pairs; empty for plain `export *`.
    specs: Vec<(String, String)>,
    /// True for plain `export *` (namespace re-exports carry one spec).
    is_star: bool,
}

/// Module export name to string (`default` for default names).
fn export_name_of(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_str().to_string(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str().to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str().to_string(),
    }
}

/// Parse + collect top-level binding classes. `None` = ineligible
/// (`eval`, `with`, unparseable member code is an error instead).
fn analyze(module: &ConcatModule) -> Result<Option<Analyzed>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &module.code, SourceType::mjs()).parse();
    if parsed.fatal_error {
        return Err(FerriteError::Parse {
            id: module.id.clone(),
            message: "concat member does not parse".to_string(),
            frame: None,
        });
    }
    let program = parsed.program;
    // Direct `eval` / `with` defeat scope renaming: bail.
    struct Guard {
        found: bool,
    }
    impl<'a> Visit<'a> for Guard {
        fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
            if let Expression::Identifier(reference) = &it.callee {
                if reference.name.as_str() == "eval" {
                    self.found = true;
                    return;
                }
            }
            oxc_ast_visit::walk::walk_call_expression(self, it);
        }
        fn visit_with_statement(&mut self, _it: &oxc_ast::ast::WithStatement<'a>) {
            self.found = true;
        }
    }
    let mut guard = Guard { found: false };
    guard.visit_program(&program);
    if guard.found {
        return Ok(None);
    }
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let root = scoping.root_scope_id();
    let mut imported = HashSet::new();
    let mut imports = Vec::new();
    let mut reexports = Vec::new();
    let mut default_decl = None;
    let mut has_default_export = false;
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(decl) => {
                let mut specs = Vec::new();
                for specifier in decl.specifiers.iter().flat_map(|vec| vec.iter()) {
                    match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(spec) => {
                            let imported_name = export_name_of(&spec.imported);
                            let local = spec.local.name.as_str().to_string();
                            imported.insert(local.clone());
                            if imported_name == "default" {
                                specs.push(StmtSpec::Default { local });
                            } else {
                                specs.push(StmtSpec::Named {
                                    local,
                                    imported: imported_name,
                                });
                            }
                        }
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
                            let local = spec.local.name.as_str().to_string();
                            imported.insert(local.clone());
                            specs.push(StmtSpec::Default { local });
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
                            let local = spec.local.name.as_str().to_string();
                            imported.insert(local.clone());
                            specs.push(StmtSpec::Namespace { local });
                        }
                    }
                }
                imports.push(StmtImport {
                    source: decl.source.value.as_str().to_string(),
                    specs,
                });
            }
            Statement::ExportFromDeclaration(decl) => {
                let mut specs = Vec::new();
                for specifier in decl.specifiers.iter() {
                    specs.push((
                        export_name_of(&specifier.exported),
                        export_name_of(&specifier.local),
                    ));
                }
                reexports.push(StmtReexport {
                    source: decl.source.value.as_str().to_string(),
                    specs,
                    is_star: false,
                });
            }
            Statement::ExportAllDeclaration(decl) => {
                let exported = decl
                    .exported
                    .as_ref()
                    .map(|name| (export_name_of(name), "*".to_string()));
                reexports.push(StmtReexport {
                    source: decl.source.value.as_str().to_string(),
                    specs: exported.into_iter().collect(),
                    is_star: decl.exported.is_none(),
                });
            }
            Statement::ExportDefaultDeclaration(decl) => {
                has_default_export = true;
                default_decl = match &decl.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        func.id.as_ref().map(|id| id.name.as_str().to_string())
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        class.id.as_ref().map(|id| id.name.as_str().to_string())
                    }
                    _ => None,
                };
            }
            _ => {}
        }
    }
    let mut declared = HashSet::new();
    for (name, _) in scoping.get_bindings(root) {
        let name = name.as_str().to_string();
        if !imported.contains(&name) {
            declared.insert(name);
        }
    }
    Ok(Some(Analyzed {
        declared,
        imported,
        imports,
        reexports,
        default_decl,
        has_default_export,
    }))
}

/// Binding prefix for member `index`.
fn prefix(index: usize) -> String {
    format!("$f{index}$")
}

/// Final renamed name of member `target`'s default export.
fn default_final_name(target: usize, parsed: &[Analyzed]) -> String {
    match parsed[target].default_decl.as_deref() {
        Some(decl) => format!("{}{decl}", prefix(target)),
        None => format!("{}$default", prefix(target)),
    }
}

/// Resolve inside import aliases for member `index`: imported local →
/// final renamed target binding. `None` = ineligible.
fn resolve_aliases(
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
fn local_decl_of(module: &ConcatModule, parsed: &Analyzed, exported: &str) -> Option<String> {
    let detail = module
        .shake
        .exports
        .iter()
        .find(|export| export.from.is_none() && export.exported == exported)?;
    let local = detail.local.as_deref()?;
    parsed.declared.contains(local).then(|| local.to_string())
}

/// One emitted member.
struct Emitted {
    /// Rewritten code (ends without a trailing newline guarantee).
    code: String,
    /// Fresh map JSON (`code` → loaded code).
    map: Option<String>,
    /// Final name of this member's default export, when it has one.
    default_name: Option<String>,
}

/// Emit member `index`: rename top-level bindings, dissolve inside
/// imports/exports, convert the default export. `None` = ineligible.
fn emit_module(
    module: &ConcatModule,
    index: usize,
    modules: &[ConcatModule],
    parsed: &[Analyzed],
    aliases: &[HashMap<String, String>],
    index_of: &HashMap<&str, usize>,
) -> Result<Option<Emitted>> {
    let allocator = Allocator::default();
    let parsed_ast = Parser::new(&allocator, &module.code, SourceType::mjs()).parse();
    if parsed_ast.fatal_error {
        return Err(FerriteError::Parse {
            id: module.id.clone(),
            message: "concat member does not parse".to_string(),
            frame: None,
        });
    }
    let mut program = parsed_ast.program;
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let root = scoping.root_scope_id();
    // Symbol → final name: declared names get prefixed, imported locals
    // follow their alias (inside) or prefix (outside-kept).
    let mut rename: HashMap<SymbolId, String> = HashMap::new();
    for (name, symbol_id) in scoping.get_bindings(root) {
        let name = name.as_str();
        if let Some(alias) = aliases[index].get(name) {
            rename.insert(*symbol_id, alias.clone());
        } else if parsed[index].declared.contains(name) {
            rename.insert(*symbol_id, format!("{}{name}", prefix(index)));
        } else {
            // Imported local without an alias (unresolved edge): prefix.
            rename.insert(*symbol_id, format!("{}{name}", prefix(index)));
        }
    }
    struct Renamer<'s, 'a> {
        scoping: &'s oxc_semantic::Scoping,
        rename: &'s HashMap<SymbolId, String>,
        allocator: &'a Allocator,
    }
    impl<'s, 'a> VisitMut<'a> for Renamer<'s, 'a> {
        fn visit_identifier_reference(&mut self, it: &mut IdentifierReference<'a>) {
            if let Some(reference_id) = it.reference_id.get() {
                if let Some(symbol_id) = self.scoping.get_reference(reference_id).symbol_id() {
                    if let Some(name) = self.rename.get(&symbol_id) {
                        it.name = Ident::from_in(name.clone(), self.allocator);
                    }
                }
            }
        }
        fn visit_binding_identifier(&mut self, it: &mut BindingIdentifier<'a>) {
            if let Some(symbol_id) = it.symbol_id.get() {
                if let Some(name) = self.rename.get(&symbol_id) {
                    it.name = Ident::from_in(name.clone(), self.allocator);
                }
            }
        }
    }
    let mut renamer = Renamer {
        scoping: &scoping,
        rename: &rename,
        allocator: &allocator,
    };
    renamer.visit_program(&mut program);
    // Statement surgery: dissolve inside edges, convert exports.
    let edge_of = |source: &str| {
        module
            .imports
            .iter()
            .find(|edge| edge.specifier == source)
            .map(|edge| (edge.target.as_str(), edge.is_static))
    };
    let inside = |source: &str| {
        edge_of(source)
            .is_some_and(|(target, is_static)| is_static && index_of.contains_key(target))
    };
    let mut default_name: Option<String> = None;
    let mut index_stmt = 0;
    while index_stmt < program.body.len() {
        enum Action {
            Keep,
            Drop,
            UnwrapExport,
            ConvertDefault,
        }
        let action = match &program.body[index_stmt] {
            Statement::ImportDeclaration(decl) => {
                if inside(decl.source.value.as_str()) {
                    Action::Drop
                } else {
                    Action::Keep
                }
            }
            // `export const …` / `export function …`: unwrap.
            Statement::ExportDeclaration(_) => Action::UnwrapExport,
            // `export { a };`: dissolved (entry block re-exports).
            Statement::ExportNamedDeclaration(_) => Action::Drop,
            Statement::ExportFromDeclaration(decl) => {
                if inside(decl.source.value.as_str()) {
                    // Non-entry members bailed in `resolve_aliases`; the
                    // entry expands these in its export block instead.
                    Action::Drop
                } else {
                    Action::Keep
                }
            }
            Statement::ExportAllDeclaration(decl) => {
                if decl.exported.is_some() || !inside(decl.source.value.as_str()) {
                    // `export * as ns` / outside star: kept (`* as ns`
                    // from inside bailed via alias validation).
                    if decl.exported.is_some() && inside(decl.source.value.as_str()) {
                        return Ok(None);
                    }
                    Action::Keep
                } else {
                    Action::Drop
                }
            }
            Statement::ExportDefaultDeclaration(_) => Action::ConvertDefault,
            _ => Action::Keep,
        };
        match action {
            Action::Keep => {
                index_stmt += 1;
            }
            Action::Drop => {
                program.body.remove(index_stmt);
            }
            Action::UnwrapExport => {
                // TS-only declarations in loaded JS: bail (fail safe).
                if let Statement::ExportDeclaration(decl) = &program.body[index_stmt] {
                    if !matches!(
                        decl.declaration,
                        Declaration::VariableDeclaration(_)
                            | Declaration::FunctionDeclaration(_)
                            | Declaration::ClassDeclaration(_)
                    ) {
                        return Ok(None);
                    }
                }
                program.body[index_stmt].replace_with(|stmt| match stmt {
                    Statement::ExportDeclaration(decl) => match decl.unbox().declaration {
                        Declaration::VariableDeclaration(var) => {
                            Statement::VariableDeclaration(var)
                        }
                        Declaration::FunctionDeclaration(func) => {
                            Statement::FunctionDeclaration(func)
                        }
                        Declaration::ClassDeclaration(class) => Statement::ClassDeclaration(class),
                        _ => unreachable!("bailed above"),
                    },
                    other => other,
                });
                index_stmt += 1;
            }
            Action::ConvertDefault => {
                // TS-only defaults in loaded JS: bail (fail safe).
                if let Statement::ExportDefaultDeclaration(decl) = &program.body[index_stmt] {
                    if matches!(
                        decl.declaration,
                        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
                    ) {
                        return Ok(None);
                    }
                }
                let final_name = default_final_name(index, parsed);
                default_name = Some(final_name.clone());
                let new_name = Ident::from_in(final_name, &allocator);
                let ast_builder = oxc_ast::builder::AstBuilder::new(&allocator);
                program.body[index_stmt].replace_with(|stmt| match stmt {
                    Statement::ExportDefaultDeclaration(decl) => {
                        let span = decl.span;
                        match decl.unbox().declaration {
                            ExportDefaultDeclarationKind::FunctionDeclaration(mut func) => {
                                match func.id.as_mut() {
                                    Some(id) => {
                                        id.name = new_name;
                                    }
                                    None => {
                                        func.id = Some(BindingIdentifier::new(
                                            span,
                                            new_name,
                                            &ast_builder,
                                        ));
                                    }
                                }
                                Statement::FunctionDeclaration(func)
                            }
                            ExportDefaultDeclarationKind::ClassDeclaration(mut class) => {
                                match class.id.as_mut() {
                                    Some(id) => {
                                        id.name = new_name;
                                    }
                                    None => {
                                        class.id = Some(BindingIdentifier::new(
                                            span,
                                            new_name,
                                            &ast_builder,
                                        ));
                                    }
                                }
                                Statement::ClassDeclaration(class)
                            }
                            kind => default_expr_to_const(kind, span, new_name, &ast_builder),
                        }
                    }
                    other => other,
                });
                index_stmt += 1;
            }
        }
    }
    let _ = modules;
    let generated = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(std::path::PathBuf::from(&module.id)),
            ..Default::default()
        })
        .build(&program);
    let map = generated.map.map(|map| map.to_json_string());
    Ok(Some(Emitted {
        code: generated.code,
        map,
        default_name,
    }))
}

/// Entry export block text + preserved names.
struct ExportBlock {
    /// `export { … };` text (empty when the entry exports nothing local).
    code: String,
    /// Preserved export names.
    exports: Vec<String>,
}

/// Build the entry's export block, expanding inside barrels.
/// `None` = ineligible.
fn entry_export_block(
    entry: &ConcatModule,
    entry_index: usize,
    modules: &[ConcatModule],
    parsed: &[Analyzed],
    default_names: &[Option<String>],
    index_of: &HashMap<&str, usize>,
) -> Result<Option<ExportBlock>> {
    let edge_of = |source: &str| {
        entry
            .imports
            .iter()
            .find(|edge| edge.specifier == source)
            .map(|edge| (edge.target.as_str(), edge.is_static))
    };
    // (`exported`, final local name), in first-seen order.
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |exported: String, local: String| {
        if seen.insert(exported.clone()) {
            pairs.push((exported, local));
        }
    };
    // Local exports (including the default conversion).
    for export in &entry.shake.exports {
        if export.from.is_some() {
            continue;
        }
        if export.exported == "default" {
            let Some(name) = default_names[entry_index].as_deref() else {
                continue;
            };
            push("default".to_string(), name.to_string());
            continue;
        }
        let Some(local) = export.local.as_deref() else {
            return Ok(None);
        };
        // The local may be declared or imported-from-outside (kept
        // import, prefixed local) — never imported-from-inside (those
        // dissolved; the name resolves through the member instead).
        let known = parsed[entry_index].declared.contains(local)
            || parsed[entry_index].imported.contains(local);
        if !known {
            return Ok(None);
        }
        let final_local = format!("{}{local}", prefix(entry_index));
        push(export.exported.clone(), final_local);
    }
    // Inside barrels expand; outside re-exports were kept as statements.
    for reexport in &parsed[entry_index].reexports {
        let (target_id, is_static) = match edge_of(&reexport.source) {
            Some(edge) => edge,
            None => continue,
        };
        if !is_static {
            continue;
        }
        let Some(target) = index_of.get(target_id).copied() else {
            continue;
        };
        if reexport.is_star {
            // `export *` leaks every local except `default`; explicit
            // local exports win on conflicts.
            for export in &modules[target].shake.exports {
                if export.from.is_some() || export.exported == "default" {
                    continue;
                }
                let Some(local) =
                    local_decl_of(&modules[target], &parsed[target], &export.exported)
                else {
                    return Ok(None);
                };
                push(
                    export.exported.clone(),
                    format!("{}{local}", prefix(target)),
                );
            }
            continue;
        }
        for (exported, imported) in &reexport.specs {
            if imported == "*" {
                // `export * as ns` from inside needs a namespace object.
                return Ok(None);
            }
            let final_local = if imported == "default" {
                match default_names[target].as_deref() {
                    Some(name) => name.to_string(),
                    None => return Ok(None),
                }
            } else {
                let Some(local) = local_decl_of(&modules[target], &parsed[target], imported) else {
                    return Ok(None);
                };
                format!("{}{local}", prefix(target))
            };
            push(exported.clone(), final_local);
        }
    }
    if pairs.is_empty() {
        return Ok(Some(ExportBlock {
            code: String::new(),
            exports: Vec::new(),
        }));
    }
    let mut code = String::from("export { ");
    let mut exports = Vec::with_capacity(pairs.len());
    for (position, (exported, local)) in pairs.iter().enumerate() {
        if position > 0 {
            code.push_str(", ");
        }
        code.push_str(&format!("{local} as {exported}"));
        exports.push(exported.clone());
    }
    code.push_str(" };\n");
    Ok(Some(ExportBlock { code, exports }))
}

/// Chain each piece through its input map, then concatenate with the
/// recorded line offsets.
fn concat_maps(
    pieces: &[Emitted],
    line_starts: &[u32],
    modules: &[ConcatModule],
) -> Result<String> {
    let mut chained: Vec<String> = Vec::with_capacity(pieces.len());
    for (piece, module) in pieces.iter().zip(modules) {
        let Some(fresh) = piece.map.as_deref() else {
            // Member without a map (should not happen when the loader
            // maps everything): fall back to the input map verbatim.
            chained.push(
                module.input_map.clone().unwrap_or_else(|| {
                    "{\"version\":3,\"sources\":[],\"mappings\":\"\"}".to_string()
                }),
            );
            continue;
        };
        match module.input_map.as_deref() {
            Some(input) if !input.is_empty() => {
                chained.push(crate::chain_source_maps(fresh, input)?);
            }
            _ => chained.push(fresh.to_string()),
        }
    }
    let decoded: Vec<oxc_sourcemap::SourceMap<'_>> = chained
        .iter()
        .map(|json| {
            oxc_sourcemap::SourceMap::from_json_string(json)
                .map_err(|error| FerriteError::Other(format!("bad concat member map: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let pairs: Vec<(&oxc_sourcemap::SourceMap<'_>, u32)> =
        decoded.iter().zip(line_starts.iter().copied()).collect();
    let builder = oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&pairs);
    Ok(builder.into_owned_sourcemap().to_json_string())
}

/// `export default <expr>` → `const $name = <expr>;`.
fn default_expr_to_const<'a>(
    kind: ExportDefaultDeclarationKind<'a>,
    span: Span,
    name: Ident<'a>,
    builder: &oxc_ast::builder::AstBuilder<'a>,
) -> Statement<'a> {
    let Ok(expr) = Expression::try_from(kind) else {
        // Functions, classes, and interfaces are handled by the caller.
        unreachable!("non-expression default kind");
    };
    let declarator = VariableDeclarator::new(
        span,
        BindingPattern::BindingIdentifier(BindingIdentifier::boxed(span, name, builder)),
        None,
        Some(expr),
        false,
        builder,
    );
    Statement::VariableDeclaration(VariableDeclaration::boxed(
        span,
        VariableDeclarationKind::Const,
        [declarator],
        false,
        builder,
    ))
}

#[cfg(test)]
mod tests {
    use ferrite_core::ModuleType;

    use super::*;
    use crate::parse_module;

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
