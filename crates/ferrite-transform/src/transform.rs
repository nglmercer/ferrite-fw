//! Module transformation.

use crate::minify::*;
use crate::parse::*;
use crate::rewrite::*;
use crate::types::*;
use ferrite_core::FerriteError;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use ferrite_core::SourceMap;
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_codegen::CodegenOptions;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_transformer::JsxRuntime;
use oxc_transformer::TransformOptions;
use oxc_transformer::Transformer;
use std::path::PathBuf;

/// Transform a module: TS strip + JSX + defines + optional minify/map.
pub(crate) fn transform_module(request: TransformRequest) -> Result<TransformResult> {
    // JSON modules become ESM.
    if request.module_type == ModuleType::Json {
        let value: serde_json::Value =
            serde_json::from_str(&request.code).map_err(|error| FerriteError::Parse {
                id: request.id.clone(),
                message: format!("invalid JSON: {error}"),
                frame: None,
            })?;
        let code = format!(
            "export default {};\n",
            serde_json::to_string(&value).unwrap_or_else(|_| "null".to_string())
        );
        return Ok(TransformResult {
            code,
            map: None,
            dependencies: Vec::new(),
            imports: Vec::new(),
            exports: vec!["default".to_string()],
        });
    }
    // Non-JS modules pass through (CSS/assets have dedicated pipelines).
    if !request.module_type.is_js_like() {
        return Ok(TransformResult {
            code: request.code.clone(),
            map: None,
            dependencies: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
        });
    }

    let needs_transform = matches!(
        request.module_type,
        ModuleType::Ts | ModuleType::Tsx | ModuleType::Jsx
    );
    let (mut code, mut map) = if needs_transform {
        transform_js_like(&request)?
    } else {
        // Plain JS: validate with the parser, keep original formatting.
        parse_module(&request.id, &request.code, &request.module_type)?;
        (request.code.clone(), None)
    };
    // Compile-time defines (AST-safe enough: word-boundary replacement).
    if !request.define.is_empty() {
        code = apply_define(&code, &request.define);
    }
    if request.minify {
        let minified = minify_module(&MinifyRequest {
            id: request.id.clone(),
            code,
            sourcemap: request.sourcemap,
            input_map: map,
        })?;
        code = minified.code;
        map = minified.map;
    }
    // Fresh import spans from the final code (§28: AST ranges, not regex).
    let parsed = parse_module(&request.id, &code, &ModuleType::Js)?;
    Ok(TransformResult {
        code,
        map,
        dependencies: Vec::new(),
        imports: parsed.imports,
        exports: parsed.exports,
    })
}

/// Run Oxc semantic analysis + transformer + codegen.
pub(crate) fn transform_js_like(request: &TransformRequest) -> Result<(String, Option<SourceMap>)> {
    let allocator = Allocator::default();
    let source_type = source_type_for(&request.id, &request.module_type);
    let parsed = Parser::new(&allocator, &request.code, source_type).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        let message = parsed
            .diagnostics
            .iter()
            .take(3)
            .map(|diagnostic| diagnostic.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FerriteError::Parse {
            id: request.id.clone(),
            message: if message.is_empty() {
                "syntax error".to_string()
            } else {
                message
            },
            frame: Some(ferrite_core::code_frame(&request.code, 0, 2)),
        });
    }
    let mut program = parsed.program;
    let scoping = SemanticBuilder::new()
        .build(&program)
        .semantic
        .into_scoping();
    let mut options = TransformOptions::default();
    options.jsx.jsx_plugin = true;
    options.jsx.development = request.development;
    options.jsx.runtime = if request.jsx_runtime == "classic" {
        JsxRuntime::Classic
    } else {
        JsxRuntime::Automatic
    };
    let path = PathBuf::from(request.id.split('?').next().unwrap_or(&request.id));
    let transform_return =
        Transformer::new(&allocator, &path, &options).build_with_scoping(scoping, &mut program);
    if !transform_return.diagnostics.is_empty() {
        let message = transform_return
            .diagnostics
            .iter()
            .take(3)
            .map(|diagnostic| diagnostic.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FerriteError::Transform {
            id: request.id.clone(),
            message,
        });
    }
    let codegen_options = CodegenOptions {
        source_map_path: request.sourcemap.then(|| path.clone()),
        ..Default::default()
    };
    let generated = Codegen::new().with_options(codegen_options).build(&program);
    let map = generated
        .map
        .map(|map| SourceMap::external(map.to_json_string()));
    Ok((generated.code, map))
}

/// Drop unused export statements (§38 statement-level DCE).
///
/// `used` holds the exported names live importers need. Unused exports
/// are *unwrapped* (`export const a = …` → `const a = …`, unused
/// specifiers removed); callers must minify afterwards so dead bindings
/// (and only provably pure ones) are collected. Returns `None` when
/// nothing changed, so callers keep the original code and source map.
///
/// Conservative by design: re-exports, default expressions, heritage
/// classes, complex patterns, and mixed multi-declarators are kept.
pub fn drop_unused_exports(
    id: &str,
    code: &str,
    used: &std::collections::HashSet<String>,
) -> Result<Option<String>> {
    use oxc_allocator::ReplaceWith as _;
    use oxc_ast::ast::*;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if parsed.fatal_error {
        return Err(FerriteError::Parse {
            id: id.to_string(),
            message: "syntax error".to_string(),
            frame: Some(ferrite_core::code_frame(code, 0, 2)),
        });
    }
    let mut program = parsed.program;
    let mut changed = false;
    let mut index = 0;
    while index < program.body.len() {
        // Decide by reference first so untouched statements are never moved.
        enum Action {
            Keep,
            DropSpecifiers,
            UnwrapExport,
            UnwrapDefaultFunction,
            DropStatement,
        }
        let action = match &mut program.body[index] {
            // `export { a as b, c };` — drop unused specifiers.
            Statement::ExportNamedDeclaration(decl) => {
                let before = decl.specifiers.len();
                decl.specifiers
                    .retain(|spec| used.contains(&module_export_name(&spec.exported)));
                if decl.specifiers.is_empty() {
                    Action::DropStatement
                } else if decl.specifiers.len() != before {
                    Action::DropSpecifiers
                } else {
                    Action::Keep
                }
            }
            // `export const/function/class …` — unwrap when fully unused.
            Statement::ExportDeclaration(decl) => match &decl.declaration {
                Declaration::VariableDeclaration(var) => {
                    if declarator_names(&var.declarations)
                        .is_some_and(|names| names.iter().all(|name| !used.contains(name)))
                    {
                        Action::UnwrapExport
                    } else {
                        Action::Keep
                    }
                }
                Declaration::FunctionDeclaration(fun) => {
                    if fun
                        .id
                        .as_ref()
                        .is_some_and(|id| !used.contains(id.name.as_str()))
                    {
                        Action::UnwrapExport
                    } else {
                        Action::Keep
                    }
                }
                Declaration::ClassDeclaration(class) => {
                    if class
                        .id
                        .as_ref()
                        .is_some_and(|id| !used.contains(id.name.as_str()))
                    {
                        Action::UnwrapExport
                    } else {
                        Action::Keep
                    }
                }
                _ => Action::Keep,
            },
            // `export default …` — unwrap named functions, drop anonymous
            // ones, keep everything else (heritage/expressions may run).
            Statement::ExportDefaultDeclaration(decl) => {
                if used.contains("default") {
                    Action::Keep
                } else if let ExportDefaultDeclarationKind::FunctionDeclaration(fun) =
                    &decl.declaration
                {
                    if fun.id.is_some() {
                        Action::UnwrapDefaultFunction
                    } else {
                        Action::DropStatement
                    }
                } else {
                    Action::Keep
                }
            }
            // Re-exports, star exports, plain statements: keep.
            _ => Action::Keep,
        };
        match action {
            Action::Keep => index += 1,
            Action::DropSpecifiers => {
                changed = true;
                index += 1;
            }
            Action::DropStatement => {
                program.body.remove(index);
                changed = true;
            }
            Action::UnwrapExport => {
                changed = true;
                program.body[index].replace_with(|stmt| match stmt {
                    Statement::ExportDeclaration(decl) => match decl.unbox().declaration {
                        Declaration::VariableDeclaration(var) => {
                            Statement::VariableDeclaration(var)
                        }
                        Declaration::FunctionDeclaration(fun) => {
                            Statement::FunctionDeclaration(fun)
                        }
                        Declaration::ClassDeclaration(class) => Statement::ClassDeclaration(class),
                        // Unreachable: the decision gate selects only these three.
                        _ => unreachable!("shake decision/action mismatch"),
                    },
                    // Unreachable: same gate.
                    other => other,
                });
                index += 1;
            }
            Action::UnwrapDefaultFunction => {
                changed = true;
                program.body[index].replace_with(|stmt| match stmt {
                    Statement::ExportDefaultDeclaration(decl) => {
                        match decl.unbox().declaration {
                            ExportDefaultDeclarationKind::FunctionDeclaration(fun) => {
                                Statement::FunctionDeclaration(fun)
                            }
                            // Unreachable: the decision gate selects only functions.
                            _ => unreachable!("shake decision/action mismatch"),
                        }
                    }
                    // Unreachable: same gate.
                    other => other,
                });
                index += 1;
            }
        }
    }
    if !changed {
        return Ok(None);
    }
    Ok(Some(
        Codegen::new()
            .with_options(CodegenOptions::default())
            .build(&program)
            .code,
    ))
}

/// Exported-name text of a module export name.
pub(crate) fn module_export_name(name: &oxc_ast::ast::ModuleExportName<'_>) -> String {
    match name {
        oxc_ast::ast::ModuleExportName::IdentifierName(name) => name.name.as_str().to_string(),
        oxc_ast::ast::ModuleExportName::IdentifierReference(name) => name.name.as_str().to_string(),
        oxc_ast::ast::ModuleExportName::StringLiteral(name) => name.value.as_str().to_string(),
    }
}

/// Bound top-level names of declarators (`None` unless every pattern is
/// a single identifier — complex patterns stay conservative).
pub(crate) fn declarator_names(
    declarators: &[oxc_ast::ast::VariableDeclarator<'_>],
) -> Option<Vec<String>> {
    declarators
        .iter()
        .map(|declarator| match &declarator.id {
            oxc_ast::ast::BindingPattern::BindingIdentifier(ident) => {
                Some(ident.name.as_str().to_string())
            }
            _ => None,
        })
        .collect()
}
