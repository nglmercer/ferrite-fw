//! Ferrite end-to-end testing: Playwright-style browser automation in pure
//! Rust, with no Node.js required (Chromium over CDP, Firefox over BiDi).
//!
//! ```rust,no_run
//! use ferrite_e2e::{Browser, E2eResult};
//!
//! #[tokio::main]
//! async fn main() -> E2eResult<()> {
//!     let browser = Browser::launch_default().await?;
//!     let page = browser.new_page().await?;
//!     page.goto("http://127.0.0.1:5173/").await?;
//!     page.locator("h1").expect_visible().await?;
//!     page.close().await?;
//!     browser.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! Tests usually run through the [`Runner`] (parallel workers, retries,
//! timeouts, reporters) while `ferrite e2e` boots the web server:
//!
//! ```rust,no_run
//! use ferrite_e2e::{test, Browser, E2eResult, Runner};
//!
//! #[tokio::main]
//! async fn main() -> E2eResult<()> {
//!     let browser = Browser::launch_default().await?;
//!     let report = Runner::default()
//!         .run(&browser, vec![test("home renders", |page| async move {
//!             page.goto("/").await?;
//!             page.expect_title("home").await?;
//!             Ok(())
//!         })])
//!         .await;
//!     browser.close().await?;
//!     std::process::exit(report.exit_code());
//! }
//! ```

mod action_options;
mod api;
mod api_cookies;
mod assertion_options;
mod bidi;
mod browser;
mod bundle;
mod callbacks;
mod cdp;
mod config;
mod context;
mod context_cookies;
mod cookie_filter;
mod coverage;
mod driver;
mod error;
mod event;
mod expect;
mod file_payload;
mod frame_locator;
mod function_wait;
mod har;
mod jshandle;
mod locator;
mod network;
mod operation;
mod page;
mod report;
mod routing;
mod runner;
mod snapshot;
mod url_matcher;
mod url_wait;
mod video;
mod webserver;

pub use action_options::{ActionOptions, ActionPosition, DragOptions, KeyboardModifier};
pub use api::{
    ApiClient, ApiClientOptions, ApiCredentialsSend, ApiRequestOptions, ApiResponse, MultipartField,
};
pub use assertion_options::{
    CheckedOptions, MatchOptions, StateAssertion, TextAssertionOptions, TextMatcher,
};
pub use bidi::BidiConnection;
pub use browser::{find_chromium, find_firefox, Browser, BrowserKind, LaunchOptions};
pub use bundle::ReportBundle;
pub use callbacks::BindingSource;
pub use cdp::CdpConnection;
pub use config::config_from_env;
pub use context::{
    BrowserContext, ContextEvent, ContextEventKind, ContextOptions, ServiceWorkerMode,
    TracingOptions,
};
pub use cookie_filter::CookieFilter;
pub use coverage::{Coverage, CoverageFunction, CoverageRange, CssCoverageEntry, JsCoverageEntry};
pub use driver::FrameStream;
pub use error::{E2eError, E2eResult};
pub use event::{DispatchEventOptions, DomEventKind};
pub use expect::{expect_poll, expect_to_pass, LocatorExpect, PageExpect, SoftAsserts, Timeout};
pub use file_payload::FilePayload;
pub use frame_locator::FrameLocator;
pub use function_wait::{FunctionPolling, FunctionWaitOptions};
pub use har::{HarContentMode, HarFile, HarReplayEntry};
pub use jshandle::JSHandle;
pub use locator::{
    set_test_id_attribute, test_id_attribute, FilterOptions, GetByRoleOptions, Locator,
    LocatorOptions, SelectOption, Selector, WaitForState,
};
pub use network::{
    HttpHeader, NetworkEvent, NetworkEvents, Request, RequestCompletion, RequestFailure,
    RequestSnapshot, Response,
};
pub use operation::{CancellationToken, OperationOptions};
pub use page::{
    AbortReason, ClickOptions, ColorScheme, ConsoleLocation, ConsoleMessage, Cookie,
    DeviceDescriptor, DialogDecision, DialogInfo, Download, ElementRect, ElementState, Frame,
    HttpCredentials, KeyPress, KeyPressOptions, LoadState, LocatorHandlerFn, LocatorHandlerOptions,
    MouseButton, MouseClickOptions, NavigationOptions, NetworkRequest, Page, PageEvent,
    PageEventKind, RecordedRequest, ReducedMotion, RouteAction, RouteFromHarOptions, RouteHandler,
    RouteHandlerEntry, RouteInfo, RouteRule, ScreenshotOptions, StorageEntry, StorageOrigin,
    StorageState, Viewport, WebSocketDirection, WebSocketEvent,
};
pub use report::{
    Attachment, AttemptInfo, AttemptResult, AttemptStatus, Reporter, SourceLocation,
    StepAnnotation, StepCategory, StepContext, StepInfo, StepOptions, StepOutcome, StepStatus,
    TestError, TestReport, TestResult, TestStatus,
};
pub use routing::{UnrouteBehavior, UnrouteOptions};
pub use runner::{
    describe, test, test_with_context, ContextHook, Fixture, FixtureMap, FixtureScope, GlobalHook,
    HookFn, Project, Runner, Suite, Test, TestContext, TestContextFn, TestInfo, TestMode,
    WorkerContext, WorkerHook, WorkerInfo,
};
pub use snapshot::{
    assert_snapshot_png, assert_snapshot_text, compare_png, match_text_snapshot,
    match_text_snapshot_with, SnapshotDiff, SnapshotOptions, SnapshotUpdate,
};
pub use url_matcher::UrlMatcher;
pub use url_wait::UrlWaitOptions;
pub use video::{find_ffmpeg, find_ffprobe, VideoFormat, VideoFrame, VideoMode, VideoOptions};
pub use webserver::{wait_for_url, RunningWebServer, WebServer};

pub use ferrite_config::{E2eConfig, ViewportConfig, WebServerConfig};
