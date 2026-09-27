//! Stable introspection helpers for editor, agent, and protocol integrations.
//!
//! This crate is intentionally thin. The compiler owns parsing and semantic meaning; this crate
//! only provides a small discoverable package boundary for consumers that do not need the source
//! indexing APIs.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub use utilitycss_compiler::{
    BrowserSupportReport, BrowserTarget, CapabilityManifest, CompletionItem, CssFeature,
    DiagnosticDescriptor, ExplainDiagnostic, ExplainRequest, ExplainResult, ExplainSuggestion,
    GeneratedArtifacts, HoverInfo, ProvenanceEntry, ResolutionStatus, ValidationResult,
};

/// Explains one candidate using a compiler instance's active configuration.
#[must_use]
pub fn explain(compiler: &utilitycss_compiler::Compiler, candidate: &str) -> ExplainResult {
    compiler.explain_candidate(candidate)
}

/// Validates one candidate using a compiler instance's active configuration.
#[must_use]
pub fn validate(compiler: &utilitycss_compiler::Compiler, candidate: &str) -> ValidationResult {
    compiler.validate(ExplainRequest::new(candidate))
}

/// Returns the complete generated artifact set for a compiler configuration.
#[must_use]
pub fn artifacts(compiler: &utilitycss_compiler::Compiler) -> GeneratedArtifacts {
    compiler.generated_artifacts()
}
