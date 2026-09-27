//! A small CSS intermediate representation with deterministic serialization.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::cmp::Ordering;

use serde::Serialize;

/// Controls whitespace emitted by [`CssDocument::to_css`].
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum CssSerializationMode {
    /// Emit readable, indented CSS.
    #[default]
    Pretty,
    /// Emit compact CSS without optional whitespace.
    Minified,
}

/// Browser capability target used by deterministic compatibility analysis.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum BrowserTarget {
    /// Current modern browsers with the full vNext feature set.
    #[default]
    Modern,
    /// Evergreen browser releases suitable for normal production output.
    Evergreen,
    /// Safari 15.0 (September 2021), the 15.x series floor: every tracked
    /// feature newer than 15.0 reports unsupported, including features added
    /// in later 15.x releases such as `oklch()` and unprefixed masking (15.4).
    Safari15,
    /// A conservative legacy target that accepts only broadly established CSS features.
    Legacy,
}

impl BrowserTarget {
    /// Returns the stable configuration name for this target.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Evergreen => "evergreen",
            Self::Safari15 => "safari-15",
            Self::Legacy => "legacy",
        }
    }

    /// Parses a stable browser-target name.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "modern" => Some(Self::Modern),
            "evergreen" => Some(Self::Evergreen),
            "safari-15" | "safari15" => Some(Self::Safari15),
            "legacy" => Some(Self::Legacy),
            _ => None,
        }
    }

    /// Returns whether this target is expected to support a CSS feature.
    ///
    /// Safari 15 floors are verified against MDN browser-compat-data:
    /// `oklch()` and unprefixed masking stabilized in 15.4, `color-mix()` in
    /// 16.2, `light-dark()` in 17.5, `content-visibility` and unprefixed
    /// `backdrop-filter` in 18.0, and `field-sizing` in 26.2.
    #[must_use]
    pub const fn supports(self, feature: CssFeature) -> bool {
        match self {
            Self::Modern | Self::Evergreen => true,
            Self::Safari15 => !matches!(
                feature,
                CssFeature::ColorMix
                    | CssFeature::LightDark
                    | CssFeature::Oklch
                    | CssFeature::ContentVisibility
                    | CssFeature::FieldSizing
                    | CssFeature::BackdropFilter
                    | CssFeature::Masking
            ),
            Self::Legacy => false,
        }
    }
}

/// CSS features that may require browser-aware lowering or a compatibility warning.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum CssFeature {
    /// The `color-mix()` color interpolation function.
    ColorMix,
    /// The `light-dark()` color function.
    LightDark,
    /// The `oklch()` color function.
    Oklch,
    /// The `content-visibility` property.
    ContentVisibility,
    /// The `field-sizing` property.
    FieldSizing,
    /// The `backdrop-filter` property or function.
    BackdropFilter,
    /// The CSS masking property family.
    Masking,
}

impl CssFeature {
    /// Returns a stable feature name for manifests and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ColorMix => "color-mix",
            Self::LightDark => "light-dark",
            Self::Oklch => "oklch",
            Self::ContentVisibility => "content-visibility",
            Self::FieldSizing => "field-sizing",
            Self::BackdropFilter => "backdrop-filter",
            Self::Masking => "masking",
        }
    }
}

/// Browser compatibility findings for a CSS IR document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserSupportReport {
    /// Target used for the analysis.
    pub target: BrowserTarget,
    /// Features required by the document in deterministic order.
    pub required: Vec<CssFeature>,
    /// Required features not supported by the target.
    pub unsupported: Vec<CssFeature>,
}

impl BrowserSupportReport {
    /// Returns whether all required features are supported by the target.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.unsupported.is_empty()
    }
}

/// A stable ordering key assigned before CSS serialization.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct OrderKey {
    layer: u16,
    variant: u16,
    utility: u16,
    modifier: u16,
    tie_breaker: u64,
}

impl OrderKey {
    /// Creates an ordering key from explicit semantic ordering components.
    #[must_use]
    pub const fn new(layer: u16, variant: u16, utility: u16, tie_breaker: u64) -> Self {
        Self { layer, variant, utility, modifier: 0, tie_breaker }
    }

    /// Returns the layer ordering component.
    #[must_use]
    pub const fn layer(self) -> u16 {
        self.layer
    }

    /// Returns the variant ordering component.
    #[must_use]
    pub const fn variant(self) -> u16 {
        self.variant
    }

    /// Returns the utility ordering component.
    #[must_use]
    pub const fn utility(self) -> u16 {
        self.utility
    }

    /// Returns the modifier ordering component.
    #[must_use]
    pub const fn modifier(self) -> u16 {
        self.modifier
    }

    /// Returns a copy with an explicit modifier ordering component.
    #[must_use]
    pub const fn with_modifier(mut self, modifier: u16) -> Self {
        self.modifier = modifier;
        self
    }

    /// Returns the deterministic tie-breaker component.
    #[must_use]
    pub const fn tie_breaker(self) -> u64 {
        self.tie_breaker
    }

    /// Converts this legacy compact key into the wide vNext ordering model.
    #[must_use]
    pub const fn rule_order(self) -> RuleOrderKey {
        RuleOrderKey {
            layer: self.layer,
            variant_order: self.variant as u32,
            utility_order: self.utility as u32,
            modifier_order: self.modifier,
            candidate_tiebreaker: self.tie_breaker,
        }
    }
}

/// Wide semantic ordering key used by vNext introspection and serializers.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuleOrderKey {
    /// Cascade layer order.
    pub layer: u16,
    /// Variant order.
    pub variant_order: u32,
    /// Utility order.
    pub utility_order: u32,
    /// Modifier order.
    pub modifier_order: u16,
    /// Stable candidate tie-breaker.
    pub candidate_tiebreaker: u64,
}

impl RuleOrderKey {
    /// Creates a wide semantic ordering key.
    #[must_use]
    pub const fn new(
        layer: u16,
        variant_order: u32,
        utility_order: u32,
        modifier_order: u16,
        candidate_tiebreaker: u64,
    ) -> Self {
        Self { layer, variant_order, utility_order, modifier_order, candidate_tiebreaker }
    }

    /// Converts this key to the compact CSS IR key when all ranks fit in `u16`.
    #[must_use]
    pub const fn compact(self) -> Option<OrderKey> {
        if self.variant_order > u16::MAX as u32 || self.utility_order > u16::MAX as u32 {
            return None;
        }
        Some(OrderKey {
            layer: self.layer,
            variant: self.variant_order as u16,
            utility: self.utility_order as u16,
            modifier: self.modifier_order,
            tie_breaker: self.candidate_tiebreaker,
        })
    }
}

/// A custom-property dependency used by a CSS declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CssDependency {
    /// Referenced custom-property name, including its leading `--`.
    pub name: String,
}

impl CssDependency {
    /// Creates a dependency reference for a custom property.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// One CSS declaration in a style rule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssDeclaration {
    property: String,
    value: String,
    important: bool,
    dependencies: Vec<CssDependency>,
}

impl CssDeclaration {
    /// Creates a declaration from a property and value.
    #[must_use]
    pub fn new(property: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            property: property.into(),
            value: value.into(),
            important: false,
            dependencies: Vec::new(),
        }
    }

    /// Marks or unmarks the declaration as important.
    #[must_use]
    pub fn with_important(mut self, important: bool) -> Self {
        self.important = important;
        self
    }

    /// Adds a custom-property dependency to this declaration.
    #[must_use]
    pub fn with_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.dependencies.push(CssDependency::new(dependency));
        self
    }

    /// Returns the declaration property.
    #[must_use]
    pub fn property(&self) -> &str {
        &self.property
    }

    /// Returns the declaration value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Returns whether the declaration is marked important.
    #[must_use]
    pub const fn is_important(&self) -> bool {
        self.important
    }

    /// Returns custom-property dependencies in declaration order.
    #[must_use]
    pub fn dependencies(&self) -> &[CssDependency] {
        &self.dependencies
    }

    /// Returns browser-sensitive features used by this declaration.
    #[must_use]
    pub fn required_features(&self) -> Vec<CssFeature> {
        declaration_features(&self.property, &self.value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuleKind {
    Style { selector: String, declarations: Vec<CssDeclaration> },
    AtRule { name: String, prelude: String, children: Vec<CssRule> },
}

/// A CSS rule with an explicit ordering key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssRule {
    order: OrderKey,
    kind: RuleKind,
}

impl CssRule {
    /// Creates a style rule.
    #[must_use]
    pub fn style(
        order: OrderKey,
        selector: impl Into<String>,
        declarations: Vec<CssDeclaration>,
    ) -> Self {
        Self { order, kind: RuleKind::Style { selector: selector.into(), declarations } }
    }

    /// Creates an at-rule containing nested rules.
    #[must_use]
    pub fn at_rule(
        order: OrderKey,
        name: impl Into<String>,
        prelude: impl Into<String>,
        children: Vec<Self>,
    ) -> Self {
        Self {
            order,
            kind: RuleKind::AtRule { name: name.into(), prelude: prelude.into(), children },
        }
    }

    /// Returns this rule's explicit ordering key.
    #[must_use]
    pub const fn order(&self) -> OrderKey {
        self.order
    }

    /// Returns the style selector when this is a style rule.
    #[must_use]
    pub fn selector(&self) -> Option<&str> {
        match &self.kind {
            RuleKind::Style { selector, .. } => Some(selector),
            RuleKind::AtRule { .. } => None,
        }
    }

    /// Returns the declarations when this is a style rule.
    #[must_use]
    pub fn declarations(&self) -> Option<&[CssDeclaration]> {
        match &self.kind {
            RuleKind::Style { declarations, .. } => Some(declarations),
            RuleKind::AtRule { .. } => None,
        }
    }

    /// Returns nested rules when this is an at-rule.
    #[must_use]
    pub fn children(&self) -> Option<&[CssRule]> {
        match &self.kind {
            RuleKind::Style { .. } => None,
            RuleKind::AtRule { children, .. } => Some(children),
        }
    }

    /// Returns browser-sensitive features used by this rule and its descendants.
    #[must_use]
    pub fn required_features(&self) -> Vec<CssFeature> {
        let mut features = Vec::new();
        match &self.kind {
            RuleKind::Style { declarations, .. } => {
                for declaration in declarations {
                    features.extend(declaration.required_features());
                }
            }
            RuleKind::AtRule { children, .. } => {
                for child in children {
                    features.extend(child.required_features());
                }
            }
        }
        features.sort_unstable();
        features.dedup();
        features
    }

    /// Analyzes this rule and its descendants against a browser target.
    #[must_use]
    pub fn browser_support(&self, target: BrowserTarget) -> BrowserSupportReport {
        let required = self.required_features();
        let unsupported =
            required.iter().copied().filter(|feature| !target.supports(*feature)).collect();
        BrowserSupportReport { target, required, unsupported }
    }

    /// Creates a copy of a style rule with a new selector.
    #[must_use]
    pub fn with_selector(&self, selector: impl Into<String>) -> Option<Self> {
        match &self.kind {
            RuleKind::Style { declarations, .. } => {
                Some(Self::style(self.order, selector, declarations.clone()))
            }
            RuleKind::AtRule { .. } => None,
        }
    }

    /// Creates a copy with a different explicit ordering key.
    #[must_use]
    pub fn with_order(&self, order: OrderKey) -> Self {
        Self { order, kind: self.kind.clone() }
    }

    /// Returns a copy with every declaration marked important or unimportant.
    #[must_use]
    pub fn with_important(&self, important: bool) -> Self {
        Self { order: self.order, kind: map_important(&self.kind, important) }
    }

    /// Maps every nested style selector while preserving the rule structure.
    #[must_use]
    pub fn map_selectors<F>(&self, mut mapper: F) -> Self
    where
        F: FnMut(&str) -> String,
    {
        Self { order: self.order, kind: map_rule_kind(&self.kind, &mut mapper) }
    }
}

/// A collection of CSS rules that serializes in deterministic order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssDocument {
    rules: Vec<CssRule>,
}

impl CssDocument {
    /// Creates an empty CSS document.
    #[must_use]
    pub const fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Adds a rule to the document.
    pub fn push(&mut self, rule: CssRule) {
        self.rules.push(rule);
    }

    /// Returns the rules in insertion order.
    #[must_use]
    pub fn rules(&self) -> &[CssRule] {
        &self.rules
    }

    /// Returns whether the document has no rules.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Returns browser-sensitive features used by the document.
    #[must_use]
    pub fn required_features(&self) -> Vec<CssFeature> {
        let mut features =
            self.rules.iter().flat_map(CssRule::required_features).collect::<Vec<_>>();
        features.sort_unstable();
        features.dedup();
        features
    }

    /// Analyzes the document against a deterministic browser target.
    #[must_use]
    pub fn browser_support(&self, target: BrowserTarget) -> BrowserSupportReport {
        let required = self.required_features();
        let unsupported =
            required.iter().copied().filter(|feature| !target.supports(*feature)).collect();
        BrowserSupportReport { target, required, unsupported }
    }

    /// Serializes the document using deterministic ordering.
    #[must_use]
    pub fn to_css(&self, mode: CssSerializationMode) -> String {
        render_rules(&self.rules, mode, 0)
    }
}

fn declaration_features(property: &str, value: &str) -> Vec<CssFeature> {
    let lower_property = property.to_ascii_lowercase();
    let lower_value = value.to_ascii_lowercase();
    let mut features = Vec::new();
    if lower_value.contains("color-mix(") {
        features.push(CssFeature::ColorMix);
    }
    if lower_value.contains("light-dark(") {
        features.push(CssFeature::LightDark);
    }
    if lower_value.contains("oklch(") {
        features.push(CssFeature::Oklch);
    }
    if lower_property == "content-visibility" {
        features.push(CssFeature::ContentVisibility);
    }
    if lower_property == "field-sizing" {
        features.push(CssFeature::FieldSizing);
    }
    if lower_property == "backdrop-filter" || lower_value.contains("backdrop-filter(") {
        features.push(CssFeature::BackdropFilter);
    }
    if lower_property.starts_with("mask-") || lower_property == "mask" {
        features.push(CssFeature::Masking);
    }
    features
}

fn map_rule_kind<F>(kind: &RuleKind, mapper: &mut F) -> RuleKind
where
    F: FnMut(&str) -> String,
{
    match kind {
        RuleKind::Style { selector, declarations } => {
            RuleKind::Style { selector: mapper(selector), declarations: declarations.clone() }
        }
        RuleKind::AtRule { name, prelude, children } => RuleKind::AtRule {
            name: name.clone(),
            prelude: prelude.clone(),
            children: children
                .iter()
                .map(|child| CssRule {
                    order: child.order,
                    kind: map_rule_kind(&child.kind, mapper),
                })
                .collect(),
        },
    }
}

fn map_important(kind: &RuleKind, important: bool) -> RuleKind {
    match kind {
        RuleKind::Style { selector, declarations } => RuleKind::Style {
            selector: selector.clone(),
            declarations: declarations
                .iter()
                .cloned()
                .map(|declaration| declaration.with_important(important))
                .collect(),
        },
        RuleKind::AtRule { name, prelude, children } => RuleKind::AtRule {
            name: name.clone(),
            prelude: prelude.clone(),
            children: children.iter().map(|child| child.with_important(important)).collect(),
        },
    }
}

fn render_rules(rules: &[CssRule], mode: CssSerializationMode, indent: usize) -> String {
    let mut ordered = rules.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| compare_rules(left, right, mode));

    let separator = match mode {
        CssSerializationMode::Pretty => "\n",
        CssSerializationMode::Minified => "",
    };

    ordered.iter().map(|rule| render_rule(rule, mode, indent)).collect::<Vec<_>>().join(separator)
}

fn compare_rules(left: &CssRule, right: &CssRule, mode: CssSerializationMode) -> Ordering {
    left.order
        .cmp(&right.order)
        .then_with(|| render_rule(left, mode, 0).cmp(&render_rule(right, mode, 0)))
}

fn render_rule(rule: &CssRule, mode: CssSerializationMode, indent: usize) -> String {
    match &rule.kind {
        RuleKind::Style { selector, declarations } => {
            render_style_rule(selector, declarations, mode, indent)
        }
        RuleKind::AtRule { name, prelude, children } => {
            let prefix =
                if prelude.is_empty() { format!("@{name}") } else { format!("@{name} {prelude}") };
            let body = render_rules(children, mode, indent + 2);

            match mode {
                CssSerializationMode::Pretty => {
                    if body.is_empty() {
                        format!("{}{} {{}}", indentation(indent), prefix)
                    } else {
                        format!(
                            "{}{} {{\n{}\n{}}}",
                            indentation(indent),
                            prefix,
                            body,
                            indentation(indent)
                        )
                    }
                }
                CssSerializationMode::Minified => format!("{prefix}{{{body}}}"),
            }
        }
    }
}

fn render_style_rule(
    selector: &str,
    declarations: &[CssDeclaration],
    mode: CssSerializationMode,
    indent: usize,
) -> String {
    match mode {
        CssSerializationMode::Pretty => {
            let mut output = format!("{}{} {{", indentation(indent), selector);
            if declarations.is_empty() {
                output.push('}');
                return output;
            }

            output.push('\n');
            for declaration in declarations {
                output.push_str(&indentation(indent + 2));
                output.push_str(declaration.property());
                output.push_str(": ");
                output.push_str(declaration.value());
                if declaration.is_important() {
                    output.push_str(" !important");
                }
                output.push_str(";\n");
            }
            output.push_str(&indentation(indent));
            output.push('}');
            output
        }
        CssSerializationMode::Minified => {
            let mut output = format!("{selector}{{");
            for declaration in declarations {
                output.push_str(declaration.property());
                output.push(':');
                output.push_str(declaration.value());
                if declaration.is_important() {
                    output.push_str("!important");
                }
                output.push(';');
            }
            output.push('}');
            output
        }
    }
}

fn indentation(width: usize) -> String {
    " ".repeat(width)
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserTarget, CssDeclaration, CssDocument, CssFeature, CssRule, CssSerializationMode,
        OrderKey, RuleOrderKey,
    };

    fn rule(order: OrderKey, selector: &str, property: &str, value: &str) -> CssRule {
        CssRule::style(order, selector, vec![CssDeclaration::new(property, value)])
    }

    #[test]
    fn serialization_is_independent_of_insertion_order() {
        let first = rule(OrderKey::new(0, 0, 2, 0), ".p-4", "padding", "1rem");
        let second = rule(OrderKey::new(0, 0, 1, 0), ".flex", "display", "flex");

        let mut left = CssDocument::new();
        left.push(first.clone());
        left.push(second.clone());

        let mut right = CssDocument::new();
        right.push(second);
        right.push(first);

        assert_eq!(
            left.to_css(CssSerializationMode::Minified),
            right.to_css(CssSerializationMode::Minified)
        );
        assert_eq!(
            left.to_css(CssSerializationMode::Minified),
            ".flex{display:flex;}.p-4{padding:1rem;}"
        );
    }

    #[test]
    fn pretty_serialization_supports_nested_at_rules_and_important_values() {
        let style = CssRule::style(
            OrderKey::default(),
            ".p-4",
            vec![CssDeclaration::new("padding", "1rem").with_important(true)],
        );
        let media =
            CssRule::at_rule(OrderKey::new(0, 1, 0, 0), "media", "(min-width: 768px)", vec![style]);
        let mut document = CssDocument::new();
        document.push(media);

        assert_eq!(
            document.to_css(CssSerializationMode::Pretty),
            "@media (min-width: 768px) {\n  .p-4 {\n    padding: 1rem !important;\n  }\n}"
        );
    }

    #[test]
    fn wide_order_keys_and_dependencies_round_trip() {
        let key = RuleOrderKey::new(1, 2, 3, 4, 5);
        assert_eq!(key.compact().expect("ranks fit").rule_order(), key);
        let declaration =
            CssDeclaration::new("background", "var(--stops)").with_dependency("--stops");
        assert_eq!(declaration.dependencies()[0].name, "--stops");
    }

    #[test]
    fn browser_support_analysis_is_recursive_and_deterministic() {
        let style = CssRule::style(
            OrderKey::default(),
            ".demo",
            vec![
                CssDeclaration::new("color", "color-mix(in srgb, red 50%, transparent)"),
                CssDeclaration::new("field-sizing", "content"),
            ],
        );
        let media = CssRule::at_rule(OrderKey::new(0, 1, 0, 0), "media", "print", vec![style]);
        let mut document = CssDocument::new();
        document.push(media);

        let report = document.browser_support(BrowserTarget::Safari15);
        assert_eq!(report.required, vec![CssFeature::ColorMix, CssFeature::FieldSizing]);
        assert_eq!(report.unsupported, report.required);
        assert!(!report.is_supported());
        assert_eq!(BrowserTarget::parse("safari-15"), Some(BrowserTarget::Safari15));
    }

    #[test]
    fn safari15_support_matches_safari_15_0_floors() {
        // Floors verified against MDN browser-compat-data for Safari 15.0
        // (September 2021), the 15.x series floor: oklch()/masking 15.4,
        // color-mix() 16.2, light-dark() 17.5, content-visibility and
        // unprefixed backdrop-filter 18.0, field-sizing 26.2.
        for feature in [
            CssFeature::ColorMix,
            CssFeature::LightDark,
            CssFeature::Oklch,
            CssFeature::ContentVisibility,
            CssFeature::FieldSizing,
            CssFeature::BackdropFilter,
            CssFeature::Masking,
        ] {
            assert!(!BrowserTarget::Safari15.supports(feature), "{feature:?} is newer than 15.0");
            assert!(BrowserTarget::Modern.supports(feature));
            assert!(BrowserTarget::Evergreen.supports(feature));
            assert!(!BrowserTarget::Legacy.supports(feature));
        }
    }
}
