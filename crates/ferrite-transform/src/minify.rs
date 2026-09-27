//! Minification and source maps.

use crate::types::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use ferrite_core::SourceMap;
use oxc_allocator::Allocator;
use oxc_codegen::Codegen;
use oxc_codegen::CodegenOptions;
use oxc_minifier::Minifier;
use oxc_minifier::MinifierOptions;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) fn minify_module(request: &MinifyRequest) -> Result<MinifyResult> {
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
pub(crate) fn intern_chain_name<'a>(
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
