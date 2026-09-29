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

mod bidi;
mod browser;
mod cdp;
mod context;
mod driver;
mod error;
mod expect;
mod locator;
mod page;
mod report;
mod runner;
mod video;
mod webserver;

pub use bidi::BidiConnection;
pub use browser::{find_chromium, find_firefox, Browser, BrowserKind, LaunchOptions};
pub use cdp::CdpConnection;
pub use context::{BrowserContext, ContextOptions};
pub use driver::FrameStream;
pub use error::{E2eError, E2eResult};
pub use expect::{PageExpect, Timeout};
pub use locator::{Locator, LocatorOptions, Selector};
pub use page::{
    ClickOptions, ColorScheme, ConsoleMessage, Cookie, DialogInfo, ElementState, KeyPress,
    LoadState, NavigationOptions, Page, RecordedRequest, ReducedMotion, RouteAction, RouteRule,
    ScreenshotOptions, StorageState, Viewport,
};
pub use report::{TestReport, TestResult, TestStatus};
pub use runner::{describe, test, Runner, Test};
pub use video::{find_ffmpeg, find_ffprobe, VideoFormat, VideoFrame, VideoMode, VideoOptions};
pub use webserver::{wait_for_url, RunningWebServer, WebServer};

pub use ferrite_config::{E2eConfig, ViewportConfig, WebServerConfig};
