//! Chromium JavaScript and CSS coverage. Firefox rejects these APIs explicitly.
use crate::{BrowserKind, E2eError, E2eResult, Page};
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
    pub functions: Vec<CoverageFunction>,
}

/// Source and CSS rule-use ranges. Offsets use CDP's UTF-16 convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CssCoverageEntry {
    pub url: String,
    pub style_sheet_id: String,
    pub source: String,
    pub ranges: Vec<CoverageRange>,
}

#[derive(Default)]
pub(crate) struct CoverageState {
    js: bool,
    css: bool,
    headers: Arc<Mutex<HashMap<String, String>>>,
    pump: Option<tokio::task::AbortHandle>,
}

impl CoverageState {
    pub(crate) fn cancel(&mut self) {
        if let Some(pump) = self.pump.take() {
            pump.abort();
        }
        self.js = false;
        self.css = false;
    }
}

/// Per-page coverage controller. It preserves V8 block ranges rather than
/// flattening them to Playwright's disjoint ranges; navigation options are deferred.
#[derive(Clone)]
pub struct Coverage {
    page: Page,
}
impl Coverage {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }
    fn require_chromium(&self) -> E2eResult<()> {
        if self.page.browser_kind() != BrowserKind::Chromium {
            return Err(E2eError::Config(
                "coverage requires Chromium; Firefox BiDi exposes no coverage domain".into(),
            ));
        }
        Ok(())
    }
    pub async fn start_js_coverage(&self) -> E2eResult<()> {
        self.require_chromium()?;
        let mut state = self.page.coverage_state.lock().await;
        if state.js {
            return Err(E2eError::Config(
                "JavaScript coverage is already active".into(),
            ));
        }
        self.page
            .call("Debugger.enable", serde_json::json!({}))
            .await?;
        self.page
            .call("Profiler.enable", serde_json::json!({}))
            .await?;
        self.page
            .call(
                "Profiler.startPreciseCoverage",
                serde_json::json!({"callCount":true,"detailed":true}),
            )
            .await?;
        state.js = true;
        Ok(())
    }
    pub async fn stop_js_coverage(&self) -> E2eResult<Vec<JsCoverageEntry>> {
        self.require_chromium()?;
        let mut state = self.page.coverage_state.lock().await;
        if !state.js {
            return Err(E2eError::Config("JavaScript coverage is not active".into()));
        }
        let result = self
            .page
            .call("Profiler.takePreciseCoverage", serde_json::json!({}))
            .await;
        self.page
            .call("Profiler.stopPreciseCoverage", serde_json::json!({}))
            .await?;
        state.js = false;
        let result = result?;
        let mut entries = Vec::new();
        for script in result["result"].as_array().into_iter().flatten() {
            let id = script["scriptId"].as_str().unwrap_or_default();
            let source = self
                .page
                .call(
                    "Debugger.getScriptSource",
                    serde_json::json!({"scriptId":id}),
                )
                .await?;
            entries.push(JsCoverageEntry {
                url: script["url"].as_str().unwrap_or_default().into(),
                script_id: id.into(),
                source: source["scriptSource"].as_str().unwrap_or_default().into(),
                functions: serde_json::from_value(script["functions"].clone())?,
            });
        }
        self.page
            .call("Profiler.disable", serde_json::json!({}))
            .await?;
        self.page
            .call("Debugger.disable", serde_json::json!({}))
            .await?;
        Ok(entries)
    }
    pub async fn start_css_coverage(&self) -> E2eResult<()> {
        self.require_chromium()?;
        let mut state = self.page.coverage_state.lock().await;
        if state.css {
            return Err(E2eError::Config("CSS coverage is already active".into()));
        }
        let (session, mut events) = self.page.cdp_events()?;
        state
            .headers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        let headers = state.headers.clone();
        state.pump = Some(
            tokio::spawn(async move {
                loop {
                    let event = match events.recv().await {
                        Ok(event) => event,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    };
                    if event.session.as_deref() == Some(&session)
                        && event.method == "CSS.styleSheetAdded"
                    {
                        let header = &event.params["header"];
                        if let Some(id) = header["styleSheetId"].as_str() {
                            headers.lock().unwrap_or_else(|e| e.into_inner()).insert(
                                id.into(),
                                header["sourceURL"].as_str().unwrap_or_default().into(),
                            );
                        }
                    }
                }
            })
            .abort_handle(),
        );
        let result = async {
            self.page.call("DOM.enable", serde_json::json!({})).await?;
            self.page.call("CSS.enable", serde_json::json!({})).await?;
            self.page
                .call("CSS.startRuleUsageTracking", serde_json::json!({}))
                .await?;
            E2eResult::Ok(())
        }
        .await;
        if let Err(error) = result {
            if let Some(pump) = state.pump.take() {
                pump.abort();
            }
            return Err(error);
        }
        state.css = true;
        Ok(())
    }
    pub async fn stop_css_coverage(&self) -> E2eResult<Vec<CssCoverageEntry>> {
        self.require_chromium()?;
        let mut state = self.page.coverage_state.lock().await;
        if !state.css {
            return Err(E2eError::Config("CSS coverage is not active".into()));
        }
        let result = self
            .page
            .call("CSS.stopRuleUsageTracking", serde_json::json!({}))
            .await?;
        state.css = false;
        if let Some(pump) = state.pump.take() {
            pump.abort();
        }
        let mut grouped: HashMap<String, Vec<CoverageRange>> = HashMap::new();
        for rule in result["ruleUsage"].as_array().into_iter().flatten() {
            grouped
                .entry(rule["styleSheetId"].as_str().unwrap_or_default().into())
                .or_default()
                .push(CoverageRange {
                    start_offset: rule["startOffset"].as_u64().unwrap_or(0),
                    end_offset: rule["endOffset"].as_u64().unwrap_or(0),
                    count: u64::from(rule["used"].as_bool().unwrap_or(false)),
                });
        }
        let mut entries = Vec::new();
        for (id, ranges) in grouped {
            let source = self
                .page
                .call(
                    "CSS.getStyleSheetText",
                    serde_json::json!({"styleSheetId":id}),
                )
                .await?;
            let url = state
                .headers
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&id)
                .cloned()
                .unwrap_or_default();
            entries.push(CssCoverageEntry {
                url,
                style_sheet_id: id,
                source: source["text"].as_str().unwrap_or_default().into(),
                ranges,
            });
        }
        self.page.call("CSS.disable", serde_json::json!({})).await?;
        entries.sort_by(|a, b| a.style_sheet_id.cmp(&b.style_sheet_id));
        Ok(entries)
    }
}
