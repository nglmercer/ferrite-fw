//! Concat member emission.

use crate::concat::analyze::*;
use crate::concat::maps::*;
use crate::concat::rename::*;
use crate::concat::types::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use oxc_allocator::Allocator;
use oxc_allocator::FromIn as _;
use oxc_allocator::ReplaceWith as _;
use oxc_ast::ast::*;
use oxc_ast_visit::VisitMut;
use oxc_codegen::Codegen;
use oxc_codegen::CodegenOptions;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_semantic::SymbolId;
use oxc_span::SourceType;
use std::collections::HashMap;
use std::collections::HashSet;

/// One emitted member.
pub(crate) struct Emitted {
    /// Rewritten code (ends without a trailing newline guarantee).
    pub(crate) code: String,
    /// Fresh map JSON (`code` → loaded code).
    pub(crate) map: Option<String>,
    /// Final name of this member's default export, when it has one.
    pub(crate) default_name: Option<String>,
}

/// Emit member `index`: rename top-level bindings, dissolve inside
/// imports/exports, convert the default export. `None` = ineligible.
pub(crate) fn emit_module(
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
pub(crate) struct ExportBlock {
    /// `export { … };` text (empty when the entry exports nothing local).
    pub(crate) code: String,
    /// Preserved export names.
    pub(crate) exports: Vec<String>,
}

/// Build the entry's export block, expanding inside barrels.
/// `None` = ineligible.
pub(crate) fn entry_export_block(
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
