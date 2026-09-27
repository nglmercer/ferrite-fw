//! Registry and semantic lowering for the initial utility set.
//!
//! Utility lowering produces declarations and an escaped class selector. Variants are applied by
//! the separate `utilitycss-variants` crate.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{collections::BTreeMap, error::Error, fmt};

use serde::Serialize;
use utilitycss_css_ir::{CssDeclaration, CssRule, OrderKey};
use utilitycss_diagnostics::{Diagnostic, DiagnosticCode};
use utilitycss_span::{SourceId, Span};
use utilitycss_syntax::{decode_arbitrary, CandidateAst, ValueAst};
use utilitycss_theme::Theme;

/// Stable semantic identifier for a utility family.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UtilityId(String);

impl UtilityId {
    /// Creates an identifier from a canonical family name.
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

fn register_extended_utilities(registry: &mut UtilityRegistry) {
    for (name, property, value, order) in [
        ("inline-block", "display", "inline-block", 10),
        ("table", "display", "table", 10),
        ("inline-table", "display", "inline-table", 10),
        ("table-caption", "display", "table-caption", 10),
        ("table-cell", "display", "table-cell", 10),
        ("table-column", "display", "table-column", 10),
        ("table-column-group", "display", "table-column-group", 10),
        ("table-footer-group", "display", "table-footer-group", 10),
        ("table-header-group", "display", "table-header-group", 10),
        ("table-row-group", "display", "table-row-group", 10),
        ("table-row", "display", "table-row", 10),
        ("flow-root", "display", "flow-root", 10),
        ("list-item", "display", "list-item", 10),
        ("contents", "display", "contents", 10),
        ("inline-flex", "display", "inline-flex", 10),
        ("inline-grid", "display", "inline-grid", 10),
        ("sr-only", "position", "absolute", 12),
        ("not-sr-only", "position", "static", 12),
        ("static", "position", "static", 13),
        ("fixed", "position", "fixed", 13),
        ("absolute", "position", "absolute", 13),
        ("relative", "position", "relative", 13),
        ("sticky", "position", "sticky", 13),
        ("visible", "visibility", "visible", 14),
        ("invisible", "visibility", "hidden", 14),
        ("collapse", "visibility", "collapse", 14),
        ("isolate", "isolation", "isolate", 15),
        ("isolation-auto", "isolation", "auto", 15),
        ("float-right", "float", "right", 16),
        ("float-left", "float", "left", 16),
        ("float-none", "float", "none", 16),
        ("clear-left", "clear", "left", 17),
        ("clear-right", "clear", "right", 17),
        ("clear-both", "clear", "both", 17),
        ("clear-none", "clear", "none", 17),
        ("box-border", "box-sizing", "border-box", 18),
        ("box-content", "box-sizing", "content-box", 18),
        ("object-contain", "object-fit", "contain", 19),
        ("object-cover", "object-fit", "cover", 19),
        ("object-fill", "object-fit", "fill", 19),
        ("object-none", "object-fit", "none", 19),
        ("object-scale-down", "object-fit", "scale-down", 19),
        ("object-bottom", "object-position", "bottom", 20),
        ("object-center", "object-position", "center", 20),
        ("object-left", "object-position", "left", 20),
        ("object-right", "object-position", "right", 20),
        ("object-top", "object-position", "top", 20),
        ("overflow-auto", "overflow", "auto", 24),
        ("overflow-hidden", "overflow", "hidden", 24),
        ("overflow-clip", "overflow", "clip", 24),
        ("overflow-visible", "overflow", "visible", 24),
        ("overflow-scroll", "overflow", "scroll", 24),
        ("overscroll-auto", "overscroll-behavior", "auto", 25),
        ("overscroll-contain", "overscroll-behavior", "contain", 25),
        ("overscroll-none", "overscroll-behavior", "none", 25),
        ("aspect-auto", "aspect-ratio", "auto", 26),
        ("aspect-square", "aspect-ratio", "1 / 1", 26),
        ("aspect-video", "aspect-ratio", "16 / 9", 26),
        ("flex-row", "flex-direction", "row", 70),
        ("flex-row-reverse", "flex-direction", "row-reverse", 70),
        ("flex-col", "flex-direction", "column", 70),
        ("flex-col-reverse", "flex-direction", "column-reverse", 70),
        ("flex-wrap", "flex-wrap", "wrap", 71),
        ("flex-wrap-reverse", "flex-wrap", "wrap-reverse", 71),
        ("flex-nowrap", "flex-wrap", "nowrap", 71),
        ("flex-1", "flex", "1 1 0%", 72),
        ("flex-auto", "flex", "1 1 auto", 72),
        ("flex-initial", "flex", "0 1 auto", 72),
        ("flex-none", "flex", "none", 72),
        ("grow", "flex-grow", "1", 73),
        ("shrink", "flex-shrink", "1", 74),
        ("grid-flow-row", "grid-auto-flow", "row", 75),
        ("grid-flow-col", "grid-auto-flow", "column", 75),
        ("grid-flow-dense", "grid-auto-flow", "dense", 75),
        ("grid-flow-row-dense", "grid-auto-flow", "row dense", 75),
        ("grid-flow-col-dense", "grid-auto-flow", "column dense", 75),
        ("table-auto", "table-layout", "auto", 110),
        ("table-fixed", "table-layout", "fixed", 110),
        ("border-collapse", "border-collapse", "collapse", 111),
        ("border-separate", "border-collapse", "separate", 111),
        ("caption-top", "caption-side", "top", 112),
        ("caption-bottom", "caption-side", "bottom", 112),
        ("italic", "font-style", "italic", 120),
        ("not-italic", "font-style", "normal", 120),
        ("antialiased", "-webkit-font-smoothing", "antialiased", 121),
        ("subpixel-antialiased", "-webkit-font-smoothing", "auto", 121),
        ("uppercase", "text-transform", "uppercase", 130),
        ("lowercase", "text-transform", "lowercase", 130),
        ("capitalize", "text-transform", "capitalize", 130),
        ("normal-case", "text-transform", "none", 130),
        ("truncate", "overflow", "hidden", 134),
        ("text-ellipsis", "text-overflow", "ellipsis", 134),
        ("text-clip", "text-overflow", "clip", 134),
        ("whitespace-normal", "white-space", "normal", 135),
        ("whitespace-nowrap", "white-space", "nowrap", 135),
        ("whitespace-pre", "white-space", "pre", 135),
        ("whitespace-pre-line", "white-space", "pre-line", 135),
        ("whitespace-pre-wrap", "white-space", "pre-wrap", 135),
        ("break-normal", "overflow-wrap", "normal", 136),
        ("break-words", "overflow-wrap", "break-word", 136),
        ("break-all", "word-break", "break-all", 136),
        ("break-keep", "word-break", "keep-all", 136),
        ("underline", "text-decoration-line", "underline", 140),
        ("overline", "text-decoration-line", "overline", 140),
        ("line-through", "text-decoration-line", "line-through", 140),
        ("no-underline", "text-decoration-line", "none", 140),
        ("decoration-solid", "text-decoration-style", "solid", 141),
        ("decoration-double", "text-decoration-style", "double", 141),
        ("decoration-dotted", "text-decoration-style", "dotted", 141),
        ("decoration-dashed", "text-decoration-style", "dashed", 141),
        ("decoration-wavy", "text-decoration-style", "wavy", 141),
        ("bg-fixed", "background-attachment", "fixed", 150),
        ("bg-local", "background-attachment", "local", 150),
        ("bg-scroll", "background-attachment", "scroll", 150),
        ("bg-clip-border", "background-clip", "border-box", 151),
        ("bg-clip-padding", "background-clip", "padding-box", 151),
        ("bg-clip-content", "background-clip", "content-box", 151),
        ("bg-clip-text", "background-clip", "text", 151),
        ("bg-origin-border", "background-origin", "border-box", 152),
        ("bg-origin-padding", "background-origin", "padding-box", 152),
        ("bg-origin-content", "background-origin", "content-box", 152),
        ("bg-no-repeat", "background-repeat", "no-repeat", 153),
        ("bg-repeat", "background-repeat", "repeat", 153),
        ("bg-repeat-x", "background-repeat", "repeat-x", 153),
        ("bg-repeat-y", "background-repeat", "repeat-y", 153),
        ("bg-cover", "background-size", "cover", 154),
        ("bg-contain", "background-size", "contain", 154),
        ("bg-auto", "background-size", "auto", 154),
        ("bg-none", "background-image", "none", 155),
        ("bg-bottom", "background-position", "bottom", 149),
        ("bg-center", "background-position", "center", 149),
        ("bg-left", "background-position", "left", 149),
        ("bg-left-bottom", "background-position", "left bottom", 149),
        ("bg-left-top", "background-position", "left top", 149),
        ("bg-right", "background-position", "right", 149),
        ("bg-right-bottom", "background-position", "right bottom", 149),
        ("bg-right-top", "background-position", "right top", 149),
        ("bg-top", "background-position", "top", 149),
        ("border-solid", "border-style", "solid", 160),
        ("border-dashed", "border-style", "dashed", 160),
        ("border-dotted", "border-style", "dotted", 160),
        ("border-double", "border-style", "double", 160),
        ("border-hidden", "border-style", "hidden", 160),
        ("border-none", "border-style", "none", 160),
        ("outline-none", "outline", "2px solid transparent", 170),
        ("outline", "outline-style", "solid", 170),
        ("shadow-none", "box-shadow", "none", 180),
        ("shadow", "box-shadow", "0 1px 3px 0 rgb(0 0 0 / 0.1)", 180),
        ("shadow-sm", "box-shadow", "0 1px 2px 0 rgb(0 0 0 / 0.05)", 180),
        ("shadow-md", "box-shadow", "0 4px 6px -1px rgb(0 0 0 / 0.1)", 180),
        ("shadow-lg", "box-shadow", "0 10px 15px -3px rgb(0 0 0 / 0.1)", 180),
        ("shadow-xl", "box-shadow", "0 20px 25px -5px rgb(0 0 0 / 0.1)", 180),
        ("shadow-2xl", "box-shadow", "0 25px 50px -12px rgb(0 0 0 / 0.25)", 180),
        ("shadow-inner", "box-shadow", "inset 0 2px 4px 0 rgb(0 0 0 / 0.05)", 180),
        ("blur-none", "filter", "blur(0)", 181),
        ("filter-none", "filter", "none", 181),
        ("backdrop-filter-none", "backdrop-filter", "none", 182),
        ("transform-none", "transform", "none", 190),
        ("transform-flat", "transform-style", "flat", 191),
        ("transform-3d", "transform-style", "preserve-3d", 191),
        ("transition-none", "transition-property", "none", 200),
        ("transition-all", "transition-property", "all", 200),
        ("transition", "transition-property", "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, filter, backdrop-filter", 200),
        ("ease-linear", "transition-timing-function", "linear", 202),
        ("ease-in", "transition-timing-function", "cubic-bezier(0.4, 0, 1, 1)", 202),
        ("ease-out", "transition-timing-function", "cubic-bezier(0, 0, 0.2, 1)", 202),
        ("ease-in-out", "transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)", 202),
        ("animate-none", "animation", "none", 210),
        ("pointer-events-none", "pointer-events", "none", 220),
        ("pointer-events-auto", "pointer-events", "auto", 220),
        ("select-none", "user-select", "none", 221),
        ("select-text", "user-select", "text", 221),
        ("select-all", "user-select", "all", 221),
        ("select-auto", "user-select", "auto", 221),
        ("resize-none", "resize", "none", 222),
        ("resize-y", "resize", "vertical", 222),
        ("resize-x", "resize", "horizontal", 222),
        ("resize", "resize", "both", 222),
        ("scroll-auto", "scroll-behavior", "auto", 223),
        ("scroll-smooth", "scroll-behavior", "smooth", 223),
        ("touch-auto", "touch-action", "auto", 224),
        ("touch-none", "touch-action", "none", 224),
        ("touch-pan-x", "touch-action", "pan-x", 224),
        ("touch-pan-y", "touch-action", "pan-y", 224),
        ("appearance-none", "appearance", "none", 225),
        ("appearance-auto", "appearance", "auto", 225),
        ("bg-inherit", "background-color", "inherit", 40),
        ("bg-current", "background-color", "currentColor", 40),
        ("text-inherit", "color", "inherit", 41),
        ("text-current", "color", "currentColor", 41),
        ("text-transparent", "color", "transparent", 41),
        ("border-transparent", "border-color", "transparent", 42),
        ("border-current", "border-color", "currentColor", 42),
        ("content-none", "content", "none", 240),
        ("content-auto", "content-visibility", "auto", 241),
        ("contain-none", "contain", "none", 242),
        ("contain-content", "contain", "content", 242),
        ("contain-strict", "contain", "strict", 242),
        ("field-sizing-content", "field-sizing", "content", 243),
        ("field-sizing-fixed", "field-sizing", "fixed", 243),
        ("text-xs", "font-size", "0.75rem", 124),
        ("text-sm", "font-size", "0.875rem", 124),
        ("text-base", "font-size", "1rem", 124),
        ("text-lg", "font-size", "1.125rem", 124),
        ("text-xl", "font-size", "1.25rem", 124),
        ("text-2xl", "font-size", "1.5rem", 124),
        ("text-3xl", "font-size", "1.875rem", 124),
        ("text-4xl", "font-size", "2.25rem", 124),
        ("text-5xl", "font-size", "3rem", 124),
        ("text-6xl", "font-size", "3.75rem", 124),
        ("text-7xl", "font-size", "4.5rem", 124),
        ("text-8xl", "font-size", "6rem", 124),
        ("text-9xl", "font-size", "8rem", 124),
    ] {
        registry.register(name, UtilityDefinition::static_declaration(property, value, order));
    }

    for (name, property, namespace, negative, order) in [
        ("aspect", "aspect-ratio", ValueNamespace::Raw, false, 26),
        ("columns", "columns", ValueNamespace::Number, false, 27),
        ("break-after", "break-after", ValueNamespace::Raw, false, 28),
        ("break-before", "break-before", ValueNamespace::Raw, false, 28),
        ("break-inside", "break-inside", ValueNamespace::Raw, false, 28),
        ("object-position", "object-position", ValueNamespace::Raw, false, 29),
        ("overflow-x", "overflow-x", ValueNamespace::Raw, false, 24),
        ("overflow-y", "overflow-y", ValueNamespace::Raw, false, 24),
        ("overscroll-x", "overscroll-behavior-x", ValueNamespace::Raw, false, 25),
        ("overscroll-y", "overscroll-behavior-y", ValueNamespace::Raw, false, 25),
        ("inset", "inset", ValueNamespace::Spacing, true, 31),
        ("inset-x", "inset-inline", ValueNamespace::Spacing, true, 31),
        ("inset-y", "inset-block", ValueNamespace::Spacing, true, 31),
        ("top", "top", ValueNamespace::Spacing, true, 31),
        ("right", "right", ValueNamespace::Spacing, true, 31),
        ("bottom", "bottom", ValueNamespace::Spacing, true, 31),
        ("left", "left", ValueNamespace::Spacing, true, 31),
        ("start", "inset-inline-start", ValueNamespace::Spacing, true, 31),
        ("end", "inset-inline-end", ValueNamespace::Spacing, true, 31),
        ("basis", "flex-basis", ValueNamespace::Width, false, 76),
        ("order", "order", ValueNamespace::Integer, false, 77),
        ("z", "z-index", ValueNamespace::ZIndex, true, 78),
        ("grid-rows", "grid-template-rows", ValueNamespace::Raw, false, 78),
        ("grid-flow", "grid-auto-flow", ValueNamespace::Raw, false, 79),
        ("auto-cols", "grid-auto-columns", ValueNamespace::Raw, false, 80),
        ("auto-rows", "grid-auto-rows", ValueNamespace::Raw, false, 80),
        ("grid-column", "grid-column", ValueNamespace::Raw, false, 81),
        ("grid-row", "grid-row", ValueNamespace::Raw, false, 81),
        ("justify-items", "justify-items", ValueNamespace::Raw, false, 82),
        ("justify-self", "justify-self", ValueNamespace::Raw, false, 83),
        ("align-content", "align-content", ValueNamespace::Raw, false, 84),
        ("align-self", "align-self", ValueNamespace::Raw, false, 85),
        ("place-content", "place-content", ValueNamespace::Raw, false, 86),
        ("place-items", "place-items", ValueNamespace::Raw, false, 87),
        ("place-self", "place-self", ValueNamespace::Raw, false, 88),
        ("font", "font-family", ValueNamespace::FontFamily, false, 122),
        ("font-family", "font-family", ValueNamespace::FontFamily, false, 122),
        ("font-size", "font-size", ValueNamespace::FontSize, false, 124),
        ("leading", "line-height", ValueNamespace::LineHeight, false, 123),
        ("line-height", "line-height", ValueNamespace::LineHeight, false, 123),
        ("tracking", "letter-spacing", ValueNamespace::LetterSpacing, false, 124),
        ("letter-spacing", "letter-spacing", ValueNamespace::LetterSpacing, false, 124),
        ("font-stretch", "font-stretch", ValueNamespace::Raw, false, 125),
        ("font-variant", "font-variant", ValueNamespace::Raw, false, 125),
        ("text-indent", "text-indent", ValueNamespace::Spacing, false, 131),
        ("text-wrap", "text-wrap", ValueNamespace::Raw, false, 132),
        ("text-decoration", "text-decoration-color", ValueNamespace::Color, false, 142),
        ("text-underline", "text-underline-offset", ValueNamespace::Raw, false, 143),
        ("decoration", "text-decoration-thickness", ValueNamespace::Raw, false, 142),
        ("underline-offset", "text-underline-offset", ValueNamespace::Raw, false, 143),
        ("border-spacing", "border-spacing", ValueNamespace::Spacing, false, 162),
        ("opacity", "opacity", ValueNamespace::Percentage, false, 183),
        ("mix-blend", "mix-blend-mode", ValueNamespace::Raw, false, 184),
        ("bg-blend", "background-blend-mode", ValueNamespace::Raw, false, 185),
        ("translate", "translate", ValueNamespace::Spacing, true, 192),
        ("translate-x", "translate", ValueNamespace::Spacing, true, 192),
        ("translate-y", "translate", ValueNamespace::Spacing, true, 192),
        ("rotate", "rotate", ValueNamespace::Raw, true, 193),
        ("scale", "scale", ValueNamespace::Number, false, 194),
        ("skew", "skew", ValueNamespace::Raw, false, 195),
        ("delay", "transition-delay", ValueNamespace::Duration, false, 203),
        ("duration", "transition-duration", ValueNamespace::Duration, false, 204),
        ("will-change", "will-change", ValueNamespace::Raw, false, 226),
        ("scroll-m", "scroll-margin", ValueNamespace::Spacing, false, 227),
        ("scroll-p", "scroll-padding", ValueNamespace::Spacing, false, 228),
        ("fill", "fill", ValueNamespace::Color, false, 229),
        ("stroke", "stroke", ValueNamespace::Color, false, 229),
        ("accent", "accent-color", ValueNamespace::Color, false, 229),
        ("caret", "caret-color", ValueNamespace::Color, false, 229),
        ("from", "--tw-gradient-from", ValueNamespace::Color, false, 157),
        ("via", "--tw-gradient-stops", ValueNamespace::Color, false, 158),
        ("to", "--tw-gradient-to", ValueNamespace::Color, false, 159),
        ("mask-type", "mask-type", ValueNamespace::Raw, false, 230),
        ("mask-mode", "mask-mode", ValueNamespace::Raw, false, 230),
        ("mask-composite", "mask-composite", ValueNamespace::Raw, false, 230),
        ("mask-origin", "mask-origin", ValueNamespace::Raw, false, 230),
        ("mask-position", "mask-position", ValueNamespace::Raw, false, 230),
        ("mask-repeat", "mask-repeat", ValueNamespace::Raw, false, 230),
        ("mask-size", "mask-size", ValueNamespace::Raw, false, 230),
    ] {
        registry.register(
            name,
            UtilityDefinition::functional(property, namespace, true, negative, order),
        );
    }

    for (name, property, value, order) in [
        ("text-left", "text-align", "left", 125),
        ("text-center", "text-align", "center", 125),
        ("text-right", "text-align", "right", 125),
        ("text-justify", "text-align", "justify", 125),
        ("font-thin", "font-weight", "100", 126),
        ("font-extralight", "font-weight", "200", 126),
        ("font-light", "font-weight", "300", 126),
        ("font-normal", "font-weight", "400", 126),
        ("font-medium", "font-weight", "500", 126),
        ("font-semibold", "font-weight", "600", 126),
        ("font-bold", "font-weight", "700", 126),
        ("font-extrabold", "font-weight", "800", 126),
        ("font-black", "font-weight", "900", 126),
        ("font-sans", "font-family", "ui-sans-serif, system-ui, sans-serif", 122),
        ("font-serif", "font-family", "ui-serif, Georgia, serif", 122),
        ("font-mono", "font-family", "ui-monospace, SFMono-Regular, monospace", 122),
        ("leading-none", "line-height", "1", 123),
        ("leading-tight", "line-height", "1.25", 123),
        ("leading-snug", "line-height", "1.375", 123),
        ("leading-normal", "line-height", "1.5", 123),
        ("leading-relaxed", "line-height", "1.625", 123),
        ("leading-loose", "line-height", "2", 123),
    ] {
        registry.register(name, UtilityDefinition::static_declaration(property, value, order));
    }

    for (name, property, value, order) in [
        ("grid-cols-none", "grid-template-columns", "none", 62),
        ("grid-cols-subgrid", "grid-template-columns", "subgrid", 62),
        ("grid-rows-none", "grid-template-rows", "none", 78),
        ("grid-rows-subgrid", "grid-template-rows", "subgrid", 78),
        ("auto-cols-auto", "grid-auto-columns", "auto", 80),
        ("auto-cols-min", "grid-auto-columns", "min-content", 80),
        ("auto-cols-max", "grid-auto-columns", "max-content", 80),
        ("auto-cols-fr", "grid-auto-columns", "minmax(0, 1fr)", 80),
        ("auto-rows-auto", "grid-auto-rows", "auto", 80),
        ("auto-rows-min", "grid-auto-rows", "min-content", 80),
        ("auto-rows-max", "grid-auto-rows", "max-content", 80),
        ("auto-rows-fr", "grid-auto-rows", "minmax(0, 1fr)", 80),
        ("basis-auto", "flex-basis", "auto", 76),
        ("basis-full", "flex-basis", "100%", 76),
        ("items-start", "align-items", "flex-start", 60),
        ("items-end", "align-items", "flex-end", 60),
        ("items-center", "align-items", "center", 60),
        ("items-baseline", "align-items", "baseline", 60),
        ("items-stretch", "align-items", "stretch", 60),
        ("justify-normal", "justify-content", "normal", 61),
        ("justify-start", "justify-content", "flex-start", 61),
        ("justify-end", "justify-content", "flex-end", 61),
        ("justify-center", "justify-content", "center", 61),
        ("justify-between", "justify-content", "space-between", 61),
        ("justify-around", "justify-content", "space-around", 61),
        ("justify-evenly", "justify-content", "space-evenly", 61),
        ("justify-stretch", "justify-content", "stretch", 61),
        ("content-normal", "align-content", "normal", 63),
        ("content-center", "align-content", "center", 63),
        ("content-start", "align-content", "flex-start", 63),
        ("content-end", "align-content", "flex-end", 63),
        ("content-between", "align-content", "space-between", 63),
        ("content-around", "align-content", "space-around", 63),
        ("content-evenly", "align-content", "space-evenly", 63),
        ("content-baseline", "align-content", "baseline", 63),
        ("content-stretch", "align-content", "stretch", 63),
        ("fill-none", "fill", "none", 229),
        ("stroke-none", "stroke", "none", 229),
        ("stroke-0", "stroke-width", "0", 229),
        ("stroke-1", "stroke-width", "1", 229),
        ("stroke-2", "stroke-width", "2", 229),
        ("text-balance", "text-wrap", "balance", 132),
        ("text-pretty", "text-wrap", "pretty", 132),
        ("decoration-auto", "text-decoration-thickness", "auto", 142),
        ("decoration-from-font", "text-decoration-thickness", "from-font", 142),
        ("underline-offset-auto", "text-underline-offset", "auto", 143),
        ("list-none", "list-style-type", "none", 137),
        ("list-disc", "list-style-type", "disc", 137),
        ("list-decimal", "list-style-type", "decimal", 137),
        ("list-inside", "list-style-position", "inside", 138),
        ("list-outside", "list-style-position", "outside", 138),
        ("align-baseline", "vertical-align", "baseline", 139),
        ("align-top", "vertical-align", "top", 139),
        ("align-middle", "vertical-align", "middle", 139),
        ("align-bottom", "vertical-align", "bottom", 139),
        ("align-text-top", "vertical-align", "text-top", 139),
        ("align-text-bottom", "vertical-align", "text-bottom", 139),
        ("whitespace-break-spaces", "white-space", "break-spaces", 135),
        ("break-keep", "word-break", "keep-all", 136),
    ] {
        registry.register(name, UtilityDefinition::static_declaration(property, value, order));
    }

    registry.register(
        "bg-gradient-to-r",
        UtilityDefinition::composite(
            vec![(
                "background-image".to_owned(),
                "linear-gradient(to right, var(--tw-gradient-stops))".to_owned(),
            )],
            156,
        ),
    );
    registry.register(
        "bg-gradient-to-b",
        UtilityDefinition::composite(
            vec![(
                "background-image".to_owned(),
                "linear-gradient(to bottom, var(--tw-gradient-stops))".to_owned(),
            )],
            156,
        ),
    );
}

/// High-level semantic category used by tooling and generated documentation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum UtilityCategory {
    /// Layout, display, position, overflow, and containment.
    Layout,
    /// Flexbox and grid behavior.
    FlexGrid,
    /// Margins, padding, gaps, and dimensions.
    SpacingSizing,
    /// Fonts, text, lists, and content.
    Typography,
    /// Backgrounds, gradients, borders, and outlines.
    BackgroundBorder,
    /// Shadows, opacity, filters, masks, and compositing.
    Effects,
    /// Transforms, transitions, and animations.
    TransformsTransitions,
    /// Pointer, scrolling, selection, and accessibility behavior.
    Interactivity,
    /// Table and SVG presentation.
    TablesSvg,
    /// New or browser-sensitive CSS features.
    ModernCss,
    /// User-provided extension.
    Custom,
}

/// Stability annotation exposed in capability manifests.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Stability {
    /// Supported native behavior.
    #[default]
    Stable,
    /// Implemented but browser or language behavior may evolve.
    Experimental,
    /// Retained for migration and not recommended for new code.
    Deprecated,
    /// Only enabled by an explicit compatibility profile.
    CompatibilityOnly,
}

/// Policy for a utility's negative marker.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum NegativePolicy {
    /// A negative marker is rejected.
    Unsupported,
    /// A negative marker is lowered by negating the resolved value.
    Allowed,
}

/// Policy for bracketed arbitrary values.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ArbitraryPolicy {
    /// Bracketed values are rejected.
    Unsupported,
    /// Bracketed values are accepted after CSS safety validation.
    Allowed,
    /// Bracketed values are accepted and may carry a type hint.
    Typed,
}

/// Semantic kind of the value accepted after a utility modifier slash.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ModifierKind {
    /// The utility does not accept a modifier.
    Unsupported,
    /// The modifier is a numeric value.
    Numeric,
    /// The modifier is a percentage value.
    Percentage,
    /// The modifier resolves through a theme namespace.
    ThemeBacked,
    /// The modifier is arbitrary validated CSS.
    Arbitrary,
    /// The modifier has utility-specific semantics.
    UtilitySpecific,
}

/// Theme/value namespace consulted by a functional utility.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ValueNamespace {
    /// No theme lookup; values are validated CSS or keywords.
    Raw,
    /// Spacing tokens and numeric spacing scale values.
    Spacing,
    /// Color tokens.
    Color,
    /// Width tokens and spacing scale values.
    Width,
    /// Height tokens and spacing scale values.
    Height,
    /// Border-radius tokens.
    Radius,
    /// Font-family tokens.
    FontFamily,
    /// Font-size tokens.
    FontSize,
    /// Line-height tokens.
    LineHeight,
    /// Letter-spacing tokens.
    LetterSpacing,
    /// Box-shadow tokens.
    Shadow,
    /// Transition duration tokens.
    Duration,
    /// Easing tokens.
    Ease,
    /// Z-index tokens.
    ZIndex,
    /// Numeric values.
    Number,
    /// CSS length values.
    Length,
    /// Percentage values.
    Percentage,
    /// Integer values.
    Integer,
    /// A finite set of CSS keyword/value pairs.
    Keyword,
}

impl ValueNamespace {
    /// Returns the stable manifest name for this namespace.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Spacing => "spacing",
            Self::Color => "color",
            Self::Width => "width",
            Self::Height => "height",
            Self::Radius => "radius",
            Self::FontFamily => "font-family",
            Self::FontSize => "font-size",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::Shadow => "shadow",
            Self::Duration => "duration",
            Self::Ease => "ease",
            Self::ZIndex => "z-index",
            Self::Number => "number",
            Self::Length => "length",
            Self::Percentage => "percentage",
            Self::Integer => "integer",
            Self::Keyword => "keyword",
        }
    }
}

/// Schema for a utility's primary value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValueSchema {
    /// Value namespace or primitive type.
    pub namespace: ValueNamespace,
    /// Whether the value is required.
    pub required: bool,
    /// Whether arbitrary bracketed values are accepted.
    pub arbitrary: ArbitraryPolicy,
    /// Finite named values accepted directly by the utility, when applicable.
    pub allowed_values: Vec<String>,
    /// Short human-readable description.
    pub description: String,
}

impl ValueSchema {
    /// Creates a value schema.
    #[must_use]
    pub fn new(namespace: ValueNamespace, required: bool, arbitrary: ArbitraryPolicy) -> Self {
        Self {
            namespace,
            required,
            arbitrary,
            allowed_values: Vec::new(),
            description: String::new(),
        }
    }

    /// Adds the finite named values accepted by the schema.
    #[must_use]
    pub fn with_allowed_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_values = values.into_iter().map(Into::into).collect();
        self
    }

    /// Adds a description to the schema.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
}

/// Schema for a utility modifier after `/`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ModifierSchema {
    /// Whether the modifier may be omitted.
    pub supported: bool,
    /// Whether the modifier must be present for this utility family.
    pub required: bool,
    /// Accepted modifier value kind.
    pub namespace: ValueNamespace,
    /// Semantic kind of the modifier value.
    pub kind: ModifierKind,
    /// Whether arbitrary bracketed modifier values are accepted.
    pub arbitrary: ArbitraryPolicy,
    /// Short human-readable description.
    pub description: String,
}

impl ModifierSchema {
    /// Returns a schema for a utility with no modifier.
    #[must_use]
    pub fn unsupported() -> Self {
        Self {
            supported: false,
            required: false,
            namespace: ValueNamespace::Raw,
            kind: ModifierKind::Unsupported,
            arbitrary: ArbitraryPolicy::Unsupported,
            description: "Modifiers are not accepted by this utility family".to_owned(),
        }
    }

    /// Returns a schema for a supported modifier.
    #[must_use]
    pub fn optional(namespace: ValueNamespace, description: impl Into<String>) -> Self {
        let kind = modifier_kind_for_namespace(&namespace);
        Self {
            supported: true,
            required: false,
            namespace,
            kind,
            arbitrary: ArbitraryPolicy::Allowed,
            description: description.into(),
        }
    }

    /// Returns a schema for a required modifier with an explicit arbitrary-value policy.
    #[must_use]
    pub fn required(
        namespace: ValueNamespace,
        arbitrary: ArbitraryPolicy,
        description: impl Into<String>,
    ) -> Self {
        let kind = modifier_kind_for_namespace(&namespace);
        Self {
            supported: true,
            required: true,
            namespace,
            kind,
            arbitrary,
            description: description.into(),
        }
    }

    /// Returns a schema for a modifier with utility-specific semantics.
    #[must_use]
    pub fn utility_specific(description: impl Into<String>) -> Self {
        Self {
            supported: true,
            required: false,
            namespace: ValueNamespace::Raw,
            kind: ModifierKind::UtilitySpecific,
            arbitrary: ArbitraryPolicy::Allowed,
            description: description.into(),
        }
    }
}

fn modifier_kind_for_namespace(namespace: &ValueNamespace) -> ModifierKind {
    match namespace {
        ValueNamespace::Number | ValueNamespace::Integer | ValueNamespace::Length => {
            ModifierKind::Numeric
        }
        ValueNamespace::Percentage => ModifierKind::Percentage,
        ValueNamespace::Raw | ValueNamespace::Keyword => ModifierKind::Arbitrary,
        ValueNamespace::Spacing
        | ValueNamespace::Color
        | ValueNamespace::Width
        | ValueNamespace::Height
        | ValueNamespace::Radius
        | ValueNamespace::FontFamily
        | ValueNamespace::FontSize
        | ValueNamespace::LineHeight
        | ValueNamespace::LetterSpacing
        | ValueNamespace::Shadow
        | ValueNamespace::Duration
        | ValueNamespace::Ease
        | ValueNamespace::ZIndex => ModifierKind::ThemeBacked,
    }
}

/// A CSS property named in utility metadata.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CssProperty(String);

impl CssProperty {
    /// Creates a CSS property metadata value.
    #[must_use]
    pub fn new(property: impl Into<String>) -> Self {
        Self(property.into())
    }

    /// Returns the property name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A deterministic utility documentation example.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UtilityExample {
    /// Candidate text.
    pub candidate: String,
    /// Human-readable meaning.
    pub description: String,
}

/// Machine-readable metadata for one registered utility family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UtilityDescriptor {
    /// Stable semantic utility identifier.
    pub id: UtilityId,
    /// Canonical and alias names accepted by the registry.
    pub names: Vec<String>,
    /// Semantic category.
    pub category: UtilityCategory,
    /// Human-readable description.
    pub description: String,
    /// Primary value schema.
    pub value_schema: ValueSchema,
    /// Modifier schema.
    pub modifier_schema: ModifierSchema,
    /// Negative marker policy.
    pub negative_policy: NegativePolicy,
    /// Arbitrary value policy.
    pub arbitrary_policy: ArbitraryPolicy,
    /// Theme namespaces used by the utility.
    pub theme_namespaces: Vec<String>,
    /// CSS properties this family may emit.
    pub emitted_properties: Vec<CssProperty>,
    /// Custom-property dependencies emitted or consumed by this family.
    pub dependencies: Vec<String>,
    /// Stable ordering group.
    pub ordering_group: u32,
    /// Deterministic examples.
    pub examples: Vec<UtilityExample>,
    /// Documentation key.
    pub docs_key: String,
    /// Stability status.
    pub stability: Stability,
}

/// The edge or axis affected by a spacing utility.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SpacingEdge {
    /// All sides or the unqualified property.
    All,
    /// Horizontal sides.
    X,
    /// Vertical sides.
    Y,
    /// The top side.
    Top,
    /// The right side.
    Right,
    /// The bottom side.
    Bottom,
    /// The left side.
    Left,
}

/// A dimension affected by a sizing utility.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Dimension {
    /// The `width` property.
    Width,
    /// The `height` property.
    Height,
    /// The `min-width` property.
    MinWidth,
    /// The `max-width` property.
    MaxWidth,
    /// The `min-height` property.
    MinHeight,
    /// The `max-height` property.
    MaxHeight,
}

impl Dimension {
    fn property(self) -> &'static str {
        match self {
            Self::Width => "width",
            Self::Height => "height",
            Self::MinWidth => "min-width",
            Self::MaxWidth => "max-width",
            Self::MinHeight => "min-height",
            Self::MaxHeight => "max-height",
        }
    }
}

/// The color property affected by a color utility.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ColorKind {
    /// The `background-color` property.
    Background,
    /// The `color` property.
    Text,
    /// The `border-color` property.
    Border,
}

impl ColorKind {
    fn property(self) -> &'static str {
        match self {
            Self::Background => "background-color",
            Self::Text => "color",
            Self::Border => "border-color",
        }
    }
}

/// A semantic utility definition registered under a family name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UtilityDefinition {
    /// A fixed property/value pair such as `flex` or `hidden`.
    Static {
        /// CSS property name.
        property: String,
        /// CSS property value.
        value: String,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A fixed multi-declaration utility such as a gradient or transform preset.
    Composite {
        /// CSS declarations emitted in their authored order.
        declarations: Vec<(String, String)>,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A padding or margin utility.
    Spacing {
        /// Whether the utility writes padding or margin.
        margin: bool,
        /// Sides affected by the utility.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A gap utility.
    Gap {
        /// Sides/axes affected by the gap utility.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A width or height utility.
    Size {
        /// Dimension affected by the utility.
        dimension: Dimension,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A color utility.
    Color {
        /// Color property affected by the utility.
        kind: ColorKind,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A border-radius utility.
    Radius {
        /// Stable utility ordering rank.
        order: u16,
    },
    /// An `align-items` utility.
    AlignItems {
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A `justify-content` utility.
    JustifyContent {
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A `grid-template-columns` utility.
    GridColumns {
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A directional border-width utility.
    BorderWidth {
        /// Sides affected by the border width.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A directional border-radius utility.
    RadiusEdge {
        /// Sides affected by the radius.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A sibling spacing utility.
    Space {
        /// Axis affected by sibling spacing.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A sibling divider utility.
    Divide {
        /// Axis affected by the divider.
        edge: SpacingEdge,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A utility that sets both width and height.
    SizeBoth {
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A grid span utility.
    GridSpan {
        /// Whether this is a row span rather than a column span.
        row: bool,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A grid line start/end utility.
    GridLine {
        /// Whether this is a row line rather than a column line.
        row: bool,
        /// Whether this is a start rather than an end line.
        start: bool,
        /// Stable utility ordering rank.
        order: u16,
    },
    /// A declarative one-property value utility.
    Functional {
        /// CSS property emitted by the family.
        property: String,
        /// Namespace used to resolve the primary value.
        namespace: ValueNamespace,
        /// Whether a value must be present.
        required: bool,
        /// Whether negative values are accepted.
        negative: bool,
        /// Stable utility ordering rank.
        order: u16,
    },
}

impl UtilityDefinition {
    /// Creates a fixed property/value utility definition.
    #[must_use]
    pub fn static_declaration(
        property: impl Into<String>,
        value: impl Into<String>,
        order: u16,
    ) -> Self {
        Self::Static { property: property.into(), value: value.into(), order }
    }

    /// Creates a fixed utility that emits multiple declarations.
    #[must_use]
    pub fn composite(declarations: Vec<(String, String)>, order: u16) -> Self {
        Self::Composite { declarations, order }
    }

    /// Creates a functional one-property utility backed by a value namespace.
    #[must_use]
    pub fn functional(
        property: impl Into<String>,
        namespace: ValueNamespace,
        required: bool,
        negative: bool,
        order: u16,
    ) -> Self {
        Self::Functional { property: property.into(), namespace, required, negative, order }
    }

    /// Returns a canonical semantic fingerprint for this definition.
    ///
    /// The representation is deliberately version-independent of Rust's formatting traits. Field
    /// tags, lengths, enum names, and values are encoded in the order defined by this method.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        match self {
            Self::Static { property, value, order } => {
                hash = hash_tag(hash, 0);
                hash = hash_text(hash, property);
                hash = hash_text(hash, value);
                hash_u16(hash, *order)
            }
            Self::Composite { declarations, order } => {
                hash = hash_tag(hash, 1);
                hash = hash_usize(hash, declarations.len());
                for (property, value) in declarations {
                    hash = hash_text(hash, property);
                    hash = hash_text(hash, value);
                }
                hash_u16(hash, *order)
            }
            Self::Spacing { margin, edge, order } => {
                hash = hash_tag(hash, 2);
                hash = hash_bool(hash, *margin);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::Gap { edge, order } => {
                hash = hash_tag(hash, 3);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::Size { dimension, order } => {
                hash = hash_tag(hash, 4);
                hash = hash_tag(hash, dimension_tag(*dimension));
                hash_u16(hash, *order)
            }
            Self::Color { kind, order } => {
                hash = hash_tag(hash, 5);
                hash = hash_tag(hash, color_kind_tag(*kind));
                hash_u16(hash, *order)
            }
            Self::Radius { order } => hash_u16(hash_tag(hash, 6), *order),
            Self::AlignItems { order } => hash_u16(hash_tag(hash, 7), *order),
            Self::JustifyContent { order } => hash_u16(hash_tag(hash, 8), *order),
            Self::GridColumns { order } => hash_u16(hash_tag(hash, 9), *order),
            Self::BorderWidth { edge, order } => {
                hash = hash_tag(hash, 10);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::RadiusEdge { edge, order } => {
                hash = hash_tag(hash, 11);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::Space { edge, order } => {
                hash = hash_tag(hash, 12);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::Divide { edge, order } => {
                hash = hash_tag(hash, 13);
                hash = hash_tag(hash, spacing_edge_tag(*edge));
                hash_u16(hash, *order)
            }
            Self::SizeBoth { order } => hash_u16(hash_tag(hash, 14), *order),
            Self::GridSpan { row, order } => {
                hash = hash_tag(hash, 15);
                hash = hash_bool(hash, *row);
                hash_u16(hash, *order)
            }
            Self::GridLine { row, start, order } => {
                hash = hash_tag(hash, 16);
                hash = hash_bool(hash, *row);
                hash = hash_bool(hash, *start);
                hash_u16(hash, *order)
            }
            Self::Functional { property, namespace, required, negative, order } => {
                hash = hash_tag(hash, 17);
                hash = hash_text(hash, property);
                hash = hash_text(hash, namespace.as_str());
                hash = hash_bool(hash, *required);
                hash = hash_bool(hash, *negative);
                hash_u16(hash, *order)
            }
        }
    }

    fn order(&self) -> u16 {
        match self {
            Self::Static { order, .. }
            | Self::Composite { order, .. }
            | Self::Spacing { order, .. }
            | Self::Gap { order, .. }
            | Self::Size { order, .. }
            | Self::Color { order, .. }
            | Self::Radius { order }
            | Self::AlignItems { order }
            | Self::JustifyContent { order }
            | Self::GridColumns { order }
            | Self::BorderWidth { order, .. }
            | Self::RadiusEdge { order, .. }
            | Self::Space { order, .. }
            | Self::Divide { order, .. }
            | Self::SizeBoth { order }
            | Self::GridSpan { order, .. }
            | Self::GridLine { order, .. }
            | Self::Functional { order, .. } => *order,
        }
    }
}

fn hash_tag(hash: u64, tag: u8) -> u64 {
    fnv1a(hash, &[tag])
}

fn hash_bool(hash: u64, value: bool) -> u64 {
    hash_tag(hash, u8::from(value))
}

fn hash_u16(hash: u64, value: u16) -> u64 {
    fnv1a(hash, &value.to_le_bytes())
}

fn hash_usize(hash: u64, value: usize) -> u64 {
    hash_usize_bytes(hash, value)
}

fn hash_text(hash: u64, value: &str) -> u64 {
    let hash = hash_usize_bytes(hash, value.len());
    fnv1a(hash, value.as_bytes())
}

fn hash_usize_bytes(hash: u64, value: usize) -> u64 {
    fnv1a(hash, &u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes())
}

fn spacing_edge_tag(edge: SpacingEdge) -> u8 {
    match edge {
        SpacingEdge::All => 0,
        SpacingEdge::X => 1,
        SpacingEdge::Y => 2,
        SpacingEdge::Top => 3,
        SpacingEdge::Right => 4,
        SpacingEdge::Bottom => 5,
        SpacingEdge::Left => 6,
    }
}

fn dimension_tag(dimension: Dimension) -> u8 {
    match dimension {
        Dimension::Width => 0,
        Dimension::Height => 1,
        Dimension::MinWidth => 2,
        Dimension::MaxWidth => 3,
        Dimension::MinHeight => 4,
        Dimension::MaxHeight => 5,
    }
}

fn color_kind_tag(kind: ColorKind) -> u8 {
    match kind {
        ColorKind::Background => 0,
        ColorKind::Text => 1,
        ColorKind::Border => 2,
    }
}

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Built-in utility definitions indexed by family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityRegistry {
    definitions: BTreeMap<String, UtilityDefinition>,
}

impl UtilityRegistry {
    /// Creates a registry containing the initial built-in utility set.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self { definitions: BTreeMap::new() };
        registry.register("block", UtilityDefinition::static_declaration("display", "block", 10));
        registry.register("inline", UtilityDefinition::static_declaration("display", "inline", 10));
        registry.register("flex", UtilityDefinition::static_declaration("display", "flex", 10));
        registry.register("grid", UtilityDefinition::static_declaration("display", "grid", 10));
        registry.register("hidden", UtilityDefinition::static_declaration("display", "none", 10));

        for (family, edge) in [
            ("p", SpacingEdge::All),
            ("px", SpacingEdge::X),
            ("py", SpacingEdge::Y),
            ("pt", SpacingEdge::Top),
            ("pr", SpacingEdge::Right),
            ("pb", SpacingEdge::Bottom),
            ("pl", SpacingEdge::Left),
        ] {
            registry
                .register(family, UtilityDefinition::Spacing { margin: false, edge, order: 20 });
        }
        for (family, edge) in [
            ("m", SpacingEdge::All),
            ("mx", SpacingEdge::X),
            ("my", SpacingEdge::Y),
            ("mt", SpacingEdge::Top),
            ("mr", SpacingEdge::Right),
            ("mb", SpacingEdge::Bottom),
            ("ml", SpacingEdge::Left),
        ] {
            registry.register(family, UtilityDefinition::Spacing { margin: true, edge, order: 21 });
        }
        for (family, edge) in
            [("gap", SpacingEdge::All), ("gap-x", SpacingEdge::X), ("gap-y", SpacingEdge::Y)]
        {
            registry.register(family, UtilityDefinition::Gap { edge, order: 22 });
        }

        for (family, dimension) in [
            ("w", Dimension::Width),
            ("h", Dimension::Height),
            ("min-w", Dimension::MinWidth),
            ("max-w", Dimension::MaxWidth),
            ("min-h", Dimension::MinHeight),
            ("max-h", Dimension::MaxHeight),
        ] {
            registry.register(family, UtilityDefinition::Size { dimension, order: 30 });
        }

        registry
            .register("bg", UtilityDefinition::Color { kind: ColorKind::Background, order: 40 });
        registry.register("text", UtilityDefinition::Color { kind: ColorKind::Text, order: 41 });
        registry
            .register("border", UtilityDefinition::Color { kind: ColorKind::Border, order: 42 });
        registry.register("rounded", UtilityDefinition::Radius { order: 50 });
        for (family, edge) in [
            ("rounded-t", SpacingEdge::Top),
            ("rounded-r", SpacingEdge::Right),
            ("rounded-b", SpacingEdge::Bottom),
            ("rounded-l", SpacingEdge::Left),
        ] {
            registry.register(family, UtilityDefinition::RadiusEdge { edge, order: 50 });
        }
        for (family, edge) in [
            ("border-x", SpacingEdge::X),
            ("border-y", SpacingEdge::Y),
            ("border-t", SpacingEdge::Top),
            ("border-r", SpacingEdge::Right),
            ("border-b", SpacingEdge::Bottom),
            ("border-l", SpacingEdge::Left),
        ] {
            registry.register(family, UtilityDefinition::BorderWidth { edge, order: 43 });
        }
        for (family, edge) in [("space-x", SpacingEdge::X), ("space-y", SpacingEdge::Y)] {
            registry.register(family, UtilityDefinition::Space { edge, order: 23 });
        }
        for (family, edge) in [("divide-x", SpacingEdge::X), ("divide-y", SpacingEdge::Y)] {
            registry.register(family, UtilityDefinition::Divide { edge, order: 44 });
        }
        registry.register("size", UtilityDefinition::SizeBoth { order: 32 });
        for (family, row) in [("col-span", false), ("row-span", true)] {
            registry.register(family, UtilityDefinition::GridSpan { row, order: 89 });
        }
        for (family, row, start) in [
            ("col-start", false, true),
            ("col-end", false, false),
            ("row-start", true, true),
            ("row-end", true, false),
        ] {
            registry.register(family, UtilityDefinition::GridLine { row, start, order: 90 });
        }
        registry.register("items", UtilityDefinition::AlignItems { order: 60 });
        registry.register("justify", UtilityDefinition::JustifyContent { order: 61 });
        registry.register("grid-cols", UtilityDefinition::GridColumns { order: 62 });
        register_extended_utilities(&mut registry);
        registry
    }

    /// Registers or replaces a utility family.
    pub fn register(&mut self, family: impl Into<String>, definition: UtilityDefinition) {
        self.definitions.insert(family.into(), definition);
    }

    /// Registers a utility after validating its public family name.
    pub fn register_checked(
        &mut self,
        family: impl Into<String>,
        definition: UtilityDefinition,
    ) -> Result<(), RegistryError> {
        let family = family.into();
        if family.is_empty()
            || !family
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(RegistryError::InvalidName(family));
        }
        if self.definitions.contains_key(&family) {
            return Err(RegistryError::DuplicateName(family));
        }
        self.definitions.insert(family, definition);
        Ok(())
    }

    /// Returns a registered definition for a utility family.
    #[must_use]
    pub fn get(&self, family: &str) -> Option<&UtilityDefinition> {
        self.definitions.get(family)
    }

    /// Returns whether a utility family is registered.
    #[must_use]
    pub fn contains(&self, family: &str) -> bool {
        self.definitions.contains_key(family)
    }

    /// Returns all registered family names in deterministic order.
    #[must_use]
    pub fn families(&self) -> Vec<&str> {
        self.definitions.keys().map(String::as_str).collect()
    }

    /// Returns every registered definition in deterministic name order.
    pub fn definitions(&self) -> impl Iterator<Item = (&str, &UtilityDefinition)> {
        self.definitions.iter().map(|(name, definition)| (name.as_str(), definition))
    }

    /// Returns metadata for one utility family.
    #[must_use]
    pub fn descriptor(&self, family: &str) -> Option<UtilityDescriptor> {
        self.get(family).map(|definition| descriptor_for(family, definition))
    }

    /// Returns all utility metadata in deterministic order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<UtilityDescriptor> {
        self.definitions().map(|(name, definition)| descriptor_for(name, definition)).collect()
    }

    /// Validates names and ordering ambiguity in the registry.
    pub fn validate(&self) -> Result<(), RegistryError> {
        for (name, definition) in &self.definitions {
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(RegistryError::InvalidName(name.clone()));
            }
            match definition {
                UtilityDefinition::Static { property, value, .. } => {
                    validate_declaration(name, property, value)?;
                }
                UtilityDefinition::Composite { declarations, .. } => {
                    for (property, value) in declarations {
                        validate_declaration(name, property, value)?;
                    }
                }
                UtilityDefinition::Functional { property, .. } if !safe_property(property) => {
                    return Err(RegistryError::InvalidDefinition {
                        name: name.clone(),
                        message: format!("invalid CSS property `{property}`"),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Default for UtilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation errors raised by a utility registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    /// A family name contains unsupported characters or is empty.
    InvalidName(String),
    /// A checked registration attempted to reuse an existing name.
    DuplicateName(String),
    /// A definition would emit unsafe CSS.
    InvalidDefinition {
        /// Utility family containing the invalid definition.
        name: String,
        /// Validation failure detail.
        message: String,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(formatter, "invalid utility name `{name}`"),
            Self::DuplicateName(name) => write!(formatter, "duplicate utility name `{name}`"),
            Self::InvalidDefinition { name, message } => {
                write!(formatter, "invalid utility definition `{name}`: {message}")
            }
        }
    }
}

fn validate_declaration(name: &str, property: &str, value: &str) -> Result<(), RegistryError> {
    if !safe_property(property) {
        return Err(RegistryError::InvalidDefinition {
            name: name.to_owned(),
            message: format!("invalid CSS property `{property}`"),
        });
    }
    if unsafe_css_fragment(value) {
        return Err(RegistryError::InvalidDefinition {
            name: name.to_owned(),
            message: "CSS declaration value contains an unsafe fragment".to_owned(),
        });
    }
    Ok(())
}

impl Error for RegistryError {}

fn descriptor_for(name: &str, definition: &UtilityDefinition) -> UtilityDescriptor {
    let (value_schema, negative_policy, arbitrary_policy, properties, category, description) =
        match definition {
            UtilityDefinition::Static { property, .. } => {
                let namespace =
                    static_theme_namespace(name, property).unwrap_or(ValueNamespace::Keyword);
                (
                    ValueSchema::new(namespace, false, ArbitraryPolicy::Unsupported),
                    NegativePolicy::Unsupported,
                    ArbitraryPolicy::Unsupported,
                    vec![CssProperty::new(property)],
                    category_for(name),
                    format!("Sets {property}"),
                )
            }
            UtilityDefinition::Composite { declarations, .. } => (
                ValueSchema::new(ValueNamespace::Keyword, false, ArbitraryPolicy::Unsupported),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Unsupported,
                declarations.iter().map(|(property, _)| CssProperty::new(property)).collect(),
                category_for(name),
                "Emits a deterministic declaration bundle".to_owned(),
            ),
            UtilityDefinition::Spacing { margin, edge, .. } => (
                ValueSchema::new(ValueNamespace::Spacing, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Allowed,
                ArbitraryPolicy::Allowed,
                spacing_properties(*margin, *edge),
                UtilityCategory::SpacingSizing,
                if *margin { "Sets margin spacing" } else { "Sets padding spacing" }.to_owned(),
            ),
            UtilityDefinition::Gap { edge, .. } => (
                ValueSchema::new(ValueNamespace::Spacing, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                gap_properties(*edge),
                UtilityCategory::FlexGrid,
                "Sets a grid or flex gap".to_owned(),
            ),
            UtilityDefinition::Size { dimension, .. } => (
                ValueSchema::new(
                    match dimension {
                        Dimension::Width | Dimension::MinWidth | Dimension::MaxWidth => {
                            ValueNamespace::Width
                        }
                        Dimension::Height | Dimension::MinHeight | Dimension::MaxHeight => {
                            ValueNamespace::Height
                        }
                    },
                    true,
                    ArbitraryPolicy::Allowed,
                ),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(dimension.property())],
                UtilityCategory::SpacingSizing,
                "Sets a box dimension".to_owned(),
            ),
            UtilityDefinition::Color { kind, .. } => (
                ValueSchema::new(ValueNamespace::Color, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(kind.property())],
                UtilityCategory::BackgroundBorder,
                "Sets a color".to_owned(),
            ),
            UtilityDefinition::Radius { .. } => (
                ValueSchema::new(ValueNamespace::Radius, false, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new("border-radius")],
                UtilityCategory::BackgroundBorder,
                "Sets border radius".to_owned(),
            ),
            UtilityDefinition::AlignItems { .. } => (
                ValueSchema::new(ValueNamespace::Keyword, true, ArbitraryPolicy::Unsupported),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Unsupported,
                vec![CssProperty::new("align-items")],
                UtilityCategory::FlexGrid,
                "Aligns items on the cross axis".to_owned(),
            ),
            UtilityDefinition::JustifyContent { .. } => (
                ValueSchema::new(ValueNamespace::Keyword, true, ArbitraryPolicy::Unsupported),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Unsupported,
                vec![CssProperty::new("justify-content")],
                UtilityCategory::FlexGrid,
                "Justifies content on the main axis".to_owned(),
            ),
            UtilityDefinition::GridColumns { .. } => (
                ValueSchema::new(ValueNamespace::Integer, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new("grid-template-columns")],
                UtilityCategory::FlexGrid,
                "Defines grid columns".to_owned(),
            ),
            UtilityDefinition::BorderWidth { edge, .. } => (
                ValueSchema::new(ValueNamespace::Length, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                border_properties(*edge),
                UtilityCategory::BackgroundBorder,
                "Sets border width".to_owned(),
            ),
            UtilityDefinition::RadiusEdge { edge, .. } => (
                ValueSchema::new(ValueNamespace::Radius, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                radius_properties(*edge),
                UtilityCategory::BackgroundBorder,
                "Sets directional border radius".to_owned(),
            ),
            UtilityDefinition::Space { edge, .. } => (
                ValueSchema::new(ValueNamespace::Spacing, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Allowed,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(match edge {
                    SpacingEdge::X => "margin-left",
                    SpacingEdge::Y => "margin-top",
                    _ => "margin",
                })],
                UtilityCategory::SpacingSizing,
                "Adds spacing between siblings".to_owned(),
            ),
            UtilityDefinition::Divide { edge, .. } => (
                ValueSchema::new(ValueNamespace::Keyword, false, ArbitraryPolicy::Unsupported),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Unsupported,
                vec![CssProperty::new(match edge {
                    SpacingEdge::X => "border-left-width",
                    SpacingEdge::Y => "border-top-width",
                    _ => "border-width",
                })],
                UtilityCategory::BackgroundBorder,
                "Adds borders between siblings".to_owned(),
            ),
            UtilityDefinition::SizeBoth { .. } => (
                ValueSchema::new(ValueNamespace::Spacing, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new("width"), CssProperty::new("height")],
                UtilityCategory::SpacingSizing,
                "Sets width and height together".to_owned(),
            ),
            UtilityDefinition::GridSpan { row, .. } => (
                ValueSchema::new(ValueNamespace::Integer, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(if *row { "grid-row" } else { "grid-column" })],
                UtilityCategory::FlexGrid,
                "Sets a grid span".to_owned(),
            ),
            UtilityDefinition::GridLine { row, start, .. } => (
                ValueSchema::new(ValueNamespace::Integer, true, ArbitraryPolicy::Allowed),
                NegativePolicy::Unsupported,
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(if *row {
                    if *start {
                        "grid-row-start"
                    } else {
                        "grid-row-end"
                    }
                } else if *start {
                    "grid-column-start"
                } else {
                    "grid-column-end"
                })],
                UtilityCategory::FlexGrid,
                "Sets a grid line".to_owned(),
            ),
            UtilityDefinition::Functional { property, namespace, negative, .. } => (
                ValueSchema::new(namespace.clone(), true, ArbitraryPolicy::Allowed),
                if *negative { NegativePolicy::Allowed } else { NegativePolicy::Unsupported },
                ArbitraryPolicy::Allowed,
                vec![CssProperty::new(property)],
                category_for(name),
                format!("Sets {property}"),
            ),
        };
    let modifier_schema = if matches!(definition, UtilityDefinition::Color { .. }) {
        ModifierSchema::optional(ValueNamespace::Percentage, "Optional color alpha modifier")
    } else if name.starts_with("shadow") {
        ModifierSchema::utility_specific("Optional shadow alpha modifier")
    } else {
        ModifierSchema::unsupported()
    };
    let value_schema = if value_schema.description.is_empty() {
        let namespace = value_schema.namespace.as_str();
        value_schema.with_description(format!("{namespace} values"))
    } else {
        value_schema
    };
    let value_schema = match definition {
        UtilityDefinition::AlignItems { .. } => {
            value_schema.with_allowed_values(["start", "end", "center", "baseline", "stretch"])
        }
        UtilityDefinition::JustifyContent { .. } => value_schema.with_allowed_values([
            "normal", "start", "end", "center", "between", "around", "evenly", "stretch",
        ]),
        _ => value_schema,
    };
    let namespaces = match &value_schema.namespace {
        ValueNamespace::Keyword | ValueNamespace::Raw => Vec::new(),
        namespace => vec![namespace.as_str().to_owned()],
    };
    UtilityDescriptor {
        id: UtilityId::new(name),
        names: vec![name.to_owned()],
        category,
        description,
        value_schema,
        modifier_schema,
        negative_policy,
        arbitrary_policy,
        theme_namespaces: namespaces,
        emitted_properties: properties,
        dependencies: descriptor_dependencies(definition),
        ordering_group: u32::from(definition.order()),
        examples: vec![UtilityExample {
            candidate: name.to_owned(),
            description: "Registered utility family".to_owned(),
        }],
        docs_key: format!("utility.{name}"),
        stability: Stability::Stable,
    }
}

fn descriptor_dependencies(definition: &UtilityDefinition) -> Vec<String> {
    let mut dependencies = BTreeMap::new();
    if let UtilityDefinition::Composite { declarations, .. } = definition {
        for (_, value) in declarations {
            for name in css_dependencies(value) {
                dependencies.insert(name, ());
            }
        }
    }
    dependencies.into_keys().collect()
}

fn static_theme_namespace(name: &str, property: &str) -> Option<ValueNamespace> {
    match property {
        "font-size" if name.starts_with("text-") => Some(ValueNamespace::FontSize),
        "font-family" if name.starts_with("font-") => Some(ValueNamespace::FontFamily),
        "line-height" if name.starts_with("leading-") => Some(ValueNamespace::LineHeight),
        "box-shadow" if name.starts_with("shadow") => Some(ValueNamespace::Shadow),
        _ => None,
    }
}

fn category_for(name: &str) -> UtilityCategory {
    match name {
        name if name.starts_with("text-")
            || matches!(name, "text" | "font" | "leading" | "tracking") =>
        {
            UtilityCategory::Typography
        }
        name if name.starts_with("bg")
            || name.starts_with("border")
            || name.starts_with("rounded") =>
        {
            UtilityCategory::BackgroundBorder
        }
        name if name.starts_with("shadow")
            || name.starts_with("opacity")
            || name.starts_with("filter") =>
        {
            UtilityCategory::Effects
        }
        name if name.starts_with("transition")
            || name.starts_with("duration")
            || name.starts_with("ease")
            || name.starts_with("animate") =>
        {
            UtilityCategory::TransformsTransitions
        }
        name if name.starts_with("grid")
            || name.starts_with("flex")
            || name.starts_with("justify")
            || name.starts_with("items")
            || name.starts_with("gap") =>
        {
            UtilityCategory::FlexGrid
        }
        name if name.starts_with("p")
            || name.starts_with("m")
            || name.starts_with("w")
            || name.starts_with("h")
            || name.starts_with("size") =>
        {
            UtilityCategory::SpacingSizing
        }
        _ => UtilityCategory::Layout,
    }
}

fn spacing_properties(margin: bool, edge: SpacingEdge) -> Vec<CssProperty> {
    let property = if margin { "margin" } else { "padding" };
    match edge {
        SpacingEdge::All => vec![CssProperty::new(property)],
        SpacingEdge::X => vec![
            CssProperty::new(format!("{property}-left")),
            CssProperty::new(format!("{property}-right")),
        ],
        SpacingEdge::Y => vec![
            CssProperty::new(format!("{property}-top")),
            CssProperty::new(format!("{property}-bottom")),
        ],
        SpacingEdge::Top => vec![CssProperty::new(format!("{property}-top"))],
        SpacingEdge::Right => vec![CssProperty::new(format!("{property}-right"))],
        SpacingEdge::Bottom => vec![CssProperty::new(format!("{property}-bottom"))],
        SpacingEdge::Left => vec![CssProperty::new(format!("{property}-left"))],
    }
}

fn gap_properties(edge: SpacingEdge) -> Vec<CssProperty> {
    vec![CssProperty::new(match edge {
        SpacingEdge::All => "gap",
        SpacingEdge::X => "column-gap",
        SpacingEdge::Y => "row-gap",
        SpacingEdge::Top | SpacingEdge::Right | SpacingEdge::Bottom | SpacingEdge::Left => "gap",
    })]
}

fn border_properties(edge: SpacingEdge) -> Vec<CssProperty> {
    match edge {
        SpacingEdge::All => vec![CssProperty::new("border-width")],
        SpacingEdge::X => {
            vec![CssProperty::new("border-left-width"), CssProperty::new("border-right-width")]
        }
        SpacingEdge::Y => {
            vec![CssProperty::new("border-top-width"), CssProperty::new("border-bottom-width")]
        }
        SpacingEdge::Top => vec![CssProperty::new("border-top-width")],
        SpacingEdge::Right => vec![CssProperty::new("border-right-width")],
        SpacingEdge::Bottom => vec![CssProperty::new("border-bottom-width")],
        SpacingEdge::Left => vec![CssProperty::new("border-left-width")],
    }
}

fn radius_properties(edge: SpacingEdge) -> Vec<CssProperty> {
    match edge {
        SpacingEdge::All => vec![CssProperty::new("border-radius")],
        SpacingEdge::X => vec![
            CssProperty::new("border-start-start-radius"),
            CssProperty::new("border-end-end-radius"),
        ],
        SpacingEdge::Y => vec![
            CssProperty::new("border-start-start-radius"),
            CssProperty::new("border-end-end-radius"),
        ],
        SpacingEdge::Top => vec![
            CssProperty::new("border-top-left-radius"),
            CssProperty::new("border-top-right-radius"),
        ],
        SpacingEdge::Right => vec![
            CssProperty::new("border-top-right-radius"),
            CssProperty::new("border-bottom-right-radius"),
        ],
        SpacingEdge::Bottom => vec![
            CssProperty::new("border-bottom-left-radius"),
            CssProperty::new("border-bottom-right-radius"),
        ],
        SpacingEdge::Left => vec![
            CssProperty::new("border-top-left-radius"),
            CssProperty::new("border-bottom-left-radius"),
        ],
    }
}

/// The category of a utility resolution error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UtilityErrorKind {
    /// No definition was registered for the family.
    UnknownUtility,
    /// A value was required but was absent.
    MissingValue,
    /// A named value was not found in the relevant theme map.
    UnknownThemeValue,
    /// An arbitrary value contains characters that could escape a declaration.
    InvalidArbitraryValue,
    /// A negative marker was used by a utility that does not support it.
    UnsupportedNegative,
    /// A named value is not accepted by the utility family.
    InvalidValue,
    /// A utility that accepts no value received one.
    UnexpectedValue,
    /// A registered static declaration would produce invalid or unsafe CSS.
    InvalidDeclaration,
    /// A parsed modifier is not supported by the utility definition.
    UnsupportedModifier,
    /// A supported modifier has the wrong value shape.
    InvalidModifier,
}

impl UtilityErrorKind {
    const fn code(self) -> DiagnosticCode {
        DiagnosticCode::new(match self {
            Self::UnknownUtility => "utility.unknown",
            Self::MissingValue => "utility.missing-value",
            Self::UnknownThemeValue => "utility.unknown-theme-value",
            Self::InvalidArbitraryValue => "utility.invalid-arbitrary-value",
            Self::UnsupportedNegative => "utility.unsupported-negative",
            Self::InvalidValue => "utility.invalid-value",
            Self::UnexpectedValue => "utility.unexpected-value",
            Self::InvalidDeclaration => "utility.invalid-declaration",
            Self::UnsupportedModifier => "utility.unsupported-modifier",
            Self::InvalidModifier => "utility.invalid-modifier",
        })
    }
}

/// A typed error raised while lowering one parsed utility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UtilityError {
    kind: UtilityErrorKind,
    family: String,
    value: Option<String>,
}

impl UtilityError {
    fn new(kind: UtilityErrorKind, family: &str, value: Option<&str>) -> Self {
        Self { kind, family: family.to_owned(), value: value.map(str::to_owned) }
    }

    /// Returns the error category.
    #[must_use]
    pub const fn kind(&self) -> UtilityErrorKind {
        self.kind
    }

    /// Returns the utility family involved in the error.
    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Returns the optional value involved in the error.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    /// Converts the error into a source-aware diagnostic.
    #[must_use]
    pub fn to_diagnostic(&self, source: SourceId, span: Span) -> Diagnostic {
        Diagnostic::error(self.kind.code(), self.to_string())
            .with_source(source)
            .with_span(span)
            .with_explanation(
            "The parsed utility was rejected by its declarative value, modifier, or safety policy.",
        )
    }
}

impl fmt::Display for UtilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.kind, self.value.as_deref()) {
            (UtilityErrorKind::UnknownUtility, _) => {
                write!(formatter, "unknown utility family `{}`", self.family)
            }
            (UtilityErrorKind::MissingValue, _) => {
                write!(formatter, "utility `{}` requires a value", self.family)
            }
            (UtilityErrorKind::UnknownThemeValue, Some(value)) => {
                write!(formatter, "unknown theme value `{value}` for utility `{}`", self.family)
            }
            (UtilityErrorKind::InvalidArbitraryValue, Some(value)) => {
                write!(formatter, "unsafe arbitrary value `{value}` for utility `{}`", self.family)
            }
            (UtilityErrorKind::UnsupportedNegative, _) => {
                write!(formatter, "utility `{}` does not support negative values", self.family)
            }
            (UtilityErrorKind::InvalidValue, Some(value)) => {
                write!(formatter, "invalid value `{value}` for utility `{}`", self.family)
            }
            (UtilityErrorKind::UnexpectedValue, Some(value)) => {
                write!(formatter, "utility `{}` does not accept value `{value}`", self.family)
            }
            (UtilityErrorKind::InvalidDeclaration, Some(value)) => {
                write!(
                    formatter,
                    "unsafe static declaration `{value}` for utility `{}`",
                    self.family
                )
            }
            (UtilityErrorKind::UnsupportedModifier, Some(value)) => {
                write!(formatter, "utility `{}` does not support modifier `{value}`", self.family)
            }
            (UtilityErrorKind::InvalidModifier, Some(value)) => {
                write!(formatter, "invalid modifier `{value}` for utility `{}`", self.family)
            }
            _ => write!(formatter, "invalid utility `{}`", self.family),
        }
    }
}

impl Error for UtilityError {}

/// A utility lowered into a selector, declarations, and a stable utility rank.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUtility {
    selector: String,
    declarations: Vec<CssDeclaration>,
    order: u16,
    value: Option<ResolvedValue>,
    modifier: Option<ResolvedValue>,
}

/// A typed value produced by semantic resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ResolvedValue {
    /// A CSS length.
    Length(String),
    /// A CSS number.
    Number(String),
    /// A CSS integer.
    Integer(String),
    /// A CSS percentage.
    Percentage(String),
    /// A CSS color.
    Color(String),
    /// A CSS ratio.
    Ratio(String),
    /// A CSS time.
    Time(String),
    /// A CSS angle.
    Angle(String),
    /// A CSS keyword.
    Keyword(String),
    /// Validated raw CSS content.
    RawCss(String),
}

impl ResolvedValue {
    /// Returns the CSS serialization of the typed value.
    #[must_use]
    pub fn css(&self) -> &str {
        match self {
            Self::Length(value)
            | Self::Number(value)
            | Self::Integer(value)
            | Self::Percentage(value)
            | Self::Color(value)
            | Self::Ratio(value)
            | Self::Time(value)
            | Self::Angle(value)
            | Self::Keyword(value)
            | Self::RawCss(value) => value,
        }
    }
}

impl ResolvedUtility {
    /// Returns the escaped class selector.
    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Returns declarations in their deterministic property order.
    #[must_use]
    pub fn declarations(&self) -> &[CssDeclaration] {
        &self.declarations
    }

    /// Returns the utility ordering rank.
    #[must_use]
    pub const fn order(&self) -> u16 {
        self.order
    }

    /// Returns the typed primary value, if this utility has one.
    #[must_use]
    pub fn value(&self) -> Option<&ResolvedValue> {
        self.value.as_ref()
    }

    /// Returns the typed modifier, if one was consumed.
    #[must_use]
    pub fn modifier(&self) -> Option<&ResolvedValue> {
        self.modifier.as_ref()
    }

    /// Converts this utility into a CSS IR style rule.
    #[must_use]
    pub fn into_rule(self, tie_breaker: u64) -> CssRule {
        CssRule::style(
            OrderKey::new(0, 0, self.order, tie_breaker),
            self.selector,
            self.declarations,
        )
    }
}

/// Resolves a parsed candidate through a registry and theme.
pub fn resolve(
    candidate: &CandidateAst<'_>,
    theme: &Theme,
    registry: &UtilityRegistry,
) -> Result<ResolvedUtility, UtilityError> {
    let utility = candidate.utility();
    if let Some(property) = utility.arbitrary_property() {
        if !safe_property(property.property) || unsafe_css_fragment(property.value.content()) {
            return Err(UtilityError::new(
                UtilityErrorKind::InvalidDeclaration,
                property.property,
                Some(property.value.content()),
            ));
        }
        let value = decode_arbitrary(property.value.content()).map_err(|_| {
            UtilityError::new(
                UtilityErrorKind::InvalidArbitraryValue,
                property.property,
                Some(property.value.content()),
            )
        })?;
        let declaration =
            declaration_with_dependencies(property.property, value, candidate.is_important());
        return Ok(ResolvedUtility {
            selector: escape_class_selector(candidate.raw()),
            declarations: vec![declaration],
            order: 1,
            value: Some(ResolvedValue::RawCss(property.value.content().to_owned())),
            modifier: None,
        });
    }
    let family = utility.family();
    let family_definition = registry.get(family);
    let exact_base_name = utility.value().map(|value| format!("{}-{}", family, value.raw()));
    let exact_definition = registry
        .get(utility.raw())
        .or_else(|| exact_base_name.as_deref().and_then(|name| registry.get(name)));
    let exact_name_match = utility.raw() != family && exact_definition.is_some();
    let definition =
        if exact_name_match { exact_definition } else { family_definition.or(exact_definition) }
            .ok_or_else(|| UtilityError::new(UtilityErrorKind::UnknownUtility, family, None))?;
    validate_typed_value(utility.value(), definition, family)?;
    let value_text = utility.value().map(ValueAst::content);
    let important = candidate.is_important();
    let mut resolved_value = None;
    let mut resolved_modifier = None;
    let declarations = match definition {
        UtilityDefinition::Static { property, value, .. } => {
            if !safe_property(property) || unsafe_css_fragment(value) {
                return Err(UtilityError::new(
                    UtilityErrorKind::InvalidDeclaration,
                    family,
                    Some(value),
                ));
            }
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            if value_text.is_some() && !exact_name_match {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnexpectedValue,
                    family,
                    value_text,
                ));
            }
            let mut rendered_value = static_theme_value(utility.raw(), property, value, theme);
            if unsafe_css_fragment(&rendered_value) {
                return Err(UtilityError::new(
                    UtilityErrorKind::InvalidDeclaration,
                    family,
                    Some(&rendered_value),
                ));
            }
            if let Some(modifier) = utility.modifier() {
                if property != "box-shadow" {
                    return Err(UtilityError::new(
                        UtilityErrorKind::UnsupportedModifier,
                        family,
                        Some(modifier.content()),
                    ));
                }
                let (shadow, typed_modifier) = shadow_modifier(&rendered_value, modifier, family)?;
                rendered_value = shadow;
                resolved_modifier = Some(typed_modifier);
            }
            resolved_value = Some(ResolvedValue::RawCss(rendered_value.clone()));
            vec![CssDeclaration::new(property.clone(), rendered_value).with_important(important)]
        }
        UtilityDefinition::Composite { declarations, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            if value_text.is_some() && !exact_name_match {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnexpectedValue,
                    family,
                    value_text,
                ));
            }
            declarations
                .iter()
                .map(|(property, value)| {
                    if !safe_property(property) || unsafe_css_fragment(value) {
                        return Err(UtilityError::new(
                            UtilityErrorKind::InvalidDeclaration,
                            family,
                            Some(value),
                        ));
                    }
                    Ok(declaration_with_dependencies(property.clone(), value.clone(), important))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        UtilityDefinition::Spacing { margin, edge, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            let value = required_value(utility.value(), family)?;
            let value = spacing_value(value, theme, family)?;
            let value = if utility.is_negative() { negate(value) } else { value };
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            spacing_declarations(*margin, *edge, value, important)
        }
        UtilityDefinition::Gap { edge, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = spacing_value(value, theme, family)?;
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            gap_declarations(*edge, value, important)
        }
        UtilityDefinition::Size { dimension, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = size_value(value, theme, *dimension, family)?;
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            vec![CssDeclaration::new(dimension.property(), value).with_important(important)]
        }
        UtilityDefinition::Color { kind, .. } => {
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            if *kind == ColorKind::Border && utility.value().is_none() {
                ensure_no_modifier(utility.modifier(), family)?;
                resolved_value = Some(ResolvedValue::Length("1px".to_owned()));
                return Ok(ResolvedUtility {
                    selector: escape_class_selector(candidate.raw()),
                    declarations: vec![
                        CssDeclaration::new("border-width", "1px").with_important(important)
                    ],
                    order: definition.order(),
                    value: resolved_value,
                    modifier: None,
                });
            }
            if *kind == ColorKind::Border
                && utility.value().is_some_and(|value| {
                    matches!(value, ValueAst::Named(name) if name == "0" || name.parse::<u16>().is_ok())
                })
            {
                ensure_no_modifier(utility.modifier(), family)?;
                let value = border_width_value(required_value(utility.value(), family)?, family)?;
                resolved_value = Some(ResolvedValue::Length(value.clone()));
                return Ok(ResolvedUtility {
                    selector: escape_class_selector(candidate.raw()),
                    declarations: vec![
                        CssDeclaration::new("border-width", value).with_important(important),
                    ],
                    order: definition.order(),
                    value: resolved_value,
                    modifier: None,
                });
            }
            let value = required_value(utility.value(), family)?;
            if let Some(hint) = value.type_hint() {
                if *kind == ColorKind::Background && hint == "image" {
                    ensure_no_modifier(utility.modifier(), family)?;
                    let image = safe_arbitrary(value.content(), family)?;
                    resolved_value = Some(ResolvedValue::RawCss(image.clone()));
                    return Ok(ResolvedUtility {
                        selector: escape_class_selector(candidate.raw()),
                        declarations: vec![declaration_with_dependencies(
                            "background-image",
                            image,
                            important,
                        )],
                        order: definition.order(),
                        value: resolved_value,
                        modifier: None,
                    });
                }
                if !((hint == "color")
                    || (*kind == ColorKind::Text && matches!(hint, "length" | "number")))
                {
                    return Err(UtilityError::new(
                        UtilityErrorKind::InvalidValue,
                        family,
                        Some(hint),
                    ));
                }
            }
            if *kind == ColorKind::Text
                && value.type_hint().is_some_and(|hint| matches!(hint, "length" | "number"))
            {
                ensure_no_modifier(utility.modifier(), family)?;
                let value = size_value(value, theme, Dimension::Width, family)?;
                resolved_value = Some(ResolvedValue::Length(value.clone()));
                return Ok(ResolvedUtility {
                    selector: escape_class_selector(candidate.raw()),
                    declarations: vec![
                        CssDeclaration::new("font-size", value).with_important(important)
                    ],
                    order: definition.order(),
                    value: resolved_value,
                    modifier: None,
                });
            }
            let value = color_value(value, theme, family, *kind)?;
            resolved_value = Some(ResolvedValue::Color(value.clone()));
            let value = if let Some(modifier) = utility.modifier() {
                let (css_modifier, typed_modifier) = color_modifier(modifier, family)?;
                resolved_modifier = Some(typed_modifier);
                format!("color-mix(in srgb, {value} {css_modifier}, transparent)")
            } else {
                value
            };
            vec![CssDeclaration::new(kind.property(), value).with_important(important)]
        }
        UtilityDefinition::Radius { .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = match utility.value() {
                Some(value) => radius_value(value, theme, family)?,
                None => theme
                    .radius("DEFAULT")
                    .map(|value| validated_theme_value(value, family))
                    .transpose()?
                    .ok_or_else(|| {
                        UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, value_text)
                    })?,
            };
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            vec![CssDeclaration::new("border-radius", value).with_important(important)]
        }
        UtilityDefinition::AlignItems { .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            let value = named_choice(
                utility.value(),
                family,
                &["start", "end", "center", "baseline", "stretch"],
            )?;
            resolved_value = Some(ResolvedValue::Keyword(value.clone()));
            vec![CssDeclaration::new("align-items", value.clone()).with_important(important)]
        }
        UtilityDefinition::JustifyContent { .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            let value = named_choice(
                utility.value(),
                family,
                &["normal", "start", "end", "center", "between", "around", "evenly", "stretch"],
            )?;
            let value = match value.as_str() {
                "between" => "space-between",
                "around" => "space-around",
                "evenly" => "space-evenly",
                _ => value.as_str(),
            };
            resolved_value = Some(ResolvedValue::Keyword(value.to_owned()));
            vec![CssDeclaration::new("justify-content", value.to_owned()).with_important(important)]
        }
        UtilityDefinition::GridColumns { .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = grid_columns_value(value, family)?;
            resolved_value = Some(ResolvedValue::RawCss(value.clone()));
            vec![CssDeclaration::new("grid-template-columns", value).with_important(important)]
        }
        UtilityDefinition::BorderWidth { edge, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = border_width_value(value, family)?;
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            border_declarations(*edge, value, important)
        }
        UtilityDefinition::RadiusEdge { edge, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = radius_value(value, theme, family)?;
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            radius_declarations(*edge, value, important)
        }
        UtilityDefinition::Space { edge, .. } => {
            if utility.modifier().is_some() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedModifier,
                    family,
                    utility.modifier().map(ValueAst::content),
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = spacing_value(value, theme, family)?;
            let value = if utility.is_negative() { negate(value) } else { value };
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            vec![CssDeclaration::new(
                if *edge == SpacingEdge::X { "margin-left" } else { "margin-top" },
                value,
            )
            .with_important(important)]
        }
        UtilityDefinition::Divide { edge, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = utility
                .value()
                .map(|value| border_width_value(value, family))
                .transpose()?
                .unwrap_or_else(|| "1px".to_owned());
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            vec![CssDeclaration::new(
                if *edge == SpacingEdge::X { "border-left-width" } else { "border-top-width" },
                value,
            )
            .with_important(important)]
        }
        UtilityDefinition::SizeBoth { .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = size_value(value, theme, Dimension::Width, family)?;
            resolved_value = Some(ResolvedValue::Length(value.clone()));
            vec![
                CssDeclaration::new("width", value.clone()).with_important(important),
                CssDeclaration::new("height", value).with_important(important),
            ]
        }
        UtilityDefinition::GridSpan { row, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let ValueAst::Named(value) = value else {
                return Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, value_text));
            };
            if value.parse::<u16>().is_err() {
                return Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(value)));
            }
            let property = if *row { "grid-row" } else { "grid-column" };
            let rendered = format!("span {value} / span {value}");
            resolved_value = Some(ResolvedValue::Integer(value.to_owned()));
            vec![CssDeclaration::new(property, rendered).with_important(important)]
        }
        UtilityDefinition::GridLine { row, start, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = required_value(utility.value(), family)?;
            let value = match value {
                ValueAst::Named(value) => value.to_owned(),
                ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
                    safe_arbitrary(content, family)?
                }
                ValueAst::Fraction { numerator, .. } => numerator.to_owned(),
            };
            let property = if *row {
                if *start {
                    "grid-row-start"
                } else {
                    "grid-row-end"
                }
            } else if *start {
                "grid-column-start"
            } else {
                "grid-column-end"
            };
            resolved_value = Some(ResolvedValue::Integer(value.clone()));
            vec![CssDeclaration::new(property, value).with_important(important)]
        }
        UtilityDefinition::Functional { property, namespace, required, negative, .. } => {
            ensure_no_modifier(utility.modifier(), family)?;
            if utility.is_negative() && !negative {
                return Err(UtilityError::new(
                    UtilityErrorKind::UnsupportedNegative,
                    family,
                    value_text,
                ));
            }
            let value = if *required {
                required_value(utility.value(), family)?
            } else if let Some(value) = utility.value() {
                value
            } else {
                return Err(UtilityError::new(UtilityErrorKind::MissingValue, family, None));
            };
            let mut value = resolve_namespace_value(value, theme, namespace, family)?;
            if utility.is_negative() {
                value = negate(value);
            }
            resolved_value = Some(resolved_value_for_namespace(value.clone(), namespace));
            vec![CssDeclaration::new(property.clone(), value).with_important(important)]
        }
    };

    let escaped_selector = escape_class_selector(candidate.raw());
    let selector = match definition {
        UtilityDefinition::Space { .. } => {
            format!("{escaped_selector} > :not(:last-child)")
        }
        UtilityDefinition::Divide { .. } => {
            format!("{escaped_selector} > :not(:last-child) ~ :not(:last-child)")
        }
        _ => escaped_selector,
    };
    Ok(ResolvedUtility {
        selector,
        declarations,
        order: definition.order(),
        value: resolved_value,
        modifier: resolved_modifier,
    })
}

fn required_value<'source>(
    value: Option<ValueAst<'source>>,
    family: &str,
) -> Result<ValueAst<'source>, UtilityError> {
    value.ok_or_else(|| UtilityError::new(UtilityErrorKind::MissingValue, family, None))
}

fn validate_typed_value(
    value: Option<ValueAst<'_>>,
    definition: &UtilityDefinition,
    family: &str,
) -> Result<(), UtilityError> {
    let Some(hint) = value.and_then(ValueAst::type_hint) else {
        return Ok(());
    };
    let valid = match definition {
        UtilityDefinition::Color { kind: ColorKind::Background, .. } => {
            matches!(hint, "color" | "image")
        }
        UtilityDefinition::Color { kind: ColorKind::Text, .. } => {
            matches!(hint, "color" | "length" | "number")
        }
        UtilityDefinition::Color { kind: ColorKind::Border, .. } => hint == "color",
        UtilityDefinition::Spacing { .. }
        | UtilityDefinition::Gap { .. }
        | UtilityDefinition::Space { .. } => matches!(hint, "length" | "percentage"),
        UtilityDefinition::Size { .. } | UtilityDefinition::SizeBoth { .. } => {
            matches!(hint, "length" | "percentage" | "number")
        }
        UtilityDefinition::Radius { .. } | UtilityDefinition::RadiusEdge { .. } => {
            matches!(hint, "length" | "percentage")
        }
        UtilityDefinition::BorderWidth { .. } | UtilityDefinition::Divide { .. } => {
            matches!(hint, "length" | "number")
        }
        UtilityDefinition::Functional { namespace, .. } => typed_namespace_accepts(namespace, hint),
        UtilityDefinition::Static { .. }
        | UtilityDefinition::Composite { .. }
        | UtilityDefinition::AlignItems { .. }
        | UtilityDefinition::JustifyContent { .. }
        | UtilityDefinition::GridColumns { .. }
        | UtilityDefinition::GridSpan { .. }
        | UtilityDefinition::GridLine { .. } => true,
    };
    if valid {
        Ok(())
    } else {
        Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(hint)))
    }
}

fn typed_namespace_accepts(namespace: &ValueNamespace, hint: &str) -> bool {
    match namespace {
        ValueNamespace::Raw | ValueNamespace::Keyword => true,
        ValueNamespace::Spacing
        | ValueNamespace::Width
        | ValueNamespace::Height
        | ValueNamespace::Radius => matches!(hint, "length" | "percentage" | "number"),
        ValueNamespace::Color => hint == "color",
        ValueNamespace::FontFamily
        | ValueNamespace::Shadow
        | ValueNamespace::Duration
        | ValueNamespace::Ease
        | ValueNamespace::ZIndex => false,
        ValueNamespace::FontSize | ValueNamespace::LineHeight => {
            matches!(hint, "length" | "number")
        }
        ValueNamespace::LetterSpacing => hint == "length",
        ValueNamespace::Number => hint == "number",
        ValueNamespace::Length => hint == "length",
        ValueNamespace::Percentage => hint == "percentage",
        ValueNamespace::Integer => hint == "number",
    }
}

fn ensure_no_modifier(modifier: Option<ValueAst<'_>>, family: &str) -> Result<(), UtilityError> {
    modifier.map_or(Ok(()), |modifier| {
        Err(UtilityError::new(
            UtilityErrorKind::UnsupportedModifier,
            family,
            Some(modifier.content()),
        ))
    })
}

fn spacing_value(value: ValueAst<'_>, theme: &Theme, family: &str) -> Result<String, UtilityError> {
    match value {
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator } => {
            fraction_percentage(numerator, denominator, family)
        }
        ValueAst::Named(key) => {
            if let Some(value) = theme.spacing(key) {
                validated_theme_value(value, family)
            } else if key.parse::<f64>().is_ok() {
                Ok(format!("calc(var(--spacing) * {key})"))
            } else {
                Err(UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key)))
            }
        }
    }
}

fn size_value(
    value: ValueAst<'_>,
    theme: &Theme,
    dimension: Dimension,
    family: &str,
) -> Result<String, UtilityError> {
    match value {
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator } => {
            fraction_percentage(numerator, denominator, family)
        }
        ValueAst::Named(key) => {
            let token = match dimension {
                Dimension::Width | Dimension::MinWidth | Dimension::MaxWidth => {
                    theme.width(key).or_else(|| theme.spacing(key))
                }
                Dimension::Height | Dimension::MinHeight | Dimension::MaxHeight => {
                    theme.height(key).or_else(|| theme.spacing(key))
                }
            };
            if let Some(token) = token {
                validated_theme_value(token, family)
            } else if key.parse::<f64>().is_ok() {
                Ok(format!("calc(var(--spacing) * {key})"))
            } else {
                Err(UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key)))
            }
        }
    }
}

fn color_value(
    value: ValueAst<'_>,
    theme: &Theme,
    family: &str,
    _kind: ColorKind,
) -> Result<String, UtilityError> {
    match value {
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator } => {
            fraction_percentage(numerator, denominator, family)
        }
        ValueAst::Named(key) => {
            let value = theme.color(key).ok_or_else(|| {
                UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key))
            })?;
            validated_theme_value(value, family)
        }
    }
}

fn radius_value(value: ValueAst<'_>, theme: &Theme, family: &str) -> Result<String, UtilityError> {
    match value {
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator } => {
            fraction_percentage(numerator, denominator, family)
        }
        ValueAst::Named(key) => {
            let value = theme.radius(key).ok_or_else(|| {
                UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key))
            })?;
            validated_theme_value(value, family)
        }
    }
}

fn named_choice(
    value: Option<ValueAst<'_>>,
    family: &str,
    choices: &[&str],
) -> Result<String, UtilityError> {
    let value = required_value(value, family)?;
    let ValueAst::Named(name) = value else {
        return Err(UtilityError::new(
            UtilityErrorKind::InvalidValue,
            family,
            Some(value.content()),
        ));
    };
    if choices.contains(&name) {
        Ok(name.to_owned())
    } else {
        Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(name)))
    }
}

fn grid_columns_value(value: ValueAst<'_>, family: &str) -> Result<String, UtilityError> {
    match value {
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator: _ } => {
            Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(numerator)))
        }
        ValueAst::Named(columns) if columns.parse::<u16>().is_ok() => {
            Ok(format!("repeat({columns}, minmax(0, 1fr))"))
        }
        ValueAst::Named(columns) => {
            Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(columns)))
        }
    }
}

fn safe_arbitrary(content: &str, family: &str) -> Result<String, UtilityError> {
    if unsafe_css_fragment(content) {
        return Err(UtilityError::new(
            UtilityErrorKind::InvalidArbitraryValue,
            family,
            Some(content),
        ));
    }
    decode_arbitrary(content).map_err(|_| {
        UtilityError::new(UtilityErrorKind::InvalidArbitraryValue, family, Some(content))
    })
}

fn fraction_percentage(
    numerator: &str,
    denominator: &str,
    family: &str,
) -> Result<String, UtilityError> {
    let numerator = numerator.parse::<f64>().ok();
    let denominator = denominator.parse::<f64>().ok();
    let Some((numerator, denominator)) = numerator.zip(denominator) else {
        return Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some("fraction")));
    };
    if denominator == 0.0 {
        return Err(UtilityError::new(
            UtilityErrorKind::InvalidValue,
            family,
            Some("fraction with zero denominator"),
        ));
    }
    let percentage = numerator / denominator * 100.0;
    let rendered_number = if percentage.fract() == 0.0 {
        format!("{percentage:.0}")
    } else {
        format!("{percentage:.4}").trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    let rendered = format!("{rendered_number}%");
    Ok(rendered)
}

fn color_modifier(
    modifier: ValueAst<'_>,
    family: &str,
) -> Result<(String, ResolvedValue), UtilityError> {
    let content = match modifier {
        ValueAst::Named(value) => value.to_owned(),
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            decode_arbitrary(content).map_err(|_| {
                UtilityError::new(UtilityErrorKind::InvalidModifier, family, Some(content))
            })?
        }
        ValueAst::Fraction { numerator, denominator: _ } => {
            return Err(UtilityError::new(
                UtilityErrorKind::InvalidModifier,
                family,
                Some(numerator),
            ))
        }
    };
    let numeric = content.strip_suffix('%').unwrap_or(&content).parse::<f64>().ok();
    let Some(numeric) = numeric.filter(|value| (0.0..=100.0).contains(value)) else {
        return Err(UtilityError::new(UtilityErrorKind::InvalidModifier, family, Some(&content)));
    };
    let css = if numeric.fract() == 0.0 { format!("{numeric:.0}%") } else { format!("{numeric}%") };
    Ok((css.clone(), ResolvedValue::Percentage(css)))
}

fn resolve_namespace_value(
    value: ValueAst<'_>,
    theme: &Theme,
    namespace: &ValueNamespace,
    family: &str,
) -> Result<String, UtilityError> {
    match namespace {
        ValueNamespace::Spacing => spacing_value(value, theme, family),
        ValueNamespace::Color => color_value(value, theme, family, ColorKind::Text),
        ValueNamespace::Width => size_value(value, theme, Dimension::Width, family),
        ValueNamespace::Height => size_value(value, theme, Dimension::Height, family),
        ValueNamespace::Radius => radius_value(value, theme, family),
        ValueNamespace::FontFamily => named_theme_value(value, theme, "font-family", family),
        ValueNamespace::FontSize => named_theme_value(value, theme, "font-size", family),
        ValueNamespace::LineHeight => named_theme_value(value, theme, "line-height", family),
        ValueNamespace::LetterSpacing => named_theme_value(value, theme, "letter-spacing", family),
        ValueNamespace::Shadow => named_theme_value(value, theme, "shadow", family),
        ValueNamespace::Duration => named_theme_value(value, theme, "duration", family),
        ValueNamespace::Ease => named_theme_value(value, theme, "ease", family),
        ValueNamespace::ZIndex => match value {
            ValueAst::Named(key) => {
                let value = theme
                    .token("z-index", key)
                    .map(str::to_owned)
                    .or_else(|| key.parse::<i32>().ok().map(|_| key.to_owned()))
                    .ok_or_else(|| {
                        UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key))
                    })?;
                validated_theme_value(&value, family)
            }
            ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
                safe_arbitrary(content, family)
            }
            ValueAst::Fraction { numerator, .. } => {
                Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(numerator)))
            }
        },
        ValueNamespace::Number
        | ValueNamespace::Percentage
        | ValueNamespace::Integer
        | ValueNamespace::Length => primitive_value(value, namespace, family),
        ValueNamespace::Raw | ValueNamespace::Keyword => raw_value(value, family),
    }
}

fn named_theme_value(
    value: ValueAst<'_>,
    theme: &Theme,
    namespace: &str,
    family: &str,
) -> Result<String, UtilityError> {
    match value {
        ValueAst::Named(key) => {
            let value = theme.token(namespace, key).map(str::to_owned).ok_or_else(|| {
                UtilityError::new(UtilityErrorKind::UnknownThemeValue, family, Some(key))
            })?;
            validated_theme_value(&value, family)
        }
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, .. } => {
            Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(numerator)))
        }
    }
}

fn primitive_value(
    value: ValueAst<'_>,
    namespace: &ValueNamespace,
    family: &str,
) -> Result<String, UtilityError> {
    let is_arbitrary = value.is_arbitrary();
    let content = match value {
        ValueAst::Named(value) => value.to_owned(),
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)?
        }
        ValueAst::Fraction { numerator, denominator } => format!("{numerator}/{denominator}"),
    };
    let valid = match namespace {
        ValueNamespace::Integer => content.parse::<i32>().is_ok(),
        ValueNamespace::Number => content.parse::<f64>().is_ok(),
        ValueNamespace::Percentage => content
            .strip_suffix('%')
            .unwrap_or(&content)
            .parse::<f64>()
            .is_ok_and(|value| (0.0..=100.0).contains(&value)),
        _ => true,
    };
    if valid {
        if *namespace == ValueNamespace::Percentage && !is_arbitrary && !content.ends_with('%') {
            Ok(format!("{content}%"))
        } else {
            Ok(content)
        }
    } else {
        Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(&content)))
    }
}

fn static_theme_value(raw: &str, property: &str, fallback: &str, theme: &Theme) -> String {
    let token = if property == "font-size" {
        raw.strip_prefix("text-").and_then(|key| theme.token("font-size", key))
    } else if property == "font-family" {
        raw.strip_prefix("font-").and_then(|key| theme.token("font-family", key))
    } else if property == "line-height" {
        raw.strip_prefix("leading-").and_then(|key| theme.token("line-height", key))
    } else if property == "box-shadow" {
        let key =
            if raw == "shadow" { "DEFAULT" } else { raw.strip_prefix("shadow-").unwrap_or(raw) };
        theme.token("shadow", key)
    } else {
        None
    };
    token.unwrap_or(fallback).to_owned()
}

fn validated_theme_value(value: &str, family: &str) -> Result<String, UtilityError> {
    if unsafe_css_fragment(value) {
        return Err(UtilityError::new(UtilityErrorKind::InvalidDeclaration, family, Some(value)));
    }
    Ok(value.to_owned())
}

fn raw_value(value: ValueAst<'_>, family: &str) -> Result<String, UtilityError> {
    match value {
        ValueAst::Named(value) => Ok(value.to_owned()),
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, denominator } => Ok(format!("{numerator}/{denominator}")),
    }
}

fn shadow_modifier(
    shadow: &str,
    modifier: ValueAst<'_>,
    family: &str,
) -> Result<(String, ResolvedValue), UtilityError> {
    let (alpha, typed_modifier) = color_modifier(modifier, family)?;
    let mut output = String::with_capacity(shadow.len() + alpha.len() + 3);
    let mut cursor = 0;
    let mut replaced = false;
    while cursor < shadow.len() {
        let Some((relative, open_end)) = shadow[cursor..].char_indices().find_map(|(offset, _)| {
            let tail = shadow.get(cursor + offset..)?;
            if tail.starts_with("rgb(") || tail.starts_with("rgba(") {
                Some((offset, cursor + offset + tail.find('(')? + 1))
            } else {
                None
            }
        }) else {
            output.push_str(&shadow[cursor..]);
            break;
        };
        let function_start = cursor + relative;
        let Some(close) = matching_parenthesis(shadow, open_end) else {
            return Err(UtilityError::new(UtilityErrorKind::InvalidModifier, family, Some(shadow)));
        };
        output.push_str(&shadow[cursor..function_start]);
        let function_name_end = open_end;
        output.push_str(&shadow[function_start..function_name_end]);
        let inner = &shadow[open_end..close];
        output.push_str(&shadow_color_with_alpha(inner, &alpha));
        output.push(')');
        cursor = close + 1;
        replaced = true;
    }
    if !replaced {
        return Err(UtilityError::new(UtilityErrorKind::InvalidModifier, family, Some(shadow)));
    }
    Ok((output, typed_modifier))
}

fn matching_parenthesis(input: &str, open_end: usize) -> Option<usize> {
    let mut depth = 1_u32;
    for (relative, character) in input.get(open_end..)?.char_indices() {
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open_end + relative);
                }
            }
            _ => {}
        }
    }
    None
}

fn shadow_color_with_alpha(color: &str, alpha: &str) -> String {
    if let Some(index) = color.rfind('/') {
        return format!("{} / {}", color[..index].trim_end(), alpha);
    }
    if let Some(index) = color.rfind(',') {
        return format!("{},{}", color[..index].trim_end(), alpha);
    }
    format!("{} / {alpha}", color.trim_end())
}

fn resolved_value_for_namespace(value: String, namespace: &ValueNamespace) -> ResolvedValue {
    match namespace {
        ValueNamespace::Spacing
        | ValueNamespace::Width
        | ValueNamespace::Height
        | ValueNamespace::Radius => ResolvedValue::Length(value),
        ValueNamespace::Color => ResolvedValue::Color(value),
        ValueNamespace::FontSize | ValueNamespace::LineHeight | ValueNamespace::LetterSpacing => {
            ResolvedValue::Length(value)
        }
        ValueNamespace::Duration => ResolvedValue::Time(value),
        ValueNamespace::Number => ResolvedValue::Number(value),
        ValueNamespace::Length => ResolvedValue::Length(value),
        ValueNamespace::Integer | ValueNamespace::ZIndex => ResolvedValue::Integer(value),
        ValueNamespace::Percentage => ResolvedValue::Percentage(value),
        ValueNamespace::FontFamily | ValueNamespace::Ease | ValueNamespace::Keyword => {
            ResolvedValue::Keyword(value)
        }
        ValueNamespace::Raw | ValueNamespace::Shadow => ResolvedValue::RawCss(value),
    }
}

fn safe_property(property: &str) -> bool {
    let bytes = property.as_bytes();
    let start = if bytes.starts_with(b"--") {
        2
    } else if bytes.starts_with(b"-") {
        1
    } else {
        0
    };
    bytes.get(start).is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        && bytes[start..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_'))
}

fn unsafe_css_fragment(content: &str) -> bool {
    let contains_style_close = content
        .as_bytes()
        .windows(b"</style".len())
        .any(|window| window.eq_ignore_ascii_case(b"</style"));
    if contains_style_close {
        return true;
    }
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in content.char_indices() {
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
        if content[offset..].starts_with("/*") || content[offset..].starts_with("*/") {
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

fn declaration_with_dependencies(
    property: impl Into<String>,
    value: impl Into<String>,
    important: bool,
) -> CssDeclaration {
    let value = value.into();
    let mut declaration = CssDeclaration::new(property, value.clone()).with_important(important);
    for name in css_dependencies(&value) {
        declaration = declaration.with_dependency(name);
    }
    declaration
}

fn css_dependencies(value: &str) -> Vec<String> {
    let mut dependencies = BTreeMap::new();
    let mut remaining = value;
    while let Some(start) = remaining.find("var(--") {
        let suffix = &remaining[start + 4..];
        let end = suffix
            .char_indices()
            .find(|(_, character)| {
                !character.is_ascii_alphanumeric() && !matches!(*character, '-' | '_')
            })
            .map_or(suffix.len(), |(offset, _)| offset);
        let name = &suffix[..end];
        if name.starts_with("--") {
            dependencies.insert(name.to_owned(), ());
        }
        remaining = suffix.get(end..).unwrap_or_default();
    }
    dependencies.into_keys().collect()
}

fn negate(value: String) -> String {
    if value == "0px" || value == "0" {
        value
    } else if value.starts_with("calc(") {
        format!("calc(-1 * {value})")
    } else {
        format!("-{value}")
    }
}

fn spacing_declarations(
    margin: bool,
    edge: SpacingEdge,
    value: String,
    important: bool,
) -> Vec<CssDeclaration> {
    let property = if margin { "margin" } else { "padding" };
    let mut declarations = Vec::new();
    match edge {
        SpacingEdge::All => declarations.push(CssDeclaration::new(property, value.clone())),
        SpacingEdge::X => {
            declarations.push(CssDeclaration::new(format!("{property}-left"), value.clone()));
            declarations.push(CssDeclaration::new(format!("{property}-right"), value.clone()));
        }
        SpacingEdge::Y => {
            declarations.push(CssDeclaration::new(format!("{property}-top"), value.clone()));
            declarations.push(CssDeclaration::new(format!("{property}-bottom"), value.clone()));
        }
        SpacingEdge::Top => {
            declarations.push(CssDeclaration::new(format!("{property}-top"), value.clone()))
        }
        SpacingEdge::Right => {
            declarations.push(CssDeclaration::new(format!("{property}-right"), value.clone()))
        }
        SpacingEdge::Bottom => {
            declarations.push(CssDeclaration::new(format!("{property}-bottom"), value.clone()))
        }
        SpacingEdge::Left => {
            declarations.push(CssDeclaration::new(format!("{property}-left"), value.clone()))
        }
    }
    declarations.into_iter().map(|declaration| declaration.with_important(important)).collect()
}

fn gap_declarations(edge: SpacingEdge, value: String, important: bool) -> Vec<CssDeclaration> {
    let property = match edge {
        SpacingEdge::All => "gap",
        SpacingEdge::X => "column-gap",
        SpacingEdge::Y => "row-gap",
        SpacingEdge::Top | SpacingEdge::Right | SpacingEdge::Bottom | SpacingEdge::Left => "gap",
    };
    vec![CssDeclaration::new(property, value).with_important(important)]
}

fn border_width_value(value: ValueAst<'_>, family: &str) -> Result<String, UtilityError> {
    match value {
        ValueAst::Named("0") => Ok("0px".to_owned()),
        ValueAst::Named("DEFAULT") => Ok("1px".to_owned()),
        ValueAst::Named(value) if value.parse::<u16>().is_ok() => Ok(format!("{value}px")),
        ValueAst::Arbitrary { content, .. } | ValueAst::TypedArbitrary { content, .. } => {
            safe_arbitrary(content, family)
        }
        ValueAst::Fraction { numerator, .. } => {
            Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(numerator)))
        }
        ValueAst::Named(value) => {
            Err(UtilityError::new(UtilityErrorKind::InvalidValue, family, Some(value)))
        }
    }
}

fn border_declarations(edge: SpacingEdge, value: String, important: bool) -> Vec<CssDeclaration> {
    let properties = match edge {
        SpacingEdge::All => vec!["border-width"],
        SpacingEdge::X => vec!["border-left-width", "border-right-width"],
        SpacingEdge::Y => vec!["border-top-width", "border-bottom-width"],
        SpacingEdge::Top => vec!["border-top-width"],
        SpacingEdge::Right => vec!["border-right-width"],
        SpacingEdge::Bottom => vec!["border-bottom-width"],
        SpacingEdge::Left => vec!["border-left-width"],
    };
    properties
        .into_iter()
        .map(|property| CssDeclaration::new(property, value.clone()).with_important(important))
        .collect()
}

fn radius_declarations(edge: SpacingEdge, value: String, important: bool) -> Vec<CssDeclaration> {
    let properties = match edge {
        SpacingEdge::All => vec!["border-radius"],
        SpacingEdge::X => vec![
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-left-radius",
            "border-bottom-right-radius",
        ],
        SpacingEdge::Y => vec![
            "border-top-left-radius",
            "border-top-right-radius",
            "border-bottom-left-radius",
            "border-bottom-right-radius",
        ],
        SpacingEdge::Top => vec!["border-top-left-radius", "border-top-right-radius"],
        SpacingEdge::Right => vec!["border-top-right-radius", "border-bottom-right-radius"],
        SpacingEdge::Bottom => vec!["border-bottom-left-radius", "border-bottom-right-radius"],
        SpacingEdge::Left => vec!["border-top-left-radius", "border-bottom-left-radius"],
    };
    properties
        .into_iter()
        .map(|property| CssDeclaration::new(property, value.clone()).with_important(important))
        .collect()
}

/// Escapes a candidate as a CSS class selector.
///
/// Serialization follows the CSS Syntax identifier rules (the same algorithm
/// as `CSS.escape`): a leading digit becomes a hexadecimal code-point escape,
/// a lone leading `-` is backslash-escaped, a digit after a leading `-` is
/// code-point escaped, NULL becomes the replacement character, other control
/// characters become code-point escapes, ASCII punctuation is
/// backslash-escaped, and every other character (including non-ASCII) is
/// emitted literally.
#[must_use]
pub fn escape_class_selector(candidate: &str) -> String {
    let mut selector = String::from(".");
    let mut chars = candidate.chars().peekable();
    let mut index = 0usize;
    while let Some(character) = chars.next() {
        if character == '\0' {
            selector.push(char::REPLACEMENT_CHARACTER);
        } else if (index == 0 && character.is_ascii_digit())
            || (index == 1 && character.is_ascii_digit() && candidate.starts_with('-'))
        {
            push_code_point_escape(&mut selector, character);
        } else if index == 0 && character == '-' && chars.peek().is_none() {
            selector.push_str("\\-");
        } else if character.is_alphanumeric()
            || character == '-'
            || character == '_'
            || !character.is_ascii()
        {
            selector.push(character);
        } else if character.is_ascii_control() {
            push_code_point_escape(&mut selector, character);
        } else {
            selector.push('\\');
            selector.push(character);
        }
        index += 1;
    }
    selector
}

fn push_code_point_escape(selector: &mut String, character: char) {
    selector.push('\\');
    selector.push_str(&format!("{:x} ", character as u32));
}

#[cfg(test)]
mod tests {
    use utilitycss_syntax::parse;
    use utilitycss_theme::Theme;

    use super::{
        escape_class_selector, resolve, UtilityDefinition, UtilityErrorKind, UtilityRegistry,
        ValueNamespace,
    };

    #[test]
    fn resolves_static_spacing_and_color_utilities() {
        let registry = UtilityRegistry::new();
        let theme = Theme::default();

        let padding = resolve(&parse("p-4").expect("valid candidate"), &theme, &registry)
            .expect("known spacing value");
        let color = resolve(&parse("bg-red-500").expect("valid candidate"), &theme, &registry)
            .expect("known color value");

        assert_eq!(padding.selector(), ".p-4");
        assert_eq!(padding.declarations()[0].value(), "1rem");
        assert_eq!(color.declarations()[0].value(), "#ef4444");
    }

    #[test]
    fn resolves_variants_ready_selectors_and_arbitrary_values() {
        let registry = UtilityRegistry::new();
        let theme = Theme::default();
        let candidate = parse("!hover:bg-[rgb(1_2_3)]").expect("valid candidate");
        let resolved = resolve(&candidate, &theme, &registry).expect("safe arbitrary color");

        assert_eq!(resolved.selector(), r".\!hover\:bg-\[rgb\(1_2_3\)\]");
        assert_eq!(resolved.declarations()[0].value(), "rgb(1 2 3)");
        assert!(resolved.declarations()[0].is_important());
    }

    #[test]
    fn rejects_unsafe_arbitrary_values() {
        let registry = UtilityRegistry::new();
        let error = resolve(
            &parse("w-[1rem;display:block]").expect("brackets are structurally valid"),
            &Theme::default(),
            &registry,
        )
        .expect_err("declaration delimiters are unsafe");

        assert_eq!(error.kind(), UtilityErrorKind::InvalidArbitraryValue);
    }

    #[test]
    fn rejects_control_comment_and_style_termination_sequences() {
        for candidate in
            ["w-[1rem\u{0000}]", "w-[1rem\u{0009}]", "w-[1rem/*comment*/]", "w-[1rem</STYLE>]"]
        {
            let error = resolve(
                &parse(candidate).expect("candidate structure is valid"),
                &Theme::default(),
                &UtilityRegistry::default(),
            )
            .expect_err("unsafe CSS value is rejected");
            assert_eq!(error.kind(), UtilityErrorKind::InvalidArbitraryValue);
        }
    }

    #[test]
    fn custom_static_definitions_extend_the_registry() {
        let mut registry = UtilityRegistry::new();
        registry.register(
            "content-center",
            UtilityDefinition::static_declaration("place-content", "center", 63),
        );
        let resolved = resolve(
            &parse("content-center").expect("valid candidate"),
            &Theme::default(),
            &registry,
        )
        .expect("custom utility is registered");

        assert_eq!(resolved.declarations()[0].property(), "place-content");
    }

    #[test]
    fn rejects_unsafe_custom_static_declarations() {
        let mut registry = UtilityRegistry::new();
        registry.register(
            "unsafe",
            UtilityDefinition::static_declaration("display; color", "block", 99),
        );

        let error =
            resolve(&parse("unsafe").expect("candidate is valid"), &Theme::default(), &registry)
                .expect_err("unsafe custom declaration is rejected");

        assert_eq!(error.kind(), UtilityErrorKind::InvalidDeclaration);
        assert!(registry.validate().is_err());
    }

    #[test]
    fn utility_fingerprints_include_utility_kind_and_fields() {
        assert_ne!(
            UtilityDefinition::static_declaration("display", "block", 1).fingerprint(),
            UtilityDefinition::static_declaration("display", "flex", 1).fingerprint()
        );
        assert_ne!(
            UtilityDefinition::static_declaration("display", "block", 1).fingerprint(),
            UtilityDefinition::functional("display", ValueNamespace::Keyword, false, false, 1,)
                .fingerprint()
        );
    }

    #[test]
    fn rejects_unsafe_theme_values_and_property_identifiers() {
        let theme = Theme::builder().color("unsafe", "red; display:block").build();
        let error = resolve(
            &parse("bg-unsafe").expect("candidate is syntactically valid"),
            &theme,
            &UtilityRegistry::default(),
        )
        .expect_err("theme values cannot terminate declarations");
        assert_eq!(error.kind(), UtilityErrorKind::InvalidDeclaration);

        let error = resolve(
            &parse("rounded").expect("default radius candidate is valid"),
            &Theme::builder().radius("DEFAULT", "0px;display:block").build(),
            &UtilityRegistry::default(),
        )
        .expect_err("the default radius token is validated at its sink");
        assert_eq!(error.kind(), UtilityErrorKind::InvalidDeclaration);

        let error = resolve(
            &parse("[1color:red]").expect("arbitrary property structure is valid"),
            &Theme::default(),
            &UtilityRegistry::default(),
        )
        .expect_err("property names must start with a CSS identifier character");
        assert_eq!(error.kind(), UtilityErrorKind::InvalidDeclaration);
    }

    #[test]
    fn escapes_css_punctuation() {
        assert_eq!(escape_class_selector("md:hover:bg-red-500/50"), r".md\:hover\:bg-red-500\/50");
    }

    #[test]
    fn escapes_leading_digit_breakpoints() {
        assert_eq!(escape_class_selector("2xl:p-4"), r".\32 xl\:p-4");
        assert_eq!(escape_class_selector("2xl:grid"), r".\32 xl\:grid");
        assert_eq!(escape_class_selector("2xl"), r".\32 xl");
    }

    #[test]
    fn escapes_lone_dash_and_dash_digit_starts() {
        assert_eq!(escape_class_selector("-"), r".\-");
        assert_eq!(escape_class_selector("-2x"), r".-\32 x");
        assert_eq!(escape_class_selector("-md:p-4"), r".-md\:p-4");
    }

    #[test]
    fn escapes_control_and_null_characters() {
        assert_eq!(escape_class_selector("a\x01b"), ".a\\1 b");
        let nul_escaped = escape_class_selector("a\0b");
        assert!(nul_escaped.starts_with(".a"));
        assert_eq!(nul_escaped.chars().nth(2), Some(char::REPLACEMENT_CHARACTER));
        assert!(nul_escaped.ends_with("b"));
        assert_eq!(escape_class_selector("a\x7fb"), ".a\\7f b");
    }

    #[test]
    fn preserves_non_ascii_identifier_characters() {
        assert_eq!(escape_class_selector("café"), ".café");
        assert_eq!(escape_class_selector("日本語"), ".日本語");
    }

    #[test]
    fn resolves_vnext_layout_typography_and_grid_families() {
        let registry = UtilityRegistry::default();
        let theme = Theme::default();
        for (candidate, property, expected) in [
            ("aspect-square", "aspect-ratio", "1 / 1"),
            ("overflow-hidden", "overflow", "hidden"),
            ("font-bold", "font-weight", "700"),
            ("text-center", "text-align", "center"),
            ("size-4", "width", "1rem"),
            ("col-span-2", "grid-column", "span 2 / span 2"),
            ("opacity-50", "opacity", "50%"),
        ] {
            let resolved =
                resolve(&parse(candidate).expect("candidate is valid"), &theme, &registry)
                    .expect("extended utility is registered");
            assert!(resolved
                .declarations()
                .iter()
                .any(|declaration| declaration.property() == property
                    && declaration.value() == expected));
        }
    }

    #[test]
    fn consumes_color_modifiers_and_decodes_arbitrary_properties() {
        let registry = UtilityRegistry::default();
        let color = resolve(
            &parse("bg-red-500/50").expect("candidate is valid"),
            &Theme::default(),
            &registry,
        )
        .expect("alpha modifier is supported");
        assert_eq!(color.modifier().map(|value| value.css()), Some("50%"));
        assert!(color.declarations()[0].value().contains("50%"));

        let property = resolve(
            &parse("[color:light-dark(#000,_#fff)]").expect("arbitrary property is valid"),
            &Theme::default(),
            &registry,
        )
        .expect("arbitrary property is safe");
        assert_eq!(property.declarations()[0].property(), "color");
        assert_eq!(property.declarations()[0].value(), "light-dark(#000, #fff)");
    }

    #[test]
    fn typed_color_values_select_semantic_background_image_lowering() {
        let resolved = resolve(
            &parse("bg-[image:url('/hero_art.png')]").expect("candidate is valid"),
            &Theme::default(),
            &UtilityRegistry::default(),
        )
        .expect("typed background image is supported");

        assert_eq!(resolved.declarations()[0].property(), "background-image");
        assert_eq!(resolved.declarations()[0].value(), "url('/hero_art.png')");
        assert!(resolve(
            &parse("bg-[length:2rem]").expect("candidate is valid"),
            &Theme::default(),
            &UtilityRegistry::default(),
        )
        .is_err());
        assert!(resolve(
            &parse("w-[color:red]").expect("candidate is valid"),
            &Theme::default(),
            &UtilityRegistry::default(),
        )
        .is_err());
    }

    #[test]
    fn consumes_shadow_alpha_modifiers_and_exposes_modifier_metadata() {
        let registry = UtilityRegistry::new();
        let resolved = resolve(
            &parse("shadow-xl/30").expect("shadow modifier is syntactically valid"),
            &Theme::default(),
            &registry,
        )
        .expect("shadow alpha modifier is supported");

        assert!(resolved.declarations()[0].value().contains("rgb(0 0 0 / 30%)"));
        assert_eq!(
            registry
                .descriptor("shadow-xl")
                .expect("shadow descriptor exists")
                .modifier_schema
                .kind,
            super::ModifierKind::UtilitySpecific
        );
    }

    #[test]
    fn theme_overrides_backed_static_families() {
        let theme = Theme::builder()
            .font_size("lg", "2rem")
            .font_family("sans", "Custom Sans")
            .line_height("tight", "1.1")
            .shadow("xl", "0 0 4px #000")
            .build();
        let registry = UtilityRegistry::default();

        assert_eq!(
            resolve(&parse("text-lg").expect("text size is valid"), &theme, &registry)
                .expect("text size resolves")
                .declarations()[0]
                .value(),
            "2rem"
        );
        assert_eq!(
            resolve(&parse("font-sans").expect("font family is valid"), &theme, &registry)
                .expect("font family resolves")
                .declarations()[0]
                .value(),
            "Custom Sans"
        );
        assert_eq!(
            resolve(&parse("leading-tight").expect("line height is valid"), &theme, &registry)
                .expect("line height resolves")
                .declarations()[0]
                .value(),
            "1.1"
        );
    }

    #[test]
    fn composite_utilities_expose_variable_dependencies() {
        assert!(UtilityRegistry::default().contains("bg-gradient-to-r"));
        let parsed = parse("bg-gradient-to-r").expect("composite candidate is valid");
        assert_eq!(parsed.utility().raw(), "bg-gradient-to-r");
        assert_eq!(parsed.utility().family(), "bg");
        assert!(matches!(
            UtilityRegistry::default().get(parsed.utility().raw()),
            Some(UtilityDefinition::Composite { .. })
        ));
        let resolved = resolve(&parsed, &Theme::default(), &UtilityRegistry::default())
            .expect("gradient utility is registered");
        assert_eq!(resolved.declarations()[0].dependencies()[0].name, "--tw-gradient-stops");
        let descriptor = UtilityRegistry::default()
            .descriptor("bg-gradient-to-r")
            .expect("descriptor is registered");
        assert_eq!(descriptor.dependencies, vec!["--tw-gradient-stops"]);
    }

    #[test]
    fn unsupported_modifiers_are_reported_instead_of_dropped() {
        let error = resolve(
            &parse("p-4/2").expect("candidate is syntactically valid"),
            &Theme::default(),
            &UtilityRegistry::default(),
        )
        .expect_err("padding does not define a modifier");
        assert_eq!(error.kind(), UtilityErrorKind::UnsupportedModifier);
    }
}
