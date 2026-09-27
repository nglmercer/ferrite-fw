//! A language-agnostic text scanner for possible utility class candidates.
//!
//! The scanner deliberately over-collects. It does not know themes, utilities, or host-language
//! syntax; those concerns belong to parsing and semantic resolution.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use utilitycss_span::{validate_source_len, SourceSizeError, Span};

/// Source-discovery strategy used to produce candidate tokens.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExtractionMode {
    /// Language-agnostic text scanning.
    #[default]
    Text,
    /// Static template and literal extraction.
    Static,
    /// Host-language AST extraction.
    Ast,
    /// Deterministic union of static and text extraction.
    Hybrid,
}

impl ExtractionMode {
    /// Returns the stable wire-format name for this extraction mode.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Static => "static",
            Self::Ast => "ast",
            Self::Hybrid => "hybrid",
        }
    }

    /// Parses a stable extraction-mode name supplied by a host adapter.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::Text),
            "static" => Some(Self::Static),
            "ast" => Some(Self::Ast),
            "hybrid" => Some(Self::Hybrid),
            _ => None,
        }
    }
}

/// A borrowed candidate token and its byte span in the scanned source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateToken<'source> {
    raw: &'source str,
    span: Span,
}

impl<'source> CandidateToken<'source> {
    /// Creates a token from a source slice and an already validated span.
    #[must_use]
    pub const fn new(raw: &'source str, span: Span) -> Self {
        Self { raw, span }
    }

    /// Returns the candidate text exactly as it appeared in the source.
    #[must_use]
    pub const fn raw(self) -> &'source str {
        self.raw
    }

    /// Returns the candidate's byte span in the scanned source.
    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }
}

/// Scans source text for possible utility candidates.
///
/// Candidate discovery is linear in the number of source bytes. Bracketed arbitrary values may
/// contain whitespace and nested brackets; an unterminated bracket consumes the remainder of the
/// source and is left for the parser to diagnose. For a checked failure on an oversized source,
/// use [`scan_checked`]; this convenience function returns no candidates for that input.
#[must_use]
pub fn scan(source: &str) -> Vec<CandidateToken<'_>> {
    scan_checked(source).unwrap_or_default()
}

/// Scans source text and rejects input whose source locations cannot fit in [`Span`].
pub fn scan_checked(source: &str) -> Result<Vec<CandidateToken<'_>>, SourceSizeError> {
    validate_source_len(source.len())?;
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let character = source[cursor..]
            .chars()
            .next()
            .expect("cursor always points at a valid UTF-8 boundary");
        if is_candidate_start(character) {
            let end = scan_candidate_end(source, cursor);
            let raw = &source[cursor..end];
            if raw.chars().any(char::is_alphanumeric) {
                let span = Span::new(
                    u32::try_from(cursor).expect("validated source length fits in a span"),
                    u32::try_from(end).expect("validated source length fits in a span"),
                )
                .expect("scanner end is never before scanner start");
                tokens.push(CandidateToken::new(raw, span));
            }
            cursor = end;
        } else {
            cursor += character.len_utf8();
        }
    }

    Ok(tokens)
}

fn scan_candidate_end(source: &str, start: usize) -> usize {
    let mut cursor = start;
    let mut bracket_depth = 0_u32;
    let mut escaped = false;
    let mut quote = None;

    while cursor < source.len() {
        let character = source[cursor..]
            .chars()
            .next()
            .expect("cursor always points at a valid UTF-8 boundary");

        if escaped {
            escaped = false;
            cursor += character.len_utf8();
            continue;
        }

        if character == '\\' {
            escaped = true;
            cursor += character.len_utf8();
            continue;
        }

        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            cursor += character.len_utf8();
            continue;
        }

        if bracket_depth > 0 {
            match character {
                '[' => bracket_depth = bracket_depth.saturating_add(1),
                ']' => bracket_depth = bracket_depth.saturating_sub(1),
                '\'' | '"' => quote = Some(character),
                _ => {}
            }
            cursor += character.len_utf8();
            continue;
        }

        match character {
            '[' => {
                bracket_depth = 1;
                cursor += character.len_utf8();
            }
            character if is_candidate_continue(character) => cursor += character.len_utf8(),
            _ => break,
        }
    }

    cursor
}

fn is_candidate_start(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '!' | '-' | '_' | '[')
}

fn is_candidate_continue(character: char) -> bool {
    character.is_alphanumeric()
        || matches!(
            character,
            '!' | '_' | '-' | ':' | '/' | '.' | '%' | '#' | '=' | '~' | '+' | '*' | '(' | ')' | '@'
        )
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{scan, ExtractionMode};

    #[test]
    fn extraction_modes_have_stable_wire_names() {
        assert_eq!(ExtractionMode::Text.as_str(), "text");
        assert_eq!(ExtractionMode::parse("hybrid"), Some(ExtractionMode::Hybrid));
        assert_eq!(ExtractionMode::parse("unknown"), None);
    }

    fn contains(source: &str, expected: &str) -> bool {
        scan(source).iter().any(|token| token.raw() == expected)
    }

    #[test]
    fn finds_candidates_in_html_jsx_and_template_like_source() {
        let source = r#"
            <div class="flex gap-4 p-4">{items.map(item => <span className={`text-${item}`} />)}</div>
            <Button className="md:grid hover:bg-red-500/50" />
        "#;

        assert!(contains(source, "flex"));
        assert!(contains(source, "gap-4"));
        assert!(contains(source, "p-4"));
        assert!(contains(source, "md:grid"));
        assert!(contains(source, "hover:bg-red-500/50"));
    }

    #[test]
    fn preserves_unicode_and_arbitrary_value_whitespace() {
        let source = "class=\"text-élan w-[calc(100% - 2rem)]\"";
        let tokens = scan(source);

        assert!(contains(source, "text-élan"));
        assert!(contains(source, "w-[calc(100% - 2rem)]"));
        assert_eq!(
            tokens.iter().find(|token| token.raw() == "text-élan").map(|token| token.span().len()),
            Some(10)
        );
    }

    #[test]
    fn escaped_delimiters_stay_inside_one_candidate() {
        assert!(contains(r#"class="hover\:bg-red-500""#, r"hover\:bg-red-500"));
    }

    #[test]
    fn malformed_brackets_are_collected_without_panicking() {
        let tokens = scan("class=\"w-[calc(100% - 2rem) p-4\"");

        assert!(tokens.iter().any(|token| token.raw().starts_with("w-[")));
    }

    #[test]
    fn reaches_arbitrary_properties_selectors_and_at_rules() {
        let source =
            "[mask-type:luminance] [&>*]:p-4 [@supports(display:grid)]:grid hover:bg-red-500/50!";
        for candidate in [
            "[mask-type:luminance]",
            "[&>*]:p-4",
            "[@supports(display:grid)]:grid",
            "hover:bg-red-500/50!",
        ] {
            assert!(contains(source, candidate), "scanner missed `{candidate}`");
        }
    }

    #[test]
    fn long_input_is_scanned_linearly() {
        let source = format!("class=\"{}\"", "p-4 ".repeat(10_000));
        let tokens = scan(&source);

        assert_eq!(tokens.iter().filter(|token| token.raw() == "p-4").count(), 10_000);
    }

    proptest! {
        #[test]
        fn arbitrary_utf8_input_never_panics(source in any::<String>()) {
            let _ = scan(&source);
        }
    }
}
