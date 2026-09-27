//! Runtime-independent compiler facade.
//!
//! This crate coordinates source indexing, candidate parsing, semantic resolution, and CSS output.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use serde::Serialize;
use utilitycss_css_ir::{CssDocument, CssSerializationMode, OrderKey};
use utilitycss_diagnostics::{Diagnostic, DiagnosticBag, DiagnosticCode, Severity};
use utilitycss_scanner::{scan_checked, ExtractionMode};
use utilitycss_span::{validate_source_len, SourceSizeError};
use utilitycss_span::{SourceId, Span};
use utilitycss_syntax::{parse, CandidateAstOwned, GRAMMAR_VERSION};
use utilitycss_theme::Theme;
use utilitycss_utilities::{resolve, UtilityErrorKind, UtilityRegistry};
use utilitycss_variants::{apply, validate_selector, VariantRegistry};

pub use utilitycss_css_ir::{BrowserSupportReport, BrowserTarget, CssFeature};

/// Compatibility behavior selected for candidate explanation and validation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub enum CompatibilityProfile {
    /// Native utilitycss grammar and semantics.
    #[default]
    Native,
    /// Explicit Tailwind-v4-inspired behavior supported by this release.
    TailwindV4Like,
    /// Explicit Tailwind-v3-inspired behavior supported by this release.
    TailwindV3Like,
    /// A host-defined profile name.
    Custom(String),
}

impl CompatibilityProfile {
    /// Returns the stable profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Native => "native",
            Self::TailwindV4Like => "tailwind-v4-like",
            Self::TailwindV3Like => "tailwind-v3-like",
            Self::Custom(name) => name,
        }
    }
}

/// Request for one structured candidate explanation.
pub struct ExplainRequest<'a> {
    /// Candidate source text.
    pub candidate: &'a str,
    /// Optional configuration override for this request.
    pub config: Option<&'a CompilerConfig>,
    /// Compatibility policy used for the explanation.
    pub compatibility: CompatibilityProfile,
}

impl<'a> ExplainRequest<'a> {
    /// Creates a request against the compiler's current configuration.
    #[must_use]
    pub fn new(candidate: &'a str) -> Self {
        Self { candidate, config: None, compatibility: CompatibilityProfile::Native }
    }

    /// Selects a compatibility profile.
    #[must_use]
    pub fn with_compatibility(mut self, compatibility: CompatibilityProfile) -> Self {
        self.compatibility = compatibility;
        self
    }

    /// Uses a request-local compiler configuration.
    #[must_use]
    pub fn with_config(mut self, config: &'a CompilerConfig) -> Self {
        self.config = Some(config);
        self
    }
}

/// Resolution classification returned by explain and validation APIs.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ResolutionStatus {
    /// Candidate parsed, resolved, and emitted successfully.
    Valid,
    /// Candidate syntax is invalid.
    InvalidSyntax,
    /// Candidate syntax is valid but no semantic definition resolved it.
    Unresolved,
    /// Candidate is accepted by the language but needs a browser capability warning.
    UnsupportedBrowser,
    /// Candidate uses deprecated compatibility behavior.
    Deprecated,
    /// Candidate exists only in a compatibility profile.
    CompatibilityOnly,
}

/// A serializable diagnostic view used by explain/validate and adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExplainDiagnostic {
    /// Severity name.
    pub severity: String,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional source span.
    pub span: Option<Span>,
    /// Optional help text.
    pub help: Option<String>,
    /// Optional longer explanation.
    pub explanation: Option<String>,
    /// Deterministic suggestions.
    pub suggestions: Vec<ExplainSuggestion>,
}

/// A ranked correction suggestion for a candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExplainSuggestion {
    /// Suggested candidate text.
    pub candidate: String,
    /// Lower scores are better.
    pub score: u32,
    /// Why this alternative was selected.
    pub reason: String,
}

/// Provenance describing which semantic source introduced a behavior.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProvenanceEntry {
    /// Source category (`utility`, `variant`, `theme`, or `preset`).
    pub kind: String,
    /// Stable source key.
    pub key: String,
    /// Optional detail.
    pub detail: Option<String>,
}

/// Complete result of explaining one candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExplainResult {
    /// Original candidate text.
    pub candidate: String,
    /// Grammar version used to parse the candidate.
    pub grammar_version: u16,
    /// Owned parsed AST when syntax was valid.
    pub parsed: Option<CandidateAstOwned>,
    /// Stable normalized candidate representation.
    pub normalized: String,
    /// Resolution classification.
    pub status: ResolutionStatus,
    /// CSS emitted for a valid candidate.
    pub css: Option<String>,
    /// Structured diagnostics.
    pub diagnostics: Vec<ExplainDiagnostic>,
    /// Ranked alternatives.
    pub alternatives: Vec<ExplainSuggestion>,
    /// Semantic provenance entries.
    pub provenance: Vec<ProvenanceEntry>,
    /// Compatibility profile used for this request.
    pub compatibility: String,
    /// Browser target used for compatibility analysis.
    pub browser_target: String,
}

/// Result of validating one candidate against a compiled configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ValidationResult {
    /// Candidate text.
    pub candidate: String,
    /// Whether the candidate is valid and emitted.
    pub valid: bool,
    /// Resolution classification.
    pub status: ResolutionStatus,
    /// Structured diagnostics.
    pub diagnostics: Vec<ExplainDiagnostic>,
    /// Ranked corrections.
    pub alternatives: Vec<ExplainSuggestion>,
}

/// One protocol-neutral completion item derived from semantic metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompletionItem {
    /// Candidate label to insert.
    pub label: String,
    /// Short detail string.
    pub detail: String,
    /// Human-readable documentation.
    pub documentation: String,
}

/// Protocol-neutral hover information for a candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HoverInfo {
    /// Candidate being described.
    pub candidate: String,
    /// Semantic description.
    pub description: String,
    /// Optional generated CSS.
    pub css: Option<String>,
    /// Provenance entries.
    pub provenance: Vec<ProvenanceEntry>,
}

/// Machine-readable registry, grammar, theme, and diagnostics capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityManifest {
    /// Manifest schema version.
    pub schema_version: u16,
    /// Native grammar version.
    pub grammar_version: u16,
    /// Compact grammar summary for clients that do not load the EBNF document.
    pub grammar: String,
    /// Compiler package version.
    pub compiler_version: String,
    /// Active declarative preset identifier.
    pub active_preset: String,
    /// Built-in utility descriptors.
    pub utilities: Vec<utilitycss_utilities::UtilityDescriptor>,
    /// Built-in variant descriptors.
    pub variants: Vec<utilitycss_variants::VariantDescriptor>,
    /// Available theme tokens.
    pub theme: Vec<utilitycss_theme::ThemeToken>,
    /// Stable diagnostic catalog.
    pub diagnostics: Vec<DiagnosticDescriptor>,
    /// Compatibility profiles exposed by this build.
    pub compatibility_profiles: Vec<String>,
    /// Extraction modes supported by the compiler facade.
    pub extraction_modes: Vec<String>,
    /// Browser target configured for this compiler instance.
    pub browser_target: String,
}

/// One stable diagnostic code in the capability manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticDescriptor {
    /// Stable code.
    pub code: String,
    /// Default severity.
    pub severity: String,
    /// Short explanation.
    pub explanation: String,
}

/// Reproducible generated documentation and machine artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifacts {
    /// Capability manifest JSON.
    pub capabilities_json: String,
    /// JSON Schema for capability consumers.
    pub schema_json: String,
    /// Generated Markdown reference.
    pub reference_markdown: String,
    /// Compact LLM reference.
    pub llms_txt: String,
    /// Expanded LLM reference.
    pub llms_full_txt: String,
    /// Compatibility report JSON.
    pub compatibility_report_json: String,
}

/// Configuration for a compiler instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerConfig {
    theme: Theme,
    utilities: UtilityRegistry,
    variants: VariantRegistry,
    serialization_mode: CssSerializationMode,
    preset: String,
    browser_target: BrowserTarget,
}

impl CompilerConfig {
    /// Creates configuration with the built-in theme and registries.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the theme used for semantic resolution.
    #[must_use]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Replaces the utility registry.
    #[must_use]
    pub fn with_utility_registry(mut self, utilities: UtilityRegistry) -> Self {
        self.utilities = utilities;
        self
    }

    /// Replaces the variant registry.
    #[must_use]
    pub fn with_variant_registry(mut self, variants: VariantRegistry) -> Self {
        self.variants = variants;
        self
    }

    /// Selects the CSS serialization mode.
    #[must_use]
    pub fn with_serialization_mode(mut self, mode: CssSerializationMode) -> Self {
        self.serialization_mode = mode;
        self
    }

    /// Records the declarative preset that contributed the active semantics.
    #[must_use]
    pub fn with_preset_name(mut self, preset: impl Into<String>) -> Self {
        self.preset = preset.into();
        self
    }

    /// Selects the browser capability target used for warnings and lowering policy.
    #[must_use]
    pub const fn with_browser_target(mut self, target: BrowserTarget) -> Self {
        self.browser_target = target;
        self
    }

    /// Returns the configured theme.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Returns the configured utility registry.
    #[must_use]
    pub fn utilities(&self) -> &UtilityRegistry {
        &self.utilities
    }

    /// Returns the configured variant registry.
    #[must_use]
    pub fn variants(&self) -> &VariantRegistry {
        &self.variants
    }

    /// Returns the selected CSS serialization mode.
    #[must_use]
    pub const fn serialization_mode(&self) -> CssSerializationMode {
        self.serialization_mode
    }

    /// Returns the active preset identifier.
    #[must_use]
    pub fn preset_name(&self) -> &str {
        &self.preset
    }

    /// Returns the configured browser capability target.
    #[must_use]
    pub const fn browser_target(&self) -> BrowserTarget {
        self.browser_target
    }

    /// Returns a deterministic fingerprint for semantic configuration.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash =
            stable_hash_with_seed(0xcbf29ce484222325_u64, b"utilitycss-compiler-fingerprint-v2");
        hash = stable_hash_with_seed(hash, &self.theme.fingerprint().to_le_bytes());
        for (name, definition) in self.utilities.definitions() {
            hash = stable_hash_text(hash, name);
            hash = stable_hash_with_seed(hash, &definition.fingerprint().to_le_bytes());
        }
        for (name, definition) in self.variants.definitions() {
            hash = stable_hash_text(hash, name);
            hash = stable_hash_with_seed(hash, &definition.fingerprint().to_le_bytes());
        }
        hash = stable_hash_text(
            hash,
            match self.serialization_mode {
                CssSerializationMode::Pretty => "pretty",
                CssSerializationMode::Minified => "minified",
            },
        );
        hash = stable_hash_text(hash, &self.preset);
        stable_hash_text(hash, self.browser_target.as_str())
    }
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            utilities: UtilityRegistry::default(),
            variants: VariantRegistry::default(),
            serialization_mode: CssSerializationMode::Minified,
            preset: "utilitycss".to_owned(),
            browser_target: BrowserTarget::Modern,
        }
    }
}

/// Source content supplied by a host application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInput {
    id: SourceId,
    path: Option<String>,
    content: String,
}

impl SourceInput {
    /// Creates a source input without path metadata.
    #[must_use]
    pub fn new(id: SourceId, content: impl Into<String>) -> Self {
        Self { id, path: None, content: content.into() }
    }

    /// Adds optional path metadata without making the compiler depend on a filesystem.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Returns the stable source identity.
    #[must_use]
    pub fn id(&self) -> &SourceId {
        &self.id
    }

    /// Returns optional path metadata.
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Returns the source content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// A candidate supplied by an optional host-language extractor.
///
/// The candidate text MUST be an exact slice of the associated [`SourceInput`] content. This
/// keeps extractor adapters thin while preserving source-aware diagnostics in the compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateInput {
    raw: String,
    span: Span,
    mode: ExtractionMode,
}

/// Provenance for one candidate in an incremental source index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateOrigin {
    /// Stable source identity.
    pub source_id: SourceId,
    /// Source-relative candidate span.
    pub span: Span,
    /// Extraction mode that produced the candidate.
    pub extraction_mode: ExtractionMode,
}

/// One candidate explicitly supplied to an `@apply` composition operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyCandidate {
    raw: String,
    span: Span,
}

impl ApplyCandidate {
    /// Creates an apply candidate with its source-relative byte span.
    #[must_use]
    pub fn new(raw: impl Into<String>, span: Span) -> Self {
        Self { raw: raw.into(), span }
    }

    /// Returns the candidate text.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the candidate's byte span.
    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }
}

/// Input to one selector-aware composition batch.
pub struct CompositionInput<'a> {
    /// Source identity used by diagnostics.
    pub source: &'a SourceId,
    /// The selector receiving the composed declarations.
    pub selector: &'a str,
    /// Candidates in the directive's source order.
    pub candidates: &'a [ApplyCandidate],
}

/// CSS rules and diagnostics produced by one composition batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionOutput {
    /// Rules lowered through the normal utility and variant engines.
    pub rules: Vec<utilitycss_css_ir::CssRule>,
    /// Explicit composition diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}

impl CandidateInput {
    /// Creates a candidate from its source text and byte span.
    #[must_use]
    pub fn new(raw: impl Into<String>, span: Span) -> Self {
        Self { raw: raw.into(), span, mode: ExtractionMode::Text }
    }

    /// Marks this candidate as produced by a particular extraction mode.
    #[must_use]
    pub const fn with_extraction_mode(mut self, mode: ExtractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Returns the candidate text.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns the candidate's byte span.
    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }

    /// Returns the extraction mode attached to this candidate.
    #[must_use]
    pub const fn extraction_mode(&self) -> ExtractionMode {
        self.mode
    }
}

/// Errors raised before a compiler build can begin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerError {
    /// The source identity was empty and could not support stable indexing.
    EmptySourceId,
    /// The source is larger than the 32-bit source-location model can represent.
    SourceTooLarge {
        /// Rejected source length in bytes.
        length: usize,
    },
    /// An extractor supplied a span that does not exactly match its candidate text.
    InvalidCandidateSpan {
        /// Candidate text supplied by the extractor.
        raw: String,
        /// Candidate span supplied by the extractor.
        span: Span,
    },
}

impl fmt::Display for CompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySourceId => formatter.write_str("source identity must not be empty"),
            Self::SourceTooLarge { length } => {
                write!(formatter, "{length} bytes: source is too large for 32-bit source locations")
            }
            Self::InvalidCandidateSpan { raw, span } => write!(
                formatter,
                "candidate `{raw}` does not match source span {}..{}",
                span.start(),
                span.end()
            ),
        }
    }
}

impl Error for CompilerError {}

/// The result of one compiler build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileOutput {
    css: String,
    diagnostics: DiagnosticBag,
    stats: CompileStats,
}

impl CompileOutput {
    /// Returns the serialized CSS output.
    #[must_use]
    pub fn css(&self) -> &str {
        &self.css
    }

    /// Returns diagnostics emitted during the build.
    #[must_use]
    pub const fn diagnostics(&self) -> &DiagnosticBag {
        &self.diagnostics
    }

    /// Returns counters for the work performed during this build.
    #[must_use]
    pub const fn stats(&self) -> &CompileStats {
        &self.stats
    }
}

/// Counters describing source scanning, candidate caching, and rule generation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompileStats {
    sources_scanned: usize,
    bytes_scanned: usize,
    candidates_found: usize,
    unique_candidates: usize,
    candidates_parsed: usize,
    cache_hits: usize,
    rules_generated: usize,
    rules_removed: usize,
}

impl CompileStats {
    /// Returns the number of source updates that required scanning.
    #[must_use]
    pub const fn sources_scanned(&self) -> usize {
        self.sources_scanned
    }

    /// Returns the number of source bytes scanned during updates.
    #[must_use]
    pub const fn bytes_scanned(&self) -> usize {
        self.bytes_scanned
    }

    /// Returns the number of candidate occurrences found during updates.
    #[must_use]
    pub const fn candidates_found(&self) -> usize {
        self.candidates_found
    }

    /// Returns the number of unique candidates active at build time.
    #[must_use]
    pub const fn unique_candidates(&self) -> usize {
        self.unique_candidates
    }

    /// Returns the number of candidates parsed and resolved on this build.
    #[must_use]
    pub const fn candidates_parsed(&self) -> usize {
        self.candidates_parsed
    }

    /// Returns the number of active candidates served from the cache.
    #[must_use]
    pub const fn cache_hits(&self) -> usize {
        self.cache_hits
    }

    /// Returns the number of active rules emitted.
    #[must_use]
    pub const fn rules_generated(&self) -> usize {
        self.rules_generated
    }

    /// Returns the number of cached candidate rules removed from active references.
    #[must_use]
    pub const fn rules_removed(&self) -> usize {
        self.rules_removed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CandidateCacheEntry {
    rule: Option<utilitycss_css_ir::CssRule>,
    diagnostics: Vec<Diagnostic>,
}

/// A compiler instance with deterministic source identity storage.
#[derive(Clone, Debug, Default)]
pub struct Compiler {
    config: CompilerConfig,
    sources: BTreeMap<SourceId, SourceInput>,
    source_candidates: BTreeMap<SourceId, BTreeMap<String, Vec<Span>>>,
    source_origins: BTreeMap<SourceId, Vec<CandidateOrigin>>,
    candidate_sources: BTreeMap<String, BTreeSet<SourceId>>,
    candidate_cache: BTreeMap<String, CandidateCacheEntry>,
    pending_stats: CompileStats,
}

impl Compiler {
    /// Creates an empty compiler instance.
    #[must_use]
    pub fn new(config: CompilerConfig) -> Self {
        Self {
            config,
            sources: BTreeMap::new(),
            source_candidates: BTreeMap::new(),
            source_origins: BTreeMap::new(),
            candidate_sources: BTreeMap::new(),
            candidate_cache: BTreeMap::new(),
            pending_stats: CompileStats::default(),
        }
    }

    /// Inserts or replaces source content by stable source identity.
    pub fn update_source(&mut self, source: SourceInput) -> Result<(), CompilerError> {
        if source.id().as_str().is_empty() {
            return Err(CompilerError::EmptySourceId);
        }
        validate_source_len(source.content().len()).map_err(source_size_error)?;
        let source_id = source.id().clone();
        if let Some(previous) = self.sources.get(&source_id) {
            if previous.content() == source.content() {
                self.sources.insert(source_id, source);
                return Ok(());
            }
        }

        let tokens = scan_checked(source.content())
            .map_err(source_size_error)?
            .into_iter()
            .map(|token| CandidateInput::new(token.raw(), token.span()))
            .collect::<Vec<_>>();
        self.update_source_with_candidates(source, tokens)
    }

    /// Inserts or replaces source content using candidates supplied by a host-language extractor.
    ///
    /// This operation retains the same indexes and cache behavior as [`Self::update_source`]. It
    /// is useful for AST-assisted adapters that already know which string literals are static.
    pub fn update_source_with_candidates<I>(
        &mut self,
        source: SourceInput,
        candidates: I,
    ) -> Result<(), CompilerError>
    where
        I: IntoIterator<Item = CandidateInput>,
    {
        if source.id().as_str().is_empty() {
            return Err(CompilerError::EmptySourceId);
        }
        validate_source_len(source.content().len()).map_err(source_size_error)?;
        let source_id = source.id().clone();

        let mut new_candidates: BTreeMap<String, Vec<Span>> = BTreeMap::new();
        let mut origins = Vec::new();
        let mut candidates_found = 0;
        for candidate in candidates {
            validate_candidate(&source, &candidate)?;
            candidates_found += 1;
            new_candidates.entry(candidate.raw.clone()).or_default().push(candidate.span);
            origins.push(CandidateOrigin {
                source_id: source_id.clone(),
                span: candidate.span,
                extraction_mode: candidate.mode,
            });
        }
        let previous_candidates =
            self.source_candidates.get(&source_id).cloned().unwrap_or_default();
        for raw in previous_candidates.keys() {
            if !new_candidates.contains_key(raw) {
                self.remove_candidate_reference(raw, &source_id);
            }
        }
        for raw in new_candidates.keys() {
            if !previous_candidates.contains_key(raw) {
                self.candidate_sources.entry(raw.clone()).or_default().insert(source_id.clone());
            }
        }
        self.source_candidates.insert(source_id.clone(), new_candidates);
        self.source_origins.insert(source_id.clone(), origins);
        self.pending_stats.sources_scanned += 1;
        self.pending_stats.bytes_scanned += source.content().len();
        self.pending_stats.candidates_found += candidates_found;
        self.sources.insert(source_id, source);
        Ok(())
    }

    /// Removes a source and returns whether it was present.
    pub fn remove_source(&mut self, id: &SourceId) -> bool {
        let Some(previous_candidates) = self.source_candidates.remove(id) else {
            self.source_origins.remove(id);
            return self.sources.remove(id).is_some();
        };
        for raw in previous_candidates.keys() {
            self.remove_candidate_reference(raw, id);
        }
        self.source_origins.remove(id);
        self.sources.remove(id).is_some()
    }

    /// Replaces compiler configuration and invalidates semantic candidate caches.
    pub fn replace_config(&mut self, config: CompilerConfig) {
        self.config = config;
        self.candidate_cache.clear();
    }

    /// Removes all sources, indexes, and cached semantic results.
    pub fn reset(&mut self) {
        self.sources.clear();
        self.source_candidates.clear();
        self.source_origins.clear();
        self.candidate_sources.clear();
        self.candidate_cache.clear();
        self.pending_stats = CompileStats::default();
    }

    /// Returns the number of source units currently registered.
    #[must_use]
    pub fn source_count(&self) -> usize {
        self.sources.len()
    }

    /// Returns candidate ownership and extraction origins for a source.
    #[must_use]
    pub fn candidate_origins(&self, source_id: &SourceId) -> &[CandidateOrigin] {
        self.source_origins.get(source_id).map(Vec::as_slice).unwrap_or_default()
    }

    /// Builds all known source units into deterministic CSS and diagnostics.
    #[must_use]
    pub fn build(&mut self) -> CompileOutput {
        let mut stats = std::mem::take(&mut self.pending_stats);
        stats.unique_candidates = self.candidate_sources.len();
        let mut document = CssDocument::new();
        let mut diagnostics = DiagnosticBag::new();
        for (raw, source_ids) in &self.candidate_sources {
            if source_ids.is_empty() {
                continue;
            }
            let (entry, was_cached) = if let Some(entry) = self.candidate_cache.get(raw).cloned() {
                (entry, true)
            } else {
                let entry = compile_candidate(raw, &self.config);
                self.candidate_cache.insert(raw.clone(), entry.clone());
                (entry, false)
            };
            if was_cached {
                stats.cache_hits += 1;
            } else {
                stats.candidates_parsed += 1;
            }
            if let Some(rule) = entry.rule {
                stats.rules_generated += 1;
                document.push(rule);
            }
            for diagnostic in entry.diagnostics {
                for source_id in source_ids {
                    if let Some(spans) = self
                        .source_candidates
                        .get(source_id)
                        .and_then(|candidates| candidates.get(raw))
                    {
                        for span in spans {
                            diagnostics.push(
                                diagnostic.clone().with_source(source_id.clone()).with_span(*span),
                            );
                        }
                    }
                }
            }
        }

        CompileOutput { css: document.to_css(self.config.serialization_mode()), diagnostics, stats }
    }

    /// Compiles one explicit composition candidate into a caller-owned selector.
    ///
    /// Unlike normal source scanning, an unknown utility is an error here because the caller has
    /// explicitly requested that the candidate be composed.
    #[allow(clippy::result_large_err)]
    pub fn compose_candidate(
        &mut self,
        raw: &str,
        selector: &str,
        source: SourceId,
        span: Span,
    ) -> Result<utilitycss_css_ir::CssRule, Diagnostic> {
        if selector.trim().is_empty() {
            return Err(apply_diagnostic(
                "apply.invalid-selector",
                format!("cannot compose `{raw}` into an empty selector"),
                source,
                span,
                Some("add @apply inside a style rule with a selector"),
            ));
        }
        if validate_selector(selector).is_err() {
            return Err(apply_diagnostic(
                "apply.invalid-selector",
                format!("cannot compose `{raw}` into an unsafe selector"),
                source,
                span,
                Some("use a valid CSS style-rule selector"),
            ));
        }

        let candidate = parse(raw).map_err(|error| {
            apply_diagnostic(
                "apply.invalid-syntax",
                format!("invalid @apply candidate `{raw}`: {error}"),
                source.clone(),
                span,
                Some("use a utility candidate supported by the active registry"),
            )
        })?;
        let utility =
            resolve(&candidate, self.config.theme(), self.config.utilities()).map_err(|error| {
                let (code, help) = if error.kind() == UtilityErrorKind::UnknownUtility {
                    (
                        "apply.unknown-utility",
                        Some("remove the utility or register it before applying it"),
                    )
                } else {
                    ("apply.unsupported", Some("use a utility value supported by the active theme"))
                };
                apply_diagnostic(code, error.to_string(), source.clone(), span, help)
            })?;

        let utility_order = utility.order();
        let tie_breaker = stable_hash(raw.as_bytes());
        let Some(rule) = utility.into_rule(tie_breaker).with_selector(selector.to_owned()) else {
            return Err(apply_diagnostic(
                "apply.invalid-selector",
                format!("cannot compose `{raw}` into a style selector"),
                source,
                span,
                Some("use @apply inside a CSS style rule"),
            ));
        };
        let rule = apply(&candidate, rule, self.config.theme(), self.config.variants()).map_err(
            |error| {
                apply_diagnostic(
                    "apply.variant-error",
                    error.to_string(),
                    source.clone(),
                    span,
                    Some("use a variant supported by the active variant registry"),
                )
            },
        )?;
        let variant_order = self.config.variants().order(&candidate, self.config.theme());
        Ok(rule.with_order(OrderKey::new(0, variant_order, utility_order, tie_breaker)))
    }

    /// Compiles all candidates in one explicit composition batch.
    #[must_use]
    pub fn compose(&mut self, input: CompositionInput<'_>) -> CompositionOutput {
        let mut rules = Vec::new();
        let mut diagnostics = Vec::new();
        for candidate in input.candidates {
            match self.compose_candidate(
                candidate.raw(),
                input.selector,
                input.source.clone(),
                candidate.span(),
            ) {
                Ok(rule) => rules.push(rule),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        rules.sort_by_key(|rule| rule.order());
        CompositionOutput { rules, diagnostics }
    }

    /// Returns the current compiler configuration.
    #[must_use]
    pub const fn config(&self) -> &CompilerConfig {
        &self.config
    }

    /// Explains one candidate using the current compiler configuration.
    #[must_use]
    pub fn explain_candidate(&self, candidate: &str) -> ExplainResult {
        self.explain(ExplainRequest::new(candidate))
    }

    /// Parses, resolves, and explains one candidate without mutating compiler indexes.
    #[must_use]
    pub fn explain(&self, request: ExplainRequest<'_>) -> ExplainResult {
        let config = request.config.unwrap_or(&self.config);
        let candidate_text = request.candidate.to_owned();
        let parsed = match parse(request.candidate) {
            Ok(candidate) => candidate,
            Err(error) => {
                let diagnostic = error.to_diagnostic(SourceId::new(""), Span::empty(0));
                return ExplainResult {
                    candidate: candidate_text,
                    grammar_version: GRAMMAR_VERSION,
                    parsed: None,
                    normalized: request.candidate.to_owned(),
                    status: ResolutionStatus::InvalidSyntax,
                    css: None,
                    diagnostics: vec![diagnostic_to_explain(&diagnostic)],
                    alternatives: Vec::new(),
                    provenance: Vec::new(),
                    compatibility: request.compatibility.name().to_owned(),
                    browser_target: config.browser_target().as_str().to_owned(),
                };
            }
        };
        let parsed_owned = parsed.to_owned_ast();
        let normalized = parsed.normalized();
        let mut provenance = vec![
            ProvenanceEntry {
                kind: "preset".to_owned(),
                key: config.preset_name().to_owned(),
                detail: None,
            },
            ProvenanceEntry {
                kind: "utility".to_owned(),
                key: if parsed.utility().is_arbitrary_property() {
                    "[arbitrary-property]".to_owned()
                } else {
                    parsed.utility().family().to_owned()
                },
                detail: None,
            },
        ];
        for variant in parsed.variants() {
            let key = match variant.kind() {
                utilitycss_syntax::VariantKind::Named { name, .. } => name.to_owned(),
                utilitycss_syntax::VariantKind::Arbitrary { .. } => {
                    "[arbitrary-selector]".to_owned()
                }
                utilitycss_syntax::VariantKind::ArbitraryAtRule { name, .. } => {
                    format!("@{name}")
                }
            };
            provenance.push(ProvenanceEntry { kind: "variant".to_owned(), key, detail: None });
        }

        let utility = match resolve(&parsed, config.theme(), config.utilities()) {
            Ok(utility) => utility,
            Err(error) => {
                let diagnostic = error.to_diagnostic(SourceId::new(""), Span::empty(0));
                let mut alternatives = candidate_alternatives(
                    parsed.utility().family(),
                    config.utilities().families(),
                    "same utility family",
                );
                if error.kind() == UtilityErrorKind::UnknownThemeValue {
                    alternatives.extend(theme_alternatives(&parsed, config));
                    alternatives.sort_by(|left, right| {
                        left.score
                            .cmp(&right.score)
                            .then_with(|| left.candidate.cmp(&right.candidate))
                    });
                    alternatives.dedup_by(|left, right| left.candidate == right.candidate);
                    alternatives.truncate(8);
                }
                return ExplainResult {
                    candidate: candidate_text,
                    grammar_version: GRAMMAR_VERSION,
                    parsed: Some(parsed_owned),
                    normalized,
                    status: ResolutionStatus::Unresolved,
                    css: None,
                    diagnostics: vec![diagnostic_to_explain(&diagnostic)],
                    alternatives,
                    provenance,
                    compatibility: request.compatibility.name().to_owned(),
                    browser_target: config.browser_target().as_str().to_owned(),
                };
            }
        };
        if let Some(value) = parsed.utility().value() {
            provenance.push(value_provenance(&parsed, value, config));
        }
        let utility_order = utility.order();
        let tie_breaker = stable_hash(request.candidate.as_bytes());
        let rule = utility.into_rule(tie_breaker);
        let rule = match apply(&parsed, rule, config.theme(), config.variants()) {
            Ok(rule) => rule,
            Err(error) => {
                let diagnostic = error.to_diagnostic(SourceId::new(""), Span::empty(0));
                let alternatives = candidate_alternatives(
                    parsed.variants().first().map_or("", |variant| variant.raw()),
                    config.variants().names(),
                    "same variant context",
                );
                return ExplainResult {
                    candidate: candidate_text,
                    grammar_version: GRAMMAR_VERSION,
                    parsed: Some(parsed_owned),
                    normalized,
                    status: ResolutionStatus::Unresolved,
                    css: None,
                    diagnostics: vec![diagnostic_to_explain(&diagnostic)],
                    alternatives,
                    provenance,
                    compatibility: request.compatibility.name().to_owned(),
                    browser_target: config.browser_target().as_str().to_owned(),
                };
            }
        };
        let variant_order = config.variants().order(&parsed, config.theme());
        let ordered_rule =
            rule.with_order(OrderKey::new(0, variant_order, utility_order, tie_breaker));
        let browser_report = ordered_rule.browser_support(config.browser_target());
        let diagnostics = browser_diagnostics(&browser_report);
        let status = if browser_report.is_supported() {
            ResolutionStatus::Valid
        } else {
            ResolutionStatus::UnsupportedBrowser
        };
        let mut document = CssDocument::new();
        document.push(ordered_rule);
        ExplainResult {
            candidate: candidate_text,
            grammar_version: GRAMMAR_VERSION,
            parsed: Some(parsed_owned),
            normalized,
            status,
            css: Some(document.to_css(config.serialization_mode())),
            diagnostics: diagnostics.iter().map(diagnostic_to_explain).collect(),
            alternatives: Vec::new(),
            provenance,
            compatibility: request.compatibility.name().to_owned(),
            browser_target: config.browser_target().as_str().to_owned(),
        }
    }

    /// Validates one candidate and returns only the machine-oriented validation view.
    #[must_use]
    pub fn validate(&self, request: ExplainRequest<'_>) -> ValidationResult {
        let candidate = request.candidate.to_owned();
        let explanation = self.explain(request);
        ValidationResult {
            candidate,
            valid: explanation.status == ResolutionStatus::Valid
                && explanation.diagnostics.is_empty(),
            status: explanation.status,
            diagnostics: explanation.diagnostics,
            alternatives: explanation.alternatives,
        }
    }

    /// Returns deterministic completion items derived from utility and variant descriptors.
    #[must_use]
    pub fn completions(&self, prefix: &str) -> Vec<CompletionItem> {
        let mut items = BTreeMap::new();
        for descriptor in self.config.utilities().descriptors() {
            for name in descriptor.names {
                let label =
                    if descriptor.value_schema.required { format!("{name}-") } else { name };
                if label.starts_with(prefix) {
                    items.insert(
                        label.clone(),
                        CompletionItem {
                            label,
                            detail: descriptor.description.clone(),
                            documentation: descriptor.docs_key.clone(),
                        },
                    );
                }
            }
        }
        for descriptor in self.config.variants().descriptors() {
            for name in descriptor.names {
                let label = format!("{name}:");
                if label.starts_with(prefix) {
                    items.insert(
                        label.clone(),
                        CompletionItem {
                            label,
                            detail: descriptor.description.clone(),
                            documentation: descriptor.examples.join(", "),
                        },
                    );
                }
            }
        }
        for name in
            self.config.theme().namespace("breakpoint").into_iter().flat_map(|tokens| tokens.keys())
        {
            let label = format!("{name}:");
            if label.starts_with(prefix) {
                items.insert(
                    label.clone(),
                    CompletionItem {
                        label,
                        detail: "Responsive breakpoint variant".to_owned(),
                        documentation: "Wraps the rule in a min-width media query".to_owned(),
                    },
                );
            }
        }
        for (label, detail) in [
            ("data-[...]:", "Data attribute variant"),
            ("aria-[...]:", "ARIA attribute variant"),
            ("[&...]:", "Arbitrary selector variant"),
            ("[@supports(...)]:", "Arbitrary at-rule variant"),
        ] {
            if label.starts_with(prefix) {
                items.insert(
                    label.to_owned(),
                    CompletionItem {
                        label: label.to_owned(),
                        detail: detail.to_owned(),
                        documentation: "Use a validated selector or at-rule fragment".to_owned(),
                    },
                );
            }
        }
        items.into_values().collect()
    }

    /// Returns hover information for a valid or unresolved candidate.
    #[must_use]
    pub fn hover(&self, candidate: &str) -> Option<HoverInfo> {
        let explanation = self.explain_candidate(candidate);
        let parsed = explanation.parsed.as_ref()?;
        let description = self.config.utilities().descriptor(&parsed.utility.family).map_or_else(
            || "Unknown utility family".to_owned(),
            |descriptor| descriptor.description,
        );
        Some(HoverInfo {
            candidate: candidate.to_owned(),
            description,
            css: explanation.css,
            provenance: explanation.provenance,
        })
    }

    /// Returns the complete machine-readable capability manifest.
    #[must_use]
    pub fn capability_manifest(&self) -> CapabilityManifest {
        let mut variants = self.config.variants().descriptors();
        for (name, _) in
            self.config.theme().namespace("breakpoint").into_iter().flat_map(|tokens| tokens.iter())
        {
            if !variants.iter().any(|variant| variant.names.iter().any(|known| known == name)) {
                variants.push(utilitycss_variants::VariantDescriptor {
                    id: utilitycss_variants::VariantId::new(name.clone()),
                    names: vec![name.clone()],
                    description: "Responsive breakpoint variant".to_owned(),
                    composable: true,
                    ordering: utilitycss_variants::breakpoint_rank(name, self.config.theme())
                        .map_or(0, u32::from),
                    allowed_nesting: vec!["named".to_owned()],
                    compatibility_profile: "native".to_owned(),
                    examples: vec![format!("{name}:p-4")],
                    category: utilitycss_variants::VariantCategory::AtRule,
                });
            }
        }
        for (name, description, category) in [
            (
                "data-[...]",
                "Data attribute variant with an arbitrary attribute expression",
                utilitycss_variants::VariantCategory::Selector,
            ),
            (
                "aria-[...]",
                "ARIA attribute variant with an arbitrary attribute expression",
                utilitycss_variants::VariantCategory::Selector,
            ),
            (
                "[&...]",
                "Arbitrary selector variant",
                utilitycss_variants::VariantCategory::Selector,
            ),
            (
                "[@supports(...)]",
                "Arbitrary at-rule variant",
                utilitycss_variants::VariantCategory::AtRule,
            ),
        ] {
            if !variants.iter().any(|variant| variant.names.iter().any(|known| known == name)) {
                variants.push(utilitycss_variants::VariantDescriptor {
                    id: utilitycss_variants::VariantId::new(name),
                    names: vec![name.to_owned()],
                    description: description.to_owned(),
                    composable: true,
                    ordering: 500,
                    allowed_nesting: vec!["named".to_owned()],
                    compatibility_profile: "native".to_owned(),
                    examples: vec![format!("{name}:p-4")],
                    category,
                });
            }
        }
        variants.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        CapabilityManifest {
            schema_version: 1,
            grammar_version: GRAMMAR_VERSION,
            grammar: "candidate = [\"!\"] , { variant , \" : \" } , utility , [\"!\"] ; arbitrary = \"[\" , balanced-css , \"]\" ;".to_owned(),
            compiler_version: env!("CARGO_PKG_VERSION").to_owned(),
            active_preset: self.config.preset_name().to_owned(),
            utilities: self.config.utilities().descriptors(),
            variants,
            theme: self.config.theme().tokens(),
            diagnostics: diagnostic_catalog(),
            compatibility_profiles: vec![
                "native".to_owned(),
                "tailwind-v4-like".to_owned(),
                "tailwind-v3-like".to_owned(),
            ],
            extraction_modes: ["text", "static", "ast", "hybrid"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            browser_target: self.config.browser_target().as_str().to_owned(),
        }
    }

    /// Serializes the capability manifest using stable pretty JSON.
    #[must_use]
    pub fn capability_manifest_json(&self) -> String {
        serde_json::to_string_pretty(&self.capability_manifest())
            .unwrap_or_else(|_| "{}".to_owned())
    }

    /// Generates all vNext documentation and machine artifacts.
    #[must_use]
    pub fn generated_artifacts(&self) -> GeneratedArtifacts {
        let manifest = self.capability_manifest_json();
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://utilitycss.dev/schema/capabilities-v1.json",
            "title": "utilitycss capabilities",
            "description": "Generated capability manifest for the versioned utilitycss language.",
            "type": "object",
            "additionalProperties": false,
            "required": [
                "schema_version",
                "grammar_version",
                "grammar",
                "compiler_version",
                "active_preset",
                "utilities",
                "variants",
                "theme",
                "diagnostics",
                "compatibility_profiles",
                "extraction_modes",
                "browser_target"
            ],
            "properties": {
                "schema_version": {"type": "integer", "minimum": 1},
                "grammar_version": {"type": "integer", "minimum": 1},
                "grammar": {"type": "string", "minLength": 1},
                "compiler_version": {"type": "string", "minLength": 1},
                "active_preset": {"type": "string", "minLength": 1},
                "utilities": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/utilityDescriptor"}
                },
                "variants": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/variantDescriptor"}
                },
                "theme": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/themeToken"}
                },
                "diagnostics": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/diagnosticDescriptor"}
                },
                "compatibility_profiles": {
                    "type": "array",
                    "items": {"type": "string", "minLength": 1}
                },
                "extraction_modes": {
                    "type": "array",
                    "items": {"enum": ["text", "static", "ast", "hybrid"]}
                },
                "browser_target": {
                    "enum": ["modern", "evergreen", "safari-15", "legacy"]
                }
            },
            "$defs": {
                "valueNamespace": {
                    "enum": [
                        "Raw",
                        "Spacing",
                        "Color",
                        "Width",
                        "Height",
                        "Radius",
                        "FontFamily",
                        "FontSize",
                        "LineHeight",
                        "LetterSpacing",
                        "Shadow",
                        "Duration",
                        "Ease",
                        "ZIndex",
                        "Number",
                        "Length",
                        "Percentage",
                        "Integer",
                        "Keyword"
                    ]
                },
                "arbitraryPolicy": {
                    "enum": ["Unsupported", "Allowed", "Typed"]
                },
                "utilityCategory": {
                    "enum": [
                        "Layout",
                        "FlexGrid",
                        "SpacingSizing",
                        "Typography",
                        "BackgroundBorder",
                        "Effects",
                        "TransformsTransitions",
                        "Interactivity",
                        "TablesSvg",
                        "ModernCss",
                        "Custom"
                    ]
                },
                "negativePolicy": {"enum": ["Unsupported", "Allowed"]},
                "modifierKind": {
                    "enum": [
                        "Unsupported",
                        "Numeric",
                        "Percentage",
                        "ThemeBacked",
                        "Arbitrary",
                        "UtilitySpecific"
                    ]
                },
                "stability": {
                    "enum": ["Stable", "Experimental", "Deprecated", "CompatibilityOnly"]
                },
                "valueSchema": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["namespace", "required", "arbitrary", "allowed_values", "description"],
                    "properties": {
                        "namespace": {"$ref": "#/$defs/valueNamespace"},
                        "required": {"type": "boolean"},
                        "arbitrary": {"$ref": "#/$defs/arbitraryPolicy"},
                        "allowed_values": {"type": "array", "items": {"type": "string"}},
                        "description": {"type": "string"}
                    }
                },
                "modifierSchema": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "supported",
                        "required",
                        "namespace",
                        "kind",
                        "arbitrary",
                        "description"
                    ],
                    "properties": {
                        "supported": {"type": "boolean"},
                        "required": {"type": "boolean"},
                        "namespace": {"$ref": "#/$defs/valueNamespace"},
                        "kind": {"$ref": "#/$defs/modifierKind"},
                        "arbitrary": {"$ref": "#/$defs/arbitraryPolicy"},
                        "description": {"type": "string"}
                    }
                },
                "utilityExample": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["candidate", "description"],
                    "properties": {
                        "candidate": {"type": "string"},
                        "description": {"type": "string"}
                    }
                },
                "utilityDescriptor": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "id",
                        "names",
                        "category",
                        "description",
                        "value_schema",
                        "modifier_schema",
                        "negative_policy",
                        "arbitrary_policy",
                        "theme_namespaces",
                        "emitted_properties",
                        "dependencies",
                        "ordering_group",
                        "examples",
                        "docs_key",
                        "stability"
                    ],
                    "properties": {
                        "id": {"type": "string"},
                        "names": {"type": "array", "items": {"type": "string"}},
                        "category": {"$ref": "#/$defs/utilityCategory"},
                        "description": {"type": "string"},
                        "value_schema": {"$ref": "#/$defs/valueSchema"},
                        "modifier_schema": {"$ref": "#/$defs/modifierSchema"},
                        "negative_policy": {"$ref": "#/$defs/negativePolicy"},
                        "arbitrary_policy": {"$ref": "#/$defs/arbitraryPolicy"},
                        "theme_namespaces": {"type": "array", "items": {"type": "string"}},
                        "emitted_properties": {
                            "type": "array",
                            "items": {"type": "string"}
                        },
                        "dependencies": {"type": "array", "items": {"type": "string"}},
                        "ordering_group": {"type": "integer", "minimum": 0},
                        "examples": {
                            "type": "array",
                            "items": {"$ref": "#/$defs/utilityExample"}
                        },
                        "docs_key": {"type": "string"},
                        "stability": {"$ref": "#/$defs/stability"}
                    }
                },
                "variantCategory": {"enum": ["Pseudo", "Selector", "AtRule"]},
                "variantDescriptor": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "id",
                        "names",
                        "description",
                        "composable",
                        "ordering",
                        "allowed_nesting",
                        "compatibility_profile",
                        "examples",
                        "category"
                    ],
                    "properties": {
                        "id": {"type": "string"},
                        "names": {"type": "array", "items": {"type": "string"}},
                        "description": {"type": "string"},
                        "composable": {"type": "boolean"},
                        "ordering": {"type": "integer", "minimum": 0},
                        "allowed_nesting": {"type": "array", "items": {"type": "string"}},
                        "compatibility_profile": {"type": "string"},
                        "examples": {"type": "array", "items": {"type": "string"}},
                        "category": {"$ref": "#/$defs/variantCategory"}
                    }
                },
                "themeToken": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["namespace", "key", "value"],
                    "properties": {
                        "namespace": {"type": "string"},
                        "key": {"type": "string"},
                        "value": {"type": "string"}
                    }
                },
                "diagnosticDescriptor": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["code", "severity", "explanation"],
                    "properties": {
                        "code": {"type": "string"},
                        "severity": {"enum": ["error", "warning", "note", "help"]},
                        "explanation": {"type": "string"}
                    }
                }
            }
        });
        let reference = reference_markdown(&self.capability_manifest());
        let compact = llm_reference(&self.capability_manifest(), false);
        let full = llm_reference(&self.capability_manifest(), true);
        let compatibility = compatibility_report_json(&self.capability_manifest());
        GeneratedArtifacts {
            capabilities_json: manifest,
            schema_json: serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_owned()),
            reference_markdown: reference,
            llms_txt: compact,
            llms_full_txt: full,
            compatibility_report_json: compatibility,
        }
    }

    fn remove_candidate_reference(&mut self, raw: &str, source_id: &SourceId) {
        let mut remove_candidate = false;
        if let Some(source_ids) = self.candidate_sources.get_mut(raw) {
            source_ids.remove(source_id);
            remove_candidate = source_ids.is_empty();
        }
        if remove_candidate {
            self.candidate_sources.remove(raw);
            self.candidate_cache.remove(raw);
            self.pending_stats.rules_removed += 1;
        }
    }
}

fn source_size_error(error: SourceSizeError) -> CompilerError {
    CompilerError::SourceTooLarge { length: error.length() }
}

fn compile_candidate(raw: &str, config: &CompilerConfig) -> CandidateCacheEntry {
    let diagnostic_source = SourceId::new("");
    let diagnostic_span = Span::empty(0);
    let candidate = match parse(raw) {
        Ok(candidate) => candidate,
        Err(error) => {
            return CandidateCacheEntry {
                rule: None,
                diagnostics: vec![error.to_diagnostic(diagnostic_source, diagnostic_span)],
            };
        }
    };
    let utility = match resolve(&candidate, config.theme(), config.utilities()) {
        Ok(utility) => utility,
        Err(error) if error.kind() == UtilityErrorKind::UnknownUtility => {
            return CandidateCacheEntry { rule: None, diagnostics: Vec::new() };
        }
        Err(error) => {
            return CandidateCacheEntry {
                rule: None,
                diagnostics: vec![error.to_diagnostic(diagnostic_source, diagnostic_span)],
            };
        }
    };
    let utility_order = utility.order();
    let tie_breaker = stable_hash(raw.as_bytes());
    let rule = utility.into_rule(tie_breaker);
    let rule = match apply(&candidate, rule, config.theme(), config.variants()) {
        Ok(rule) => rule,
        Err(error) => {
            return CandidateCacheEntry {
                rule: None,
                diagnostics: vec![error.to_diagnostic(diagnostic_source, diagnostic_span)],
            };
        }
    };
    let variant_order = config.variants().order(&candidate, config.theme());
    let rule = rule.with_order(OrderKey::new(0, variant_order, utility_order, tie_breaker));
    let browser_report = rule.browser_support(config.browser_target());
    CandidateCacheEntry { rule: Some(rule), diagnostics: browser_diagnostics(&browser_report) }
}

fn browser_diagnostics(report: &BrowserSupportReport) -> Vec<Diagnostic> {
    report
        .unsupported
        .iter()
        .map(|feature| {
            Diagnostic::new(
                Severity::Warning,
                DiagnosticCode::new("browser.unsupported-feature"),
                format!(
                    "browser target `{}` does not guarantee support for `{}`",
                    report.target.as_str(),
                    feature.as_str()
                ),
            )
            .with_help("select a modern target or provide a compatibility fallback")
            .with_explanation(
                "The candidate remains valid, but the configured browser target may ignore part of the generated CSS.",
            )
        })
        .collect()
}

fn apply_diagnostic(
    code: &'static str,
    message: impl Into<String>,
    source: SourceId,
    span: Span,
    help: Option<&str>,
) -> Diagnostic {
    let diagnostic = Diagnostic::error(DiagnosticCode::new(code), message)
        .with_source(source)
        .with_span(span)
        .with_explanation("The explicit composition request could not be lowered safely.");
    help.map_or(diagnostic.clone(), |help| diagnostic.with_help(help))
}

fn validate_candidate(
    source: &SourceInput,
    candidate: &CandidateInput,
) -> Result<(), CompilerError> {
    let start = usize::try_from(candidate.span.start()).unwrap_or(usize::MAX);
    let end = usize::try_from(candidate.span.end()).unwrap_or(usize::MAX);
    let matches_source = source.content().get(start..end) == Some(candidate.raw.as_str());
    if matches_source {
        Ok(())
    } else {
        Err(CompilerError::InvalidCandidateSpan {
            raw: candidate.raw.clone(),
            span: candidate.span,
        })
    }
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_hash_with_seed(seed: u64, bytes: &[u8]) -> u64 {
    let mut hash = seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn stable_hash_text(seed: u64, value: &str) -> u64 {
    let hash =
        stable_hash_with_seed(seed, &u64::try_from(value.len()).unwrap_or(u64::MAX).to_le_bytes());
    stable_hash_with_seed(hash, value.as_bytes())
}

fn diagnostic_to_explain(diagnostic: &Diagnostic) -> ExplainDiagnostic {
    ExplainDiagnostic {
        severity: severity_name(diagnostic.severity()).to_owned(),
        code: diagnostic.code().to_string(),
        message: diagnostic.message().to_owned(),
        span: diagnostic.span(),
        help: diagnostic.help().map(str::to_owned),
        explanation: diagnostic.explanation().map(str::to_owned),
        suggestions: diagnostic
            .suggestions()
            .iter()
            .map(|suggestion| ExplainSuggestion {
                candidate: suggestion.replacement.clone(),
                score: 0,
                reason: suggestion.description.clone(),
            })
            .collect(),
    }
}

fn value_provenance(
    candidate: &utilitycss_syntax::CandidateAst<'_>,
    value: utilitycss_syntax::ValueAst<'_>,
    config: &CompilerConfig,
) -> ProvenanceEntry {
    let utility = candidate.utility();
    let descriptor_name = if utility.raw() != utility.family()
        && config.utilities().descriptor(utility.raw()).is_some()
    {
        utility.raw()
    } else {
        utility.family()
    };
    let namespace = config
        .utilities()
        .descriptor(descriptor_name)
        .and_then(|descriptor| descriptor.theme_namespaces.into_iter().next());
    if let (Some(namespace), utilitycss_syntax::ValueAst::Named(key)) = (namespace, value) {
        if let Some(resolved) = config.theme().token(&namespace, key) {
            return ProvenanceEntry {
                kind: "theme".to_owned(),
                key: format!("{namespace}:{key}"),
                detail: Some(format!("resolved to {resolved}")),
            };
        }
    }
    ProvenanceEntry {
        kind: "value".to_owned(),
        key: value.content().to_owned(),
        detail: value.type_hint().map(|hint| format!("typed as {hint}")),
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
        Severity::Help => "help",
    }
}

fn candidate_alternatives(
    input: &str,
    candidates: Vec<&str>,
    reason: &str,
) -> Vec<ExplainSuggestion> {
    let mut alternatives = candidates
        .into_iter()
        .map(|candidate| ExplainSuggestion {
            candidate: candidate.to_owned(),
            score: edit_distance(input, candidate),
            reason: reason.to_owned(),
        })
        .collect::<Vec<_>>();
    alternatives.sort_by(|left, right| {
        left.score.cmp(&right.score).then_with(|| left.candidate.cmp(&right.candidate))
    });
    alternatives.truncate(8);
    alternatives
}

fn theme_alternatives(
    candidate: &utilitycss_syntax::CandidateAst<'_>,
    config: &CompilerConfig,
) -> Vec<ExplainSuggestion> {
    let utility = candidate.utility();
    let descriptor_name = if utility.raw() != utility.family()
        && config.utilities().descriptor(utility.raw()).is_some()
    {
        utility.raw()
    } else {
        utility.family()
    };
    let Some(namespace) = config
        .utilities()
        .descriptor(descriptor_name)
        .and_then(|descriptor| descriptor.theme_namespaces.into_iter().next())
    else {
        return Vec::new();
    };
    let Some(tokens) = config.theme().namespace(&namespace) else {
        return Vec::new();
    };
    let value = utility.value().map(|value| value.content()).unwrap_or_default();
    let prefix = utility.family().to_owned();
    tokens
        .keys()
        .map(|key| ExplainSuggestion {
            candidate: format!("{prefix}-{key}"),
            score: edit_distance(value, key),
            reason: format!("known {namespace} theme value"),
        })
        .collect()
}

fn edit_distance(left: &str, right: &str) -> u32 {
    let right = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right.len()).collect::<Vec<_>>();
    for (row, left_character) in left.chars().enumerate() {
        let mut current = vec![row + 1; right.len() + 1];
        for (column, right_character) in right.iter().enumerate() {
            current[column + 1] = if left_character == *right_character {
                previous[column]
            } else {
                1 + previous[column].min(previous[column + 1]).min(current[column])
            };
        }
        previous = current;
    }
    u32::try_from(*previous.last().unwrap_or(&usize::MAX)).unwrap_or(u32::MAX)
}

fn diagnostic_catalog() -> Vec<DiagnosticDescriptor> {
    [
        ("syntax.empty-candidate", "error", "The candidate is empty."),
        ("syntax.empty-variant", "error", "A variant segment is empty."),
        ("syntax.empty-utility", "error", "The utility segment is empty."),
        ("syntax.missing-name", "error", "A utility or variant name is missing."),
        ("syntax.missing-value", "error", "The utility value is missing."),
        ("syntax.unterminated-arbitrary-value", "error", "An arbitrary bracket is not closed."),
        ("syntax.empty-arbitrary-value", "error", "An arbitrary value is empty."),
        (
            "syntax.unexpected-trailing-characters",
            "error",
            "Characters follow a complete arbitrary value.",
        ),
        (
            "syntax.invalid-arbitrary-property",
            "error",
            "An arbitrary property lacks a property/value separator.",
        ),
        ("syntax.invalid-utf8", "error", "Candidate bytes are not valid UTF-8."),
        ("utility.unknown", "error", "No registered utility matches the candidate."),
        ("utility.missing-value", "error", "The utility requires a value."),
        ("utility.unknown-theme-value", "error", "The value is absent from the active theme."),
        (
            "utility.invalid-arbitrary-value",
            "error",
            "The arbitrary CSS value is unsafe or malformed.",
        ),
        ("utility.unsupported-negative", "error", "The utility does not support negative values."),
        ("utility.invalid-value", "error", "The value is not accepted by the utility."),
        ("utility.unexpected-value", "error", "The utility does not accept a value."),
        ("utility.invalid-declaration", "error", "The generated declaration is unsafe."),
        ("utility.unsupported-modifier", "error", "The utility does not support this modifier."),
        ("utility.invalid-modifier", "error", "The modifier has an invalid value."),
        ("variant.unknown", "error", "No registered variant matches the candidate."),
        ("variant.invalid-value", "error", "The variant value is invalid."),
        ("variant.unsafe-selector", "error", "The selector or at-rule is unsafe."),
        (
            "browser.unsupported-feature",
            "warning",
            "The configured browser target may not support a generated CSS feature.",
        ),
    ]
    .into_iter()
    .map(|(code, severity, explanation)| DiagnosticDescriptor {
        code: code.to_owned(),
        severity: severity.to_owned(),
        explanation: explanation.to_owned(),
    })
    .collect()
}

fn reference_markdown(manifest: &CapabilityManifest) -> String {
    let mut output = String::from("# utilitycss generated reference\n\n");
    output.push_str(&format!("- Grammar version: `{}`\n", manifest.grammar_version));
    output.push_str(&format!("- Active preset: `{}`\n", manifest.active_preset));
    output.push_str(&format!("- Browser target: `{}`\n", manifest.browser_target));
    output.push_str(&format!("- Compiler version: `{}`\n\n", manifest.compiler_version));
    output.push_str("## Utilities\n\n");
    for utility in &manifest.utilities {
        output.push_str(&format!("- `{}` — {}\n", utility.names.join("`, `"), utility.description));
    }
    output.push_str("\n## Variants\n\n");
    for variant in &manifest.variants {
        output.push_str(&format!("- `{}` — {}\n", variant.names.join("`, `"), variant.description));
    }
    output
}

fn llm_reference(manifest: &CapabilityManifest, full: bool) -> String {
    let mut output = String::from("utilitycss candidate language\n");
    output.push_str(&format!(
        "grammar=v{}; preset={}; syntax=[important] [variant:] utility[-value][/modifier]\n",
        manifest.grammar_version, manifest.active_preset
    ));
    output.push_str(&format!(
        "browser-target={}; unsupported features are reported, not silently lowered\n",
        manifest.browser_target
    ));
    output.push_str("native-important=trailing !; arbitrary=[...]; arbitrary-property=[property:value]; arbitrary-at-rule=[@name(prelude)]\n");
    output.push_str("utilities:\n");
    for utility in &manifest.utilities {
        output.push_str(&format!(
            "- {}: {}; value={}; allowed={:?}; negative={:?}; arbitrary={:?}; modifier={:?}; dependencies={:?}\n",
            utility.names.join(","),
            utility.description,
            utility.value_schema.namespace.as_str(),
            utility.value_schema.allowed_values,
            utility.negative_policy,
            utility.arbitrary_policy,
            utility.modifier_schema.kind,
            utility.dependencies,
        ));
    }
    output.push_str("variants:\n");
    for variant in &manifest.variants {
        output.push_str(&format!(
            "- {}: {}; category={:?}\n",
            variant.names.join(","),
            variant.description,
            variant.category
        ));
    }
    if full {
        output.push_str("diagnostic-codes:\n");
        for diagnostic in &manifest.diagnostics {
            output.push_str(&format!("- {}: {}\n", diagnostic.code, diagnostic.explanation));
        }
        output.push_str("theme-tokens:\n");
        for token in &manifest.theme {
            output.push_str(&format!("- {}:{} = {}\n", token.namespace, token.key, token.value));
        }
    }
    output
}

fn compatibility_report_json(manifest: &CapabilityManifest) -> String {
    let report = serde_json::json!({
        "schema_version": 1,
        "profiles": manifest.compatibility_profiles,
        "native_utility_count": manifest.utilities.len(),
        "native_variant_count": manifest.variants.len(),
        "active_preset": manifest.active_preset,
        "browser_target": manifest.browser_target,
        "claims": [],
        "status": "scoped-fixtures-required-for-compatibility-claims"
    });
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_owned())
}

#[cfg(test)]
mod tests {
    use utilitycss_css_ir::{BrowserTarget, CssSerializationMode};
    use utilitycss_span::SourceId;

    use utilitycss_span::Span;

    use super::{
        ApplyCandidate, CandidateInput, Compiler, CompilerConfig, CompilerError, CompositionInput,
        SourceInput,
    };

    #[test]
    fn source_updates_are_replacements() {
        let mut compiler = Compiler::new(CompilerConfig::new());
        let id = SourceId::new("src/app.html");

        compiler
            .update_source(SourceInput::new(id.clone(), "<div />"))
            .expect("source ID is valid");
        compiler.update_source(SourceInput::new(id, "<main />")).expect("source ID is valid");

        assert_eq!(compiler.source_count(), 1);
        assert_eq!(compiler.build().css(), "");
        assert!(compiler.build().diagnostics().is_empty());
    }

    #[test]
    fn empty_source_ids_are_typed_errors() {
        let mut compiler = Compiler::new(CompilerConfig::new());
        let result = compiler.update_source(SourceInput::new(SourceId::new(""), "content"));

        assert_eq!(result, Err(CompilerError::EmptySourceId));
    }

    #[test]
    fn compiles_deduplicated_candidates_through_the_initial_pipeline() {
        let mut compiler = Compiler::new(
            CompilerConfig::new().with_serialization_mode(CssSerializationMode::Minified),
        );
        compiler
            .update_source(SourceInput::new(
                SourceId::new("src/app.html"),
                r#"<div class="flex p-4 p-4 hover:bg-red-500/50 md:grid"></div>"#,
            ))
            .expect("source ID is valid");

        let output = compiler.build();

        assert!(output.css().contains(".flex{display:flex;}"));
        assert!(output.css().contains(".p-4{padding:1rem;}"));
        assert!(output.css().contains("@media (min-width: 768px){"));
        assert_eq!(output.css().matches(".p-4{").count(), 1);
        assert!(output.diagnostics().is_empty());
    }

    #[test]
    fn incremental_build_matches_a_clean_rebuild() {
        let first_id = SourceId::new("src/first.html");
        let second_id = SourceId::new("src/second.html");
        let first_source = SourceInput::new(first_id.clone(), r#"<div class="p-4"></div>"#);
        let second_source = SourceInput::new(second_id.clone(), r#"<div class="flex"></div>"#);
        let changed_source =
            SourceInput::new(first_id.clone(), r#"<div class="p-8 hover:bg-red-500"></div>"#);

        let mut incremental = Compiler::new(CompilerConfig::new());
        incremental.update_source(first_source).expect("source ID is valid");
        incremental.update_source(second_source.clone()).expect("source ID is valid");
        let initial = incremental.build();
        assert!(initial.stats().candidates_parsed() > 0);

        incremental.update_source(changed_source.clone()).expect("source ID is valid");
        let incremental_output = incremental.build();

        let mut clean = Compiler::new(CompilerConfig::new());
        clean.update_source(changed_source).expect("source ID is valid");
        clean.update_source(second_source).expect("source ID is valid");
        let clean_output = clean.build();

        assert_eq!(incremental_output.css(), clean_output.css());
        assert_eq!(incremental_output.diagnostics(), clean_output.diagnostics());
        assert!(incremental_output.stats().cache_hits() > 0);
        assert!(incremental_output.stats().candidates_parsed() > 0);
    }

    #[test]
    fn no_op_updates_use_cached_candidates_and_removals_drop_rules() {
        let id = SourceId::new("src/app.html");
        let source = SourceInput::new(id.clone(), r#"<div class="p-4"></div>"#);
        let mut compiler = Compiler::new(CompilerConfig::new());

        compiler.update_source(source.clone()).expect("source ID is valid");
        let first = compiler.build();
        compiler.update_source(source).expect("same source ID is valid");
        let no_op = compiler.build();

        assert_eq!(no_op.stats().sources_scanned(), 0);
        assert_eq!(no_op.stats().candidates_parsed(), 0);
        assert!(no_op.stats().cache_hits() > 0);
        assert_eq!(no_op.css(), first.css());

        assert!(compiler.remove_source(&id));
        let removed = compiler.build();
        assert_eq!(removed.css(), "");
        assert!(removed.stats().rules_removed() > 0);
    }

    #[test]
    fn inactive_candidate_cache_entries_are_evicted() {
        let id = SourceId::new("src/generated.html");
        let mut compiler = Compiler::new(CompilerConfig::new());

        for cycle in 0..3 {
            let content = (0..1_000)
                .map(|index| format!("p-[{}px]", cycle * 1_000 + index))
                .collect::<Vec<_>>()
                .join(" ");
            let source = SourceInput::new(id.clone(), content);
            compiler.update_source(source).expect("generated source is valid");
            let populated = compiler.build();
            assert_eq!(populated.stats().unique_candidates(), 1_000);
            assert_eq!(compiler.candidate_cache.len(), 1_000);

            compiler
                .update_source(SourceInput::new(id.clone(), ""))
                .expect("replacement source is valid");
            let empty = compiler.build();
            assert_eq!(empty.css(), "");
            assert_eq!(empty.stats().unique_candidates(), 0);
            assert!(compiler.candidate_cache.is_empty());
        }
    }

    #[test]
    fn configuration_replacement_invalidates_semantic_cache() {
        let id = SourceId::new("src/app.html");
        let source = SourceInput::new(id, r#"<div class="bg-red-500"></div>"#);
        let mut compiler = Compiler::new(CompilerConfig::new());
        compiler.update_source(source).expect("source ID is valid");
        let first = compiler.build();

        let theme = utilitycss_theme::Theme::builder().color("red-500", "#123456").build();
        compiler.replace_config(CompilerConfig::new().with_theme(theme));
        let second = compiler.build();

        assert_ne!(first.css(), second.css());
        assert!(second.stats().candidates_parsed() > 0);
    }

    #[test]
    fn overlapping_custom_breakpoints_emit_in_min_width_order() {
        let theme = utilitycss_theme::Theme::builder()
            .breakpoint("xs", "320px")
            .breakpoint("2xl", "1536px")
            .build();
        let mut compiler = Compiler::new(CompilerConfig::new().with_theme(theme));
        let source = SourceInput::new(
            SourceId::new("src/app.html"),
            r#"<div class="2xl:p-4 md:p-4 xs:p-4"></div>"#,
        );
        compiler.update_source(source).expect("source is valid");

        let css = compiler.build().css().to_owned();
        let xs = css.find("(min-width: 320px)").expect("xs rule is emitted");
        let md = css.find("(min-width: 768px)").expect("md rule is emitted");
        let xxl = css.find("(min-width: 1536px)").expect("2xl rule is emitted");

        assert!(xs < md && md < xxl, "breakpoints must cascade by width: {css}");
        assert!(css.contains(r".\32 xl\:p-4"), "leading-digit selector escapes: {css}");
    }

    #[test]
    fn host_extractors_can_supply_validated_candidates() {
        let source = r#"const classes = clsx("p-4");"#;
        let start = source.find("p-4").expect("candidate is present");
        let end = start + "p-4".len();
        let mut compiler = Compiler::new(CompilerConfig::new());

        compiler
            .update_source_with_candidates(
                SourceInput::new(SourceId::new("src/app.tsx"), source),
                [CandidateInput::new(
                    "p-4",
                    Span::new(start as u32, end as u32).expect("span is ordered"),
                )],
            )
            .expect("candidate span matches source");

        assert!(compiler.build().css().contains(".p-4{"));
    }

    #[test]
    fn host_extractors_cannot_invent_candidate_spans() {
        let mut compiler = Compiler::new(CompilerConfig::new());
        let result = compiler.update_source_with_candidates(
            SourceInput::new(SourceId::new("src/app.tsx"), "const classes = \"p-4\";"),
            [CandidateInput::new("p-8", Span::new(17, 20).expect("span is ordered"))],
        );

        assert!(matches!(result, Err(CompilerError::InvalidCandidateSpan { .. })));
    }

    #[test]
    fn diagnostics_preserve_every_duplicate_candidate_occurrence() {
        let source =
            SourceInput::new(SourceId::new("src/app.html"), r#"<div class="p-[] p-[]"></div>"#);
        let mut compiler = Compiler::new(CompilerConfig::new());
        compiler.update_source(source).expect("source is valid");

        let output = compiler.build();
        let diagnostics = output.diagnostics().iter().collect::<Vec<_>>();

        assert_eq!(diagnostics.len(), 2);
        assert_ne!(diagnostics[0].span(), diagnostics[1].span());
    }

    #[test]
    fn composition_uses_the_callers_selector_and_existing_variants() {
        let source = SourceId::new("styles.css");
        let candidates = [
            ApplyCandidate::new("p-4", Span::new(0, 3).expect("span is ordered")),
            ApplyCandidate::new("hover:bg-red-500", Span::new(4, 21).expect("span is ordered")),
            ApplyCandidate::new("md:p-8", Span::new(22, 28).expect("span is ordered")),
        ];
        let mut compiler = Compiler::new(CompilerConfig::new());

        let output = compiler.compose(CompositionInput {
            source: &source,
            selector: ".button",
            candidates: &candidates,
        });

        assert!(output.diagnostics.is_empty());
        assert_eq!(output.rules[0].selector(), Some(".button"));
        assert_eq!(output.rules[0].declarations().expect("style rule")[0].property(), "padding");
        assert!(output.rules.iter().any(|rule| rule.selector() == Some(".button:hover")));
        assert!(output.rules.iter().any(|rule| rule.selector().is_none()));
    }

    #[test]
    fn explicit_unknown_composition_candidates_are_errors_with_their_span() {
        let source = SourceId::new("styles.css");
        let span = Span::new(10, 27).expect("span is ordered");
        let mut compiler = Compiler::new(CompilerConfig::new());

        let error = compiler
            .compose_candidate("definitely-not-a-utility", ".button", source.clone(), span)
            .expect_err("explicit unknown utilities must fail");

        assert_eq!(error.code().as_str(), "apply.unknown-utility");
        assert_eq!(error.source(), Some(&source));
        assert_eq!(error.span(), Some(span));
        assert!(error.help().is_some());
    }

    #[test]
    fn composition_rejects_unsafe_selectors_without_building_css() {
        let mut compiler = Compiler::new(CompilerConfig::new());
        let error = compiler
            .compose_candidate(
                "p-4",
                ".button{body{color:red}}",
                SourceId::new("styles.css"),
                Span::empty(0),
            )
            .expect_err("unsafe selector must be rejected");

        assert_eq!(error.code().as_str(), "apply.invalid-selector");
    }

    #[test]
    fn explain_validate_and_capabilities_are_registry_backed() {
        let compiler = Compiler::new(CompilerConfig::new());
        let explained = compiler.explain_candidate("md:hover:bg-red-500/50!");

        assert_eq!(explained.status, super::ResolutionStatus::Valid);
        assert!(explained.css.as_deref().is_some_and(|css| css.contains("50%")));
        assert_eq!(explained.grammar_version, utilitycss_syntax::GRAMMAR_VERSION);
        assert_eq!(explained.normalized, "md:hover:bg-red-500/50!");
        assert!(explained.provenance.iter().any(|entry| entry.kind == "utility"));
        assert!(explained.provenance.iter().any(|entry| entry.key == "color:red-500"
            && entry.detail.as_deref() == Some("resolved to #ef4444")));

        let invalid = compiler.validate(super::ExplainRequest::new("unknown-utility"));
        assert!(!invalid.valid);
        assert_eq!(invalid.status, super::ResolutionStatus::Unresolved);
        assert!(!invalid.alternatives.is_empty());

        let typo = compiler.validate(super::ExplainRequest::new("bg-reed-500"));
        assert!(typo.alternatives.iter().any(|alternative| {
            alternative.candidate == "bg-red-500"
                && alternative.reason.contains("color theme value")
        }));

        let manifest = compiler.capability_manifest();
        assert!(manifest
            .utilities
            .iter()
            .any(|utility| utility.names.iter().any(|name| name == "p")));
        assert!(manifest
            .variants
            .iter()
            .any(|variant| variant.names.iter().any(|name| name == "hover")));
        assert!(compiler.capability_manifest_json().contains("grammar_version"));
    }

    #[test]
    fn generated_capability_schema_describes_manifest_items() {
        let compiler = Compiler::new(CompilerConfig::new());
        let artifacts = compiler.generated_artifacts();
        let schema: serde_json::Value =
            serde_json::from_str(&artifacts.schema_json).expect("generated schema is valid JSON");

        assert_eq!(schema["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(schema["$id"], "https://utilitycss.dev/schema/capabilities-v1.json");
        assert_eq!(schema["properties"]["utilities"]["items"]["$ref"], "#/$defs/utilityDescriptor");
        assert_eq!(schema["properties"]["variants"]["items"]["$ref"], "#/$defs/variantDescriptor");
        assert_eq!(
            schema["$defs"]["modifierSchema"]["properties"]["kind"]["$ref"],
            "#/$defs/modifierKind"
        );
        assert_eq!(schema["$defs"]["valueSchema"]["properties"]["allowed_values"]["type"], "array");
        assert!(schema["required"]
            .as_array()
            .is_some_and(|fields| { fields.iter().any(|field| field == "browser_target") }));
    }

    #[test]
    fn completion_and_hover_are_not_hand_maintained_lists() {
        let compiler = Compiler::new(CompilerConfig::new());
        assert!(compiler.completions("grid").iter().any(|item| item.label == "grid"));
        assert!(compiler.completions("hover").iter().any(|item| item.label == "hover:"));
        let hover = compiler.hover("p-4").expect("valid candidates have hover data");
        assert!(hover.css.as_deref().is_some_and(|css| css.contains("padding")));
    }

    #[test]
    fn browser_target_reports_unsupported_features_without_dropping_css() {
        let compiler =
            Compiler::new(CompilerConfig::new().with_browser_target(BrowserTarget::Safari15));
        let explanation = compiler.explain_candidate("bg-red-500/50");

        assert_eq!(explanation.status, super::ResolutionStatus::UnsupportedBrowser);
        assert!(explanation.css.as_deref().is_some_and(|css| css.contains("color-mix")));
        assert_eq!(explanation.diagnostics[0].code, "browser.unsupported-feature");
        assert_eq!(explanation.browser_target, "safari-15");
    }
}
