//! Parser and borrowed AST for the utilitycss candidate DSL.
//!
//! Parsing is independent of themes and utility registries. Semantic resolution belongs to later
//! compiler phases.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{fmt, str};

use serde::Serialize;
use utilitycss_diagnostics::{Diagnostic, DiagnosticCode};
use utilitycss_span::{SourceId, Span};

/// Version of the native candidate grammar implemented by this crate.
pub const GRAMMAR_VERSION: u16 = 1;

/// A parsed utility candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidateAst<'source> {
    raw: &'source str,
    important: bool,
    variants: Vec<VariantAst<'source>>,
    utility: UtilityAst<'source>,
}

impl<'source> CandidateAst<'source> {
    /// Returns the original candidate text.
    #[must_use]
    pub const fn raw(&self) -> &'source str {
        self.raw
    }

    /// Returns whether the candidate has an important marker.
    #[must_use]
    pub const fn is_important(&self) -> bool {
        self.important
    }

    /// Returns the structured important marker.
    #[must_use]
    pub const fn important(&self) -> ImportantAst {
        if self.important {
            ImportantAst::Important
        } else {
            ImportantAst::None
        }
    }

    /// Returns variants in author order.
    #[must_use]
    pub fn variants(&self) -> &[VariantAst<'source>] {
        &self.variants
    }

    /// Returns the parsed base utility.
    #[must_use]
    pub const fn utility(&self) -> &UtilityAst<'source> {
        &self.utility
    }

    /// Returns the canonical native spelling with important normalized to trailing `!`.
    #[must_use]
    pub fn normalized(&self) -> String {
        let mut normalized = String::new();
        for (index, variant) in self.variants.iter().enumerate() {
            if index > 0 {
                normalized.push(':');
            }
            normalized.push_str(variant.raw);
        }
        if !self.variants.is_empty() {
            normalized.push(':');
        }
        normalized.push_str(self.utility.raw);
        if self.important {
            normalized.push('!');
        }
        normalized
    }

    /// Converts the borrowed AST into an owned representation for protocols and tooling.
    #[must_use]
    pub fn to_owned_ast(&self) -> CandidateAstOwned {
        CandidateAstOwned {
            raw: self.raw.to_owned(),
            important: self.important(),
            variants: self.variants.iter().copied().map(VariantAst::to_owned_ast).collect(),
            utility: self.utility.to_owned_ast(),
        }
    }
}

/// The canonical important marker state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ImportantAst {
    /// No important marker was present.
    None,
    /// An important marker was present.
    Important,
}

impl ImportantAst {
    /// Returns whether this marker state is important.
    #[must_use]
    pub const fn is_important(self) -> bool {
        matches!(self, Self::Important)
    }
}

/// Owned form of [`CandidateAst`] for introspection and transport APIs.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidateAstOwned {
    /// Original candidate text.
    pub raw: String,
    /// Important marker state.
    pub important: ImportantAst,
    /// Variants in author order.
    pub variants: Vec<VariantAstOwned>,
    /// Parsed base utility.
    pub utility: UtilityAstOwned,
}

/// A variant in a candidate's author-ordered variant chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct VariantAst<'source> {
    raw: &'source str,
    kind: VariantKind<'source>,
}

impl<'source> VariantAst<'source> {
    /// Returns the original variant segment.
    #[must_use]
    pub const fn raw(self) -> &'source str {
        self.raw
    }

    /// Returns the structural variant kind.
    #[must_use]
    pub const fn kind(self) -> VariantKind<'source> {
        self.kind
    }

    /// Converts this borrowed variant into an owned representation.
    #[must_use]
    pub fn to_owned_ast(self) -> VariantAstOwned {
        VariantAstOwned { raw: self.raw.to_owned(), kind: self.kind.to_owned() }
    }

    /// Returns whether this is an arbitrary selector variant.
    #[must_use]
    pub const fn is_arbitrary_selector(self) -> bool {
        matches!(self.kind, VariantKind::Arbitrary { .. })
    }

    /// Returns whether this is an arbitrary at-rule variant.
    #[must_use]
    pub const fn is_arbitrary_at_rule(self) -> bool {
        matches!(self.kind, VariantKind::ArbitraryAtRule { .. })
    }
}

/// Owned form of [`VariantAst`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VariantAstOwned {
    /// Original variant segment.
    pub raw: String,
    /// Structural variant kind.
    pub kind: VariantKindOwned,
}

/// Structural forms of a parsed variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum VariantKind<'source> {
    /// A named variant, optionally carrying an arbitrary value such as `data-[state=open]`.
    Named {
        /// The named variant prefix.
        name: &'source str,
        /// The optional variant value.
        value: Option<ValueAst<'source>>,
    },
    /// An arbitrary selector variant such as `[&>*]`.
    Arbitrary {
        /// The selector content without the surrounding brackets.
        selector: &'source str,
    },
    /// An arbitrary at-rule variant such as `[@supports(display:grid)]`.
    ArbitraryAtRule {
        /// At-rule name without `@`.
        name: &'source str,
        /// At-rule prelude after the name.
        prelude: &'source str,
    },
}

impl<'source> VariantKind<'source> {
    fn to_owned(self) -> VariantKindOwned {
        match self {
            Self::Named { name, value } => VariantKindOwned::Named {
                name: name.to_owned(),
                value: value.map(ValueAst::to_owned),
            },
            Self::Arbitrary { selector } => {
                VariantKindOwned::ArbitrarySelector { selector: selector.to_owned() }
            }
            Self::ArbitraryAtRule { name, prelude } => VariantKindOwned::ArbitraryAtRule {
                name: name.to_owned(),
                prelude: prelude.to_owned(),
            },
        }
    }
}

/// Owned structural forms of a variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum VariantKindOwned {
    /// A named variant and optional value.
    Named {
        /// Variant name.
        name: String,
        /// Optional value.
        value: Option<ValueAstOwned>,
    },
    /// An arbitrary selector variant.
    ArbitrarySelector {
        /// Selector content.
        selector: String,
    },
    /// An arbitrary at-rule variant.
    ArbitraryAtRule {
        /// At-rule name.
        name: String,
        /// At-rule prelude.
        prelude: String,
    },
}

/// A parsed base utility and its optional value and modifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct UtilityAst<'source> {
    raw: &'source str,
    negative: bool,
    family: &'source str,
    value: Option<ValueAst<'source>>,
    modifier: Option<ValueAst<'source>>,
    arbitrary_property: Option<ArbitraryPropertyAst<'source>>,
}

impl<'source> UtilityAst<'source> {
    /// Returns the original utility segment without variants or the important marker.
    #[must_use]
    pub const fn raw(self) -> &'source str {
        self.raw
    }

    /// Returns whether the utility has a leading negative marker.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.negative
    }

    /// Returns the parsed utility family.
    #[must_use]
    pub const fn family(self) -> &'source str {
        self.family
    }

    /// Returns the optional utility value.
    #[must_use]
    pub const fn value(self) -> Option<ValueAst<'source>> {
        self.value
    }

    /// Returns the optional utility modifier.
    #[must_use]
    pub const fn modifier(self) -> Option<ValueAst<'source>> {
        self.modifier
    }

    /// Returns whether this utility is a structural arbitrary property.
    #[must_use]
    pub const fn is_arbitrary_property(self) -> bool {
        self.arbitrary_property.is_some()
    }

    /// Returns the parsed arbitrary property, if present.
    #[must_use]
    pub const fn arbitrary_property(self) -> Option<ArbitraryPropertyAst<'source>> {
        self.arbitrary_property
    }

    /// Converts this borrowed utility into an owned representation.
    #[must_use]
    pub fn to_owned_ast(self) -> UtilityAstOwned {
        UtilityAstOwned {
            raw: self.raw.to_owned(),
            negative: self.negative,
            family: self.family.to_owned(),
            value: self.value.map(ValueAst::to_owned),
            modifier: self.modifier.map(ValueAst::to_owned),
            arbitrary_property: self.arbitrary_property.map(ArbitraryPropertyAst::to_owned),
        }
    }
}

/// A parsed arbitrary property utility such as `[content-visibility:auto]`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ArbitraryPropertyAst<'source> {
    /// Complete content without the outer brackets.
    pub raw: &'source str,
    /// CSS property name.
    pub property: &'source str,
    /// CSS declaration value.
    pub value: ValueAst<'source>,
}

impl<'source> ArbitraryPropertyAst<'source> {
    /// Converts the property AST into an owned representation.
    #[must_use]
    pub fn to_owned(self) -> ArbitraryPropertyAstOwned {
        ArbitraryPropertyAstOwned {
            raw: self.raw.to_owned(),
            property: self.property.to_owned(),
            value: self.value.to_owned(),
        }
    }
}

/// Owned form of [`UtilityAst`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UtilityAstOwned {
    /// Original utility segment.
    pub raw: String,
    /// Negative marker state.
    pub negative: bool,
    /// Utility family, or an empty string for arbitrary properties.
    pub family: String,
    /// Optional value.
    pub value: Option<ValueAstOwned>,
    /// Optional modifier.
    pub modifier: Option<ValueAstOwned>,
    /// Optional arbitrary property.
    pub arbitrary_property: Option<ArbitraryPropertyAstOwned>,
}

/// Owned form of [`ArbitraryPropertyAst`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArbitraryPropertyAstOwned {
    /// Complete content without the outer brackets.
    pub raw: String,
    /// CSS property name.
    pub property: String,
    /// CSS declaration value.
    pub value: ValueAstOwned,
}

/// A named or bracketed utility/variant value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ValueAst<'source> {
    /// A value made of the candidate's non-bracketed source text.
    Named(&'source str),
    /// A value with balanced brackets, retaining both raw and inner text.
    Arbitrary {
        /// The value including its surrounding brackets.
        raw: &'source str,
        /// The value excluding its surrounding brackets.
        content: &'source str,
    },
    /// A typed arbitrary value such as `[color:var(--brand)]`.
    TypedArbitrary {
        /// The value including its surrounding brackets.
        raw: &'source str,
        /// Type hint such as `color` or `length`.
        hint: &'source str,
        /// The hinted CSS content without the type prefix.
        content: &'source str,
    },
    /// A numeric fraction such as `1/2`.
    Fraction {
        /// Fraction numerator.
        numerator: &'source str,
        /// Fraction denominator.
        denominator: &'source str,
    },
}

impl<'source> ValueAst<'source> {
    /// Returns the value exactly as represented in the candidate.
    #[must_use]
    pub const fn raw(self) -> &'source str {
        match self {
            Self::Named(value) => value,
            Self::Arbitrary { raw, .. } => raw,
            Self::TypedArbitrary { raw, .. } => raw,
            Self::Fraction { numerator, .. } => numerator,
        }
    }

    /// Returns the value content without arbitrary-value brackets.
    #[must_use]
    pub const fn content(self) -> &'source str {
        match self {
            Self::Named(value) => value,
            Self::Arbitrary { content, .. } => content,
            Self::TypedArbitrary { content, .. } => content,
            Self::Fraction { numerator, .. } => numerator,
        }
    }

    /// Returns whether this is an arbitrary bracketed value.
    #[must_use]
    pub const fn is_arbitrary(self) -> bool {
        matches!(self, Self::Arbitrary { .. } | Self::TypedArbitrary { .. })
    }

    /// Returns the type hint for a typed arbitrary value.
    #[must_use]
    pub const fn type_hint(self) -> Option<&'source str> {
        match self {
            Self::TypedArbitrary { hint, .. } => Some(hint),
            _ => None,
        }
    }

    /// Returns the fraction components when this is a fraction.
    #[must_use]
    pub const fn fraction(self) -> Option<(&'source str, &'source str)> {
        match self {
            Self::Fraction { numerator, denominator } => Some((numerator, denominator)),
            _ => None,
        }
    }
}

impl<'source> ValueAst<'source> {
    fn to_owned(self) -> ValueAstOwned {
        match self {
            Self::Named(value) => ValueAstOwned::Named(value.to_owned()),
            Self::Arbitrary { raw, content } => {
                ValueAstOwned::Arbitrary { raw: raw.to_owned(), content: content.to_owned() }
            }
            Self::TypedArbitrary { raw, hint, content } => ValueAstOwned::TypedArbitrary {
                raw: raw.to_owned(),
                hint: hint.to_owned(),
                content: content.to_owned(),
            },
            Self::Fraction { numerator, denominator } => ValueAstOwned::Fraction {
                numerator: numerator.to_owned(),
                denominator: denominator.to_owned(),
            },
        }
    }
}

/// Owned form of [`ValueAst`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ValueAstOwned {
    /// A named value.
    Named(String),
    /// An untyped arbitrary value.
    Arbitrary {
        /// Raw bracketed value.
        raw: String,
        /// Inner content.
        content: String,
    },
    /// A typed arbitrary value.
    TypedArbitrary {
        /// Raw bracketed value.
        raw: String,
        /// Type hint.
        hint: String,
        /// Inner content.
        content: String,
    },
    /// A numeric fraction.
    Fraction {
        /// Numerator.
        numerator: String,
        /// Denominator.
        denominator: String,
    },
}

/// The category of a parser failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseErrorKind {
    /// The candidate was empty.
    EmptyCandidate,
    /// A variant segment was empty.
    EmptyVariant,
    /// The utility segment was empty.
    EmptyUtility,
    /// A utility or named variant had no name.
    MissingName,
    /// A value was required but absent.
    MissingValue,
    /// An arbitrary value did not have a closing bracket.
    UnterminatedArbitraryValue,
    /// An arbitrary value had no content.
    EmptyArbitraryValue,
    /// Characters followed a complete arbitrary value where none were allowed.
    UnexpectedTrailingCharacters,
    /// An arbitrary property did not contain a top-level property/value separator.
    InvalidArbitraryProperty,
    /// The input bytes were not valid UTF-8.
    InvalidUtf8,
}

impl ParseErrorKind {
    const fn code(self) -> DiagnosticCode {
        let code = match self {
            Self::EmptyCandidate => "syntax.empty-candidate",
            Self::EmptyVariant => "syntax.empty-variant",
            Self::EmptyUtility => "syntax.empty-utility",
            Self::MissingName => "syntax.missing-name",
            Self::MissingValue => "syntax.missing-value",
            Self::UnterminatedArbitraryValue => "syntax.unterminated-arbitrary-value",
            Self::EmptyArbitraryValue => "syntax.empty-arbitrary-value",
            Self::UnexpectedTrailingCharacters => "syntax.unexpected-trailing-characters",
            Self::InvalidArbitraryProperty => "syntax.invalid-arbitrary-property",
            Self::InvalidUtf8 => "syntax.invalid-utf8",
        };
        DiagnosticCode::new(code)
    }
}

/// A non-panicking parser error with a byte offset in the candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    kind: ParseErrorKind,
    offset: usize,
}

impl ParseError {
    const fn new(kind: ParseErrorKind, offset: usize) -> Self {
        Self { kind, offset }
    }

    /// Returns the parser error category.
    #[must_use]
    pub const fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    /// Returns the byte offset at which parsing could no longer continue.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Converts the error into a source-aware diagnostic.
    #[must_use]
    pub fn to_diagnostic(&self, source: SourceId, span: Span) -> Diagnostic {
        Diagnostic::error(self.kind.code(), self.to_string())
            .with_source(source)
            .with_span(span)
            .with_explanation("The candidate does not match the versioned native class grammar.")
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ParseErrorKind::EmptyCandidate => "candidate is empty",
            ParseErrorKind::EmptyVariant => "variant segment is empty",
            ParseErrorKind::EmptyUtility => "utility segment is empty",
            ParseErrorKind::MissingName => "utility or variant name is missing",
            ParseErrorKind::MissingValue => "utility value is missing",
            ParseErrorKind::UnterminatedArbitraryValue => "arbitrary value is not closed",
            ParseErrorKind::EmptyArbitraryValue => "arbitrary value is empty",
            ParseErrorKind::UnexpectedTrailingCharacters => {
                "unexpected characters follow an arbitrary value"
            }
            ParseErrorKind::InvalidArbitraryProperty => {
                "arbitrary property must contain a property and value separated by `:`"
            }
            ParseErrorKind::InvalidUtf8 => "candidate bytes are not valid UTF-8",
        };
        write!(formatter, "{message} at byte {}", self.offset)
    }
}

impl std::error::Error for ParseError {}

/// Parses one candidate into a borrowed AST.
pub fn parse(candidate: &str) -> Result<CandidateAst<'_>, ParseError> {
    if candidate.is_empty() {
        return Err(ParseError::new(ParseErrorKind::EmptyCandidate, 0));
    }

    let (leading_important, body) =
        candidate.strip_prefix('!').map_or((false, candidate), |body| (true, body));
    let (trailing_important, body) =
        body.strip_suffix('!').map_or((false, body), |body| (true, body));
    let important = leading_important || trailing_important;
    if body.is_empty() {
        return Err(ParseError::new(ParseErrorKind::EmptyUtility, candidate.len()));
    }

    let segments = split_top_level(body, ':');
    if segments.iter().any(|segment| segment.is_empty()) {
        let offset = segments.iter().position(|segment| segment.is_empty()).unwrap_or_default();
        return Err(ParseError::new(ParseErrorKind::EmptyVariant, offset));
    }

    let utility_segment = segments.last().copied().unwrap_or_default();
    if utility_segment.is_empty() {
        return Err(ParseError::new(ParseErrorKind::EmptyUtility, body.len()));
    }

    let variants = segments[..segments.len().saturating_sub(1)]
        .iter()
        .map(|segment| parse_variant(segment))
        .collect::<Result<Vec<_>, _>>()?;
    let utility = parse_utility(utility_segment)?;

    Ok(CandidateAst { raw: candidate, important, variants, utility })
}

/// Parses bytes safely, returning a typed error for invalid UTF-8.
pub fn parse_bytes(candidate: &[u8]) -> Result<CandidateAst<'_>, ParseError> {
    let text = str::from_utf8(candidate)
        .map_err(|error| ParseError::new(ParseErrorKind::InvalidUtf8, error.valid_up_to()))?;
    parse(text)
}

fn parse_variant(segment: &str) -> Result<VariantAst<'_>, ParseError> {
    if segment.starts_with('[') {
        let closing = matching_bracket(segment, 0)?;
        if closing + 1 != segment.len() {
            return Err(ParseError::new(ParseErrorKind::UnexpectedTrailingCharacters, closing + 1));
        }
        if closing == 1 {
            return Err(ParseError::new(ParseErrorKind::EmptyArbitraryValue, 1));
        }
        let content = &segment[1..closing];
        if content.starts_with('@') {
            let (name, prelude) = split_at_rule(content)
                .ok_or_else(|| ParseError::new(ParseErrorKind::MissingName, 1))?;
            return Ok(VariantAst {
                raw: segment,
                kind: VariantKind::ArbitraryAtRule { name, prelude },
            });
        }
        return Ok(VariantAst { raw: segment, kind: VariantKind::Arbitrary { selector: content } });
    }

    if let Some(value_start) = find_top_level_sequence(segment, "-[") {
        let name = &segment[..value_start];
        if name.is_empty() {
            return Err(ParseError::new(ParseErrorKind::MissingName, 0));
        }
        let value = parse_value_exact(segment, value_start + 1)?;
        return Ok(VariantAst {
            raw: segment,
            kind: VariantKind::Named { name, value: Some(value) },
        });
    }

    if segment.is_empty() {
        return Err(ParseError::new(ParseErrorKind::MissingName, 0));
    }
    Ok(VariantAst { raw: segment, kind: VariantKind::Named { name: segment, value: None } })
}

fn parse_utility(segment: &str) -> Result<UtilityAst<'_>, ParseError> {
    if segment.starts_with('[') {
        return parse_arbitrary_property(segment);
    }
    let (base, modifier_text) = split_modifier(segment);
    if base.is_empty() {
        return Err(ParseError::new(ParseErrorKind::EmptyUtility, 0));
    }

    let (negative, unsigned) =
        base.strip_prefix('-').map_or((false, base), |unsigned| (true, unsigned));
    if unsigned.is_empty() {
        return Err(ParseError::new(ParseErrorKind::MissingName, 0));
    }

    let (family, mut value) = split_utility(unsigned)?;
    let mut modifier = modifier_text.map(|text| parse_value_exact(text, 0)).transpose()?;

    if let (Some(ValueAst::Named(numerator)), Some(ValueAst::Named(denominator))) =
        (value, modifier)
    {
        if is_fraction_utility_family(family)
            && is_fraction_part(numerator)
            && is_fraction_part(denominator)
        {
            value = Some(ValueAst::Fraction { numerator, denominator });
            modifier = None;
        }
    }

    Ok(UtilityAst { raw: segment, negative, family, value, modifier, arbitrary_property: None })
}

fn parse_arbitrary_property(segment: &str) -> Result<UtilityAst<'_>, ParseError> {
    let closing = matching_bracket(segment, 0)?;
    if closing + 1 != segment.len() {
        return Err(ParseError::new(ParseErrorKind::UnexpectedTrailingCharacters, closing + 1));
    }
    if closing == 1 {
        return Err(ParseError::new(ParseErrorKind::EmptyArbitraryValue, 1));
    }
    let content = &segment[1..closing];
    let (property, value_start) = find_top_level_delimiter(content, ':')
        .ok_or_else(|| ParseError::new(ParseErrorKind::InvalidArbitraryProperty, 1))?;
    if property.is_empty() {
        return Err(ParseError::new(ParseErrorKind::MissingName, 1));
    }
    let value_start = value_start + 1;
    if value_start >= content.len() {
        return Err(ParseError::new(ParseErrorKind::MissingValue, closing));
    }
    let value_content = &content[value_start..];
    if value_content.is_empty() {
        return Err(ParseError::new(ParseErrorKind::MissingValue, closing));
    }
    let value = ValueAst::Named(value_content);
    Ok(UtilityAst {
        raw: segment,
        negative: false,
        family: "",
        value: None,
        modifier: None,
        arbitrary_property: Some(ArbitraryPropertyAst { raw: content, property, value }),
    })
}

fn split_at_rule(content: &str) -> Option<(&str, &str)> {
    let content = content.strip_prefix('@')?;
    let name_end = content
        .char_indices()
        .find(|(_, character)| character.is_whitespace() || *character == '(')
        .map_or(content.len(), |(index, _)| index);
    let name = content.get(..name_end)?;
    if name.is_empty() {
        return None;
    }
    let prelude = content.get(name_end..)?.trim_start();
    if prelude.is_empty() {
        return None;
    }
    Some((name, prelude))
}

fn is_fraction_part(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_fraction_utility_family(family: &str) -> bool {
    matches!(family, "w" | "h" | "min-w" | "max-w" | "min-h" | "max-h" | "size" | "basis")
}

fn split_utility(segment: &str) -> Result<(&str, Option<ValueAst<'_>>), ParseError> {
    if let Some(value_start) = find_top_level_sequence(segment, "-[") {
        let family = &segment[..value_start];
        if family.is_empty() {
            return Err(ParseError::new(ParseErrorKind::MissingName, 0));
        }
        let value = parse_value_exact(segment, value_start + 1)?;
        return Ok((family, Some(value)));
    }

    for family in COMPOUND_UTILITY_FAMILIES {
        if let Some(value_start) =
            segment.strip_prefix(family).and_then(|rest| rest.strip_prefix('-'))
        {
            let start = segment.len() - value_start.len();
            let value = parse_value_exact(segment, start)?;
            return Ok((family, Some(value)));
        }
    }

    if let Some(value_start) = find_top_level_char(segment, '-') {
        let family = &segment[..value_start];
        if family.is_empty() {
            return Err(ParseError::new(ParseErrorKind::MissingName, 0));
        }
        let value = parse_value_exact(segment, value_start + 1)?;
        return Ok((family, Some(value)));
    }

    Ok((segment, None))
}

const COMPOUND_UTILITY_FAMILIES: &[&str] = &[
    "aspect-ratio",
    "break-after",
    "break-before",
    "break-inside",
    "box-decoration",
    "box-sizing",
    "line-clamp",
    "object-position",
    "overflow-x",
    "overflow-y",
    "overscroll-x",
    "overscroll-y",
    "grid-flow",
    "auto-cols",
    "auto-rows",
    "grid-cols",
    "grid-rows",
    "grid-column",
    "grid-row",
    "justify-items",
    "justify-self",
    "align-content",
    "align-items",
    "align-self",
    "place-content",
    "place-items",
    "place-self",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "font-stretch",
    "font-variant",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-decoration",
    "text-underline",
    "text-overflow",
    "text-indent",
    "text-wrap",
    "word-break",
    "background-attachment",
    "background-clip",
    "background-origin",
    "background-position",
    "background-repeat",
    "background-size",
    "border-radius",
    "border-width",
    "border-style",
    "border-spacing",
    "divide-x",
    "divide-y",
    "outline-width",
    "outline-offset",
    "mix-blend",
    "background-blend",
    "backdrop-blur",
    "backdrop-brightness",
    "backdrop-contrast",
    "backdrop-grayscale",
    "backdrop-hue-rotate",
    "backdrop-invert",
    "backdrop-opacity",
    "backdrop-saturate",
    "backdrop-sepia",
    "transform-origin",
    "transform-style",
    "transition-property",
    "transition-duration",
    "transition-timing",
    "transition-delay",
    "scroll-margin",
    "scroll-padding",
    "scroll-snap",
    "touch-action",
    "user-select",
    "pointer-events",
    "will-change",
    "contain-intrinsic",
    "mask-composite",
    "mask-mode",
    "mask-origin",
    "mask-position",
    "mask-repeat",
    "mask-size",
    "mask-type",
    "underline-offset",
    "col-span",
    "row-span",
    "col-start",
    "col-end",
    "row-start",
    "row-end",
    "border-x",
    "border-y",
    "border-s",
    "border-e",
    "border-t",
    "border-r",
    "border-b",
    "border-l",
    "rounded-t",
    "rounded-r",
    "rounded-b",
    "rounded-l",
    "space-x",
    "space-y",
    "divide-x",
    "divide-y",
    "inset-x",
    "inset-y",
    "translate-x",
    "translate-y",
    "scale-x",
    "scale-y",
    "max-w",
    "min-w",
    "max-h",
    "min-h",
];

fn split_modifier(segment: &str) -> (&str, Option<&str>) {
    find_top_level_char(segment, '/')
        .map_or((segment, None), |index| (&segment[..index], Some(&segment[index + 1..])))
}

fn parse_value_exact(input: &str, start: usize) -> Result<ValueAst<'_>, ParseError> {
    if start >= input.len() {
        return Err(ParseError::new(ParseErrorKind::MissingValue, start));
    }

    let first = input[start..].chars().next().expect("start is a valid non-end byte offset");
    if first == '[' {
        let closing = matching_bracket(input, start)?;
        if closing == start + 1 {
            return Err(ParseError::new(ParseErrorKind::EmptyArbitraryValue, start + 1));
        }
        if closing + 1 != input.len() {
            return Err(ParseError::new(ParseErrorKind::UnexpectedTrailingCharacters, closing + 1));
        }
        let content = &input[start + 1..closing];
        if let Some((hint, separator)) = find_top_level_delimiter(content, ':') {
            if is_type_hint(hint) && separator + 1 < content.len() {
                return Ok(ValueAst::TypedArbitrary {
                    raw: &input[start..=closing],
                    hint,
                    content: &content[separator + 1..],
                });
            }
        }
        return Ok(ValueAst::Arbitrary { raw: &input[start..=closing], content });
    }

    if input[start..].contains('[') || input[start..].contains(']') {
        return Err(ParseError::new(ParseErrorKind::UnterminatedArbitraryValue, start));
    }
    Ok(ValueAst::Named(&input[start..]))
}

fn matching_bracket(input: &str, open: usize) -> Result<usize, ParseError> {
    let mut depth = 0_u32;
    let mut parentheses = 0_u32;
    let mut escaped = false;
    let mut quote = None;

    for (relative, character) in input[open..].char_indices() {
        let index = open + relative;
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => depth = depth.saturating_add(1),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            ']' if parentheses == 0 => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }

    Err(ParseError::new(ParseErrorKind::UnterminatedArbitraryValue, open))
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut bracket_depth = 0_u32;
    let mut parentheses = 0_u32;
    let mut escaped = false;
    let mut quote = None;

    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            character if character == delimiter && bracket_depth == 0 && parentheses == 0 => {
                pieces.push(&input[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    pieces.push(&input[start..]);
    pieces
}

fn find_top_level_char(input: &str, target: char) -> Option<usize> {
    find_top_level(input, |character| character == target)
}

fn find_top_level_sequence(input: &str, target: &str) -> Option<usize> {
    let target_start = target.chars().next()?;
    let mut bracket_depth = 0_u32;
    let mut parentheses = 0_u32;
    let mut escaped = false;
    let mut quote = None;

    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            character
                if character == target_start
                    && bracket_depth == 0
                    && parentheses == 0
                    && input[index..].starts_with(target) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn find_top_level<F>(input: &str, mut predicate: F) -> Option<usize>
where
    F: FnMut(char) -> bool,
{
    let mut bracket_depth = 0_u32;
    let mut parentheses = 0_u32;
    let mut escaped = false;
    let mut quote = None;
    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' => bracket_depth = bracket_depth.saturating_add(1),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            character if bracket_depth == 0 && parentheses == 0 && predicate(character) => {
                return Some(index)
            }
            _ => {}
        }
    }
    None
}

fn find_top_level_delimiter(input: &str, delimiter: char) -> Option<(&str, usize)> {
    let index = find_top_level_char(input, delimiter)?;
    Some((&input[..index], index))
}

fn is_type_hint(value: &str) -> bool {
    matches!(
        value,
        "angle"
            | "color"
            | "image"
            | "length"
            | "number"
            | "percentage"
            | "position"
            | "ratio"
            | "url"
    )
}

/// An error returned by [`decode_arbitrary`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeErrorKind {
    /// The arbitrary value ended while a quote was open.
    UnterminatedQuote,
}

/// A non-panicking arbitrary-value decoding error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeError {
    kind: DecodeErrorKind,
    offset: usize,
}

impl DecodeError {
    /// Returns the error category.
    #[must_use]
    pub const fn kind(self) -> DecodeErrorKind {
        self.kind
    }

    /// Returns the byte offset at which decoding stopped.
    #[must_use]
    pub const fn offset(self) -> usize {
        self.offset
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "arbitrary value has an unterminated quote at byte {}", self.offset)
    }
}

impl std::error::Error for DecodeError {}

/// Decodes the shared underscore and escape rules for arbitrary CSS content.
///
/// Unescaped underscores become spaces outside quoted strings and URL functions. Escaped
/// underscores remain literal underscores. The function does not evaluate CSS or permit unsafe
/// fragments; semantic consumers remain responsible for safety validation.
pub fn decode_arbitrary(input: &str) -> Result<String, DecodeError> {
    let mut output = String::with_capacity(input.len());
    let mut quote = None;
    let mut escaped = false;
    let mut url_parentheses = 0_u32;
    let mut parentheses = 0_u32;

    for character in input.chars() {
        if escaped {
            output.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            output.push(character);
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => {
                quote = Some(character);
                output.push(character);
            }
            '_' if url_parentheses == 0 => output.push(' '),
            '(' => {
                let is_url = output
                    .trim_end_matches(|character: char| character.is_ascii_whitespace())
                    .rsplit(|character: char| !character.is_ascii_alphabetic())
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("url"));
                parentheses = parentheses.saturating_add(1);
                if is_url {
                    url_parentheses = url_parentheses.saturating_add(1);
                }
                output.push(character);
            }
            ')' => {
                parentheses = parentheses.saturating_sub(1);
                if url_parentheses > 0 && parentheses < url_parentheses {
                    url_parentheses = url_parentheses.saturating_sub(1);
                }
                output.push(character);
            }
            _ => output.push(character),
        }
    }

    if escaped {
        output.push('\\');
    }
    if quote.is_some() {
        return Err(DecodeError { kind: DecodeErrorKind::UnterminatedQuote, offset: input.len() });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use utilitycss_span::{SourceId, Span};

    use super::{decode_arbitrary, parse, parse_bytes, ParseErrorKind, ValueAst, VariantKind};

    #[test]
    fn parses_variants_values_modifiers_and_important() {
        let candidate = parse("!md:hover:bg-red-500/50").expect("candidate is valid");

        assert!(candidate.is_important());
        assert_eq!(candidate.variants().len(), 2);
        assert_eq!(candidate.variants()[0].raw(), "md");
        assert_eq!(candidate.variants()[1].raw(), "hover");
        assert_eq!(candidate.utility().family(), "bg");
        assert_eq!(candidate.utility().value().map(ValueAst::content), Some("red-500"));
        assert_eq!(candidate.utility().modifier().map(ValueAst::content), Some("50"));
    }

    #[test]
    fn parses_negative_and_arbitrary_values_structurally() {
        let candidate = parse("-grid-cols-[1fr_2fr]").expect("candidate is valid");
        let value = candidate.utility().value().expect("value is present");

        assert!(candidate.utility().is_negative());
        assert_eq!(candidate.utility().family(), "grid-cols");
        assert!(value.is_arbitrary());
        assert_eq!(value.raw(), "[1fr_2fr]");
        assert_eq!(value.content(), "1fr_2fr");
    }

    #[test]
    fn parses_arbitrary_variants_and_variant_values() {
        let arbitrary = parse("[&>*]:p-4").expect("candidate is valid");
        assert_eq!(arbitrary.variants()[0].kind(), VariantKind::Arbitrary { selector: "&>*" });

        let data = parse("data-[state=open]:opacity-100").expect("candidate is valid");
        assert_eq!(
            data.variants()[0].kind(),
            VariantKind::Named {
                name: "data",
                value: Some(ValueAst::Arbitrary { raw: "[state=open]", content: "state=open" })
            }
        );
    }

    #[test]
    fn parses_vnext_structural_forms() {
        let trailing_important = parse("hover:bg-red-500/50!").expect("candidate is valid");
        assert!(trailing_important.is_important());
        assert_eq!(trailing_important.utility().modifier().map(ValueAst::content), Some("50"));

        let fraction = parse("w-1/2").expect("fraction is valid");
        assert_eq!(fraction.utility().value().and_then(ValueAst::fraction), Some(("1", "2")));

        let typed = parse("bg-[color:light-dark(#000,#fff)]").expect("typed value is valid");
        assert_eq!(typed.utility().value().and_then(ValueAst::type_hint), Some("color"));

        let property = parse("[content-visibility:auto]").expect("property is valid");
        let property = property.utility().arbitrary_property().expect("property is present");
        assert_eq!(property.property, "content-visibility");
        assert_eq!(property.value.content(), "auto");

        let at_rule = parse("[@supports(display:grid)]:grid").expect("at-rule is valid");
        assert!(at_rule.variants()[0].is_arbitrary_at_rule());
        assert_eq!(parse("!hover:p-4").expect("candidate is valid").normalized(), "hover:p-4!");
    }

    #[test]
    fn arbitrary_decoder_preserves_urls_and_escaped_underscores() {
        assert_eq!(
            decode_arbitrary("calc(100%_-_2rem)").expect("decode succeeds"),
            "calc(100% - 2rem)"
        );
        assert_eq!(
            decode_arbitrary("url('/what_a_rush.png')").expect("decode succeeds"),
            "url('/what_a_rush.png')"
        );
        assert_eq!(decode_arbitrary("'hello\\_world'").expect("decode succeeds"), "'hello_world'");
        assert!(decode_arbitrary("'unterminated").is_err());
    }

    #[test]
    fn escaped_colons_do_not_create_variants() {
        let candidate = parse(r"hover\:bg-red-500").expect("candidate is syntactically valid");

        assert!(candidate.variants().is_empty());
        assert_eq!(candidate.utility().family(), r"hover\:bg");
    }

    #[test]
    fn malformed_arbitrary_values_return_diagnostics_without_panicking() {
        let error = parse("w-[calc(100%-2rem)").expect_err("bracket is unclosed");

        assert_eq!(error.kind(), ParseErrorKind::UnterminatedArbitraryValue);
        let diagnostic = error.to_diagnostic(
            SourceId::new("src/app.html"),
            Span::new(4, 23).expect("valid candidate span"),
        );
        assert_eq!(diagnostic.code().as_str(), "syntax.unterminated-arbitrary-value");
        assert_eq!(diagnostic.source().map(SourceId::as_str), Some("src/app.html"));
    }

    #[test]
    fn arbitrary_bytes_are_rejected_as_typed_errors() {
        let error = parse_bytes(b"p-4\xff").expect_err("invalid UTF-8 must be rejected");

        assert_eq!(error.kind(), ParseErrorKind::InvalidUtf8);
        assert_eq!(error.offset(), 3);
    }

    #[test]
    fn parsing_is_deterministic() {
        let first = parse("md:hover:p-4").expect("candidate is valid");
        let second = parse("md:hover:p-4").expect("candidate is valid");

        assert_eq!(first, second);
    }

    #[test]
    fn versioned_fixture_matrix_covers_at_least_250_candidates() {
        let mut fixtures = Vec::new();
        for index in 0..100 {
            fixtures.push(format!("p-{index}"));
            fixtures.push(format!("hover:p-{index}"));
        }
        for index in 0..25 {
            fixtures.push(format!("w-[calc(100%_-_{index}rem)]"));
            fixtures.push(format!("bg-red-500/{index}"));
        }
        for index in 0..10 {
            fixtures.push(format!("[@supports(display:grid)]:p-{index}"));
        }
        assert!(fixtures.len() >= 250);
        for fixture in fixtures {
            parse(&fixture).unwrap_or_else(|error| panic!("fixture `{fixture}` failed: {error}"));
        }
    }

    proptest! {
        #[test]
        fn arbitrary_utf8_candidates_never_panic(candidate in any::<String>()) {
            let _ = parse(&candidate);
        }

        #[test]
        fn arbitrary_bytes_never_panic(candidate in prop::collection::vec(any::<u8>(), 0..256)) {
            let _ = parse_bytes(&candidate);
        }
    }
}
