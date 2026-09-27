//! Typed, deterministic theme tokens used during semantic utility resolution.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeMap, error::Error, fmt};

use serde::Serialize;

/// Design tokens consumed by the initial utility compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Theme {
    colors: BTreeMap<String, String>,
    spacing: BTreeMap<String, String>,
    breakpoints: BTreeMap<String, String>,
    radii: BTreeMap<String, String>,
    widths: BTreeMap<String, String>,
    heights: BTreeMap<String, String>,
    font_families: BTreeMap<String, String>,
    font_sizes: BTreeMap<String, String>,
    line_heights: BTreeMap<String, String>,
    letter_spacings: BTreeMap<String, String>,
    shadows: BTreeMap<String, String>,
    durations: BTreeMap<String, String>,
    easings: BTreeMap<String, String>,
    z_indices: BTreeMap<String, String>,
    custom: BTreeMap<String, BTreeMap<String, String>>,
}

/// One deterministic theme token with its namespace and provenance key.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ThemeToken {
    /// Token namespace, such as `color` or `spacing`.
    pub namespace: String,
    /// Token key within the namespace.
    pub key: String,
    /// CSS value.
    pub value: String,
}

/// A theme token that cannot be safely embedded in generated CSS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeValidationError {
    namespace: String,
    key: String,
}

impl ThemeValidationError {
    fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self { namespace: namespace.into(), key: key.into() }
    }

    /// Returns the token namespace containing the unsafe value.
    #[must_use]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the token key containing the unsafe value.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Display for ThemeValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "theme token `{}:{}` contains an unsafe CSS fragment",
            self.namespace, self.key
        )
    }
}

impl Error for ThemeValidationError {}

impl Theme {
    /// Returns a builder pre-populated with the built-in baseline tokens.
    #[must_use]
    pub fn builder() -> ThemeBuilder {
        ThemeBuilder::with_defaults()
    }

    /// Looks up a color token by name.
    #[must_use]
    pub fn color(&self, key: &str) -> Option<&str> {
        self.colors.get(key).map(String::as_str)
    }

    /// Looks up a spacing token by name.
    #[must_use]
    pub fn spacing(&self, key: &str) -> Option<&str> {
        self.spacing.get(key).map(String::as_str)
    }

    /// Looks up a responsive breakpoint by name.
    #[must_use]
    pub fn breakpoint(&self, key: &str) -> Option<&str> {
        self.breakpoints.get(key).map(String::as_str)
    }

    /// Looks up a border-radius token by name.
    #[must_use]
    pub fn radius(&self, key: &str) -> Option<&str> {
        self.radii.get(key).map(String::as_str)
    }

    /// Looks up a width token by name.
    #[must_use]
    pub fn width(&self, key: &str) -> Option<&str> {
        self.widths.get(key).map(String::as_str)
    }

    /// Looks up a height token by name.
    #[must_use]
    pub fn height(&self, key: &str) -> Option<&str> {
        self.heights.get(key).map(String::as_str)
    }

    /// Looks up a font-family token by name.
    #[must_use]
    pub fn font_family(&self, key: &str) -> Option<&str> {
        self.font_families.get(key).map(String::as_str)
    }

    /// Looks up a font-size token by name.
    #[must_use]
    pub fn font_size(&self, key: &str) -> Option<&str> {
        self.font_sizes.get(key).map(String::as_str)
    }

    /// Looks up a line-height token by name.
    #[must_use]
    pub fn line_height(&self, key: &str) -> Option<&str> {
        self.line_heights.get(key).map(String::as_str)
    }

    /// Looks up a letter-spacing token by name.
    #[must_use]
    pub fn letter_spacing(&self, key: &str) -> Option<&str> {
        self.letter_spacings.get(key).map(String::as_str)
    }

    /// Looks up a token in a named namespace.
    #[must_use]
    pub fn token(&self, namespace: &str, key: &str) -> Option<&str> {
        self.namespace(namespace).and_then(|tokens| tokens.get(key).map(String::as_str))
    }

    /// Returns all keys and values in a namespace in deterministic order.
    #[must_use]
    pub fn namespace(&self, namespace: &str) -> Option<&BTreeMap<String, String>> {
        match namespace {
            "color" | "colors" => Some(&self.colors),
            "spacing" => Some(&self.spacing),
            "breakpoint" | "breakpoints" => Some(&self.breakpoints),
            "radius" | "radii" => Some(&self.radii),
            "width" | "widths" => Some(&self.widths),
            "height" | "heights" => Some(&self.heights),
            "font" | "font-family" => Some(&self.font_families),
            "font-size" => Some(&self.font_sizes),
            "line-height" => Some(&self.line_heights),
            "letter-spacing" => Some(&self.letter_spacings),
            "shadow" | "shadows" => Some(&self.shadows),
            "duration" | "durations" => Some(&self.durations),
            "ease" | "easings" => Some(&self.easings),
            "z" | "z-index" => Some(&self.z_indices),
            _ => self.custom.get(namespace),
        }
    }

    /// Returns every token with its namespace, sorted by namespace and key.
    #[must_use]
    pub fn tokens(&self) -> Vec<ThemeToken> {
        let mut tokens = Vec::new();
        for namespace in [
            "color",
            "spacing",
            "breakpoint",
            "radius",
            "width",
            "height",
            "font-family",
            "font-size",
            "line-height",
            "letter-spacing",
            "shadow",
            "duration",
            "ease",
            "z-index",
        ] {
            if let Some(values) = self.namespace(namespace) {
                tokens.extend(values.iter().map(|(key, value)| ThemeToken {
                    namespace: namespace.to_owned(),
                    key: key.clone(),
                    value: value.clone(),
                }));
            }
        }
        for (namespace, values) in &self.custom {
            tokens.extend(values.iter().map(|(key, value)| ThemeToken {
                namespace: namespace.clone(),
                key: key.clone(),
                value: value.clone(),
            }));
        }
        tokens
    }

    /// Validates every configured value against the compiler's structural CSS safety policy.
    ///
    /// This check is intentionally conservative. Theme values are untrusted configuration and
    /// are eventually embedded in declarations, selectors, or at-rule preludes. Individual
    /// semantic sinks still validate values as defense in depth.
    pub fn validate(&self) -> Result<(), ThemeValidationError> {
        for token in self.tokens() {
            if unsafe_css_fragment(&token.value) {
                return Err(ThemeValidationError::new(token.namespace, token.key));
            }
        }
        Ok(())
    }

    /// Returns a stable fingerprint for all theme tokens.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        for map in [
            &self.colors,
            &self.spacing,
            &self.breakpoints,
            &self.radii,
            &self.widths,
            &self.heights,
            &self.font_families,
            &self.font_sizes,
            &self.line_heights,
            &self.letter_spacings,
            &self.shadows,
            &self.durations,
            &self.easings,
            &self.z_indices,
        ] {
            for (key, value) in map {
                hash = fnv1a(hash, key.as_bytes());
                hash = fnv1a(hash, &[0]);
                hash = fnv1a(hash, value.as_bytes());
                hash = fnv1a(hash, &[0xff]);
            }
            hash = fnv1a(hash, &[0xfe]);
        }
        for (namespace, values) in &self.custom {
            hash = fnv1a(hash, namespace.as_bytes());
            hash = fnv1a(hash, &[0xfd]);
            for (key, value) in values {
                hash = fnv1a(hash, key.as_bytes());
                hash = fnv1a(hash, &[0]);
                hash = fnv1a(hash, value.as_bytes());
                hash = fnv1a(hash, &[0xff]);
            }
        }
        hash
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// A builder for deterministic theme tokens.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThemeBuilder {
    colors: BTreeMap<String, String>,
    spacing: BTreeMap<String, String>,
    breakpoints: BTreeMap<String, String>,
    radii: BTreeMap<String, String>,
    widths: BTreeMap<String, String>,
    heights: BTreeMap<String, String>,
    font_families: BTreeMap<String, String>,
    font_sizes: BTreeMap<String, String>,
    line_heights: BTreeMap<String, String>,
    letter_spacings: BTreeMap<String, String>,
    shadows: BTreeMap<String, String>,
    durations: BTreeMap<String, String>,
    easings: BTreeMap<String, String>,
    z_indices: BTreeMap<String, String>,
    custom: BTreeMap<String, BTreeMap<String, String>>,
}

impl ThemeBuilder {
    /// Creates a builder with the built-in baseline tokens.
    #[must_use]
    pub fn new() -> Self {
        Self::with_defaults()
    }

    fn with_defaults() -> Self {
        let mut builder = Self::default();
        for (key, value) in [
            ("0", "0px"),
            ("px", "1px"),
            ("0.5", "0.125rem"),
            ("1", "0.25rem"),
            ("2", "0.5rem"),
            ("3", "0.75rem"),
            ("4", "1rem"),
            ("6", "1.5rem"),
            ("8", "2rem"),
            ("12", "3rem"),
            ("16", "4rem"),
            ("24", "6rem"),
            ("32", "8rem"),
            ("40", "10rem"),
            ("48", "12rem"),
            ("56", "14rem"),
            ("64", "16rem"),
            ("72", "18rem"),
            ("80", "20rem"),
            ("96", "24rem"),
        ] {
            builder = builder.spacing(key, value);
        }
        for (key, value) in [
            ("transparent", "transparent"),
            ("black", "#000"),
            ("white", "#fff"),
            ("red-500", "#ef4444"),
            ("red-50", "#fef2f2"),
            ("red-600", "#dc2626"),
            ("orange-500", "#f97316"),
            ("amber-500", "#f59e0b"),
            ("yellow-500", "#eab308"),
            ("green-500", "#22c55e"),
            ("emerald-500", "#10b981"),
            ("teal-500", "#14b8a6"),
            ("cyan-500", "#06b6d4"),
            ("sky-500", "#0ea5e9"),
            ("blue-500", "#3b82f6"),
            ("indigo-500", "#6366f1"),
            ("violet-500", "#8b5cf6"),
            ("purple-500", "#a855f7"),
            ("fuchsia-500", "#d946ef"),
            ("pink-500", "#ec4899"),
            ("rose-500", "#f43f5e"),
            ("gray-50", "#f9fafb"),
            ("gray-100", "#f3f4f6"),
            ("gray-500", "#6b7280"),
            ("gray-900", "#111827"),
            ("brand-600", "oklch(55% 0.2 260)"),
        ] {
            builder = builder.color(key, value);
        }
        for (key, value) in [("sm", "640px"), ("md", "768px"), ("lg", "1024px"), ("xl", "1280px")] {
            builder = builder.breakpoint(key, value);
        }
        for (key, value) in [
            ("none", "0px"),
            ("sm", "0.125rem"),
            ("DEFAULT", "0.25rem"),
            ("md", "0.375rem"),
            ("lg", "0.5rem"),
            ("full", "9999px"),
        ] {
            builder = builder.radius(key, value);
        }
        for (key, value) in [("auto", "auto"), ("full", "100%"), ("screen", "100vw")] {
            builder = builder.width(key, value);
        }
        for (key, value) in [("auto", "auto"), ("full", "100%"), ("screen", "100vh")] {
            builder = builder.height(key, value);
        }
        for (key, value) in [
            ("sans", "ui-sans-serif, system-ui, sans-serif"),
            ("serif", "ui-serif, Georgia, serif"),
            ("mono", "ui-monospace, SFMono-Regular, monospace"),
        ] {
            builder = builder.font_family(key, value);
        }
        for (key, value) in [
            ("xs", "0.75rem"),
            ("sm", "0.875rem"),
            ("base", "1rem"),
            ("lg", "1.125rem"),
            ("xl", "1.25rem"),
            ("2xl", "1.5rem"),
            ("3xl", "1.875rem"),
            ("4xl", "2.25rem"),
            ("5xl", "3rem"),
            ("6xl", "3.75rem"),
            ("7xl", "4.5rem"),
            ("8xl", "6rem"),
            ("9xl", "8rem"),
        ] {
            builder = builder.font_size(key, value);
        }
        for (key, value) in [
            ("none", "1"),
            ("tight", "1.25"),
            ("snug", "1.375"),
            ("normal", "1.5"),
            ("relaxed", "1.625"),
            ("loose", "2"),
        ] {
            builder = builder.line_height(key, value);
        }
        for (key, value) in [
            ("tighter", "-0.05em"),
            ("tight", "-0.025em"),
            ("normal", "0em"),
            ("wide", "0.025em"),
            ("wider", "0.05em"),
            ("widest", "0.1em"),
        ] {
            builder = builder.letter_spacing(key, value);
        }
        for (key, value) in [
            ("none", "none"),
            ("sm", "0 1px 2px 0 rgb(0 0 0 / 0.05)"),
            ("DEFAULT", "0 1px 3px 0 rgb(0 0 0 / 0.1)"),
            ("md", "0 4px 6px -1px rgb(0 0 0 / 0.1)"),
        ] {
            builder = builder.shadow(key, value);
        }
        for (key, value) in [
            ("75", "75ms"),
            ("100", "100ms"),
            ("150", "150ms"),
            ("200", "200ms"),
            ("300", "300ms"),
            ("500", "500ms"),
            ("700", "700ms"),
            ("1000", "1000ms"),
        ] {
            builder = builder.duration(key, value);
        }
        for (key, value) in [
            ("linear", "linear"),
            ("in", "cubic-bezier(0.4, 0, 1, 1)"),
            ("out", "cubic-bezier(0, 0, 0.2, 1)"),
            ("in-out", "cubic-bezier(0.4, 0, 0.2, 1)"),
        ] {
            builder = builder.ease(key, value);
        }
        for (key, value) in [
            ("0", "0"),
            ("10", "10"),
            ("20", "20"),
            ("30", "30"),
            ("40", "40"),
            ("50", "50"),
            ("auto", "auto"),
        ] {
            builder = builder.z_index(key, value);
        }
        builder
    }

    /// Inserts or replaces a color token.
    #[must_use]
    pub fn color(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.colors.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a spacing token.
    #[must_use]
    pub fn spacing(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.spacing.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a breakpoint token.
    #[must_use]
    pub fn breakpoint(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.breakpoints.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a border-radius token.
    #[must_use]
    pub fn radius(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.radii.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a width token.
    #[must_use]
    pub fn width(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.widths.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a height token.
    #[must_use]
    pub fn height(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.heights.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a font-family token.
    #[must_use]
    pub fn font_family(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.font_families.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a font-size token.
    #[must_use]
    pub fn font_size(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.font_sizes.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a line-height token.
    #[must_use]
    pub fn line_height(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.line_heights.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a letter-spacing token.
    #[must_use]
    pub fn letter_spacing(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.letter_spacings.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a box-shadow token.
    #[must_use]
    pub fn shadow(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.shadows.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a transition duration token.
    #[must_use]
    pub fn duration(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.durations.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces an easing token.
    #[must_use]
    pub fn ease(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.easings.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a z-index token.
    #[must_use]
    pub fn z_index(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.z_indices.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces a token in a custom namespace.
    #[must_use]
    pub fn token(
        mut self,
        namespace: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.custom.entry(namespace.into()).or_default().insert(key.into(), value.into());
        self
    }

    /// Builds an immutable theme.
    #[must_use]
    pub fn build(self) -> Theme {
        Theme {
            colors: self.colors,
            spacing: self.spacing,
            breakpoints: self.breakpoints,
            radii: self.radii,
            widths: self.widths,
            heights: self.heights,
            font_families: self.font_families,
            font_sizes: self.font_sizes,
            line_heights: self.line_heights,
            letter_spacings: self.letter_spacings,
            shadows: self.shadows,
            durations: self.durations,
            easings: self.easings,
            z_indices: self.z_indices,
            custom: self.custom,
        }
    }
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn unsafe_css_fragment(value: &str) -> bool {
    let contains_style_close = value
        .as_bytes()
        .windows(b"</style".len())
        .any(|window| window.eq_ignore_ascii_case(b"</style"));
    if contains_style_close {
        return true;
    }
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in value.char_indices() {
        if character.is_control() {
            return true;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        if value[offset..].starts_with("/*") || value[offset..].starts_with("*/") {
            return true;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if matches!(character, ';' | '{' | '}') {
            return true;
        }
    }
    quote.is_some()
}

#[cfg(test)]
mod tests {
    use super::Theme;

    #[test]
    fn builtins_provide_initial_tokens() {
        let theme = Theme::default();

        assert_eq!(theme.spacing("4"), Some("1rem"));
        assert_eq!(theme.color("red-500"), Some("#ef4444"));
        assert_eq!(theme.breakpoint("md"), Some("768px"));
        assert_eq!(theme.radius("DEFAULT"), Some("0.25rem"));
    }

    #[test]
    fn custom_tokens_change_a_stable_fingerprint() {
        let first = Theme::default().fingerprint();
        let second = Theme::builder().color("brand-500", "#123456").build().fingerprint();

        assert_ne!(first, second);
    }

    #[test]
    fn rejects_structural_css_fragments_in_theme_values() {
        let theme = Theme::builder().breakpoint("evil", "0px){body{color:red}").build();

        let error = theme.validate().expect_err("unsafe theme values are rejected");

        assert_eq!(error.namespace(), "breakpoint");
        assert_eq!(error.key(), "evil");
    }
}
