//! Snapshot assertions: screenshot and text snapshots (Playwright
//! `toHaveScreenshot` / `toMatchSnapshot`).
//!
//! Snapshots live under `<output_dir>/snapshots/` as `<slug>.png` / `<slug>.snap`.
//! The directory resolves as: explicit [`SnapshotOptions::dir`],
//! `FERRITE_SNAPSHOT_DIR` (set by [`Runner`](crate::runner::Runner) runs),
//! then `test-results/snapshots`. Update behavior resolves as: explicit
//! [`SnapshotOptions::update`], `FERRITE_UPDATE_SNAPSHOTS`
//! (`missing`/`all`/`none`, set by `ferrite e2e --update-snapshots`), then
//! [`SnapshotUpdate::Missing`].

use std::path::{Path, PathBuf};

use image::GenericImageView;

use crate::error::{E2eError, E2eResult};
use crate::runner::slug;

/// What to do about snapshot files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotUpdate {
    /// Write new snapshots, compare existing ones (default).
    #[default]
    Missing,
    /// Always overwrite with the actual value.
    All,
    /// Never write; missing snapshots fail.
    None,
}

impl SnapshotUpdate {
    /// Parse `missing` / `all` / `none` (case-insensitive).
    pub fn parse(name: &str) -> E2eResult<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "missing" => Ok(Self::Missing),
            "all" => Ok(Self::All),
            "none" => Ok(Self::None),
            other => Err(E2eError::Config(format!(
                "unknown snapshot update mode {other:?}: expected \"missing\", \"all\" or \"none\""
            ))),
        }
    }

    /// From `FERRITE_UPDATE_SNAPSHOTS` (unset/empty = [`SnapshotUpdate::Missing`];
    /// garbage warns and falls back to `Missing`).
    pub fn from_env() -> Self {
        match std::env::var("FERRITE_UPDATE_SNAPSHOTS") {
            Ok(raw) if !raw.trim().is_empty() => match Self::parse(&raw) {
                Ok(mode) => mode,
                Err(_) => {
                    eprintln!(
                        "warning: ignoring invalid FERRITE_UPDATE_SNAPSHOTS={raw:?} \
                         (want \"missing\", \"all\" or \"none\")"
                    );
                    Self::Missing
                }
            },
            _ => Self::Missing,
        }
    }
}

/// Options for snapshot assertions.
#[derive(Debug, Clone, Default)]
pub struct SnapshotOptions {
    /// Maximum differing pixels that still pass.
    pub max_diff_pixels: u32,
    /// Maximum differing-pixel ratio (0.0–1.0) that still passes.
    pub max_diff_ratio: f32,
    /// Per-channel tolerance (0 = exact).
    pub threshold: u8,
    /// Snapshot directory (defaults per module docs).
    pub dir: Option<PathBuf>,
    /// Update behavior (defaults per module docs).
    pub update: Option<SnapshotUpdate>,
}

/// Pixel comparison result.
#[derive(Debug, Clone)]
pub struct SnapshotDiff {
    /// Compared width (both images agree).
    pub width: u32,
    /// Compared height (both images agree).
    pub height: u32,
    /// Pixels differing beyond the threshold.
    pub diff_pixels: u64,
    /// Total compared pixels.
    pub total_pixels: u64,
    /// Bounding region of differing pixels as (x, y, w, h); `None` when clean.
    pub bounds: Option<(u32, u32, u32, u32)>,
}

impl SnapshotDiff {
    /// Fraction of differing pixels (0.0 when empty).
    #[must_use]
    pub fn diff_ratio(&self) -> f32 {
        if self.total_pixels == 0 {
            0.0
        } else {
            self.diff_pixels as f32 / self.total_pixels as f32
        }
    }

    /// Whether the diff satisfies both `max_diff_pixels` and `max_diff_ratio`.
    #[must_use]
    pub fn passed(&self, opts: &SnapshotOptions) -> bool {
        self.diff_pixels <= u64::from(opts.max_diff_pixels)
            && self.diff_ratio() <= opts.max_diff_ratio
    }

    /// One-line human summary (`12/2073600 pixels differ (0.0006%), …`).
    #[must_use]
    pub fn summary(&self) -> String {
        let mut out = format!(
            "{}/{} pixels differ ({:.4}%)",
            self.diff_pixels,
            self.total_pixels,
            self.diff_ratio() * 100.0
        );
        if let Some((x, y, w, h)) = self.bounds {
            out.push_str(&format!(" region x={x} y={y} w={w} h={h}"));
        }
        out
    }
}

/// Compare PNG bytes pixel by pixel.
///
/// A pixel differs when any RGBA channel differs by more than `threshold`.
/// Dimension mismatches fail loudly (exact sizes required).
pub fn compare_png(actual: &[u8], expected: &[u8], threshold: u8) -> E2eResult<SnapshotDiff> {
    let actual_img = image::load_from_memory_with_format(actual, image::ImageFormat::Png)
        .map_err(|error| E2eError::Config(format!("cannot decode actual PNG: {error}")))?;
    let expected_img = image::load_from_memory_with_format(expected, image::ImageFormat::Png)
        .map_err(|error| E2eError::Config(format!("cannot decode snapshot PNG: {error}")))?;
    if actual_img.dimensions() != expected_img.dimensions() {
        let (aw, ah) = actual_img.dimensions();
        let (ew, eh) = expected_img.dimensions();
        return Err(E2eError::Expect(format!(
            "snapshot size differs: actual {aw}x{ah}, expected {ew}x{eh}"
        )));
    }
    let (width, height) = actual_img.dimensions();
    let actual_rgba = actual_img.to_rgba8();
    let expected_rgba = expected_img.to_rgba8();
    let mut diff_pixels = 0u64;
    let (mut min_x, mut min_y) = (width, height);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    for (x, y, pixel) in actual_rgba.enumerate_pixels() {
        let other = expected_rgba.get_pixel(x, y);
        let differs = pixel
            .0
            .iter()
            .zip(other.0.iter())
            .any(|(a, b)| a.abs_diff(*b) > threshold);
        if differs {
            diff_pixels += 1;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    let total_pixels = u64::from(width) * u64::from(height);
    let bounds = if diff_pixels == 0 {
        None
    } else {
        Some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
    };
    Ok(SnapshotDiff {
        width,
        height,
        diff_pixels,
        total_pixels,
        bounds,
    })
}

/// Resolve the snapshot directory (explicit > env > default).
pub(crate) fn resolve_dir(explicit: Option<&Path>) -> PathBuf {
    if let Some(dir) = explicit {
        return dir.to_path_buf();
    }
    if let Ok(dir) = std::env::var("FERRITE_SNAPSHOT_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    PathBuf::from("test-results/snapshots")
}

/// Resolve the update mode (explicit > env > `Missing`).
pub(crate) fn resolve_update(explicit: Option<SnapshotUpdate>) -> SnapshotUpdate {
    explicit.unwrap_or_else(SnapshotUpdate::from_env)
}

/// Snapshot file path for `name` (`<slug>.<ext>`).
fn snap_path(dir: &Path, name: &str, ext: &str) -> PathBuf {
    dir.join(format!("{}.{ext}", slug(name)))
}

/// Snapshot file path for `name` with the resolved directory.
pub(crate) fn snap_path_for(name: &str, ext: &str, opts: &SnapshotOptions) -> PathBuf {
    snap_path(&resolve_dir(opts.dir.as_deref()), name, ext)
}

/// Assert PNG bytes against the named snapshot (`<slug>.png`).
///
/// Missing snapshots are written under `Missing`/`All` (pass) and fail under
/// `None`; `All` overwrites unconditionally. Mismatches write
/// `<slug>.actual.png` next to the expected file and fail with the diff summary.
pub fn assert_snapshot_png(name: &str, actual: &[u8], opts: &SnapshotOptions) -> E2eResult<()> {
    let dir = resolve_dir(opts.dir.as_deref());
    let path = snap_path(&dir, name, "png");
    let update = resolve_update(opts.update);
    if !path.is_file() {
        return match update {
            SnapshotUpdate::None => Err(E2eError::Expect(format!(
                "no snapshot {name:?} (update=none)"
            ))),
            _ => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, actual)?;
                Ok(())
            }
        };
    }
    if update == SnapshotUpdate::All {
        std::fs::write(&path, actual)?;
        return Ok(());
    }
    let expected = std::fs::read(&path)?;
    match compare_png(actual, &expected, opts.threshold) {
        Ok(diff) if diff.passed(opts) => Ok(()),
        Ok(diff) => Err(mismatch(&dir, name, "png", actual, diff.summary())),
        Err(error) => Err(mismatch(&dir, name, "png", actual, error.to_string())),
    }
}

/// Assert text against the named snapshot (`<slug>.snap`, same update rules).
pub fn assert_snapshot_text(name: &str, actual: &str, opts: &SnapshotOptions) -> E2eResult<()> {
    let dir = resolve_dir(opts.dir.as_deref());
    let path = snap_path(&dir, name, "snap");
    let update = resolve_update(opts.update);
    if !path.is_file() {
        return match update {
            SnapshotUpdate::None => Err(E2eError::Expect(format!(
                "no snapshot {name:?} (update=none)"
            ))),
            _ => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, actual)?;
                Ok(())
            }
        };
    }
    if update == SnapshotUpdate::All {
        std::fs::write(&path, actual)?;
        return Ok(());
    }
    let expected = std::fs::read_to_string(&path)?;
    if expected == actual {
        Ok(())
    } else {
        Err(mismatch(
            &dir,
            name,
            "snap",
            actual.as_bytes(),
            text_diff_preview(&expected, actual),
        ))
    }
}

/// Expect-style text assertion: `match_text_snapshot("name", &text)`.
pub fn match_text_snapshot(name: &str, actual: &str) -> E2eResult<()> {
    assert_snapshot_text(name, actual, &SnapshotOptions::default())
}

/// [`match_text_snapshot`] with explicit options.
pub fn match_text_snapshot_with(name: &str, actual: &str, opts: &SnapshotOptions) -> E2eResult<()> {
    assert_snapshot_text(name, actual, opts)
}

/// Write `<slug>.actual.<ext>` (best-effort) and build the mismatch error.
fn mismatch(dir: &Path, name: &str, ext: &str, actual: &[u8], detail: String) -> E2eError {
    let actual_path = dir.join(format!("{}.actual.{ext}", slug(name)));
    std::fs::write(&actual_path, actual).ok();
    E2eError::Expect(format!(
        "snapshot {name:?} differs: {detail} (actual: {})",
        actual_path.display()
    ))
}

/// Bounded preview around the first differing line (`±3` context, capped width).
fn text_diff_preview(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let first = (0..expected_lines.len().max(actual_lines.len()))
        .find(|i| expected_lines.get(*i) != actual_lines.get(*i))
        .unwrap_or(0);
    let start = first.saturating_sub(3);
    let end = (first + 4).min(expected_lines.len().max(actual_lines.len()));
    let mut out = vec![format!("first difference at line {}", first + 1)];
    for i in start..end {
        let marker = if expected_lines.get(i) != actual_lines.get(i) {
            ">"
        } else {
            " "
        };
        out.push(format!(
            "{marker} {:>4} - {}",
            i + 1,
            truncate(expected_lines.get(i).copied().unwrap_or("<eof>"))
        ));
        out.push(format!(
            "{marker} {:>4} + {}",
            i + 1,
            truncate(actual_lines.get(i).copied().unwrap_or("<eof>"))
        ));
    }
    out.join("\n")
}

/// Cap one preview line at 160 chars (char-boundary safe).
fn truncate(line: &str) -> String {
    if line.chars().count() > 160 {
        format!("{}…", line.chars().take(159).collect::<String>())
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    use std::io::Cursor;

    /// Encode an RGBA test image (`f` picks each pixel).
    fn png_bytes(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(f(x, y)));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ferrite-snap-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn identical_images_pass() {
        let bytes = png_bytes(8, 8, |x, y| [(x * 32) as u8, (y * 32) as u8, 0, 255]);
        let diff = compare_png(&bytes, &bytes, 0).unwrap();
        assert_eq!(diff.diff_pixels, 0);
        assert_eq!(diff.total_pixels, 64);
        assert!(diff.bounds.is_none());
        assert!(diff.passed(&SnapshotOptions::default()));
        assert!(diff.summary().contains("0/64"));
    }

    #[test]
    fn single_pixel_diff_reports_bounds() {
        let base = png_bytes(8, 8, |_, _| [10, 20, 30, 255]);
        let one = png_bytes(8, 8, |x, y| {
            if (x, y) == (5, 2) {
                [200, 20, 30, 255]
            } else {
                [10, 20, 30, 255]
            }
        });
        let diff = compare_png(&one, &base, 0).unwrap();
        assert_eq!(diff.diff_pixels, 1);
        assert_eq!(diff.bounds, Some((5, 2, 1, 1)));
        assert!(!diff.passed(&SnapshotOptions::default()));
        assert!(diff.passed(&SnapshotOptions {
            max_diff_pixels: 1,
            max_diff_ratio: 1.0,
            ..Default::default()
        }));
        assert!(diff.passed(&SnapshotOptions {
            max_diff_pixels: 10,
            max_diff_ratio: 0.02,
            ..Default::default()
        }));
        // Both limits must hold.
        assert!(!diff.passed(&SnapshotOptions {
            max_diff_pixels: 10,
            max_diff_ratio: 0.0001,
            ..Default::default()
        }));
    }

    #[test]
    fn threshold_forgives_small_drift() {
        let base = png_bytes(4, 4, |_, _| [100, 100, 100, 255]);
        let drifted = png_bytes(4, 4, |_, _| [103, 100, 100, 255]);
        assert_eq!(compare_png(&drifted, &base, 0).unwrap().diff_pixels, 16);
        assert_eq!(compare_png(&drifted, &base, 3).unwrap().diff_pixels, 0);
    }

    #[test]
    fn size_mismatch_is_loud() {
        let small = png_bytes(4, 4, |_, _| [0, 0, 0, 255]);
        let big = png_bytes(8, 8, |_, _| [0, 0, 0, 255]);
        let error = compare_png(&small, &big, 0).unwrap_err();
        assert!(error.to_string().contains("4x4"), "{error}");
        assert!(error.to_string().contains("8x8"), "{error}");
    }

    #[test]
    fn corrupt_png_is_loud() {
        let good = png_bytes(4, 4, |_, _| [0, 0, 0, 255]);
        let error = compare_png(b"not a png", &good, 0).unwrap_err();
        assert!(error.to_string().contains("decode"), "{error}");
    }

    #[test]
    fn update_modes_parse() {
        assert_eq!(
            SnapshotUpdate::parse("missing").unwrap(),
            SnapshotUpdate::Missing
        );
        assert_eq!(SnapshotUpdate::parse("ALL").unwrap(), SnapshotUpdate::All);
        assert_eq!(
            SnapshotUpdate::parse(" none ").unwrap(),
            SnapshotUpdate::None
        );
        assert!(SnapshotUpdate::parse("sometimes").is_err());
    }

    #[test]
    fn env_resolution() {
        let dir_key = "FERRITE_SNAPSHOT_DIR";
        let mode_key = "FERRITE_UPDATE_SNAPSHOTS";
        let saved_dir = std::env::var(dir_key).ok();
        let saved_mode = std::env::var(mode_key).ok();

        std::env::remove_var(dir_key);
        std::env::remove_var(mode_key);
        assert_eq!(resolve_dir(None), PathBuf::from("test-results/snapshots"));
        assert_eq!(resolve_update(None), SnapshotUpdate::Missing);

        std::env::set_var(dir_key, "/tmp/snaps");
        std::env::set_var(mode_key, "all");
        assert_eq!(resolve_dir(None), PathBuf::from("/tmp/snaps"));
        assert_eq!(resolve_update(None), SnapshotUpdate::All);

        std::env::set_var(mode_key, "garbage!!");
        assert_eq!(resolve_update(None), SnapshotUpdate::Missing);

        // Explicit options win over the environment.
        assert_eq!(
            resolve_dir(Some(Path::new("/tmp/explicit"))),
            PathBuf::from("/tmp/explicit")
        );
        assert_eq!(
            resolve_update(Some(SnapshotUpdate::None)),
            SnapshotUpdate::None
        );

        match saved_dir {
            Some(value) => std::env::set_var(dir_key, value),
            None => std::env::remove_var(dir_key),
        }
        match saved_mode {
            Some(value) => std::env::set_var(mode_key, value),
            None => std::env::remove_var(mode_key),
        }
    }

    #[test]
    fn png_snapshot_flow() {
        let dir = temp_dir("png");
        let opts = SnapshotOptions {
            dir: Some(dir.clone()),
            update: Some(SnapshotUpdate::Missing),
            ..Default::default()
        };
        let shot = png_bytes(8, 8, |x, y| [(x * 32) as u8, (y * 32) as u8, 0, 255]);

        // Missing snapshots are written and pass.
        assert_snapshot_png("home page!", &shot, &opts).unwrap();
        assert!(dir.join("home-page.png").is_file());
        // Identical content passes.
        assert_snapshot_png("home page!", &shot, &opts).unwrap();
        // Drift fails and leaves the actual file behind.
        let drifted = png_bytes(8, 8, |_, _| [1, 2, 3, 255]);
        let error = assert_snapshot_png("home page!", &drifted, &opts).unwrap_err();
        assert!(error.to_string().contains("differs"), "{error}");
        assert!(dir.join("home-page.actual.png").is_file());
        // `all` overwrites and passes; `none` refuses missing files.
        let all = SnapshotOptions {
            update: Some(SnapshotUpdate::All),
            ..opts.clone()
        };
        assert_snapshot_png("home page!", &drifted, &all).unwrap();
        assert_snapshot_png("home page!", &drifted, &opts).unwrap();
        let none = SnapshotOptions {
            update: Some(SnapshotUpdate::None),
            ..opts.clone()
        };
        let error = assert_snapshot_png("never-written", &shot, &none).unwrap_err();
        assert!(error.to_string().contains("no snapshot"), "{error}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn text_snapshot_flow() {
        let dir = temp_dir("text");
        let opts = SnapshotOptions {
            dir: Some(dir.clone()),
            update: Some(SnapshotUpdate::Missing),
            ..Default::default()
        };
        match_text_snapshot_with("aria home", "line1\nline2\nline3", &opts).unwrap();
        match_text_snapshot_with("aria home", "line1\nline2\nline3", &opts).unwrap();
        let error =
            match_text_snapshot_with("aria home", "line1\nCHANGED\nline3", &opts).unwrap_err();
        assert!(error.to_string().contains("line 2"), "{error}");
        assert!(error.to_string().contains("CHANGED"), "{error}");
        assert!(dir.join("aria-home.actual.snap").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
