//! Specifier and import rewriting.

use crate::parse::*;
use crate::types::*;
use ferrite_core::ModuleType;
use ferrite_core::Result;
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
    if mapping.is_empty() {
        let parsed = parse_module("<rewrite>", code, module_type)?;
        return Ok((code.to_string(), parsed.imports));
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
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut output = code.to_string();
    for (start, end, replacement) in edits {
        output.replace_range(start..end, &replacement);
    }
    let reparsed = parse_module("<rewrite>", &output, &ModuleType::Js)?;
    Ok((output, reparsed.imports))
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

/// Rewrite `import.meta.hot` to the Ferrite HMR registry (§33).
///
/// v0.1 uses a scoped textual rewrite (only outside strings/comments is a
/// roadmap item); the dev client provides `globalThis.__ferrite_create_hot__`.
pub fn rewrite_import_meta_hot(code: &str, id: &str) -> String {
    if !code.contains("import.meta.hot") {
        return code.to_string();
    }
    let Ok(pattern) = regex::Regex::new(r"import\.meta\.hot([^A-Za-z0-9_$]|$)") else {
        return code.to_string();
    };
    let replacement = format!("globalThis.__ferrite_create_hot__({id:?})");
    pattern
        .replace_all(code, |captures: &regex::Captures| {
            format!("{}{}", replacement, &captures[1])
        })
        .into_owned()
}

/// Prepend the HMR client import for dev transforms.
#[must_use]
pub fn with_hmr_client(code: &str) -> String {
    format!("import \"/@ferrite/client\";\n{code}")
}
