//! Versioned, runtime-independent transport types for compiler hosts.
//!
//! The protocol keeps bindings and external processes on a compact plain-data surface. It does
//! not expose the compiler's internal AST or CSS IR types.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use utilitycss_compiler::{
    CandidateInput, CompatibilityProfile, Compiler, CompilerConfig, ExplainRequest, SourceInput,
};
use utilitycss_css_ir::CssSerializationMode;
use utilitycss_diagnostics::Severity;
use utilitycss_scanner::ExtractionMode;
use utilitycss_span::{SourceId, Span};

/// Current request/response schema version.
pub const PROTOCOL_VERSION: u16 = 2;

/// A candidate supplied by an AST-assisted host extractor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolCandidate {
    /// Candidate text, which MUST match the source span exactly.
    pub raw: String,
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
    /// Optional extraction mode supplied by an AST/static host.
    #[serde(rename = "extractionMode", default, skip_serializing_if = "Option::is_none")]
    pub extraction_mode: Option<String>,
}

/// A versioned compiler request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProtocolRequest {
    /// Starts or resets a compiler session.
    Initialize {
        /// Request schema version.
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
        /// Whether CSS should use readable formatting.
        #[serde(default)]
        pretty: bool,
    },
    /// Inserts or replaces one source unit.
    UpdateSource {
        /// Stable source identity.
        id: String,
        /// Optional path metadata.
        path: Option<String>,
        /// Source content.
        content: String,
        /// Optional statically extracted candidates.
        #[serde(default)]
        candidates: Option<Vec<ProtocolCandidate>>,
    },
    /// Removes one source unit.
    RemoveSource {
        /// Stable source identity.
        id: String,
    },
    /// Builds the current source set.
    Build,
    /// Explains one candidate without mutating the compiler session.
    Explain {
        /// Candidate source text.
        candidate: String,
        /// Optional compatibility profile name.
        #[serde(default)]
        compatibility: Option<String>,
    },
    /// Validates one candidate without emitting a stylesheet.
    Validate {
        /// Candidate source text.
        candidate: String,
        /// Optional compatibility profile name.
        #[serde(default)]
        compatibility: Option<String>,
    },
    /// Returns the active registry, grammar, theme, and diagnostic manifest.
    Capabilities,
    /// Removes all sources and cached semantic results.
    Reset,
}

/// A structured diagnostic transported across process or runtime boundaries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolDiagnostic {
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
    /// Optional longer explanation.
    #[serde(default)]
    pub explanation: Option<String>,
    /// Deterministic replacement suggestions.
    #[serde(default)]
    pub suggestions: Vec<String>,
}

/// Build counters transported across process or runtime boundaries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolStats {
    /// Number of source units scanned on updates.
    pub sources_scanned: u64,
    /// Number of source bytes scanned on updates.
    pub bytes_scanned: u64,
    /// Number of candidate occurrences found on updates.
    pub candidates_found: u64,
    /// Number of unique active candidates.
    pub unique_candidates: u64,
    /// Number of candidates parsed on this build.
    pub candidates_parsed: u64,
    /// Number of active candidates served from cache.
    pub cache_hits: u64,
    /// Number of active rules emitted.
    pub rules_generated: u64,
    /// Number of cached candidate rules removed from active references.
    pub rules_removed: u64,
}

/// A compiler build result transported across process or runtime boundaries.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolBuildResult {
    /// Serialized CSS output.
    pub css: String,
    /// Structured diagnostics.
    pub diagnostics: Vec<ProtocolDiagnostic>,
    /// Build counters.
    pub stats: ProtocolStats,
}

/// A versioned compiler response.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProtocolResponse {
    /// Session initialization succeeded.
    Initialized {
        /// Active schema version.
        #[serde(rename = "protocolVersion")]
        protocol_version: u16,
    },
    /// Source update succeeded.
    Updated,
    /// Source removal completed.
    Removed {
        /// Whether a source with that identity existed.
        existed: bool,
    },
    /// Build completed.
    Build {
        /// Build result.
        result: ProtocolBuildResult,
    },
    /// Candidate explanation completed.
    Explained {
        /// Serialized explanation payload.
        result: serde_json::Value,
    },
    /// Candidate validation completed.
    Validated {
        /// Serialized validation payload.
        result: serde_json::Value,
    },
    /// Capability manifest returned.
    Capabilities {
        /// Serialized capability payload.
        result: serde_json::Value,
    },
    /// All sources and caches were cleared.
    Reset,
}

/// Errors raised while decoding or handling protocol messages.
#[derive(Debug)]
pub enum ProtocolError {
    /// The input JSON was invalid.
    InvalidJson(String),
    /// The requested schema version is unsupported.
    UnsupportedVersion(u16),
    /// A request requiring a compiler arrived before initialization.
    NotInitialized,
    /// The compiler rejected a request.
    Compiler(utilitycss_compiler::CompilerError),
    /// A candidate used an invalid half-open span.
    InvalidCandidateSpan {
        /// Inclusive start byte offset.
        start: u32,
        /// Exclusive end byte offset.
        end: u32,
    },
    /// A host supplied an unknown extraction mode.
    InvalidExtractionMode(String),
    /// A response could not be serialized.
    Serialization(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid protocol JSON: {message}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported protocol version {version}")
            }
            Self::NotInitialized => formatter.write_str("protocol session is not initialized"),
            Self::Compiler(error) => error.fmt(formatter),
            Self::InvalidCandidateSpan { start, end } => {
                write!(formatter, "candidate span {start}..{end} is not ordered")
            }
            Self::InvalidExtractionMode(mode) => {
                write!(formatter, "unknown extraction mode `{mode}`")
            }
            Self::Serialization(message) => {
                write!(formatter, "protocol serialization failed: {message}")
            }
        }
    }
}

impl Error for ProtocolError {}

/// A stateful request handler for line-oriented or embedded protocol users.
#[derive(Debug, Default)]
pub struct ProtocolSession {
    compiler: Option<Compiler>,
}

impl ProtocolSession {
    /// Creates an uninitialized protocol session.
    #[must_use]
    pub const fn new() -> Self {
        Self { compiler: None }
    }

    /// Handles one typed request.
    pub fn handle(&mut self, request: ProtocolRequest) -> Result<ProtocolResponse, ProtocolError> {
        match request {
            ProtocolRequest::Initialize { protocol_version, pretty } => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(ProtocolError::UnsupportedVersion(protocol_version));
                }
                let mode = if pretty {
                    CssSerializationMode::Pretty
                } else {
                    CssSerializationMode::Minified
                };
                self.compiler =
                    Some(Compiler::new(CompilerConfig::new().with_serialization_mode(mode)));
                Ok(ProtocolResponse::Initialized { protocol_version })
            }
            ProtocolRequest::UpdateSource { id, path, content, candidates } => {
                let compiler = self.compiler.as_mut().ok_or(ProtocolError::NotInitialized)?;
                let source_id = SourceId::new(id);
                let source = match path {
                    Some(path) => SourceInput::new(source_id, content).with_path(path),
                    None => SourceInput::new(source_id, content),
                };
                if let Some(candidates) = candidates {
                    let candidates = candidates
                        .into_iter()
                        .map(|candidate| {
                            let span = Span::new(candidate.start, candidate.end).ok_or(
                                ProtocolError::InvalidCandidateSpan {
                                    start: candidate.start,
                                    end: candidate.end,
                                },
                            )?;
                            let mode = match candidate.extraction_mode.as_deref() {
                                None => None,
                                Some(name) => {
                                    Some(ExtractionMode::parse(name).ok_or_else(|| {
                                        ProtocolError::InvalidExtractionMode(name.to_owned())
                                    })?)
                                }
                            };
                            let input = CandidateInput::new(candidate.raw, span);
                            Ok(match mode {
                                Some(mode) => input.with_extraction_mode(mode),
                                None => input,
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    compiler
                        .update_source_with_candidates(source, candidates)
                        .map_err(ProtocolError::Compiler)?;
                } else {
                    compiler.update_source(source).map_err(ProtocolError::Compiler)?;
                }
                Ok(ProtocolResponse::Updated)
            }
            ProtocolRequest::RemoveSource { id } => {
                let compiler = self.compiler.as_mut().ok_or(ProtocolError::NotInitialized)?;
                let existed = compiler.remove_source(&SourceId::new(id));
                Ok(ProtocolResponse::Removed { existed })
            }
            ProtocolRequest::Build => {
                let compiler = self.compiler.as_mut().ok_or(ProtocolError::NotInitialized)?;
                Ok(ProtocolResponse::Build { result: build_result(compiler) })
            }
            ProtocolRequest::Explain { candidate, compatibility } => {
                let compiler = self.compiler.as_ref().ok_or(ProtocolError::NotInitialized)?;
                let request = ExplainRequest::new(&candidate)
                    .with_compatibility(parse_compatibility(compatibility));
                let result = serde_json::to_value(compiler.explain(request))
                    .map_err(|error| ProtocolError::Serialization(error.to_string()))?;
                Ok(ProtocolResponse::Explained { result })
            }
            ProtocolRequest::Validate { candidate, compatibility } => {
                let compiler = self.compiler.as_ref().ok_or(ProtocolError::NotInitialized)?;
                let request = ExplainRequest::new(&candidate)
                    .with_compatibility(parse_compatibility(compatibility));
                let result = serde_json::to_value(compiler.validate(request))
                    .map_err(|error| ProtocolError::Serialization(error.to_string()))?;
                Ok(ProtocolResponse::Validated { result })
            }
            ProtocolRequest::Capabilities => {
                let compiler = self.compiler.as_ref().ok_or(ProtocolError::NotInitialized)?;
                let result = serde_json::to_value(compiler.capability_manifest())
                    .map_err(|error| ProtocolError::Serialization(error.to_string()))?;
                Ok(ProtocolResponse::Capabilities { result })
            }
            ProtocolRequest::Reset => {
                let compiler = self.compiler.as_mut().ok_or(ProtocolError::NotInitialized)?;
                compiler.reset();
                Ok(ProtocolResponse::Reset)
            }
        }
    }

    /// Decodes one JSON request and encodes its typed response.
    pub fn handle_json(&mut self, input: &str) -> Result<String, ProtocolError> {
        let request = serde_json::from_str(input)
            .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
        let response = self.handle(request)?;
        serde_json::to_string(&response)
            .map_err(|error| ProtocolError::Serialization(error.to_string()))
    }
}

fn parse_compatibility(name: Option<String>) -> CompatibilityProfile {
    match name.as_deref() {
        None | Some("native") => CompatibilityProfile::Native,
        Some("tailwind-v4-like") | Some("tailwind-v4-subset") => {
            CompatibilityProfile::TailwindV4Like
        }
        Some("tailwind-v3-like") | Some("tailwind-v3-subset") => {
            CompatibilityProfile::TailwindV3Like
        }
        Some(name) => CompatibilityProfile::Custom(name.to_owned()),
    }
}

fn build_result(compiler: &mut Compiler) -> ProtocolBuildResult {
    let output = compiler.build();
    let diagnostics = output
        .diagnostics()
        .iter()
        .map(|diagnostic| ProtocolDiagnostic {
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
        })
        .collect();
    let stats = output.stats();
    ProtocolBuildResult {
        css: output.css().to_owned(),
        diagnostics,
        stats: ProtocolStats {
            sources_scanned: saturating_u64(stats.sources_scanned()),
            bytes_scanned: saturating_u64(stats.bytes_scanned()),
            candidates_found: saturating_u64(stats.candidates_found()),
            unique_candidates: saturating_u64(stats.unique_candidates()),
            candidates_parsed: saturating_u64(stats.candidates_parsed()),
            cache_hits: saturating_u64(stats.cache_hits()),
            rules_generated: saturating_u64(stats.rules_generated()),
            rules_removed: saturating_u64(stats.rules_removed()),
        },
    }
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{
        ProtocolCandidate, ProtocolError, ProtocolRequest, ProtocolResponse, ProtocolSession,
        PROTOCOL_VERSION,
    };

    #[test]
    fn typed_session_builds_one_source() {
        let mut session = ProtocolSession::new();
        assert_eq!(
            session
                .handle(ProtocolRequest::Initialize {
                    protocol_version: PROTOCOL_VERSION,
                    pretty: false
                })
                .expect("version is supported"),
            ProtocolResponse::Initialized { protocol_version: PROTOCOL_VERSION }
        );
        session
            .handle(ProtocolRequest::UpdateSource {
                id: "src/app.html".to_owned(),
                path: None,
                content: r#"<div class="p-4"></div>"#.to_owned(),
                candidates: None,
            })
            .expect("source update succeeds");

        let ProtocolResponse::Build { result } =
            session.handle(ProtocolRequest::Build).expect("build succeeds")
        else {
            panic!("expected build response");
        };
        assert!(result.css.contains(".p-4{"));
    }

    #[test]
    fn json_round_trip_uses_stable_field_names() {
        let mut session = ProtocolSession::new();
        let response = session
            .handle_json(r#"{"type":"initialize","protocolVersion":2,"pretty":true}"#)
            .expect("request is valid");

        assert_eq!(response, r#"{"type":"initialized","protocolVersion":2}"#);
    }

    #[test]
    fn introspection_requests_are_runtime_neutral() {
        let mut session = ProtocolSession::new();
        session
            .handle(ProtocolRequest::Initialize {
                protocol_version: PROTOCOL_VERSION,
                pretty: false,
            })
            .expect("version is supported");

        let ProtocolResponse::Explained { result } = session
            .handle(ProtocolRequest::Explain {
                candidate: "hover:bg-red-500/50!".to_owned(),
                compatibility: None,
            })
            .expect("explanation succeeds")
        else {
            panic!("expected explanation response");
        };
        assert_eq!(result["status"], "Valid");
        assert_eq!(result["grammar_version"], 1);

        let ProtocolResponse::Capabilities { result } =
            session.handle(ProtocolRequest::Capabilities).expect("capabilities succeed")
        else {
            panic!("expected capabilities response");
        };
        assert!(result["utilities"].as_array().is_some_and(|items| !items.is_empty()));
    }

    #[test]
    fn candidate_transport_preserves_optional_extraction_mode() {
        let candidate = ProtocolCandidate {
            raw: "p-4".to_owned(),
            start: 0,
            end: 3,
            extraction_mode: Some("ast".to_owned()),
        };
        let encoded = serde_json::to_string(&candidate).expect("candidate serializes");
        assert!(encoded.contains("\"extractionMode\":\"ast\""));

        let decoded: ProtocolCandidate = serde_json::from_str(r#"{"raw":"p-4","start":0,"end":3}"#)
            .expect("legacy candidate payload remains valid");
        assert_eq!(decoded.extraction_mode, None);

        let mut session = ProtocolSession::new();
        session
            .handle(ProtocolRequest::Initialize {
                protocol_version: PROTOCOL_VERSION,
                pretty: false,
            })
            .expect("version is supported");
        let error = session
            .handle(ProtocolRequest::UpdateSource {
                id: "src/app.tsx".to_owned(),
                path: None,
                content: "p-4".to_owned(),
                candidates: Some(vec![ProtocolCandidate {
                    extraction_mode: Some("unknown".to_owned()),
                    ..candidate
                }]),
            })
            .expect_err("unknown extraction modes are rejected");
        assert!(matches!(error, ProtocolError::InvalidExtractionMode(mode) if mode == "unknown"));
    }
}
