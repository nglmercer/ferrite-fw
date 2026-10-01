//! Bounded summaries of scheduled Rust test/project results, not source files.
use crate::{E2eError, E2eResult, TestReport};
use serde::{Deserialize, Serialize};

pub use ferrite_config::SlowTestOptions;

/// One scheduled result. Its index distinguishes duplicate names and projects;
/// retries contribute through the result's total wall duration only once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlowTestSummary {
    pub result_index: usize,
    pub name: String,
    pub project: Option<String>,
    pub repeat_each_index: u32,
    pub duration_ms: u64,
    pub attempts: u32,
}
impl TestReport {
    /// Configured bounded summary; disabled for old/unconfigured reports.
    pub fn slow_tests(&self) -> E2eResult<Vec<SlowTestSummary>> {
        match self
            .configuration
            .as_ref()
            .and_then(|config| config.report_slow_tests)
        {
            Some(options) => self.slow_tests_with(options),
            None => Ok(Vec::new()),
        }
    }
    /// Select durations strictly above the threshold, longest first. Equal
    /// durations preserve report order. Each result remains distinct; there is
    /// no grouping by names or source files and no summing overlapping attempts.
    pub fn slow_tests_with(&self, options: SlowTestOptions) -> E2eResult<Vec<SlowTestSummary>> {
        options.validate().map_err(E2eError::Config)?;
        let mut selected: Vec<SlowTestSummary> = Vec::with_capacity(options.max);
        if options.max == 0 {
            return Ok(selected);
        }
        for (result_index, result) in self.results.iter().enumerate() {
            if result.duration_ms <= options.threshold_ms {
                continue;
            }
            let position =
                selected.partition_point(|entry| entry.duration_ms >= result.duration_ms);
            if position >= options.max {
                continue;
            }
            if selected.len() == options.max {
                selected.pop();
            }
            selected.insert(
                position,
                SlowTestSummary {
                    result_index,
                    name: result.name.clone(),
                    project: result.project.clone(),
                    repeat_each_index: result.repeat_each_index,
                    duration_ms: result.duration_ms,
                    attempts: result.attempts,
                },
            );
        }
        Ok(selected)
    }
}
impl TestReport {
    pub(crate) fn report_context_text(&self) -> String {
        let mut output = String::new();
        if let Some(config) = &self.configuration {
            if let Some(name) = &config.run_name {
                output.push_str(&format!(
                    "Run name: {}\n",
                    serde_json::to_string(name).unwrap()
                ));
            }
            if let Some(metadata) = &config.metadata {
                output.push_str(&format!(
                    "Run metadata: {}\n",
                    serde_json::to_string(metadata).unwrap()
                ));
            }
            for project in &config.projects {
                if let Some(metadata) = &project.metadata {
                    output.push_str(&format!(
                        "Project {:?} metadata: {}\n",
                        project.name,
                        serde_json::to_string(metadata).unwrap()
                    ));
                }
            }
        }
        match self.slow_tests() {
            Ok(entries) => {
                for entry in entries {
                    output.push_str(&format!(
                        "Slow result #{}: {:?}, project {:?}, repeat {}, {}ms ({} attempts)\n",
                        entry.result_index,
                        entry.name,
                        entry.project,
                        entry.repeat_each_index,
                        entry.duration_ms,
                        entry.attempts
                    ));
                }
            }
            Err(error) => output.push_str(&format!("Slow-test summary unavailable: {error}\n")),
        }
        output
    }
    pub(crate) fn run_context_html(&self) -> String {
        let Some(config) = &self.configuration else {
            return String::new();
        };
        let mut output = String::new();
        if let Some(name) = &config.run_name {
            output.push_str(&format!(
                "<p class=\"run-name\">Run: {}</p>",
                crate::report::xml_escape(name)
            ));
        }
        if let Some(metadata) = &config.metadata {
            output.push_str(&format!("<details class=\"run-metadata\"><summary>Run metadata</summary><pre>{}</pre></details>", crate::report::xml_escape(&serde_json::to_string_pretty(metadata).unwrap())));
        }
        for project in &config.projects {
            if let Some(metadata) = &project.metadata {
                output.push_str(&format!("<details class=\"project-metadata\"><summary>Project {} metadata</summary><pre>{}</pre></details>", crate::report::xml_escape(project.name.as_deref().unwrap_or("default")), crate::report::xml_escape(&serde_json::to_string_pretty(metadata).unwrap())));
            }
        }
        output
    }
    pub(crate) fn slow_tests_html(&self) -> String {
        let Some(options) = self
            .configuration
            .as_ref()
            .and_then(|config| config.report_slow_tests)
        else {
            return String::new();
        };
        if options.max == 0 {
            return String::new();
        }
        let entries = match self.slow_tests() {
            Ok(entries) => entries,
            Err(error) => {
                return format!(
                    "<p>Slow-test summary unavailable: {}</p>",
                    crate::report::xml_escape(&error.to_string())
                )
            }
        };
        let mut output = format!("<section class=\"slow-tests\"><h2>Slow tests</h2><p>Whole-run scheduled results above {}ms; at most {} entries. Retries count once in each result’s total duration.</p>", options.threshold_ms, options.max);
        if entries.is_empty() {
            output.push_str("<p>No results exceeded the threshold.</p>");
        } else {
            output.push_str("<table><thead><tr><th>Result</th><th>Test</th><th>Project</th><th>Repeat</th><th>Duration</th><th>Attempts</th></tr></thead><tbody>");
            for entry in entries {
                output.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}ms</td><td>{}</td></tr>",
                    entry.result_index,
                    crate::report::xml_escape(&entry.name),
                    crate::report::xml_escape(entry.project.as_deref().unwrap_or("default")),
                    entry.repeat_each_index,
                    entry.duration_ms,
                    entry.attempts
                ));
            }
            output.push_str("</tbody></table>");
        }
        output.push_str("</section>");
        output
    }
    pub(crate) fn metadata_xml_properties(&self) -> String {
        let mut output = String::new();
        let mut property = |name: &str, value: String| {
            output.push_str(&format!(
                "<property name=\"{name}\" value=\"{}\"/>",
                crate::report::xml_escape(&value)
            ))
        };
        if let Some(config) = &self.configuration {
            if let Some(name) = &config.run_name {
                property("ferrite.run.name", serde_json::to_string(name).unwrap());
            }
            if let Some(metadata) = &config.metadata {
                property(
                    "ferrite.run.metadata",
                    serde_json::to_string(metadata).unwrap(),
                );
            }
        }
        match self.slow_tests() {
            Ok(entries) if !entries.is_empty() => property(
                "ferrite.slow_tests",
                serde_json::to_string(&entries).unwrap(),
            ),
            Err(error) => property(
                "ferrite.slow_tests.error",
                serde_json::to_string(&error.to_string()).unwrap(),
            ),
            _ => {}
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(durations: &[u64]) -> TestReport {
        TestReport {
            results: durations
                .iter()
                .map(|duration| {
                    serde_json::from_value(serde_json::json!({
                        "name":"duplicate", "status":"passed", "duration_ms":duration,
                        "attempts":2, "project":"project", "repeat_each_index":1
                    }))
                    .unwrap()
                })
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn bounded_order_and_duplicate_identity_use_result_duration_once() {
        let report = report(&[100, 50, 100, 10, 200]);
        let entries = report
            .slow_tests_with(SlowTestOptions {
                threshold_ms: 50,
                max: 3,
            })
            .unwrap();
        assert_eq!(
            entries.iter().map(|e| e.result_index).collect::<Vec<_>>(),
            [4, 0, 2]
        );
        assert_eq!(
            entries.iter().map(|e| e.duration_ms).collect::<Vec<_>>(),
            [200, 100, 100]
        );
        assert!(entries
            .iter()
            .all(|e| e.attempts == 2 && e.repeat_each_index == 1));
        let encoded = serde_json::to_string(&entries).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<SlowTestSummary>>(&encoded).unwrap(),
            entries
        );
    }
    #[test]
    fn configured_serialization_escaping_and_legacy_reports_agree() {
        let mut report = report(&[100, 200]);
        let metadata = [
            (
                "unsafe".into(),
                serde_json::json!("<script>bad</script>&\"\n\u{0}"),
            ),
            (
                "nested".into(),
                serde_json::json!([true, null, {"snow":"雪"}]),
            ),
        ]
        .into_iter()
        .collect();
        report.configuration = Some(crate::ResolvedRunConfig {
            run_name: Some("<script>name</script>\u{0}".into()),
            metadata: Some(metadata),
            report_slow_tests: Some(SlowTestOptions {
                threshold_ms: 0,
                max: 2,
            }),
            projects: vec![crate::ResolvedProjectConfig {
                name: Some("project".into()),
                metadata: Some(Default::default()),
                ..Default::default()
            }],
            ..Default::default()
        });
        let json = report.to_json();
        let copy: TestReport = serde_json::from_str(&json).unwrap();
        assert_eq!(copy.to_json(), json);
        assert_eq!(copy.slow_tests().unwrap(), report.slow_tests().unwrap());
        let html = report.to_html();
        assert!(!html.contains("<script>bad"));
        assert!(html.contains("&lt;script&gt;bad"));
        assert!(html.contains("class=\"slow-tests\""));
        let xml = report.to_junit();
        assert!(!xml.contains("<script>"));
        assert!(!xml.contains('\u{0}'));
        assert!(xml.contains("ferrite.run.metadata"));
        assert!(xml.contains("ferrite.project.metadata"));
        assert!(xml.contains("ferrite.slow_tests"));
        assert!(report.to_list().contains("Slow result #1"));
        let legacy: TestReport = serde_json::from_str("{\"results\":[]}").unwrap();
        assert!(legacy.slow_tests().unwrap().is_empty());
        assert!(!legacy.to_html().contains("class=\"run-metadata\""));
        assert!(!legacy.to_html().contains("class=\"slow-tests\""));
    }
    #[test]
    fn invalid_configured_summary_fails_bundle_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("absent");
        let mut report = report(&[1]);
        report.configuration = Some(crate::ResolvedRunConfig {
            report_slow_tests: Some(SlowTestOptions {
                threshold_ms: 0,
                max: 1001,
            }),
            ..Default::default()
        });
        assert!(serde_json::to_string(&report).is_err());
        assert!(report.write_bundle(&destination).is_err());
        assert!(!destination.exists());
    }
    #[test]
    fn maximum_storage_stays_bounded_for_large_reports() {
        let durations: Vec<_> = (0..5000).collect();
        let entries = report(&durations)
            .slow_tests_with(SlowTestOptions {
                threshold_ms: 0,
                max: 1000,
            })
            .unwrap();
        assert_eq!(entries.len(), 1000);
        assert_eq!(entries[0].result_index, 4999);
        assert_eq!(entries[999].result_index, 4000);
    }
    #[test]
    fn disabled_empty_strict_threshold_and_maximum_validation() {
        let report = report(&[15_000]);
        assert!(report
            .slow_tests_with(Default::default())
            .unwrap()
            .is_empty());
        assert!(report
            .slow_tests_with(SlowTestOptions {
                threshold_ms: 0,
                max: 0
            })
            .unwrap()
            .is_empty());
        assert!(report
            .slow_tests_with(SlowTestOptions {
                threshold_ms: 0,
                max: 1001
            })
            .is_err());
        assert!(TestReport::default()
            .slow_tests_with(Default::default())
            .unwrap()
            .is_empty());
        assert_eq!(
            serde_json::from_str::<SlowTestOptions>("{}").unwrap(),
            SlowTestOptions::default()
        );
    }
}
