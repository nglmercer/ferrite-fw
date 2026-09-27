//! Variant registry and transformations for selectors and CSS wrappers.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{cmp::Ordering, collections::BTreeMap, error::Error, fmt};

use serde::Serialize;
use utilitycss_css_ir::CssRule;
use utilitycss_diagnostics::{Diagnostic, DiagnosticCode};
use utilitycss_span::{SourceId, Span};
use utilitycss_syntax::{decode_arbitrary, CandidateAst, ValueAst, VariantAst, VariantKind};
use utilitycss_theme::Theme;

/// Stable semantic identifier for a named variant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct VariantId(String);

impl VariantId {
    /// Creates an identifier from a canonical variant name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// High-level variant category exposed to tooling.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum VariantCategory {
    /// Pseudo-class or pseudo-element selector transformation.
    Pseudo,
    /// Ancestor or arbitrary selector transformation.
    Selector,
    /// Media, supports, container, or other at-rule wrapper.
    AtRule,
}

/// Machine-readable metadata for a named variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VariantDescriptor {
    /// Stable semantic identifier.
    pub id: VariantId,
    /// Accepted canonical names.
    pub names: Vec<String>,
    /// Human-readable description.
    pub description: String,
    /// Whether the variant can be composed with other variants.
    pub composable: bool,
    /// Stable ordering rank.
    pub ordering: u32,
    /// Allowed nesting guidance.
    pub allowed_nesting: Vec<String>,
    /// Compatibility profile annotation.
    pub compatibility_profile: String,
    /// Deterministic examples.
    pub examples: Vec<String>,
    /// Variant category.
    pub category: VariantCategory,
}

/// A registered variant behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantDefinition {
    /// Appends a pseudo-class or pseudo-element to each selector.
    Pseudo {
        /// Pseudo selector suffix, including its leading colon.
        suffix: String,
        /// Stable variant ordering rank.
        order: u16,
    },
    /// Wraps rules in an at-rule.
    Media {
        /// At-rule name such as `media`.
        name: String,
        /// At-rule prelude.
        prelude: String,
        /// Stable variant ordering rank.
        order: u16,
    },
    /// Prefixes each selector with an ancestor selector and combinator.
    Ancestor {
        /// Selector prefix, including any trailing combinator whitespace.
        prefix: String,
        /// Stable variant ordering rank.
        order: u16,
    },
    /// Applies a selector pattern, replacing `&` with the current selector.
    Selector {
        /// Selector pattern containing `&` or an ancestor selector.
        pattern: String,
        /// Stable variant ordering rank.
        order: u16,
    },
    /// Wraps rules in a named at-rule. This is useful for supports and container queries.
    AtRule {
        /// At-rule name such as `supports` or `container`.
        name: String,
        /// At-rule prelude.
        prelude: String,
        /// Stable variant ordering rank.
        order: u16,
    },
}

impl VariantDefinition {
    /// Creates a pseudo selector variant.
    #[must_use]
    pub fn pseudo(suffix: impl Into<String>, order: u16) -> Self {
        Self::Pseudo { suffix: suffix.into(), order }
    }

    /// Creates an at-rule wrapper variant.
    #[must_use]
    pub fn media(name: impl Into<String>, prelude: impl Into<String>, order: u16) -> Self {
        Self::Media { name: name.into(), prelude: prelude.into(), order }
    }

    /// Creates an ancestor selector variant.
    #[must_use]
    pub fn ancestor(prefix: impl Into<String>, order: u16) -> Self {
        Self::Ancestor { prefix: prefix.into(), order }
    }

    /// Creates a composable selector-pattern variant.
    #[must_use]
    pub fn selector(pattern: impl Into<String>, order: u16) -> Self {
        Self::Selector { pattern: pattern.into(), order }
    }

    /// Creates a generic at-rule variant.
    #[must_use]
    pub fn at_rule(name: impl Into<String>, prelude: impl Into<String>, order: u16) -> Self {
        Self::AtRule { name: name.into(), prelude: prelude.into(), order }
    }

    /// Returns a canonical semantic fingerprint for this definition.
    ///
    /// The representation is deliberately independent of Rust's `Debug` formatting so cache and
    /// protocol identities remain stable across harmless implementation refactors.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        match self {
            Self::Pseudo { suffix, order } => {
                hash = hash_tag(hash, 0);
                hash = hash_text(hash, suffix);
                hash_u16(hash, *order)
            }
            Self::Media { name, prelude, order } => {
                hash = hash_tag(hash, 1);
                hash = hash_text(hash, name);
                hash = hash_text(hash, prelude);
                hash_u16(hash, *order)
            }
            Self::Ancestor { prefix, order } => {
                hash = hash_tag(hash, 2);
                hash = hash_text(hash, prefix);
                hash_u16(hash, *order)
            }
            Self::Selector { pattern, order } => {
                hash = hash_tag(hash, 3);
                hash = hash_text(hash, pattern);
                hash_u16(hash, *order)
            }
            Self::AtRule { name, prelude, order } => {
                hash = hash_tag(hash, 4);
                hash = hash_text(hash, name);
                hash = hash_text(hash, prelude);
                hash_u16(hash, *order)
            }
        }
    }

    fn order(&self) -> u16 {
        match self {
            Self::Pseudo { order, .. }
            | Self::Media { order, .. }
            | Self::Ancestor { order, .. }
            | Self::Selector { order, .. }
            | Self::AtRule { order, .. } => *order,
        }
    }
}

fn hash_tag(hash: u64, tag: u8) -> u64 {
    fnv1a(hash, &[tag])
}

fn hash_u16(hash: u64, value: u16) -> u64 {
    fnv1a(hash, &value.to_le_bytes())
}

fn hash_text(hash: u64, value: &str) -> u64 {
    let hash = fnv1a(hash, &u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    fnv1a(hash, value.as_bytes())
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Built-in variant definitions indexed by their source name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantRegistry {
    definitions: BTreeMap<String, VariantDefinition>,
}

impl VariantRegistry {
    /// Creates a registry containing the initial pseudo and dark variants.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self { definitions: BTreeMap::new() };
        for (name, suffix, order) in [
            ("hover", ":hover", 100),
            ("focus", ":focus", 110),
            ("active", ":active", 120),
            ("disabled", ":disabled", 130),
            ("focus-within", ":focus-within", 111),
            ("focus-visible", ":focus-visible", 112),
            ("visited", ":visited", 101),
            ("target", ":target", 102),
            ("first", ":first-child", 103),
            ("last", ":last-child", 104),
            ("only", ":only-child", 105),
            ("odd", ":nth-child(odd)", 106),
            ("even", ":nth-child(even)", 107),
            ("first-of-type", ":first-of-type", 108),
            ("last-of-type", ":last-of-type", 109),
            ("only-of-type", ":only-of-type", 113),
            ("empty", ":empty", 114),
            ("root", ":root", 115),
            ("required", ":required", 131),
            ("optional", ":optional", 132),
            ("valid", ":valid", 133),
            ("invalid", ":invalid", 134),
            ("in-range", ":in-range", 135),
            ("out-of-range", ":out-of-range", 136),
            ("placeholder-shown", ":placeholder-shown", 137),
            ("autofill", ":autofill", 138),
            ("read-only", ":read-only", 139),
            ("read-write", ":read-write", 140),
            ("default", ":default", 141),
            ("checked", ":checked", 142),
            ("indeterminate", ":indeterminate", 143),
            ("open", ":is([open], :popover-open)", 144),
            ("before", "::before", 145),
            ("after", "::after", 146),
            ("first-letter", "::first-letter", 147),
            ("first-line", "::first-line", 148),
            ("marker", "::marker", 149),
            ("selection", "::selection", 150),
            ("placeholder", "::placeholder", 151),
            ("file", "::file-selector-button", 152),
            ("backdrop", "::backdrop", 153),
        ] {
            registry.register(name, VariantDefinition::pseudo(suffix, order));
        }
        registry.register(
            "dark",
            VariantDefinition::media("media", "(prefers-color-scheme: dark)", 300),
        );
        registry.register("group-hover", VariantDefinition::ancestor(".group:hover ", 150));
        for (name, suffix, order) in [
            ("group-focus", ".group:focus ", 151),
            ("group-active", ".group:active ", 152),
            ("group-focus-within", ".group:focus-within ", 153),
            ("group-disabled", ".group:disabled ", 154),
            ("group-checked", ".group:checked ", 155),
        ] {
            registry.register(name, VariantDefinition::ancestor(suffix, order));
        }
        registry.register("peer-checked", VariantDefinition::ancestor(".peer:checked ~ ", 160));
        for (name, suffix, order) in [
            ("peer-hover", ".peer:hover ~ ", 161),
            ("peer-focus", ".peer:focus ~ ", 162),
            ("peer-active", ".peer:active ~ ", 163),
            ("peer-disabled", ".peer:disabled ~ ", 164),
            ("peer-invalid", ".peer:invalid ~ ", 165),
            ("peer-valid", ".peer:valid ~ ", 166),
        ] {
            registry.register(name, VariantDefinition::ancestor(suffix, order));
        }
        for (name, at_name, prelude, order) in [
            ("motion-safe", "media", "(prefers-reduced-motion: no-preference)", 310),
            ("motion-reduce", "media", "(prefers-reduced-motion: reduce)", 311),
            ("print", "media", "print", 312),
            ("portrait", "media", "(orientation: portrait)", 313),
            ("landscape", "media", "(orientation: landscape)", 314),
            ("contrast-more", "media", "(prefers-contrast: more)", 315),
            ("contrast-less", "media", "(prefers-contrast: less)", 316),
            ("forced-colors", "media", "(forced-colors: active)", 317),
        ] {
            registry.register(name, VariantDefinition::media(at_name, prelude, order));
        }
        registry.register("rtl", VariantDefinition::selector("[dir=\"rtl\"] &", 320));
        registry.register("ltr", VariantDefinition::selector("[dir=\"ltr\"] &", 321));
        registry.register(
            "supports-grid",
            VariantDefinition::at_rule("supports", "(display: grid)", 330),
        );
        registry
    }

    /// Registers or replaces a named variant.
    pub fn register(&mut self, name: impl Into<String>, definition: VariantDefinition) {
        self.definitions.insert(name.into(), definition);
    }

    /// Registers a named variant after validating its name and duplicate policy.
    pub fn register_checked(
        &mut self,
        name: impl Into<String>,
        definition: VariantDefinition,
    ) -> Result<(), VariantRegistryError> {
        let name = name.into();
        if name.is_empty()
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(VariantRegistryError::InvalidName(name));
        }
        if self.definitions.contains_key(&name) {
            return Err(VariantRegistryError::DuplicateName(name));
        }
        self.definitions.insert(name, definition);
        Ok(())
    }

    /// Returns a named variant definition.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&VariantDefinition> {
        self.definitions.get(name)
    }

    /// Returns all registered variant names in deterministic order.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.definitions.keys().map(String::as_str).collect()
    }

    /// Returns every registered definition in deterministic name order.
    pub fn definitions(&self) -> impl Iterator<Item = (&str, &VariantDefinition)> {
        self.definitions.iter().map(|(name, definition)| (name.as_str(), definition))
    }

    /// Returns metadata for one named variant.
    #[must_use]
    pub fn descriptor(&self, name: &str) -> Option<VariantDescriptor> {
        self.get(name).map(|definition| descriptor_for(name, definition))
    }

    /// Returns all variant metadata in deterministic order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<VariantDescriptor> {
        self.definitions().map(|(name, definition)| descriptor_for(name, definition)).collect()
    }

    /// Validates registered names and at-rule safety before compilation.
    pub fn validate(&self) -> Result<(), VariantRegistryError> {
        for (name, definition) in &self.definitions {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(VariantRegistryError::InvalidName(name.clone()));
            }
            let invalid = match definition {
                VariantDefinition::Pseudo { suffix, .. } => {
                    if !suffix.starts_with(':') {
                        Some("pseudo variants must start with `:`".to_owned())
                    } else {
                        safe_selector(suffix, name).err().map(|error| error.to_string())
                    }
                }
                VariantDefinition::Media { name: at_name, prelude, .. }
                | VariantDefinition::AtRule { name: at_name, prelude, .. } => {
                    safe_at_rule_name(at_name).err().map(|error| error.to_string()).or_else(|| {
                        safe_selector(prelude, name).err().map(|error| error.to_string())
                    })
                }
                VariantDefinition::Ancestor { prefix, .. }
                | VariantDefinition::Selector { pattern: prefix, .. } => {
                    safe_selector(prefix, name).err().map(|error| error.to_string())
                }
            };
            if let Some(message) = invalid {
                return Err(VariantRegistryError::InvalidDefinition {
                    name: name.clone(),
                    message,
                });
            }
        }
        Ok(())
    }

    /// Returns the highest registered ordering rank in a candidate's variant chain.
    #[must_use]
    pub fn order(&self, candidate: &CandidateAst<'_>, theme: &Theme) -> u16 {
        candidate
            .variants()
            .iter()
            .map(|variant| match variant.kind() {
                VariantKind::Named { name, .. } => self
                    .get(name)
                    .map_or_else(|| breakpoint_order(name, theme), VariantDefinition::order),
                VariantKind::Arbitrary { .. } | VariantKind::ArbitraryAtRule { .. } => 500,
            })
            .max()
            .unwrap_or(0)
    }
}

impl Default for VariantRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation errors raised by a variant registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VariantRegistryError {
    /// A name is empty or contains unsupported characters.
    InvalidName(String),
    /// A checked registration attempted to reuse a name.
    DuplicateName(String),
    /// A definition contains an unsafe selector or at-rule fragment.
    InvalidDefinition {
        /// Variant name containing the invalid definition.
        name: String,
        /// Validation failure detail.
        message: String,
    },
}

impl fmt::Display for VariantRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(formatter, "invalid variant name `{name}`"),
            Self::DuplicateName(name) => write!(formatter, "duplicate variant name `{name}`"),
            Self::InvalidDefinition { name, message } => {
                write!(formatter, "invalid variant definition `{name}`: {message}")
            }
        }
    }
}

impl Error for VariantRegistryError {}

fn descriptor_for(name: &str, definition: &VariantDefinition) -> VariantDescriptor {
    let (category, description) = match definition {
        VariantDefinition::Pseudo { .. } => {
            (VariantCategory::Pseudo, "Transforms the candidate selector with a pseudo selector")
        }
        VariantDefinition::Ancestor { .. } | VariantDefinition::Selector { .. } => {
            (VariantCategory::Selector, "Prefixes the candidate with an ancestor selector")
        }
        VariantDefinition::Media { .. } | VariantDefinition::AtRule { .. } => {
            (VariantCategory::AtRule, "Wraps the candidate in a CSS at-rule")
        }
    };
    VariantDescriptor {
        id: VariantId::new(name),
        names: vec![name.to_owned()],
        description: description.to_owned(),
        composable: true,
        ordering: u32::from(definition.order()),
        allowed_nesting: vec![
            "named".to_owned(),
            "arbitrary-selector".to_owned(),
            "arbitrary-at-rule".to_owned(),
        ],
        compatibility_profile: "native".to_owned(),
        examples: vec![format!("{name}:p-4")],
        category,
    }
}

/// The category of a variant resolution error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariantErrorKind {
    /// No registered or built-in variant matched the source segment.
    UnknownVariant,
    /// A variant value is malformed or unsupported.
    InvalidValue,
    /// A selector contains a structure-breaking character.
    UnsafeSelector,
}

impl VariantErrorKind {
    const fn code(self) -> DiagnosticCode {
        DiagnosticCode::new(match self {
            Self::UnknownVariant => "variant.unknown",
            Self::InvalidValue => "variant.invalid-value",
            Self::UnsafeSelector => "variant.unsafe-selector",
        })
    }
}

/// A typed error raised while applying a variant chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantError {
    kind: VariantErrorKind,
    name: String,
    value: Option<String>,
}

impl VariantError {
    fn new(kind: VariantErrorKind, name: &str, value: Option<&str>) -> Self {
        Self { kind, name: name.to_owned(), value: value.map(str::to_owned) }
    }

    /// Returns the error category.
    #[must_use]
    pub const fn kind(&self) -> VariantErrorKind {
        self.kind
    }

    /// Returns the variant name involved in the error.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Converts the error into a source-aware diagnostic.
    #[must_use]
    pub fn to_diagnostic(&self, source: SourceId, span: Span) -> Diagnostic {
        Diagnostic::error(self.kind.code(), self.to_string())
            .with_source(source)
            .with_span(span)
            .with_explanation(
                "The variant could not be applied because it is unknown, malformed, or unsafe.",
            )
    }
}

impl fmt::Display for VariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind, self.value.as_deref()) {
            (VariantErrorKind::UnknownVariant, _) => {
                write!(formatter, "unknown variant `{}`", self.name)
            }
            (VariantErrorKind::InvalidValue, Some(value)) => {
                write!(formatter, "invalid value `{value}` for variant `{}`", self.name)
            }
            (VariantErrorKind::UnsafeSelector, Some(value)) => {
                write!(formatter, "unsafe selector `{value}` for variant `{}`", self.name)
            }
            _ => write!(formatter, "invalid variant `{}`", self.name),
        }
    }
}

impl Error for VariantError {}

/// Applies a candidate's variants to a resolved base rule.
pub fn apply(
    candidate: &CandidateAst<'_>,
    mut rule: CssRule,
    theme: &Theme,
    registry: &VariantRegistry,
) -> Result<CssRule, VariantError> {
    for variant in candidate.variants().iter().rev() {
        rule = apply_one(variant, rule, theme, registry)?;
    }
    Ok(rule)
}

fn apply_one(
    variant: &VariantAst<'_>,
    rule: CssRule,
    theme: &Theme,
    registry: &VariantRegistry,
) -> Result<CssRule, VariantError> {
    match variant.kind() {
        VariantKind::Arbitrary { selector } => {
            let selector = decode_arbitrary(selector).map_err(|_| {
                VariantError::new(VariantErrorKind::UnsafeSelector, "[arbitrary]", Some(selector))
            })?;
            let selector = safe_selector(&selector, "[arbitrary]")?;
            Ok(rule.map_selectors(|base| apply_selector(selector, base)))
        }
        VariantKind::ArbitraryAtRule { name, prelude } => {
            let prelude = decode_arbitrary(prelude).map_err(|_| {
                VariantError::new(VariantErrorKind::UnsafeSelector, name, Some(prelude))
            })?;
            let name = safe_at_rule_name(name)?;
            let prelude = safe_selector(&prelude, name)?;
            Ok(CssRule::at_rule(rule.order(), name, prelude, vec![rule]))
        }
        VariantKind::Named { name, value } => {
            if let Some(definition) = registry.get(name) {
                return apply_definition(name, definition, rule);
            }
            if let Some(value) = value {
                if matches!(name, "data" | "aria") {
                    return apply_attribute(name, value, rule);
                }
                if name == "supports" {
                    let prelude = decode_arbitrary(value.content()).map_err(|_| {
                        VariantError::new(
                            VariantErrorKind::InvalidValue,
                            name,
                            Some(value.content()),
                        )
                    })?;
                    let prelude = safe_selector(&prelude, name)?;
                    return Ok(CssRule::at_rule(rule.order(), "supports", prelude, vec![rule]));
                }
                return Err(VariantError::new(
                    VariantErrorKind::InvalidValue,
                    name,
                    Some(value.content()),
                ));
            }
            if let Some(attribute) = name.strip_prefix("data-") {
                let attribute = safe_selector(attribute, name)?;
                if attribute.is_empty() {
                    return Err(VariantError::new(VariantErrorKind::InvalidValue, name, None));
                }
                return Ok(rule.map_selectors(|selector| format!("{selector}[data-{attribute}]")));
            }
            if let Some(attribute) = name.strip_prefix("aria-") {
                let attribute = safe_selector(attribute, name)?;
                if attribute.is_empty() {
                    return Err(VariantError::new(VariantErrorKind::InvalidValue, name, None));
                }
                return Ok(
                    rule.map_selectors(|selector| format!("{selector}[aria-{attribute}=\"true\"]"))
                );
            }
            if let Some(breakpoint) = theme.breakpoint(name) {
                let breakpoint = safe_selector(breakpoint, name)?;
                return Ok(CssRule::at_rule(
                    rule.order(),
                    "media",
                    format!("(min-width: {breakpoint})"),
                    vec![rule],
                ));
            }
            Err(VariantError::new(VariantErrorKind::UnknownVariant, name, None))
        }
    }
}

fn apply_definition(
    name: &str,
    definition: &VariantDefinition,
    rule: CssRule,
) -> Result<CssRule, VariantError> {
    match definition {
        VariantDefinition::Pseudo { suffix, .. } => {
            let suffix = safe_selector(suffix, name)?;
            if !suffix.starts_with(':') {
                return Err(VariantError::new(VariantErrorKind::InvalidValue, name, Some(suffix)));
            }
            Ok(rule.map_selectors(|selector| format!("{selector}{suffix}")))
        }
        VariantDefinition::Media { name, prelude, .. } => {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(VariantError::new(VariantErrorKind::UnsafeSelector, name, Some(name)));
            }
            let prelude = safe_selector(prelude, name)?;
            Ok(CssRule::at_rule(rule.order(), name, prelude, vec![rule]))
        }
        VariantDefinition::Ancestor { prefix, .. } => {
            let prefix = safe_selector(prefix, name)?;
            Ok(rule.map_selectors(|selector| format!("{prefix}{selector}")))
        }
        VariantDefinition::Selector { pattern, .. } => {
            let pattern = safe_selector(pattern, name)?;
            Ok(rule.map_selectors(|selector| apply_selector(pattern, selector)))
        }
        VariantDefinition::AtRule { name, prelude, .. } => {
            let name = safe_at_rule_name(name)?;
            let prelude = safe_selector(prelude, name)?;
            Ok(CssRule::at_rule(rule.order(), name, prelude, vec![rule]))
        }
    }
}

fn apply_attribute(
    name: &str,
    value: ValueAst<'_>,
    rule: CssRule,
) -> Result<CssRule, VariantError> {
    let content = decode_arbitrary(value.content()).map_err(|_| {
        VariantError::new(VariantErrorKind::InvalidValue, name, Some(value.content()))
    })?;
    let content = safe_selector(&content, name)?;
    if content.is_empty() {
        return Err(VariantError::new(VariantErrorKind::InvalidValue, name, Some(content)));
    }
    Ok(rule.map_selectors(|selector| format!("{selector}[{name}-{content}]")))
}

fn safe_selector<'a>(selector: &'a str, name: &str) -> Result<&'a str, VariantError> {
    let contains_style_close = selector
        .as_bytes()
        .windows(b"</style".len())
        .any(|window| window.eq_ignore_ascii_case(b"</style"));
    if selector
        .chars()
        .any(|character| character.is_control() || matches!(character, '{' | '}' | ';'))
        || selector.contains("/*")
        || selector.contains("*/")
        || contains_style_close
    {
        return Err(VariantError::new(VariantErrorKind::UnsafeSelector, name, Some(selector)));
    }
    Ok(selector)
}

fn safe_at_rule_name(name: &str) -> Result<&str, VariantError> {
    let bytes = name.as_bytes();
    let start = usize::from(bytes.starts_with(b"-"));
    if bytes.get(start).is_none_or(|byte| !byte.is_ascii_alphabetic() && *byte != b'_')
        || !bytes[start..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
    {
        return Err(VariantError::new(VariantErrorKind::UnsafeSelector, name, Some(name)));
    }
    Ok(name)
}

/// Validates a selector supplied by a caller before variant transformations are applied.
///
/// The validation intentionally uses the same safety rules as variant-generated selector
/// fragments. It is a structural guard, not a complete CSS selector grammar.
pub fn validate_selector(selector: &str) -> Result<(), VariantError> {
    safe_selector(selector, "[base]").map(|_| ())
}

fn apply_selector(selector: &str, base: &str) -> String {
    if selector.contains('&') {
        selector.replace('&', base)
    } else {
        format!("{selector} {base}")
    }
}

/// Base ordering rank assigned to the narrowest configured breakpoint.
const BREAKPOINT_RANK_BASE: u16 = 200;
/// Rank spacing between adjacent configured breakpoints.
const BREAKPOINT_RANK_STEP: u16 = 10;

/// Parses a breakpoint value into min-width pixels.
///
/// `px` values are used directly, `rem`/`em` scale by the conventional 16px
/// root, and unitless numbers are read as pixels. Anything else returns `None`
/// so the breakpoint sorts deterministically after all parseable values.
fn breakpoint_width_px(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    let split = trimmed
        .char_indices()
        .find(|(_, character)| !character.is_ascii_digit() && !matches!(character, '.' | '+' | '-'))
        .map_or(trimmed.len(), |(index, _)| index);
    let (number, unit) = trimmed.split_at(split);
    if number.is_empty() {
        return None;
    }
    let magnitude: f32 = number.parse().ok()?;
    if !magnitude.is_finite() {
        return None;
    }
    let unit = unit.trim();
    let scale = if unit.eq_ignore_ascii_case("px") || unit.is_empty() {
        1.0
    } else if unit.eq_ignore_ascii_case("rem") || unit.eq_ignore_ascii_case("em") {
        16.0
    } else {
        return None;
    };
    Some(magnitude * scale)
}

/// Returns the deterministic ordering rank for a theme breakpoint.
///
/// Configured breakpoints rank by ascending min-width value so custom
/// breakpoints interleave correctly with the built-in set. Equal widths break
/// ties by name, and unparseable values sort after all parseable ones, also by
/// name. Returns `None` when `name` is not a configured breakpoint.
#[must_use]
pub fn breakpoint_rank(name: &str, theme: &Theme) -> Option<u16> {
    let tokens = theme.namespace("breakpoint")?;
    if !tokens.contains_key(name) {
        return None;
    }
    let mut ordered: Vec<(&str, Option<f32>)> =
        tokens.iter().map(|(key, value)| (key.as_str(), breakpoint_width_px(value))).collect();
    ordered.sort_by(|left, right| {
        match (left.1, right.1) {
            (Some(left_px), Some(right_px)) => {
                left_px.partial_cmp(&right_px).unwrap_or(Ordering::Equal)
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| left.0.cmp(right.0))
    });
    ordered.iter().position(|(key, _)| *key == name).map(|index| {
        BREAKPOINT_RANK_BASE.saturating_add((index as u16).saturating_mul(BREAKPOINT_RANK_STEP))
    })
}

fn breakpoint_order(name: &str, theme: &Theme) -> u16 {
    breakpoint_rank(name, theme).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use utilitycss_css_ir::{CssDeclaration, CssRule, CssSerializationMode, OrderKey};
    use utilitycss_syntax::parse;
    use utilitycss_theme::Theme;

    use super::{apply, VariantDefinition, VariantErrorKind, VariantRegistry};

    fn base_rule() -> CssRule {
        CssRule::style(
            OrderKey::default(),
            ".hover\\:bg-red-500",
            vec![CssDeclaration::new("color", "red")],
        )
    }

    #[test]
    fn applies_pseudo_and_responsive_variants_in_author_order() {
        let candidate = parse("md:hover:bg-red-500").expect("candidate is valid");
        let rule = apply(&candidate, base_rule(), &Theme::default(), &VariantRegistry::new())
            .expect("built-in variants are valid");
        let mut document = utilitycss_css_ir::CssDocument::new();
        document.push(rule);

        assert_eq!(
            document.to_css(CssSerializationMode::Minified),
            "@media (min-width: 768px){.hover\\:bg-red-500:hover{color:red;}}"
        );
    }

    #[test]
    fn applies_arbitrary_and_attribute_variants() {
        let arbitrary = parse("[&>*]:p-4").expect("candidate is valid");
        let arbitrary_rule =
            apply(&arbitrary, base_rule(), &Theme::default(), &VariantRegistry::new())
                .expect("arbitrary selector is safe");
        assert_eq!(arbitrary_rule.selector(), Some(".hover\\:bg-red-500>*"));

        let data = parse("data-[state=open]:p-4").expect("candidate is valid");
        let data_rule = apply(&data, base_rule(), &Theme::default(), &VariantRegistry::new())
            .expect("attribute selector is safe");
        assert_eq!(data_rule.selector(), Some(".hover\\:bg-red-500[data-state=open]"));
    }

    #[test]
    fn rejects_unknown_variants() {
        let candidate = parse("unknown:p-4").expect("candidate is syntactically valid");
        let error = apply(&candidate, base_rule(), &Theme::default(), &VariantRegistry::new())
            .expect_err("variant is not registered");

        assert_eq!(error.kind(), VariantErrorKind::UnknownVariant);
    }

    #[test]
    fn rejects_unsafe_registered_variant_definitions() {
        let mut registry = VariantRegistry::new();
        registry.register("unsafe", VariantDefinition::pseudo(":hover;body{}", 999));
        let candidate = parse("unsafe:flex").expect("candidate is valid");

        let error = apply(&candidate, base_rule(), &Theme::default(), &registry)
            .expect_err("unsafe registered selector is rejected");

        assert_eq!(error.kind(), VariantErrorKind::UnsafeSelector);

        let mut registry = VariantRegistry::new();
        registry.register("unsafe-at", VariantDefinition::at_rule("1media", "screen", 999));
        let candidate = parse("unsafe-at:flex").expect("candidate is valid");
        let error = apply(&candidate, base_rule(), &Theme::default(), &registry)
            .expect_err("at-rule names must be valid identifiers");
        assert_eq!(error.kind(), VariantErrorKind::UnsafeSelector);
    }

    #[test]
    fn validates_registered_variant_definitions_before_resolution() {
        let mut registry = VariantRegistry::new();
        registry.register("unsafe", VariantDefinition::pseudo(":hover;body{}", 999));

        let error = registry.validate().expect_err("unsafe definitions fail early");
        assert!(matches!(
            error,
            super::VariantRegistryError::InvalidDefinition { name, .. } if name == "unsafe"
        ));
    }

    #[test]
    fn rejects_unsafe_theme_breakpoints_at_resolution() {
        let theme = Theme::builder().breakpoint("evil", "0px){body{color:red}").build();
        let candidate = parse("evil:flex").expect("candidate is valid");

        let error = apply(&candidate, base_rule(), &theme, &VariantRegistry::new())
            .expect_err("theme breakpoints are validated before media emission");

        assert_eq!(error.kind(), VariantErrorKind::UnsafeSelector);
    }

    #[test]
    fn custom_breakpoints_rank_by_min_width_value() {
        let theme = Theme::builder()
            .breakpoint("xs", "320px")
            .breakpoint("tablet", "48rem")
            .breakpoint("2xl", "1536px")
            .build();
        let registry = VariantRegistry::new();
        let rank = |candidate: &str| {
            let parsed = parse(candidate).expect("candidate is valid");
            registry.order(&parsed, &theme)
        };

        // Width order: xs 320 < sm 640 < md 768 = tablet 768 < lg 1024 < xl 1280 < 2xl 1536.
        // The md/tablet tie breaks deterministically by name ("md" < "tablet").
        assert_eq!(rank("xs:p-4"), 200);
        assert_eq!(rank("sm:p-4"), 210);
        assert_eq!(rank("md:p-4"), 220);
        assert_eq!(rank("tablet:p-4"), 230);
        assert_eq!(rank("lg:p-4"), 240);
        assert_eq!(rank("xl:p-4"), 250);
        assert_eq!(rank("2xl:p-4"), 260);
    }

    #[test]
    fn default_breakpoints_keep_historical_ranks() {
        let theme = Theme::default();
        let registry = VariantRegistry::new();
        let rank = |candidate: &str| {
            let parsed = parse(candidate).expect("candidate is valid");
            registry.order(&parsed, &theme)
        };

        assert_eq!(
            (rank("sm:p-4"), rank("md:p-4"), rank("lg:p-4"), rank("xl:p-4")),
            (200, 210, 220, 230)
        );
        assert_eq!(rank("unknown:p-4"), 0);
    }

    #[test]
    fn variant_fingerprints_include_variant_kind_and_fields() {
        assert_ne!(
            VariantDefinition::pseudo(":hover", 1).fingerprint(),
            VariantDefinition::pseudo(":focus", 1).fingerprint()
        );
        assert_ne!(
            VariantDefinition::pseudo(":hover", 1).fingerprint(),
            VariantDefinition::selector("&:hover", 1).fingerprint()
        );
    }

    #[test]
    fn applies_common_state_pseudo_and_at_rule_variants() {
        let candidate =
            parse("supports-[display:grid]:focus-visible:p-4").expect("candidate is valid");
        let rule = apply(&candidate, base_rule(), &Theme::default(), &VariantRegistry::new())
            .expect("registered variants are valid");
        let mut document = utilitycss_css_ir::CssDocument::new();
        document.push(rule);
        let css = document.to_css(CssSerializationMode::Minified);
        assert!(css.contains("@supports display:grid"));
        assert!(css.contains(":focus-visible"));

        let aria = parse("aria-checked:p-4").expect("aria variant is valid");
        let aria_rule = apply(&aria, base_rule(), &Theme::default(), &VariantRegistry::new())
            .expect("aria variant is valid");
        assert_eq!(aria_rule.selector(), Some(".hover\\:bg-red-500[aria-checked=\"true\"]"));
    }
}
