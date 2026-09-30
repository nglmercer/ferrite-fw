//! Typed native observations. A request identity denotes one redirect hop.
use crate::{E2eError, E2eResult, Frame, OperationOptions, Page, RecordedRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, Weak},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFailure {
    pub error_text: String,
    pub cancelled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum RequestCompletion {
    Pending,
    Finished,
    Failed(RequestFailure),
    Unavailable(String),
}

/// Serializable metadata; absent native fields remain None. `recorded` keeps the
/// legacy shape, while typed observations distinguish pending response headers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSnapshot {
    pub id: String,
    pub native_id: String,
    pub page_id: Option<String>,
    pub frame_id: Option<String>,
    pub resource_type: Option<String>,
    pub is_navigation_request: Option<bool>,
    pub redirected_from: Option<String>,
    pub redirected_to: Option<String>,
    pub response_received: bool,
    /// Some(true) means Chromium's raw network-stack headers were observed.
    /// Some(false) means they are missing or exceeded the correlation budget;
    /// None means the backend provides no completeness signal.
    pub request_headers_complete: Option<bool>,
    /// Uses the same native completeness semantics as request_headers_complete.
    pub response_headers_complete: Option<bool>,
    pub post_data_truncated: bool,
    pub headers_truncated: bool,
    pub redirect_history_truncated: bool,
    pub completion: RequestCompletion,
    pub recorded: RecordedRequest,
}

#[derive(Default)]
pub(crate) struct RequestDetails {
    pub frame_id: Option<String>,
    pub resource_type: Option<String>,
    pub is_navigation: Option<bool>,
    pub redirect: bool,
    pub raw_headers: bool,
}
pub(crate) struct RequestState {
    pub id: String,
    pub native_id: String,
    pub method: String,
    pub url: String,
    page_id: Option<String>,
    details: RequestDetails,
    data: Mutex<ObservationData>,
    completion: tokio::sync::watch::Sender<RequestCompletion>,
    previous: Option<(String, Weak<RequestState>)>,
    next: Mutex<Option<(String, Weak<RequestState>)>>,
}
struct ObservationData {
    record: RecordedRequest,
    received: bool,
    request_headers_complete: Option<bool>,
    response_headers_complete: Option<bool>,
    post_data_truncated: bool,
    headers_truncated: bool,
}
const MAX_METADATA_TEXT: usize = 64 * 1024;
fn bound_headers(headers: &mut Vec<(String, String)>) -> bool {
    let mut bytes = 0;
    let before = headers.len();
    let keep = headers
        .iter()
        .take(256)
        .take_while(|(name, value)| {
            bytes += name.len() + value.len();
            bytes <= MAX_METADATA_TEXT
        })
        .count();
    headers.truncate(keep);
    before != keep
}
impl ObservationData {
    fn new(mut record: RecordedRequest) -> Self {
        let post_data_truncated = record
            .post_data
            .as_ref()
            .is_some_and(|value| value.len() > MAX_METADATA_TEXT);
        if post_data_truncated {
            record.post_data = None;
        }
        let headers_truncated =
            bound_headers(&mut record.headers) | bound_headers(&mut record.response_headers);
        Self {
            record,
            received: false,
            request_headers_complete: None,
            response_headers_complete: None,
            post_data_truncated,
            headers_truncated,
        }
    }
    fn size(&self) -> usize {
        self.record.url.len() * 2
            + self.record.method.len() * 2
            + self.record.post_data.as_ref().map_or(0, String::len)
            + self
                .record
                .headers
                .iter()
                .chain(&self.record.response_headers)
                .map(|(n, v)| n.len() + v.len())
                .sum::<usize>()
    }
}
impl RequestState {
    pub(crate) fn snapshot(&self) -> RequestSnapshot {
        let data = self.data.lock().unwrap_or_else(|e| e.into_inner());
        let next = self.next.lock().unwrap_or_else(|e| e.into_inner());
        RequestSnapshot {
            id: self.id.clone(),
            native_id: self.native_id.clone(),
            page_id: self.page_id.clone(),
            frame_id: self.details.frame_id.clone(),
            resource_type: self.details.resource_type.clone(),
            is_navigation_request: self.details.is_navigation,
            redirected_from: self.previous.as_ref().map(|(id, _)| id.clone()),
            redirected_to: next.as_ref().map(|(id, _)| id.clone()),
            redirect_history_truncated: (self.details.redirect && self.previous.is_none())
                || self
                    .previous
                    .as_ref()
                    .is_some_and(|(_, state)| state.upgrade().is_none())
                || next
                    .as_ref()
                    .is_some_and(|(_, state)| state.upgrade().is_none()),
            response_received: data.received,
            request_headers_complete: data.request_headers_complete,
            response_headers_complete: data.response_headers_complete,
            post_data_truncated: data.post_data_truncated,
            headers_truncated: data.headers_truncated,
            completion: self.completion.borrow().clone(),
            recorded: data.record.clone(),
        }
    }
}

#[derive(Default)]
struct ExtraHeaders {
    requests: VecDeque<Option<Vec<(String, String)>>>,
    responses: VecDeque<Option<Vec<(String, String)>>>,
    request_slots: VecDeque<Weak<RequestState>>,
    response_slots: VecDeque<Weak<RequestState>>,
}
impl ExtraHeaders {
    fn size(&self) -> usize {
        self.requests
            .iter()
            .chain(&self.responses)
            .filter_map(Option::as_ref)
            .flat_map(|headers| headers.iter())
            .map(|(n, v)| n.len() + v.len())
            .sum()
    }
    fn apply(&mut self) {
        for response in [false, true] {
            let (headers, slots) = if response {
                (&mut self.responses, &mut self.response_slots)
            } else {
                (&mut self.requests, &mut self.request_slots)
            };
            while !headers.is_empty() && !slots.is_empty() {
                let values = headers.pop_front().unwrap();
                if let Some(state) = slots.pop_front().unwrap().upgrade() {
                    let mut data = state.data.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(values) = values {
                        if response {
                            data.record.response_headers = values;
                            data.response_headers_complete = Some(true);
                        } else {
                            data.record.headers = values;
                            data.request_headers_complete = Some(true);
                        }
                    } else {
                        data.headers_truncated = true;
                    }
                }
            }
        }
    }
    fn empty(&self) -> bool {
        self.requests.is_empty()
            && self.responses.is_empty()
            && self.request_slots.is_empty()
            && self.response_slots.is_empty()
    }
}

/// Retained metadata has count/byte caps; caller-held handles live independently.
/// Weak redirect links avoid retaining evicted chains; snapshot IDs stay intact.
pub(crate) struct NetworkLog {
    sequence: u64,
    current: HashMap<String, Arc<RequestState>>,
    latest: HashMap<String, Weak<RequestState>>,
    recent: VecDeque<Arc<RequestState>>,
    extra: HashMap<String, ExtraHeaders>,
    raw_headers_enabled: bool,
    events: tokio::sync::broadcast::Sender<NetworkNotice>,
}
impl Default for NetworkLog {
    fn default() -> Self {
        Self {
            sequence: 0,
            current: HashMap::new(),
            latest: HashMap::new(),
            recent: VecDeque::new(),
            extra: HashMap::new(),
            raw_headers_enabled: true,
            events: tokio::sync::broadcast::channel(256).0,
        }
    }
}
#[derive(Clone)]
pub(crate) enum NetworkNotice {
    Request(Arc<RequestState>),
    Response(Arc<RequestState>),
    Finished(Arc<RequestState>),
    Failed(Arc<RequestState>),
}
const MAX_OBSERVATIONS: usize = 4096;
const MAX_METADATA_HISTORY: usize = 16 * 1024 * 1024;
impl NetworkLog {
    fn prune(&mut self) {
        let mut bytes: usize = self
            .recent
            .iter()
            .map(|state| state.data.lock().unwrap_or_else(|e| e.into_inner()).size())
            .sum();
        while self.recent.len() > MAX_OBSERVATIONS || bytes > MAX_METADATA_HISTORY {
            let old = self.recent.pop_front().unwrap();
            bytes = bytes.saturating_sub(old.data.lock().unwrap_or_else(|e| e.into_inner()).size());
            if self
                .current
                .get(&old.native_id)
                .is_some_and(|s| Arc::ptr_eq(s, &old))
            {
                self.current.remove(&old.native_id);
                old.completion.send_replace(RequestCompletion::Unavailable(
                    "native observation history capacity exceeded".into(),
                ));
            }
            if self
                .latest
                .get(&old.native_id)
                .and_then(Weak::upgrade)
                .is_some_and(|s| Arc::ptr_eq(&s, &old))
            {
                self.latest.remove(&old.native_id);
            }
        }
        let bytes: usize = self.extra.values().map(ExtraHeaders::size).sum();
        if self.extra.len() > MAX_OBSERVATIONS || bytes > MAX_METADATA_HISTORY {
            // Correlation after dropping an unmatched row would assign a later
            // hop's headers to an earlier one. Preserve primary metadata and
            // explicitly leave raw-header completeness false for this page.
            self.extra.clear();
            self.raw_headers_enabled = false;
        }
    }
    pub(crate) fn start(
        &mut self,
        record: RecordedRequest,
        details: RequestDetails,
        page_id: Option<String>,
    ) -> Arc<RequestState> {
        let native_id = record.request_id.clone().unwrap_or_default();
        let previous = details
            .redirect
            .then(|| self.latest.get(&native_id).and_then(Weak::upgrade))
            .flatten();
        self.sequence += 1;
        let state = Arc::new(RequestState {
            id: format!("{native_id}:{}", self.sequence),
            native_id: native_id.clone(),
            method: record.method.clone(),
            url: record.url.clone(),
            page_id,
            details,
            data: Mutex::new(ObservationData::new(record)),
            completion: tokio::sync::watch::channel(RequestCompletion::Pending).0,
            previous: previous.as_ref().map(|s| (s.id.clone(), Arc::downgrade(s))),
            next: Mutex::new(None),
        });
        if let Some(previous) = previous {
            *previous.next.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((state.id.clone(), Arc::downgrade(&state)));
        }
        if state.details.raw_headers {
            state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .request_headers_complete = Some(false);
        }

        self.current.insert(native_id.clone(), state.clone());
        self.latest.insert(native_id, Arc::downgrade(&state));
        self.recent.push_back(state.clone());
        self.prune();
        let _ = self.events.send(NetworkNotice::Request(state.clone()));
        state
    }
    pub(crate) fn response(
        &mut self,
        id: &str,
        record: RecordedRequest,
        extra: Option<bool>,
    ) -> Option<Arc<RequestState>> {
        let state = self.current.get(id)?.clone();
        let mut data = ObservationData::new(record);
        data.received = true;
        let mut old = state.data.lock().unwrap_or_else(|e| e.into_inner());
        let initial = !old.received;
        data.request_headers_complete = old.request_headers_complete;
        if old.request_headers_complete == Some(true) {
            data.record.headers = old.record.headers.clone();
        }
        if extra.is_some() {
            data.response_headers_complete = Some(false);
        }
        if old.response_headers_complete == Some(true) {
            data.record.response_headers = old.record.response_headers.clone();
            data.response_headers_complete = Some(true);
        }
        data.headers_truncated |= old.headers_truncated;
        *old = data;
        drop(old);
        if self.raw_headers_enabled {
            if extra == Some(true) && initial {
                if self.extra.get(id).is_some_and(|headers| {
                    headers.request_slots.len() >= 64 || headers.response_slots.len() >= 64
                }) {
                    self.extra.clear();
                    self.raw_headers_enabled = false;
                } else {
                    let headers = self.extra.entry(id.into()).or_default();
                    // CDP explicitly says which redirect hops have extra events.
                    // Wait for that flag before assigning either raw-header row:
                    // a cached hop can have none, while the next hop's row arrives
                    // before its request/response base event.
                    headers.request_slots.push_back(Arc::downgrade(&state));
                    headers.response_slots.push_back(Arc::downgrade(&state));
                    headers.apply();
                    if headers.empty() {
                        self.extra.remove(id);
                    }
                }
            } else if extra == Some(false) {
                if let Some(headers) = self.extra.get_mut(id) {
                    headers
                        .request_slots
                        .retain(|slot| !slot.upgrade().is_some_and(|s| Arc::ptr_eq(&s, &state)));
                    if headers.empty() {
                        self.extra.remove(id);
                    }
                }
            }
        }
        self.prune();
        if initial {
            let _ = self.events.send(NetworkNotice::Response(state.clone()));
        }
        Some(state)
    }
    pub(crate) fn extra_headers(
        &mut self,
        id: &str,
        response: bool,
        mut values: Vec<(String, String)>,
    ) {
        if !self.raw_headers_enabled {
            return;
        }
        // A dropped batch still consumes its FIFO row, preventing cross-hop data.
        let values = if bound_headers(&mut values) {
            None
        } else {
            Some(values)
        };
        let headers = self.extra.entry(id.into()).or_default();
        let queue = if response {
            &mut headers.responses
        } else {
            &mut headers.requests
        };
        if queue.len() >= 64 {
            self.extra.clear();
            self.raw_headers_enabled = false;
            return;
        }
        queue.push_back(values);
        headers.apply();
        if headers.empty() {
            self.extra.remove(id);
        }
        self.prune();
    }
    pub(crate) fn finish(&mut self, id: &str, failure: Option<RequestFailure>) {
        if let Some(state) = self.current.remove(id) {
            let failed = failure.is_some();
            state.completion.send_replace(match failure {
                Some(failure) => RequestCompletion::Failed(failure),
                None => RequestCompletion::Finished,
            });
            let _ = self.events.send(if failed {
                NetworkNotice::Failed(state)
            } else {
                NetworkNotice::Finished(state)
            });
        }
    }
    pub(crate) fn close(&mut self, reason: &str) {
        for (_, state) in self.current.drain() {
            state
                .completion
                .send_replace(RequestCompletion::Unavailable(reason.into()));
        }
        self.extra.clear();
    }
    pub(crate) fn states(&self) -> Vec<Arc<RequestState>> {
        self.recent.iter().cloned().collect()
    }
    pub(crate) fn subscribe(&self) -> tokio::sync::broadcast::Receiver<NetworkNotice> {
        self.events.subscribe()
    }
}

/// Typed events preserve native ordering; their Request/Response metadata is live.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    Request(Request),
    Response(Response),
    Finished(Request),
    Failed(Request),
}

/// Subscription owns its page scope; the broadcaster stores only request states.
/// Dropping a subscription reclaims its listener without a background pump.
pub struct NetworkEvents {
    pub(crate) page: Page,
    pub(crate) receiver: tokio::sync::broadcast::Receiver<NetworkNotice>,
}
impl NetworkEvents {
    pub async fn recv(&mut self) -> E2eResult<NetworkEvent> {
        self.recv_with_options(OperationOptions {
            timeout: Some(std::time::Duration::ZERO),
            cancellation: None,
        })
        .await
    }
    pub async fn recv_with_options(
        &mut self,
        options: OperationOptions,
    ) -> E2eResult<NetworkEvent> {
        let timeout = options.timeout.unwrap_or_else(|| self.page.timeout());
        let mut page = self.page.with_timeout(timeout);
        if let Some(token) = options.cancellation {
            page = page.with_cancellation(token);
        }
        let notice = page
            .run_operation(
                crate::operation::Deadline::new(timeout).run("network event", async {
                    self.receiver.recv().await.map_err(|error| match error {
                        tokio::sync::broadcast::error::RecvError::Lagged(count) => {
                            E2eError::Config(format!(
                                "network event subscription lost {count} events"
                            ))
                        }
                        tokio::sync::broadcast::error::RecvError::Closed => {
                            E2eError::Disconnected("network event stream closed".into())
                        }
                    })
                }),
            )
            .await?;
        Ok(match notice {
            NetworkNotice::Request(state) => {
                NetworkEvent::Request(Request::new(self.page.clone(), state))
            }
            NetworkNotice::Response(state) => NetworkEvent::Response(Response {
                request: Request::new(self.page.clone(), state),
            }),
            NetworkNotice::Finished(state) => {
                NetworkEvent::Finished(Request::new(self.page.clone(), state))
            }
            NetworkNotice::Failed(state) => {
                NetworkEvent::Failed(Request::new(self.page.clone(), state))
            }
        })
    }
}

/// A live observation of one native request/redirect hop. Clones share metadata.
/// Holding it retains its owning Page handle, but the Page history never owns
/// these wrappers, so ownership contains no strong-reference cycle.
#[derive(Clone)]
pub struct Request {
    pub(crate) page: Page,
    pub(crate) state: Arc<RequestState>,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("id", &self.id())
            .field("url", &self.url())
            .finish()
    }
}
impl Request {
    pub(crate) fn new(page: Page, state: Arc<RequestState>) -> Self {
        Self {
            page: page.owning_page(),
            state,
        }
    }
    pub fn id(&self) -> &str {
        &self.state.id
    }
    pub fn native_id(&self) -> &str {
        &self.state.native_id
    }
    pub fn method(&self) -> &str {
        &self.state.method
    }
    pub fn url(&self) -> &str {
        &self.state.url
    }
    pub fn page(&self) -> Page {
        self.page.clone()
    }
    pub fn page_id(&self) -> Option<&str> {
        self.state.page_id.as_deref()
    }
    pub fn frame_id(&self) -> Option<&str> {
        self.state.details.frame_id.as_deref()
    }
    pub fn resource_type(&self) -> Option<&str> {
        self.state.details.resource_type.as_deref()
    }
    pub fn is_navigation_request(&self) -> Option<bool> {
        self.state.details.is_navigation
    }
    pub fn snapshot(&self) -> RequestSnapshot {
        self.state.snapshot()
    }
    pub async fn frame(&self) -> E2eResult<Option<Frame>> {
        let Some(id) = self.frame_id() else {
            return Ok(None);
        };
        Ok(self
            .page
            .document_frames()
            .await?
            .into_iter()
            .find(|f| f.id() == id))
    }
    pub fn headers_array(&self) -> Vec<HttpHeader> {
        header_array(
            &self
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .headers,
        )
    }
    pub fn header_values(&self, name: &str) -> Vec<String> {
        header_values(
            &self
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .headers,
            name,
        )
    }
    pub fn header_value(&self, name: &str) -> Option<String> {
        header_value(
            &self
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .headers,
            name,
        )
    }
    pub fn post_data(&self) -> Option<String> {
        self.state
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record
            .post_data
            .clone()
    }
    pub fn post_data_json(&self) -> E2eResult<Option<Value>> {
        let Some(data) = self.post_data() else {
            return Ok(None);
        };
        if self.header_value("content-type").is_some_and(|h| {
            h.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .eq_ignore_ascii_case("application/x-www-form-urlencoded")
        }) {
            let mut url = reqwest::Url::parse("http://post.invalid/")
                .map_err(|e| E2eError::Config(format!("invalid form post data: {e}")))?;
            url.set_query(Some(&data));
            Ok(Some(Value::Object(
                url.query_pairs()
                    .map(|(k, v)| (k.into_owned(), Value::String(v.into_owned())))
                    .collect(),
            )))
        } else {
            Ok(Some(serde_json::from_str(&data)?))
        }
    }
    pub fn failure(&self) -> Option<RequestFailure> {
        match &*self.state.completion.borrow() {
            RequestCompletion::Failed(failure) => Some(failure.clone()),
            _ => None,
        }
    }
    pub fn completion(&self) -> RequestCompletion {
        self.state.completion.borrow().clone()
    }
    pub fn redirected_from(&self) -> Option<Self> {
        self.state
            .previous
            .as_ref()
            .and_then(|(_, s)| s.upgrade())
            .map(|s| Self::new(self.page.clone(), s))
    }
    pub fn redirected_to(&self) -> Option<Self> {
        self.state
            .next
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|(_, s)| s.upgrade())
            .map(|s| Self::new(self.page.clone(), s))
    }
    /// None until response headers are observed; a transport failure may have none.
    pub fn response(&self) -> Option<Response> {
        self.state
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .received
            .then(|| Response {
                request: self.clone(),
            })
    }
}

/// Response headers are independent of completion; no body capture is needed.
#[derive(Debug, Clone)]
pub struct Response {
    request: Request,
}
impl Response {
    pub(crate) fn with_page(mut self, page: Page) -> Self {
        self.request.page = page.owning_page();
        self
    }
    pub fn request(&self) -> Request {
        self.request.clone()
    }
    pub fn url(&self) -> &str {
        self.request.url()
    }
    pub fn page(&self) -> Page {
        self.request.page()
    }
    pub async fn frame(&self) -> E2eResult<Option<Frame>> {
        self.request.frame().await
    }
    pub fn status(&self) -> u16 {
        self.request
            .state
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record
            .status
    }
    pub fn status_text(&self) -> String {
        self.request
            .state
            .data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record
            .status_text
            .clone()
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status())
    }
    pub fn headers_array(&self) -> Vec<HttpHeader> {
        header_array(
            &self
                .request
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .response_headers,
        )
    }
    pub fn header_values(&self, name: &str) -> Vec<String> {
        header_values(
            &self
                .request
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .response_headers,
            name,
        )
    }
    pub fn header_value(&self, name: &str) -> Option<String> {
        header_value(
            &self
                .request
                .state
                .data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record
                .response_headers,
            name,
        )
    }
    /// Await native completion. HTTP 4xx/5xx complete successfully; transport
    /// failures fail. Already completed observations remain usable after close.
    pub async fn finished(&self) -> E2eResult<()> {
        self.finished_with_options(OperationOptions::default())
            .await
    }
    pub async fn finished_with_options(&self, options: OperationOptions) -> E2eResult<()> {
        if let Some(token) = &options.cancellation {
            token.check()?;
        }
        let mut completion = self.request.state.completion.subscribe();
        if let Some(result) = terminal(&completion.borrow(), self.url()) {
            return result;
        }
        let timeout = options
            .timeout
            .unwrap_or_else(|| self.request.page.timeout());
        let mut page = self.request.page.with_timeout(timeout);
        if let Some(token) = options.cancellation {
            page = page.with_cancellation(token);
        }
        page.run_operation(crate::operation::Deadline::new(timeout).run(
            "response.finished",
            async {
                loop {
                    if let Some(result) = terminal(&completion.borrow(), self.url()) {
                        return result;
                    }
                    completion.changed().await.map_err(|_| {
                        E2eError::Disconnected("request completion observation ended".into())
                    })?;
                }
            },
        ))
        .await
    }
}
fn terminal(completion: &RequestCompletion, url: &str) -> Option<E2eResult<()>> {
    match completion {
        RequestCompletion::Pending => None,
        RequestCompletion::Finished => Some(Ok(())),
        RequestCompletion::Failed(failure) => Some(Err(E2eError::Network {
            url: url.into(),
            message: failure.error_text.clone(),
        })),
        RequestCompletion::Unavailable(reason) => Some(Err(E2eError::Config(format!(
            "request observation unavailable: {reason}"
        )))),
    }
}
pub(crate) fn header_array(headers: &[(String, String)]) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpHeader {
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}
pub(crate) fn header_values(headers: &[(String, String)], name: &str) -> Vec<String> {
    headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.clone())
        .collect()
}
pub(crate) fn header_value(headers: &[(String, String)], name: &str) -> Option<String> {
    let values = header_values(headers, name);
    (!values.is_empty()).then(|| {
        values.join(if name.eq_ignore_ascii_case("set-cookie") {
            "\n"
        } else {
            ", "
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(id: &str, url: &str) -> RecordedRequest {
        RecordedRequest {
            method: "GET".into(),
            url: url.into(),
            status: 0,
            headers: vec![],
            post_data: None,
            duration_ms: None,
            status_text: String::new(),
            mime_type: String::new(),
            response_headers: vec![],
            started_ms: None,
            request_id: Some(id.into()),
            body: None,
            body_truncated: false,
        }
    }
    #[test]
    fn raw_headers_correlate_early_late_and_redirect_events_without_cross_hop_values() {
        let mut log = NetworkLog::default();
        let first = log.start(
            record("native", "http://host/first"),
            RequestDetails {
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.extra_headers("native", false, vec![("X-Request".into(), "first".into())]);
        let mut response = record("native", "http://host/first");
        response.status = 302;
        log.response("native", response, Some(true));
        log.finish("native", None);
        let second = log.start(
            record("native", "http://host/second"),
            RequestDetails {
                redirect: true,
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.extra_headers("native", false, vec![("X-Request".into(), "second".into())]);
        log.extra_headers(
            "native",
            true,
            vec![("Set-Cookie".into(), "hop=first".into())],
        );
        let mut response = record("native", "http://host/second");
        response.status = 200;
        log.response("native", response, Some(true));
        log.finish("native", None);
        log.extra_headers(
            "native",
            true,
            vec![
                ("Set-Cookie".into(), "hop=second".into()),
                ("Set-Cookie".into(), "duplicate=kept".into()),
            ],
        );
        let first = first.snapshot();
        let second = second.snapshot();
        assert_ne!(first.id, second.id);
        assert_eq!(first.redirected_to, Some(second.id.clone()));
        assert_eq!(second.redirected_from, Some(first.id.clone()));
        assert_eq!(first.recorded.headers[0].1, "first");
        assert_eq!(second.recorded.headers[0].1, "second");
        assert_eq!(first.recorded.response_headers[0].1, "hop=first");
        assert_eq!(second.recorded.response_headers.len(), 2);
        assert_eq!(second.response_headers_complete, Some(true));
        assert!(log.extra.is_empty());
        assert_eq!(second.completion, RequestCompletion::Finished);

        let cached = log.start(
            record("skipped", "http://host/cached"),
            RequestDetails {
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.extra_headers("skipped", false, vec![("X-Hop".into(), "uncached".into())]);
        log.response(
            "skipped",
            record("skipped", "http://host/cached"),
            Some(false),
        );
        log.finish("skipped", None);
        let uncached = log.start(
            record("skipped", "http://host/uncached"),
            RequestDetails {
                redirect: true,
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.response(
            "skipped",
            record("skipped", "http://host/uncached"),
            Some(true),
        );
        assert!(cached.snapshot().recorded.headers.is_empty());
        assert_eq!(cached.snapshot().request_headers_complete, Some(false));
        assert_eq!(uncached.snapshot().recorded.headers[0].1, "uncached");

        // Oversized early raw batches consume their hop rather than shifting
        // the next redirect's headers into the first response.
        log.extra_headers(
            "bounded",
            true,
            vec![("huge".into(), "x".repeat(MAX_METADATA_TEXT + 1))],
        );
        log.extra_headers("bounded", true, vec![("X-Hop".into(), "next".into())]);
        let first = log.start(
            record("bounded", "http://host/oversized"),
            RequestDetails {
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.response(
            "bounded",
            record("bounded", "http://host/oversized"),
            Some(true),
        );
        log.finish("bounded", None);
        let next = log.start(
            record("bounded", "http://host/next"),
            RequestDetails {
                raw_headers: true,
                redirect: true,
                ..Default::default()
            },
            None,
        );
        log.response("bounded", record("bounded", "http://host/next"), Some(true));
        assert!(first.snapshot().headers_truncated);
        assert_eq!(first.snapshot().response_headers_complete, Some(false));
        assert!(first.snapshot().recorded.response_headers.is_empty());
        assert_eq!(next.snapshot().recorded.response_headers[0].1, "next");

        // Once correlation overflows, primary metadata survives and future
        // raw rows are ignored rather than being attributed to the wrong hop.
        for _ in 0..65 {
            log.extra_headers("unmatched", true, vec![("X".into(), "value".into())]);
        }
        assert!(!log.raw_headers_enabled);
        assert!(log.extra.is_empty());
        let retained = log.start(
            record("retained", "http://host/retained"),
            RequestDetails {
                raw_headers: true,
                ..Default::default()
            },
            None,
        );
        log.extra_headers(
            "retained",
            false,
            vec![("X-Wrong".into(), "ignored".into())],
        );
        assert_eq!(retained.snapshot().request_headers_complete, Some(false));
        assert!(retained.snapshot().recorded.headers.is_empty());
    }
    #[test]
    fn caps_mark_unavailable_pending_observations_and_do_not_retain_evicted_chains() {
        let mut log = NetworkLog::default();
        let first = log.start(
            record("native", "http://host/first"),
            RequestDetails::default(),
            None,
        );
        let weak = Arc::downgrade(&first);
        let id = first.id.clone();
        log.finish("native", None);
        let second = log.start(
            record("native", "http://host/second"),
            RequestDetails {
                redirect: true,
                ..Default::default()
            },
            None,
        );
        drop(first);
        for index in 0..MAX_OBSERVATIONS {
            log.start(
                record(&index.to_string(), "http://host/cap"),
                RequestDetails::default(),
                None,
            );
        }
        assert!(weak.upgrade().is_none());
        let snapshot = second.snapshot();
        assert_eq!(snapshot.redirected_from, Some(id));
        assert!(snapshot.redirect_history_truncated);
        assert!(matches!(
            snapshot.completion,
            RequestCompletion::Unavailable(_)
        ));
        let missing_previous = log.start(
            record("native", "http://host/third"),
            RequestDetails {
                redirect: true,
                ..Default::default()
            },
            None,
        );
        assert!(missing_previous.snapshot().redirect_history_truncated);
        assert!(missing_previous.snapshot().redirected_from.is_none());
        assert_eq!(log.recent.len(), MAX_OBSERVATIONS);
        assert_eq!(log.current.len(), MAX_OBSERVATIONS);
        log.close("closed");
        assert!(log.current.is_empty());
        let mut large = record("large", "http://host/large");
        large.post_data = Some("a".repeat(MAX_METADATA_TEXT + 1));
        large
            .headers
            .push(("big".into(), "b".repeat(MAX_METADATA_TEXT + 1)));
        let large = log.start(large, RequestDetails::default(), None).snapshot();
        assert!(large.post_data_truncated);
        assert!(large.headers_truncated);
        assert!(large.recorded.post_data.is_none());
        assert!(large.recorded.headers.is_empty());
    }
}
