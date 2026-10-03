//! Specifier and import rewriting.

use crate::parse::*;
use crate::types::*;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashMap;

/// Rewrite specifiers in `code` using AST ranges (§28).
///
/// Only specifiers present in `mapping` are replaced; returns the rewritten
/// code plus the fresh import list.
pub fn rewrite_specifiers(
    code: &str,
    module_type: &ModuleType,
    mapping: &HashMap<String, String>,
) -> Result<(String, Vec<ParsedImport>)> {
    let (code, imports, _) =
        rewrite_specifiers_mapped("<rewrite>", code, module_type, mapping, false)?;
    Ok((code, imports))
}

/// Specifier rewriting with a generated-to-input map for every unchanged character.
pub fn rewrite_specifiers_mapped(
    id: &str,
    code: &str,
    module_type: &ModuleType,
    mapping: &HashMap<String, String>,
    sourcemap: bool,
) -> Result<(String, Vec<ParsedImport>, Option<String>)> {
    if mapping.is_empty() {
        let parsed = parse_module("<rewrite>", code, module_type)?;
        return Ok((code.to_string(), parsed.imports, None));
    }
    let parsed = parse_module("<rewrite>", code, module_type)?;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for import in &parsed.imports {
        if import.is_type {
            continue;
        }
        if let Some(replacement) = mapping.get(&import.specifier) {
            let (start, end) = import.range;
            let original = code.get(start..end).unwrap_or("\"\"");
            let quote = original.as_bytes().first().copied().unwrap_or(b'"');
            let quote_char = if quote == b'\'' {
                '\''
            } else if quote == b'`' {
                '`'
            } else {
                '"'
            };
            edits.push((start, end, format!("{quote_char}{replacement}{quote_char}")));
        }
    }
    let (output, map) = apply_text_edits(id, code, &edits, sourcemap)?;
    let reparsed = parse_module("<rewrite>", &output, &ModuleType::Js)?;
    Ok((output, reparsed.imports, map))
}

/// Apply byte-range edits while preserving accurate UTF-16 source positions.
/// Inserted code is unmapped; unchanged characters retain their original positions.
pub fn apply_text_edits(
    id: &str,
    code: &str,
    edits: &[(usize, usize, String)],
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| edit.0);
    let mut output = String::new();
    let mut cursor = 0;
    let mut builder = oxc_sourcemap::SourceMapBuilder::default();
    let source = builder.set_source_and_content(id, code);
    let (mut dst_line, mut dst_col, mut src_line, mut src_col) = (0, 0, 0, 0);
    for (start, end, replacement) in
        edits
            .iter()
            .chain(std::iter::once(&(code.len(), code.len(), String::new())))
    {
        if *start < cursor
            || *end < *start
            || !code.is_char_boundary(*start)
            || !code.is_char_boundary(*end)
        {
            return Err(ferrite_core::FerriteError::Other(format!(
                "invalid or overlapping text edit in {id}"
            )));
        }
        let unchanged = &code[cursor..*start];
        for ch in unchanged.chars() {
            if sourcemap {
                builder.add_token(dst_line, dst_col, src_line, src_col, Some(source), None);
            }
            output.push(ch);
            advance(ch, &mut src_line, &mut src_col);
            advance(ch, &mut dst_line, &mut dst_col);
        }
        for ch in code[*start..*end].chars() {
            advance(ch, &mut src_line, &mut src_col);
        }
        if sourcemap && !replacement.is_empty() {
            builder.add_token(dst_line, dst_col, 0, 0, None, None);
        }
        output.push_str(replacement);
        for ch in replacement.chars() {
            if sourcemap && dst_col == 0 {
                builder.add_token(dst_line, dst_col, 0, 0, None, None);
            }
            advance(ch, &mut dst_line, &mut dst_col);
        }
        cursor = *end;
    }
    Ok((
        output,
        sourcemap.then(|| builder.into_sourcemap().to_json_string()),
    ))
}

fn advance(ch: char, line: &mut u32, column: &mut u32) {
    if ch == '\n' {
        *line += 1;
        *column = 0;
    } else {
        *column += ch.len_utf16() as u32;
    }
}

/// Apply compile-time defines with word-boundary safety (§43).
///
/// The `regex` crate has no look-around, so boundaries are consumable groups
/// (`(^|[^word])KEY([^word]|$)`). All keys are rewritten in a single manual
/// scan: after each match the scan resumes at the trailing boundary (so it
/// can serve as the next leading boundary for adjacent occurrences) and
/// replacement text is never re-scanned, so a define value containing a key
/// is left alone.
pub fn apply_define(code: &str, define: &HashMap<String, String>) -> String {
    let mut keys: Vec<&String> = define.keys().filter(|key| !key.is_empty()).collect();
    if keys.is_empty() {
        return code.to_string();
    }
    // Longest first so the alternation prefers the longest key at each spot.
    keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
    let alternation = keys
        .iter()
        .map(|key| regex::escape(key))
        .collect::<Vec<_>>()
        .join("|");
    let pattern = format!(r"(^|[^A-Za-z0-9_$.])({alternation})([^A-Za-z0-9_$]|$)");
    let Ok(regex) = regex::Regex::new(&pattern) else {
        return code.to_string();
    };
    let mut output = String::with_capacity(code.len());
    let mut pos = 0;
    while pos <= code.len() {
        let Some(captures) = regex.captures_at(code, pos) else {
            break;
        };
        let matched = captures.get(0).expect("regex match");
        let key = &captures[2];
        let Some(value) = define.get(key) else {
            break;
        };
        output.push_str(&code[pos..matched.start()]);
        output.push_str(&captures[1]);
        output.push_str(value);
        // Resume at the trailing boundary: it is re-emitted verbatim (or
        // reused as the next leading boundary) and the match always
        // consumes the non-empty key, so this always advances.
        pos = captures.get(3).expect("trailing boundary").start();
        if pos == code.len() {
            break;
        }
    }
    output.push_str(&code[pos..]);
    output
}

/// Initialize one shared `import.meta.hot` context for this module (§33).
/// AST analysis excludes strings, comments and template text. Real accesses
/// retain their original locations; one generated initializer precedes them.
/// Unparseable code is unchanged here; the mapped pipeline reports parse errors.
pub fn rewrite_import_meta_hot(code: &str, id: &str) -> String {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        return code.to_string();
    }
    if import_meta_hot_spans(&parsed.program).is_empty() {
        return code.to_string();
    }
    format!("import.meta.hot = globalThis.__ferrite_create_hot__({id:?});\n{code}")
}

/// Prepend the HMR client import for dev transforms.
#[must_use]
pub fn with_hmr_client(code: &str) -> String {
    format!("import \"/@ferrite/client\";\n{code}")
}

/// Initialize the hot context and prepend the client import with an edit map.
pub fn inject_hmr_mapped(
    id: &str,
    code: &str,
    sourcemap: bool,
) -> Result<(String, Option<String>)> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, code, SourceType::mjs()).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        return Err(ferrite_core::FerriteError::Parse {
            id: id.into(),
            message: "invalid JavaScript before HMR injection".into(),
            frame: None,
        });
    }
    let mut prefix = String::from("import \"/@ferrite/client\";\n");
    if !import_meta_hot_spans(&parsed.program).is_empty() {
        prefix.push_str(&format!(
            "import.meta.hot = globalThis.__ferrite_create_hot__({id:?});\n"
        ));
    }
    let edits = vec![(0, 0, prefix)];
    apply_text_edits(id, code, &edits, sourcemap)
}

#[cfg(test)]
mod map_tests {
    use super::*;

    #[test]
    fn edits_preserve_utf16_positions_and_leave_generated_code_unmapped() {
        let source = "const smile = '😀'; const value = 1;\nconsole.log(value);\n";
        let (code, map) = apply_text_edits(
            "/original.js",
            source,
            &[(0, 0, "// generated\n".into())],
            true,
        )
        .unwrap();
        let json = map.unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        let expected_column = source[..source.find("value").unwrap()]
            .encode_utf16()
            .count() as u32;
        let token = map
            .get_tokens()
            .find(|token| token.get_dst_line() == 1 && token.get_dst_col() == expected_column)
            .unwrap();
        assert_eq!(token.get_src_line(), 0);
        assert_eq!(token.get_src_col(), expected_column);
        assert!(map
            .get_tokens()
            .filter(|token| token.get_dst_line() == 0)
            .all(|token| token.get_source_id().is_none()));
        assert_eq!(code, format!("// generated\n{source}"));
    }

    #[test]
    fn composition_does_not_cross_unmapped_insertions() {
        let source = "export const original = 1;";
        let (intermediate, inner) = apply_text_edits(
            "original.js",
            source,
            &[(0, 0, "generated(); ".into())],
            true,
        )
        .unwrap();
        let (_, outer) = apply_text_edits("intermediate.js", &intermediate, &[], true).unwrap();
        let json = crate::chain_source_maps(&outer.unwrap(), &inner.unwrap()).unwrap();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        assert!(map
            .get_tokens()
            .filter(|token| token.get_dst_col() < 13)
            .all(|token| token.get_source_id().is_none()));
        assert!(map
            .get_tokens()
            .filter(|token| token.get_dst_col() >= 13)
            .all(|token| token.get_source_id().is_some()));
    }

    #[test]
    fn malformed_edits_are_errors() {
        assert!(apply_text_edits("x", "abc", &[(2, 1, String::new())], true).is_err());
        assert!(apply_text_edits("x", "😀", &[(1, 2, String::new())], true).is_err());
        assert!(apply_text_edits(
            "x",
            "abc",
            &[(0, 2, String::new()), (1, 3, String::new())],
            true
        )
        .is_err());
    }
}
