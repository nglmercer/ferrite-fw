//! napi-vm options and info.

use std::path::PathBuf;

/// Pinned napi-vm revision backing this integration.
pub const NAPI_VM_PIN: &str = "0fa987d8860d620cd1008a84f2a17c9b67c495cd";

/// One allowlisted native addon.
#[derive(Debug, Clone)]
pub struct NativeAddonAllow {
    /// Addon path (absolute, or relative to the first configured root).
    pub path: PathBuf,
    /// Expected SHA-256 (hex) — `None` pins nothing (discouraged).
    pub sha256_hex: Option<String>,
}

/// napi-vm backend options.
#[derive(Debug, Clone, Default)]
pub struct NapiVmOptions {
    /// Maximum queued jobs (0 selects the bounded default of 64).
    pub queue_capacity: usize,
    /// Maximum encoded request payload bytes (0 selects 8 MiB).
    pub max_request_bytes: usize,
    /// Filesystem roots for the CJS loader and addon resolution.
    pub roots: Vec<PathBuf>,
    /// CJS entry filename for top-level `require()` resolution.
    pub entry: Option<String>,
    /// Native addon allowlist (empty = native loading disabled).
    pub native_allow: Vec<NativeAddonAllow>,
    /// Fuel budget (0 = napi-vm default).
    pub fuel_budget: u64,
    /// Loop budget (0 = napi-vm default).
    pub loop_budget: u64,
    /// Highest Node-API version the host reports/accepts (0 = napi-vm
    /// default 10; otherwise must be 1–10).
    pub max_napi_version: u32,
}

/// Options snapshot the worker can report (debugging aid).
#[derive(Debug, Clone, Default)]
pub struct NapiVmInfo {
    /// Backend name.
    pub backend: String,
    /// Pinned revision.
    pub pin: String,
    /// Native loading enabled.
    pub native_enabled: bool,
}
