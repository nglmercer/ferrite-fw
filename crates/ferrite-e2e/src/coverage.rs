//! Chromium JavaScript and CSS coverage. Firefox rejects these APIs explicitly.
use crate::{BrowserKind, E2eError, E2eResult, OperationOptions, Page};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// V8 source range. Offsets are UTF-16 code units, matching CDP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
    pub start_offset: u64,
    pub end_offset: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageFunction {
    pub function_name: String,
    pub ranges: Vec<CoverageRange>,
    pub is_block_coverage: bool,
}

/// Source and complete V8 function/block ranges, including uncovered blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsCoverageEntry {
    pub url: String,
    pub script_id: String,
    pub source: String,
    #[serde(default)]
    pub source_status: CoverageSourceStatus,
    pub functions: Vec<CoverageFunction>,
}

/// Source and CSS rule-use ranges. Offsets use CDP's UTF-16 convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssCoverageEntry {
    pub url: String,
    pub style_sheet_id: String,
    pub source: String,
    #[serde(default)]
    pub source_status: CoverageSourceStatus,
    pub ranges: Vec<CoverageRange>,
}

/// Whether source was collected, omitted by options, or unavailable natively.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum CoverageSourceStatus {
    #[default]
    Included,
    Omitted,
    Unavailable(String),
    Truncated {
        limit: usize,
    },
}

/// JavaScript collection options. Raw V8 ranges are preserved.
#[derive(Debug, Clone)]
pub struct JsCoverageOptions {
    pub reset_on_navigation: bool,
    pub report_anonymous_scripts: bool,
    pub include_source: bool,
    pub operation: OperationOptions,
}
impl Default for JsCoverageOptions {
    fn default() -> Self {
        Self {
            reset_on_navigation: true,
            report_anonymous_scripts: false,
            include_source: true,
            operation: OperationOptions::default(),
        }
    }
}
/// CSS collection options. Anonymous styles remain supported for compatibility.
#[derive(Debug, Clone)]
pub struct CssCoverageOptions {
    pub reset_on_navigation: bool,
    pub include_source: bool,
    pub operation: OperationOptions,
}
impl Default for CssCoverageOptions {
    fn default() -> Self {
        Self {
            reset_on_navigation: true,
            include_source: true,
            operation: OperationOptions::default(),
        }
    }
}
const MAX_ENTRIES: usize = 4096;
const MAX_SOURCE: usize = 1024 * 1024;
const MAX_HISTORY: usize = 16 * 1024 * 1024;
const MAX_RANGES: usize = 100_000;
fn bounded_reason(reason: &str) -> String {
    let mut end = reason.len().min(4096);
    while !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_owned()
}
#[derive(Clone)]
struct SourceRecord {
    url: String,
    source: String,
    status: CoverageSourceStatus,
}
impl SourceRecord {
    fn size(&self) -> usize {
        self.url.len()
            + self.source.len()
            + match &self.status {
                CoverageSourceStatus::Unavailable(reason) => reason.len(),
                _ => 0,
            }
    }
}
#[derive(Default)]
struct Collection {
    records: HashMap<String, SourceRecord>,
    bytes: usize,
    failure: Option<String>,
}
impl Collection {
    fn fail(&mut self, reason: &str) {
        if self.failure.is_none() {
            self.failure = Some(bounded_reason(reason));
            self.records.clear();
            self.bytes = 0;
        }
    }
    fn insert(&mut self, id: String, record: SourceRecord) {
        if id.len() > 1024 || record.url.len() > 16 * 1024 {
            self.fail("coverage identity exceeds retained metadata limits");
            return;
        }
        let old = self.records.get(&id).map_or(0, SourceRecord::size);
        let bytes = self.bytes.saturating_sub(old)
            + record.size()
            + if !self.records.contains_key(&id) {
                id.len()
            } else {
                0
            };
        if (self.records.len() >= MAX_ENTRIES && !self.records.contains_key(&id))
            || bytes > MAX_HISTORY
        {
            self.fail("coverage retained source history capacity exceeded");
            return;
        }
        self.bytes = bytes;
        self.records.insert(id, record);
    }
    fn navigation(&mut self, reset: bool) {
        if reset {
            self.records.clear();
            self.bytes = 0;
        }
        // An earlier lag/capacity failure cannot be hidden by navigation.
    }
    fn check(&self) -> E2eResult<()> {
        match &self.failure {
            Some(reason) => Err(E2eError::Config(format!("coverage incomplete: {reason}"))),
            None => Ok(()),
        }
    }
}
struct Capture {
    data: Arc<Mutex<Collection>>,
    flush: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::AbortHandle,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Capture {
    async fn flush(&self) -> E2eResult<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self.flush.send(tx).await.is_err() || rx.await.is_err() {
            self.data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .check()?;
            return Err(E2eError::Disconnected(
                "coverage event collector ended".into(),
            ));
        }
        self.data.lock().unwrap_or_else(|e| e.into_inner()).check()
    }
}
#[derive(Default)]
pub(crate) struct CoverageState {
    js: Option<Capture>,
    css: Option<Capture>,
    js_dirty: bool,
    css_dirty: bool,
}
impl CoverageState {
    pub(crate) fn cancel(&mut self) {
        self.js = None;
        self.css = None;
    }
}
impl Drop for CoverageState {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Js,
    Css,
}
impl Kind {
    fn source(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Js => ("Debugger.getScriptSource", "scriptId", "scriptSource"),
            Self::Css => ("CSS.getStyleSheetText", "styleSheetId", "text"),
        }
    }
}
#[derive(Clone, Copy)]
struct CollectOptions {
    kind: Kind,
    reset: bool,
    include: bool,
    anonymous: bool,
}
async fn process_event(
    driver: &crate::driver::Driver,
    event: crate::cdp::CdpEvent,
    session: &str,
    data: &Arc<Mutex<Collection>>,
    options: CollectOptions,
) {
    let CollectOptions {
        kind,
        reset,
        include,
        anonymous,
    } = options;
    if event.session.as_deref() != Some(session) {
        return;
    }
    if event.method == "Runtime.executionContextsCleared" {
        data.lock()
            .unwrap_or_else(|e| e.into_inner())
            .navigation(reset);
        return;
    }
    let (id, url) = match kind {
        Kind::Js if event.method == "Debugger.scriptParsed" => (
            event.params["scriptId"].as_str().unwrap_or_default(),
            event.params["url"].as_str().unwrap_or_default(),
        ),
        Kind::Css if event.method == "CSS.styleSheetAdded" => (
            event.params["header"]["styleSheetId"]
                .as_str()
                .unwrap_or_default(),
            event.params["header"]["sourceURL"]
                .as_str()
                .unwrap_or_default(),
        ),
        _ => return,
    };
    if id.is_empty() || (matches!(kind, Kind::Js) && url.is_empty() && !anonymous) {
        return;
    }
    if id.len() > 1024 || url.len() > 16 * 1024 {
        data.lock()
            .unwrap_or_else(|e| e.into_inner())
            .fail("coverage identity exceeds retained metadata limits");
        return;
    }
    let (method, key, result_key) = kind.source();
    let (source, status) = if !include {
        (String::new(), CoverageSourceStatus::Omitted)
    } else {
        match driver
            .coverage_call(method, serde_json::json!({(key):id}))
            .await
        {
            Ok(reply) => match reply[result_key].as_str() {
                Some(source) if source.len() <= MAX_SOURCE => {
                    (source.to_owned(), CoverageSourceStatus::Included)
                }
                Some(_) => (
                    String::new(),
                    CoverageSourceStatus::Truncated { limit: MAX_SOURCE },
                ),
                None => (
                    String::new(),
                    CoverageSourceStatus::Unavailable("native source reply missing text".into()),
                ),
            },
            Err(error) => (
                String::new(),
                CoverageSourceStatus::Unavailable(bounded_reason(&error.to_string())),
            ),
        }
    };
    let url = url.to_owned();
    data.lock().unwrap_or_else(|e| e.into_inner()).insert(
        id.to_owned(),
        SourceRecord {
            url,
            source,
            status,
        },
    );
}
fn collector(
    page: &Page,
    kind: Kind,
    reset: bool,
    include: bool,
    anonymous: bool,
) -> E2eResult<Capture> {
    let options = CollectOptions {
        kind,
        reset,
        include,
        anonymous,
    };
    let (session, mut events) = page.cdp_events()?;
    let data = Arc::new(Mutex::new(Collection::default()));
    let collected = data.clone();
    let driver = page.driver.clone(); // No Page/CoverageState owner cycle.
    let (flush, mut commands) = tokio::sync::mpsc::channel::<tokio::sync::oneshot::Sender<()>>(1);
    let task = tokio::spawn(async move {
        loop {
            let next = driver
                .run(async {
                    tokio::select! { biased;
                        event = events.recv() => Ok(Some(Err(event))),
                        command = commands.recv() => Ok(command.map(Ok)),
                    }
                })
                .await;
            match next {
                Ok(Some(Err(Ok(event)))) => {
                    process_event(&driver, event, &session, &collected, options).await
                }
                Ok(Some(Ok(reply))) => {
                    let _ = reply.send(());
                }
                Ok(None) => break,
                Ok(Some(Err(Err(error)))) => {
                    collected
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .fail(&format!("native coverage event stream: {error}"));
                    break;
                }
                Err(error) => {
                    collected
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .fail(&error.to_string());
                    break;
                }
            }
            if collected
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .failure
                .is_some()
            {
                break;
            }
        }
    })
    .abort_handle();
    Ok(Capture { data, flush, task })
}
async fn cleanup(page: &Page, kind: Kind) -> E2eResult<()> {
    let methods: &[&str] = match kind {
        Kind::Js => &[
            "Profiler.stopPreciseCoverage",
            "Profiler.disable",
            "Debugger.disable",
        ],
        Kind::Css => &["CSS.stopRuleUsageTracking", "CSS.disable"],
    };
    for method in methods {
        match page.call(method, serde_json::json!({})).await {
            Ok(_) => {}
            Err(E2eError::Cdp { .. }) if *method == methods[0] => {} // Already stopped/not enabled.
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
/// Chromium-only coverage. Collections are independent; returned V8/CSS ranges
/// preserve native structure rather than flattening to used disjoint ranges.
#[derive(Clone)]
pub struct Coverage {
    page: Page,
}
impl Coverage {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }
    fn require_chromium(&self) -> E2eResult<()> {
        if self.page.browser_kind() == BrowserKind::Chromium {
            Ok(())
        } else {
            Err(E2eError::Config(
                "coverage requires Chromium; Firefox BiDi exposes no coverage domain".into(),
            ))
        }
    }
    pub async fn start_js_coverage(&self) -> E2eResult<()> {
        self.start_js_coverage_with(JsCoverageOptions::default())
            .await
    }
    pub async fn start_js_coverage_with(&self, options: JsCoverageOptions) -> E2eResult<()> {
        self.require_chromium()?;
        let page = self.page.operation_page(&options.operation);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "start JavaScript coverage",
            async {
                let mut state = page.coverage_state.lock().await;
                if state.js.is_some() {
                    return Err(E2eError::Config(
                        "JavaScript coverage is already active".into(),
                    ));
                }
                if state.js_dirty {
                    cleanup(&page, Kind::Js).await?;
                }
                state.js_dirty = true;
                let capture = collector(
                    &self.page,
                    Kind::Js,
                    options.reset_on_navigation,
                    options.include_source,
                    options.report_anonymous_scripts,
                )?;
                page.call("Debugger.enable", serde_json::json!({})).await?;
                page.call("Profiler.enable", serde_json::json!({})).await?;
                page.call(
                    "Profiler.startPreciseCoverage",
                    serde_json::json!({"callCount":true,"detailed":true}),
                )
                .await?;
                state.js = Some(capture);
                Ok(())
            },
        ))
        .await
    }
    pub async fn start_css_coverage(&self) -> E2eResult<()> {
        self.start_css_coverage_with(CssCoverageOptions::default())
            .await
    }
    pub async fn start_css_coverage_with(&self, options: CssCoverageOptions) -> E2eResult<()> {
        self.require_chromium()?;
        let page = self.page.operation_page(&options.operation);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "start CSS coverage",
            async {
                let mut state = page.coverage_state.lock().await;
                if state.css.is_some() {
                    return Err(E2eError::Config("CSS coverage is already active".into()));
                }
                if state.css_dirty {
                    cleanup(&page, Kind::Css).await?;
                }
                state.css_dirty = true;
                let capture = collector(
                    &self.page,
                    Kind::Css,
                    options.reset_on_navigation,
                    options.include_source,
                    false,
                )?;
                page.call("DOM.enable", serde_json::json!({})).await?;
                page.call("CSS.enable", serde_json::json!({})).await?;
                page.call("CSS.startRuleUsageTracking", serde_json::json!({}))
                    .await?;
                state.css = Some(capture);
                Ok(())
            },
        ))
        .await
    }
    pub async fn stop_js_coverage(&self) -> E2eResult<Vec<JsCoverageEntry>> {
        self.stop_js_coverage_with_options(OperationOptions::default())
            .await
    }
    pub async fn stop_js_coverage_with_options(
        &self,
        options: OperationOptions,
    ) -> E2eResult<Vec<JsCoverageEntry>> {
        self.require_chromium()?;
        let page = self.page.operation_page(&options);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "stop JavaScript coverage",
            async {
                let mut state = page.coverage_state.lock().await;
                let capture = state
                    .js
                    .take()
                    .ok_or_else(|| E2eError::Config("JavaScript coverage is not active".into()))?;
                let result = page
                    .call("Profiler.takePreciseCoverage", serde_json::json!({}))
                    .await;
                let flushed = capture.flush().await;
                cleanup(&page, Kind::Js).await?;
                state.js_dirty = false;
                flushed?;
                let result = result?;
                let data = capture.data.lock().unwrap_or_else(|e| e.into_inner());
                data.check()?;
                let mut entries = Vec::new();
                let mut ranges = 0;
                for script in result["result"].as_array().into_iter().flatten() {
                    let id = script["scriptId"].as_str().unwrap_or_default();
                    let Some(record) = data.records.get(id) else {
                        continue;
                    };
                    for function in script["functions"].as_array().into_iter().flatten() {
                        ranges += 1 + function["ranges"].as_array().map_or(0, Vec::len);
                    }
                    if ranges > MAX_RANGES {
                        return Err(E2eError::Config(
                            "coverage native range capacity exceeded".into(),
                        ));
                    }
                    entries.push(JsCoverageEntry {
                        url: record.url.clone(),
                        script_id: id.into(),
                        source: record.source.clone(),
                        source_status: record.status.clone(),
                        functions: serde_json::from_value(script["functions"].clone())?,
                    });
                }
                entries.sort_by(|a, b| a.script_id.cmp(&b.script_id));
                Ok(entries)
            },
        ))
        .await
    }
    pub async fn stop_css_coverage(&self) -> E2eResult<Vec<CssCoverageEntry>> {
        self.stop_css_coverage_with_options(OperationOptions::default())
            .await
    }
    pub async fn stop_css_coverage_with_options(
        &self,
        options: OperationOptions,
    ) -> E2eResult<Vec<CssCoverageEntry>> {
        self.require_chromium()?;
        let page = self.page.operation_page(&options);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "stop CSS coverage",
            async {
                let mut state = page.coverage_state.lock().await;
                let capture = state
                    .css
                    .take()
                    .ok_or_else(|| E2eError::Config("CSS coverage is not active".into()))?;
                let result = page
                    .call("CSS.stopRuleUsageTracking", serde_json::json!({}))
                    .await;
                let flushed = capture.flush().await;
                cleanup(&page, Kind::Css).await?;
                state.css_dirty = false;
                flushed?;
                let result = result?;
                let data = capture.data.lock().unwrap_or_else(|e| e.into_inner());
                data.check()?;
                let native = result["ruleUsage"].as_array();
                if native.is_some_and(|ranges| ranges.len() > MAX_RANGES) {
                    return Err(E2eError::Config(
                        "coverage native range capacity exceeded".into(),
                    ));
                }
                let mut grouped: HashMap<String, Vec<CoverageRange>> = HashMap::new();
                for rule in native.into_iter().flatten() {
                    let id = rule["styleSheetId"].as_str().unwrap_or_default();
                    if data.records.contains_key(id) {
                        grouped.entry(id.into()).or_default().push(CoverageRange {
                            start_offset: rule["startOffset"].as_u64().unwrap_or(0),
                            end_offset: rule["endOffset"].as_u64().unwrap_or(0),
                            count: u64::from(rule["used"].as_bool().unwrap_or(false)),
                        });
                    }
                }
                let mut entries = Vec::new();
                for (id, record) in &data.records {
                    entries.push(CssCoverageEntry {
                        url: record.url.clone(),
                        style_sheet_id: id.clone(),
                        source: record.source.clone(),
                        source_status: record.status.clone(),
                        ranges: grouped.remove(id).unwrap_or_default(),
                    });
                }
                entries.sort_by(|a, b| a.style_sheet_id.cmp(&b.style_sheet_id));
                Ok(entries)
            },
        ))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(source: &str) -> SourceRecord {
        SourceRecord {
            url: "url".into(),
            source: source.into(),
            status: CoverageSourceStatus::Included,
        }
    }
    #[test]
    fn collection_replacement_navigation_and_failure_preserve_accounting() {
        let mut data = Collection::default();
        data.insert("id".into(), record("long"));
        assert_eq!(data.bytes, 2 + 3 + 4);
        data.insert("id".into(), record("x"));
        assert_eq!(data.bytes, 2 + 3 + 1);
        data.navigation(false);
        assert_eq!(data.records.len(), 1);
        data.navigation(true);
        assert_eq!(data.bytes, 0);
        data.insert("id".into(), record("x"));
        data.fail("lag");
        assert!(data.check().is_err());
        assert!(data.records.is_empty());
        data.navigation(true);
        assert!(data.check().is_err());
    }
    #[test]
    fn collection_caps_fail_loudly_and_release_unusable_sources() {
        let mut data = Collection::default();
        for index in 0..MAX_ENTRIES {
            data.insert(index.to_string(), record(""));
        }
        data.check().unwrap();
        data.insert("overflow".into(), record(""));
        assert!(data.check().unwrap_err().to_string().contains("capacity"));
        assert_eq!(data.bytes, 0);
        let mut data = Collection::default();
        for index in 0..16 {
            data.insert(index.to_string(), record(&"x".repeat(MAX_SOURCE)));
        }
        assert!(data.check().is_err());
        assert!(data.records.is_empty());
        let mut data = Collection::default();
        data.insert("x".repeat(1025), record(""));
        assert!(data.check().is_err());
    }
    #[test]
    fn defaults_and_legacy_source_status_are_compatible() {
        let js = JsCoverageOptions::default();
        let css = CssCoverageOptions::default();
        assert!(
            js.reset_on_navigation
                && css.reset_on_navigation
                && js.include_source
                && css.include_source
        );
        assert!(!js.report_anonymous_scripts);
        let legacy: JsCoverageEntry = serde_json::from_value(
            serde_json::json!({"url":"url","script_id":"id","source":"","functions":[]}),
        )
        .unwrap();
        assert_eq!(legacy.source_status, CoverageSourceStatus::Included);
        for status in [
            CoverageSourceStatus::Included,
            CoverageSourceStatus::Omitted,
            CoverageSourceStatus::Unavailable("native lost".into()),
            CoverageSourceStatus::Truncated { limit: MAX_SOURCE },
        ] {
            assert_eq!(
                serde_json::from_value::<CoverageSourceStatus>(
                    serde_json::to_value(&status).unwrap()
                )
                .unwrap(),
                status
            );
        }
        assert_eq!(bounded_reason(&"🦀".repeat(4096)).len(), 4096);
    }
    #[tokio::test]
    async fn capture_drop_aborts_and_releases_collector_data() {
        let data = Arc::new(Mutex::new(Collection::default()));
        let weak = Arc::downgrade(&data);
        let owned = data.clone();
        let (ready, started) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _owned = owned;
            let _ = ready.send(());
            std::future::pending::<()>().await;
        });
        let (flush, _) = tokio::sync::mpsc::channel(1);
        let capture = Capture {
            data,
            flush,
            task: task.abort_handle(),
        };
        started.await.unwrap();
        let mut state = CoverageState::default();
        state.js = Some(capture);
        state.cancel();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(weak.upgrade().is_none());
    }
}
