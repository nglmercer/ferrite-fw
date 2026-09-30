//! Owned, serializable configuration snapshots; no callbacks or browser handles.
use crate::{BrowserKind, ContextOptions, LaunchOptions, SnapshotUpdate, VideoMode};
use serde::{Deserialize, Serialize};

/// Effective selected project, before any test/suite override.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedProjectConfig {
    pub name: Option<String>,
    pub browser: BrowserKind,
    /// Native version; None when a dedicated browser has not launched.
    pub browser_version: Option<String>,
    /// Requested launch options for a dedicated browser. Supplied owners have None.
    pub launch_options: Option<LaunchOptions>,
    pub grep: Option<String>,
    pub grep_invert: Option<String>,
    pub retries: u32,
    pub timeout_ms: u64,
    pub repeat_each: u32,
    pub output_dir: String,
    pub snapshot_dir: String,
    pub context: ContextOptions,
}

/// Effective run configuration. Paths and legacy environment fallback are fixed
/// at the run boundary; returned copies cannot change scheduling.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedRunConfig {
    pub workers: usize,
    pub retries: u32,
    pub timeout_ms: u64,
    pub expect_timeout_ms: u64,
    pub cleanup_timeout_ms: u64,
    pub global_timeout_ms: u64,
    pub max_failures: usize,
    pub reporter: String,
    pub list_progress: bool,
    pub forbid_only: bool,
    pub screenshot_always: bool,
    pub screenshot_on_failure: bool,
    pub trace: bool,
    pub video: VideoMode,
    pub video_fps: u32,
    pub output_dir: String,
    pub snapshot_dir: String,
    pub snapshot_update: SnapshotUpdate,
    pub repeat_each: u32,
    pub filter: Option<String>,
    pub grep: Option<String>,
    pub grep_invert: Option<String>,
    pub shard: Option<(usize, usize)>,
    pub selected_projects: Vec<String>,
    pub projects: Vec<ResolvedProjectConfig>,
    /// Actual supplied browser state, not inferred from E2eConfig launch inputs.
    pub browser: BrowserKind,
    pub browser_version: String,
    pub base_url: Option<String>,
    pub context: ContextOptions,
}
impl ResolvedRunConfig {
    /// Find a selected project (None denotes the implicit supplied-browser project).
    pub fn project(&self, name: Option<&str>) -> Option<&ResolvedProjectConfig> {
        self.projects
            .iter()
            .find(|project| project.name.as_deref() == name)
    }
}

/// Effective settings for an actual attempt, including suite/test overrides and
/// current runtime timeout. Other fields describe the initial resolved settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolvedTestSettings {
    pub project: Option<String>,
    pub browser: BrowserKind,
    pub browser_version: String,
    pub base_url: Option<String>,
    pub retries: u32,
    pub timeout_ms: u64,
    pub expect_timeout_ms: u64,
    pub cleanup_timeout_ms: u64,
    pub repeat_each: u32,
    pub repeat_each_index: u32,
    pub project_output_dir: String,
    pub output_dir: String,
    pub snapshot_dir: String,
    pub snapshot_update: SnapshotUpdate,
    pub context: ContextOptions,
}
