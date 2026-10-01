//! Native label derivation, errors, cancellation, retries and owned reports.
use ferrite_e2e::*;
use serde_json::{json, Value};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

const BUDGET: Duration = Duration::from_secs(5);
struct LocalNumber(std::rc::Rc<u64>);
impl serde::Serialize for LocalNumber {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(self.0.as_ref(), serializer)
    }
}
impl<'de> serde::Deserialize<'de> for LocalNumber {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        <u64 as serde::Deserialize>::deserialize(deserializer)
            .map(|value| Self(std::rc::Rc::new(value)))
    }
}
const HTML: &str = "<section><button>A</button><button>B</button></section><iframe srcdoc=\"<button>Frame</button>\"></iframe>";

async fn browsers() -> Vec<Browser> {
    let mut result = Vec::new();
    for kind in [BrowserKind::Chromium, BrowserKind::Firefox] {
        let executable = match kind {
            BrowserKind::Chromium => find_chromium(None),
            BrowserKind::Firefox => find_firefox(None),
        };
        let Some(executable) = executable else {
            continue;
        };
        let browser = Browser::launch(
            LaunchOptions::default()
                .browser(kind)
                .executable(executable),
        )
        .await
        .unwrap();
        eprintln!(
            "locator descriptions {} {}",
            kind.name(),
            browser.version().await.unwrap()
        );
        result.push(browser);
    }
    if std::env::var_os("FERRITE_E2E_REQUIRE_BOTH_BROWSERS").is_some() {
        assert_eq!(result.len(), 2);
    }
    result
}
fn reference(name: &str) -> Value {
    let data: Value = serde_json::from_str(include_str!(
        "../../../scripts/e2e-conformance/locator-description-reference.json"
    ))
    .unwrap();
    assert_eq!(data["playwright"], "1.63.0");
    data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["result"]
        .clone()
}
fn flatten(steps: &[StepInfo]) -> Vec<&StepInfo> {
    steps
        .iter()
        .flat_map(|step| std::iter::once(step).chain(flatten(&step.steps)))
        .collect()
}

#[tokio::test]
async fn pinned_derivation_rules_preserve_resolution_frames_and_clone_decorators() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(HTML).await.unwrap();
        let original = page.locator("section");
        let labeled = original.describe("Checkout");
        let other = original.describe("Other");
        let frame = page
            .locator("iframe")
            .describe("Payment frame")
            .content_frame();
        let variants = [
            ("original", original.clone()),
            ("labeled", labeled.clone()),
            ("replaced", labeled.describe("Replacement")),
            ("empty", labeled.describe("")),
            ("removed", labeled.clear_description()),
            ("first", labeled.first()),
            ("last", labeled.last()),
            ("nth", labeled.nth(0)),
            ("filter", labeled.filter("A")),
            ("visible", labeled.visible()),
            ("scoped", labeled.locator("button")),
            ("role", labeled.get_by_role("button", "")),
            ("union", labeled.or_(&other).unwrap()),
            ("intersection", labeled.and_(&other).unwrap()),
            ("all", labeled.all().await.unwrap().remove(0)),
            ("frameOwner", frame.owner()),
            ("framePick", frame.first().owner()),
            ("frameChild", frame.locator("button")),
        ];
        let result: serde_json::Map<_, _> = variants
            .iter()
            .map(|(name, locator)| ((*name).into(), json!(locator.description())))
            .collect();
        assert_eq!(Value::Object(result), reference("description-chaining"));
        assert_eq!(
            json!({"original":original.count().await.unwrap(),"labeled":labeled.count().await.unwrap(),
            "child":labeled.locator("button").count().await.unwrap(),"unchanged":original.description().is_none()}),
            reference("same-resolution")
        );
        assert_eq!(labeled.selector(), original.selector());
        assert_eq!(labeled.to_string(), "Checkout");
        assert_eq!(original.to_string(), "section");
        assert!(
            labeled.exact().description().is_none()
                && labeled.matching("A").description().is_none()
        );
        assert!(labeled.strict().description().is_none());
        assert_eq!(
            labeled.with_timeout(Duration::ZERO).description(),
            Some("Checkout")
        );
        assert_eq!(
            labeled
                .with_cancellation(CancellationToken::new())
                .description(),
            Some("Checkout")
        );
        frame
            .clone()
            .locator("button")
            .describe("Frame button")
            .expect()
            .text("Frame")
            .await
            .unwrap();
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[tokio::test]
async fn native_action_assertion_validation_and_opaque_errors_keep_labels_and_codes() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content(HTML).await.unwrap();
        let missing = page
            .locator("#missing")
            .describe("Checkout")
            .with_timeout(Duration::from_millis(70));
        let error = missing.click().await.unwrap_err();
        assert!(matches!(error, E2eError::Timeout(_, _)), "{error:?}");
        let result = json!({"operation":error.to_string().contains("locator.click"),"description":error.to_string().contains("Checkout"),"selector":error.to_string().contains("#missing")});
        let upstream = reference("action-error");
        assert_eq!(result["operation"], upstream["operation"]);
        assert_eq!(result["selector"], upstream["selector"]);
        assert_eq!(upstream["description"], false);
        assert_eq!(result["description"], true); // Intentional richer Rust diagnostics.
        assert_eq!(error.to_string().matches("Checkout").count(), 1);
        let button = page.locator("button").first().describe("Action button");
        let local: LocalNumber = button.evaluate("el => 42").await.unwrap();
        assert_eq!(*local.0, 42);
        let echoed: u64 = button
            .evaluate_with_arg("(el, value) => value", &local)
            .await
            .unwrap();
        assert_eq!(echoed, 42);
        button
            .expect()
            .satisfies("local callback", |_| {
                let value = local.0.clone();
                async move { Ok(*value == 42) }
            })
            .await
            .unwrap();
        let error = button
            .drag_to_with_options(&button, DragOptions::default().steps(0))
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Config(_)));
        assert!(error.to_string().contains("locator.drag_to_with_options"));
        assert!(error.to_string().contains("Action button"));
        let error = button
            .evaluate::<u64>("el => 'not a number'")
            .await
            .unwrap_err();
        assert_eq!(error.code(), "FERRITE_E2E_JSON");
        assert!(
            matches!(error,E2eError::Diagnostic { ref source,.. } if matches!(**source,E2eError::Json(_)))
        );
        assert!(
            error.to_string().contains("locator.evaluate")
                && error.to_string().contains("Action button")
        );
        let token = CancellationToken::new();
        token.cancel_with_reason("caller cancellation");
        let error = missing.with_cancellation(token).click().await.unwrap_err();
        assert!(matches!(error, E2eError::Cancelled(_)), "{error:?}");
        assert!(
            error.to_string().contains("Checkout")
                && error.to_string().contains("caller cancellation")
        );
        let error = button
            .expect()
            .timeout(Duration::from_millis(60))
            .text_with(
                &TextMatcher::exact("wrong"),
                TextAssertionOptions::default(),
            )
            .await
            .unwrap_err();
        assert!(matches!(error, E2eError::Expect(_)), "{error:?}");
        assert!(
            error.to_string().contains("expect.text_with")
                && error.to_string().contains("Action button")
        );
        assert_eq!(error.to_string().matches("Action button").count(), 1);
        page.close().await.unwrap();
        browser.close().await.unwrap();
    }
}

#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<StepInfo>>>);
impl Reporter for Events {
    fn on_step_end(&self, _: &AttemptInfo, step: &StepInfo) {
        self.0.lock().unwrap().push(step.clone());
    }
}
const LABEL: &str = "Checkout <script>bad()</script> & \"雪\"";
#[tokio::test]
async fn native_retry_steps_live_events_traces_json_and_html_retain_owned_labels_once() {
    for browser in browsers().await {
        let kind = browser.kind();
        let dir = tempfile::tempdir().unwrap();
        let events = Events::default();
        let report = Runner::from_config(&E2eConfig {
            screenshot: "off".into(),
            ..Default::default()
        })
        .workers(1)
        .retries(1)
        .test_timeout(BUDGET)
        .cleanup_timeout(BUDGET)
        .output_dir(dir.path().display().to_string())
        .list_progress(false)
        .custom_reporter(events.clone())
        .run(
            &browser,
            vec![test_with_context("label retries", |ctx| async move {
                ctx.page.set_content(HTML).await?;
                let button = ctx
                    .page
                    .locator("button")
                    .first()
                    .describe("Successful action");
                button.click().await?;
                button
                    .expect()
                    .text_with(&TextMatcher::exact("A"), TextAssertionOptions::default())
                    .await?;
                let missing = ctx
                    .page
                    .locator("#missing")
                    .describe(LABEL)
                    .with_timeout(Duration::from_millis(90));
                ctx.page
                    .step("user action", async { missing.click().await })
                    .await
            })],
        )
        .await;
        assert_eq!(report.failed(), 1);
        let attempts = &report.results[0].attempt_results;
        assert_eq!(attempts.len(), 2);
        for (index, attempt) in attempts.iter().enumerate() {
            assert_eq!(attempt.info.retry, index as u32);
            let steps = flatten(&attempt.steps);
            let labeled: Vec<_> = steps
                .iter()
                .filter(|step| step.title.contains("description:"))
                .collect();
            assert_eq!(
                labeled.len(),
                3,
                "{:?}",
                steps.iter().map(|step| &step.title).collect::<Vec<_>>()
            );
            for step in &labeled {
                assert_eq!(step.location.file, attempt.info.file);
                assert_eq!(step.location.line, attempt.info.line);
            }
            let failed = labeled
                .iter()
                .find(|step| step.title.starts_with("locator.click #missing"))
                .unwrap();
            assert!(failed
                .error
                .as_ref()
                .unwrap()
                .message
                .contains("<script>bad()</script>"));
            assert_eq!(failed.error.as_ref().unwrap().code, "FERRITE_E2E_TIMEOUT");
            let parent = steps
                .iter()
                .find(|step| step.title == "user action")
                .unwrap();
            assert_eq!(failed.parent_id, Some(parent.id));
            let live = events.0.lock().unwrap();
            assert!(live.iter().any(|step| step.id == failed.id
                && step.error.as_ref().unwrap().code == "FERRITE_E2E_TIMEOUT"));
            drop(live);
            let trace: Value = serde_json::from_str(
                &std::fs::read_to_string(attempt.trace.as_ref().unwrap()).unwrap(),
            )
            .unwrap();
            let operations: Vec<Value> = trace["trace"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|entry| entry["kind"] == "locator-operation")
                .map(|entry| serde_json::from_str(entry["detail"].as_str().unwrap()).unwrap())
                .collect();
            assert_eq!(operations.len(), 3, "{operations:?}");
            assert_eq!(
                operations
                    .iter()
                    .filter(|entry| entry["description"] == LABEL)
                    .count(),
                1
            );
        }
        browser.close().await.unwrap();
        let json = report.to_json();
        let restored: TestReport = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.results[0].attempt_results.len(), 2);
        assert!(json.contains("Checkout") && json.contains("Successful action"));
        let html = report.to_html();
        assert!(!html.contains("<script>bad()</script>"));
        assert!(html.contains("&lt;script&gt;bad()&lt;/script&gt;"));
        if let Some(root) = std::env::var_os("FERRITE_LOCATOR_REPORT_PREVIEW") {
            report
                .write_bundle(Path::new(&root).join(kind.name()))
                .unwrap();
        }
    }
}

async fn assert_probe_control(page: &Page, code: &str) {
    let locator = page.locator("#audit").describe("control audit");
    let finite = Duration::from_millis(60);
    let title = tokio::time::timeout(
        Duration::from_secs(1),
        page.expect().timeout(Duration::ZERO).title("unused"),
    )
    .await
    .expect("control error must settle with assertion timeout disabled");
    assert_eq!(title.unwrap_err().code(), code);
    let visible = tokio::time::timeout(
        Duration::from_secs(1),
        locator.expect().timeout(Duration::ZERO).visible(),
    )
    .await
    .expect("locator control error must settle with assertion timeout disabled");
    assert_eq!(visible.unwrap_err().code(), code);
    for result in [
        page.expect().timeout(finite).url("about:blank").await,
        locator.expect().timeout(finite).text("unused").await,
        locator.expect().timeout(finite).count(1).await,
        locator
            .expect()
            .not()
            .timeout(finite)
            .attribute("id", "audit")
            .await,
        locator
            .expect()
            .timeout(finite)
            .css("color", "unused")
            .await,
        locator
            .expect()
            .timeout(finite)
            .js_property("disabled", &false)
            .await,
        locator
            .expect()
            .timeout(finite)
            .accessible_name("unused")
            .await,
        locator.expect().timeout(finite).texts(&["unused"]).await,
    ] {
        assert_eq!(result.unwrap_err().code(), code);
    }
    let wait = tokio::time::timeout(
        Duration::from_secs(1),
        locator.wait_for_function("el => true", Duration::ZERO),
    )
    .await
    .expect("locator function must preserve control errors");
    assert_eq!(wait.unwrap_err().code(), code);
}

#[tokio::test]
async fn native_assertion_probes_preserve_cancellation_disconnect_and_interrupted_reports() {
    for browser in browsers().await {
        let page = browser.new_page().await.unwrap();
        page.set_content("<button id='audit'>Ready</button>")
            .await
            .unwrap();
        let token = CancellationToken::new();
        token.cancel_with_reason("audit caller cancelled");
        assert_probe_control(&page.with_cancellation(token), "FERRITE_E2E_CANCELLED").await;
        page.locator("#audit").expect().visible().await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        let report = Runner::from_config(&E2eConfig {
            screenshot: "off".into(),
            ..Default::default()
        })
        .workers(1)
        .test_timeout(BUDGET)
        .cleanup_timeout(BUDGET)
        .output_dir(dir.path().display().to_string())
        .list_progress(false)
        .run(
            &browser,
            vec![test_with_context(
                "assertion control cause",
                |ctx| async move {
                    ctx.page
                        .set_content("<button id='audit'>Ready</button>")
                        .await?;
                    let token = CancellationToken::new();
                    token.cancel_with_reason("audit reported cancellation");
                    ctx.page
                        .locator("#audit")
                        .describe("reported control audit")
                        .with_cancellation(token)
                        .expect()
                        .timeout(Duration::ZERO)
                        .visible()
                        .await
                },
            )],
        )
        .await;
        let attempt = &report.results[0].attempt_results[0];
        assert_eq!(attempt.status, AttemptStatus::Interrupted);
        assert_eq!(attempt.errors[0].code, "FERRITE_E2E_CANCELLED");
        let interrupted: Vec<_> = flatten(&attempt.steps)
            .into_iter()
            .filter(|step| step.title.contains("reported control audit"))
            .collect();
        assert!(!interrupted.is_empty());
        for step in interrupted {
            assert_eq!(step.status, StepStatus::Interrupted);
            assert!(step.interrupted);
            assert_eq!(step.error.as_ref().unwrap().code, "FERRITE_E2E_CANCELLED");
        }
        if let Some(connection) = browser.cdp() {
            connection.close();
        } else {
            browser.bidi().unwrap().close();
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while browser.cdp().map_or_else(
                || browser.bidi().unwrap().is_open(),
                |connection| connection.is_open(),
            ) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("actual transport disconnection");
        assert_probe_control(&page, "FERRITE_E2E_DISCONNECTED").await;
        browser.close().await.unwrap();
    }
}
