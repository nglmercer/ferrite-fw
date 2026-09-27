//! CSS stylesheet parsing and selector-aware `@apply` transformation.
//!
//! This crate owns stylesheet syntax and serialization. Utility meaning remains in
//! [`utilitycss_compiler::Compiler`]; this layer only finds CSS scopes, replaces explicit
//! directives, and preserves unrelated authored rules.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;

use cssparser::{Parser, ParserInput};
use utilitycss_compiler::{ApplyCandidate, Compiler, CompositionInput};
use utilitycss_css_ir::{CssDeclaration, CssDocument, CssRule, CssSerializationMode};
use utilitycss_diagnostics::{Diagnostic, DiagnosticCode};
use utilitycss_span::{validate_source_len, SourceId, Span};

/// Source content supplied to the stylesheet transformer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylesheetInput {
    source: SourceId,
    content: String,
    path: Option<String>,
}

impl StylesheetInput {
    /// Creates stylesheet input without optional path metadata.
    #[must_use]
    pub fn new(source: SourceId, content: impl Into<String>) -> Self {
        Self { source, content: content.into(), path: None }
    }

    /// Adds optional path metadata for host adapters.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Returns the source identity.
    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.source
    }

    /// Returns the stylesheet content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Returns optional path metadata.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

/// Result of transforming one stylesheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StylesheetOutput {
    css: String,
    diagnostics: Vec<Diagnostic>,
}

impl StylesheetOutput {
    /// Creates a stylesheet result.
    #[must_use]
    pub fn new(css: String, diagnostics: Vec<Diagnostic>) -> Self {
        Self { css, diagnostics }
    }

    /// Returns transformed CSS.
    #[must_use]
    pub fn css(&self) -> &str {
        &self.css
    }

    /// Returns structured diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consumes the result and returns its CSS and diagnostics.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<Diagnostic>) {
        (self.css, self.diagnostics)
    }
}

/// Transforms explicit `@apply` directives using the active compiler configuration.
///
/// The parser is token-aware and tracks balanced CSS blocks, strings, comments, functions, and
/// arbitrary values. It never evaluates imports, URLs, or script-like content.
#[must_use]
pub fn transform_stylesheet(compiler: &mut Compiler, input: StylesheetInput) -> StylesheetOutput {
    let source = input.source;
    let content = input.content;
    let mode = compiler.config().serialization_mode();
    let mut diagnostics = Vec::new();

    if let Err(error) = validate_source_len(content.len()) {
        diagnostics.push(
            Diagnostic::error(
                DiagnosticCode::new("stylesheet.source-too-large"),
                error.to_string(),
            )
            .with_source(source)
            .with_help("split the stylesheet into smaller source units"),
        );
        return StylesheetOutput::new(content, diagnostics);
    }

    // Keep the common no-apply path allocation-free and preserve authored formatting exactly.
    if !content.as_bytes().windows("@apply".len()).any(|window| window == b"@apply") {
        return StylesheetOutput::new(content, diagnostics);
    }

    if let Err(error) = validate_css_tokens(&content) {
        diagnostics.push(stylesheet_diagnostic(
            "stylesheet.invalid-syntax",
            error.to_string(),
            source.clone(),
            span_for_offset(&content, error.offset()),
            Some("fix the CSS syntax before using @apply"),
        ));
        return StylesheetOutput::new(content, diagnostics);
    }

    let mut transformer = Transformer { compiler, source, content: &content, diagnostics };
    let document = match parse_nodes(&mut transformer, 0, content.len(), ScopeKind::Root) {
        Ok(nodes) => nodes,
        Err(error) => {
            transformer.diagnostics.push(stylesheet_diagnostic(
                "stylesheet.invalid-syntax",
                error.to_string(),
                transformer.source.clone(),
                span_for_offset(transformer.content, error.offset()),
                Some("fix the CSS syntax before using @apply"),
            ));
            let diagnostics = std::mem::take(&mut transformer.diagnostics);
            drop(transformer);
            return StylesheetOutput::new(content, diagnostics);
        }
    };
    let css = serialize_nodes(&document, mode, 0, &mut CssSerializationContext { mode });
    StylesheetOutput::new(css, transformer.diagnostics)
}

struct Transformer<'a> {
    compiler: &'a mut Compiler,
    source: SourceId,
    content: &'a str,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopeKind {
    Root,
    DeclarationBlock { allow_apply: bool, nested_style: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Node {
    Statement(String),
    Block { prelude: String, body: Body, generated: Vec<CssRule> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Body {
    Nodes(Vec<Node>),
    Items(Vec<BodyItem>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BodyItem {
    Statement(String),
    Nested(Node),
    Apply { declarations: Vec<CssDeclaration>, generated: Vec<CssRule> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParseFailure {
    offset: usize,
    message: String,
}

impl ParseFailure {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self { offset, message: message.into() }
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for ParseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

fn parse_nodes(
    transformer: &mut Transformer<'_>,
    start: usize,
    end: usize,
    scope: ScopeKind,
) -> Result<Vec<Node>, ParseFailure> {
    let mut nodes = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let boundary = next_boundary(transformer.content, cursor, end)?;
        match boundary {
            Boundary::Semicolon { end: item_end } => {
                let raw = &transformer.content[cursor..item_end];
                if is_apply_statement(raw) {
                    transformer.diagnostics.push(stylesheet_diagnostic(
                        "apply.invalid-context",
                        "@apply is only supported directly inside a CSS style rule",
                        transformer.source.clone(),
                        span_for_range(cursor, item_end),
                        Some("move @apply into a non-nested style rule"),
                    ));
                }
                nodes.push(Node::Statement(raw.to_owned()));
                cursor = item_end;
            }
            Boundary::Block { open, close } => {
                let (leading, prelude) = split_block_prelude(&transformer.content[cursor..open]);
                if prelude.is_empty() {
                    return Err(ParseFailure::new(
                        open,
                        "CSS block is missing a selector or at-rule",
                    ));
                }
                if !leading.trim().is_empty() {
                    nodes.push(Node::Statement(leading));
                }
                let (body, generated) = if prelude.starts_with('@') {
                    if is_declaration_at_rule(&prelude) {
                        (
                            Body::Items(parse_items(
                                transformer,
                                open + 1,
                                close,
                                ScopeKind::DeclarationBlock {
                                    allow_apply: false,
                                    nested_style: false,
                                },
                            )?),
                            Vec::new(),
                        )
                    } else {
                        (
                            Body::Nodes(parse_nodes(
                                transformer,
                                open + 1,
                                close,
                                if is_keyframes_at_rule(&prelude) {
                                    ScopeKind::DeclarationBlock {
                                        allow_apply: false,
                                        nested_style: true,
                                    }
                                } else {
                                    ScopeKind::Root
                                },
                            )?),
                            Vec::new(),
                        )
                    }
                } else {
                    transform_style_body(
                        transformer,
                        &prelude,
                        open + 1,
                        close,
                        matches!(scope, ScopeKind::Root),
                    )?
                };
                nodes.push(Node::Block { prelude, body, generated });
                cursor = close + 1;
            }
            Boundary::End => {
                if cursor < end {
                    let raw = &transformer.content[cursor..end];
                    if is_apply_statement(raw) {
                        transformer.diagnostics.push(stylesheet_diagnostic(
                            "apply.invalid-context",
                            "@apply is only supported directly inside a CSS style rule",
                            transformer.source.clone(),
                            span_for_range(cursor, end),
                            Some("move @apply into a non-nested style rule and terminate it with `;`"),
                        ));
                    }
                    nodes.push(Node::Statement(raw.to_owned()));
                }
                cursor = end;
            }
        }
    }
    Ok(nodes)
}

fn parse_items(
    transformer: &mut Transformer<'_>,
    start: usize,
    end: usize,
    scope: ScopeKind,
) -> Result<Vec<BodyItem>, ParseFailure> {
    let mut items = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let boundary = next_boundary(transformer.content, cursor, end)?;
        match boundary {
            Boundary::Semicolon { end: item_end } => {
                let raw = &transformer.content[cursor..item_end];
                items.push(transform_statement(transformer, raw, cursor, scope));
                cursor = item_end;
            }
            Boundary::Block { open, close } => {
                let (leading, prelude) = split_block_prelude(&transformer.content[cursor..open]);
                if prelude.is_empty() {
                    return Err(ParseFailure::new(open, "nested CSS block is missing a prelude"));
                }
                if !leading.trim().is_empty() {
                    items.push(BodyItem::Statement(leading));
                }
                // Nested style composition requires selector expansion across the nesting tree.
                // Parse and preserve the block, but keep explicit @apply diagnostics strict.
                let nested_body = if prelude.starts_with('@') && !is_declaration_at_rule(&prelude) {
                    Body::Nodes(parse_nodes(transformer, open + 1, close, ScopeKind::Root)?)
                } else {
                    Body::Items(parse_items(
                        transformer,
                        open + 1,
                        close,
                        ScopeKind::DeclarationBlock { allow_apply: false, nested_style: true },
                    )?)
                };
                items.push(BodyItem::Nested(Node::Block {
                    prelude,
                    body: nested_body,
                    generated: Vec::new(),
                }));
                cursor = close + 1;
            }
            Boundary::End => {
                if cursor < end {
                    let raw = &transformer.content[cursor..end];
                    if !raw.trim().is_empty() {
                        items.push(transform_statement(transformer, raw, cursor, scope));
                    }
                }
                cursor = end;
            }
        }
    }
    Ok(items)
}

fn transform_statement(
    transformer: &mut Transformer<'_>,
    raw: &str,
    offset: usize,
    scope: ScopeKind,
) -> BodyItem {
    if !is_apply_statement(raw) {
        return BodyItem::Statement(raw.to_owned());
    }

    let can_apply =
        matches!(scope, ScopeKind::DeclarationBlock { allow_apply: true, nested_style: false });
    if !can_apply {
        let code = if matches!(scope, ScopeKind::DeclarationBlock { nested_style: true, .. }) {
            "apply.unsupported"
        } else {
            "apply.invalid-context"
        };
        transformer.diagnostics.push(stylesheet_diagnostic(
            code,
            "@apply is only supported directly inside a CSS style rule",
            transformer.source.clone(),
            span_for_range(offset, offset.saturating_add(raw.len())),
            Some("move @apply into a non-nested style rule"),
        ));
        return BodyItem::Statement(raw.to_owned());
    }

    let parsed = match parse_apply_candidates(raw, offset) {
        Ok(parsed) => parsed,
        Err(error) => {
            transformer.diagnostics.push(stylesheet_diagnostic(
                "apply.invalid-syntax",
                error.to_string(),
                transformer.source.clone(),
                span_for_offset(transformer.content, error.offset()),
                Some("separate utility candidates with whitespace and balance arbitrary values"),
            ));
            return BodyItem::Statement(raw.to_owned());
        }
    };
    if parsed.candidates.is_empty() {
        transformer.diagnostics.push(stylesheet_diagnostic(
            "apply.invalid-syntax",
            "@apply requires at least one utility candidate",
            transformer.source.clone(),
            span_for_range(offset, offset.saturating_add(raw.len())),
            Some("add one or more registered utility candidates"),
        ));
        return BodyItem::Statement(raw.to_owned());
    }

    // The containing selector is supplied by the style-node transform below. The parser first
    // stores directives as statements; this branch is replaced by transform_style_body.
    BodyItem::Statement(raw.to_owned())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedApply {
    candidates: Vec<ApplyCandidate>,
    important: bool,
}

fn parse_apply_candidates(raw: &str, offset: usize) -> Result<ParsedApply, ParseFailure> {
    let Some(keyword_start) = find_apply_keyword(raw) else {
        return Err(ParseFailure::new(offset, "invalid @apply directive"));
    };
    let content_start = keyword_start + "@apply".len();
    let content_end = raw.strip_suffix(';').map_or(raw.len(), str::len);
    let body = &raw[content_start..content_end];
    let mut tokens = split_apply_tokens(body, offset + content_start)?;
    let important =
        tokens.last().is_some_and(|candidate| candidate.raw().eq_ignore_ascii_case("!important"));
    if important {
        tokens.pop();
    }
    if tokens.iter().any(|candidate| candidate.raw().eq_ignore_ascii_case("!important")) {
        return Err(ParseFailure::new(
            offset + content_start,
            "`!important` is only supported at the end of an @apply directive",
        ));
    }
    Ok(ParsedApply { candidates: tokens, important })
}

fn split_apply_tokens(body: &str, offset: usize) -> Result<Vec<ApplyCandidate>, ParseFailure> {
    let mut candidates = Vec::new();
    let mut cursor = 0;
    while cursor < body.len() {
        cursor = skip_trivia(body, cursor)?;
        if cursor >= body.len() {
            break;
        }
        let start = cursor;
        let mut square = 0_u32;
        let mut parentheses = 0_u32;
        let mut quote = None;
        let mut escaped = false;
        while cursor < body.len() {
            let character =
                body[cursor..].chars().next().expect("cursor is always a UTF-8 boundary");
            let width = character.len_utf8();
            if escaped {
                escaped = false;
                cursor += width;
                continue;
            }
            if character == '\\' {
                escaped = true;
                cursor += width;
                continue;
            }
            if let Some(quote_character) = quote {
                if character == quote_character {
                    quote = None;
                }
                cursor += width;
                continue;
            }
            if character == '\'' || character == '"' {
                quote = Some(character);
                cursor += width;
                continue;
            }
            if body[cursor..].starts_with("/*") {
                let Some(end) = body[cursor + 2..].find("*/") else {
                    return Err(ParseFailure::new(offset + cursor, "unterminated CSS comment"));
                };
                cursor += end + 4;
                continue;
            }
            match character {
                '[' => square = square.saturating_add(1),
                ']' => {
                    if square == 0 {
                        return Err(ParseFailure::new(offset + cursor, "unmatched `]` in @apply"));
                    }
                    square -= 1;
                }
                '(' => parentheses = parentheses.saturating_add(1),
                ')' => {
                    if parentheses == 0 {
                        return Err(ParseFailure::new(offset + cursor, "unmatched `)` in @apply"));
                    }
                    parentheses -= 1;
                }
                character if character.is_whitespace() && square == 0 && parentheses == 0 => break,
                _ => {}
            }
            cursor += width;
        }
        if quote.is_some() || square != 0 || parentheses != 0 {
            return Err(ParseFailure::new(offset + start, "unbalanced value in @apply"));
        }
        if start == cursor {
            return Err(ParseFailure::new(offset + cursor, "empty @apply candidate"));
        }
        let raw = &body[start..cursor];
        let start_u32 = u32::try_from(offset + start)
            .map_err(|_| ParseFailure::new(offset + start, "stylesheet source is too large"))?;
        let end_u32 = u32::try_from(offset + cursor)
            .map_err(|_| ParseFailure::new(offset + cursor, "stylesheet source is too large"))?;
        let span = Span::new(start_u32, end_u32)
            .ok_or_else(|| ParseFailure::new(offset + start, "invalid @apply source span"))?;
        candidates.push(ApplyCandidate::new(raw, span));
    }
    Ok(candidates)
}

fn skip_trivia(input: &str, mut cursor: usize) -> Result<usize, ParseFailure> {
    loop {
        while cursor < input.len()
            && input[cursor..].chars().next().is_some_and(char::is_whitespace)
        {
            cursor += input[cursor..].chars().next().map_or(1, char::len_utf8);
        }
        if !input[cursor..].starts_with("/*") {
            return Ok(cursor);
        }
        let Some(end) = input[cursor + 2..].find("*/") else {
            return Err(ParseFailure::new(cursor, "unterminated CSS comment"));
        };
        cursor += end + 4;
    }
}

fn find_apply_keyword(raw: &str) -> Option<usize> {
    let trimmed_start = raw.len() - trim_leading_css_trivia(raw).len();
    let rest = &raw[trimmed_start..];
    if !rest.starts_with("@apply") {
        return None;
    }
    let boundary = rest["@apply".len()..].chars().next();
    if boundary.is_none_or(|character| character.is_whitespace() || character == ';') {
        Some(trimmed_start)
    } else {
        None
    }
}

fn trim_leading_css_trivia(input: &str) -> &str {
    &input[leading_css_trivia_end(input)..]
}

fn leading_css_trivia_end(input: &str) -> usize {
    let mut cursor = 0;
    loop {
        while cursor < input.len() {
            let character = input[cursor..].chars().next().expect("cursor is a UTF-8 boundary");
            if !character.is_whitespace() {
                break;
            }
            cursor += character.len_utf8();
        }
        if !input[cursor..].starts_with("/*") {
            return cursor;
        }
        let Some(end) = input[cursor + 2..].find("*/") else {
            return cursor;
        };
        cursor += end + 4;
    }
}

fn split_block_prelude(raw: &str) -> (String, String) {
    let leading_end = leading_css_trivia_end(raw);
    (raw[..leading_end].to_owned(), raw[leading_end..].trim().to_owned())
}

fn is_apply_statement(raw: &str) -> bool {
    find_apply_keyword(raw).is_some()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Boundary {
    Semicolon { end: usize },
    Block { open: usize, close: usize },
    End,
}

fn next_boundary(input: &str, start: usize, end: usize) -> Result<Boundary, ParseFailure> {
    let mut cursor = start;
    let mut parentheses = 0_u32;
    let mut square = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    while cursor < end {
        let character = input[cursor..].chars().next().expect("cursor is always a UTF-8 boundary");
        let width = character.len_utf8();
        if escaped {
            escaped = false;
            cursor += width;
            continue;
        }
        if character == '\\' {
            escaped = true;
            cursor += width;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            cursor += width;
            continue;
        }
        if input[cursor..].starts_with("/*") {
            let Some(comment_end) = input[cursor + 2..end].find("*/") else {
                return Err(ParseFailure::new(cursor, "unterminated CSS comment"));
            };
            cursor += comment_end + 4;
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            cursor += width;
            continue;
        }
        match character {
            '(' => parentheses = parentheses.saturating_add(1),
            ')' if parentheses > 0 => parentheses -= 1,
            ']' if square == 0 => {
                return Err(ParseFailure::new(cursor, "unmatched `]` in CSS"));
            }
            ']' => square -= 1,
            '[' => square = square.saturating_add(1),
            ';' if parentheses == 0 && square == 0 => {
                return Ok(Boundary::Semicolon { end: cursor + width });
            }
            '{' if parentheses == 0 && square == 0 => {
                let close = matching_brace(input, cursor, end)?;
                return Ok(Boundary::Block { open: cursor, close });
            }
            '}' if parentheses == 0 && square == 0 => return Ok(Boundary::End),
            _ => {}
        }
        cursor += width;
    }
    if quote.is_some() {
        return Err(ParseFailure::new(start, "unterminated CSS string"));
    }
    if parentheses != 0 || square != 0 {
        return Err(ParseFailure::new(start, "unbalanced CSS function or bracket"));
    }
    Ok(Boundary::End)
}

fn matching_brace(input: &str, open: usize, end: usize) -> Result<usize, ParseFailure> {
    let mut cursor = open + 1;
    let mut depth = 1_u32;
    let mut parentheses = 0_u32;
    let mut square = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    while cursor < end {
        let character = input[cursor..].chars().next().expect("cursor is always a UTF-8 boundary");
        let width = character.len_utf8();
        if escaped {
            escaped = false;
            cursor += width;
            continue;
        }
        if character == '\\' {
            escaped = true;
            cursor += width;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            cursor += width;
            continue;
        }
        if input[cursor..].starts_with("/*") {
            let Some(comment_end) = input[cursor + 2..end].find("*/") else {
                return Err(ParseFailure::new(cursor, "unterminated CSS comment"));
            };
            cursor += comment_end + 4;
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            cursor += width;
            continue;
        }
        match character {
            '(' => parentheses = parentheses.saturating_add(1),
            ')' if parentheses > 0 => parentheses -= 1,
            '[' => square = square.saturating_add(1),
            ']' if square > 0 => square -= 1,
            '{' if parentheses == 0 && square == 0 => depth = depth.saturating_add(1),
            '}' if parentheses == 0 && square == 0 => {
                depth -= 1;
                if depth == 0 {
                    return Ok(cursor);
                }
            }
            _ => {}
        }
        cursor += width;
    }
    Err(ParseFailure::new(open, "unterminated CSS block"))
}

fn is_declaration_at_rule(prelude: &str) -> bool {
    let name = prelude
        .trim_start_matches('@')
        .split(|character: char| character.is_whitespace() || character == '(')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(name.as_str(), "font-face" | "page" | "property" | "counter-style" | "viewport")
}

fn is_keyframes_at_rule(prelude: &str) -> bool {
    let name = prelude
        .trim_start_matches('@')
        .split(|character: char| character.is_whitespace() || character == '(')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(name.as_str(), "keyframes" | "-webkit-keyframes" | "-moz-keyframes")
}

fn validate_css_tokens(content: &str) -> Result<(), ParseFailure> {
    let mut input = ParserInput::new(content);
    let mut parser = Parser::new(&mut input);
    parser.expect_no_error_token().map_err(|error| {
        ParseFailure::new(0, format!("CSS tokenizer rejected the stylesheet: {error:?}"))
    })
}

fn transform_style_body(
    transformer: &mut Transformer<'_>,
    selector: &str,
    body_start: usize,
    body_end: usize,
    allow_apply: bool,
) -> Result<(Body, Vec<CssRule>), ParseFailure> {
    let mut items = Vec::new();
    let mut generated = Vec::new();
    let mut cursor = body_start;
    while cursor < body_end {
        let boundary = next_boundary(transformer.content, cursor, body_end)?;
        match boundary {
            Boundary::Semicolon { end: item_end } => {
                let raw = &transformer.content[cursor..item_end];
                if is_apply_statement(raw) && allow_apply {
                    let leading_end = leading_css_trivia_end(raw);
                    let leading = &raw[..leading_end];
                    let parsed = parse_apply_candidates(raw, cursor)?;
                    if parsed.candidates.is_empty() {
                        transformer.diagnostics.push(stylesheet_diagnostic(
                            "apply.invalid-syntax",
                            "@apply requires at least one utility candidate",
                            transformer.source.clone(),
                            span_for_range(cursor, item_end),
                            Some("add one or more registered utility candidates"),
                        ));
                        items.push(BodyItem::Statement(raw.to_owned()));
                    } else {
                        if !leading.trim().is_empty() {
                            items.push(BodyItem::Statement(leading.to_owned()));
                        }
                        let output = transformer.compiler.compose(CompositionInput {
                            source: &transformer.source,
                            selector,
                            candidates: &parsed.candidates,
                        });
                        transformer.diagnostics.extend(output.diagnostics);
                        let mut declarations = Vec::new();
                        for rule in output.rules {
                            let rule =
                                if parsed.important { rule.with_important(true) } else { rule };
                            if rule.selector() == Some(selector) {
                                if let Some(rule_declarations) = rule.declarations() {
                                    declarations.extend(rule_declarations.iter().cloned());
                                }
                            } else {
                                generated.push(rule);
                            }
                        }
                        items.push(BodyItem::Apply { declarations, generated: Vec::new() });
                    }
                } else if is_apply_statement(raw) {
                    transformer.diagnostics.push(stylesheet_diagnostic(
                        if allow_apply { "apply.invalid-context" } else { "apply.unsupported" },
                        "@apply is only supported directly inside a non-nested style rule",
                        transformer.source.clone(),
                        span_for_range(cursor, item_end),
                        Some("move @apply into a supported style rule"),
                    ));
                    items.push(BodyItem::Statement(raw.to_owned()));
                } else {
                    items.push(BodyItem::Statement(raw.to_owned()));
                }
                cursor = item_end;
            }
            Boundary::Block { open, close } => {
                let (leading, prelude) = split_block_prelude(&transformer.content[cursor..open]);
                if prelude.is_empty() {
                    return Err(ParseFailure::new(open, "nested CSS block is missing a prelude"));
                }
                if !leading.trim().is_empty() {
                    items.push(BodyItem::Statement(leading));
                }
                let nested = if prelude.starts_with('@') && !is_declaration_at_rule(&prelude) {
                    Node::Block {
                        prelude,
                        body: Body::Nodes(parse_nodes(
                            transformer,
                            open + 1,
                            close,
                            ScopeKind::DeclarationBlock { allow_apply: false, nested_style: true },
                        )?),
                        generated: Vec::new(),
                    }
                } else {
                    let (body, generated) =
                        transform_style_body(transformer, selector, open + 1, close, false)?;
                    Node::Block { prelude, body, generated }
                };
                items.push(BodyItem::Nested(nested));
                cursor = close + 1;
            }
            Boundary::End => {
                if cursor < body_end && !transformer.content[cursor..body_end].trim().is_empty() {
                    let raw = &transformer.content[cursor..body_end];
                    if is_apply_statement(raw) {
                        transformer.diagnostics.push(stylesheet_diagnostic(
                            "apply.invalid-syntax",
                            "@apply must end with a semicolon",
                            transformer.source.clone(),
                            span_for_range(cursor, body_end),
                            Some("terminate the @apply directive with `;`"),
                        ));
                    }
                    items.push(BodyItem::Statement(raw.to_owned()));
                }
                cursor = body_end;
            }
        }
    }
    Ok((Body::Items(items), generated))
}

fn serialize_nodes(
    nodes: &[Node],
    mode: CssSerializationMode,
    indent: usize,
    context: &mut CssSerializationContext,
) -> String {
    let separator = if mode == CssSerializationMode::Pretty { "\n" } else { "" };
    nodes
        .iter()
        .map(|node| serialize_node(node, mode, indent, context))
        .filter(|node| !node.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

struct CssSerializationContext {
    mode: CssSerializationMode,
}

fn serialize_node(
    node: &Node,
    mode: CssSerializationMode,
    indent: usize,
    context: &mut CssSerializationContext,
) -> String {
    match node {
        Node::Statement(raw) => canonical_statement(raw, mode, indent),
        Node::Block { prelude, body, generated } => {
            let body_css = match body {
                Body::Nodes(nodes) => serialize_nodes(nodes, mode, indent + 2, context),
                Body::Items(items) => serialize_items(items, mode, indent + 2, context),
            };
            let prefix = if mode == CssSerializationMode::Pretty {
                format!("{}{}", indentation(indent), prelude.trim())
            } else {
                prelude.trim().to_owned()
            };
            let mut output = if mode == CssSerializationMode::Pretty {
                if body_css.is_empty() {
                    format!("{prefix} {{}}")
                } else {
                    format!("{prefix} {{\n{body_css}\n{}}}", indentation(indent))
                }
            } else {
                format!("{prefix}{{{body_css}}}")
            };
            if !generated.is_empty() {
                let mut document = CssDocument::new();
                for rule in generated {
                    document.push(rule.clone());
                }
                let generated_css = document.to_css(context.mode);
                if !generated_css.is_empty() {
                    if mode == CssSerializationMode::Pretty {
                        output.push('\n');
                        output.push_str(&indent_block(&generated_css, indent));
                    } else {
                        output.push_str(&generated_css);
                    }
                }
            }
            output
        }
    }
}

fn serialize_items(
    items: &[BodyItem],
    mode: CssSerializationMode,
    indent: usize,
    context: &mut CssSerializationContext,
) -> String {
    let separator = if mode == CssSerializationMode::Pretty { "\n" } else { "" };
    items
        .iter()
        .map(|item| match item {
            BodyItem::Statement(raw) => canonical_statement(raw, mode, indent),
            BodyItem::Nested(node) => serialize_node(node, mode, indent, context),
            BodyItem::Apply { declarations, generated } => {
                let mut output = declarations
                    .iter()
                    .map(|declaration| serialize_declaration(declaration, mode, indent))
                    .collect::<Vec<_>>()
                    .join(separator);
                if !generated.is_empty() {
                    let mut document = CssDocument::new();
                    for rule in generated {
                        document.push(rule.clone());
                    }
                    let generated_css = document.to_css(context.mode);
                    if !generated_css.is_empty() {
                        if !output.is_empty() && mode == CssSerializationMode::Pretty {
                            output.push('\n');
                        }
                        if mode == CssSerializationMode::Pretty {
                            output
                                .push_str(&indent_block(&generated_css, indent.saturating_sub(2)));
                        } else {
                            output.push_str(&generated_css);
                        }
                    }
                }
                output
            }
        })
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

fn canonical_statement(raw: &str, mode: CssSerializationMode, indent: usize) -> String {
    let leading_end = leading_css_trivia_end(raw);
    let leading = raw[..leading_end].trim();
    let trimmed = raw[leading_end..].trim();
    if leading.is_empty() && trimmed.is_empty() {
        return String::new();
    }
    if trimmed.is_empty() {
        return if mode == CssSerializationMode::Pretty {
            indent_block(leading, indent)
        } else {
            leading.to_owned()
        };
    }
    let without_semicolon = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();
    let statement = if without_semicolon.starts_with('@') {
        format!("{without_semicolon};")
    } else if let Some(colon) = find_top_level_colon(without_semicolon) {
        let property = without_semicolon[..colon].trim();
        let value = without_semicolon[colon + 1..].trim();
        if property.is_empty() {
            format!("{without_semicolon};")
        } else {
            format!("{property}:{value};")
        }
    } else {
        format!("{without_semicolon};")
    };
    let result = if leading.is_empty() {
        statement
    } else if mode == CssSerializationMode::Pretty {
        format!(
            "{}\n{}{}",
            indent_block(leading, indent),
            indentation(indent),
            pretty_statement(&statement)
        )
    } else {
        format!("{leading}{statement}")
    };
    if mode == CssSerializationMode::Pretty {
        if leading.is_empty() {
            format!("{}{}", indentation(indent), pretty_statement(&result))
        } else {
            result
        }
    } else {
        result
    }
}

fn pretty_statement(statement: &str) -> String {
    if let Some(colon) = find_top_level_colon(statement) {
        format!("{}: {}", &statement[..colon], statement[colon + 1..].trim_start())
    } else {
        statement.to_owned()
    }
}

fn serialize_declaration(
    declaration: &CssDeclaration,
    mode: CssSerializationMode,
    indent: usize,
) -> String {
    let important = if declaration.is_important() { " !important" } else { "" };
    if mode == CssSerializationMode::Pretty {
        format!(
            "{}{}: {}{};",
            indentation(indent),
            declaration.property(),
            declaration.value(),
            important
        )
    } else {
        format!("{}:{}{};", declaration.property(), declaration.value(), important.trim())
    }
}

fn find_top_level_colon(input: &str) -> Option<usize> {
    let mut cursor = 0;
    let mut parentheses = 0_u32;
    let mut square = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    while cursor < input.len() {
        let character = input[cursor..].chars().next()?;
        let width = character.len_utf8();
        if escaped {
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else {
            match character {
                '(' => parentheses = parentheses.saturating_add(1),
                ')' if parentheses > 0 => parentheses -= 1,
                '[' => square = square.saturating_add(1),
                ']' if square > 0 => square -= 1,
                ':' if parentheses == 0 && square == 0 => return Some(cursor),
                _ => {}
            }
        }
        cursor += width;
    }
    None
}

fn indentation(width: usize) -> String {
    " ".repeat(width)
}

fn indent_block(input: &str, indent: usize) -> String {
    let prefix = indentation(indent);
    input.lines().map(|line| format!("{prefix}{line}")).collect::<Vec<_>>().join("\n")
}

fn stylesheet_diagnostic(
    code: &'static str,
    message: impl Into<String>,
    source: SourceId,
    span: Span,
    help: Option<&str>,
) -> Diagnostic {
    let diagnostic =
        Diagnostic::error(DiagnosticCode::new(code), message).with_source(source).with_span(span);
    help.map_or(diagnostic.clone(), |help| diagnostic.with_help(help))
}

fn span_for_range(start: usize, end: usize) -> Span {
    let Some(start) = u32::try_from(start).ok() else {
        return Span::empty(0);
    };
    let Some(end) = u32::try_from(end).ok() else {
        return Span::empty(start);
    };
    Span::new(start, end).unwrap_or_else(|| Span::empty(start))
}

fn span_for_offset(content: &str, offset: usize) -> Span {
    let offset = offset.min(content.len());
    span_for_range(offset, offset.saturating_add(1).min(content.len()))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use utilitycss_compiler::{Compiler, CompilerConfig};
    use utilitycss_span::{SourceId, Span};

    use super::{transform_stylesheet, StylesheetInput};

    fn transform(css: &str) -> super::StylesheetOutput {
        let mut compiler = Compiler::new(CompilerConfig::new());
        transform_stylesheet(&mut compiler, StylesheetInput::new(SourceId::new("styles.css"), css))
    }

    #[test]
    fn composes_basic_utilities_and_preserves_declarations() {
        let output = transform(".button { color: red; @apply flex p-4; display: block; }");

        assert_eq!(output.diagnostics().len(), 0);
        assert_eq!(output.css(), ".button{color:red;display:flex;padding:1rem;display:block;}");
    }

    #[test]
    fn supports_multiple_directives_at_their_author_positions() {
        let output =
            transform(".button { @apply flex items-center; color: red; @apply p-4 rounded; }");

        assert_eq!(output.diagnostics().len(), 0);
        assert_eq!(
            output.css(),
            ".button{display:flex;align-items:center;color:red;padding:1rem;border-radius:0.25rem;}"
        );
    }

    #[test]
    fn composes_variants_and_responsive_rules_into_the_author_selector() {
        let output = transform(".button { @apply hover:bg-red-500 md:p-8; }");

        assert_eq!(output.diagnostics().len(), 0);
        assert!(!output.css().contains(".button{ }"));
        assert!(output.css().contains(".button:hover{background-color:#ef4444;}"));
        assert!(output.css().contains("@media (min-width: 768px){.button{padding:2rem;}}"));
    }

    #[test]
    fn composes_registered_attribute_ancestor_dark_and_arbitrary_variants() {
        let output = transform(
            ".item { @apply dark:bg-black group-hover:text-white data-[state=open]:p-4 [&>*]:rounded; }",
        );

        assert!(output.diagnostics().is_empty());
        assert!(output
            .css()
            .contains("@media (prefers-color-scheme: dark){.item{background-color:#000;}}"));
        assert!(output.css().contains(".group:hover .item{color:#fff;}"));
        assert!(output.css().contains(".item[data-state=open]{padding:1rem;}"));
        assert!(output.css().contains(".item>*{border-radius:0.25rem;}"));
    }

    #[test]
    fn unknown_apply_candidates_are_strict_and_span_specific() {
        let input = "/* café  */\n.button { @apply p-4 invalid rounded; }";
        let output = transform(input);

        assert!(output.css().contains("padding:1rem;"));
        let diagnostic = output.diagnostics().first().expect("unknown utility diagnostic");
        assert_eq!(diagnostic.code().as_str(), "apply.unknown-utility");
        assert_eq!(diagnostic.span(), Span::new(34, 41));
    }

    #[test]
    fn arbitrary_values_and_important_are_lowered_by_the_registry() {
        let output = transform(".card { @apply w-[42rem] p-4 !important; }");

        assert_eq!(output.diagnostics().len(), 0);
        assert!(output.css().contains("width:42rem!important;"));
        assert!(output.css().contains("padding:1rem!important;"));
    }

    #[test]
    fn preserves_functions_custom_properties_and_outer_at_rules() {
        let output = transform(
            "@media (min-width: 640px) { .button { color: rgb(10 20 30); --custom: calc(100% - 2rem); @apply p-4; container-type: inline-size; } }",
        );

        assert!(output.diagnostics().is_empty());
        assert_eq!(
            output.css(),
            "@media (min-width: 640px){.button{color:rgb(10 20 30);--custom:calc(100% - 2rem);padding:1rem;container-type:inline-size;}}"
        );
    }

    #[test]
    fn resolves_registered_custom_utilities_through_the_same_registry() {
        let mut registry = utilitycss_utilities::UtilityRegistry::default();
        registry.register(
            "custom-surface",
            utilitycss_utilities::UtilityDefinition::static_declaration("background", "canvas", 70),
        );
        let mut compiler = Compiler::new(
            utilitycss_compiler::CompilerConfig::new().with_utility_registry(registry),
        );
        let output = transform_stylesheet(
            &mut compiler,
            StylesheetInput::new(SourceId::new("styles.css"), ".card { @apply custom-surface; }"),
        );

        assert!(output.diagnostics().is_empty());
        assert_eq!(output.css(), ".card{background:canvas;}");
    }

    #[test]
    fn no_apply_stylesheets_keep_authored_bytes_unchanged() {
        let input = "/* keep */\n.button { color: rgb(10 20 30); }\n";
        let output = transform(input);

        assert_eq!(output.css(), input);
        assert!(output.diagnostics().is_empty());
    }

    #[test]
    fn preserves_comments_around_authored_declarations_and_apply() {
        let output = transform(
            "/* top */\n.button { /* before */ color: red; /* apply */ @apply p-4; /* after */ display: block; }",
        );

        assert!(output.diagnostics().is_empty());
        assert!(output.css().contains("/* top */"));
        assert!(output.css().contains("/* before */"));
        assert!(output.css().contains("color:red;"));
        assert!(output.css().contains("/* apply */"));
        assert!(output.css().contains("padding:1rem;"));
        assert!(output.css().contains("/* after */"));
        assert!(output.css().contains("display:block;"));
    }

    #[test]
    fn malformed_apply_values_report_diagnostics_without_panicking() {
        let output = transform(".card { @apply w-[calc(100% - 2rem); }");

        assert!(!output.diagnostics().is_empty());
        assert!(output
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code().as_str() == "stylesheet.invalid-syntax"
                || diagnostic.code().as_str() == "apply.invalid-syntax"));
    }

    #[test]
    fn root_and_nested_apply_are_diagnosed_without_dropping_css() {
        let output = transform("@apply p-4; .button { @apply flex; &:hover { @apply p-8; } }");

        assert_eq!(output.diagnostics().len(), 2);
        assert!(output.css().contains("@apply p-4;"));
        assert!(output.css().contains("@apply p-8;"));
    }

    #[test]
    fn rejects_apply_inside_keyframes_without_reinterpreting_the_block() {
        let output = transform("@keyframes spin { from { @apply p-4; } }");

        assert_eq!(output.diagnostics().len(), 1);
        assert_eq!(output.diagnostics()[0].code().as_str(), "apply.unsupported");
        assert!(output.css().contains("@apply p-4;"));
    }

    #[test]
    fn diagnoses_root_apply_without_a_semicolon() {
        let output = transform("@apply p-4");

        assert_eq!(output.diagnostics().len(), 1);
        assert_eq!(output.diagnostics()[0].code().as_str(), "apply.invalid-context");
    }

    proptest! {
        #[test]
        fn arbitrary_utf8_stylesheets_never_panic(input in any::<String>()) {
            let _ = transform(&input);
        }
    }
}
