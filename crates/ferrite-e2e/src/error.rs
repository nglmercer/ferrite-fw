//! E2E error model.

/// Ferrite e2e result type.
pub type E2eResult<T> = std::result::Result<T, E2eError>;

/// Errors raised by the e2e framework.
#[derive(Debug, thiserror::Error)]
pub enum E2eError {
    /// No Chromium executable found.
    #[error("chromium not found: {0}")]
    BrowserNotFound(String),
    /// Chromium failed to launch or crashed.
    #[error("browser launch failed: {0}")]
    Launch(String),
    /// CDP protocol error.
    #[error("cdp error ({method}): {message}")]
    Cdp {
        /// CDP method that failed.
        method: String,
        /// Protocol error message.
        message: String,
    },
    /// CDP connection dropped.
    #[error("cdp connection closed: {0}")]
    Disconnected(String),
    /// Operation timed out.
    #[error("timed out after {0}ms: {1}")]
    Timeout(u64, String),
    /// An operation was canceled by its caller or owning lifecycle.
    #[error("operation canceled: {0}")]
    Cancelled(String),
    /// Runtime skip control flow; cleanup still runs.
    #[error("test skipped: {0}")]
    Skipped(String),
    /// Step-local skip control flow, consumed by Page::step_with.
    #[error("step skipped: {0}")]
    StepSkipped(String),
    /// Selector resolved to zero (or ambiguous) elements.
    #[error("locator error for `{selector}`: {message}")]
    Locator {
        /// Selector text.
        selector: String,
        /// Human-readable message.
        message: String,
    },
    /// Assertion failed after the retry window.
    #[error("expect failed: {0}")]
    Expect(String),
    /// Navigation failed.
    #[error("navigation to `{url}` failed: {message}")]
    Navigation {
        /// Target URL.
        url: String,
        /// Human-readable message.
        message: String,
    },
    /// Web server did not become ready.
    #[error("web server not ready: {0}")]
    WebServer(String),
    /// A native browser request failed at the transport layer.
    #[error("network request failed for `{url}`: {message}")]
    Network { url: String, message: String },
    /// Configuration error.
    #[error("config error: {0}")]
    Config(String),
    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// API client transport failure.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    /// JSON failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    /// Operation context for an opaque error whose original typed cause is retained.
    #[error("{context}: {source}")]
    Diagnostic {
        /// Calling operation and locator description.
        context: String,
        /// Original typed I/O, HTTP or JSON failure.
        #[source]
        source: Box<E2eError>,
    },
}

impl E2eError {
    /// Machine-readable error code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::BrowserNotFound(_) => "FERRITE_E2E_BROWSER_NOT_FOUND",
            Self::Launch(_) => "FERRITE_E2E_LAUNCH",
            Self::Cdp { .. } => "FERRITE_E2E_CDP",
            Self::Disconnected(_) => "FERRITE_E2E_DISCONNECTED",
            Self::Timeout(_, _) => "FERRITE_E2E_TIMEOUT",
            Self::Cancelled(_) => "FERRITE_E2E_CANCELLED",
            Self::StepSkipped(_) => "FERRITE_E2E_STEP_SKIPPED",
            Self::Skipped(_) => "FERRITE_E2E_SKIPPED",
            Self::Locator { .. } => "FERRITE_E2E_LOCATOR",
            Self::Expect(_) => "FERRITE_E2E_EXPECT",
            Self::Navigation { .. } => "FERRITE_E2E_NAVIGATION",
            Self::WebServer(_) => "FERRITE_E2E_WEB_SERVER",
            Self::Network { .. } => "FERRITE_E2E_NETWORK",
            Self::Config(_) => "FERRITE_E2E_CONFIG",
            Self::Io(_) => "FERRITE_E2E_IO",
            Self::Json(_) => "FERRITE_E2E_JSON",
            Self::Http(_) => "FERRITE_E2E_HTTP",
            Self::Diagnostic { source, .. } => source.code(),
        }
    }

    pub(crate) fn is_control_flow(&self) -> bool {
        matches!(
            self.code(),
            "FERRITE_E2E_CANCELLED"
                | "FERRITE_E2E_DISCONNECTED"
                | "FERRITE_E2E_SKIPPED"
                | "FERRITE_E2E_STEP_SKIPPED"
        )
    }

    pub(crate) fn with_context(self, context: &str) -> Self {
        let annotate = |message: String| format!("{context}: {message}");
        match self {
            Self::BrowserNotFound(message) => Self::BrowserNotFound(annotate(message)),
            Self::Launch(message) => Self::Launch(annotate(message)),
            Self::Cdp { method, message } => Self::Cdp {
                method,
                message: annotate(message),
            },
            Self::Disconnected(message) => Self::Disconnected(annotate(message)),
            Self::Timeout(ms, message) => Self::Timeout(ms, annotate(message)),
            Self::Cancelled(message) => Self::Cancelled(annotate(message)),
            Self::Locator { selector, message } => Self::Locator {
                selector,
                message: annotate(message),
            },
            Self::Expect(message) => Self::Expect(annotate(message)),
            Self::Navigation { url, message } => Self::Navigation {
                url,
                message: annotate(message),
            },
            Self::Network { url, message } => Self::Network {
                url,
                message: annotate(message),
            },
            Self::WebServer(message) => Self::WebServer(annotate(message)),
            Self::Config(message) => Self::Config(annotate(message)),
            // Skip reasons are runner control flow, not operation failures.
            error @ (Self::Skipped(_) | Self::StepSkipped(_)) => error,
            source => Self::Diagnostic {
                context: context.into(),
                source: Box::new(source),
            },
        }
    }
}

impl From<E2eError> for ferrite_core::FerriteError {
    fn from(error: E2eError) -> Self {
        Self::Other(format!("{}: {error}", error.code()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_context_preserves_control_flow_codes_and_native_identity() {
        let context = "locator.click #missing [description: Checkout]";
        let error = E2eError::Timeout(42, "not ready".into()).with_context(context);
        assert_eq!(error.code(), "FERRITE_E2E_TIMEOUT");
        assert!(matches!(error, E2eError::Timeout(42, _)));
        let error = E2eError::Cancelled("caller".into()).with_context(context);
        assert!(matches!(error, E2eError::Cancelled(_)));
        assert!(error.to_string().contains(context));
        let error = E2eError::Disconnected("reader".into()).with_context(context);
        assert!(matches!(error, E2eError::Disconnected(_)));
        let error = E2eError::Locator {
            selector: "#missing".into(),
            message: "strict".into(),
        }
        .with_context(context);
        assert!(matches!(error, E2eError::Locator { selector, .. } if selector == "#missing"));
        let error = E2eError::Cdp {
            method: "Runtime.callFunctionOn".into(),
            message: "native".into(),
        }
        .with_context(context);
        assert!(
            matches!(error, E2eError::Cdp { method, .. } if method == "Runtime.callFunctionOn")
        );
        assert!(
            matches!(E2eError::Skipped("reason".into()).with_context(context), E2eError::Skipped(reason) if reason == "reason")
        );
        assert!(
            matches!(E2eError::StepSkipped("local".into()).with_context(context), E2eError::StepSkipped(reason) if reason == "local")
        );
    }

    #[test]
    fn opaque_diagnostics_retain_typed_causes_and_codes() {
        let error = E2eError::Io(std::io::Error::from(std::io::ErrorKind::NotFound))
            .with_context("locator.screenshot labelled");
        assert_eq!(error.code(), "FERRITE_E2E_IO");
        assert!(std::error::Error::source(&error).is_some());
        assert!(
            matches!(error, E2eError::Diagnostic { source, .. } if matches!(*source, E2eError::Io(ref error) if error.kind() == std::io::ErrorKind::NotFound))
        );
        let json = serde_json::from_str::<u64>("\"text\"").unwrap_err();
        let error = E2eError::Json(json).with_context("locator.evaluate labelled");
        assert_eq!(error.code(), "FERRITE_E2E_JSON");
        assert!(
            matches!(error, E2eError::Diagnostic { source, .. } if matches!(*source, E2eError::Json(ref error) if error.is_data()))
        );
    }
}
