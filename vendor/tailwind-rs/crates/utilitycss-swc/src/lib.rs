//! Production JavaScript and TypeScript extraction backed by the SWC parser.
//!
//! The extractor traverses syntax trees instead of searching source text. It extracts class
//! attributes and literals passed to known class helpers while preserving the original byte spans
//! for incremental compilation and diagnostics. It never evaluates user code.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeSet, fmt};

use swc_common::{sync::Lrc, BytePos, FileName, SourceMap, Span as SwcSpan, Spanned};
use swc_ecma_ast::{
    CallExpr, Callee, ClassDecl, Expr, FnDecl, ImportDecl, ImportSpecifier, JSXAttr, JSXAttrName,
    JSXAttrValue, Pat, Str, Tpl, VarDeclarator,
};
use swc_ecma_parser::{lexer::Lexer, EsSyntax, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_visit::{Visit, VisitWith};
use utilitycss_scanner::scan_checked;
use utilitycss_span::{validate_source_len, SourceSizeError, Span};

/// The source grammar used when parsing a source unit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SourceKind {
    /// JavaScript without JSX syntax.
    #[default]
    JavaScript,
    /// TypeScript without JSX syntax.
    TypeScript,
    /// JavaScript with JSX syntax.
    Jsx,
    /// TypeScript with JSX syntax.
    Tsx,
}

impl SourceKind {
    fn syntax(self) -> Syntax {
        match self {
            Self::JavaScript => Syntax::Es(EsSyntax::default()),
            Self::TypeScript => Syntax::Typescript(TsSyntax::default()),
            Self::Jsx => Syntax::Es(EsSyntax { jsx: true, ..Default::default() }),
            Self::Tsx => Syntax::Typescript(TsSyntax { tsx: true, ..Default::default() }),
        }
    }
}

/// A statically visible candidate and its byte span in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractedCandidate<'source> {
    raw: &'source str,
    span: Span,
}

impl<'source> ExtractedCandidate<'source> {
    /// Creates an extracted candidate from source text and a validated span.
    #[must_use]
    pub const fn new(raw: &'source str, span: Span) -> Self {
        Self { raw, span }
    }

    /// Returns the candidate text exactly as it appeared in the source.
    #[must_use]
    pub const fn raw(self) -> &'source str {
        self.raw
    }

    /// Returns the candidate's byte span in the source.
    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }
}

/// The kind of failure encountered while parsing a source unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtractionErrorKind {
    /// SWC rejected the source as invalid for the selected grammar.
    Parse,
    /// A parser span could not be mapped back to the supplied UTF-8 source.
    InvalidSpan,
    /// The source is larger than the 32-bit source-location model can represent.
    SourceTooLarge,
}

/// A typed extraction error that is safe to expose across adapter boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionError {
    kind: ExtractionErrorKind,
    message: String,
    offset: Option<u32>,
}

impl ExtractionError {
    fn parse(message: impl Into<String>, offset: Option<u32>) -> Self {
        Self { kind: ExtractionErrorKind::Parse, message: message.into(), offset }
    }

    fn invalid_span(offset: Option<u32>) -> Self {
        Self {
            kind: ExtractionErrorKind::InvalidSpan,
            message: "SWC returned a span outside the supplied source".to_owned(),
            offset,
        }
    }

    fn source_too_large(error: SourceSizeError) -> Self {
        Self { kind: ExtractionErrorKind::SourceTooLarge, message: error.to_string(), offset: None }
    }

    /// Returns the structured error kind.
    #[must_use]
    pub const fn kind(&self) -> &ExtractionErrorKind {
        &self.kind
    }

    /// Returns the human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the byte offset associated with the parser error, when available.
    #[must_use]
    pub const fn offset(&self) -> Option<u32> {
        self.offset
    }
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(offset) = self.offset {
            write!(formatter, "{} at byte {offset}", self.message)
        } else {
            formatter.write_str(&self.message)
        }
    }
}

impl std::error::Error for ExtractionError {}

/// Extracts statically visible candidates from a JavaScript-family source unit.
///
/// Static string literals in `class`/`className` JSX attributes and in known helper calls are
/// scanned. Dynamic expressions are not evaluated. Duplicate occurrences are preserved and the
/// result is sorted by source span for deterministic incremental updates.
pub fn extract(
    source: &str,
    source_kind: SourceKind,
) -> Result<Vec<ExtractedCandidate<'_>>, ExtractionError> {
    validate_source_len(source.len()).map_err(ExtractionError::source_too_large)?;
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map
        .new_source_file(FileName::Custom("utilitycss-input".to_owned()).into(), source.to_owned());
    let base = file.start_pos;
    let lexer =
        Lexer::new(source_kind.syntax(), Default::default(), StringInput::from(&*file), None);
    let mut parser = Parser::new_from(lexer);
    let program = parser.parse_program().map_err(|error| {
        ExtractionError::parse(
            format!("failed to parse source: {:?}", error.kind()),
            source_offset(error.span(), base),
        )
    })?;

    if let Some(error) = parser.take_errors().into_iter().next() {
        return Err(ExtractionError::parse(
            format!("failed to parse source: {:?}", error.kind()),
            source_offset(error.span(), base),
        ));
    }

    let mut bindings = BindingCollector::default();
    program.visit_with(&mut bindings);
    let mut visitor =
        CandidateVisitor::new(source, base, bindings.helper_aliases, bindings.shadowed_helpers);
    program.visit_with(&mut visitor);
    visitor.finish()
}

fn source_offset(span: SwcSpan, base: BytePos) -> Option<u32> {
    span.lo.0.checked_sub(base.0)
}

struct CandidateVisitor<'source> {
    source: &'source str,
    base: BytePos,
    helper_depth: usize,
    dynamic_template_depth: usize,
    helper_aliases: BTreeSet<String>,
    shadowed_helpers: BTreeSet<String>,
    candidates: Vec<ExtractedCandidate<'source>>,
    error: Option<ExtractionError>,
}

impl<'source> CandidateVisitor<'source> {
    fn new(
        source: &'source str,
        base: BytePos,
        helper_aliases: BTreeSet<String>,
        shadowed_helpers: BTreeSet<String>,
    ) -> Self {
        Self {
            source,
            base,
            helper_depth: 0,
            dynamic_template_depth: 0,
            helper_aliases,
            shadowed_helpers,
            candidates: Vec::new(),
            error: None,
        }
    }

    fn finish(mut self) -> Result<Vec<ExtractedCandidate<'source>>, ExtractionError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.candidates.sort_by_key(|candidate| (candidate.span().start(), candidate.span().end()));
        Ok(self.candidates)
    }

    fn is_class_attribute(name: &JSXAttrName) -> bool {
        matches!(name, JSXAttrName::Ident(name) if matches!(name.sym.as_ref(), "class" | "className"))
    }

    fn is_helper(&self, call: &CallExpr) -> bool {
        let Callee::Expr(callee) = &call.callee else {
            return false;
        };
        let Expr::Ident(identifier) = callee.as_ref() else {
            return false;
        };
        let name = identifier.sym.as_ref();
        (is_known_helper(name) || self.helper_aliases.contains(name))
            && !self.shadowed_helpers.contains(name)
    }

    fn add_literal(&mut self, span: SwcSpan) -> Result<(), ExtractionError> {
        let (start, end) = self.source_range(span)?;
        let (content_start, content_end) = strip_literal_delimiters(self.source, start, end);
        self.add_scan_region(content_start, content_end)
    }

    fn record_error(&mut self, result: Result<(), ExtractionError>) {
        if let Err(error) = result {
            self.error.get_or_insert(error);
        }
    }

    fn add_scan_region(&mut self, start: usize, end: usize) -> Result<(), ExtractionError> {
        let region = self
            .source
            .get(start..end)
            .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
        for token in scan_checked(region).map_err(ExtractionError::source_too_large)? {
            let local_start = usize::try_from(token.span().start())
                .map_err(|_| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
            let local_end = usize::try_from(token.span().end())
                .map_err(|_| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
            let absolute_start = start
                .checked_add(local_start)
                .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
            let absolute_end = start
                .checked_add(local_end)
                .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
            let raw = self
                .source
                .get(absolute_start..absolute_end)
                .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(absolute_start).ok()))?;
            let span = Span::new(
                u32::try_from(absolute_start).map_err(|_| ExtractionError::invalid_span(None))?,
                u32::try_from(absolute_end).map_err(|_| ExtractionError::invalid_span(None))?,
            )
            .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(absolute_start).ok()))?;
            self.candidates.push(ExtractedCandidate::new(raw, span));
        }
        Ok(())
    }

    fn source_range(&self, span: SwcSpan) -> Result<(usize, usize), ExtractionError> {
        let start = span
            .lo
            .0
            .checked_sub(self.base.0)
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or_else(|| ExtractionError::invalid_span(None))?;
        let end = span
            .hi
            .0
            .checked_sub(self.base.0)
            .and_then(|offset| usize::try_from(offset).ok())
            .ok_or_else(|| ExtractionError::invalid_span(u32::try_from(start).ok()))?;
        if start > end || self.source.get(start..end).is_none() {
            return Err(ExtractionError::invalid_span(u32::try_from(start).ok()));
        }
        Ok((start, end))
    }
}

#[derive(Default)]
struct BindingCollector {
    helper_aliases: BTreeSet<String>,
    shadowed_helpers: BTreeSet<String>,
}

impl BindingCollector {
    fn add(&mut self, name: &str) {
        if is_known_helper(name) {
            self.shadowed_helpers.insert(name.to_owned());
        }
    }
}

impl Visit for BindingCollector {
    fn visit_import_decl(&mut self, node: &ImportDecl) {
        for specifier in &node.specifiers {
            let ImportSpecifier::Named(named) = specifier else {
                continue;
            };
            if node.type_only || named.is_type_only {
                continue;
            }
            let Some(imported) = named.imported.as_ref() else {
                continue;
            };
            let imported = imported.atom().to_string();
            if is_known_helper(&imported) {
                self.helper_aliases.insert(named.local.sym.to_string());
            }
        }
    }

    fn visit_var_declarator(&mut self, node: &VarDeclarator) {
        if let Pat::Ident(binding) = &node.name {
            self.add(binding.id.sym.as_ref());
        }
        node.visit_children_with(self);
    }

    fn visit_fn_decl(&mut self, node: &FnDecl) {
        self.add(node.ident.sym.as_ref());
        node.visit_children_with(self);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.add(node.ident.sym.as_ref());
        node.visit_children_with(self);
    }
}

fn is_known_helper(name: &str) -> bool {
    matches!(name, "clsx" | "classnames" | "cn" | "cva" | "tv" | "twJoin" | "twMerge")
}

impl Visit for CandidateVisitor<'_> {
    fn visit_call_expr(&mut self, node: &CallExpr) {
        if self.is_helper(node) {
            self.helper_depth = self.helper_depth.saturating_add(1);
            node.visit_children_with(self);
            self.helper_depth = self.helper_depth.saturating_sub(1);
        } else {
            node.visit_children_with(self);
        }
    }

    fn visit_jsx_attr(&mut self, node: &JSXAttr) {
        if Self::is_class_attribute(&node.name) {
            if let Some(JSXAttrValue::Str(value)) = &node.value {
                let result = self.add_literal(value.span);
                self.record_error(result);
            }
        }
        node.visit_children_with(self);
    }

    fn visit_str(&mut self, node: &Str) {
        if self.helper_depth > 0 && self.dynamic_template_depth == 0 {
            let result = self.add_literal(node.span);
            self.record_error(result);
        }
    }

    fn visit_tpl(&mut self, node: &Tpl) {
        if self.helper_depth > 0 && node.exprs.is_empty() {
            let result = self.add_literal(node.span);
            self.record_error(result);
        } else if !node.exprs.is_empty() {
            self.dynamic_template_depth = self.dynamic_template_depth.saturating_add(1);
            node.visit_children_with(self);
            self.dynamic_template_depth = self.dynamic_template_depth.saturating_sub(1);
            return;
        }
        node.visit_children_with(self);
    }
}

fn strip_literal_delimiters(source: &str, start: usize, end: usize) -> (usize, usize) {
    let Some(bytes) = source.get(start..end).map(str::as_bytes) else {
        return (start, end);
    };
    if bytes.len() >= 2
        && matches!(bytes[0], b'\'' | b'"' | b'`')
        && bytes[0] == bytes[bytes.len() - 1]
    {
        (start + 1, end - 1)
    } else {
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{extract, SourceKind};

    fn raws(source: &str, source_kind: SourceKind) -> Vec<&str> {
        extract(source, source_kind)
            .expect("source parses")
            .into_iter()
            .map(|candidate| candidate.raw())
            .collect()
    }

    #[test]
    fn extracts_jsx_attributes_and_helper_literals() {
        let source = r#"
            export function Card({ active }) {
                return <div className="flex p-4" title="text-red-500">
                    {clsx("rounded", active && "bg-red-500", `text-${"blue"}-500`)}
                </div>;
            }
        "#;

        assert_eq!(raws(source, SourceKind::Jsx), vec!["flex", "p-4", "rounded", "bg-red-500"]);
    }

    #[test]
    fn extracts_typescript_cva_configuration() {
        let source = r#"
            const button = cva("rounded", {
                variants: { size: { sm: "px-2 py-1" } },
                compoundVariants: [{ class: "font-bold" }],
            });
            const element = <button className={button({ size: "sm" })} />;
        "#;

        assert_eq!(raws(source, SourceKind::Tsx), vec!["rounded", "px-2", "py-1", "font-bold"]);
    }

    #[test]
    fn preserves_original_byte_spans() {
        let source = r#"const value = clsx('hover:bg-red-500/50');"#;
        let candidate = extract(source, SourceKind::JavaScript)
            .expect("source parses")
            .into_iter()
            .next()
            .expect("candidate exists");
        let start = usize::try_from(candidate.span().start()).expect("span fits");
        let end = usize::try_from(candidate.span().end()).expect("span fits");

        assert_eq!(&source[start..end], candidate.raw());
    }

    #[test]
    fn ignores_dynamic_templates_and_unrelated_literals() {
        let source = r#"
            const className = `p-${size}`;
            const unrelated = "text-blue-500";
            const classes = clsx(`text-${tone}`, "p-8");
        "#;

        assert_eq!(raws(source, SourceKind::JavaScript), vec!["p-8"]);
    }

    #[test]
    fn does_not_treat_shadowed_helper_names_as_class_helpers() {
        let source = r#"
            function cn(value) { return encrypt(value); }
            cn("flex");
        "#;

        assert!(extract(source, SourceKind::JavaScript).expect("source parses").is_empty());
    }

    #[test]
    fn preserves_imported_helper_names() {
        let source = r#"
            import { cn as cx } from "./styles";
            cx("flex");
        "#;

        assert_eq!(raws(source, SourceKind::JavaScript), vec!["flex"]);
    }

    #[test]
    fn reports_invalid_syntax_without_panicking() {
        let error = extract("const = ;", SourceKind::JavaScript).expect_err("syntax is invalid");

        assert_eq!(error.kind(), &super::ExtractionErrorKind::Parse);
        assert!(error.offset().is_some());
    }

    proptest! {
        #[test]
        fn arbitrary_js_input_never_panics(source in any::<String>()) {
            let _ = extract(&source, SourceKind::JavaScript);
        }
    }
}
