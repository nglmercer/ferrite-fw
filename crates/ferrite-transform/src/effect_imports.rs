//! Remove imports whose only remaining purpose is executing an effect-free module.
use crate::{chain_source_maps, MinifyResult};
use ferrite_core::{FerriteError, Result, SourceMap};
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashSet;
use std::path::PathBuf;

/// `specifiers` must have been proven effect-free by the caller. Live binding
/// imports and named re-exports are never removed by this helper. Star exports
/// require the caller to prove that none of their exports is used.
pub fn remove_effect_only_imports(
    id: &str,
    code: &str,
    specifiers: &HashSet<String>,
    unused_reexports: &HashSet<String>,
    sourcemap: bool,
    input_map: Option<&str>,
) -> Result<Option<MinifyResult>> {
    if specifiers.is_empty() && unused_reexports.is_empty() {
        return Ok(None);
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        return Err(FerriteError::Parse {
            id: id.into(),
            message: "cannot remove effect-only imports from invalid JavaScript".into(),
            frame: None,
        });
    }
    let mut program = parsed.program;
    let before = program.body.len();
    program.body.retain(|statement| {
        let source = match statement {
            Statement::ImportDeclaration(decl)
                if decl.specifiers.as_ref().is_none_or(|list| list.is_empty()) =>
            {
                Some(&decl.source)
            }
            Statement::ExportFromDeclaration(decl) if decl.specifiers.is_empty() => {
                Some(&decl.source)
            }
            Statement::ExportAllDeclaration(decl) => {
                return !unused_reexports.contains(decl.source.value.as_str())
            }
            _ => None,
        };
        !source.is_some_and(|source| specifiers.contains(source.value.as_str()))
    });
    if program.body.len() == before {
        return Ok(None);
    }
    let generated = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: sourcemap.then(|| PathBuf::from(id)),
            ..Default::default()
        })
        .build(&program);
    let map = generated
        .map
        .map(|map| {
            let map = map.to_json_string();
            input_map.map_or_else(|| Ok(map.clone()), |input| chain_source_maps(&map, input))
        })
        .transpose()?
        .map(SourceMap::external);
    Ok(Some(MinifyResult {
        code: generated.code,
        map,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successive_removals_chain_to_original_source_lines_and_contents() {
        let source = "import '/first.js';\nimport '/second.js';\nexport const value = 42;\n";
        let first = remove_effect_only_imports(
            "/entry.js",
            source,
            &HashSet::from(["/first.js".into()]),
            &HashSet::new(),
            true,
            None,
        )
        .unwrap()
        .unwrap();
        let second = remove_effect_only_imports(
            "/entry.js",
            &first.code,
            &HashSet::from(["/second.js".into()]),
            &HashSet::new(),
            true,
            Some(&first.map.unwrap().mappings),
        )
        .unwrap()
        .unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&second.map.unwrap().mappings)
            .unwrap()
            .into_owned();
        let lookup = map.generate_lookup_table();
        let token = map.lookup_source_view_token(&lookup, 0, 0).unwrap();
        assert_eq!(token.get_src_line(), 2);
        assert_eq!(map.get_source_content(0), Some(source));
    }
    #[test]
    fn effect_only_removal_does_not_delete_live_star_or_namespace_reexports() {
        let output = remove_effect_only_imports(
            "/entry.js",
            "import '/dep.js'; export * from '/dep.js'; export * as ns from '/dep.js';",
            &HashSet::from(["/dep.js".into()]),
            &HashSet::new(),
            false,
            None,
        )
        .unwrap()
        .unwrap();
        assert!(!output.code.contains("import "));
        assert!(output.code.contains("export * from"));
        assert!(output.code.contains("export * as ns from"));
    }
    #[test]
    fn removal_preserves_binding_imports_strings_dynamic_imports_and_source_positions() {
        let code = "import './drop.js';\nexport {} from './drop.js';\nexport * from './drop.js';\nimport {keep} from './drop.js';\nexport {other} from './drop.js';\nexport const value = [keep, import('./drop.js'), \"import './drop.js'\"];\n";
        let output = remove_effect_only_imports(
            "/entry.js",
            code,
            &HashSet::from(["./drop.js".into()]),
            &HashSet::from(["./drop.js".into()]),
            true,
            None,
        )
        .unwrap()
        .unwrap();
        assert!(!output.code.contains("export *"));
        assert!(output.code.contains("{ keep }"));
        assert!(output.code.contains("{ other }"));
        assert!(output.code.contains("import(\"./drop.js\")"));
        assert!(output.code.contains("import './drop.js'"));
        let map = oxc_sourcemap::SourceMap::from_json_string(&output.map.unwrap().mappings)
            .unwrap()
            .into_owned();
        let lookup = map.generate_lookup_table();
        let line = output
            .code
            .lines()
            .position(|line| line.contains("export const value"))
            .unwrap() as u32;
        assert_eq!(
            map.lookup_source_view_token(&lookup, line, 0)
                .unwrap()
                .get_src_line(),
            5
        );
        assert_eq!(map.get_source_content(0), Some(code));
    }
}
