//! Thin WebAssembly bindings over the runtime-independent compiler facade.
//!
//! The surface mirrors the N-API adapter: [`WasmCompiler::new`] accepts the same
//! `pretty`/`config`/`browserTarget` triple, candidate and stylesheet methods share
//! shapes, and introspection payloads (`explain`, `validate`, `capabilities`) are
//! JSON strings in both adapters so hosts parse one stable encoding.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use utilitycss_compiler::{CandidateInput, Compiler, CompilerConfig, SourceInput};
use utilitycss_config::{parse_css, parse_json, ConfigFile};
use utilitycss_css_ir::{BrowserTarget, CssSerializationMode};
use utilitycss_extractor::{extract_for_framework, Framework};
use utilitycss_scanner::ExtractionMode;
use utilitycss_span::{validate_source_len, SourceId, Span};
use utilitycss_stylesheet::{transform_stylesheet, StylesheetInput};
use utilitycss_swc::{extract as extract_swc, SourceKind as SwcSourceKind};
use wasm_bindgen::prelude::*;

/// A reusable WebAssembly-facing compiler instance.
#[wasm_bindgen]
pub struct WasmCompiler {
    inner: Compiler,
}

#[derive(Serialize)]
struct WasmDiagnostic {
    severity: String,
    code: String,
    message: String,
    source: Option<String>,
    start: Option<u32>,
    end: Option<u32>,
    help: Option<String>,
    explanation: Option<String>,
    suggestions: Vec<String>,
}

#[derive(Serialize)]
struct WasmStylesheetResult {
    css: String,
    diagnostics: Vec<WasmDiagnostic>,
}

/// Build work counters serialized with the same camel-case keys as the N-API adapter.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmBuildStats {
    sources_scanned: usize,
    bytes_scanned: usize,
    candidates_found: usize,
    unique_candidates: usize,
    candidates_parsed: usize,
    cache_hits: usize,
    rules_generated: usize,
    rules_removed: usize,
}

#[derive(Serialize)]
struct WasmBuildResult {
    css: String,
    diagnostics: Vec<WasmDiagnostic>,
    stats: WasmBuildStats,
}

fn wasm_diagnostic(diagnostic: &utilitycss_diagnostics::Diagnostic) -> WasmDiagnostic {
    WasmDiagnostic {
        severity: format!("{:?}", diagnostic.severity()).to_ascii_lowercase(),
        code: diagnostic.code().to_string(),
        message: diagnostic.message().to_owned(),
        source: diagnostic.source().map(ToString::to_string),
        start: diagnostic.span().map(|span| span.start()),
        end: diagnostic.span().map(|span| span.end()),
        help: diagnostic.help().map(str::to_owned),
        explanation: diagnostic.explanation().map(str::to_owned),
        suggestions: diagnostic
            .suggestions()
            .iter()
            .map(|suggestion| suggestion.replacement.clone())
            .collect(),
    }
}

fn wasm_build_result(output: &utilitycss_compiler::CompileOutput) -> WasmBuildResult {
    let stats = output.stats();
    WasmBuildResult {
        css: output.css().to_owned(),
        diagnostics: output.diagnostics().iter().map(wasm_diagnostic).collect(),
        stats: WasmBuildStats {
            sources_scanned: stats.sources_scanned(),
            bytes_scanned: stats.bytes_scanned(),
            candidates_found: stats.candidates_found(),
            unique_candidates: stats.unique_candidates(),
            candidates_parsed: stats.candidates_parsed(),
            cache_hits: stats.cache_hits(),
            rules_generated: stats.rules_generated(),
            rules_removed: stats.rules_removed(),
        },
    }
}

#[derive(Deserialize)]
struct WasmCandidateInput {
    raw: String,
    start: u32,
    end: u32,
    #[serde(default, rename = "extractionMode")]
    extraction_mode: Option<String>,
}

/// A statically extracted candidate returned across the WASM boundary.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmCandidate {
    raw: String,
    start: u32,
    end: u32,
    extraction_mode: Option<String>,
}

#[wasm_bindgen]
impl WasmCompiler {
    /// Creates a compiler with optional readable output, declarative config, and browser target.
    ///
    /// Every argument is optional. `config_source` accepts the same declarative JSON or
    /// CSS-first configuration text as the N-API adapter; an explicit `browser_target`
    /// overrides the config value, defaulting to `modern`.
    #[wasm_bindgen(constructor)]
    pub fn new(
        pretty: Option<bool>,
        config_source: Option<String>,
        browser_target: Option<String>,
    ) -> Result<Self, JsValue> {
        build_compiler(pretty, config_source.as_deref(), browser_target.as_deref())
            .map(|inner| Self { inner })
            .map_err(|error| JsValue::from_str(&error))
    }

    /// Inserts or replaces one source unit.
    ///
    /// `path` selects host-language extraction the same way the N-API adapter does.
    /// Host applications that can parse their source language should use
    /// [`Self::update_source_with_candidates`] to preserve the same candidate-selection
    /// semantics as the native AST adapters.
    #[wasm_bindgen(js_name = updateSource)]
    pub fn update_source(
        &mut self,
        id: String,
        content: String,
        path: Option<String>,
    ) -> Result<(), JsValue> {
        let source_id = SourceId::new(id);
        let source = match path {
            Some(path) => SourceInput::new(source_id, content).with_path(path),
            None => SourceInput::new(source_id, content),
        };
        self.inner.update_source(source).map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Inserts or replaces one source unit using host-selected candidate spans.
    ///
    /// Each item must contain `raw`, `start`, and `end` fields. `extractionMode` is optional;
    /// accepted values are `text`, `static`, `ast`, and `hybrid`.
    #[wasm_bindgen(js_name = updateSourceWithCandidates)]
    pub fn update_source_with_candidates(
        &mut self,
        id: String,
        content: String,
        path: Option<String>,
        candidates: JsValue,
    ) -> Result<(), JsValue> {
        let inputs = serde_wasm_bindgen::from_value::<Vec<WasmCandidateInput>>(candidates)
            .map_err(|error| JsValue::from_str(&format!("invalid candidate list: {error}")))?
            .into_iter()
            .map(|candidate| {
                let span = Span::new(candidate.start, candidate.end)
                    .ok_or_else(|| "candidate start must not exceed candidate end".to_owned())?;
                let input = CandidateInput::new(candidate.raw, span);
                match candidate.extraction_mode.as_deref() {
                    None => Ok(input),
                    Some(name) => ExtractionMode::parse(name)
                        .map(|mode| input.with_extraction_mode(mode))
                        .ok_or_else(|| format!("unknown extraction mode `{name}`")),
                }
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(|error| JsValue::from_str(&error))?;

        let source_id = SourceId::new(id);
        let source = match path {
            Some(path) => SourceInput::new(source_id, content).with_path(path),
            None => SourceInput::new(source_id, content),
        };
        self.inner
            .update_source_with_candidates(source, inputs)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Extracts static class candidates using SWC or the selected framework adapter.
    ///
    /// The returned array shares the N-API `JsCandidate` shape (`raw`, `start`, `end`,
    /// `extractionMode`).
    #[wasm_bindgen(js_name = extractCandidates)]
    pub fn extract_candidates(
        &self,
        content: String,
        path: Option<String>,
    ) -> Result<JsValue, JsValue> {
        validate_source_len(content.len())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let candidates = match source_kind(path.as_deref()) {
            SourceKind::JavaScript(kind) => extract_swc(&content, kind)
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .into_iter()
                .map(|candidate| (candidate.raw(), candidate.span(), ExtractionMode::Ast))
                .collect::<Vec<_>>(),
            SourceKind::Framework(framework) => extract_for_framework(&content, framework)
                .map_err(|error| JsValue::from_str(&error.to_string()))?
                .into_iter()
                .map(|candidate| (candidate.raw(), candidate.span(), ExtractionMode::Static))
                .collect(),
        };
        let out = candidates
            .into_iter()
            .map(|(raw, span, mode)| WasmCandidate {
                raw: raw.to_owned(),
                start: span.start(),
                end: span.end(),
                extraction_mode: Some(mode.as_str().to_owned()),
            })
            .collect::<Vec<_>>();
        serde_wasm_bindgen::to_value(&out).map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Removes one source unit and returns whether it existed.
    #[wasm_bindgen(js_name = removeSource)]
    pub fn remove_source(&mut self, id: String) -> bool {
        self.inner.remove_source(&SourceId::new(id))
    }

    /// Builds the current sources and returns CSS, structured diagnostics, and
    /// work counters, matching the N-API adapter result shape.
    pub fn build(&mut self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&wasm_build_result(&self.inner.build()))
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Explains one candidate and returns the stable JSON introspection payload.
    pub fn explain(&self, candidate: String) -> Result<String, JsValue> {
        serde_json::to_string(&self.inner.explain_candidate(&candidate))
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Validates one candidate and returns the stable JSON validation payload.
    pub fn validate(&self, candidate: String) -> Result<String, JsValue> {
        serde_json::to_string(
            &self.inner.validate(utilitycss_compiler::ExplainRequest::new(&candidate)),
        )
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Returns the active machine-readable capability manifest as JSON.
    pub fn capabilities(&self) -> String {
        self.inner.capability_manifest_json()
    }

    /// Transforms authored CSS and returns CSS plus structured diagnostics.
    #[wasm_bindgen(js_name = transformStylesheet)]
    pub fn transform_stylesheet(
        &mut self,
        id: String,
        content: String,
        path: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let input = match path {
            Some(path) => StylesheetInput::new(SourceId::new(id), content).with_path(path),
            None => StylesheetInput::new(SourceId::new(id), content),
        };
        let output = transform_stylesheet(&mut self.inner, input);
        let diagnostics = output.diagnostics().iter().map(wasm_diagnostic).collect();
        serde_wasm_bindgen::to_value(&WasmStylesheetResult {
            css: output.css().to_owned(),
            diagnostics,
        })
        .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn parse_config_source(source: &str) -> Result<ConfigFile, String> {
    let trimmed = source.trim_start();
    let result = if trimmed.starts_with('{') { parse_json(source) } else { parse_css(source) };
    result.map_err(|error| error.to_string())
}

/// Builds the core compiler from constructor arguments without touching
/// JavaScript values, so host tests can cover argument handling.
fn build_compiler(
    pretty: Option<bool>,
    config_source: Option<&str>,
    browser_target: Option<&str>,
) -> Result<Compiler, String> {
    let config = config_source.map(parse_config_source).transpose()?;
    let target = match browser_target {
        Some(target) => BrowserTarget::parse(target)
            .ok_or_else(|| format!("unknown browser target `{target}`"))?,
        None => config.as_ref().map(ConfigFile::browser_target).unwrap_or(BrowserTarget::Modern),
    };
    let mode = pretty.map_or_else(
        || {
            config
                .as_ref()
                .map(ConfigFile::serialization_mode)
                .unwrap_or(CssSerializationMode::Minified)
        },
        |pretty| {
            if pretty {
                CssSerializationMode::Pretty
            } else {
                CssSerializationMode::Minified
            }
        },
    );
    let mut compiler_config =
        CompilerConfig::new().with_serialization_mode(mode).with_browser_target(target);
    if let Some(config) = config {
        compiler_config = compiler_config
            .with_theme(config.theme().clone())
            .with_utility_registry(config.utilities().clone())
            .with_variant_registry(config.variants().clone())
            .with_preset_name(config.preset().name());
    }
    Ok(Compiler::new(compiler_config))
}

enum SourceKind {
    JavaScript(SwcSourceKind),
    Framework(Framework),
}

fn source_kind(path: Option<&str>) -> SourceKind {
    let extension = path
        .map(std::path::Path::new)
        .and_then(std::path::Path::extension)
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("js") | Some("mjs") | Some("cjs") => SourceKind::JavaScript(SwcSourceKind::JavaScript),
        Some("jsx") | Some("mjsx") | Some("cjsx") => SourceKind::JavaScript(SwcSourceKind::Jsx),
        Some("ts") | Some("mts") | Some("cts") => SourceKind::JavaScript(SwcSourceKind::TypeScript),
        Some("tsx") | Some("mtsx") | Some("ctsx") => SourceKind::JavaScript(SwcSourceKind::Tsx),
        Some("vue") => SourceKind::Framework(Framework::Vue),
        Some("svelte") => SourceKind::Framework(Framework::Svelte),
        Some("astro") => SourceKind::Framework(Framework::Astro),
        _ => SourceKind::Framework(Framework::Html),
    }
}

#[cfg(test)]
mod tests {
    use utilitycss_compiler::{Compiler, CompilerConfig, SourceInput};
    use utilitycss_diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSuggestion, Severity};
    use utilitycss_span::{SourceId, Span};

    use super::{wasm_build_result, wasm_diagnostic};

    #[test]
    fn diagnostic_mapping_preserves_structured_fields() {
        let diagnostic =
            Diagnostic::new(Severity::Error, DiagnosticCode::new("test.code"), "broken")
                .with_source(SourceId::new("src/app.html"))
                .with_span(Span::new(4, 8).expect("span is ordered"))
                .with_help("fix it")
                .with_explanation("because reasons")
                .with_suggestion(DiagnosticSuggestion::new("p-4", "use padding"));

        let mapped = wasm_diagnostic(&diagnostic);

        assert_eq!(mapped.severity, "error");
        assert_eq!(mapped.code, "test.code");
        assert_eq!(mapped.message, "broken");
        assert_eq!(mapped.source.as_deref(), Some("src/app.html"));
        assert_eq!((mapped.start, mapped.end), (Some(4), Some(8)));
        assert_eq!(mapped.help.as_deref(), Some("fix it"));
        assert_eq!(mapped.explanation.as_deref(), Some("because reasons"));
        assert_eq!(mapped.suggestions, vec!["p-4".to_owned()]);
    }

    #[test]
    fn build_result_keeps_diagnostics_and_camel_case_stats() {
        let mut compiler = Compiler::new(CompilerConfig::new());
        compiler
            .update_source(SourceInput::new(
                SourceId::new("src/app.html"),
                "<div class=\"p-4 p-[]\"></div>",
            ))
            .expect("source is valid");

        let result = wasm_build_result(&compiler.build());

        assert!(result.css.contains("padding:1rem"), "css: {}", result.css);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.stats.sources_scanned, 1);

        let json = serde_json::to_value(&result).expect("result serializes");
        assert_eq!(json["css"], serde_json::Value::String(result.css.clone()));
        assert_eq!(json["diagnostics"].as_array().map(Vec::len), Some(1));
        assert_eq!(json["stats"]["sourcesScanned"], 1);
        assert!(json["stats"].get("sources_scanned").is_none());
    }

    #[test]
    fn constructor_accepts_config_and_browser_target_like_napi() {
        let compiler =
            super::WasmCompiler::new(None, Some("{}".to_owned()), Some("safari-15".to_owned()))
                .expect("config and target are accepted");
        let explained: serde_json::Value =
            serde_json::from_str(&compiler.explain("p-4".to_owned()).expect("explain serializes"))
                .expect("explain payload is JSON");
        assert_eq!(explained["candidate"], "p-4");
        assert_eq!(explained["status"], "Valid");
        assert_eq!(explained["browser_target"], "safari-15");

        let modern = super::WasmCompiler::new(None, None, None).expect("defaults are accepted");
        let explained: serde_json::Value =
            serde_json::from_str(&modern.explain("p-4".to_owned()).expect("explain serializes"))
                .expect("explain payload is JSON");
        assert_eq!(explained["browser_target"], "modern");
    }

    #[test]
    fn constructor_rejects_unknown_targets_and_invalid_config() {
        // Host tests target the pure builder: formatting the error as a
        // JavaScript value requires the WASM runtime.
        use super::build_compiler;

        assert!(
            build_compiler(None, None, Some("ie11")).is_err(),
            "unknown browser target must fail"
        );
        assert!(
            build_compiler(None, Some(r#"{"mode": "bogus"}"#), None).is_err(),
            "invalid config mode must fail"
        );
        assert!(build_compiler(None, Some("{oops"), None).is_err(), "malformed config must fail");
        assert!(
            build_compiler(None, Some("{}"), Some("safari-15")).is_ok(),
            "config and target are accepted"
        );
    }

    #[test]
    fn validate_returns_a_json_string_matching_napi() {
        let compiler = super::WasmCompiler::new(None, None, None).expect("defaults are accepted");
        let payload = compiler.validate("flx".to_owned()).expect("validate serializes");
        let json: serde_json::Value = serde_json::from_str(&payload).expect("payload is JSON");
        assert_eq!(json["candidate"], "flx");
        assert_eq!(json["valid"], false);
        assert_eq!(json["status"], "Unresolved");
        assert!(
            json["alternatives"].as_array().is_some_and(|alternatives| !alternatives.is_empty()),
            "typo reports alternatives: {payload}"
        );
    }

    #[test]
    fn update_source_accepts_paths_and_remove_reports_existence() {
        let mut compiler =
            super::WasmCompiler::new(Some(false), None, None).expect("defaults are accepted");
        compiler
            .update_source(
                "src/app.html".to_owned(),
                "<div class=\"p-4\"></div>".to_owned(),
                Some("src/app.html".to_owned()),
            )
            .expect("source with path is accepted");
        assert!(compiler.remove_source("src/app.html".to_owned()));
        assert!(!compiler.remove_source("src/app.html".to_owned()));
    }

    #[test]
    fn source_kind_dispatch_matches_napi_extension_table() {
        use utilitycss_swc::SourceKind as SwcSourceKind;

        use super::SourceKind;
        use crate::source_kind;

        assert!(matches!(
            source_kind(Some("src/app.tsx")),
            SourceKind::JavaScript(SwcSourceKind::Tsx)
        ));
        assert!(matches!(
            source_kind(Some("src/app.ts")),
            SourceKind::JavaScript(SwcSourceKind::TypeScript)
        ));
        assert!(matches!(
            source_kind(Some("src/app.vue")),
            SourceKind::Framework(utilitycss_extractor::Framework::Vue)
        ));
        assert!(matches!(
            source_kind(None),
            SourceKind::Framework(utilitycss_extractor::Framework::Html)
        ));
        assert!(matches!(
            source_kind(Some("src/app.html")),
            SourceKind::Framework(utilitycss_extractor::Framework::Html)
        ));
    }
}
