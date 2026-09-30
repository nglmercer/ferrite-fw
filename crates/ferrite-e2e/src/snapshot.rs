//! Snapshot assertions: screenshot and text snapshots (Playwright
//! `toHaveScreenshot` / `toMatchSnapshot`).
//!
//! Snapshots live under `<output_dir>/snapshots/` as `<slug>.png` / `<slug>.snap`.
//! The directory resolves as explicit [`SnapshotOptions::dir`], then the page's
//! resolved runner/project seed. Standalone pages use `FERRITE_SNAPSHOT_DIR`
//! followed by `test-results/snapshots`. Update behavior resolves as explicit
//! [`SnapshotOptions::update`], then the runner's captured mode. Standalone pages
//! use `FERRITE_UPDATE_SNAPSHOTS` (`missing`/`changed`/`all`/`none`), then
//! [`SnapshotUpdate::Missing`]. Runner inputs are fixed at run startup; they do
//! not mutate the process environment. Explicit assertion options take precedence.
//! An explicit path_template replaces the legacy slug path. Runner pages inherit
//! project/run templates and frozen metadata; standalone helpers can use
//! TestInfo::snapshot_options or supply SnapshotPathContext explicitly.

use std::path::{Path, PathBuf};

use crate::error::{E2eError, E2eResult};

/// What to do about snapshot files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotUpdate {
    /// Write new snapshots, compare existing ones (default).
    #[default]
    Missing,
    /// Always overwrite with the actual value.
    All,
    /// Create missing snapshots and replace only snapshots outside the tolerance.
    Changed,
    /// Never write; missing snapshots fail.
    None,
}

impl SnapshotUpdate {
    /// Parse `missing` / `changed` / `all` / `none` (case-insensitive).
    pub fn parse(name: &str) -> E2eResult<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "missing" => Ok(Self::Missing),
            "all" => Ok(Self::All),
            "changed" => Ok(Self::Changed),
            "none" => Ok(Self::None),
            other => Err(E2eError::Config(format!(
                "unknown snapshot update mode {other:?}: expected \"missing\", \"changed\", \"all\" or \"none\""
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
                         (want \"missing\", \"changed\", \"all\" or \"none\")"
                    );
                    Self::Missing
                }
            },
            _ => Self::Missing,
        }
    }
}

/// Options for snapshot assertions.
#[derive(Debug, Clone)]
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
    /// Capture options for page/locator assertions. None uses CSS scale, hidden
    /// carets and animation suppression. JPEG is not a snapshot format.
    pub capture: Option<crate::ScreenshotOptions>,
    /// Wait for reachable same-origin documents' fonts after capture style preparation.
    pub wait_for_fonts: bool,
    /// Baseline path template; None retains the legacy slug path.
    pub path_template: Option<String>,
    /// Explicit metadata/base for the template; runner assertions inherit it.
    pub path_context: Option<crate::SnapshotPathContext>,
}

impl Default for SnapshotOptions {
    fn default() -> Self {
        Self {
            max_diff_pixels: 0,
            max_diff_ratio: 0.0,
            threshold: 0,
            dir: None,
            update: None,
            capture: None,
            wait_for_fonts: true,
            path_template: None,
            path_context: None,
        }
    }
}

impl SnapshotOptions {
    pub(crate) fn validate(&self) -> E2eResult<()> {
        if let Some(template) = &self.path_template {
            crate::snapshot_path::validate_template(template)?;
        }
        if !self.max_diff_ratio.is_finite() || !(0.0..=1.0).contains(&self.max_diff_ratio) {
            return Err(E2eError::Config(
                "snapshot max_diff_ratio must be finite and between zero and one".into(),
            ));
        }
        if self
            .capture
            .as_ref()
            .is_some_and(|capture| capture.quality.is_some())
        {
            return Err(E2eError::Config(
                "screenshot snapshots require PNG; JPEG quality is unavailable".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn capture_options(&self) -> crate::ScreenshotOptions {
        let mut capture = self
            .capture
            .clone()
            .unwrap_or_else(|| crate::ScreenshotOptions {
                scale: crate::ScreenshotScale::Css,
                disable_animations: true,
                hide_caret: true,
                ..Default::default()
            });
        // The assertion owns the shared window. An explicit per-capture cap may
        // shorten it, but the page's unrelated action timeout must not renew it.
        capture.timeout.get_or_insert(std::time::Duration::ZERO);
        capture
    }
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
    compare_png_checked(actual, expected, threshold, || Ok(()))
}

pub(crate) fn validate_png_checked(
    bytes: &[u8],
    check: impl Fn() -> E2eResult<()>,
) -> E2eResult<SnapshotDiff> {
    check()?;
    let image = decode_png(bytes, "actual")?;
    check()?;
    let (width, height) = (image.width(), image.height());
    Ok(SnapshotDiff {
        width,
        height,
        diff_pixels: 0,
        total_pixels: u64::from(width) * u64::from(height),
        bounds: None,
    })
}

pub(crate) fn compare_png_checked(
    actual: &[u8],
    expected: &[u8],
    threshold: u8,
    mut check: impl FnMut() -> E2eResult<()>,
) -> E2eResult<SnapshotDiff> {
    check()?;
    let actual_img = decode_png(actual, "actual")?.into_rgba8();
    check()?;
    let expected_img = decode_png(expected, "snapshot")?.into_rgba8();
    check()?;
    if actual_img.dimensions() != expected_img.dimensions() {
        let (aw, ah) = actual_img.dimensions();
        let (ew, eh) = expected_img.dimensions();
        return Err(E2eError::Expect(format!(
            "snapshot size differs: actual {aw}x{ah}, expected {ew}x{eh}"
        )));
    }
    let (width, height) = actual_img.dimensions();
    let actual_rgba = actual_img;
    let expected_rgba = expected_img;
    let mut diff_pixels = 0u64;
    let (mut min_x, mut min_y) = (width, height);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    for (index, (x, y, pixel)) in actual_rgba.enumerate_pixels().enumerate() {
        if index & 4095 == 0 {
            check()?;
        }
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

fn decode_png(bytes: &[u8], label: &str) -> E2eResult<image::DynamicImage> {
    decode_image(bytes, image::ImageFormat::Png, label)
}

pub(crate) fn decode_image(
    bytes: &[u8],
    format: image::ImageFormat,
    label: &str,
) -> E2eResult<image::DynamicImage> {
    let kind = if format == image::ImageFormat::Png {
        "PNG"
    } else {
        "JPEG"
    };
    if bytes.len() > 512 * 1024 * 1024 {
        return Err(E2eError::Config(format!(
            "{label} {kind} exceeds the 512 MiB encoded input limit"
        )));
    }
    let reader = || {
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(64_000_000);
        limits.max_image_height = Some(64_000_000);
        reader.limits(limits);
        reader
    };
    let (width, height) = reader()
        .into_dimensions()
        .map_err(|error| E2eError::Config(format!("cannot decode {label} {kind}: {error}")))?;
    if u64::from(width) * u64::from(height) > 64_000_000 {
        return Err(E2eError::Config(format!(
            "{label} {kind} exceeds the 64 million pixel comparison limit"
        )));
    }
    reader()
        .decode()
        .map_err(|error| E2eError::Config(format!("cannot decode {label} {kind}: {error}")))
}

/// Visual diagnostic, using the same per-channel threshold as comparison.
/// Different pixels are red; nonoverlapping dimension regions are magenta.
/// Unchanged pixels are muted grayscale. The union raster is bounded separately.
#[cfg(test)]
pub(crate) fn diff_png(actual: &[u8], expected: &[u8], threshold: u8) -> E2eResult<Vec<u8>> {
    diff_png_checked(actual, expected, threshold, || Ok(()))
}

pub(crate) fn diff_png_checked(
    actual: &[u8],
    expected: &[u8],
    threshold: u8,
    mut check: impl FnMut() -> E2eResult<()>,
) -> E2eResult<Vec<u8>> {
    check()?;
    let actual = decode_png(actual, "actual")?.into_rgba8();
    check()?;
    let expected = decode_png(expected, "snapshot")?.into_rgba8();
    check()?;
    let width = actual.width().max(expected.width());
    let height = actual.height().max(expected.height());
    if u64::from(width) * u64::from(height) > 64_000_000 {
        return Err(E2eError::Config(
            "snapshot diff exceeds the 64 million pixel limit".into(),
        ));
    }
    let mut diff = image::RgbaImage::new(width, height);
    for (index, (x, y, output)) in diff.enumerate_pixels_mut().enumerate() {
        if index & 4095 == 0 {
            check()?;
        }
        let in_actual = x < actual.width() && y < actual.height();
        let in_expected = x < expected.width() && y < expected.height();
        if !in_actual && !in_expected {
            *output = image::Rgba([255, 255, 255, 0]);
            continue;
        }
        if in_actual != in_expected {
            *output = image::Rgba([255, 0, 255, 255]);
            continue;
        }
        let a = actual.get_pixel(x, y);
        let e = expected.get_pixel(x, y);
        *output = if a
            .0
            .iter()
            .zip(e.0.iter())
            .any(|(a, e)| a.abs_diff(*e) > threshold)
        {
            image::Rgba([255, 0, 0, 255])
        } else {
            let gray = (u16::from(e[0]) + u16::from(e[1]) + u16::from(e[2])) / 3;
            let muted = (128 + gray / 2) as u8;
            image::Rgba([muted, muted, muted, 255])
        };
    }
    check()?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    diff.write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| E2eError::Config(format!("encoding snapshot diff PNG: {error}")))?;
    check()?;
    Ok(bytes.into_inner())
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

/// Snapshot file path for `name`, including an explicit template when present.
pub(crate) fn snap_path_for(
    name: &str,
    kind: crate::SnapshotKind,
    opts: &SnapshotOptions,
) -> E2eResult<PathBuf> {
    opts.path(name, kind)
}

/// Assert PNG bytes against the named snapshot (`<slug>.png`).
///
/// Missing snapshots are written under `Missing`/`All` (pass) and fail under
/// `None`; `All` overwrites unconditionally. Mismatches write
/// `<slug>.actual.png` next to the expected file and fail with the diff summary.
pub fn assert_snapshot_png(name: &str, actual: &[u8], opts: &SnapshotOptions) -> E2eResult<()> {
    opts.validate()?;
    // Invalid bytes must never become a new baseline, even under update=all.
    compare_png(actual, actual, opts.threshold)?;
    let path = opts.path(name, crate::SnapshotKind::Screenshot)?;
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
        Ok(_) if update == SnapshotUpdate::Changed => {
            std::fs::write(&path, actual)?;
            Ok(())
        }
        Ok(diff) => Err(mismatch(&path, name, "png", actual, diff.summary())),
        Err(error) if error.code() == "FERRITE_E2E_EXPECT" => {
            if update == SnapshotUpdate::Changed {
                std::fs::write(&path, actual)?;
                Ok(())
            } else {
                Err(mismatch(&path, name, "png", actual, error.to_string()))
            }
        }
        Err(error) => Err(error),
    }
}

/// Assert text against the named snapshot (`<slug>.snap`, same update rules).
pub fn assert_snapshot_text(name: &str, actual: &str, opts: &SnapshotOptions) -> E2eResult<()> {
    opts.validate()?;
    let path = opts.path(name, crate::SnapshotKind::Text)?;
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
    } else if update == SnapshotUpdate::Changed {
        std::fs::write(&path, actual)?;
        Ok(())
    } else {
        Err(mismatch(
            &path,
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
fn mismatch(path: &Path, name: &str, ext: &str, actual: &[u8], detail: String) -> E2eError {
    let actual_path = path.with_extension(format!("actual.{ext}"));
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = actual_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&actual_path, actual)
    })();
    let error = E2eError::Expect(format!(
        "snapshot {name:?} differs: {detail} (actual: {})",
        actual_path.display()
    ));
    match result {
        Ok(()) => error,
        Err(write) => error.with_context(&format!("writing failure artifact also failed: {write}")),
    }
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

    #[test]
    fn png_and_jpeg_headers_reject_excessive_pixels_before_raster_decode() {
        let image = image::RgbImage::from_pixel(1, 1, image::Rgb([255, 0, 0]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut png = png.into_inner();
        assert_eq!(&png[12..16], b"IHDR");
        png[16..20].copy_from_slice(&8001u32.to_be_bytes());
        png[20..24].copy_from_slice(&8001u32.to_be_bytes());
        let mut crc = u32::MAX;
        for byte in &png[12..29] {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb88320u32 & (0u32.wrapping_sub(crc & 1)));
            }
        }
        png[29..33].copy_from_slice(&(!crc).to_be_bytes());

        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image)
            .unwrap();
        let frame = jpeg
            .windows(2)
            .position(|bytes| bytes == [0xff, 0xc0])
            .unwrap();
        jpeg[frame + 5..frame + 7].copy_from_slice(&8001u16.to_be_bytes());
        jpeg[frame + 7..frame + 9].copy_from_slice(&8001u16.to_be_bytes());
        // These tiny payloads describe huge rasters while retaining only one
        // compressed pixel. The size guard must win before raster decoding.
        for (bytes, format) in [
            (&png, image::ImageFormat::Png),
            (&jpeg, image::ImageFormat::Jpeg),
        ] {
            let error = decode_image(bytes, format, "test").unwrap_err();
            assert_eq!(error.code(), "FERRITE_E2E_CONFIG");
            assert!(error.to_string().contains("64 million pixel"), "{error}");
        }
    }

    #[test]
    fn visual_diff_uses_threshold_and_marks_dimension_regions() {
        let png = |image: image::RgbaImage| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
            bytes.into_inner()
        };
        let expected = png(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([100, 100, 100, 255]),
        ));
        let mut actual = image::RgbaImage::from_pixel(3, 1, image::Rgba([102, 100, 100, 255]));
        actual.put_pixel(1, 0, image::Rgba([200, 0, 0, 255]));
        let actual = png(actual);
        let diff = image::load_from_memory(&diff_png(&actual, &expected, 2).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(diff.dimensions(), (3, 2));
        assert_eq!(diff.get_pixel(0, 0).0, [178, 178, 178, 255]);
        assert_eq!(diff.get_pixel(1, 0).0, [255, 0, 0, 255]);
        assert_eq!(diff.get_pixel(2, 0).0, [255, 0, 255, 255]);
        assert_eq!(diff.get_pixel(0, 1).0, [255, 0, 255, 255]);
        assert_eq!(diff.get_pixel(2, 1).0, [255, 255, 255, 0]);
        assert_eq!(
            diff_png(b"invalid", &expected, 0).unwrap_err().code(),
            "FERRITE_E2E_CONFIG"
        );
    }
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
            SnapshotUpdate::parse("CHANGED").unwrap(),
            SnapshotUpdate::Changed
        );
        assert_eq!(
            serde_json::to_string(&SnapshotUpdate::Changed).unwrap(),
            "\"changed\""
        );
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
    fn changed_updates_only_mismatches_and_invalid_png_never_becomes_a_baseline() {
        let dir = tempfile::tempdir().unwrap();
        let options = SnapshotOptions {
            dir: Some(dir.path().into()),
            update: Some(SnapshotUpdate::Changed),
            ..Default::default()
        };
        let first = png_bytes(2, 2, |_, _| [10, 20, 30, 255]);
        let second = png_bytes(2, 2, |_, _| [100, 20, 30, 255]);
        assert_snapshot_png("changed", &first, &options).unwrap();
        let path = dir.path().join("changed.png");
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_snapshot_png("changed", &first, &options).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        assert_snapshot_png("changed", &second, &options).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), second);
        let tolerant = SnapshotOptions {
            threshold: 100,
            ..options.clone()
        };
        assert_snapshot_png("changed", &first, &tolerant).unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            second,
            "a match within tolerance must retain the baseline"
        );
        let bigger = png_bytes(3, 2, |_, _| [100, 20, 30, 255]);
        assert_snapshot_png("changed", &bigger, &options).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bigger);
        for mode in [
            SnapshotUpdate::Missing,
            SnapshotUpdate::All,
            SnapshotUpdate::Changed,
        ] {
            let options = SnapshotOptions {
                update: Some(mode),
                ..options.clone()
            };
            assert_eq!(
                assert_snapshot_png("invalid", b"not png", &options)
                    .unwrap_err()
                    .code(),
                "FERRITE_E2E_CONFIG"
            );
            assert!(!dir.path().join("invalid.png").exists());
        }
        std::fs::write(&path, b"corrupt baseline").unwrap();
        assert_eq!(
            assert_snapshot_png("changed", &first, &options)
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CONFIG"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"corrupt baseline");
        assert_snapshot_text("text", "first", &options).unwrap();
        assert_snapshot_text("text", "second", &options).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("text.snap")).unwrap(),
            "second"
        );
        let invalid = SnapshotOptions {
            max_diff_ratio: f32::NAN,
            ..options
        };
        assert_eq!(
            assert_snapshot_png("nan", &first, &invalid)
                .unwrap_err()
                .code(),
            "FERRITE_E2E_CONFIG"
        );
        assert!(!dir.path().join("nan.png").exists());
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
