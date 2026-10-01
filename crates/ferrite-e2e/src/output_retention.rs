use crate::AttemptResult;
use serde::{Deserialize, Serialize};

/// Policy for retaining runner-owned attempt outputs after diagnostics/export.
/// This value classifies attempts; runner filesystem integration is separate.
/// Successful attempts and expected failures are expected. Failed retry attempts,
/// interruptions, cleanup errors and unexpected passes are unexpected and retained
/// by `FailuresOnly`, even when a later retry recovers the test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputRetention {
    #[default]
    Always,
    Never,
    FailuresOnly,
}
impl OutputRetention {
    /// Use the final classification including cleanup and expectation checks,
    /// rather than guessing from an attempt's raw status or the final test status.
    #[must_use]
    pub fn retains_attempt(self, attempt: &AttemptResult) -> bool {
        self.retains_expected(attempt.is_expected)
    }
    fn retains_expected(self, is_expected: bool) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::FailuresOnly => !is_expected,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialized_policy_is_closed_and_defaults_to_preservation() {
        assert_eq!(OutputRetention::default(), OutputRetention::Always);
        for (name, policy) in [
            ("always", OutputRetention::Always),
            ("never", OutputRetention::Never),
            ("failures-only", OutputRetention::FailuresOnly),
        ] {
            assert_eq!(
                serde_json::from_value::<OutputRetention>(serde_json::json!(name)).unwrap(),
                policy
            );
            assert_eq!(serde_json::to_value(policy).unwrap(), name);
        }
        assert!(serde_json::from_value::<OutputRetention>(serde_json::json!("failure")).is_err());
        assert!(serde_json::from_value::<OutputRetention>(serde_json::json!(true)).is_err());
    }
    #[test]
    fn reference_outcome_classes_preserve_failed_retries_and_interruptions() {
        // Filesystem observations in output-retention-reference.json; final test
        // success must not hide an unexpected failed retry attempt.
        for (name, expected, keep) in [
            ("passing", true, false),
            ("expected failure", true, false),
            ("runtime skip", true, false),
            ("recovered retry failure", false, true),
            ("timeout", false, true),
            ("cleanup failure", false, true),
            ("interrupted", false, true),
            ("unexpected pass", false, true),
        ] {
            assert_eq!(
                OutputRetention::FailuresOnly.retains_expected(expected),
                keep,
                "{name}"
            );
            assert!(OutputRetention::Always.retains_expected(expected));
            assert!(!OutputRetention::Never.retains_expected(expected));
        }
    }
}
