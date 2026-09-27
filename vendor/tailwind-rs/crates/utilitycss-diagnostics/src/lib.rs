//! Structured diagnostics that can be rendered by any host adapter.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::fmt;

use serde::Serialize;
use utilitycss_span::{SourceId, Span};

/// Stable severity levels for compiler diagnostics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Severity {
    /// A condition that prevents the requested operation from succeeding.
    Error,
    /// A condition that does not prevent completion but may indicate a mistake.
    Warning,
    /// Context that explains another diagnostic.
    Note,
    /// An actionable suggestion for resolving a diagnostic.
    Help,
}

/// A stable diagnostic code represented as a static identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    /// Creates a diagnostic code from a static identifier.
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self(code)
    }

    /// Returns the diagnostic code identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A structured compiler diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    source: Option<SourceId>,
    span: Option<Span>,
    help: Option<String>,
    explanation: Option<String>,
    suggestions: Vec<DiagnosticSuggestion>,
    provenance: Option<DiagnosticProvenance>,
}

/// A machine-readable repair suggestion attached to a diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticSuggestion {
    /// Replacement text, usually a candidate or configuration fragment.
    pub replacement: String,
    /// Explanation of why the replacement is useful.
    pub description: String,
}

impl DiagnosticSuggestion {
    /// Creates a suggestion from replacement text and a short explanation.
    #[must_use]
    pub fn new(replacement: impl Into<String>, description: impl Into<String>) -> Self {
        Self { replacement: replacement.into(), description: description.into() }
    }
}

/// Provenance describing the registry, preset, or theme source behind a result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticProvenance {
    /// Provenance category, such as `utility`, `theme`, or `preset`.
    pub kind: String,
    /// Stable key within that category.
    pub key: String,
    /// Optional human-readable detail.
    pub detail: Option<String>,
}

impl DiagnosticProvenance {
    /// Creates a provenance entry.
    #[must_use]
    pub fn new(kind: impl Into<String>, key: impl Into<String>) -> Self {
        Self { kind: kind.into(), key: key.into(), detail: None }
    }

    /// Adds a human-readable detail string.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl Diagnostic {
    /// Creates a diagnostic with no source location or help text.
    #[must_use]
    pub fn new(severity: Severity, code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            source: None,
            span: None,
            help: None,
            explanation: None,
            suggestions: Vec::new(),
            provenance: None,
        }
    }

    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    /// Attaches a source identity to the diagnostic.
    #[must_use]
    pub fn with_source(mut self, source: SourceId) -> Self {
        self.source = Some(source);
        self
    }

    /// Attaches a source span to the diagnostic.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches an actionable help message to the diagnostic.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attaches a longer explanation suitable for IDE and LLM clients.
    #[must_use]
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = Some(explanation.into());
        self
    }

    /// Attaches a deterministic repair suggestion.
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: DiagnosticSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Attaches provenance for the diagnostic.
    #[must_use]
    pub fn with_provenance(mut self, provenance: DiagnosticProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// Returns the diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source identity, if one was attached.
    #[must_use]
    pub fn source(&self) -> Option<&SourceId> {
        self.source.as_ref()
    }

    /// Returns the source span, if one was attached.
    #[must_use]
    pub const fn span(&self) -> Option<Span> {
        self.span
    }

    /// Returns the help text, if one was attached.
    #[must_use]
    pub fn help(&self) -> Option<&str> {
        self.help.as_deref()
    }

    /// Returns the optional explanatory text.
    #[must_use]
    pub fn explanation(&self) -> Option<&str> {
        self.explanation.as_deref()
    }

    /// Returns deterministic repair suggestions.
    #[must_use]
    pub fn suggestions(&self) -> &[DiagnosticSuggestion] {
        &self.suggestions
    }

    /// Returns the optional provenance entry.
    #[must_use]
    pub fn provenance(&self) -> Option<&DiagnosticProvenance> {
        self.provenance.as_ref()
    }
}

/// A collection of diagnostics produced during one operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    /// Creates an empty diagnostic bag.
    #[must_use]
    pub const fn new() -> Self {
        Self { diagnostics: Vec::new() }
    }

    /// Appends a diagnostic in encounter order.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Returns the number of diagnostics in the bag.
    #[must_use]
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns whether the bag contains no diagnostics.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Iterates over diagnostics in encounter order.
    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.diagnostics.iter()
    }

    /// Consumes the bag and returns its diagnostics.
    #[must_use]
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl IntoIterator for DiagnosticBag {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.diagnostics.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use utilitycss_span::{SourceId, Span};

    use super::{Diagnostic, DiagnosticBag, DiagnosticCode, Severity};

    #[test]
    fn diagnostics_preserve_actionable_context() {
        let diagnostic = Diagnostic::error(DiagnosticCode::new("parse.invalid"), "bad candidate")
            .with_source(SourceId::new("src/app.html"))
            .with_span(Span::new(3, 8).expect("valid span"))
            .with_help("use a valid utility name");

        assert_eq!(diagnostic.severity(), Severity::Error);
        assert_eq!(diagnostic.code().as_str(), "parse.invalid");
        assert_eq!(diagnostic.source().map(SourceId::as_str), Some("src/app.html"));
        assert_eq!(diagnostic.span().map(Span::len), Some(5));
        assert_eq!(diagnostic.help(), Some("use a valid utility name"));
    }

    #[test]
    fn bags_keep_diagnostic_order() {
        let mut bag = DiagnosticBag::new();
        bag.push(Diagnostic::new(Severity::Warning, DiagnosticCode::new("first"), "first"));
        bag.push(Diagnostic::new(Severity::Note, DiagnosticCode::new("second"), "second"));

        let codes = bag.iter().map(|diagnostic| diagnostic.code().as_str()).collect::<Vec<_>>();
        assert_eq!(codes, ["first", "second"]);
    }
}
