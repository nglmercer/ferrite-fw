//! Build report.

use std::path::PathBuf;

/// A single-environment build report.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// Environment.
    pub env: String,
    /// Output directory.
    pub out_dir: PathBuf,
    /// Entry ids.
    pub entries: Vec<String>,
    /// Files written.
    pub chunks: usize,
    /// Code bytes.
    pub bytes: usize,
    /// Modules emitted.
    pub modules: usize,
    /// Modules dropped by tree-shaking.
    pub dropped: usize,
    /// Prebuilt standalone binary (`--target`), when produced.
    pub standalone_binary: Option<PathBuf>,
}
