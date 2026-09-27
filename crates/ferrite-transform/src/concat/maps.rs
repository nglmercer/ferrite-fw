//! Concat source maps.

use crate::concat::emit::*;
use crate::concat::types::*;
use ferrite_core::FerriteError;
use ferrite_core::Result;
use oxc_ast::ast::*;

/// Chain each piece through its input map, then concatenate with the
/// recorded line offsets.
pub(crate) fn concat_maps(
    pieces: &[Emitted],
    line_starts: &[u32],
    modules: &[ConcatModule],
) -> Result<String> {
    let mut chained: Vec<String> = Vec::with_capacity(pieces.len());
    for (piece, module) in pieces.iter().zip(modules) {
        let Some(fresh) = piece.map.as_deref() else {
            // Member without a map (should not happen when the loader
            // maps everything): fall back to the input map verbatim.
            chained.push(
                module.input_map.clone().unwrap_or_else(|| {
                    "{\"version\":3,\"sources\":[],\"mappings\":\"\"}".to_string()
                }),
            );
            continue;
        };
        match module.input_map.as_deref() {
            Some(input) if !input.is_empty() => {
                chained.push(crate::chain_source_maps(fresh, input)?);
            }
            _ => chained.push(fresh.to_string()),
        }
    }
    let decoded: Vec<oxc_sourcemap::SourceMap<'_>> = chained
        .iter()
        .map(|json| {
            oxc_sourcemap::SourceMap::from_json_string(json)
                .map_err(|error| FerriteError::Other(format!("bad concat member map: {error}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let pairs: Vec<(&oxc_sourcemap::SourceMap<'_>, u32)> =
        decoded.iter().zip(line_starts.iter().copied()).collect();
    let builder = oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&pairs);
    Ok(builder.into_owned_sourcemap().to_json_string())
}

/// `export default <expr>` → `const $name = <expr>;`.
pub(crate) fn default_expr_to_const<'a>(
    kind: ExportDefaultDeclarationKind<'a>,
    span: Span,
    name: Ident<'a>,
    builder: &oxc_ast::builder::AstBuilder<'a>,
) -> Statement<'a> {
    let Ok(expr) = Expression::try_from(kind) else {
        // Functions, classes, and interfaces are handled by the caller.
        unreachable!("non-expression default kind");
    };
    let declarator = VariableDeclarator::new(
        span,
        BindingPattern::BindingIdentifier(BindingIdentifier::boxed(span, name, builder)),
        None,
        Some(expr),
        false,
        builder,
    );
    Statement::VariableDeclaration(VariableDeclaration::boxed(
        span,
        VariableDeclarationKind::Const,
        [declarator],
        false,
        builder,
    ))
}
