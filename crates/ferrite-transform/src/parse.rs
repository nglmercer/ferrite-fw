//! Module parsing.

use crate::types::*;
use ferrite_core::FerriteError;
use ferrite_core::ModuleType;
use ferrite_core::Result;
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashMap;
use std::path::PathBuf;

/// Infer an Oxc [`SourceType`] from module type + id.
pub(crate) fn source_type_for(id: &str, module_type: &ModuleType) -> SourceType {
    let path = PathBuf::from(id.split('?').next().unwrap_or(id));
    if let Ok(inferred) = SourceType::from_path(&path) {
        // `from_path` understands extensions; force JSX/TSX from module type
        // for virtual ids without usable extensions.
        return inferred;
    }
    match module_type {
        ModuleType::Tsx => SourceType::tsx(),
        ModuleType::Ts => SourceType::ts(),
        ModuleType::Jsx => SourceType::jsx(),
        ModuleType::Js => SourceType::mjs(),
        _ => SourceType::mjs(),
    }
}

/// Parse a module and extract imports/exports.
/// Exported name text (`None` for null entries).
pub(crate) fn export_name_of(
    name: &oxc_syntax::module_record::ExportExportName<'_>,
) -> Option<String> {
    match name {
        oxc_syntax::module_record::ExportExportName::Name(name) => {
            Some(name.name.as_str().to_string())
        }
        oxc_syntax::module_record::ExportExportName::Default(_) => Some("default".to_string()),
        oxc_syntax::module_record::ExportExportName::Null => None,
    }
}

/// Source-side re-export name (`export { a as b } from` → `a`).
pub(crate) fn import_name_of(
    name: &oxc_syntax::module_record::ExportImportName<'_>,
) -> Option<String> {
    match name {
        oxc_syntax::module_record::ExportImportName::Name(name) => {
            Some(name.name.as_str().to_string())
        }
        _ => None,
    }
}

/// Local binding text (`None` when not locally accessible).
pub(crate) fn local_name_of(
    name: &oxc_syntax::module_record::ExportLocalName<'_>,
) -> Option<String> {
    match name {
        oxc_syntax::module_record::ExportLocalName::Name(name) => {
            Some(name.name.as_str().to_string())
        }
        oxc_syntax::module_record::ExportLocalName::Default(name) => {
            Some(name.name.as_str().to_string())
        }
        oxc_syntax::module_record::ExportLocalName::Null => None,
    }
}

pub(crate) fn parse_module(id: &str, code: &str, module_type: &ModuleType) -> Result<ParsedModule> {
    let allocator = Allocator::default();
    let source_type = source_type_for(id, module_type);
    let parsed = Parser::new(&allocator, code, source_type).parse();
    if parsed.fatal_error || !parsed.diagnostics.is_empty() {
        let message = parsed
            .diagnostics
            .iter()
            .take(3)
            .map(|diagnostic| diagnostic.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        let frame = (!code.is_empty()).then(|| ferrite_core::code_frame(code, 0, 2));
        return Err(FerriteError::Parse {
            id: id.to_string(),
            message: if message.is_empty() {
                "syntax error".to_string()
            } else {
                message
            },
            frame,
        });
    }
    let record = &parsed.module_record;
    // Aggregate bindings per (specifier, range): one statement can carry
    // several entries (`import d, {a} from "x"`).
    let mut import_map: HashMap<(String, (usize, usize)), ParsedImport> = HashMap::new();
    let mut push_binding = |specifier: String,
                            range: (usize, usize),
                            kind: ParsedImportKind,
                            is_type: bool,
                            binding: ImportBinding| {
        import_map
            .entry((specifier.clone(), range))
            .or_insert_with(|| ParsedImport {
                specifier,
                range,
                kind,
                is_type,
                bindings: Vec::new(),
            })
            .bindings
            .push(binding);
    };
    for entry in record.import_entries.iter() {
        let specifier = entry.module_request.name.as_str().to_string();
        let range = expand_to_quotes(code, entry.module_request.span);
        let binding = match &entry.import_name {
            oxc_syntax::module_record::ImportImportName::Name(name) => {
                ImportBinding::Named(name.name.as_str().to_string())
            }
            oxc_syntax::module_record::ImportImportName::NamespaceObject => {
                ImportBinding::Namespace
            }
            oxc_syntax::module_record::ImportImportName::Default(_) => ImportBinding::Default,
        };
        push_binding(
            specifier,
            range,
            ParsedImportKind::Static,
            entry.is_type,
            binding,
        );
    }
    for entry in record
        .indirect_export_entries
        .iter()
        .chain(record.star_export_entries.iter())
    {
        if let Some(request) = &entry.module_request {
            let specifier = request.name.as_str().to_string();
            let range = expand_to_quotes(code, request.span);
            push_binding(
                specifier,
                range,
                ParsedImportKind::Static,
                entry.is_type,
                ImportBinding::Reexport,
            );
        }
    }
    for dynamic in record.dynamic_imports.iter() {
        if let Some((specifier, range)) = static_dynamic_specifier(code, dynamic.module_request) {
            push_binding(
                specifier,
                range,
                ParsedImportKind::Dynamic,
                false,
                ImportBinding::Namespace,
            );
        }
    }
    // Side-effect imports (`import "./x"`) carry no bindings and may be absent
    // from `import_entries`; `requested_modules` covers every static request.
    for (specifier, requests) in record.requested_modules.iter() {
        for requested in requests.iter() {
            let range = expand_to_quotes(code, requested.span);
            let key = (specifier.as_str().to_string(), range);
            import_map
                .entry(key.clone())
                .or_insert_with(|| ParsedImport {
                    specifier: key.0,
                    range: key.1,
                    kind: ParsedImportKind::Static,
                    is_type: requested.is_type,
                    bindings: vec![ImportBinding::SideEffect],
                });
        }
    }
    let mut imports: Vec<ParsedImport> = import_map.into_values().collect();
    imports.sort_by_key(|import| import.range.0);

    let mut exports = Vec::new();
    let mut export_details = Vec::new();
    for entry in record.local_export_entries.iter() {
        let Some(exported) = export_name_of(&entry.export_name) else {
            continue;
        };
        exports.push(exported.clone());
        export_details.push(ParsedExport {
            exported,
            local: local_name_of(&entry.local_name),
            from: None,
            imported: None,
            target: None,
        });
    }
    for entry in record.indirect_export_entries.iter() {
        let Some(exported) = export_name_of(&entry.export_name) else {
            continue;
        };
        exports.push(exported.clone());
        export_details.push(ParsedExport {
            exported,
            local: None,
            from: entry
                .module_request
                .as_ref()
                .map(|request| request.name.as_str().to_string()),
            imported: import_name_of(&entry.import_name),
            target: None,
        });
    }
    for entry in record.star_export_entries.iter() {
        export_details.push(ParsedExport {
            exported: "*".to_string(),
            local: None,
            from: entry
                .module_request
                .as_ref()
                .map(|request| request.name.as_str().to_string()),
            imported: None,
            target: None,
        });
    }
    if !record.star_export_entries.is_empty() {
        exports.push("*".to_string());
    }

    Ok(ParsedModule {
        id: id.to_string(),
        imports,
        exports,
        export_details,
        has_module_syntax: record.has_module_syntax,
        uses_import_meta_hot: code.contains("import.meta.hot"),
        uses_import_meta_env: code.contains("import.meta.env"),
    })
}

/// Expand a span to cover surrounding quotes (robust to inner/outer spans).
pub(crate) fn expand_to_quotes(code: &str, span: oxc_span::Span) -> (usize, usize) {
    let bytes = code.as_bytes();
    let mut start = (span.start as usize).min(bytes.len());
    let mut end = (span.end as usize).min(bytes.len());
    if start < end {
        let first = bytes[start];
        if first == b'"' || first == b'\'' || first == b'`' {
            return (start, end);
        }
    }
    if start > 0 {
        let before = bytes[start - 1];
        if before == b'"' || before == b'\'' || before == b'`' {
            start -= 1;
        }
    }
    if end < bytes.len() {
        let after = bytes[end];
        if after == b'"' || after == b'\'' || after == b'`' {
            end += 1;
        }
    }
    (start, end)
}

/// Extract a static specifier from a dynamic-import expression span.
pub(crate) fn static_dynamic_specifier(
    code: &str,
    span: oxc_span::Span,
) -> Option<(String, (usize, usize))> {
    let start = span.start as usize;
    let end = (span.end as usize).min(code.len());
    if start >= end {
        return None;
    }
    let text = code.get(start..end)?.trim();
    let quote = text.as_bytes().first()?;
    if !matches!(quote, b'"' | b'\'' | b'`') || !text.ends_with(*quote as char) || text.len() < 2 {
        return None;
    }
    let inner = &text[1..text.len() - 1];
    if inner.contains("${") {
        return None; // template with interpolation: not statically analyzable
    }
    // Map back to absolute offsets.
    let leading = code[start..end].find(text)?;
    Some((
        inner.to_string(),
        (start + leading, start + leading + text.len()),
    ))
}
