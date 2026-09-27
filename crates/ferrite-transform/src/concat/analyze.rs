//! Concat member analysis.

use crate::concat::types::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::collections::HashSet;

/// Parsed member: top-level bindings, import locals.
pub(crate) struct Analyzed {
    /// Top-level declared names (excluding import locals).
    pub(crate) declared: HashSet<String>,
    /// Import local names (specifier locals).
    pub(crate) imported: HashSet<String>,
    /// Import statements (`source`, specifiers).
    pub(crate) imports: Vec<StmtImport>,
    /// Re-export statements (`export … from`).
    pub(crate) reexports: Vec<StmtReexport>,
    /// Declared name of a named default export, when present.
    pub(crate) default_decl: Option<String>,
    /// True when any default export exists.
    pub(crate) has_default_export: bool,
}

/// One import statement's specifiers.
pub(crate) struct StmtImport {
    /// Module source text.
    pub(crate) source: String,
    /// (`local`, specifier kind).
    pub(crate) specs: Vec<StmtSpec>,
}

/// Import specifier kinds.
pub(crate) enum StmtSpec {
    /// `import { imported as local }` (`imported == local` unaliased).
    Named { local: String, imported: String },
    /// `import local`.
    Default { local: String },
    /// `import * as local`.
    Namespace { local: String },
}

/// One re-export statement.
pub(crate) struct StmtReexport {
    /// Module source text.
    pub(crate) source: String,
    /// (`exported`, `imported`) pairs; empty for plain `export *`.
    pub(crate) specs: Vec<(String, String)>,
    /// True for plain `export *` (namespace re-exports carry one spec).
    pub(crate) is_star: bool,
}

/// Module export name to string (`default` for default names).
pub(crate) fn export_name_of(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(ident) => ident.name.as_str().to_string(),
        ModuleExportName::IdentifierReference(ident) => ident.name.as_str().to_string(),
        ModuleExportName::StringLiteral(lit) => lit.value.as_str().to_string(),
    }
}

/// Parse + collect top-level binding classes. `None` = ineligible
/// (`eval`, `with`, unparseable member code is an error instead).
pub(crate) fn analyze(module: &ConcatModule) -> Result<Option<Analyzed>> {
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
