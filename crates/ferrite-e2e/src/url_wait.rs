use crate::{CancellationToken, LoadState};
use std::time::Duration;

pub(crate) const DOCUMENT_OBSERVATION: &str = "({url:location.href, ready:document.readyState, document:performance.timeOrigin, domLoaded: document.readyState === 'complete' || performance.getEntriesByType('navigation').some(e => e.domContentLoadedEventStart > 0)})";
pub(crate) const DOM_CONTENT_LOADED: &str = "document.readyState === 'complete' || performance.getEntriesByType('navigation').some(e => e.domContentLoadedEventStart > 0)";

pub(crate) fn document_ready(observed: &serde_json::Value, state: LoadState) -> bool {
    match state {
        LoadState::Commit => true,
        LoadState::DomContentLoaded => observed["domLoaded"] == true,
        LoadState::Load | LoadState::NetworkIdle => observed["ready"] == "complete",
    }
}

/// URL matching and document readiness share one operation budget. The default
/// waits for Load and uses the Page navigation timeout. Zero disables the local
/// timeout; enclosing cancellation/deadlines remain active.
#[derive(Debug, Clone, Default)]
pub struct UrlWaitOptions {
    pub wait_until: LoadState,
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}
impl UrlWaitOptions {
    pub fn wait_until(mut self, state: LoadState) -> Self {
        self.wait_until = state;
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    pub fn cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
}

/// Retry only loss of an execution realm during navigation, preserving errors
/// from detached frames, invalid lazy selectors, disconnection and cancellation.
pub(crate) fn navigation_replaced_realm(error: &crate::E2eError) -> bool {
    let crate::E2eError::Cdp { message, .. } = error else {
        return false;
    };
    let message = message.to_ascii_lowercase();
    message.contains("execution context was destroyed")
        || message.contains("cannot find context with specified id")
        || message.contains("cannot find default execution context")
        || message.contains("no such realm")
}
