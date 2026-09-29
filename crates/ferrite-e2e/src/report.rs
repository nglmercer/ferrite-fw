//! Test results and reporters (`list`, `json`, `junit`, `html`).

use serde::{Deserialize, Serialize};

/// Outcome of one test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    /// Passed (possibly after retries).
    Passed,
    /// Failed all attempts.
    Failed,
    /// Skipped by the filter.
    Skipped,
}

/// Result of one test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test name.
    pub name: String,
    /// Outcome.
    pub status: TestStatus,
    /// Attempts used (1 + retries).
    pub attempts: u32,
    /// Total wall time in milliseconds.
    pub duration_ms: u64,
    /// Failure message, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Screenshot artifact paths.
    #[serde(default)]
    pub screenshots: Vec<String>,
    /// Trace artifact path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// Video artifact path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
}

/// Aggregate report for a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TestReport {
    /// Per-test results in completion order.
    pub results: Vec<TestResult>,
}

impl TestReport {
    /// Number of passed tests.
    #[must_use]
    pub fn passed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count()
    }

    /// Number of failed tests.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count()
    }

    /// True when no test failed.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.failed() == 0
    }

    /// Process exit code (0 = all passed).
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        if self.ok() {
            0
        } else {
            1
        }
    }

    /// One-line summary.
    #[must_use]
    pub fn summary(&self) -> String {
        let skipped = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count();
        format!(
            "{} passed, {} failed, {} skipped ({} total)",
            self.passed(),
            self.failed(),
            skipped,
            self.results.len()
        )
    }

    /// Render the `list` reporter output.
    #[must_use]
    pub fn to_list(&self) -> String {
        let mut out = String::new();
        for result in &self.results {
            let mark = match result.status {
                TestStatus::Passed => "ok",
                TestStatus::Failed => "FAIL",
                TestStatus::Skipped => "skip",
            };
            out.push_str(&format!(
                "[{mark}] {} ({}ms, {} attempt{})\n",
                result.name,
                result.duration_ms,
                result.attempts,
                if result.attempts == 1 { "" } else { "s" },
            ));
            if let Some(error) = &result.error {
                for line in error.lines() {
                    out.push_str(&format!("       {line}\n"));
                }
            }
            for shot in &result.screenshots {
                out.push_str(&format!("       screenshot: {shot}\n"));
            }
            if let Some(trace) = &result.trace {
                out.push_str(&format!("       trace: {trace}\n"));
            }
            if let Some(video) = &result.video {
                out.push_str(&format!("       video: {video}\n"));
            }
        }
        out.push_str(&self.summary());
        out.push('\n');
        out
    }

    /// Render the `json` reporter output.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Render the `junit` reporter output.
    #[must_use]
    pub fn to_junit(&self) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str(&format!(
            "<testsuite name=\"ferrite-e2e\" tests=\"{}\" failures=\"{}\">\n",
            self.results.len(),
            self.failed()
        ));
        for result in &self.results {
            out.push_str(&format!(
                "  <testcase name=\"{}\" time=\"{:.3}\">\n",
                xml_escape(&result.name),
                result.duration_ms as f64 / 1000.0
            ));
            if result.status == TestStatus::Skipped {
                out.push_str("    <skipped/>\n");
            }
            if let Some(error) = &result.error {
                out.push_str(&format!(
                    "    <failure message=\"{}\"/>\n",
                    xml_escape(&one_line(error))
                ));
            }
            if let Some(video) = &result.video {
                out.push_str(&format!(
                    "    <properties><property name=\"video\" value=\"{}\"/></properties>\n",
                    xml_escape(video)
                ));
            }
            out.push_str("  </testcase>\n");
        }
        out.push_str("</testsuite>\n");
        out
    }

    /// Render the `html` reporter output (single self-contained file).
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut out = String::from(
            "<!doctype html><html><head><meta charset=\"utf-8\">\
             <title>ferrite e2e report</title><style>\
             body{font-family:sans-serif;margin:2em;color:#222}\
             table{border-collapse:collapse;width:100%}\
             th,td{border:1px solid #ccc;padding:.4em .6em;text-align:left;vertical-align:top}\
             th{background:#f0f0f0}pre{background:#f6f6f6;padding:.4em;white-space:pre-wrap}\
             .pill{display:inline-block;padding:.1em .6em;border-radius:1em;color:#fff;font-size:.85em}\
             .pass{background:#2a7}.fail{background:#c33}.skip{background:#888}\
             </style></head><body>",
        );
        out.push_str(&format!(
            "<h1>ferrite e2e</h1><p>{}</p>",
            xml_escape(&self.summary())
        ));
        out.push_str(
            "<table><thead><tr><th>status</th><th>test</th><th>time</th>\
             <th>attempts</th><th>details</th></tr></thead><tbody>",
        );
        for result in &self.results {
            let (label, class) = match result.status {
                TestStatus::Passed => ("passed", "pass"),
                TestStatus::Failed => ("failed", "fail"),
                TestStatus::Skipped => ("skipped", "skip"),
            };
            out.push_str(&format!(
                "<tr><td><span class=\"pill {class}\">{label}</span></td><td>{}</td>\
                 <td>{}ms</td><td>{}</td><td>",
                xml_escape(&result.name),
                result.duration_ms,
                result.attempts
            ));
            if let Some(error) = &result.error {
                out.push_str(&format!("<pre>{}</pre>", xml_escape(error)));
            }
            for shot in &result.screenshots {
                let href = xml_escape(shot);
                out.push_str(&format!("<a href=\"{href}\">screenshot</a> "));
            }
            if let Some(trace) = &result.trace {
                let href = xml_escape(trace);
                out.push_str(&format!("<a href=\"{href}\">trace</a> "));
            }
            if let Some(video) = &result.video {
                let href = xml_escape(video);
                out.push_str(&format!("<a href=\"{href}\">video</a>"));
            }
            out.push_str("</td></tr>");
        }
        out.push_str("</tbody></table></body></html>");
        out
    }
}

fn one_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> TestReport {
        TestReport {
            results: vec![
                TestResult {
                    name: "passes".to_string(),
                    status: TestStatus::Passed,
                    attempts: 1,
                    duration_ms: 120,
                    error: None,
                    screenshots: vec![],
                    trace: None,
                    video: None,
                },
                TestResult {
                    name: "fails <bad>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 3,
                    duration_ms: 4500,
                    error: Some("expect failed: title".to_string()),
                    screenshots: vec!["test-results/fails.png".to_string()],
                    trace: Some("test-results/fails.json".to_string()),
                    video: Some("test-results/fails.webm".to_string()),
                },
            ],
        }
    }

    #[test]
    fn summary_counts() {
        let report = sample();
        assert_eq!(report.passed(), 1);
        assert_eq!(report.failed(), 1);
        assert!(!report.ok());
        assert_eq!(report.exit_code(), 1);
        assert!(report.summary().contains("1 passed, 1 failed"));
    }

    #[test]
    fn list_marks_results() {
        let list = sample().to_list();
        assert!(list.contains("[ok] passes"), "{list}");
        assert!(list.contains("[FAIL] fails <bad>"), "{list}");
        assert!(
            list.contains("screenshot: test-results/fails.png"),
            "{list}"
        );
        assert!(list.contains("video: test-results/fails.webm"), "{list}");
    }

    #[test]
    fn json_round_trips() {
        let report = sample();
        let parsed: TestReport = serde_json::from_str(&report.to_json()).unwrap();
        assert_eq!(parsed.results.len(), 2);
    }

    #[test]
    fn html_renders_report() {
        let html = sample().to_html();
        assert!(html.contains("<title>ferrite e2e report</title>"), "{html}");
        assert!(html.contains("1 passed, 1 failed"), "{html}");
        assert!(html.contains("fails &lt;bad&gt;"), "{html}");
        assert!(html.contains("pill fail"), "{html}");
        assert!(html.contains("expect failed: title"), "{html}");
        assert!(html.contains("href=\"test-results/fails.png\""), "{html}");
        assert!(html.contains("href=\"test-results/fails.webm\""), "{html}");
    }

    #[test]
    fn junit_escapes_names() {
        let junit = sample().to_junit();
        assert!(junit.contains("tests=\"2\" failures=\"1\""), "{junit}");
        assert!(junit.contains("fails &lt;bad&gt;"), "{junit}");
        assert!(junit.contains("<failure"), "{junit}");
        assert!(
            junit.contains("<property name=\"video\" value=\"test-results/fails.webm\"/>"),
            "{junit}"
        );
    }
}
