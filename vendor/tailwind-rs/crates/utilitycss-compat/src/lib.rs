//! Explicit, opt-in compatibility profiles for utilitycss.
//!
//! Compatibility is represented as data and reporting. Native compiler semantics remain the
//! default, and this package does not claim 1:1 compatibility with another implementation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use utilitycss_compiler::{ExplainRequest, ResolutionStatus};

pub use utilitycss_compiler::CompatibilityProfile;
pub use utilitycss_config::{CompatibilityPreset, Preset};

/// A single expected compatibility behavior described as portable fixture data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityFixture {
    /// Profile name used to evaluate the candidate.
    pub profile: String,
    /// Candidate text to explain.
    pub candidate: String,
    /// Expected serialized CSS. `None` means that validity alone is asserted.
    pub expected_css: Option<String>,
}

impl CompatibilityFixture {
    /// Creates a fixture with an expected CSS string.
    #[must_use]
    pub fn new(
        profile: impl Into<String>,
        candidate: impl Into<String>,
        expected_css: impl Into<String>,
    ) -> Self {
        Self {
            profile: profile.into(),
            candidate: candidate.into(),
            expected_css: Some(expected_css.into()),
        }
    }

    /// Creates a fixture that asserts only successful resolution.
    #[must_use]
    pub fn valid(profile: impl Into<String>, candidate: impl Into<String>) -> Self {
        Self { profile: profile.into(), candidate: candidate.into(), expected_css: None }
    }
}

/// Result for one executed compatibility fixture.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompatibilityCaseResult {
    /// Profile evaluated for this case.
    pub profile: String,
    /// Candidate evaluated for this case.
    pub candidate: String,
    /// Whether the actual result matched the fixture.
    pub passed: bool,
    /// Actual compiler CSS, when resolution succeeded.
    pub actual_css: Option<String>,
    /// Expected CSS from the fixture.
    pub expected_css: Option<String>,
    /// Semantic resolution status.
    pub status: ResolutionStatus,
    /// Stable diagnostic codes returned by the compiler.
    pub diagnostic_codes: Vec<String>,
}

/// Deterministic report produced by the compatibility fixture harness.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompatibilityReport {
    /// Report schema version.
    pub schema_version: u16,
    /// Number of fixture cases.
    pub total: usize,
    /// Number of passing cases.
    pub passed: usize,
    /// Number of failing cases.
    pub failed: usize,
    /// Results in fixture input order.
    pub cases: Vec<CompatibilityCaseResult>,
}

/// Converts a portable profile name to the compiler profile enum.
#[must_use]
pub fn profile_from_name(name: &str) -> CompatibilityProfile {
    match name {
        "native" => CompatibilityProfile::Native,
        "tailwind-v4-like" | "tailwind-v4-subset" => CompatibilityProfile::TailwindV4Like,
        "tailwind-v3-like" | "tailwind-v3-subset" => CompatibilityProfile::TailwindV3Like,
        other => CompatibilityProfile::Custom(other.to_owned()),
    }
}

/// Parses a JSON array of compatibility fixtures.
pub fn parse_fixtures_json(input: &str) -> Result<Vec<CompatibilityFixture>, serde_json::Error> {
    serde_json::from_str(input)
}

/// Runs portable compatibility fixtures against one compiler configuration.
#[must_use]
pub fn run_fixtures(
    compiler: &utilitycss_compiler::Compiler,
    fixtures: &[CompatibilityFixture],
) -> CompatibilityReport {
    let cases = fixtures
        .iter()
        .map(|fixture| {
            let explanation = compiler.explain(
                ExplainRequest::new(&fixture.candidate)
                    .with_compatibility(profile_from_name(&fixture.profile)),
            );
            let actual_css = explanation.css.clone();
            let passed = explanation.status == ResolutionStatus::Valid
                && fixture
                    .expected_css
                    .as_deref()
                    .is_none_or(|expected| actual_css.as_deref() == Some(expected));
            CompatibilityCaseResult {
                profile: fixture.profile.clone(),
                candidate: fixture.candidate.clone(),
                passed,
                actual_css,
                expected_css: fixture.expected_css.clone(),
                status: explanation.status,
                diagnostic_codes: explanation
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.code.clone())
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let passed = cases.iter().filter(|case| case.passed).count();
    CompatibilityReport {
        schema_version: 1,
        total: cases.len(),
        passed,
        failed: cases.len().saturating_sub(passed),
        cases,
    }
}

/// Serializes a compatibility fixture report as deterministic pretty JSON.
pub fn run_fixtures_json(
    compiler: &utilitycss_compiler::Compiler,
    fixtures: &[CompatibilityFixture],
) -> String {
    serde_json::to_string_pretty(&run_fixtures(compiler, fixtures)).unwrap_or_else(|_| {
        "{\"schema_version\":1,\"total\":0,\"passed\":0,\"failed\":0,\"cases\":[]}".to_owned()
    })
}

/// Returns the deterministic compatibility report for a compiler configuration.
#[must_use]
pub fn report(compiler: &utilitycss_compiler::Compiler) -> String {
    compiler.generated_artifacts().compatibility_report_json
}

#[cfg(test)]
mod tests {
    use super::{run_fixtures, CompatibilityFixture};
    use utilitycss_compiler::{Compiler, CompilerConfig};

    #[test]
    fn fixture_harness_reports_css_mismatches_deterministically() {
        let compiler = Compiler::new(CompilerConfig::new());
        let report = run_fixtures(
            &compiler,
            &[
                CompatibilityFixture::new("native", "p-4", ".p-4{padding:1rem;}"),
                CompatibilityFixture::new("native", "p-4", ".p-4{padding:2rem;}"),
            ],
        );

        assert_eq!((report.total, report.passed, report.failed), (2, 1, 1));
        assert!(report.cases[0].passed);
        assert!(!report.cases[1].passed);
    }
}
