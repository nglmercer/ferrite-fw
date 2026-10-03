//! Scope-aware compile-time globals with mapped text edits.
use ferrite_core::{FerriteError, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::{Scoping, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use std::collections::HashMap;

/// Replace global expressions, preserving comments, strings, shadowed bindings
/// and property names. Invalid keys/values are actionable configuration errors.
pub fn apply_defines_mapped(
    id: &str,
    code: &str,
    definitions: &HashMap<String, String>,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    if definitions.is_empty() {
        return Ok((code.into(), None));
    }
    let allocator = Allocator::default();
    let mut values = HashMap::new();
    for (key, value) in definitions {
        if key.is_empty() || key.split('.').any(|part| part.is_empty()) {
            return Err(FerriteError::Transform {
                id: id.into(),
                message: format!("invalid compile-time define key {key:?}"),
            });
        }
        let key_expression = Parser::new(&allocator, key, SourceType::mjs()).parse_expression().map_err(|_| FerriteError::Transform { id: id.into(), message: format!("invalid compile-time define key {key:?}; use a global identifier or dotted property path") })?;
        if syntax_path(&key_expression).as_deref() != Some(key.as_str()) {
            return Err(FerriteError::Transform { id: id.into(), message: format!("unsupported compile-time define key {key:?}; use a global identifier or dotted property path") });
        }
        let expression = Parser::new(&allocator, value, SourceType::mjs()).parse_expression().map_err(|_| FerriteError::Transform { id: id.into(), message: format!("compile-time define {key} requires a valid JavaScript expression; quote string values") })?;
        let simple = matches!(
            expression,
            Expression::Identifier(_)
                | Expression::StringLiteral(_)
                | Expression::BooleanLiteral(_)
                | Expression::NullLiteral(_)
                | Expression::NumericLiteral(_)
                | Expression::BigIntLiteral(_)
        );
        values.insert(key.clone(), (value.as_str(), simple));
    }
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        return Err(FerriteError::Parse {
            id: id.into(),
            message: "cannot apply compile-time defines to invalid JavaScript".into(),
            frame: Some(ferrite_core::code_frame(code, 0, 2)),
        });
    }
    let scoping = SemanticBuilder::new()
        .build(&parsed.program)
        .semantic
        .into_scoping();
    let mut visitor = Defines {
        scoping: &scoping,
        values: &values,
        code,
        edits: Vec::new(),
    };
    visitor.visit_program(&parsed.program);
    if visitor.edits.is_empty() {
        return Ok((code.into(), None));
    }
    crate::apply_text_edits(id, code, &visitor.edits, sourcemap)
}
fn syntax_path(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::ImportMeta(_) => Some("import.meta".into()),
        expression => {
            let member = expression.as_member_expression()?;
            let property = member.static_property_name()?;
            if property.contains('.') {
                return None;
            }
            Some(format!("{}.{}", syntax_path(member.object())?, property))
        }
    }
}
struct Defines<'s, 'v> {
    scoping: &'s Scoping,
    values: &'s HashMap<String, (&'v str, bool)>,
    code: &'s str,
    edits: Vec<(usize, usize, String)>,
}
impl Defines<'_, '_> {
    fn path(&self, expression: &Expression<'_>) -> Option<String> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier)
                if identifier
                    .reference_id
                    .get()
                    .is_some_and(|id| self.scoping.get_reference(id).symbol_id().is_none()) =>
            {
                Some(identifier.name.to_string())
            }
            Expression::ImportMeta(_) => Some("import.meta".into()),
            expression => {
                let member = expression.as_member_expression()?;
                let property = member.static_property_name()?;
                // A string property containing dots is a single property, not
                // the dotted path named by a definition.
                if property.contains('.') {
                    return None;
                }
                Some(format!("{}.{}", self.path(member.object())?, property))
            }
        }
    }
}
impl<'a> Visit<'a> for Defines<'_, '_> {
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            if let Some(key) = self.path(&property.value) {
                if let Some((value, simple)) = self.values.get(&key) {
                    let span = property.value.span();
                    let value = if *simple {
                        (*value).to_string()
                    } else {
                        format!("({value})")
                    };
                    self.edits.push((
                        span.start as usize,
                        span.end as usize,
                        format!("{key}: {value}"),
                    ));
                    return;
                }
            }
        }
        walk::walk_object_property(self, property);
    }
    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if let Some(key) = self.path(expression) {
            if let Some((value, simple)) = self.values.get(&key) {
                let span = expression.span();
                let after = self
                    .code
                    .get(span.end as usize..)
                    .unwrap_or_default()
                    .trim_start();
                let replacement = if *simple && !after.starts_with('.') && !after.starts_with("?.")
                {
                    (*value).to_string()
                } else {
                    format!("({value})")
                };
                self.edits
                    .push((span.start as usize, span.end as usize, replacement));
                return;
            }
        }
        walk::walk_expression(self, expression);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_definitions_preserve_strings_comments_shadowing_and_precedence() {
        let definitions = HashMap::from([
            ("process.env.NODE_ENV".into(), "\"production\"".into()),
            ("FLAG".into(), "1 + 2".into()),
        ]);
        let code = "const text = 'process.env.NODE_ENV'; // process.env.NODE_ENV\nfunction local(process) { return process.env.NODE_ENV; } const mode = process.env.NODE_ENV; const value = FLAG * 3; const object = process['env.NODE_ENV'];";
        let (result, map) = apply_defines_mapped("source.js", code, &definitions, true).unwrap();
        assert!(result.contains("'process.env.NODE_ENV'"));
        assert!(result.contains("// process.env.NODE_ENV"));
        assert!(result.contains("return process.env.NODE_ENV"));
        assert!(result.contains("const mode = \"production\""));
        assert!(result.contains("(1 + 2) * 3"));
        assert!(result.contains("process['env.NODE_ENV']"));
        assert!(map.is_some());
    }
    #[test]
    fn adjacent_identifiers_import_meta_and_computed_paths() {
        let definitions = HashMap::from([
            ("A".into(), "1".into()),
            ("import.meta.env.DEV".into(), "false".into()),
            ("process.env.NODE_ENV".into(), "\"development\"".into()),
        ]);
        let (output, _) = apply_defines_mapped("x.js", "f(A,A); A+A; const dev = import.meta.env.DEV; const mode = process['env']['NODE_ENV'];", &definitions, false).unwrap();
        assert!(output.contains("f(1,1); 1+1;"));
        assert!(output.contains("const dev = false"));
        assert!(output.contains("const mode = \"development\""));
        assert!(apply_defines_mapped(
            "x.js",
            "A",
            &HashMap::from([("A".into(), "not quoted text".into())]),
            false
        )
        .is_err());
    }
    #[test]
    fn shorthand_keeps_property_names_and_invalid_keys_fail() {
        let definitions = HashMap::from([("FLAG".into(), "false".into())]);
        let (output, _) = apply_defines_mapped(
            "x.js",
            "const flags = { FLAG }; function local(FLAG) { return { FLAG }; }",
            &definitions,
            false,
        )
        .unwrap();
        assert!(output.contains("{ FLAG: false }"));
        assert!(output.contains("return { FLAG }"));
        assert!(apply_defines_mapped(
            "x.js",
            "FLAG",
            &HashMap::from([("A+B".into(), "true".into())]),
            false
        )
        .is_err());
    }
}
