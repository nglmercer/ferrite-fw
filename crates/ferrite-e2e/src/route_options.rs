//! Request replay and response preparation for intercepted routes.
//!
use crate::{
    ApiClient, ApiRequestOptions, ApiResponse, BrowserContext, CancellationToken, E2eError,
    E2eResult, RouteAction, RouteInfo,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

/// Availability of the original intercepted request's payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteBodyState {
    /// Native metadata confirms that there is no body.
    Absent,
    /// Lossless bytes are available in `RouteInfo::post_data`.
    Captured,
    /// Native bytes are missing, incomplete or exceed the capture bound.
    Unavailable,
}

/// Overrides for a real HTTP fetch, without resolving the intercepted request.
#[derive(Debug, Clone, Default)]
pub struct RouteFetchOptions {
    pub url: Option<String>,
    pub method: Option<String>,
    /// Replacement header set; None preserves the intercepted headers.
    pub headers: Option<Vec<(String, String)>>,
    /// Explicit bytes, including an empty replacement. Mutually exclusive with json.
    pub body: Option<Vec<u8>>,
    pub json: Option<Value>,
    /// None uses the live owning page timeout; zero disables it.
    pub timeout: Option<Duration>,
    pub max_redirects: Option<usize>,
    pub max_retries: u32,
    pub cancellation: Option<CancellationToken>,
}

/// Prepare a synthetic response, optionally inheriting a fetched response.
#[derive(Debug, Clone, Default)]
pub struct RouteFulfillOptions {
    pub response: Option<ApiResponse>,
    pub status: Option<u16>,
    pub status_text: Option<String>,
    /// Replacement set, retaining duplicates; None inherits response headers.
    pub headers: Option<Vec<(String, String)>>,
    /// Body and JSON are mutually exclusive; a path overrides either payload.
    pub body: Option<Vec<u8>>,
    pub json: Option<Value>,
    /// Regular file (including symlinks to one); directories/devices/FIFOs are rejected.
    pub path: Option<PathBuf>,
    /// Takes precedence over headers and inferred JSON/file types.
    pub content_type: Option<String>,
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}

/// Weak registration lookup; retained RouteInfo never keeps a page/context alive.
#[derive(Debug, Clone)]
pub(crate) struct RouteFetchOwner {
    pub(crate) registry: Weak<Mutex<Vec<BrowserContext>>>,
    pub(crate) context_id: Option<String>,
    pub(crate) timeout: Arc<Mutex<Duration>>,
    pub(crate) page: CancellationToken,
    pub(crate) context: CancellationToken,
    pub(crate) transport: CancellationToken,
}
impl RouteFetchOwner {
    fn client(&self) -> E2eResult<ApiClient> {
        self.page.check()?;
        self.context.check()?;
        if let Some(reason) = self.transport.reason() {
            return Err(E2eError::Disconnected(reason));
        }
        let registry = self
            .registry
            .upgrade()
            .ok_or_else(|| E2eError::Cancelled("route context is no longer available".into()))?;
        let context = registry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|context| context.id() == self.context_id.as_deref())
            .cloned()
            .ok_or_else(|| E2eError::Cancelled("route context is no longer registered".into()))?;
        Ok(context.request())
    }
    async fn run<T>(
        &self,
        future: impl std::future::Future<Output = E2eResult<T>>,
    ) -> E2eResult<T> {
        self.page.check()?;
        self.context.check()?;
        if let Some(reason) = self.transport.reason() {
            return Err(E2eError::Disconnected(reason));
        }
        tokio::select! { biased;
            reason=self.page.cancelled()=>Err(E2eError::Cancelled(reason)),
            reason=self.context.cancelled()=>Err(E2eError::Cancelled(reason)),
            reason=self.transport.cancelled()=>Err(E2eError::Disconnected(reason)),
            result=future=>result,
        }
    }
}
impl RouteInfo {
    /// Construct a detached HTTP replay record with known, caller-supplied bytes.
    /// Native intercepted requests instead acquire their owning context automatically.
    pub fn new(
        url: impl Into<String>,
        method: impl Into<String>,
        headers: Vec<(String, String)>,
        post_data: Option<Vec<u8>>,
    ) -> Self {
        let body_state = if post_data.is_some() {
            RouteBodyState::Captured
        } else {
            RouteBodyState::Absent
        };
        Self {
            url: url.into(),
            method: method.into(),
            headers,
            post_data,
            body_state,
            owner: None,
        }
    }
    pub fn body_state(&self) -> RouteBodyState {
        self.body_state
    }
    pub(crate) fn native(mut self, body_state: RouteBodyState) -> Self {
        self.body_state = body_state;
        self
    }
    pub(crate) fn set_fetch_owner(&mut self, owner: Option<RouteFetchOwner>) {
        self.owner = owner;
    }
    /// Fetch with the owning context's cookies/TLS/proxy/auth and live page timeout.
    /// Detached records use a standalone client. Unavailable bodies require an override.
    pub async fn fetch(&self) -> E2eResult<ApiResponse> {
        self.fetch_with(RouteFetchOptions::default()).await
    }
    /// Fetch with explicit HTTP replay options.
    /// Fetching leaves the intercepted request paused; return a prepared action to
    /// resolve it. Native records inherit their context's HTTP defaults automatically.
    ///
    /// ```no_run
    /// use ferrite_e2e::{Page, E2eResult, RouteFetchOptions, RouteFulfillOptions};
    /// use serde_json::Value;
    /// async fn rewrite_account(page: &Page) -> E2eResult<()> {
    ///     page.route_with_handler("**/api/account", |route| async move {
    ///         let response = route.fetch_with(RouteFetchOptions {
    ///             max_redirects: Some(5),
    ///             max_retries: 1,
    ///             ..Default::default()
    ///         }).await?;
    ///         let mut account: Value = response.json()?;
    ///         account["plan"] = Value::String("fixture".into());
    ///         route.fulfill_with(RouteFulfillOptions {
    ///             response: Some(response),
    ///             json: Some(account),
    ///             ..Default::default()
    ///         }).await
    ///     }).await
    /// }
    /// ```
    pub async fn fetch_with(&self, options: RouteFetchOptions) -> E2eResult<ApiResponse> {
        let cancellation = options.cancellation.clone().unwrap_or_default();
        let timeout = options.timeout.unwrap_or_else(|| {
            self.owner
                .as_ref()
                .map_or(Duration::from_secs(30), |owner| {
                    *owner.timeout.lock().unwrap_or_else(|e| e.into_inner())
                })
        });
        let future = async {
            if options.body.is_some() && options.json.is_some() {
                return Err(E2eError::Config(
                    "route fetch body and JSON are mutually exclusive".into(),
                ));
            }
            if options.body.is_none()
                && options.json.is_none()
                && self.body_state == RouteBodyState::Unavailable
            {
                return Err(E2eError::Config("original route request body is unavailable; supply an explicit body or JSON override".into()));
            }
            let client = match &self.owner {
                Some(owner) => owner.client()?,
                None => ApiClient::new(),
            };
            let destination = client.url(options.url.as_deref().unwrap_or(&self.url))?;
            let destination = reqwest::Url::parse(&destination)
                .map_err(|e| E2eError::Config(format!("invalid route fetch URL: {e}")))?;
            if !matches!(destination.scheme(), "http" | "https") {
                return Err(E2eError::Config("route fetch URL must use HTTP(S)".into()));
            }
            let mut headers = options.headers.unwrap_or_else(|| self.headers.clone());
            // Recompute transport-controlled framing for the actual replay bytes/target.
            headers.retain(|(name, _)| {
                !name.eq_ignore_ascii_case("content-length")
                    && !name.eq_ignore_ascii_case("host")
                    && !name.starts_with(':')
            });
            let body = if options.json.is_some() {
                None
            } else {
                options.body.or_else(|| self.post_data.clone())
            };
            client
                .fetch_with(
                    options.method.as_deref().unwrap_or(&self.method),
                    destination.as_str(),
                    ApiRequestOptions {
                        headers,
                        body,
                        json: options.json,
                        timeout: Some(timeout),
                        max_redirects: options.max_redirects,
                        max_retries: options.max_retries,
                        ..Default::default()
                    },
                )
                .await
        };
        let future =
            cancellation.run(crate::operation::Deadline::new(timeout).run("route fetch", future));
        match &self.owner {
            Some(owner) => owner.run(future).await,
            None => future.await,
        }
    }
    fn apply_cors(&self, action: &mut RouteAction) -> E2eResult<()> {
        let Some((_, origin)) = self
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("origin"))
        else {
            return Ok(());
        };
        let request =
            reqwest::Url::parse(&self.url).map_err(|e| E2eError::Config(e.to_string()))?;
        if !matches!(request.scheme(), "http" | "https")
            || request.origin().ascii_serialization() == origin.trim()
        {
            return Ok(());
        }
        if let RouteAction::Fulfill { headers, .. } = action {
            if !headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("access-control-allow-origin"))
            {
                reqwest::header::HeaderValue::from_str(origin)
                    .map_err(|e| E2eError::Config(format!("invalid route origin: {e}")))?;
                headers.extend([
                    ("access-control-allow-origin".into(), origin.clone()),
                    ("access-control-allow-credentials".into(), "true".into()),
                    ("vary".into(), "Origin".into()),
                ]);
            }
        }
        Ok(())
    }
    /// Prepare fulfillment within this route's page/context lifecycle and budget.
    pub async fn fulfill_with(&self, mut options: RouteFulfillOptions) -> E2eResult<RouteAction> {
        if options.timeout.is_none() {
            options.timeout = Some(
                self.owner
                    .as_ref()
                    .map_or(Duration::from_secs(30), |owner| {
                        *owner.timeout.lock().unwrap_or_else(|e| e.into_inner())
                    }),
            );
        }
        let future = async {
            let mut action = RouteAction::fulfill_with(options).await?;
            self.apply_cors(&mut action)?;
            Ok(action)
        };
        match &self.owner {
            Some(owner) => owner.run(future).await,
            None => future.await,
        }
    }
}
impl RouteAction {
    /// Resolve response/body/header precedence before producing a synthetic action.
    pub async fn fulfill_with(options: RouteFulfillOptions) -> E2eResult<Self> {
        let cancellation = options.cancellation.clone().unwrap_or_default();
        cancellation
            .run(
                crate::operation::Deadline::new(options.timeout.unwrap_or(Duration::from_secs(30)))
                    .run("route fulfillment preparation", async {
                        if options.body.is_some() && options.json.is_some() {
                            return Err(E2eError::Config(
                                "route fulfillment body and JSON are mutually exclusive".into(),
                            ));
                        }
                        let explicit_payload = options.body.is_some()
                            || options.json.is_some()
                            || options.path.is_some();
                        let source = options.response.as_ref();
                        let status = options
                            .status
                            .unwrap_or_else(|| source.map_or(200, ApiResponse::status));
                        if !(200..=599).contains(&status) {
                            return Err(E2eError::Config(
                                "route fulfillment requires a final HTTP status in 200..=599"
                                    .into(),
                            ));
                        }
                        let inferred = if options.json.as_ref().is_some_and(json_truthy) {
                            Some("application/json".into())
                        } else {
                            options.path.as_ref().map(|path| {
                                mime_guess::from_path(path)
                                    .first_or_octet_stream()
                                    .to_string()
                            })
                        };
                        let body = if let Some(path) = options.path {
                            // Avoid opening FIFOs/devices whose reads can outlive cancellation
                            // on Tokio's blocking file pool. Symlinks to regular files work.
                            if !tokio::fs::metadata(&path).await?.is_file() {
                                return Err(E2eError::Config(
                                    "route fulfillment path must identify a regular file".into(),
                                ));
                            }
                            tokio::fs::read(path).await?
                        } else if let Some(json) = options.json {
                            serde_json::to_vec(&json)?
                        } else if let Some(body) = options.body {
                            body
                        } else {
                            source.map_or_else(Vec::new, |response| response.bytes().to_vec())
                        };
                        let mut headers = options.headers.unwrap_or_else(|| {
                            source.map_or_else(Vec::new, |response| response.headers().to_vec())
                        });
                        if let Some(content_type) = options.content_type.or(inferred) {
                            replace_header(&mut headers, "content-type", content_type);
                        }
                        if explicit_payload
                            && !body.is_empty()
                            && !headers
                                .iter()
                                .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        {
                            replace_header(&mut headers, "content-length", body.len().to_string());
                        }
                        for (name, value) in &headers {
                            reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(
                                |e| E2eError::Config(format!("invalid fulfillment header: {e}")),
                            )?;
                            reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                                E2eError::Config(format!("invalid fulfillment header value: {e}"))
                            })?;
                        }
                        let status_text = options.status_text.unwrap_or_else(|| {
                            if options.status.is_none() {
                                source.map_or_else(String::new, |response| {
                                    response.status_text().into()
                                })
                            } else {
                                String::new()
                            }
                        });
                        if status_text.contains(['\r', '\n', '\0']) {
                            return Err(E2eError::Config("invalid fulfillment status text".into()));
                        }
                        Ok(Self::fulfill_full(status, status_text, headers, body))
                    }),
            )
            .await
    }
}
fn replace_header(headers: &mut Vec<(String, String)>, name: &str, value: String) {
    headers.retain(|(key, _)| !key.eq_ignore_ascii_case(name));
    headers.push((name.into(), value));
}
fn json_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        _ => true,
    }
}

// Native CDP binary entries are lossless. Text-only fallback remains a preview:
// it may omit files or replace invalid UTF-8, so never advertise replayable bytes.
pub(crate) fn cdp_body(request: &Value) -> (Option<Vec<u8>>, RouteBodyState) {
    const MAX_BODY: usize = 16 * 1024 * 1024;
    if let Some(entries) = request["postDataEntries"].as_array() {
        use base64::Engine;
        if entries.is_empty() && request["hasPostData"].as_bool() == Some(true) {
            return (None, RouteBodyState::Unavailable);
        }
        let mut body = Vec::new();
        for entry in entries {
            let Some(bytes) = entry["bytes"].as_str() else {
                return (None, RouteBodyState::Unavailable);
            };
            if bytes.len() > MAX_BODY.saturating_sub(body.len()).saturating_add(2) / 3 * 4 + 4 {
                return (None, RouteBodyState::Unavailable);
            }
            let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(bytes) else {
                return (None, RouteBodyState::Unavailable);
            };
            if bytes.len() > MAX_BODY - body.len() {
                return (None, RouteBodyState::Unavailable);
            }
            body.extend(bytes);
        }
        return (Some(body), RouteBodyState::Captured);
    }
    let preview = request["postData"]
        .as_str()
        .map(|value| value.as_bytes()[..value.len().min(64 * 1024)].to_vec());
    if preview.is_some() || request["hasPostData"].as_bool() == Some(true) {
        (preview, RouteBodyState::Unavailable)
    } else {
        (None, RouteBodyState::Absent)
    }
}
pub(crate) fn bidi_body(request: &Value) -> RouteBodyState {
    if request["bodySize"].as_i64() == Some(0) {
        RouteBodyState::Absent
    } else if request["bodySize"].as_i64().is_some_and(|size| size > 0) {
        RouteBodyState::Unavailable
    } else if matches!(request["method"].as_str(), Some("GET" | "HEAD")) {
        RouteBodyState::Absent
    } else {
        RouteBodyState::Unavailable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_body_capture_distinguishes_lossless_missing_and_preview_bytes() {
        assert_eq!(
            cdp_body(
                &json!({"postDataEntries":[{"bytes":"AP8="},{"bytes":"gA0K"}],"hasPostData":true})
            ),
            (Some(vec![0, 255, 128, 13, 10]), RouteBodyState::Captured)
        );
        assert_eq!(cdp_body(&json!({})), (None, RouteBodyState::Absent));
        assert_eq!(
            cdp_body(&json!({"postDataEntries":[]})),
            (Some(vec![]), RouteBodyState::Captured)
        );
        for request in [
            json!({"postDataEntries":[],"hasPostData":true}),
            json!({"postDataEntries":[{}]}),
            json!({"postDataEntries":[{"bytes":"not base64!"}]}),
            json!({"hasPostData":true}),
        ] {
            assert_eq!(cdp_body(&request), (None, RouteBodyState::Unavailable));
        }
        let preview = "x".repeat(64 * 1024 + 1);
        let (bytes, state) = cdp_body(&json!({"postData":preview}));
        assert_eq!(state, RouteBodyState::Unavailable);
        assert_eq!(bytes.unwrap().len(), 64 * 1024);
        for request in [
            json!({"method":"POST","bodySize":5}),
            json!({"method":"POST"}),
        ] {
            assert_eq!(bidi_body(&request), RouteBodyState::Unavailable);
        }
        for request in [
            json!({"method":"POST","bodySize":0}),
            json!({"method":"GET"}),
            json!({"method":"HEAD"}),
        ] {
            assert_eq!(bidi_body(&request), RouteBodyState::Absent);
        }
    }

    #[test]
    fn native_body_capture_rejects_oversized_binary_entries() {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.encode(vec![0; 16 * 1024 * 1024 + 1]);
        assert_eq!(
            cdp_body(&json!({"postDataEntries":[{"bytes":bytes}]})),
            (None, RouteBodyState::Unavailable)
        );
    }
}
