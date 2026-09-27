//! JavaScript/TypeScript compiler abstraction (spec §5, §90).
//!
//! [`JsCompiler`] is the stable boundary: the module graph, server, and
//! bundler only see [`ParsedModule`] / [`TransformResult`], never Oxc or SWC
//! AST types (§92).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use ferrite_core::{EnvironmentKind, FerriteError, ModuleType, Result, SourceMap, Target};
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_minifier::{Minifier, MinifierOptions};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{JsxRuntime, TransformOptions, Transformer};

/// Compiler abstraction (spec §5).
pub trait JsCompiler: Send + Sync {
    /// Parse a module and extract imports/exports.
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule>;
    /// Transform a module (TS strip, JSX, lowering, defines).
    fn transform(&self, request: TransformRequest) -> Result<TransformResult>;
    /// Minify a module.
    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult>;
}

/// Compiler engine selection (§5).
#[derive(Clone)]
pub enum CompilerEngine {
    /// Oxc backend (default).
    Oxc,
    /// SWC backend (compatibility; see [`SwcCompiler`]).
    Swc,
    /// Custom compiler implementation.
    Custom(Arc<dyn JsCompiler>),
}

/// Build the compiler for an engine name (`oxc` / `swc`).
pub fn compiler_for_engine(engine: &str) -> Result<Arc<dyn JsCompiler>> {
    match engine.to_ascii_lowercase().as_str() {
        "oxc" => Ok(Arc::new(OxcCompiler::new(OxcOptions::default()))),
        "swc" => Ok(Arc::new(SwcCompiler)),
        other => Err(FerriteError::Other(format!(
            "unknown compiler engine `{other}`"
        ))),
    }
}

/// Parse request.
#[derive(Debug, Clone)]
pub struct ParseRequest {
    /// Module id (used for source-type inference).
    pub id: String,
    /// Source code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
}

/// A parsed import specifier with its source range.
#[derive(Debug, Clone)]
pub struct ParsedImport {
    /// Raw specifier text (unquoted).
    pub specifier: String,
    /// Byte range of the quoted literal in the source.
    pub range: (usize, usize),
    /// Static import/re-export vs dynamic `import()`.
    pub kind: ParsedImportKind,
    /// True for `import type ...` (elided at runtime).
    pub is_type: bool,
}

/// Static vs dynamic import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedImportKind {
    /// `import`, `export ... from`.
    Static,
    /// `import()`.
    Dynamic,
}

/// A parsed module (§5, decoupled from compiler ASTs).
#[derive(Debug, Clone)]
pub struct ParsedModule {
    /// Module id.
    pub id: String,
    /// Extracted imports with AST ranges.
    pub imports: Vec<ParsedImport>,
    /// Exported names (`default` for default exports, `*` for star re-exports).
    pub exports: Vec<String>,
    /// True when the module uses ESM syntax.
    pub has_module_syntax: bool,
    /// True when `import.meta.hot` is referenced.
    pub uses_import_meta_hot: bool,
    /// True when `import.meta.env` is referenced.
    pub uses_import_meta_env: bool,
}

/// Transform request.
#[derive(Debug, Clone)]
pub struct TransformRequest {
    /// Module id.
    pub id: String,
    /// Source code.
    pub code: String,
    /// Module type.
    pub module_type: ModuleType,
    /// Target environment.
    pub environment: EnvironmentKind,
    /// True for SSR transforms.
    pub ssr: bool,
    /// Compilation target.
    pub target: Target,
    /// Minify after transforming.
    pub minify: bool,
    /// Emit a source map.
    pub sourcemap: bool,
    /// Compile-time defines.
    pub define: HashMap<String, String>,
    /// JSX runtime (`automatic` or `classic`).
    pub jsx_runtime: String,
    /// Enable development helpers (JSX dev, refresh preamble hooks).
    pub development: bool,
}

impl TransformRequest {
    /// Minimal request for tests and one-shot transforms.
    #[must_use]
    pub fn new(id: impl Into<String>, code: impl Into<String>, module_type: ModuleType) -> Self {
        Self {
            id: id.into(),
            code: code.into(),
            module_type,
            environment: EnvironmentKind::Client,
            ssr: false,
            target: Target::default(),
            minify: false,
            sourcemap: false,
            define: HashMap::new(),
            jsx_runtime: "automatic".to_string(),
            development: true,
        }
    }
}

/// Transform result (§41).
#[derive(Debug, Clone)]
pub struct TransformResult {
    /// Transformed code.
    pub code: String,
    /// Source map, when requested.
    pub map: Option<SourceMap>,
    /// Extra file dependencies discovered during transform.
    pub dependencies: Vec<String>,
    /// Imports extracted from the transformed code.
    pub imports: Vec<ParsedImport>,
    /// Exported names.
    pub exports: Vec<String>,
}

/// Minify request.
#[derive(Debug, Clone)]
pub struct MinifyRequest {
    /// Module id.
    pub id: String,
    /// Source code.
    pub code: String,
    /// Emit a source map (chained through `input_map` when present).
    pub sourcemap: bool,
    /// Map from a previous step (`code` → original); chained, not dropped.
    pub input_map: Option<SourceMap>,
}

/// Minify result.
#[derive(Debug, Clone)]
pub struct MinifyResult {
    /// Minified code.
    pub code: String,
    /// Source map, when requested.
    pub map: Option<SourceMap>,
}

/// Oxc backend options (§90).
#[derive(Debug, Clone)]
pub struct OxcOptions {
    /// Target string recorded in cache keys (env lowering roadmap).
    pub target: String,
}

impl Default for OxcOptions {
    fn default() -> Self {
        Self {
            target: "es2022".to_string(),
        }
    }
}

/// Oxc-backed compiler: parser + transformer + minifier (§90).
#[derive(Debug, Clone)]
pub struct OxcCompiler {
    /// Backend options.
    pub options: OxcOptions,
}

impl OxcCompiler {
    /// Create an Oxc compiler.
    #[must_use]
    pub fn new(options: OxcOptions) -> Self {
        Self { options }
    }

    /// Compiler version string for cache keys.
    #[must_use]
    pub fn version(&self) -> &'static str {
        "oxc-0.151"
    }
}

impl JsCompiler for OxcCompiler {
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule> {
        parse_module(&request.id, &request.code, &request.module_type)
    }

    fn transform(&self, request: TransformRequest) -> Result<TransformResult> {
        transform_module(request)
    }

    fn minify(&self, request: MinifyRequest) -> Result<MinifyResult> {
        minify_module(&request)
    }
}

/// SWC compatibility backend (§89).
///
/// v0.1 ships the adapter type with the stable [`JsCompiler`] interface; the
/// native SWC pipeline is an explicit roadmap item (spec §101 defers full
/// ecosystem-compat transforms). Configure `engine = "oxc"` (default).
#[derive(Debug, Clone, Copy, Default)]
pub struct SwcCompiler;

impl JsCompiler for SwcCompiler {
    fn parse(&self, request: ParseRequest) -> Result<ParsedModule> {
        // Parsing is engine-agnostic through the Oxc frontend; the SWC adapter
        // reuses it until the native SWC pipeline lands.
        OxcCompiler::new(OxcOptions::default()).parse(request)
    }

    fn transform(&self, _request: TransformRequest) -> Result<TransformResult> {
        Err(FerriteError::Other(
            "the SWC transform backend is not compiled into this build; set `[compiler] engine = \"oxc\"` (default)".to_string(),
        ))
    }

    fn minify(&self, _request: MinifyRequest) -> Result<MinifyResult> {
        Err(FerriteError::Other(
            "the SWC minify backend is not compiled into this build; set `[compiler] engine = \"oxc\"` (default)".to_string(),
        ))
    }
}

/// Infer an Oxc [`SourceType`] from module type + id.
fn source_type_for(id: &str, module_type: &ModuleType) -> SourceType {
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
fn parse_module(id: &str, code: &str, module_type: &ModuleType) -> Result<ParsedModule> {
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
    let mut imports = Vec::new();
    for entry in record.import_entries.iter() {
        let specifier = entry.module_request.name.as_str().to_string();
        let range = expand_to_quotes(code, entry.module_request.span);
        imports.push(ParsedImport {
            specifier,
            range,
            kind: ParsedImportKind::Static,
            is_type: entry.is_type,
        });
    }
    for entry in record
        .indirect_export_entries
        .iter()
        .chain(record.star_export_entries.iter())
    {
        if let Some(request) = &entry.module_request {
            let specifier = request.name.as_str().to_string();
            let range = expand_to_quotes(code, request.span);
            imports.push(ParsedImport {
                specifier,
                range,
                kind: ParsedImportKind::Static,
                is_type: entry.is_type,
            });
        }
    }
    for dynamic in record.dynamic_imports.iter() {
        if let Some((specifier, range)) = static_dynamic_specifier(code, dynamic.module_request) {
            imports.push(ParsedImport {
                specifier,
                range,
                kind: ParsedImportKind::Dynamic,
                is_type: false,
            });
        }
    }
    // Side-effect imports (`import "./x"`) carry no bindings and may be absent
    // from `import_entries`; `requested_modules` covers every static request.
    for (specifier, requests) in record.requested_modules.iter() {
        for requested in requests.iter() {
            let range = expand_to_quotes(code, requested.span);
            imports.push(ParsedImport {
                specifier: specifier.as_str().to_string(),
                range,
                kind: ParsedImportKind::Static,
                is_type: requested.is_type,
            });
        }
    }
    imports.sort_by_key(|import| import.range.0);
    imports.dedup_by(|a, b| a.range == b.range && a.specifier == b.specifier);

    let mut exports = Vec::new();
    for entry in record.local_export_entries.iter() {
        match &entry.export_name {
            oxc_syntax::module_record::ExportExportName::Name(name) => {
                exports.push(name.name.as_str().to_string());
            }
            oxc_syntax::module_record::ExportExportName::Default(_) => {
                exports.push("default".to_string());
            }
            oxc_syntax::module_record::ExportExportName::Null => {}
        }
    }
    for entry in record.indirect_export_entries.iter() {
        match &entry.export_name {
            oxc_syntax::module_record::ExportExportName::Name(name) => {
                exports.push(name.name.as_str().to_string());
            }
            oxc_syntax::module_record::ExportExportName::Default(_) => {
                exports.push("default".to_string());
            }
            oxc_syntax::module_record::ExportExportName::Null => {}
        }
    }
    if !record.star_export_entries.is_empty() {
        exports.push("*".to_string());
    }

    Ok(ParsedModule {
        id: id.to_string(),
        imports,
        exports,
        has_module_syntax: record.has_module_syntax,
        uses_import_meta_hot: code.contains("import.meta.hot"),
        uses_import_meta_env: code.contains("import.meta.env"),
    })
}

/// Expand a span to cover surrounding quotes (robust to inner/outer spans).
fn expand_to_quotes(code: &str, span: oxc_span::Span) -> (usize, usize) {
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
fn static_dynamic_specifier(code: &str, span: oxc_span::Span) -> Option<(String, (usize, usize))> {
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

/// Transform a module: TS strip + JSX + defines + optional minify/map.
fn transform_module(request: TransformRequest) -> Result<TransformResult> {
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
fn transform_js_like(request: &TransformRequest) -> Result<(String, Option<SourceMap>)> {
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

/// Minify a module with Oxc.
fn minify_module(request: &MinifyRequest) -> Result<MinifyResult> {
    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let parsed = Parser::new(&allocator, &request.code, source_type).parse();
    if parsed.fatal_error {
        return Err(FerriteError::Parse {
            id: request.id.clone(),
            message: "syntax error".to_string(),
            frame: Some(ferrite_core::code_frame(&request.code, 0, 2)),
        });
    }
    let mut program = parsed.program;
    Minifier::new(MinifierOptions::default()).minify(&allocator, &mut program);
    let path = PathBuf::from(request.id.split('?').next().unwrap_or(&request.id));
    let generated = Codegen::new()
        .with_options(CodegenOptions {
            minify: true,
            source_map_path: request.sourcemap.then_some(path),
            ..Default::default()
        })
        .build(&program);
    let map = match (request.sourcemap, generated.map) {
        (true, Some(outer)) => {
            let json = outer.to_json_string();
            let chained = match &request.input_map {
                Some(input) if !input.inline => Some(chain_source_maps(&json, &input.mappings)?),
                Some(_) => {
                    return Err(FerriteError::Other(format!(
                        "cannot chain an inline source map for {}",
                        request.id
                    )));
                }
                None => Some(json),
            };
            chained.map(SourceMap::external)
        }
        _ => None,
    };
    Ok(MinifyResult {
        code: generated.code,
        map,
    })
}

/// Chain two source maps (§41): `outer` maps generated→intermediate and
/// `inner` maps intermediate→original; returns generated→original JSON.
///
/// Positions the inner map cannot resolve pass through sourceless rather
/// than failing the build; unparseable inputs are a loud error.
pub fn chain_source_maps(outer_json: &str, inner_json: &str) -> Result<String> {
    let outer = oxc_sourcemap::SourceMap::from_json_string(outer_json)
        .map_err(|error| FerriteError::Other(format!("bad outer source map: {error}")))?;
    let inner = oxc_sourcemap::SourceMap::from_json_string(inner_json)
        .map_err(|error| FerriteError::Other(format!("bad inner source map: {error}")))?;
    // Index inner tokens by intermediate (generated) line → columns.
    // Entry: (dst_col, src_line, src_col, source_id, name_id).
    type LineIndex =
        std::collections::BTreeMap<u32, Vec<(u32, u32, u32, Option<u32>, Option<u32>)>>;
    let mut index: LineIndex = std::collections::BTreeMap::new();
    for token in inner.get_tokens() {
        index.entry(token.get_dst_line()).or_default().push((
            token.get_dst_col(),
            token.get_src_line(),
            token.get_src_col(),
            token.get_source_id(),
            token.get_name_id(),
        ));
    }
    for tokens in index.values_mut() {
        tokens.sort_by_key(|entry| entry.0);
    }
    let mut builder = oxc_sourcemap::SourceMapBuilder::default();
    // Preserve the inner source table (ids stay identical); contents are
    // fixed up after the build because the builder only takes `&str`.
    for source in inner.get_sources() {
        builder.set_source_and_content(source, "");
    }
    let outer_names: Vec<&str> = outer.get_names().collect();
    let inner_names: Vec<&str> = inner.get_names().collect();
    let mut name_cache: HashMap<(bool, u32), u32> = HashMap::new();
    for token in outer.get_tokens() {
        let (dst_line, dst_col) = (token.get_dst_line(), token.get_dst_col());
        let mut hit: Option<(u32, u32, Option<u32>, Option<u32>)> = None;
        if token.get_source_id().is_some() {
            let (line, col) = (token.get_src_line(), token.get_src_col());
            if let Some(candidates) = index.get(&line) {
                if let Some(entry) = candidates
                    .iter()
                    .rev()
                    .find(|entry| entry.0 <= col && entry.3.is_some())
                {
                    hit = Some((entry.1, entry.2, entry.3, entry.4));
                }
            }
        }
        match hit {
            Some((src_line, src_col, src_id, inner_name)) => {
                let name_id = match (token.get_name_id(), inner_name) {
                    (Some(id), _) => {
                        intern_chain_name(&mut builder, &mut name_cache, &outer_names, true, id)
                    }
                    (None, Some(id)) => {
                        intern_chain_name(&mut builder, &mut name_cache, &inner_names, false, id)
                    }
                    (None, None) => None,
                };
                builder.add_token(dst_line, dst_col, src_line, src_col, src_id, name_id);
            }
            None => builder.add_token(dst_line, dst_col, 0, 0, None, None),
        }
    }
    let mut chained = builder.into_sourcemap();
    chained.set_source_contents(inner.get_source_contents().collect());
    if let Some(file) = outer.get_file() {
        chained.set_file(file);
    }
    Ok(chained.to_json_string())
}

/// Intern a chain-local name id (outer names win over inner names).
fn intern_chain_name<'a>(
    builder: &mut oxc_sourcemap::SourceMapBuilder<'a>,
    cache: &mut HashMap<(bool, u32), u32>,
    names: &[&'a str],
    is_outer: bool,
    id: u32,
) -> Option<u32> {
    if let Some(cached) = cache.get(&(is_outer, id)) {
        return Some(*cached);
    }
    let name = names.get(id as usize).copied()?;
    let interned = builder.add_name(name);
    cache.insert((is_outer, id), interned);
    Some(interned)
}

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
/// (`(^|[^word])KEY([^word]|$)`); replacement runs to fixpoint (bounded) so
/// adjacent occurrences are all rewritten.
pub fn apply_define(code: &str, define: &HashMap<String, String>) -> String {
    let mut output = code.to_string();
    let mut keys: Vec<&String> = define.keys().collect();
    keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
    for key in keys {
        let value = &define[key];
        let pattern = format!(
            r"(^|[^A-Za-z0-9_$.]){}([^A-Za-z0-9_$]|$)",
            regex::escape(key)
        );
        let Ok(regex) = regex::Regex::new(&pattern) else {
            continue;
        };
        for _ in 0..8 {
            let next = regex
                .replace_all(&output, |captures: &regex::Captures| {
                    format!("{}{}{}", &captures[1], value, &captures[2])
                })
                .into_owned();
            if next == output {
                break;
            }
            output = next;
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn compiler() -> OxcCompiler {
        OxcCompiler::new(OxcOptions::default())
    }

    #[test]
    fn parses_imports_with_ranges() {
        let parsed = compiler()
            .parse(ParseRequest {
                id: "/src/main.ts".to_string(),
                code: "import { x } from \"./x\";\nconst m = await import(\"./lazy.ts\");\nconsole.log(x, m);\n".to_string(),
                module_type: ModuleType::Ts,
            })
            .unwrap();
        assert_eq!(parsed.imports.len(), 2);
        assert!(parsed.has_module_syntax);
        for import in &parsed.imports {
            let (start, end) = import.range;
            assert!(start < end);
        }
    }

    #[test]
    fn strips_typescript() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/src/main.ts",
                "const x: number = 1;\nexport default x;\n",
                ModuleType::Ts,
            ))
            .unwrap();
        assert!(result.code.contains("const x = 1"), "{}", result.code);
        assert!(result.exports.contains(&"default".to_string()));
    }

    #[test]
    fn transforms_jsx_automatic() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/src/app.tsx",
                "export const App = () => <div className=\"a\">hi</div>;\n",
                ModuleType::Tsx,
            ))
            .unwrap();
        assert!(result.code.contains("jsx"), "{}", result.code);
        assert!(!result.code.contains("<div"), "{}", result.code);
    }

    #[test]
    fn parses_side_effect_imports() {
        let parsed = compiler()
            .parse(ParseRequest {
                id: "/src/main.ts".to_string(),
                code: "import \"./style.css\";\nimport \"./polyfill\";\nconsole.log(1);\n"
                    .to_string(),
                module_type: ModuleType::Ts,
            })
            .unwrap();
        let specifiers: Vec<&str> = parsed
            .imports
            .iter()
            .map(|i| i.specifier.as_str())
            .collect();
        assert!(specifiers.contains(&"./style.css"), "{specifiers:?}");
        assert!(specifiers.contains(&"./polyfill"), "{specifiers:?}");
    }

    #[test]
    fn rewrites_specifiers_by_ast() {
        let code =
            "import { x } from \"pkg\";\nimport y from \"./local.ts\";\nconsole.log(x, y);\n";
        let mapping = HashMap::from([
            ("pkg".to_string(), "/@npm/pkg@1.0.0/index.js".to_string()),
            ("./local.ts".to_string(), "/src/local.ts".to_string()),
        ]);
        let (rewritten, _) = rewrite_specifiers(code, &ModuleType::Ts, &mapping).unwrap();
        assert!(
            rewritten.contains("/@npm/pkg@1.0.0/index.js"),
            "{rewritten}"
        );
        assert!(rewritten.contains("/src/local.ts"), "{rewritten}");
    }

    #[test]
    fn json_becomes_esm() {
        let result = compiler()
            .transform(TransformRequest::new(
                "/data.json",
                "{\"a\":1}",
                ModuleType::Json,
            ))
            .unwrap();
        assert!(result.code.contains("export default"), "{}", result.code);
    }

    #[test]
    fn define_replacement_is_word_safe() {
        let mut define = HashMap::new();
        define.insert("__VERSION__".to_string(), "\"1.2.3\"".to_string());
        let output = apply_define("const v = __VERSION__; const w = __VERSION__X;", &define);
        assert!(output.contains("\"1.2.3\";"));
        assert!(output.contains("__VERSION__X"));
    }

    #[test]
    fn minifies() {
        let result = compiler()
            .minify(MinifyRequest {
                id: "/a.js".to_string(),
                code: "const  longName  =  1 + 2;\nconsole.log(longName);\n".to_string(),
                sourcemap: false,
                input_map: None,
            })
            .unwrap();
        assert!(result.code.len() < 60, "{}", result.code);
    }

    #[test]
    fn minify_chains_through_transform_map() {
        let compiler = compiler();
        let transformed = compiler
            .transform({
                let mut request = TransformRequest::new(
                    "/src/a.ts",
                    "const greeting: string = \"hi\";\nconsole.log(greeting);\n",
                    ModuleType::Ts,
                );
                request.sourcemap = true;
                request
            })
            .unwrap();
        let input_map = transformed.map.expect("transform map");
        let minified = compiler
            .minify(MinifyRequest {
                id: "/gen/a.js".to_string(),
                code: transformed.code,
                sourcemap: true,
                input_map: Some(input_map),
            })
            .unwrap();
        let chained = minified.map.expect("chained map");
        let decoded =
            oxc_sourcemap::SourceMap::from_json_string(&chained.mappings).expect("decode chain");
        // Chained sources are the ORIGINAL .ts file, not the intermediate.
        let sources: Vec<&str> = decoded.get_sources().collect();
        assert_eq!(sources, vec!["/src/a.ts"]);
        // Minified output maps back to original lines (no dangling refs).
        let mut mapped = 0;
        for token in decoded.get_tokens() {
            if let Some(source_id) = token.get_source_id() {
                assert!(decoded.get_source(source_id).is_some());
                mapped += 1;
            }
        }
        assert!(mapped > 0, "expected mapped tokens");
    }

    #[test]
    fn chain_unmapped_positions_pass_through_sourceless() {
        fn build(source: &str, tokens: &[(u32, u32, u32, u32, bool)]) -> String {
            let mut builder = oxc_sourcemap::SourceMapBuilder::default();
            builder.set_source_and_content(source, "content");
            for (dst_line, dst_col, src_line, src_col, mapped) in tokens {
                let (src_line, src_col, src_id) = if *mapped {
                    (*src_line, *src_col, Some(0))
                } else {
                    (0, 0, None)
                };
                builder.add_token(*dst_line, *dst_col, src_line, src_col, src_id, None);
            }
            builder.into_sourcemap().to_json_string()
        }
        // inner: intermediate (0,0) → original (7,3).
        let inner = build("orig.ts", &[(0, 0, 7, 3, true)]);
        // outer: hit at intermediate (0,0), miss at (5,0), sourceless.
        let outer = build(
            "mid.js",
            &[(0, 2, 0, 0, true), (0, 9, 5, 0, true), (0, 12, 0, 0, false)],
        );
        let chained = chain_source_maps(&outer, &inner).unwrap();
        let decoded = oxc_sourcemap::SourceMap::from_json_string(&chained).unwrap();
        assert_eq!(decoded.get_sources().collect::<Vec<_>>(), vec!["orig.ts"]);
        let tokens: Vec<_> = decoded.get_tokens().collect();
        assert_eq!(tokens.len(), 3);
        // Hit remaps to the original position.
        assert_eq!(
            (
                tokens[0].get_src_line(),
                tokens[0].get_src_col(),
                tokens[0].get_source_id()
            ),
            (7, 3, Some(0))
        );
        // Miss and sourceless stay sourceless.
        assert_eq!(tokens[1].get_source_id(), None);
        assert_eq!(tokens[2].get_source_id(), None);
    }

    #[test]
    fn chain_rejects_bad_json() {
        assert!(chain_source_maps("nope", "{\"version\":3}").is_err());
        let inner = "{\"version\":3,\"sources\":[],\"names\":[],\"mappings\":\"\"}";
        assert!(chain_source_maps("nope", inner).is_err());
        assert!(chain_source_maps(inner, "nope").is_err());
    }

    #[test]
    fn sourcemap_requested() {
        let mut request =
            TransformRequest::new("/src/a.ts", "const x: number = 1;\n", ModuleType::Ts);
        request.sourcemap = true;
        let result = compiler().transform(request).unwrap();
        assert!(result.map.is_some());
    }
}
