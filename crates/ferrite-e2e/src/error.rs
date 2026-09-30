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
        }
    }
}

impl From<E2eError> for ferrite_core::FerriteError {
    fn from(error: E2eError) -> Self {
        Self::Other(format!("{}: {error}", error.code()))
    }
}
