//! Validated Chromium printing options; legacy basic PDF export stays separate.
use crate::{CancellationToken, E2eError, E2eResult, OperationOptions, Page};
use serde_json::{json, Value};
use std::time::Duration;

/// CSS pixels use 96 px per inch. Dimensions must be finite; margins may be zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfLength {
    Inches(f64),
    Millimeters(f64),
    Centimeters(f64),
    Pixels(f64),
}
impl PdfLength {
    fn inches(self) -> f64 {
        match self {
            Self::Inches(n) => n,
            Self::Millimeters(n) => n / 25.4,
            Self::Centimeters(n) => n / 2.54,
            Self::Pixels(n) => n / 96.0,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PdfFormat {
    #[default]
    Letter,
    Legal,
    Tabloid,
    Ledger,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
}
impl PdfFormat {
    fn dimensions(self) -> (f64, f64) {
        match self {
            Self::Letter => (8.5, 11.0),
            Self::Legal => (8.5, 14.0),
            Self::Tabloid => (11.0, 17.0),
            Self::Ledger => (17.0, 11.0),
            Self::A0 => (33.1, 46.8),
            Self::A1 => (23.4, 33.1),
            Self::A2 => (16.54, 23.4),
            Self::A3 => (11.7, 16.54),
            Self::A4 => (8.27, 11.7),
            Self::A5 => (5.83, 8.27),
            Self::A6 => (4.13, 5.83),
        }
    }
}
/// Format and custom dimensions are mutually exclusive by construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfPageSize {
    Format(PdfFormat),
    Custom { width: PdfLength, height: PdfLength },
}
impl Default for PdfPageSize {
    fn default() -> Self {
        Self::Format(PdfFormat::Letter)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfMargins {
    pub top: PdfLength,
    pub right: PdfLength,
    pub bottom: PdfLength,
    pub left: PdfLength,
}
impl PdfMargins {
    pub fn all(length: PdfLength) -> Self {
        Self {
            top: length,
            right: length,
            bottom: length,
            left: length,
        }
    }
}
impl Default for PdfMargins {
    fn default() -> Self {
        Self::all(PdfLength::Inches(0.0))
    }
}
/// New options default to Letter with zero margins (legacy pdf() uses native defaults).
/// Header/footer HTML is an isolated native print template, not page DOM content.
#[derive(Debug, Clone)]
pub struct PdfOptions {
    pub page_size: PdfPageSize,
    pub margins: PdfMargins,
    pub landscape: bool,
    pub print_background: bool,
    pub scale: f64,
    pub display_header_footer: bool,
    pub header_template: String,
    pub footer_template: String,
    pub prefer_css_page_size: bool,
    /// Empty means all pages; positive one-based pages/ranges separated by commas.
    pub page_ranges: String,
    pub tagged: bool,
    pub outline: bool,
    /// Root document font readiness and printing share one clock.
    pub wait_for_fonts: bool,
    pub timeout: Option<Duration>,
    pub cancellation: Option<CancellationToken>,
}
impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_size: Default::default(),
            margins: Default::default(),
            landscape: false,
            print_background: false,
            scale: 1.0,
            display_header_footer: false,
            header_template: String::new(),
            footer_template: String::new(),
            prefer_css_page_size: false,
            page_ranges: String::new(),
            tagged: false,
            outline: false,
            wait_for_fonts: true,
            timeout: None,
            cancellation: None,
        }
    }
}
impl PdfOptions {
    pub fn format(mut self, format: PdfFormat) -> Self {
        self.page_size = PdfPageSize::Format(format);
        self
    }
    pub fn size(mut self, width: PdfLength, height: PdfLength) -> Self {
        self.page_size = PdfPageSize::Custom { width, height };
        self
    }
    pub fn margins(mut self, margins: PdfMargins) -> Self {
        self.margins = margins;
        self
    }
    pub fn landscape(mut self, landscape: bool) -> Self {
        self.landscape = landscape;
        self
    }
    pub fn background(mut self, background: bool) -> Self {
        self.print_background = background;
        self
    }
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
    pub fn header_footer(mut self, header: impl Into<String>, footer: impl Into<String>) -> Self {
        self.display_header_footer = true;
        self.header_template = header.into();
        self.footer_template = footer.into();
        self
    }
    pub fn prefer_css_page_size(mut self, prefer: bool) -> Self {
        self.prefer_css_page_size = prefer;
        self
    }
    pub fn page_ranges(mut self, ranges: impl Into<String>) -> Self {
        self.page_ranges = ranges.into();
        self
    }
    pub fn tagged(mut self, tagged: bool) -> Self {
        self.tagged = tagged;
        self
    }
    pub fn outline(mut self, outline: bool) -> Self {
        self.outline = outline;
        self
    }
    pub fn wait_for_fonts(mut self, wait: bool) -> Self {
        self.wait_for_fonts = wait;
        self
    }
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
    pub fn cancellation(mut self, token: CancellationToken) -> Self {
        self.cancellation = Some(token);
        self
    }
    pub fn validate(&self) -> E2eResult<()> {
        self.parameters().map(|_| ())
    }
    pub(crate) fn parameters(&self) -> E2eResult<Value> {
        let (width, height) = match self.page_size {
            PdfPageSize::Format(format) => format.dimensions(),
            PdfPageSize::Custom { width, height } => (width.inches(), height.inches()),
        };
        for (name, n) in [("width", width), ("height", height)] {
            if !n.is_finite() || n <= 0.0 || !(n * 72.0).is_finite() {
                return Err(E2eError::Config(format!(
                    "PDF {name} must be a finite positive dimension"
                )));
            }
        }
        if !self.scale.is_finite() || !(0.1..=2.0).contains(&self.scale) {
            return Err(E2eError::Config(
                "PDF scale must be finite and between 0.1 and 2".into(),
            ));
        }
        let top = self.margins.top.inches();
        let right = self.margins.right.inches();
        let bottom = self.margins.bottom.inches();
        let left = self.margins.left.inches();
        for (name, n) in [
            ("top", top),
            ("right", right),
            ("bottom", bottom),
            ("left", left),
        ] {
            if !n.is_finite() || n < 0.0 {
                return Err(E2eError::Config(format!(
                    "PDF {name} margin must be finite and nonnegative"
                )));
            }
        }
        let (physical_width, physical_height) = if self.landscape {
            (height, width)
        } else {
            (width, height)
        };
        if left + right >= physical_width || top + bottom >= physical_height {
            return Err(E2eError::Config(
                "PDF margins must leave a positive printable area".into(),
            ));
        }
        validate_ranges(&self.page_ranges)?;
        Ok(
            json!({"paperWidth":width,"paperHeight":height,"landscape":self.landscape,
            "printBackground":self.print_background,"scale":self.scale,
            "marginTop":top,"marginRight":right,"marginBottom":bottom,"marginLeft":left,
            "displayHeaderFooter":self.display_header_footer,"headerTemplate":self.header_template,
            "footerTemplate":self.footer_template,"preferCSSPageSize":self.prefer_css_page_size,
            "pageRanges":self.page_ranges.trim(),"generateTaggedPDF":self.tagged,
            "generateDocumentOutline":self.outline}),
        )
    }
}
fn validate_ranges(ranges: &str) -> E2eResult<()> {
    if ranges.trim().is_empty() {
        return Ok(());
    }
    let invalid = || {
        E2eError::Config(
            "PDF page ranges require positive pages or ascending start-end pairs".into(),
        )
    };
    for range in ranges.split(',') {
        let parse = |number: &str| {
            number
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(invalid)
        };
        match range.trim().split_once('-') {
            Some((first, last)) if parse(first)? <= parse(last)? => {}
            Some(_) => return Err(invalid()),
            None => {
                parse(range)?;
            }
        }
    }
    Ok(())
}
impl Page {
    /// Validated Chromium PDF bytes. Basic Firefox printing remains available through pdf().
    pub async fn pdf_with(&self, options: PdfOptions) -> E2eResult<Vec<u8>> {
        let parameters = options.parameters()?;
        if matches!(self.driver, crate::driver::Driver::Bidi(_)) {
            return Err(E2eError::Config(
                "PDF options are supported only on Chromium; use pdf() for basic Firefox printing"
                    .into(),
            ));
        }
        let page = self.operation_page(&OperationOptions {
            timeout: options.timeout,
            cancellation: options.cancellation.clone(),
        });
        let timeout = options.timeout.unwrap_or_else(|| self.timeout());
        self.auto_step_local(
            "page.pdf_with",
            crate::StepCategory::Action,
            page.run_operation(crate::operation::Deadline::new(timeout).run(
                "PDF font readiness and printing",
                async {
                    if options.wait_for_fonts {
                        page.evaluate_value(
                            "document.fonts ? document.fonts.ready.then(() => true) : true",
                        )
                        .await?;
                    }
                    page.driver.print_pdf_with(parameters).await
                },
            )),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_formats_builders_and_native_fields() {
        for length in [
            PdfLength::Inches(1.0),
            PdfLength::Millimeters(25.4),
            PdfLength::Centimeters(2.54),
            PdfLength::Pixels(96.0),
        ] {
            assert!((length.inches() - 1.0).abs() < 1e-10);
        }
        for format in [
            PdfFormat::Letter,
            PdfFormat::Legal,
            PdfFormat::Tabloid,
            PdfFormat::Ledger,
            PdfFormat::A0,
            PdfFormat::A1,
            PdfFormat::A2,
            PdfFormat::A3,
            PdfFormat::A4,
            PdfFormat::A5,
            PdfFormat::A6,
        ] {
            PdfOptions::default().format(format).validate().unwrap();
        }
        let options = PdfOptions::default()
            .size(PdfLength::Pixels(384.0), PdfLength::Millimeters(152.4))
            .margins(PdfMargins::all(PdfLength::Inches(0.25)))
            .landscape(true)
            .background(true)
            .scale(0.5)
            .header_footer("header", "footer")
            .prefer_css_page_size(true)
            .page_ranges("1, 2-3,2")
            .tagged(true)
            .outline(true);
        let params = options.parameters().unwrap();
        assert_eq!(params["paperWidth"], 4.0);
        assert!((params["paperHeight"].as_f64().unwrap() - 6.0).abs() < 1e-10);
        assert_eq!(params["marginTop"], 0.25);
        assert_eq!(params["scale"], 0.5);
        assert_eq!(params["displayHeaderFooter"], true);
        assert_eq!(params["printBackground"], true);
        assert_eq!(params["generateTaggedPDF"], true);
        assert_eq!(params["generateDocumentOutline"], true);
        assert_eq!(
            PdfOptions::default().parameters().unwrap()["marginTop"],
            0.0
        );
        assert_eq!(
            PdfOptions::default()
                .page_ranges("  ")
                .parameters()
                .unwrap()["pageRanges"],
            ""
        );
        assert_eq!(
            options.format(PdfFormat::A4).page_size,
            PdfPageSize::Format(PdfFormat::A4)
        );
    }
    #[test]
    fn invalid_numbers_margins_ranges_fail_before_native_printing() {
        for n in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -1.0,
            f64::MAX,
        ] {
            assert!(PdfOptions::default()
                .size(PdfLength::Inches(n), PdfLength::Inches(6.0))
                .validate()
                .is_err());
        }
        for n in [f64::NAN, f64::INFINITY, -1.0, 0.0, 0.099, 2.001] {
            assert!(PdfOptions::default().scale(n).validate().is_err());
        }
        for n in [f64::NAN, f64::INFINITY, -1.0, 6.0] {
            assert!(PdfOptions::default()
                .margins(PdfMargins::all(PdfLength::Inches(n)))
                .validate()
                .is_err());
        }
        for ranges in ["0", "-1", "3-2", "1-", "one", "1,,2", "1-2-3", "4294967296"] {
            assert!(
                PdfOptions::default()
                    .page_ranges(ranges)
                    .validate()
                    .is_err(),
                "{ranges}"
            );
        }
        for ranges in ["", " ", "1", "1,3", "1-3,2"] {
            PdfOptions::default()
                .page_ranges(ranges)
                .validate()
                .unwrap();
        }
        // Landscape swaps the printable geometry used to validate margins.
        PdfOptions::default()
            .size(PdfLength::Inches(2.0), PdfLength::Inches(6.0))
            .landscape(true)
            .margins(PdfMargins {
                left: PdfLength::Inches(2.0),
                right: PdfLength::Inches(1.0),
                ..Default::default()
            })
            .validate()
            .unwrap();
    }
}
