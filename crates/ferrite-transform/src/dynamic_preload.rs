//! Wrap real dynamic imports without changing literals, options or source positions.
use crate::apply_text_edits;
use ferrite_core::{FerriteError, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ImportExpression};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashMap;

/// Add CSS-before-import calls. The helper import is generated and unmapped.
pub fn preload_dynamic_imports(
    id: &str,
    code: &str,
    css: &HashMap<String, Vec<String>>,
    helper_url: &str,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let mut binding = "__ferrite_css_preload__".to_string();
    while code.contains(&binding) {
        binding.push('_');
    }
    struct Finder<'m> {
        css: &'m HashMap<String, Vec<String>>,
        binding: &'m str,
        edits: Vec<(usize, usize, String)>,
    }
    impl<'a> Visit<'a> for Finder<'_> {
        fn visit_import_expression(&mut self, expr: &ImportExpression<'a>) {
            let specifier = match &expr.source {
                Expression::StringLiteral(value) => Some(value.value.as_str()),
                Expression::TemplateLiteral(value)
                    if value.expressions.is_empty() && value.quasis.len() == 1 =>
                {
                    value.quasis[0].value.cooked.as_ref().map(|v| v.as_str())
                }
                _ => None,
            };
            if let Some(styles) = specifier
                .and_then(|specifier| self.css.get(specifier))
                .filter(|styles| !styles.is_empty())
            {
                let start = expr.span.start as usize;
                let end = expr.span.end as usize;
                self.edits
                    .push((start, start, format!("{}(()=>", self.binding)));
                self.edits.push((
                    end,
                    end,
                    format!(
                        ",{},import.meta.url)",
                        serde_json::to_string(styles).expect("string vectors serialize")
                    ),
                ));
            }
            oxc_ast_visit::walk::walk_import_expression(self, expr);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(FerriteError::Build(format!(
            "cannot wrap dynamic imports in invalid JavaScript `{id}`"
        )));
    }
    let mut finder = Finder {
        css,
        binding: &binding,
        edits: Vec::new(),
    };
    finder.visit_program(&parsed.program);
    if finder.edits.is_empty() {
        return Ok((code.into(), None));
    }
    finder.edits.push((
        code.len(),
        code.len(),
        format!(
            "\nimport {{preload as {binding}}} from {};\n",
            serde_json::to_string(helper_url)?
        ),
    ));
    apply_text_edits(id, code, &finder.edits, sourcemap)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps_only_real_literal_imports_preserving_options_and_map_boundaries() {
        let code="const __ferrite_css_preload__ = 'occupied';\nconst text = \"import('./a.js')\";\nconst emoji = '🙂'; export const load=()=>import(`./a.js`, {with:{type:'json'}});\nconst other=()=>import(variable);";
        let css = HashMap::from([("./a.js".into(), vec!["./style.css".into()])]);
        let (output, map) =
            preload_dynamic_imports("/entry.js", code, &css, "./helper.js", true).unwrap();
        assert!(output.contains("const text = \"import('./a.js')\""));
        assert!(
            output.contains("__ferrite_css_preload___(()=>import(`./a.js`, {with:{type:'json'}})")
        );
        assert!(output.contains("import(variable)"));
        crate::parse::parse_module("/entry.js", &output, &ferrite_core::ModuleType::Js).unwrap();
        let json = map.unwrap();
        let decoded = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        let position = output.find("__ferrite_css_preload___(()=>").unwrap();
        let prefix = &output[..position];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() as u32;
        let col = prefix.rsplit('\n').next().unwrap().encode_utf16().count() as u32;
        let token = decoded
            .lookup_token(&decoded.generate_lookup_table(), line, col)
            .unwrap();
        assert_eq!(
            token.get_source_id(),
            None,
            "generated preload calls must be unmapped"
        );
    }
}
