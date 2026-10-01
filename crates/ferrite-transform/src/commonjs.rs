//! AST-based CommonJS analysis and lazy ESM factories.
//!
//! Named exports are snapshots of statically discoverable properties. Factories
//! are separate from ESM facades so importing a dependency does not execute its
//! CommonJS body before the corresponding require call.

use ferrite_core::{FerriteError, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::SourceType;
use std::collections::{BTreeMap, BTreeSet};

pub const CJS_FACTORY_QUERY: &str = "ferrite-cjs-factory";
pub const CJS_REQUIRE_EXPORT: &str = "__ferrite_cjs_require__";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CommonJsAnalysis {
    pub is_commonjs: bool,
    pub requires: BTreeSet<String>,
    pub named_exports: BTreeSet<String>,
    pub reexports: BTreeSet<String>,
    /// Unsupported global require references, with source byte locations.
    pub unsupported: Vec<(usize, String)>,
}

pub fn analyze_commonjs(id: &str, code: &str) -> Result<CommonJsAnalysis> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        return Err(FerriteError::Parse {
            id: id.into(),
            message: "CommonJS analysis requires valid lowered JavaScript".into(),
            frame: None,
        });
    }
    let scoping = SemanticBuilder::new()
        .build(&parsed.program)
        .semantic
        .into_scoping();
    let mut visitor = Analyzer {
        scoping: &scoping,
        analysis: CommonJsAnalysis::default(),
        allowed_require: BTreeSet::new(),
        require_refs: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    for span in visitor.require_refs {
        if !visitor.allowed_require.contains(&span) {
            visitor.analysis.unsupported.push((
                span,
                "aliased or indirect require; use a literal require('package') or an ESM import"
                    .into(),
            ));
        }
    }
    visitor.analysis.named_exports.remove("default");
    visitor.analysis.named_exports.remove(CJS_REQUIRE_EXPORT);
    Ok(visitor.analysis)
}

struct Analyzer<'s> {
    scoping: &'s Scoping,
    analysis: CommonJsAnalysis,
    allowed_require: BTreeSet<usize>,
    require_refs: Vec<usize>,
}
impl Analyzer<'_> {
    fn global(&self, expr: &Expression<'_>, name: &str) -> bool {
        match expr.without_parentheses() {
            Expression::Identifier(identifier) if identifier.name == name => {
                self.global_id(identifier)
            }
            _ => false,
        }
    }
    fn global_id(&self, identifier: &IdentifierReference<'_>) -> bool {
        identifier
            .reference_id
            .get()
            .is_some_and(|id| self.scoping.get_reference(id).symbol_id().is_none())
    }
    fn exports(&self, expr: &Expression<'_>) -> bool {
        self.global(expr, "exports")
            || expr.as_member_expression().is_some_and(|member| {
                self.global(member.object(), "module")
                    && member
                        .static_property_name()
                        .is_some_and(|name| name == "exports")
            })
    }
    fn require_specifier(&self, expr: &Expression<'_>) -> Option<String> {
        let Expression::CallExpression(call) = expr.without_parentheses() else {
            return None;
        };
        if !self.global(&call.callee, "require") || call.arguments.len() != 1 {
            return None;
        }
        match &call.arguments[0] {
            Argument::StringLiteral(value) => Some(value.value.to_string()),
            _ => None,
        }
    }
    fn names_from_value(&mut self, expr: &Expression<'_>) {
        if let Expression::ObjectExpression(object) = expr.without_parentheses() {
            for property in &object.properties {
                if let ObjectPropertyKind::ObjectProperty(property) = property {
                    if !property.computed || matches!(property.key, PropertyKey::StringLiteral(_)) {
                        if let Some(name) = property.key.static_name() {
                            self.analysis.named_exports.insert(name.into_owned());
                        }
                    }
                }
            }
        }
        if let Some(specifier) = self.require_specifier(expr) {
            self.analysis.reexports.insert(specifier);
        }
    }
}
impl<'a> Visit<'a> for Analyzer<'_> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if self.global_id(identifier) {
            match identifier.name.as_str() {
                "module" | "exports" => self.analysis.is_commonjs = true,
                "require" => {
                    self.analysis.is_commonjs = true;
                    self.require_refs.push(identifier.span.start as usize);
                }
                _ => (),
            }
        }
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.global(&call.callee, "require") {
            if let Expression::Identifier(identifier) = call.callee.without_parentheses() {
                self.allowed_require.insert(identifier.span.start as usize);
            }
            if call.optional || call.arguments.len() != 1 {
                self.analysis.unsupported.push((
                    call.span.start as usize,
                    "require needs exactly one static string argument".into(),
                ));
            } else if let Argument::StringLiteral(specifier) = &call.arguments[0] {
                if specifier.value.ends_with(".node") {
                    self.analysis.unsupported.push((call.span.start as usize, "native .node addons are unavailable in the browser/embedded module pipeline; use an explicitly configured Node runtime".into()));
                }
                self.analysis.requires.insert(specifier.value.to_string());
            } else {
                self.analysis.unsupported.push((call.span.start as usize, "dynamic require is unavailable; use a literal require('package') or an ESM import".into()));
            }
        }
        if call.callee.get_member_expr().is_some_and(|member| {
            self.global(member.object(), "module")
                && member
                    .static_property_name()
                    .is_some_and(|name| name == "require")
        }) {
            self.analysis.unsupported.push((
                call.span.start as usize,
                "module.require is unavailable; use a literal require('package') or an ESM import"
                    .into(),
            ));
        }
        if call
            .callee
            .is_specific_member_access("Object", "defineProperty")
            && call
                .callee
                .get_member_expr()
                .is_some_and(|member| self.global(member.object(), "Object"))
        {
            if let (Some(object), Some(Argument::StringLiteral(name))) = (
                call.arguments.first().and_then(Argument::as_expression),
                call.arguments.get(1),
            ) {
                if self.exports(object) {
                    self.analysis.named_exports.insert(name.value.to_string());
                }
            }
        }
        if call.callee.is_specific_member_access("Object", "assign")
            && call
                .callee
                .get_member_expr()
                .is_some_and(|member| self.global(member.object(), "Object"))
            && call
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .is_some_and(|object| self.exports(object))
        {
            for value in call
                .arguments
                .iter()
                .skip(1)
                .filter_map(Argument::as_expression)
            {
                self.names_from_value(value);
            }
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let Some(member) = assignment.left.as_member_expression() {
            if self.exports(member.object()) {
                if let Some(name) = member.static_property_name() {
                    self.analysis.named_exports.insert(name.to_string());
                }
            } else if self.global(member.object(), "module")
                && member
                    .static_property_name()
                    .is_some_and(|name| name == "exports")
            {
                self.names_from_value(&assignment.right);
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_unary_expression(&mut self, expr: &UnaryExpression<'a>) {
        if expr.operator == oxc_syntax::operator::UnaryOperator::Typeof
            && self.global(&expr.argument, "require")
        {
            if let Expression::Identifier(identifier) = expr.argument.without_parentheses() {
                self.allowed_require.insert(identifier.span.start as usize);
            }
        }
        walk::walk_unary_expression(self, expr);
    }
}

pub fn validate_commonjs(id: &str, code: &str, analysis: &CommonJsAnalysis) -> Result<()> {
    if let Some((offset, reason)) = analysis.unsupported.first() {
        return Err(FerriteError::Transform {
            id: id.into(),
            message: format!("{reason}\n{}", ferrite_core::code_frame(code, *offset, 2)),
        });
    }
    Ok(())
}

pub fn commonjs_factory_id(id: &str) -> String {
    let separator = if id.contains('?') { '&' } else { '?' };
    format!("{id}{separator}{CJS_FACTORY_QUERY}")
}

/// Wrap the original body without touching comments, strings or scoped calls.
/// `dependencies` maps literal require strings to factory URLs.
pub fn commonjs_factory(
    id: &str,
    code: &str,
    dependencies: &BTreeMap<String, String>,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let mut prefix = String::new();
    let mut cases = String::new();
    for (index, (specifier, target)) in dependencies.iter().enumerate() {
        let specifier = serde_json::to_string(specifier)?;
        let target = serde_json::to_string(target)?;
        prefix.push_str(&format!(
            "import * as __ferrite_dep{index} from {target};\n"
        ));
        cases.push_str(&format!(
            "case {specifier}: return __ferrite_dep{index}.{CJS_REQUIRE_EXPORT}();\n"
        ));
    }
    let file = serde_json::to_string(id.split('?').next().unwrap_or(id))?;
    let dir = serde_json::to_string(
        id.split('?')
            .next()
            .unwrap_or(id)
            .rsplit_once('/')
            .map_or("/", |(dir, _)| dir),
    )?;
    prefix.push_str(&format!("var __ferrite_cache;\nexport function {CJS_REQUIRE_EXPORT}() {{\nif (__ferrite_cache) return __ferrite_cache.exports;\nvar module = __ferrite_cache = {{ exports: {{ }}, id: {file}, filename: {file}, loaded: false }};\nfunction require(specifier) {{ switch (specifier) {{\n{cases}}}\nthrow new Error('Unsupported require in ' + {file} + ': ' + specifier); }}\ntry {{ (function(module, exports, require, __filename, __dirname) {{\n"));
    let suffix = format!("\n}}).call(module.exports, module, module.exports, require, {file}, {dir});\nmodule.loaded = true; return module.exports;\n}} catch (error) {{ __ferrite_cache = undefined; throw error; }}\n}}\n");
    let start = if code.starts_with("#!") {
        code.find('\n').unwrap_or(code.len())
    } else {
        0
    };
    crate::apply_text_edits(
        id.split('?').next().unwrap_or(id),
        code,
        &[(0, start, prefix), (code.len(), code.len(), suffix)],
        sourcemap,
    )
}

fn commonjs_exports(names: &BTreeSet<String>) -> Result<String> {
    let mut output = format!("const __ferrite_value = {CJS_REQUIRE_EXPORT}();\nexport default __ferrite_value;\nexport {{ __ferrite_value as 'module.exports' }};\n");
    for (index, name) in names
        .iter()
        .filter(|name| {
            name.as_str() != "default"
                && name.as_str() != "module.exports"
                && name.as_str() != CJS_REQUIRE_EXPORT
        })
        .enumerate()
    {
        let name = serde_json::to_string(name)?;
        output.push_str(&format!("const __ferrite_export{index} = __ferrite_value == null ? undefined : __ferrite_value[{name}];\nexport {{ __ferrite_export{index} as {name} }};\n"));
    }
    Ok(output)
}

pub fn commonjs_facade(
    id: &str,
    code: &str,
    names: &BTreeSet<String>,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let target = serde_json::to_string(&commonjs_factory_id(id))?;
    let output = format!(
        "import {{ {CJS_REQUIRE_EXPORT} }} from {target};\n{}",
        commonjs_exports(names)?
    );
    crate::apply_text_edits(id, code, &[(0, code.len(), output)], sourcemap)
}

/// Transform APIs receiving source text keep the source body in their output.
/// There is no invented filesystem resource for code supplied by the caller.
pub fn commonjs_inline(
    id: &str,
    code: &str,
    names: &BTreeSet<String>,
    dependencies: &BTreeMap<String, String>,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let (factory, input_map) = commonjs_factory(id, code, dependencies, sourcemap)?;
    let (output, outer_map) = crate::apply_text_edits(
        id,
        &factory,
        &[(factory.len(), factory.len(), commonjs_exports(names)?)],
        sourcemap,
    )?;
    let map = match (outer_map, input_map) {
        (Some(outer), Some(inner)) => Some(crate::chain_source_maps(&outer, &inner)?),
        _ => None,
    };
    Ok((output, map))
}

/// JSON's already-lowered single default expression can be a synchronous factory.
pub fn commonjs_json_factory(
    id: &str,
    code: &str,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    let statement = parsed
        .program
        .body
        .first()
        .filter(|_| parsed.program.body.len() == 1);
    let Some(Statement::ExportDefaultDeclaration(declaration)) = statement else {
        return Err(FerriteError::Transform { id: id.into(), message: "require(JSON) needs a single lowered default expression; a plugin changed its module contract".into() });
    };
    let Some(expression) = declaration.declaration.as_expression() else {
        return Err(FerriteError::Transform {
            id: id.into(),
            message: "require(JSON) cannot wrap a declaration".into(),
        });
    };
    use oxc_span::GetSpan;
    let span = expression.span();
    crate::apply_text_edits(
        id,
        code,
        &[
            (0, span.start as usize, "const __ferrite_json = ".into()),
            (
                span.end as usize,
                code.len(),
                format!(";\nexport function {CJS_REQUIRE_EXPORT}() {{ return __ferrite_json; }}\n"),
            ),
        ],
        sourcemap,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analyzes_ast_and_scope_instead_of_strings() {
        let source = r#"
        // require('ignored-comment'); module.exports.nope = 1;
        const text = "require('ignored-string')";
        function local(require, exports, module) { require(dynamic); exports.local = 1; module.exports = {}; }
        const a = require /* space */ ('./a');
        const b = require('\x2e/b');
        exports.answer = a;
        module['exports']['hyphen-name'] = b;
        Object.defineProperty(exports, 'getter', { get() { return 1; } });
        Object.assign(module.exports, { extra: true });
        module.exports = { hello: 1, ['literal']: 2, [dynamic]: 3 };
        "#;
        let facts = analyze_commonjs("/fixture.cjs", source).unwrap();
        assert_eq!(facts.requires, BTreeSet::from(["./a".into(), "./b".into()]));
        assert_eq!(
            facts.named_exports,
            BTreeSet::from(
                [
                    "answer",
                    "hyphen-name",
                    "getter",
                    "extra",
                    "hello",
                    "literal"
                ]
                .map(str::to_owned)
            )
        );
        validate_commonjs("/fixture.cjs", source, &facts).unwrap();
        assert!(
            !analyze_commonjs(
                "/plain.js",
                "const text = 'module.exports'; function f(exports) { exports.x = 1; }"
            )
            .unwrap()
            .is_commonjs
        );
    }
    #[test]
    fn unsupported_require_is_actionable() {
        for source in [
            "module.exports = require(path);",
            "const r = require; r('x');",
            "require.resolve('x');",
            "module.exports = require('./binding.node');",
            "module.require('x');",
        ] {
            let facts = analyze_commonjs("/bad.cjs", source).unwrap();
            let error = validate_commonjs("/bad.cjs", source, &facts)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("require") || error.contains("native"),
                "{error}"
            );
            assert!(error.contains("/bad.cjs"));
        }
    }
    #[test]
    fn reexports_and_factory_maps_are_preserved() {
        let source =
            "#!/usr/bin/env node\nmodule.exports = require('./base');\nexports.answer = 42;";
        let facts = analyze_commonjs("/a.cjs", source).unwrap();
        assert_eq!(facts.reexports, BTreeSet::from(["./base".into()]));
        let (code, map) = commonjs_factory(
            "/a.cjs?ferrite-cjs-factory",
            source,
            &BTreeMap::from([("./base".into(), "/base.cjs?ferrite-cjs-factory".into())]),
            true,
        )
        .unwrap();
        crate::parse::parse_module("/factory.mjs", &code, &ferrite_core::ModuleType::Js).unwrap();
        let map_json = map.unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&map_json).unwrap();
        let generated = code
            .lines()
            .position(|line| line.contains("exports.answer = 42"))
            .unwrap() as u32;
        let token = map
            .get_tokens()
            .find(|token| token.get_dst_line() == generated && token.get_dst_col() == 0)
            .unwrap();
        assert_eq!(token.get_src_line(), 2);
        assert_eq!(
            map.get_source(token.get_source_id().unwrap()),
            Some("/a.cjs")
        );
    }
}
