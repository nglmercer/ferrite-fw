//! Thin N-API transport bindings over the runtime-independent compiler facade.

// The N-API proc macro emits its own unsafe FFI glue and locally allows that generated code.
// This crate contains no handwritten unsafe blocks.
#![deny(unsafe_code)]
#![deny(missing_docs)]

use napi::{Error, Result, Status};
use napi_derive::napi;
use utilitycss_compiler::{CandidateInput, Compiler as CoreCompiler, CompilerConfig, SourceInput};
use utilitycss_config::{parse_css, parse_json, ConfigFile};
use utilitycss_css_ir::{BrowserTarget, CssSerializationMode};
use utilitycss_diagnostics::Severity;
use utilitycss_extractor::{extract_for_framework, Framework};
use utilitycss_scanner::ExtractionMode;
use utilitycss_span::{validate_source_len, SourceId, Span};
use utilitycss_stylesheet::{transform_stylesheet, StylesheetInput};
use utilitycss_swc::{extract as extract_swc, SourceKind as SwcSourceKind};

/// A diagnostic returned across the N-API boundary.
#[napi(object)]
pub struct JsDiagnostic {
    /// Stable severity name.
    pub severity: String,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional source identity.
    pub source: Option<String>,
    /// Optional start byte offset.
    pub start: Option<u32>,
    /// Optional exclusive end byte offset.
    pub end: Option<u32>,
    /// Optional actionable help text.
    pub help: Option<String>,
    /// Optional longer explanation for IDE and agent clients.
    pub explanation: Option<String>,
    /// Deterministic replacement suggestions.
    pub suggestions: Vec<String>,
}

/// A statically extracted candidate accepted by the batched source update API.
#[napi(object)]
pub struct JsCandidate {
    /// Candidate text, which MUST match the source span exactly.
    pub raw: String,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
    /// Extraction mode that produced this candidate.
    pub extraction_mode: Option<String>,
}

/// Build counters returned across the N-API boundary.
#[napi(object)]
pub struct JsStats {
    /// Number of source units scanned on updates.
    pub sources_scanned: f64,
    /// Number of source bytes scanned on updates.
    pub bytes_scanned: f64,
    /// Number of candidate occurrences found on updates.
    pub candidates_found: f64,
    /// Number of unique active candidates.
    pub unique_candidates: f64,
    /// Number of candidates parsed on this build.
    pub candidates_parsed: f64,
    /// Number of active candidates served from cache.
    pub cache_hits: f64,
    /// Number of active rules emitted.
    pub rules_generated: f64,
    /// Number of rules removed from active references.
    pub rules_removed: f64,
}

/// A compiler build result returned to JavaScript.
#[napi(object)]
pub struct JsBuildResult {
    /// Serialized CSS output.
    pub css: String,
    /// Structured compiler diagnostics.
    pub diagnostics: Vec<JsDiagnostic>,
    /// Work counters for this build.
    pub stats: JsStats,
}

/// A stylesheet transformation result returned across the N-API boundary.
#[napi(object)]
pub struct JsStylesheetResult {
    /// Transformed authored CSS.
    pub css: String,
    /// Structured stylesheet and composition diagnostics.
    pub diagnostics: Vec<JsDiagnostic>,
}

/// A reusable JavaScript-facing compiler instance.
#[napi]
pub struct Compiler {
    inner: CoreCompiler,
}

#[napi]
impl Compiler {
    /// Creates a compiler with optional readable output, declarative config, and browser target.
    #[napi(constructor)]
    pub fn new(
        pretty: Option<bool>,
        config_source: Option<String>,
        browser_target: Option<String>,
    ) -> Result<Self> {
        let config = config_source.as_deref().map(parse_config_source).transpose()?;
        let browser_target = match browser_target.as_deref() {
            Some(target) => BrowserTarget::parse(target).ok_or_else(|| {
                Error::new(Status::InvalidArg, format!("unknown browser target `{target}`"))
            })?,
            None => {
                config.as_ref().map(ConfigFile::browser_target).unwrap_or(BrowserTarget::Modern)
            }
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
            CompilerConfig::new().with_serialization_mode(mode).with_browser_target(browser_target);
        if let Some(config) = config {
            compiler_config = compiler_config
                .with_theme(config.theme().clone())
                .with_utility_registry(config.utilities().clone())
                .with_variant_registry(config.variants().clone())
                .with_preset_name(config.preset().name());
        }
        Ok(Self { inner: CoreCompiler::new(compiler_config) })
    }

    /// Inserts or replaces one source unit.
    #[napi]
    pub fn update_source(
        &mut self,
        id: String,
        content: String,
        path: Option<String>,
        candidates: Option<Vec<JsCandidate>>,
    ) -> Result<()> {
        let source_id = SourceId::new(id);
        let source = match path {
            Some(path) => SourceInput::new(source_id, content).with_path(path),
            None => SourceInput::new(source_id, content),
        };
        if let Some(candidates) = candidates {
            let candidates = candidates
                .into_iter()
                .map(|candidate| {
                    let span = Span::new(candidate.start, candidate.end).ok_or_else(|| {
                        Error::new(Status::InvalidArg, "candidate span is not ordered")
                    })?;
                    let input = CandidateInput::new(candidate.raw, span);
                    match candidate.extraction_mode.as_deref() {
                        None => Ok(input),
                        Some(name) => ExtractionMode::parse(name)
                            .map(|mode| input.with_extraction_mode(mode))
                            .ok_or_else(|| {
                                Error::new(
                                    Status::InvalidArg,
                                    format!("unknown extraction mode `{name}`"),
                                )
                            }),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            self.inner
                .update_source_with_candidates(source, candidates)
                .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))
        } else {
            self.inner
                .update_source(source)
                .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))
        }
    }

    /// Extracts static class candidates using SWC or the selected framework adapter.
    #[napi]
    pub fn extract_candidates(
        &self,
        content: String,
        path: Option<String>,
    ) -> Result<Vec<JsCandidate>> {
        validate_source_len(content.len())
            .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))?;
        let candidates = match source_kind(path.as_deref()) {
            SourceKind::JavaScript(kind) => extract_swc(&content, kind)
                .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))?
                .into_iter()
                .map(|candidate| (candidate.raw(), candidate.span(), ExtractionMode::Ast))
                .collect::<Vec<_>>(),
            SourceKind::Framework(framework) => extract_for_framework(&content, framework)
                .map_err(|error| Error::new(Status::InvalidArg, error.to_string()))?
                .into_iter()
                .map(|candidate| (candidate.raw(), candidate.span(), ExtractionMode::Static))
                .collect(),
        };
        Ok(candidates
            .into_iter()
            .map(|(raw, span, mode)| JsCandidate {
                raw: raw.to_owned(),
                start: span.start(),
                end: span.end(),
                extraction_mode: Some(mode.as_str().to_owned()),
            })
            .collect())
    }

    /// Removes one source unit and returns whether it existed.
    #[napi]
    pub fn remove_source(&mut self, id: String) -> bool {
        self.inner.remove_source(&SourceId::new(id))
    }

    /// Builds the current sources and returns CSS, diagnostics, and counters.
    #[napi]
    pub fn build(&mut self) -> JsBuildResult {
        let output = self.inner.build();
        let diagnostics = output.diagnostics().iter().map(js_diagnostic).collect();
        let stats = output.stats();
        JsBuildResult {
            css: output.css().to_owned(),
            diagnostics,
            stats: JsStats {
                sources_scanned: stats_number(stats.sources_scanned()),
                bytes_scanned: stats_number(stats.bytes_scanned()),
                candidates_found: stats_number(stats.candidates_found()),
                unique_candidates: stats_number(stats.unique_candidates()),
                candidates_parsed: stats_number(stats.candidates_parsed()),
                cache_hits: stats_number(stats.cache_hits()),
                rules_generated: stats_number(stats.rules_generated()),
                rules_removed: stats_number(stats.rules_removed()),
            },
        }
    }

    /// Explains one candidate and returns the stable JSON introspection payload.
    #[napi]
    pub fn explain(&self, candidate: String) -> Result<String> {
        serde_json::to_string(&self.inner.explain_candidate(&candidate))
            .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))
    }

    /// Validates one candidate and returns the stable JSON validation payload.
    #[napi]
    pub fn validate(&self, candidate: String) -> Result<String> {
        serde_json::to_string(
            &self.inner.validate(utilitycss_compiler::ExplainRequest::new(&candidate)),
        )
        .map_err(|error| Error::new(Status::GenericFailure, error.to_string()))
    }

    /// Returns the active machine-readable capability manifest as JSON.
    #[napi]
    pub fn capabilities(&self) -> String {
        self.inner.capability_manifest_json()
    }

    /// Transforms authored CSS and resolves explicit `@apply` directives.
    #[napi]
    pub fn transform_stylesheet(
        &mut self,
        id: String,
        content: String,
        path: Option<String>,
    ) -> JsStylesheetResult {
        let source_id = SourceId::new(id);
        let input = match path {
            Some(path) => StylesheetInput::new(source_id, content).with_path(path),
            None => StylesheetInput::new(source_id, content),
        };
        let output = transform_stylesheet(&mut self.inner, input);
        JsStylesheetResult {
            css: output.css().to_owned(),
            diagnostics: output.diagnostics().iter().map(js_diagnostic).collect(),
        }
    }
}

fn js_diagnostic(diagnostic: &utilitycss_diagnostics::Diagnostic) -> JsDiagnostic {
    JsDiagnostic {
        severity: match diagnostic.severity() {
            Severity::Error => "error".to_owned(),
            Severity::Warning => "warning".to_owned(),
            Severity::Note => "note".to_owned(),
            Severity::Help => "help".to_owned(),
        },
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

fn parse_config_source(source: &str) -> Result<ConfigFile> {
    let trimmed = source.trim_start();
    let result = if trimmed.starts_with('{') { parse_json(source) } else { parse_css(source) };
    result.map_err(|error| Error::new(Status::InvalidArg, error.to_string()))
}

fn stats_number(value: usize) -> f64 {
    value as f64
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
