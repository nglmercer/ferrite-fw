//! Serialized captures with owned, independently completing restoration.
use crate::{BrowserKind, E2eError, E2eResult, ElementRect, Locator, Page, ScreenshotOptions};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// Output pixels per CSS pixel. Firefox CSS output is normalized from its native
/// device raster with a Lanczos3 resize; Chromium renders the requested scale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ScreenshotScale {
    #[default]
    Device,
    Css,
}

#[derive(Default)]
pub(crate) struct ScreenshotState {
    pub(crate) gate: Arc<AsyncMutex<()>>,
    pub(crate) background: Mutex<Option<Value>>,
    errors: Mutex<Vec<String>>,
}
impl ScreenshotState {
    pub(crate) fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

pub(crate) enum Source {
    Page,
    Element(Box<Locator>),
    Box(ElementRect),
}
static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(1);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PIXELS: f64 = 64_000_000.0;

impl ScreenshotOptions {
    pub(crate) fn validate(&self, kind: BrowserKind, element: bool) -> E2eResult<()> {
        if self.quality.is_some_and(|q| q == 0 || q > 100) {
            return Err(E2eError::Config(
                "screenshot JPEG quality must be 1..=100".into(),
            ));
        }
        if self.omit_background && self.quality.is_some() {
            return Err(E2eError::Config(
                "transparent screenshot background requires PNG".into(),
            ));
        }
        if self.omit_background && kind == BrowserKind::Firefox {
            return Err(E2eError::Config(
                "Firefox BiDi transparent screenshot background is unavailable".into(),
            ));
        }
        if element && (self.full_page || self.clip.is_some()) {
            return Err(E2eError::Config(
                "locator screenshot defines its own region; full_page/clip are unavailable".into(),
            ));
        }
        if let Some(rect) = &self.clip {
            validate_rect(rect)?;
        }
        Ok(())
    }
}
fn validate_rect(rect: &ElementRect) -> E2eResult<()> {
    if ![
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.x + rect.width,
        rect.y + rect.height,
    ]
    .iter()
    .all(|n| n.is_finite())
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        return Err(E2eError::Config(
            "screenshot clip must have finite coordinates and positive dimensions".into(),
        ));
    }
    Ok(())
}
fn trim(rect: &ElementRect, width: f64, height: f64) -> E2eResult<ElementRect> {
    validate_rect(rect)?;
    let x = rect.x.clamp(0.0, width).floor();
    let y = rect.y.clamp(0.0, height).floor();
    let right = (rect.x + rect.width).clamp(0.0, width).ceil();
    let bottom = (rect.y + rect.height).clamp(0.0, height).ceil();
    let rect = ElementRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    };
    validate_rect(&rect).map_err(|_| {
        E2eError::Config("screenshot clip is empty or outside the capture region".into())
    })?;
    Ok(rect)
}

struct CleanupWork {
    page: Page,
    token: String,
    background: bool,
    previous_background: Option<Value>,
    gate: OwnedMutexGuard<()>,
}
impl CleanupWork {
    async fn restore(self) -> E2eResult<()> {
        let result=crate::operation::Deadline::new(CLEANUP_TIMEOUT).run("screenshot restoration",async {
            let mut error=None;
            if self.background {
                let params=self.previous_background.clone().map(|color|json!({"color":color})).unwrap_or_else(||json!({}));
                if let Err(failed)=self.page.driver.raw("Emulation.setDefaultBackgroundColorOverride",params,CLEANUP_TIMEOUT).await { error=Some(failed); }
            }
            let key=serde_json::to_string(&self.token)?;
            if let Err(failed)=self.page.evaluate_value(&format!("(() => {{const key={key}; const nodes=globalThis[key]; if(Array.isArray(nodes) && nodes.ferriteOwner === key){{let failed;for(const node of nodes){{try{{node.remove()}}catch(e){{failed ||= e}}}}if(!delete globalThis[key])failed ||= Error('screenshot ownership state could not be removed');if(failed)throw failed}}return true}})()")).await {
                error=Some(match error {Some(first)=>first.with_context(&format!("DOM restoration also failed: {failed}")),None=>failed});
            }
            error.map_or(Ok(()),Err)
        }).await;
        if let Err(error) = &result {
            let mut errors = self
                .page
                .screenshot_state
                .errors
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if errors.len() < 64 {
                errors.push(error.to_string());
            }
        }
        drop(self.gate);
        result
    }
}
struct CleanupGuard {
    close: crate::operation::SharedClose,
    work: Option<CleanupWork>,
}
impl CleanupGuard {
    async fn finish(&mut self) -> E2eResult<()> {
        let work = self.work.take().expect("one capture cleanup");
        self.close.run(work.restore()).await
    }
}
impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if let Some(work) = self.work.take() {
            let close = self.close.clone();
            tokio::spawn(async move {
                let _ = close.run(work.restore()).await;
            });
        }
    }
}

const OBSERVATION: &str = r#"(() => {
const d=document.documentElement,b=document.body||d,v=visualViewport;
return {sx:scrollX,sy:scrollY,dpr:devicePixelRatio,vs:v?.scale||1,
 vx:v?.pageLeft??scrollX,vy:v?.pageTop??scrollY,
 vw:innerWidth*(v?.scale||1),vh:innerHeight*(v?.scale||1),
 dw:Math.max(d.scrollWidth,d.offsetWidth,d.clientWidth,b.scrollWidth,b.offsetWidth,b.clientWidth),
 dh:Math.max(d.scrollHeight,d.offsetHeight,d.clientHeight,b.scrollHeight,b.offsetHeight,b.clientHeight)};
})()"#;
fn number(value: &Value, key: &str) -> E2eResult<f64> {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| E2eError::Config(format!("missing screenshot {key}")))
}

async fn prepare(page: &Page, options: &ScreenshotOptions, token: &str) -> E2eResult<()> {
    let mut css = options.style.clone().unwrap_or_default();
    if options.disable_animations {
        css.push_str("\n*,*::before,*::after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;scroll-behavior:auto!important}");
    }
    if options.hide_caret {
        css.push_str("\n*{caret-color:transparent!important}");
    }
    let key = serde_json::to_string(token)?;
    let css = serde_json::to_string(&css)?;
    page.evaluate_value(&format!(r#"(() => {{
const key={key};if(Object.hasOwn(globalThis,key))throw Error('capture ownership collision');
const nodes=[],seen=new Set();nodes.ferriteOwner=key;Object.defineProperty(globalThis,key,{{value:nodes,configurable:true}});
const visit=root=>{{if(!root||seen.has(root))return;seen.add(root);
if({css}){{const style=document.createElement('style');style.setAttribute('data-ferrite-screenshot',{key});style.textContent={css};nodes.push(style);(root.head||root).appendChild(style)}}
for(const el of root.querySelectorAll('*')){{if(el.shadowRoot)visit(el.shadowRoot);if(el.tagName==='IFRAME'){{try{{visit(el.contentDocument)}}catch(_){{}}}}}}
}};visit(document);return true;
}})()"#)).await?;
    Ok(())
}
async fn mask(page: &Page, options: &ScreenshotOptions, token: &str) -> E2eResult<()> {
    let mut rects = Vec::new();
    for locator in &options.mask {
        rects.extend(locator.with_timeout(page.timeout()).state().await?.rects);
    }
    if rects.is_empty() {
        return Ok(());
    }
    let rects = serde_json::to_string(&rects)?;
    let key = serde_json::to_string(token)?;
    let color = serde_json::to_string(options.mask_color.as_deref().unwrap_or("#FF00FF"))?;
    page.evaluate_value(&format!(r#"(() => {{const nodes=globalThis[{key}];if(!nodes || nodes.ferriteOwner !== {key})throw Error('capture document replaced');
for(const r of {rects}){{const d=document.createElement('div');d.setAttribute('data-ferrite-screenshot',{key});
d.style.setProperty('all','initial','important');
for(const [k,v] of Object.entries({{position:'absolute',left:(r.x+scrollX)+'px',top:(r.y+scrollY)+'px',width:r.width+'px',height:r.height+'px',background:{color},'z-index':'2147483647','pointer-events':'none',margin:'0',padding:'0',border:'0'}}))d.style.setProperty(k,v,'important');
nodes.push(d);document.documentElement.appendChild(d)}}return true}})()"#)).await?;
    Ok(())
}

pub(crate) async fn capture(
    page: &Page,
    options: ScreenshotOptions,
    source: Source,
) -> E2eResult<Vec<u8>> {
    let element = matches!(&source, Source::Element(_));
    options.validate(page.browser_kind(), element)?;
    if let Source::Box(rect) = &source {
        validate_rect(rect)?;
    }
    let timeout = options.timeout.unwrap_or_else(|| page.timeout());
    let page = page.owning_page().with_timeout(timeout);
    page.run_operation(
        crate::operation::Deadline::new(timeout).run("screenshot capture", async {
            let gate = page.screenshot_state.gate.clone().lock_owned().await;
            let errors = page.screenshot_state.take_errors();
            if !errors.is_empty() {
                return Err(E2eError::Config(format!(
                    "previous screenshot restoration failed: {}",
                    errors.join("; ")
                )));
            }
            for locator in &options.mask {
                if !Arc::ptr_eq(&page.screenshot_state, &locator.page().screenshot_state) {
                    return Err(E2eError::Config(
                        "screenshot masks must belong to the capturing page".into(),
                    ));
                }
            }
            if let Some(color) = &options.mask_color {
                let color = serde_json::to_string(color)?;
                if !page
                    .evaluate::<bool>(&format!("CSS.supports('color',{color})"))
                    .await?
                {
                    return Err(E2eError::Config("invalid screenshot mask color".into()));
                }
            }
            let token = format!(
                "ferrite.screenshot.{}",
                NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed)
            );
            let previous_background = page
                .screenshot_state
                .background
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            let mut cleanup = CleanupGuard {
                close: Default::default(),
                work: Some(CleanupWork {
                    page: page
                        .with_timeout(CLEANUP_TIMEOUT)
                        .with_cancellation(crate::CancellationToken::new()),
                    token: token.clone(),
                    background: options.omit_background,
                    previous_background,
                    gate,
                }),
            };
            let result = async {
                prepare(&page, &options, &token).await?;
                let box_rect = match &source {
                    Source::Element(locator) => {
                        Some(locator.with_timeout(timeout).screenshot_rect().await?)
                    }
                    Source::Box(rect) => Some(rect.clone()),
                    Source::Page => None,
                };
                let observed = page.evaluate_value(OBSERVATION).await?;
                let dw = number(&observed, "dw")?;
                let dh = number(&observed, "dh")?;
                let dpr = number(&observed, "dpr")?;
                let vs = number(&observed, "vs")?;
                if dpr <= 0.0 || vs <= 0.0 {
                    return Err(E2eError::Config("invalid screenshot native scale".into()));
                }
                let document = options.full_page || box_rect.is_some();
                let mut region = if let Some(rect) = box_rect {
                    trim(
                        &ElementRect {
                            x: rect.x + number(&observed, "sx")?,
                            y: rect.y + number(&observed, "sy")?,
                            ..rect
                        },
                        dw,
                        dh,
                    )?
                } else {
                    let (width, height) = if document {
                        (dw, dh)
                    } else {
                        (number(&observed, "vw")?, number(&observed, "vh")?)
                    };
                    trim(
                        &options.clip.clone().unwrap_or(ElementRect {
                            x: 0.0,
                            y: 0.0,
                            width,
                            height,
                        }),
                        width,
                        height,
                    )?
                };
                let output_width = region.width;
                let output_height = region.height;
                let chromium = page.browser_kind() == BrowserKind::Chromium;
                let visual_scale = if document { 1.0 } else { vs };
                if chromium && !document {
                    region.x = number(&observed, "vx")? + region.x / vs;
                    region.y = number(&observed, "vy")? + region.y / vs;
                    region.width /= vs;
                    region.height /= vs;
                }
                if region.width * region.height * dpr * dpr > MAX_PIXELS
                    || output_width * output_height > MAX_PIXELS
                {
                    return Err(E2eError::Config(
                        "screenshot exceeds 64 million native pixels".into(),
                    ));
                }
                mask(&page, &options, &token).await?;
                if options.omit_background {
                    page.driver
                        .raw(
                            "Emulation.setDefaultBackgroundColorOverride",
                            json!({"color":{"r":0,"g":0,"b":0,"a":0}}),
                            timeout,
                        )
                        .await?;
                }
                let scale = if options.scale == ScreenshotScale::Css {
                    visual_scale / dpr
                } else {
                    visual_scale
                };
                let bytes = page
                    .driver
                    .capture_region(&region, document, scale, options.quality)
                    .await?;
                if !chromium && options.scale == ScreenshotScale::Css && dpr != 1.0 {
                    normalize_css(bytes, output_width, output_height, options.quality).await
                } else {
                    Ok(bytes)
                }
            }
            .await;
            let restored = cleanup.finish().await;
            match (result, restored) {
                (Ok(bytes), Ok(())) => Ok(bytes),
                (Err(error), Ok(())) => Err(error),
                (Ok(_), Err(error)) => Err(error.with_context("screenshot restoration")),
                (Err(error), Err(cleanup)) => {
                    Err(error
                        .with_context(&format!("screenshot restoration also failed: {cleanup}")))
                }
            }
        }),
    )
    .await
}
async fn normalize_css(
    bytes: Vec<u8>,
    width: f64,
    height: f64,
    quality: Option<u8>,
) -> E2eResult<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes)
            .map_err(|e| E2eError::Config(format!("screenshot decode: {e}")))?;
        let image = image.resize_exact(
            width as u32,
            height as u32,
            image::imageops::FilterType::Lanczos3,
        );
        let mut bytes = Vec::new();
        if let Some(quality) = quality {
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
                .encode_image(&image.to_rgb8())
                .map_err(|e| E2eError::Config(format!("screenshot JPEG normalization: {e}")))?;
        } else {
            image
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .map_err(|e| E2eError::Config(format!("screenshot PNG normalization: {e}")))?;
        }
        Ok(bytes)
    })
    .await
    .map_err(|e| E2eError::Config(format!("screenshot normalization task: {e}")))?
}
