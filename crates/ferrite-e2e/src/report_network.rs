//! Owned, bounded request summaries. Native logs retain only a weak diagnostic sink.
use crate::RequestCompletion;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Attempt network diagnostics, including closed pages and popup startup sinks.
/// No headers, bodies, native handles or browser owners are retained.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSummary {
    pub requests: Vec<NetworkRequestSummary>,
    /// Requests removed by count/byte caps, or lost before sink registration.
    pub omitted_requests: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkRequestSummary {
    pub id: String,
    pub page_id: String,
    pub method: String,
    pub url: String,
    /// None means no response was observed; HTTP errors remain HTTP statuses.
    pub status: Option<u16>,
    /// Request-to-response time, not full transfer time.
    pub duration_ms: Option<u64>,
    pub started_ms: Option<u64>,
    pub resource_type: Option<String>,
    pub redirected_from: Option<String>,
    pub redirected_to: Option<String>,
    pub completion: RequestCompletion,
    /// At least one summary field exceeded its UTF-8 text budget.
    pub text_truncated: bool,
}
const MAX_REQUESTS: usize = 1000;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_TEXT: usize = 4096;
pub(crate) fn text(value: &str, truncated: &mut bool) -> String {
    if value.len() <= MAX_TEXT {
        return value.into();
    }
    *truncated = true;
    let mut end = MAX_TEXT - '…'.len_utf8();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}
impl NetworkRequestSummary {
    fn bytes(&self) -> usize {
        self.id.len()
            + self.page_id.len()
            + self.method.len()
            + self.url.len()
            + self.resource_type.as_ref().map_or(0, String::len)
            + self.redirected_from.as_ref().map_or(0, String::len)
            + self.redirected_to.as_ref().map_or(0, String::len)
            + match &self.completion {
                RequestCompletion::Failed(f) => f.error_text.len(),
                RequestCompletion::Unavailable(s) => s.len(),
                _ => 0,
            }
    }
}
#[derive(Default)]
pub(crate) struct NetworkSummaryLog {
    requests: VecDeque<NetworkRequestSummary>,
    bytes: usize,
    omitted: u64,
}
impl NetworkSummaryLog {
    pub(crate) fn omit(&mut self, count: u64) {
        self.omitted = self.omitted.saturating_add(count);
    }
    pub(crate) fn observe(&mut self, request: NetworkRequestSummary, new: bool) {
        if let Some(index) = self
            .requests
            .iter()
            .position(|old| old.id == request.id && old.page_id == request.page_id)
        {
            self.bytes = self.bytes.saturating_sub(self.requests[index].bytes());
            self.bytes += request.bytes();
            self.requests[index] = request;
        } else if new {
            self.bytes += request.bytes();
            self.requests.push_back(request);
        } else {
            return;
        } // Late updates never resurrect evicted requests.
        while self.requests.len() > MAX_REQUESTS || self.bytes > MAX_BYTES {
            let old = self.requests.pop_front().unwrap();
            self.bytes = self.bytes.saturating_sub(old.bytes());
            self.omit(1);
        }
    }
    pub(crate) fn snapshot(&self) -> NetworkSummary {
        NetworkSummary {
            requests: self.requests.iter().cloned().collect(),
            omitted_requests: self.omitted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: usize, page: &str) -> NetworkRequestSummary {
        NetworkRequestSummary {
            id: id.to_string(),
            page_id: page.into(),
            method: "GET".into(),
            url: "http://host/".into(),
            status: None,
            duration_ms: None,
            started_ms: None,
            resource_type: None,
            redirected_from: None,
            redirected_to: None,
            completion: RequestCompletion::Pending,
            text_truncated: false,
        }
    }
    #[test]
    fn caps_update_identity_without_resurrecting_evicted_entries() {
        let mut log = NetworkSummaryLog::default();
        for id in 0..=MAX_REQUESTS {
            log.observe(request(id, "one"), true);
        }
        assert_eq!(log.snapshot().omitted_requests, 1);
        log.observe(request(0, "one"), false);
        assert_eq!(log.snapshot().requests.len(), MAX_REQUESTS);
        let mut updated = request(1, "one");
        updated.status = Some(500);
        log.observe(updated, false);
        assert_eq!(log.snapshot().requests[0].status, Some(500));
        log.observe(request(1, "two"), true);
        assert_eq!(log.snapshot().omitted_requests, 2);
        assert_eq!(log.snapshot().requests.last().unwrap().page_id, "two");
    }
    #[test]
    fn byte_budget_and_unicode_are_explicit() {
        let mut log = NetworkSummaryLog::default();
        for id in 0..500 {
            let mut request = request(id, "one");
            let mut truncated = false;
            request.url = text(&"😀".repeat(2000), &mut truncated);
            assert!(truncated);
            assert!(request.url.len() <= MAX_TEXT);
            request.text_truncated = truncated;
            log.observe(request, true);
        }
        assert!(log.bytes <= MAX_BYTES);
        assert!(log.snapshot().omitted_requests > 0);
        assert!(log.snapshot().requests.len() < MAX_REQUESTS);
        assert!(log.snapshot().requests[0].url.ends_with('…'));
    }
}
