//! Video recording and frame capture.
//!
//! Chromium records via CDP screencast (JPEG frames, assembled with ffmpeg);
//! Firefox records natively to WebM via BiDi screencast. Both engines expose
//! the same [`Page`](crate::Page) API.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{E2eError, E2eResult};

/// When the runner keeps video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoMode {
    /// Never record.
    #[default]
    Off,
    /// Always record and keep.
    On,
    /// Record always, keep only for failures.
    OnlyOnFailure,
}

impl VideoMode {
    /// Parse `on` / `off` / `only-on-failure` (also `retain-on-failure`).
    pub fn parse(name: &str) -> E2eResult<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "off" | "none" => Ok(Self::Off),
            "on" | "always" => Ok(Self::On),
            "only-on-failure" | "retain-on-failure" | "on-failure" => Ok(Self::OnlyOnFailure),
            other => Err(E2eError::Config(format!(
                "unknown video mode {other:?}: expected \"on\", \"off\", or \"only-on-failure\""
            ))),
        }
    }

    /// True when tests record (kept or not).
    #[must_use]
    pub fn records(self) -> bool {
        self != Self::Off
    }
}

/// Options for video recording / frame capture.
#[derive(Debug, Clone)]
pub struct VideoOptions {
    /// Target frames per second (Chromium paces screencast; Firefox uses
    /// its default encoder rate and ignores this for recordings).
    pub fps: u32,
    /// JPEG quality 1-100 (Chromium screencast frames).
    pub quality: u8,
    /// Max frame width in pixels (aspect preserved; Chromium only).
    pub max_width: Option<u32>,
    /// Directory for the output video (and Chromium frame spool).
    pub dir: PathBuf,
    /// Video container (`webm`).
    pub format: VideoFormat,
}

/// Output container for recordings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoFormat {
    /// WebM/VP8 (native on Firefox, assembled via ffmpeg on Chromium).
    #[default]
    WebM,
}

impl VideoFormat {
    /// File extension without dot.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::WebM => "webm",
        }
    }
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            fps: 10,
            quality: 70,
            max_width: None,
            dir: PathBuf::from("test-results"),
            format: VideoFormat::WebM,
        }
    }
}

/// One captured frame (JPEG bytes).
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Zero-based frame index within the capture.
    pub index: u64,
    /// Milliseconds since capture start.
    pub timestamp_ms: u64,
    /// Frame bytes (JPEG).
    pub data: Vec<u8>,
}

/// Locate an ffmpeg executable (`FERRITE_FFMPEG_PATH` or `PATH`).
pub fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("FERRITE_FFMPEG_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("ffmpeg"))
            .find(|candidate| candidate.is_file())
    })
}

/// Locate ffprobe (test validation only).
pub fn find_ffprobe() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("ffprobe"))
            .find(|candidate| candidate.is_file())
    })
}

/// One spooled frame with its capture timestamp.
#[derive(Debug, Clone)]
pub struct SpooledFrame {
    /// File name inside the spool dir.
    pub file: String,
    /// Milliseconds since recording start.
    pub timestamp_ms: u64,
}

/// Assemble spooled JPEG frames into a WebM with ffmpeg, using per-frame
/// timestamps so sparse (damage-driven) screencasts keep correct duration.
pub async fn assemble_webm(
    spool: &Path,
    frames: &[SpooledFrame],
    total_ms: u64,
    fps: u32,
    output: &Path,
    timeout: Duration,
) -> E2eResult<()> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| {
        E2eError::Config(
            "ffmpeg not found but required to assemble Chromium recordings; \
             install ffmpeg or set FERRITE_FFMPEG_PATH"
                .to_string(),
        )
    })?;
    if frames.is_empty() {
        return Err(E2eError::Config(
            "screencast produced no frames; the page may have closed early".to_string(),
        ));
    }
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let list = spool.join("list.txt");
    std::fs::write(&list, concat_list(frames, total_ms, fps))?;
    let mut child = tokio::process::Command::new(&ffmpeg)
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(list.display().to_string())
        .arg("-c:v")
        .arg("libvpx")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-crf")
        .arg("10")
        .arg("-b:v")
        .arg("1M")
        .arg(output.display().to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| E2eError::Config(format!("spawn {}: {error}", ffmpeg.display())))?;
    let waited = tokio::time::timeout(timeout, child.wait())
        .await
        .map_err(|_| {
            E2eError::Timeout(
                timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                "ffmpeg assemble".to_string(),
            )
        })?;
    let status = waited.map_err(|error| E2eError::Config(format!("ffmpeg wait: {error}")))?;
    if !status.success() {
        let stderr = drain(&mut child).await;
        return Err(E2eError::Config(format!(
            "ffmpeg exited with {status} assembling {}: {stderr}",
            output.display()
        )));
    }
    match std::fs::metadata(output) {
        Ok(meta) if meta.len() > 0 => Ok(()),
        _ => Err(E2eError::Config(format!(
            "ffmpeg produced no output at {}",
            output.display()
        ))),
    }
}

/// Build an ffmpeg concat-demuxer list with per-frame durations.
/// Each frame holds until the next timestamp; the last holds until the
/// recording end (at least one frame interval).
fn concat_list(frames: &[SpooledFrame], total_ms: u64, fps: u32) -> String {
    let min_step = 1000.0 / f64::from(fps.max(1));
    let mut out = String::from("ffconcat version 1.0\n");
    for (i, frame) in frames.iter().enumerate() {
        let end = frames
            .get(i + 1)
            .map(|next| next.timestamp_ms)
            .unwrap_or(total_ms.max(frame.timestamp_ms));
        let duration = ((end.saturating_sub(frame.timestamp_ms)) as f64 / 1000.0)
            .max(min_step / 1000.0)
            .max(0.01);
        out.push_str(&format!("file '{}'\nduration {duration:.3}\n", frame.file));
    }
    // Concat quirk: repeat the last file so its duration applies.
    if let Some(last) = frames.last() {
        out.push_str(&format!("file '{}'\n", last.file));
    }
    out
}

async fn drain(child: &mut tokio::process::Child) -> String {
    use tokio::io::AsyncReadExt as _;
    let mut text = String::new();
    if let Some(stderr) = child.stderr.as_mut() {
        let mut buf = vec![0u8; 2048];
        if tokio::time::timeout(Duration::from_millis(300), stderr.read(&mut buf))
            .await
            .is_ok()
        {
            text.push_str(&String::from_utf8_lossy(&buf));
        }
    }
    text.lines().take(3).collect::<Vec<_>>().join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse() {
        assert_eq!(VideoMode::parse("off").unwrap(), VideoMode::Off);
        assert_eq!(VideoMode::parse("ON").unwrap(), VideoMode::On);
        assert_eq!(
            VideoMode::parse("only-on-failure").unwrap(),
            VideoMode::OnlyOnFailure
        );
        assert_eq!(
            VideoMode::parse("retain-on-failure").unwrap(),
            VideoMode::OnlyOnFailure
        );
        assert!(VideoMode::parse("sometimes").is_err());
        assert!(!VideoMode::Off.records());
        assert!(VideoMode::On.records());
    }

    #[test]
    fn concat_durations_span_timestamps() {
        let frames = vec![
            SpooledFrame {
                file: "frame-000000.jpg".to_string(),
                timestamp_ms: 0,
            },
            SpooledFrame {
                file: "frame-000001.jpg".to_string(),
                timestamp_ms: 500,
            },
        ];
        let list = concat_list(&frames, 1500, 10);
        assert!(list.contains("duration 0.500\n"), "{list}");
        assert!(list.contains("duration 1.000\n"), "{list}");
        assert!(list.starts_with("ffconcat version 1.0\n"), "{list}");
    }

    #[test]
    fn format_extension() {
        assert_eq!(VideoFormat::WebM.extension(), "webm");
        assert_eq!(VideoOptions::default().fps, 10);
    }
}
