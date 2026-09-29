//! Test results and reporters (`list`, `dot`, `json`, `junit`, `html`).

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Identity of one attempt. Retries and repetitions have separate identities.
#[derive(Debug, Clone)]
pub struct AttemptInfo {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub project: Option<String>,
    pub worker_index: usize,
    pub repeat_each_index: u32,
    pub retry: u32,
}

/// One named user step. Interrupted steps finish when their future is dropped.
#[derive(Debug, Clone)]
pub struct StepInfo {
    pub id: u64,
    pub title: String,
    pub duration_ms: u64,
    pub interrupted: bool,
}

/// Live runner callbacks, in lifecycle order within each attempt.
/// Callbacks are synchronous and may run concurrently on different workers.
/// Keep them short (enqueue slow uploads yourself). Callback panics are contained.
/// Aggregate file reporters remain available alongside these callbacks.
pub trait Reporter: Send + Sync + 'static {
    /// Discovered tests, before filtering or project expansion.
    fn on_begin(&self, _tests: &[crate::Test]) {}
    fn on_test_begin(&self, _attempt: &AttemptInfo) {}
    /// The result covers this attempt only; `attempts` is its one-based ordinal.
    fn on_test_end(&self, _attempt: &AttemptInfo, _result: &TestResult) {}
    fn on_step_begin(&self, _attempt: &AttemptInfo, _step: &StepInfo) {}
    fn on_step_end(&self, _attempt: &AttemptInfo, _step: &StepInfo) {}
    fn on_attachment(&self, _attempt: &AttemptInfo, _attachment: &Attachment) {}
    /// `None` denotes a run/worker error rather than an attempt error.
    fn on_error(&self, _attempt: Option<&AttemptInfo>, _error: &str) {}
    fn on_end(&self, _report: &TestReport) {}
}

#[derive(Clone, Default)]
pub(crate) struct ReporterHub(pub(crate) Vec<Arc<dyn Reporter>>);
impl ReporterHub {
    pub(crate) fn emit(&self, callback: impl Fn(&dyn Reporter)) {
        for reporter in &self.0 {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback(reporter.as_ref());
            }));
        }
    }
}

pub(crate) struct StepGuard {
    hub: ReporterHub,
    attempt: AttemptInfo,
    step: StepInfo,
    started: std::time::Instant,
}
impl StepGuard {
    pub(crate) fn new(hub: ReporterHub, attempt: AttemptInfo, title: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let mut step = StepInfo {
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            title: title.into(),
            duration_ms: 0,
            interrupted: false,
        };
        hub.emit(|r| r.on_step_begin(&attempt, &step));
        step.interrupted = true;
        Self {
            hub,
            attempt,
            step,
            started: std::time::Instant::now(),
        }
    }
    pub(crate) fn complete(&mut self) {
        self.step.interrupted = false;
    }
}
impl Drop for StepGuard {
    fn drop(&mut self) {
        self.step.duration_ms = self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        self.hub.emit(|r| r.on_step_end(&self.attempt, &self.step));
    }
}

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
    /// Failed as expected (`Test::fail`); does not fail the run.
    #[serde(rename = "expected")]
    FailedExpected,
}

/// One file attached to a test via [`TestInfo`](crate::runner::TestInfo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Attachment name.
    pub name: String,
    /// File path (under the output dir).
    pub path: String,
    /// MIME type.
    pub content_type: String,
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
    /// Project name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// `repeat_each` index (0 = single run).
    #[serde(default)]
    pub repeat_each_index: u32,
    /// Annotations as (kind, description) pairs.
    #[serde(default)]
    pub annotations: Vec<(String, String)>,
    /// Attached files.
    #[serde(default)]
    pub attachments: Vec<Attachment>,
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

    /// Number of expected failures.
    #[must_use]
    pub fn expected_failed(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == TestStatus::FailedExpected)
            .count()
    }

    /// One-line summary.
    #[must_use]
    pub fn summary(&self) -> String {
        let skipped = self
            .results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count();
        let expected = self.expected_failed();
        let mut out = format!(
            "{} passed, {} failed, {} skipped",
            self.passed(),
            self.failed(),
            skipped,
        );
        if expected > 0 {
            out.push_str(&format!(", {expected} expected-failed"));
        }
        out.push_str(&format!(" ({} total)", self.results.len()));
        out
    }

    /// Render the `dot` reporter output (one char per test + summary).
    ///
    /// `.` passed, `F` failed, `s` skipped, `E` failed-as-expected.
    #[must_use]
    pub fn to_dot(&self) -> String {
        let mut out = String::new();
        for result in &self.results {
            out.push(match result.status {
                TestStatus::Passed => '.',
                TestStatus::Failed => 'F',
                TestStatus::Skipped => 's',
                TestStatus::FailedExpected => 'E',
            });
        }
        out.push('\n');
        out.push_str(&self.summary());
        out.push('\n');
        out
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
                TestStatus::FailedExpected => "expected",
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
            if let Some(project) = &result.project {
                out.push_str(&format!("       project: {project}\n"));
            }
            for (kind, description) in &result.annotations {
                out.push_str(&format!("       annotation: {kind}={description}\n"));
            }
            for attachment in &result.attachments {
                out.push_str(&format!("       attachment: {}\n", attachment.path));
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
                "  <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">\n",
                xml_escape(&result.name),
                xml_escape(result.project.as_deref().unwrap_or("ferrite-e2e")),
                result.duration_ms as f64 / 1000.0
            ));
            if result.status == TestStatus::Skipped {
                out.push_str("    <skipped/>\n");
            }
            // Expected failures keep their message but never fail the suite.
            if result.status == TestStatus::Failed {
                if let Some(error) = &result.error {
                    out.push_str(&format!(
                        "    <failure message=\"{}\"/>\n",
                        xml_escape(&one_line(error))
                    ));
                }
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
             .exp{background:#b96}\
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
                TestStatus::FailedExpected => ("expected-failed", "exp"),
            };
            out.push_str(&format!(
                "<tr><td><span class=\"pill {class}\">{label}</span></td><td>{}</td>\
                 <td>{}ms</td><td>{}</td><td>",
                xml_escape(&result.name),
                result.duration_ms,
                result.attempts
            ));
            if let Some(project) = &result.project {
                out.push_str(&format!("<div>project: {}</div>", xml_escape(project)));
            }
            for (kind, description) in &result.annotations {
                out.push_str(&format!(
                    "<div>{}={}</div>",
                    xml_escape(kind),
                    xml_escape(description)
                ));
            }
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
                out.push_str(&format!("<a href=\"{href}\">video</a> "));
            }
            for attachment in &result.attachments {
                let href = xml_escape(&attachment.path);
                out.push_str(&format!(
                    "<a href=\"{href}\">{}</a> ",
                    xml_escape(&attachment.name)
                ));
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

    fn sample_result(name: &str, status: TestStatus) -> TestResult {
        TestResult {
            name: name.to_string(),
            status,
            attempts: 1,
            duration_ms: 120,
            error: None,
            screenshots: vec![],
            trace: None,
            video: None,
            project: None,
            repeat_each_index: 0,
            annotations: Vec::new(),
            attachments: Vec::new(),
        }
    }

    fn sample() -> TestReport {
        TestReport {
            results: vec![
                sample_result("passes", TestStatus::Passed),
                TestResult {
                    name: "fails <bad>".to_string(),
                    status: TestStatus::Failed,
                    attempts: 3,
                    duration_ms: 4500,
                    error: Some("expect failed: title".to_string()),
                    screenshots: vec!["test-results/fails.png".to_string()],
                    trace: Some("test-results/fails.json".to_string()),
                    video: Some("test-results/fails.webm".to_string()),
                    project: Some("shop".to_string()),
                    repeat_each_index: 0,
                    annotations: vec![("flaky".to_string(), "retry".to_string())],
                    attachments: vec![Attachment {
                        name: "console".to_string(),
                        path: "test-results/console.txt".to_string(),
                        content_type: "text/plain".to_string(),
                    }],
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
        assert!(junit.contains("classname=\"shop\""), "{junit}");
        assert!(
            junit.contains("<property name=\"video\" value=\"test-results/fails.webm\"/>"),
            "{junit}"
        );
    }

    #[test]
    fn dot_marks_statuses() {
        let report = TestReport {
            results: vec![
                sample_result("a", TestStatus::Passed),
                sample_result("b", TestStatus::Failed),
                sample_result("c", TestStatus::Skipped),
                sample_result("d", TestStatus::FailedExpected),
            ],
        };
        let dot = report.to_dot();
        assert!(dot.starts_with(".FsE\n"), "{dot}");
        assert!(dot.contains("1 expected-failed"), "{dot}");
    }

    #[test]
    fn expected_failures_do_not_fail_the_run() {
        let mut expected = sample_result("flaky", TestStatus::FailedExpected);
        expected.error = Some("timed out".to_string());
        let report = TestReport {
            results: vec![expected],
        };
        assert_eq!(report.expected_failed(), 1);
        assert!(report.ok());
        assert_eq!(report.exit_code(), 0);
        assert!(
            !report.to_junit().contains("<failure"),
            "must not fail suite"
        );
        assert!(report.to_list().contains("[expected] flaky"));
        assert!(report.to_html().contains("pill exp"));
    }

    #[test]
    fn list_shows_project_annotations_attachments() {
        let list = sample().to_list();
        assert!(list.contains("project: shop"), "{list}");
        assert!(list.contains("annotation: flaky=retry"), "{list}");
        assert!(
            list.contains("attachment: test-results/console.txt"),
            "{list}"
        );
    }
}

#[cfg(test)]
mod live_reporter_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct PanicReporter;
    impl Reporter for PanicReporter {
        fn on_begin(&self, _: &[crate::Test]) {
            panic!("reporter panic");
        }
    }
    struct Counter(Arc<AtomicUsize>);
    impl Reporter for Counter {
        fn on_begin(&self, _: &[crate::Test]) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn callback_panic_does_not_block_other_reporters() {
        let count = Arc::new(AtomicUsize::new(0));
        let hub = ReporterHub(vec![
            Arc::new(PanicReporter),
            Arc::new(Counter(count.clone())),
        ]);
        hub.emit(|r| r.on_begin(&[]));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
