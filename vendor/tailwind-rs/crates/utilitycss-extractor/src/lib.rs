//! Conservative static candidate extraction for template and class-helper source.
//!
//! This crate narrows the language-agnostic scanner to contexts where a candidate is statically
//! visible: quoted `class`/`className` attributes and string literals inside known class helpers.
//! It does not evaluate user code. A future AST adapter can feed the same candidate spans into the
//! compiler without changing utility or variant semantics. Helper names are convention-based when
//! no host AST is available; AST-capable adapters should resolve bindings when possible.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeSet, error::Error, fmt, ops::Range, sync::OnceLock};

use regex::Regex;
use utilitycss_scanner::{scan, scan_checked, ExtractionMode};
use utilitycss_span::{validate_source_len, SourceSizeError, Span};

/// Framework syntax that can be statically extracted without evaluating application code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Framework {
    /// Plain HTML-like markup.
    #[default]
    Html,
    /// Vue single-file component templates.
    Vue,
    /// Svelte component markup.
    Svelte,
    /// Astro component markup.
    Astro,
}

/// A statically visible candidate and its byte span in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractedCandidate<'source> {
    raw: &'source str,
    span: Span,
    mode: ExtractionMode,
}

/// An extraction mode that cannot be fulfilled by this language-agnostic crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractionError {
    /// AST extraction must be performed by a host-language adapter such as the SWC extractor.
    AstRequiresHostExtractor,
    /// The source is larger than the 32-bit source-location model can represent.
    SourceTooLarge,
}

impl fmt::Display for ExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AstRequiresHostExtractor => formatter.write_str(
                "AST extraction requires a host-language extractor; use utilitycss-swc or supply candidates directly",
            ),
            Self::SourceTooLarge => formatter.write_str(
                "source is too large for the compiler's 32-bit source-location model",
            ),
        }
    }
}

impl Error for ExtractionError {}

impl<'source> ExtractedCandidate<'source> {
    /// Creates an extracted candidate from source text and a validated span.
    #[must_use]
    pub const fn new(raw: &'source str, span: Span) -> Self {
        Self { raw, span, mode: ExtractionMode::Static }
    }

    /// Creates an extracted candidate with an explicit discovery mode.
    #[must_use]
    pub const fn with_mode(raw: &'source str, span: Span, mode: ExtractionMode) -> Self {
        Self { raw, span, mode }
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

    /// Returns the extraction mode that produced this candidate.
    #[must_use]
    pub const fn mode(self) -> ExtractionMode {
        self.mode
    }
}

/// Extracts candidates using an explicit source-discovery mode.
pub fn extract_with_mode(
    source: &str,
    mode: ExtractionMode,
    framework: Framework,
) -> Result<Vec<ExtractedCandidate<'_>>, ExtractionError> {
    validate_source_len(source.len())
        .map_err(|_: SourceSizeError| ExtractionError::SourceTooLarge)?;
    match mode {
        ExtractionMode::Text => Ok(scan_checked(source)
            .map_err(|_: SourceSizeError| ExtractionError::SourceTooLarge)?
            .into_iter()
            .map(|token| ExtractedCandidate::with_mode(token.raw(), token.span(), mode))
            .collect()),
        ExtractionMode::Static => Ok(extract_for_framework(source, framework)?
            .into_iter()
            .map(|candidate| ExtractedCandidate::with_mode(candidate.raw(), candidate.span(), mode))
            .collect()),
        ExtractionMode::Ast => Err(ExtractionError::AstRequiresHostExtractor),
        ExtractionMode::Hybrid => {
            let mut candidates = extract_with_mode(source, ExtractionMode::Static, framework)?
                .into_iter()
                .map(|candidate| {
                    ExtractedCandidate::with_mode(candidate.raw(), candidate.span(), mode)
                })
                .collect::<Vec<_>>();
            let mut seen =
                candidates.iter().map(|candidate| candidate.span()).collect::<BTreeSet<_>>();
            for candidate in extract_with_mode(source, ExtractionMode::Text, framework)? {
                if seen.insert(candidate.span()) {
                    candidates.push(ExtractedCandidate::with_mode(
                        candidate.raw(),
                        candidate.span(),
                        mode,
                    ));
                }
            }
            candidates.sort_by_key(|candidate| (candidate.span().start(), candidate.span().end()));
            Ok(candidates)
        }
    }
}

/// Extracts candidates from static class attributes and known class helper calls.
///
/// The result preserves scanner order and duplicate occurrences. Dynamic template strings and
/// unrelated string literals are intentionally ignored.
///
/// # Errors
///
/// Returns [`ExtractionError::SourceTooLarge`] when the source exceeds the representable size
/// limit, so oversized inputs are never mistaken for sources without candidates.
pub fn extract(source: &str) -> Result<Vec<ExtractedCandidate<'_>>, ExtractionError> {
    validate_source_len(source.len())
        .map_err(|_: SourceSizeError| ExtractionError::SourceTooLarge)?;
    let mut ranges = attribute_ranges(source);
    ranges.extend(helper_string_ranges(source));
    normalize_ranges(&mut ranges);
    let comments = comment_ranges(source.as_bytes());
    let mut comments = comments;
    normalize_ranges(&mut comments);
    let mut range_index = 0;
    let mut comment_index = 0;

    Ok(scan(source)
        .into_iter()
        .filter(|token| {
            let start = usize::try_from(token.span().start()).unwrap_or(usize::MAX);
            let end = usize::try_from(token.span().end()).unwrap_or(usize::MAX);
            let candidate = start..end;
            !contains_ordered_range(&comments, &mut comment_index, &candidate)
                && contains_ordered_range(&ranges, &mut range_index, &candidate)
        })
        .map(|token| ExtractedCandidate::new(token.raw(), token.span()))
        .collect::<Vec<_>>())
}

/// Extracts candidates using the static class forms of a supported framework template.
///
/// This supplements [`extract`] with Vue `:class`/`v-bind:class` literals, Svelte `class:`
/// directives, and Astro `class:list` expressions. It deliberately does not evaluate bindings;
/// only source literals and directive names are returned.
///
/// # Errors
///
/// Returns [`ExtractionError::SourceTooLarge`] when the source exceeds the representable size
/// limit, so oversized inputs are never mistaken for sources without candidates.
pub fn extract_for_framework(
    source: &str,
    framework: Framework,
) -> Result<Vec<ExtractedCandidate<'_>>, ExtractionError> {
    validate_source_len(source.len())
        .map_err(|_: SourceSizeError| ExtractionError::SourceTooLarge)?;
    let mut candidates = extract(source)?;
    let ranges = match framework {
        Framework::Html => Vec::new(),
        Framework::Vue => vue_ranges(source),
        Framework::Svelte => svelte_ranges(source),
        Framework::Astro => astro_ranges(source),
    };
    let existing = candidates
        .iter()
        .map(|candidate| (candidate.span(), candidate.raw()))
        .collect::<BTreeSet<_>>();
    let mut seen = existing;
    let mut ranges = ranges;
    normalize_ranges(&mut ranges);
    let mut comments = comment_ranges(source.as_bytes());
    normalize_ranges(&mut comments);
    let mut comment_index = 0;
    for range in ranges {
        if contains_ordered_range(&comments, &mut comment_index, &range) {
            continue;
        }
        let Some(region) = source.get(range.clone()) else {
            continue;
        };
        for token in scan(region) {
            let local_start = usize::try_from(token.span().start()).unwrap_or(usize::MAX);
            let local_end = usize::try_from(token.span().end()).unwrap_or(usize::MAX);
            let Some(start) = range.start.checked_add(local_start) else {
                continue;
            };
            let Some(end) = range.start.checked_add(local_end) else {
                continue;
            };
            let Some(raw) = source.get(start..end) else {
                continue;
            };
            let (Some(start), Some(end)) = (u32::try_from(start).ok(), u32::try_from(end).ok())
            else {
                continue;
            };
            let Some(span) = Span::new(start, end) else {
                continue;
            };
            if seen.insert((span, raw)) {
                candidates.push(ExtractedCandidate::new(raw, span));
            }
        }
    }
    candidates.sort_by_key(|candidate| (candidate.span().start(), candidate.span().end()));
    Ok(candidates)
}

fn normalize_ranges(ranges: &mut Vec<Range<usize>>) {
    ranges.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges.drain(..) {
        if let Some(previous) = merged.last_mut() {
            if range.start <= previous.end {
                previous.end = previous.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    *ranges = merged;
}

fn contains_ordered_range(
    ranges: &[Range<usize>],
    index: &mut usize,
    candidate: &Range<usize>,
) -> bool {
    while ranges.get(*index).is_some_and(|range| range.end <= candidate.start) {
        *index += 1;
    }
    ranges
        .get(*index)
        .is_some_and(|range| range.start <= candidate.start && candidate.end <= range.end)
}

fn vue_ranges(source: &str) -> Vec<Range<usize>> {
    let regex = vue_class_binding_regex();
    regex
        .captures_iter(source)
        .flat_map(|captures| {
            let whole = captures.get(0)?;
            let value_start = whole.end();
            let quote = source.as_bytes().get(value_start).copied()?;
            if !matches!(quote, b'\'' | b'"') {
                return None;
            }
            let content_start = value_start + 1;
            let content_end = quoted_end(source.as_bytes(), content_start, quote)?;
            Some(string_ranges(source.as_bytes(), content_start, content_end))
        })
        .flatten()
        .collect()
}

fn svelte_ranges(source: &str) -> Vec<Range<usize>> {
    let regex = svelte_class_directive_regex();
    regex
        .captures_iter(source)
        .filter_map(|captures| captures.name("class").map(|class| class.start()..class.end()))
        .collect()
}

fn astro_ranges(source: &str) -> Vec<Range<usize>> {
    let regex = astro_class_list_regex();
    regex
        .captures_iter(source)
        .flat_map(|captures| {
            let whole = captures.get(0)?;
            let mut start = whole.end();
            while source.as_bytes().get(start).is_some_and(|byte| byte.is_ascii_whitespace()) {
                start += 1;
            }
            if source.as_bytes().get(start) != Some(&b'{') {
                return None;
            }
            let end = braced_expression_end(source.as_bytes(), start)?;
            Some(string_ranges(source.as_bytes(), start + 1, end))
        })
        .flatten()
        .collect()
}

fn vue_class_binding_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"(?i)(?::class|v-bind:class)\s*=\s*").expect("valid Vue regex"))
}

fn svelte_class_directive_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?x)\bclass:(?P<class>[A-Za-z0-9_:/\\.\[\]-]+)").expect("valid Svelte regex")
    })
}

fn astro_class_list_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(?i)\bclass:list\s*=\s*").expect("valid Astro regex"))
}

fn braced_expression_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut cursor = open;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if byte == b'{' {
            depth = depth.saturating_add(1);
        } else if byte == b'}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += 1;
    }
    None
}

fn attribute_ranges(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let names = [b"class".as_slice(), b"className", b"class:list", b"classList"];
    let mut ranges = Vec::new();
    let mut cursor = 0;
    let mut in_tag = false;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        if in_tag && matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
            cursor += 1;
            continue;
        }
        if !in_tag {
            if byte == b'<'
                && bytes
                    .get(cursor + 1)
                    .is_some_and(|next| next.is_ascii_alphabetic() || *next == b'/')
            {
                in_tag = true;
            }
            cursor += 1;
            continue;
        }
        if byte == b'>' {
            in_tag = false;
            cursor += 1;
            continue;
        }
        let Some(name) = names.iter().find(|name| starts_with_name(bytes, cursor, name)) else {
            cursor += 1;
            continue;
        };
        let mut after_name = cursor + name.len();
        while after_name < bytes.len() && bytes[after_name].is_ascii_whitespace() {
            after_name += 1;
        }
        if bytes.get(after_name) != Some(&b'=') {
            cursor += name.len();
            continue;
        }
        let mut value_start = after_name + 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        let Some(&quote) = bytes.get(value_start) else {
            break;
        };
        if !matches!(quote, b'\'' | b'"') {
            cursor += name.len();
            continue;
        }
        let content_start = value_start + 1;
        let content_end = quoted_end(bytes, content_start, quote).unwrap_or(bytes.len());
        if !source[content_start..content_end].contains("${") {
            ranges.push(content_start..content_end);
        }
        cursor = content_end.saturating_add(1);
    }
    ranges
}

fn helper_string_ranges(source: &str) -> Vec<Range<usize>> {
    let bytes = source.as_bytes();
    let helpers = ["clsx", "classnames", "cn", "cva", "tv", "twJoin", "twMerge"];
    let shadowed_helpers = declared_helpers(source);
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let Some(helper) =
            helpers.iter().find(|helper| starts_with_identifier(bytes, cursor, helper.as_bytes()))
        else {
            cursor += 1;
            continue;
        };
        if shadowed_helpers.contains(*helper) {
            cursor += helper.len();
            continue;
        }
        let mut open = cursor + helper.len();
        while open < bytes.len() && bytes[open].is_ascii_whitespace() {
            open += 1;
        }
        if bytes.get(open) != Some(&b'(') {
            cursor += helper.len();
            continue;
        }
        let Some(close) = matching_paren(bytes, open) else {
            cursor = open.saturating_add(1);
            continue;
        };
        ranges.extend(string_ranges(bytes, open + 1, close));
        cursor = close.saturating_add(1);
    }
    ranges
}

fn declared_helpers(source: &str) -> BTreeSet<String> {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = REGEX.get_or_init(|| {
        Regex::new(
            r"(?x)\b(?:function|class)\s+(clsx|classnames|cn|cva|tv|twJoin|twMerge)\b
                |\b(?:const|let|var)\s+(clsx|classnames|cn|cva|tv|twJoin|twMerge)\b",
        )
        .expect("valid helper declaration regex")
    });
    let mut comments = comment_ranges(source.as_bytes());
    normalize_ranges(&mut comments);
    let mut comment_index = 0;
    regex
        .captures_iter(source)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let whole_range = whole.start()..whole.end();
            if contains_ordered_range(&comments, &mut comment_index, &whole_range) {
                return None;
            }
            (1..=3).find_map(|index| captures.get(index).map(|name| name.as_str().to_owned()))
        })
        .collect()
}

fn string_ranges(bytes: &[u8], start: usize, end: usize) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let quote = bytes[cursor];
        if !matches!(quote, b'\'' | b'"' | b'`') {
            cursor += 1;
            continue;
        }
        let content_start = cursor + 1;
        let content_end = quoted_end(&bytes[..end], content_start, quote).unwrap_or(end);
        if quote != b'`' || !bytes[content_start..content_end].windows(2).any(|pair| pair == b"${")
        {
            ranges.push(content_start..content_end);
        }
        cursor = content_end.saturating_add(1);
    }
    ranges
}

fn starts_with_name(bytes: &[u8], start: usize, name: &[u8]) -> bool {
    starts_with_identifier(bytes, start, name)
        && bytes.get(start.wrapping_sub(1)) != Some(&b':')
        && (name != b"class" || bytes.get(start + name.len()) != Some(&b':'))
}

fn starts_with_identifier(bytes: &[u8], start: usize, name: &[u8]) -> bool {
    let Some(end) = start.checked_add(name.len()) else {
        return false;
    };
    if bytes.get(start..end) != Some(name) {
        return false;
    }
    let before_is_identifier = start > 0 && is_identifier_byte(bytes[start - 1]);
    let after_is_identifier = bytes.get(end).is_some_and(|byte| is_identifier_byte(*byte));
    !before_is_identifier && !after_is_identifier
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn quoted_end(bytes: &[u8], start: usize, quote: u8) -> Option<usize> {
    let mut cursor = start;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn matching_paren(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0_u32;
    let mut cursor = open;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
        } else if byte == b'(' {
            depth = depth.saturating_add(1);
        } else if byte == b')' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += 1;
    }
    None
}

fn comment_ranges(bytes: &[u8]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = 0;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if quote.is_none() && bytes.get(cursor..cursor + 4) == Some(b"<!--") {
            let start = cursor;
            cursor += 4;
            while cursor + 2 < bytes.len() && bytes.get(cursor..cursor + 3) != Some(b"-->") {
                cursor += 1;
            }
            cursor = (cursor + 3).min(bytes.len());
            ranges.push(start..cursor);
            continue;
        }
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            cursor += 1;
            continue;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"//") {
            let start = cursor;
            cursor += 2;
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            ranges.push(start..cursor);
        } else if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            let start = cursor;
            cursor += 2;
            while cursor + 1 < bytes.len() && bytes.get(cursor..cursor + 2) != Some(b"*/") {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            ranges.push(start..cursor);
        } else {
            cursor += 1;
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use utilitycss_scanner::ExtractionMode;

    use super::{extract, extract_for_framework, extract_with_mode, Framework};

    fn raws(source: &str) -> Vec<&str> {
        extract(source)
            .expect("fixture fits")
            .into_iter()
            .map(|candidate| candidate.raw())
            .collect()
    }

    #[test]
    fn extracts_only_static_class_attributes() {
        let source =
            r#"<div class="flex p-4" title="text-red-500"></div> const value = "bg-blue-500";"#;
        let candidates = raws(source);

        assert_eq!(candidates, vec!["flex", "p-4"]);
    }

    #[test]
    fn extracts_known_helper_literals_without_evaluating_code() {
        let source = r#"
            const classes = clsx("p-4", active && "bg-red-500", `text-${tone}`);
            const variant = cva("rounded", { variants: { size: { sm: "w-4" } } });
            const unrelated = "text-blue-500";
        "#;
        let candidates = raws(source);

        assert_eq!(candidates, vec!["p-4", "bg-red-500", "rounded", "w-4"]);
    }

    #[test]
    fn preserves_exact_source_spans() {
        let source = r#"<div className='hover:bg-red-500/50'></div>"#;
        let candidate = extract(source)
            .expect("fixture fits")
            .into_iter()
            .next()
            .expect("candidate is present");

        let start = usize::try_from(candidate.span().start()).expect("span fits");
        let end = usize::try_from(candidate.span().end()).expect("span fits");
        assert_eq!(&source[start..end], candidate.raw());
    }

    #[test]
    fn ignores_dynamic_template_attributes() {
        let source = r#"<div className={`p-${size} text-red-500`}></div>"#;

        assert!(extract(source).expect("fixture fits").is_empty());
    }

    #[test]
    fn ignores_class_like_text_inside_comments() {
        let source = r#"
            // clsx("p-4")
            /* <div class="flex"></div> */
            const classes = clsx("p-8");
        "#;

        assert_eq!(raws(source), vec!["p-8"]);
    }

    #[test]
    fn extracts_vue_bound_class_literals() {
        let source = r#"<button :class="{ 'text-red-500': active, 'font-bold': strong }" />"#;

        assert_eq!(
            extract_for_framework(source, Framework::Vue)
                .expect("fixture fits")
                .into_iter()
                .map(|candidate| candidate.raw())
                .collect::<Vec<_>>(),
            vec!["text-red-500", "font-bold"]
        );
    }

    #[test]
    fn extracts_svelte_class_directives() {
        let source = r#"<div class:flex={wide} class:md:hover:bg-blue-500={active}></div>"#;

        assert_eq!(
            extract_for_framework(source, Framework::Svelte)
                .expect("fixture fits")
                .into_iter()
                .map(|candidate| candidate.raw())
                .collect::<Vec<_>>(),
            vec!["flex", "md:hover:bg-blue-500"]
        );
    }

    #[test]
    fn extracts_astro_class_list_literals() {
        let source = r#"<div class:list={['p-4', active && 'text-red-500']}></div>"#;

        assert_eq!(
            extract_for_framework(source, Framework::Astro)
                .expect("fixture fits")
                .into_iter()
                .map(|candidate| candidate.raw())
                .collect::<Vec<_>>(),
            vec!["p-4", "text-red-500"]
        );
    }

    #[test]
    fn ignores_framework_syntax_inside_comments() {
        let source = r#"<!-- :class="'p-4'" --> <div class:list={['flex']}></div>"#;

        assert_eq!(
            extract_for_framework(source, Framework::Vue)
                .expect("fixture fits")
                .into_iter()
                .map(|candidate| candidate.raw())
                .collect::<Vec<_>>(),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn hybrid_extraction_marks_every_origin_as_hybrid() {
        let candidates = extract_with_mode(
            r#"<div class="p-4"> const value = "text-red-500";"#,
            ExtractionMode::Hybrid,
            Framework::Html,
        )
        .expect("hybrid extraction is supported");
        assert!(!candidates.is_empty());
        assert!(candidates.iter().all(|candidate| candidate.mode() == ExtractionMode::Hybrid));
    }

    #[test]
    fn ast_mode_requires_a_host_language_extractor() {
        let error = extract_with_mode("<div class=\"flex\">", ExtractionMode::Ast, Framework::Html)
            .expect_err("the generic extractor cannot provide AST semantics");

        assert_eq!(error, super::ExtractionError::AstRequiresHostExtractor);
    }

    #[test]
    fn ignores_class_attribute_lookalikes_in_script_text() {
        let source = r#"const className = "flex"; const title = "p-4";"#;

        assert!(extract(source).expect("fixture fits").is_empty());
    }

    #[test]
    fn avoids_obvious_shadowed_text_helper_bindings() {
        let source = r#"function cn(value) { return value; } cn("flex");"#;

        assert!(extract(source).expect("fixture fits").is_empty());
    }
}
