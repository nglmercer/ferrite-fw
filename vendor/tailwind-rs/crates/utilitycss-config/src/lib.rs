//! Declarative, runtime-independent configuration loading for host adapters.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeMap, error::Error, fmt, fs, path::Path};

use serde::Deserialize;
use utilitycss_css_ir::{BrowserTarget, CssSerializationMode};
use utilitycss_theme::{Theme, ThemeBuilder, ThemeValidationError};
use utilitycss_utilities::{
    ColorKind, Dimension, SpacingEdge, UtilityDefinition, UtilityRegistry, ValueNamespace,
};
use utilitycss_variants::{VariantDefinition, VariantRegistry};

/// A named compatibility profile applied before user extensions.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompatibilityPreset {
    /// The native utilitycss baseline semantics.
    #[default]
    UtilityCss,
    /// The documented Tailwind-inspired subset implemented by this release.
    TailwindV4Subset,
    /// The explicitly scoped Tailwind-inspired v3 grammar profile.
    TailwindV3Subset,
}

impl CompatibilityPreset {
    /// Returns the stable preset identifier.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::UtilityCss => "utilitycss",
            Self::TailwindV4Subset => "tailwind-v4-subset",
            Self::TailwindV3Subset => "tailwind-v3-subset",
        }
    }
}

/// A composable declarative preset fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preset {
    /// Stable preset identifier.
    pub id: String,
    /// Preset semantic version.
    pub version: String,
    /// Utility definitions contributed by the preset.
    pub utilities: UtilityRegistry,
    /// Variant definitions contributed by the preset.
    pub variants: VariantRegistry,
    /// Theme fragment contributed by the preset.
    pub theme: Theme,
    /// Compatibility profile name.
    pub compatibility: CompatibilityPreset,
}

impl Preset {
    /// Returns the native built-in preset.
    #[must_use]
    pub fn native() -> Self {
        Self {
            id: "utilitycss".to_owned(),
            version: "0.1.0".to_owned(),
            utilities: UtilityRegistry::default(),
            variants: VariantRegistry::default(),
            theme: Theme::default(),
            compatibility: CompatibilityPreset::UtilityCss,
        }
    }

    /// Returns the scoped Tailwind-v4-like preset.
    #[must_use]
    pub fn tailwind_v4_like() -> Self {
        Self {
            id: "tailwind-v4-subset".to_owned(),
            version: "0.1.0".to_owned(),
            utilities: UtilityRegistry::default(),
            variants: VariantRegistry::default(),
            theme: Theme::default(),
            compatibility: CompatibilityPreset::TailwindV4Subset,
        }
    }

    /// Returns the scoped Tailwind-v3-like preset.
    #[must_use]
    pub fn tailwind_v3_like() -> Self {
        Self {
            id: "tailwind-v3-subset".to_owned(),
            version: "0.1.0".to_owned(),
            utilities: UtilityRegistry::default(),
            variants: VariantRegistry::default(),
            theme: Theme::default(),
            compatibility: CompatibilityPreset::TailwindV3Subset,
        }
    }
}

/// A validated configuration file ready to be translated into compiler options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigFile {
    preset: CompatibilityPreset,
    theme: Theme,
    utilities: UtilityRegistry,
    variants: VariantRegistry,
    serialization_mode: CssSerializationMode,
    browser_target: BrowserTarget,
}

impl ConfigFile {
    /// Returns the selected compatibility profile.
    #[must_use]
    pub const fn preset(&self) -> CompatibilityPreset {
        self.preset
    }

    /// Returns the configured theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns the utility registry after preset and plugin extensions are applied.
    #[must_use]
    pub const fn utilities(&self) -> &UtilityRegistry {
        &self.utilities
    }

    /// Returns the variant registry after preset and plugin extensions are applied.
    #[must_use]
    pub const fn variants(&self) -> &VariantRegistry {
        &self.variants
    }

    /// Returns the configured output mode.
    #[must_use]
    pub const fn serialization_mode(&self) -> CssSerializationMode {
        self.serialization_mode
    }

    /// Returns the configured browser capability target.
    #[must_use]
    pub const fn browser_target(&self) -> BrowserTarget {
        self.browser_target
    }

    /// Returns a deterministic configuration fingerprint.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = fnv1a(0xcbf29ce484222325_u64, b"utilitycss-config-fingerprint-v2");
        hash = fnv1a(hash, &self.theme.fingerprint().to_le_bytes());
        for (name, definition) in self.utilities.definitions() {
            hash = fnv1a_text(hash, name);
            hash = fnv1a(hash, &definition.fingerprint().to_le_bytes());
        }
        for (name, definition) in self.variants.definitions() {
            hash = fnv1a_text(hash, name);
            hash = fnv1a(hash, &definition.fingerprint().to_le_bytes());
        }
        hash = fnv1a_text(hash, self.preset.name());
        hash = fnv1a_text(
            hash,
            match self.serialization_mode {
                CssSerializationMode::Pretty => "pretty",
                CssSerializationMode::Minified => "minified",
            },
        );
        hash = fnv1a_text(hash, self.browser_target.as_str());
        hash
    }

    /// Validates all registry names and configured semantic definitions.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.theme.validate().map_err(ConfigError::InvalidTheme)?;
        self.utilities.validate().map_err(|error| ConfigError::InvalidPlugin {
            name: "utilities".to_owned(),
            message: error.to_string(),
        })?;
        self.variants.validate().map_err(|error| ConfigError::InvalidPlugin {
            name: "variants".to_owned(),
            message: error.to_string(),
        })?;
        Ok(())
    }
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            preset: CompatibilityPreset::UtilityCss,
            theme: Theme::default(),
            utilities: UtilityRegistry::default(),
            variants: VariantRegistry::default(),
            serialization_mode: CssSerializationMode::Minified,
            browser_target: BrowserTarget::Modern,
        }
    }
}

/// Errors raised while loading or validating configuration.
#[derive(Debug)]
pub enum ConfigError {
    /// The JSON document could not be decoded.
    InvalidJson(String),
    /// The configuration file could not be read.
    Io {
        /// Path that failed to load.
        path: String,
        /// Host I/O error message.
        message: String,
    },
    /// The configured output mode is not recognized.
    InvalidMode(String),
    /// The configured compatibility profile is not recognized.
    InvalidPreset(String),
    /// The configured browser target is not recognized.
    InvalidBrowserTarget(String),
    /// A plugin utility or variant definition is invalid.
    InvalidPlugin {
        /// User-provided utility or variant name.
        name: String,
        /// Validation failure detail.
        message: String,
    },
    /// A configured theme token contains an unsafe CSS fragment.
    InvalidTheme(ThemeValidationError),
    /// The CSS-first configuration document is malformed or unsafe.
    InvalidCss(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => {
                write!(formatter, "invalid JSON configuration: {message}")
            }
            Self::Io { path, message } => {
                write!(formatter, "could not read configuration `{path}`: {message}")
            }
            Self::InvalidMode(mode) => write!(formatter, "invalid output mode `{mode}`"),
            Self::InvalidPreset(preset) => {
                write!(formatter, "invalid compatibility preset `{preset}`")
            }
            Self::InvalidBrowserTarget(target) => {
                write!(formatter, "invalid browser target `{target}`")
            }
            Self::InvalidPlugin { name, message } => {
                write!(formatter, "invalid plugin definition `{name}`: {message}")
            }
            Self::InvalidTheme(error) => write!(formatter, "invalid theme: {error}"),
            Self::InvalidCss(message) => write!(formatter, "invalid CSS configuration: {message}"),
        }
    }
}

impl Error for ConfigError {}

/// Parses a JSON configuration document.
pub fn parse_json(input: &str) -> Result<ConfigFile, ConfigError> {
    let raw: RawConfig =
        serde_json::from_str(input).map_err(|error| ConfigError::InvalidJson(error.to_string()))?;
    let mode = match raw.mode.as_deref() {
        None | Some("minified") => CssSerializationMode::Minified,
        Some("pretty") => CssSerializationMode::Pretty,
        Some(mode) => return Err(ConfigError::InvalidMode(mode.to_owned())),
    };

    let browser_target = match raw.browser_target.as_deref() {
        None => BrowserTarget::Modern,
        Some(target) => BrowserTarget::parse(target)
            .ok_or_else(|| ConfigError::InvalidBrowserTarget(target.to_owned()))?,
    };

    let preset = match raw.preset.as_deref() {
        None | Some("utilitycss") => CompatibilityPreset::UtilityCss,
        Some("tailwind-v4-subset") => CompatibilityPreset::TailwindV4Subset,
        Some("tailwind-v3-subset") => CompatibilityPreset::TailwindV3Subset,
        Some(preset) => return Err(ConfigError::InvalidPreset(preset.to_owned())),
    };

    let mut builder = Theme::builder();
    for (key, value) in raw.theme.colors {
        builder = builder.color(key, value);
    }
    for (key, value) in raw.theme.spacing {
        builder = builder.spacing(key, value);
    }
    for (key, value) in raw.theme.breakpoints {
        builder = builder.breakpoint(key, value);
    }
    for (key, value) in raw.theme.radii {
        builder = builder.radius(key, value);
    }
    for (key, value) in raw.theme.widths {
        builder = builder.width(key, value);
    }
    for (key, value) in raw.theme.heights {
        builder = builder.height(key, value);
    }
    for (key, value) in raw.theme.font_families {
        builder = builder.font_family(key, value);
    }
    for (key, value) in raw.theme.font_sizes {
        builder = builder.font_size(key, value);
    }
    for (key, value) in raw.theme.line_heights {
        builder = builder.line_height(key, value);
    }
    for (key, value) in raw.theme.letter_spacings {
        builder = builder.letter_spacing(key, value);
    }
    for (key, value) in raw.theme.shadows {
        builder = builder.shadow(key, value);
    }
    for (key, value) in raw.theme.durations {
        builder = builder.duration(key, value);
    }
    for (key, value) in raw.theme.easings {
        builder = builder.ease(key, value);
    }
    for (key, value) in raw.theme.z_indices {
        builder = builder.z_index(key, value);
    }
    for (namespace, values) in raw.theme.custom {
        for (key, value) in values {
            builder = builder.token(namespace.clone(), key, value);
        }
    }
    let mut utilities = UtilityRegistry::default();
    for (name, definition) in raw.utilities {
        utilities.register(name.clone(), utility_definition(&name, definition)?);
    }
    let mut variants = VariantRegistry::default();
    for (name, definition) in raw.variants {
        variants.register(name.clone(), variant_definition(&name, definition)?);
    }
    let config = ConfigFile {
        preset,
        theme: builder.build(),
        utilities,
        variants,
        serialization_mode: mode,
        browser_target,
    };
    config.validate()?;
    Ok(config)
}

/// Parses the supported CSS-first configuration subset.
///
/// Only `@theme`, `@utility`, and `@variant` directives are interpreted. Other CSS is rejected so
/// that configuration cannot accidentally execute or read host-language content.
pub fn parse_css(input: &str) -> Result<ConfigFile, ConfigError> {
    let input = strip_css_comments(input)?;
    validate_css_surface(&input)?;
    let mut builder = Theme::builder();
    for block in named_blocks(&input, "theme")? {
        for (property, value) in css_declarations(block)? {
            let Some(property) = property.strip_prefix("--") else {
                return Err(ConfigError::InvalidCss(
                    "@theme declarations must use custom properties".to_owned(),
                ));
            };
            let (namespace, key) = theme_variable(property);
            builder = set_theme_token(builder, &namespace, &key, &value);
        }
    }

    let mut utilities = UtilityRegistry::default();
    for (index, (name, block)) in named_named_blocks(&input, "utility")?.into_iter().enumerate() {
        let declarations = css_declarations(block)?;
        if declarations.len() != 1 {
            return Err(ConfigError::InvalidCss(format!(
                "@utility `{name}` must contain exactly one declaration"
            )));
        }
        let (property, value) = declarations.into_iter().next().unwrap_or_default();
        let order = u16::try_from(500_usize.saturating_add(index)).unwrap_or(u16::MAX);
        utilities.register(name, UtilityDefinition::static_declaration(property, value, order));
    }

    let mut variants = VariantRegistry::default();
    for (index, (name, body)) in named_statements(&input, "variant")?.into_iter().enumerate() {
        let order = u16::try_from(400_usize.saturating_add(index)).unwrap_or(u16::MAX);
        let body = body.trim();
        let body =
            body.strip_prefix('(').and_then(|body| body.strip_suffix(')')).unwrap_or(body).trim();
        if body.starts_with('@') {
            let body = body.strip_prefix('@').unwrap_or(body);
            let name_end = body
                .char_indices()
                .find(|(_, character)| character.is_whitespace() || *character == '(')
                .map_or(body.len(), |(offset, _)| offset);
            let at_name = body.get(..name_end).unwrap_or_default();
            let prelude = body.get(name_end..).unwrap_or_default().trim();
            if at_name.is_empty() || prelude.is_empty() {
                return Err(ConfigError::InvalidCss(format!(
                    "@variant `{name}` has an invalid at-rule"
                )));
            }
            variants.register(name, VariantDefinition::at_rule(at_name, prelude, order));
        } else if body.is_empty() {
            return Err(ConfigError::InvalidCss(format!("@variant `{name}` is empty")));
        } else {
            variants.register(name, VariantDefinition::selector(body, order));
        }
    }

    let config = ConfigFile {
        preset: CompatibilityPreset::UtilityCss,
        theme: builder.build(),
        utilities,
        variants,
        serialization_mode: CssSerializationMode::Minified,
        browser_target: BrowserTarget::Modern,
    };
    config.validate()?;
    Ok(config)
}

/// Loads a CSS-first configuration file from disk.
pub fn load_css(path: impl AsRef<Path>) -> Result<ConfigFile, ConfigError> {
    let path = path.as_ref();
    let input = fs::read_to_string(path).map_err(|error| ConfigError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    parse_css(&input)
}

/// Loads and parses a JSON configuration file from disk.
pub fn load(path: impl AsRef<Path>) -> Result<ConfigFile, ConfigError> {
    let path = path.as_ref();
    let input = fs::read_to_string(path).map_err(|error| ConfigError::Io {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("css"))
    {
        parse_css(&input)
    } else {
        parse_json(&input)
    }
}

fn set_theme_token(builder: ThemeBuilder, namespace: &str, key: &str, value: &str) -> ThemeBuilder {
    match namespace {
        "color" => builder.color(key, value),
        "spacing" => builder.spacing(key, value),
        "breakpoint" => builder.breakpoint(key, value),
        "radius" => builder.radius(key, value),
        "width" => builder.width(key, value),
        "height" => builder.height(key, value),
        "font-family" => builder.font_family(key, value),
        "font-size" => builder.font_size(key, value),
        "line-height" => builder.line_height(key, value),
        "letter-spacing" => builder.letter_spacing(key, value),
        "shadow" => builder.shadow(key, value),
        "duration" => builder.duration(key, value),
        "ease" => builder.ease(key, value),
        "z-index" => builder.z_index(key, value),
        _ => builder.token(namespace, key, value),
    }
}

fn theme_variable(variable: &str) -> (String, String) {
    let prefixes = [
        ("font-family-", "font-family"),
        ("font-size-", "font-size"),
        ("line-height-", "line-height"),
        ("letter-spacing-", "letter-spacing"),
        ("breakpoint-", "breakpoint"),
        ("duration-", "duration"),
        ("z-index-", "z-index"),
        ("spacing-", "spacing"),
        ("radius-", "radius"),
        ("color-", "color"),
        ("width-", "width"),
        ("height-", "height"),
        ("shadow-", "shadow"),
        ("ease-", "ease"),
        ("font-", "font-family"),
        ("leading-", "line-height"),
        ("tracking-", "letter-spacing"),
    ];
    for (prefix, namespace) in prefixes {
        if let Some(key) = variable.strip_prefix(prefix) {
            return (namespace.to_owned(), key.to_owned());
        }
    }
    let separator = variable.find('-').unwrap_or(variable.len());
    let namespace = variable[..separator].to_owned();
    let key = variable
        .get(separator + usize::from(separator < variable.len())..)
        .unwrap_or("DEFAULT")
        .to_owned();
    (namespace, if key.is_empty() { "DEFAULT".to_owned() } else { key })
}

fn strip_css_comments(input: &str) -> Result<String, ConfigError> {
    let mut output = String::with_capacity(input.len());
    let mut comment = false;
    let mut quote = None;
    let mut escaped = false;
    let bytes = input.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if comment {
            if bytes.get(cursor..cursor + 2) == Some(b"*/") {
                comment = false;
                cursor += 2;
            } else {
                cursor += input[cursor..].chars().next().map_or(1, char::len_utf8);
            }
            continue;
        }
        let character = input[cursor..].chars().next().unwrap_or('\0');
        if escaped {
            output.push(character);
            escaped = false;
            cursor += character.len_utf8();
        } else if character == '\\' && quote.is_some() {
            output.push(character);
            escaped = true;
            cursor += character.len_utf8();
        } else if let Some(active_quote) = quote {
            output.push(character);
            if character == active_quote {
                quote = None;
            }
            cursor += character.len_utf8();
        } else if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            comment = true;
            cursor += 2;
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
            output.push(character);
            cursor += character.len_utf8();
        } else {
            output.push(character);
            cursor += character.len_utf8();
        }
    }
    if comment {
        return Err(ConfigError::InvalidCss("unterminated comment".to_owned()));
    }
    Ok(output)
}

fn named_blocks<'a>(input: &'a str, directive: &str) -> Result<Vec<&'a str>, ConfigError> {
    Ok(named_named_blocks(input, directive)?.into_iter().map(|(_, block)| block).collect())
}

fn named_named_blocks<'a>(
    input: &'a str,
    directive: &str,
) -> Result<Vec<(String, &'a str)>, ConfigError> {
    let marker = format!("@{directive}");
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(start) = top_level_directive(input, &marker, cursor) {
        let after = start + marker.len();
        let mut name_start = after;
        while input.as_bytes().get(name_start).is_some_and(u8::is_ascii_whitespace) {
            name_start += 1;
        }
        let mut open = name_start;
        while input
            .as_bytes()
            .get(open)
            .is_some_and(|byte| *byte != b'{' && !byte.is_ascii_whitespace())
        {
            open += 1;
        }
        let name = input.get(name_start..open).unwrap_or_default().trim();
        while input.as_bytes().get(open).is_some_and(u8::is_ascii_whitespace) {
            open += 1;
        }
        if input.as_bytes().get(open) != Some(&b'{') {
            return Err(ConfigError::InvalidCss(format!("@{directive} `{name}` requires a block")));
        }
        let close = balanced_block(input, open)?;
        result.push((name.to_owned(), &input[open + 1..close]));
        cursor = close + 1;
    }
    Ok(result)
}

fn named_statements(input: &str, directive: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let marker = format!("@{directive}");
    let mut result = Vec::new();
    let mut cursor = 0;
    while let Some(start) = top_level_directive(input, &marker, cursor) {
        let after = start + marker.len();
        let mut name_start = after;
        while input.as_bytes().get(name_start).is_some_and(u8::is_ascii_whitespace) {
            name_start += 1;
        }
        let mut name_end = name_start;
        while input
            .as_bytes()
            .get(name_end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
        {
            name_end += 1;
        }
        let name = input.get(name_start..name_end).unwrap_or_default();
        let semicolon = find_statement_end(input, name_end)
            .ok_or_else(|| ConfigError::InvalidCss(format!("@{directive} `{name}` requires `;")))?;
        let body = input.get(name_end..semicolon).unwrap_or_default().trim().to_owned();
        result.push((name.to_owned(), body));
        cursor = semicolon + 1;
    }
    Ok(result)
}

fn top_level_directive(input: &str, marker: &str, start: usize) -> Option<usize> {
    let mut braces = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in input.get(start..)?.char_indices() {
        let index = start + relative;
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => braces = braces.saturating_add(1),
            '}' => braces = braces.saturating_sub(1),
            '@' if braces == 0
                && input.get(index..).is_some_and(|tail| tail.starts_with(marker))
                && input.as_bytes().get(index + marker.len()).is_none_or(|byte| {
                    !byte.is_ascii_alphanumeric() && *byte != b'-' && *byte != b'_'
                }) =>
            {
                return Some(index)
            }
            _ => {}
        }
    }
    None
}

fn validate_css_surface(input: &str) -> Result<(), ConfigError> {
    let bytes = input.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        if bytes[cursor] != b'@' {
            return Err(ConfigError::InvalidCss(
                "only top-level @theme, @utility, and @variant directives are allowed".to_owned(),
            ));
        }
        let name_start = cursor + 1;
        let mut name_end = name_start;
        while bytes
            .get(name_end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
        {
            name_end += 1;
        }
        let name = input.get(name_start..name_end).unwrap_or_default();
        if !matches!(name, "theme" | "utility" | "variant") {
            return Err(ConfigError::InvalidCss(format!(
                "unsupported configuration directive `@{name}`"
            )));
        }
        if name == "variant" {
            let end = find_statement_end(input, name_end).ok_or_else(|| {
                ConfigError::InvalidCss("@variant requires a top-level `;`".to_owned())
            })?;
            cursor = end + 1;
            continue;
        }
        let mut open = name_end;
        if name == "utility" {
            while bytes.get(open).is_some_and(u8::is_ascii_whitespace) {
                open += 1;
            }
            let utility_name_start = open;
            while bytes
                .get(open)
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
            {
                open += 1;
            }
            if utility_name_start == open {
                return Err(ConfigError::InvalidCss(
                    "@utility requires a non-empty utility name".to_owned(),
                ));
            }
        }
        while bytes.get(open).is_some_and(u8::is_ascii_whitespace) {
            open += 1;
        }
        if bytes.get(open) != Some(&b'{') {
            return Err(ConfigError::InvalidCss(format!("@{name} requires a block")));
        }
        cursor = balanced_block(input, open)?.saturating_add(1);
    }
    Ok(())
}

fn find_statement_end(input: &str, start: usize) -> Option<usize> {
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (relative, character) in input.get(start..)?.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets = brackets.saturating_add(1),
            ']' => brackets = brackets.saturating_sub(1),
            ';' if parentheses == 0 && brackets == 0 => return Some(start + relative),
            _ => {}
        }
    }
    None
}

fn balanced_block(input: &str, open: usize) -> Result<usize, ConfigError> {
    let mut depth = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in input[open..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            _ => {}
        }
    }
    Err(ConfigError::InvalidCss("unterminated configuration block".to_owned()))
}

fn css_declarations(block: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let mut result = Vec::new();
    for declaration in split_css_top_level(block, ';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let Some(index) = find_css_top_level(declaration, ':') else {
            return Err(ConfigError::InvalidCss(format!(
                "declaration `{declaration}` is missing `:`"
            )));
        };
        let property = declaration[..index].trim();
        let value = declaration[index + 1..].trim();
        if property.is_empty() || value.is_empty() {
            return Err(ConfigError::InvalidCss(format!(
                "declaration `{declaration}` is incomplete"
            )));
        }
        result.push((property.to_owned(), value.to_owned()));
    }
    Ok(result)
}

fn split_css_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' => parentheses = parentheses.saturating_add(1),
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets = brackets.saturating_add(1),
            ']' => brackets = brackets.saturating_sub(1),
            character if character == delimiter && parentheses == 0 && brackets == 0 => {
                pieces.push(&input[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    pieces.push(&input[start..]);
    pieces
}

fn find_css_top_level(input: &str, delimiter: char) -> Option<usize> {
    let mut pieces = split_css_top_level(input, delimiter);
    if pieces.len() < 2 {
        return None;
    }
    let first = pieces.remove(0);
    Some(first.len())
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn fnv1a_text(hash: u64, value: &str) -> u64 {
    let hash = fnv1a(hash, &u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    fnv1a(hash, value.as_bytes())
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default, rename = "browserTarget")]
    browser_target: Option<String>,
    #[serde(default)]
    theme: RawTheme,
    #[serde(default)]
    utilities: BTreeMap<String, RawUtilityDefinition>,
    #[serde(default)]
    variants: BTreeMap<String, RawVariantDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum RawUtilityDefinition {
    Static { property: String, value: String, order: u16 },
    Composite { declarations: BTreeMap<String, String>, order: u16 },
    Spacing { margin: bool, edge: String, order: u16 },
    Gap { edge: String, order: u16 },
    Size { dimension: String, order: u16 },
    Color { kind: String, order: u16 },
    Radius { order: u16 },
    AlignItems { order: u16 },
    JustifyContent { order: u16 },
    GridColumns { order: u16 },
    Functional { property: String, namespace: String, required: bool, negative: bool, order: u16 },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum RawVariantDefinition {
    Pseudo { suffix: String, order: u16 },
    Media { name: String, prelude: String, order: u16 },
    Ancestor { prefix: String, order: u16 },
    Selector { pattern: String, order: u16 },
    AtRule { name: String, prelude: String, order: u16 },
}

fn utility_definition(
    name: &str,
    definition: RawUtilityDefinition,
) -> Result<UtilityDefinition, ConfigError> {
    let invalid = |message: &str| ConfigError::InvalidPlugin {
        name: name.to_owned(),
        message: message.to_owned(),
    };
    match definition {
        RawUtilityDefinition::Static { property, value, order } => {
            Ok(UtilityDefinition::static_declaration(property, value, order))
        }
        RawUtilityDefinition::Composite { declarations, order } => {
            Ok(UtilityDefinition::composite(declarations.into_iter().collect(), order))
        }
        RawUtilityDefinition::Spacing { margin, edge, order } => Ok(UtilityDefinition::Spacing {
            margin,
            edge: parse_edge(&edge).ok_or_else(|| invalid("unknown spacing edge"))?,
            order,
        }),
        RawUtilityDefinition::Gap { edge, order } => Ok(UtilityDefinition::Gap {
            edge: parse_edge(&edge).ok_or_else(|| invalid("unknown gap edge"))?,
            order,
        }),
        RawUtilityDefinition::Size { dimension, order } => Ok(UtilityDefinition::Size {
            dimension: parse_dimension(&dimension).ok_or_else(|| invalid("unknown dimension"))?,
            order,
        }),
        RawUtilityDefinition::Color { kind, order } => Ok(UtilityDefinition::Color {
            kind: parse_color_kind(&kind).ok_or_else(|| invalid("unknown color kind"))?,
            order,
        }),
        RawUtilityDefinition::Radius { order } => Ok(UtilityDefinition::Radius { order }),
        RawUtilityDefinition::AlignItems { order } => Ok(UtilityDefinition::AlignItems { order }),
        RawUtilityDefinition::JustifyContent { order } => {
            Ok(UtilityDefinition::JustifyContent { order })
        }
        RawUtilityDefinition::GridColumns { order } => Ok(UtilityDefinition::GridColumns { order }),
        RawUtilityDefinition::Functional { property, namespace, required, negative, order } => {
            let namespace =
                parse_namespace(&namespace).ok_or_else(|| invalid("unknown value namespace"))?;
            Ok(UtilityDefinition::functional(property, namespace, required, negative, order))
        }
    }
}

fn variant_definition(
    name: &str,
    definition: RawVariantDefinition,
) -> Result<VariantDefinition, ConfigError> {
    let invalid = |message: &str| ConfigError::InvalidPlugin {
        name: name.to_owned(),
        message: message.to_owned(),
    };
    match definition {
        RawVariantDefinition::Pseudo { suffix, order } => {
            if !suffix.starts_with(':') {
                return Err(invalid("pseudo suffix must start with `:`"));
            }
            Ok(VariantDefinition::pseudo(suffix, order))
        }
        RawVariantDefinition::Media { name, prelude, order } => {
            if name.is_empty() || prelude.is_empty() {
                return Err(invalid("media name and prelude must not be empty"));
            }
            Ok(VariantDefinition::media(name, prelude, order))
        }
        RawVariantDefinition::Ancestor { prefix, order } => {
            if prefix.is_empty() {
                return Err(invalid("ancestor prefix must not be empty"));
            }
            Ok(VariantDefinition::ancestor(prefix, order))
        }
        RawVariantDefinition::Selector { pattern, order } => {
            if pattern.is_empty() {
                return Err(invalid("selector pattern must not be empty"));
            }
            Ok(VariantDefinition::selector(pattern, order))
        }
        RawVariantDefinition::AtRule { name, prelude, order } => {
            if name.is_empty() || prelude.is_empty() {
                return Err(invalid("at-rule name and prelude must not be empty"));
            }
            Ok(VariantDefinition::at_rule(name, prelude, order))
        }
    }
}

fn parse_edge(value: &str) -> Option<SpacingEdge> {
    Some(match value {
        "all" => SpacingEdge::All,
        "x" => SpacingEdge::X,
        "y" => SpacingEdge::Y,
        "top" => SpacingEdge::Top,
        "right" => SpacingEdge::Right,
        "bottom" => SpacingEdge::Bottom,
        "left" => SpacingEdge::Left,
        _ => return None,
    })
}

fn parse_dimension(value: &str) -> Option<Dimension> {
    Some(match value {
        "width" => Dimension::Width,
        "height" => Dimension::Height,
        "minWidth" => Dimension::MinWidth,
        "maxWidth" => Dimension::MaxWidth,
        "minHeight" => Dimension::MinHeight,
        "maxHeight" => Dimension::MaxHeight,
        _ => return None,
    })
}

fn parse_color_kind(value: &str) -> Option<ColorKind> {
    Some(match value {
        "background" => ColorKind::Background,
        "text" => ColorKind::Text,
        "border" => ColorKind::Border,
        _ => return None,
    })
}

fn parse_namespace(value: &str) -> Option<ValueNamespace> {
    Some(match value {
        "raw" => ValueNamespace::Raw,
        "spacing" => ValueNamespace::Spacing,
        "color" => ValueNamespace::Color,
        "width" => ValueNamespace::Width,
        "height" => ValueNamespace::Height,
        "radius" => ValueNamespace::Radius,
        "font-family" => ValueNamespace::FontFamily,
        "font-size" => ValueNamespace::FontSize,
        "line-height" => ValueNamespace::LineHeight,
        "letter-spacing" => ValueNamespace::LetterSpacing,
        "shadow" => ValueNamespace::Shadow,
        "duration" => ValueNamespace::Duration,
        "ease" => ValueNamespace::Ease,
        "z-index" => ValueNamespace::ZIndex,
        "number" => ValueNamespace::Number,
        "length" => ValueNamespace::Length,
        "percentage" => ValueNamespace::Percentage,
        "integer" => ValueNamespace::Integer,
        "keyword" => ValueNamespace::Keyword,
        _ => return None,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    spacing: BTreeMap<String, String>,
    #[serde(default)]
    breakpoints: BTreeMap<String, String>,
    #[serde(default)]
    radii: BTreeMap<String, String>,
    #[serde(default)]
    widths: BTreeMap<String, String>,
    #[serde(default)]
    heights: BTreeMap<String, String>,
    #[serde(default, rename = "fontFamilies")]
    font_families: BTreeMap<String, String>,
    #[serde(default, rename = "fontSizes")]
    font_sizes: BTreeMap<String, String>,
    #[serde(default, rename = "lineHeights")]
    line_heights: BTreeMap<String, String>,
    #[serde(default, rename = "letterSpacings")]
    letter_spacings: BTreeMap<String, String>,
    #[serde(default)]
    shadows: BTreeMap<String, String>,
    #[serde(default)]
    durations: BTreeMap<String, String>,
    #[serde(default)]
    easings: BTreeMap<String, String>,
    #[serde(default, rename = "zIndices")]
    z_indices: BTreeMap<String, String>,
    #[serde(default)]
    custom: BTreeMap<String, BTreeMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use utilitycss_css_ir::{BrowserTarget, CssSerializationMode};

    use super::{parse_json, CompatibilityPreset, ConfigError};

    #[test]
    fn parses_typed_theme_overrides_and_mode() {
        let config = parse_json(
            r##"{
                "mode": "pretty",
                "theme": {
                    "colors": { "brand-500": "#123456" },
                    "spacing": { "4": "1.25rem" }
                }
            }"##,
        )
        .expect("configuration is valid");

        assert_eq!(config.serialization_mode(), CssSerializationMode::Pretty);
        assert_eq!(config.theme().color("brand-500"), Some("#123456"));
        assert_eq!(config.theme().spacing("4"), Some("1.25rem"));
        assert_eq!(config.theme().spacing("8"), Some("2rem"));
    }

    #[test]
    fn parses_versioned_preset_and_plugin_registries() {
        let config = parse_json(
            r##"{
                "preset": "tailwind-v4-subset",
                "utilities": {
                    "content-center": {
                        "type": "static",
                        "property": "place-content",
                        "value": "center",
                        "order": 63
                    }
                },
                "variants": {
                    "motion-safe": {
                        "type": "media",
                        "name": "media",
                        "prelude": "(prefers-reduced-motion: no-preference)",
                        "order": 170
                    }
                }
            }"##,
        )
        .expect("preset and plugin definitions are valid");

        assert_eq!(config.preset(), CompatibilityPreset::TailwindV4Subset);
        assert!(config.utilities().contains("content-center"));
        assert!(config.variants().get("motion-safe").is_some());
    }

    #[test]
    fn rejects_unknown_presets_and_invalid_plugin_shapes() {
        assert!(matches!(
            parse_json(r#"{"preset":"tailwind"}"#),
            Err(ConfigError::InvalidPreset(preset)) if preset == "tailwind"
        ));
        assert!(matches!(
            parse_json(
                r#"{"variants":{"bad":{"type":"pseudo","suffix":"hover","order":1}}}"#
            ),
            Err(ConfigError::InvalidPlugin { name, .. }) if name == "bad"
        ));
    }

    #[test]
    fn rejects_unknown_output_modes() {
        let error = parse_json(r#"{"mode":"compact"}"#).expect_err("mode is unsupported");

        assert!(matches!(error, ConfigError::InvalidMode(mode) if mode == "compact"));
    }

    #[test]
    fn parses_and_fingerprints_browser_target_policy() {
        let modern = parse_json(r#"{"browserTarget":"modern"}"#).expect("target is valid");
        let legacy = parse_json(r#"{"browserTarget":"legacy"}"#).expect("target is valid");

        assert_eq!(modern.browser_target(), BrowserTarget::Modern);
        assert_eq!(legacy.browser_target(), BrowserTarget::Legacy);
        assert_ne!(modern.fingerprint(), legacy.fingerprint());
        assert!(matches!(
            parse_json(r#"{"browserTarget":"ancient"}"#),
            Err(ConfigError::InvalidBrowserTarget(target)) if target == "ancient"
        ));
    }

    #[test]
    fn parses_css_first_theme_utilities_and_selector_variants() {
        let config = super::parse_css(
            r#"
                @theme {
                    --color-brand-500: oklch(62% 0.2 260);
                    --spacing-card: 1.125rem;
                    --font-display: "Inter", sans-serif;
                }
                @utility content-auto { content-visibility: auto; }
                @variant theme-midnight (&:where([data-theme="midnight"] *));
            "#,
        )
        .expect("CSS configuration is valid");

        assert_eq!(config.theme().color("brand-500"), Some("oklch(62% 0.2 260)"));
        assert_eq!(config.theme().spacing("card"), Some("1.125rem"));
        assert_eq!(config.theme().font_family("display"), Some("\"Inter\", sans-serif"));
        assert!(config.utilities().contains("content-auto"));
        assert!(config.variants().get("theme-midnight").is_some());
    }

    #[test]
    fn css_first_parser_preserves_utf8_and_rejects_unterminated_comments() {
        let config = super::parse_css(r#"@theme { --font-display: "Inter Élite", sans-serif; }"#)
            .expect("UTF-8 CSS configuration is valid");
        assert_eq!(config.theme().font_family("display"), Some("\"Inter Élite\", sans-serif"));
        assert!(matches!(
            super::parse_css("/* missing"),
            Err(ConfigError::InvalidCss(message)) if message == "unterminated comment"
        ));
        let config = super::parse_css(
            r#"@theme {
                --content-example: "literal /* not a comment */";
                /* this comment is removed */
                --spacing-card: 1rem;
            }"#,
        )
        .expect("comment markers inside strings are preserved");
        assert_eq!(
            config.theme().token("content", "example"),
            Some("\"literal /* not a comment */\"")
        );
        assert_eq!(config.theme().spacing("card"), Some("1rem"));
        assert!(matches!(
            super::parse_css("body { color: red; }"),
            Err(ConfigError::InvalidCss(message)) if message.contains("top-level")
        ));
    }

    #[test]
    fn css_first_parser_ignores_directive_text_inside_theme_values() {
        let config = super::parse_css(
            r#"@theme {
                --content-example: "@utility fake { color: red; }";
                --font-display: "literal @variant fake (&)";
            }"#,
        )
        .expect("quoted directive text is not a top-level directive");

        assert_eq!(
            config.theme().token("content", "example"),
            Some("\"@utility fake { color: red; }\"")
        );
        assert!(!config.utilities().contains("fake"));
        assert!(!config.variants().names().contains(&"fake"));
    }

    #[test]
    fn rejects_unsafe_theme_values_before_compilation() {
        assert!(matches!(
            parse_json(r#"{"theme":{"breakpoints":{"evil":"0px){body{color:red}"}}}"#),
            Err(ConfigError::InvalidTheme(error)) if error.namespace() == "breakpoint" && error.key() == "evil"
        ));
        assert!(matches!(
            parse_json(r#"{"theme":{"radii":{"DEFAULT":"0px;display:block"}}}"#),
            Err(ConfigError::InvalidTheme(error)) if error.namespace() == "radius" && error.key() == "DEFAULT"
        ));
    }
}
