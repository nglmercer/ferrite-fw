use crate::AttemptResult;
use serde::{Deserialize, Serialize};

/// Policy for retaining runner-owned attempt outputs after diagnostics/export.
/// Runner cleanup applies this after capture, live reporters and bundle export.
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
    pub(crate) fn parse(value: &str) -> crate::E2eResult<Self> {
        match value {
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            "failures-only" => Ok(Self::FailuresOnly),
            _ => Err(crate::E2eError::Config(format!(
                "invalid preserve_output {value:?}; expected always, never or failures-only"
            ))),
        }
    }
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
/// Visit only actual artifact links, never execution settings or caller origins.
/// Returning false removes a link, with ownership decisions made by the caller.
pub(crate) fn visit_artifacts(
    report: &mut crate::TestReport,
    mut visit: impl FnMut(&mut String) -> bool,
) {
    fn steps(records: &mut [crate::StepInfo], visit: &mut impl FnMut(&mut String) -> bool) {
        for step in records {
            step.attachments
                .retain_mut(|attachment| visit(&mut attachment.path));
            steps(&mut step.steps, visit);
        }
    }
    fn optional(path: &mut Option<String>, visit: &mut impl FnMut(&mut String) -> bool) {
        if path.as_mut().is_some_and(|path| !visit(path)) {
            *path = None;
        }
    }
    steps(&mut report.run_steps, &mut visit);
    for result in &mut report.results {
        result.screenshots.retain_mut(&mut visit);
        optional(&mut result.trace, &mut visit);
        optional(&mut result.video, &mut visit);
        result
            .attachments
            .retain_mut(|attachment| visit(&mut attachment.path));
        for attempt in &mut result.attempt_results {
            attempt.screenshots.retain_mut(&mut visit);
            optional(&mut attempt.trace, &mut visit);
            optional(&mut attempt.video, &mut visit);
            attempt
                .attachments
                .retain_mut(|attachment| visit(&mut attachment.path));
            steps(&mut attempt.steps, &mut visit);
        }
    }
}

/// Publish atomically, leaving the previous safe report intact on write failure.
pub(crate) fn write_report(path: &std::path::Path, data: &str) -> crate::E2eResult<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| crate::E2eError::Config("report path needs a parent".into()))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(data.as_bytes())?;
    staged
        .persist(path)
        .map_err(|error| crate::E2eError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_report_failure_leaves_existing_destination_untouched() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("report");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("caller"), "preserve").unwrap();
        assert!(write_report(&destination, "replacement").is_err());
        assert_eq!(
            std::fs::read_to_string(destination.join("caller")).unwrap(),
            "preserve"
        );
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn configuration_policy_is_validated_before_use() {
        assert_eq!(crate::E2eConfig::default().preserve_output, "always");
        for value in ["always", "never", "failures-only"] {
            let config = crate::E2eConfig {
                preserve_output: value.into(),
                ..Default::default()
            };
            assert!(crate::config::validate_config(&config).is_ok());
        }
        for value in ["", "failure", "Always", " never"] {
            let config = crate::E2eConfig {
                preserve_output: value.into(),
                ..Default::default()
            };
            assert!(crate::config::validate_config(&config).is_err());
        }
        let legacy: crate::ResolvedRunConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.output_retention, OutputRetention::Always);
    }
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
