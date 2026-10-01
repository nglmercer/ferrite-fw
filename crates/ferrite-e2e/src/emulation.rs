//! Typed Chromium media/device/user-agent emulation with explicit reset semantics.
use crate::{
    BrowserKind, ColorScheme, E2eError, E2eResult, OperationOptions, Page, ReducedMotion, Viewport,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Keep the current typed override, replace it, or restore the native default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EmulationOverride<T> {
    #[default]
    Keep,
    Set(T),
    Reset,
}
impl<T: Copy> EmulationOverride<T> {
    fn apply(self, current: &mut Option<T>) {
        match self {
            Self::Keep => {}
            Self::Set(value) => *current = Some(value),
            Self::Reset => *current = None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Screen,
    Print,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaColorScheme {
    Light,
    Dark,
    NoPreference,
}
impl From<ColorScheme> for MediaColorScheme {
    fn from(value: ColorScheme) -> Self {
        match value {
            ColorScheme::Light => Self::Light,
            ColorScheme::Dark => Self::Dark,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForcedColors {
    None,
    Active,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastPreference {
    NoPreference,
    More,
    Less,
    Custom,
}
#[derive(Debug, Clone, Default)]
pub struct MediaOptions {
    pub media: EmulationOverride<MediaType>,
    pub color_scheme: EmulationOverride<MediaColorScheme>,
    pub reduced_motion: EmulationOverride<ReducedMotion>,
    pub forced_colors: EmulationOverride<ForcedColors>,
    pub contrast: EmulationOverride<ContrastPreference>,
    pub operation: OperationOptions,
}
impl MediaOptions {
    pub fn reset() -> Self {
        Self {
            media: EmulationOverride::Reset,
            color_scheme: EmulationOverride::Reset,
            reduced_motion: EmulationOverride::Reset,
            forced_colors: EmulationOverride::Reset,
            contrast: EmulationOverride::Reset,
            operation: OperationOptions::default(),
        }
    }
    fn changes(&self) -> bool {
        !matches!(self.media, EmulationOverride::Keep)
            || !matches!(self.color_scheme, EmulationOverride::Keep)
            || !matches!(self.reduced_motion, EmulationOverride::Keep)
            || !matches!(self.forced_colors, EmulationOverride::Keep)
            || !matches!(self.contrast, EmulationOverride::Keep)
    }
}
#[derive(Default)]
pub(crate) struct EmulationState {
    media: MediaProfile,
}
#[derive(Clone, Default)]
struct MediaProfile {
    media: Option<MediaType>,
    color_scheme: Option<MediaColorScheme>,
    reduced_motion: Option<ReducedMotion>,
    forced_colors: Option<ForcedColors>,
    contrast: Option<ContrastPreference>,
}
impl MediaProfile {
    fn patch(&mut self, options: &MediaOptions) {
        options.media.apply(&mut self.media);
        options.color_scheme.apply(&mut self.color_scheme);
        options.reduced_motion.apply(&mut self.reduced_motion);
        options.forced_colors.apply(&mut self.forced_colors);
        options.contrast.apply(&mut self.contrast);
    }
    fn params(&self) -> Value {
        json!({"media":self.media.map(|value|match value{MediaType::Screen=>"screen",MediaType::Print=>"print"}).unwrap_or(""),"features":[
            {"name":"prefers-color-scheme","value":self.color_scheme.map(|value|match value{MediaColorScheme::Dark=>"dark",MediaColorScheme::Light=>"light",MediaColorScheme::NoPreference=>"no-preference"}).unwrap_or("")},
            {"name":"prefers-reduced-motion","value":self.reduced_motion.map(|value|match value{ReducedMotion::Reduce=>"reduce",ReducedMotion::NoPreference=>"no-preference"}).unwrap_or("")},
            {"name":"forced-colors","value":self.forced_colors.map(|value|match value{ForcedColors::Active=>"active",ForcedColors::None=>"none"}).unwrap_or("")},
            {"name":"prefers-contrast","value":self.contrast.map(|value|match value{ContrastPreference::More=>"more",ContrastPreference::Less=>"less",ContrastPreference::Custom=>"custom",ContrastPreference::NoPreference=>"no-preference"}).unwrap_or("")}
        ]})
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenOrientationType {
    PortraitPrimary,
    PortraitSecondary,
    LandscapePrimary,
    LandscapeSecondary,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenOrientation {
    pub kind: ScreenOrientationType,
    pub angle: u16,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPosition {
    pub x: u32,
    pub y: u32,
}
#[derive(Debug, Clone)]
pub struct DeviceMetricsOptions {
    pub viewport: Viewport,
    /// Zero restores the native device scale factor.
    pub device_scale_factor: f64,
    pub mobile: bool,
    pub screen: Option<Viewport>,
    pub position: Option<ScreenPosition>,
    pub orientation: Option<ScreenOrientation>,
    /// None or zero disables touch emulation; 1..=16 sets touch points.
    pub touch_points: Option<u8>,
    pub scale: Option<f64>,
    pub operation: OperationOptions,
}
impl DeviceMetricsOptions {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            viewport: Viewport { width, height },
            device_scale_factor: 1.0,
            mobile: false,
            screen: None,
            position: None,
            orientation: None,
            touch_points: None,
            scale: None,
            operation: OperationOptions::default(),
        }
    }
    fn validate(&self) -> E2eResult<()> {
        const MAX: u32 = 10_000_000;
        if self.viewport.width > MAX
            || self.viewport.height > MAX
            || self
                .screen
                .is_some_and(|screen| screen.width > MAX || screen.height > MAX)
            || self
                .position
                .is_some_and(|position| position.x > MAX || position.y > MAX)
        {
            return Err(E2eError::Config(
                "device dimensions/positions must be within 0..=10000000".into(),
            ));
        }
        if self.position.is_some_and(|position| {
            position.x > self.screen.map_or(0, |screen| screen.width)
                || position.y > self.screen.map_or(0, |screen| screen.height)
        }) {
            return Err(E2eError::Config(
                "device position must be on the overridden screen".into(),
            ));
        }
        if !self.device_scale_factor.is_finite()
            || self.device_scale_factor < 0.0
            || self
                .scale
                .is_some_and(|scale| !scale.is_finite() || scale <= 0.0 || scale > 10.0)
        {
            return Err(E2eError::Config(
                "device scale factor must be finite/nonnegative and view scale within (0,10]"
                    .into(),
            ));
        }
        if self.touch_points.is_some_and(|points| points > 16)
            || self
                .orientation
                .is_some_and(|orientation| orientation.angle >= 360)
        {
            return Err(E2eError::Config(
                "touch points must be 0..=16 and orientation angle within 0..360".into(),
            ));
        }
        Ok(())
    }
    fn params(&self) -> Value {
        let mut params = json!({"width":self.viewport.width,"height":self.viewport.height,"deviceScaleFactor":self.device_scale_factor,"mobile":self.mobile});
        if let Some(screen) = self.screen {
            params["screenWidth"] = json!(screen.width);
            params["screenHeight"] = json!(screen.height);
        }
        if let Some(position) = self.position {
            params["positionX"] = json!(position.x);
            params["positionY"] = json!(position.y);
        }
        if let Some(orientation) = self.orientation {
            params["screenOrientation"] = json!({"type":match orientation.kind {ScreenOrientationType::PortraitPrimary=>"portraitPrimary",ScreenOrientationType::PortraitSecondary=>"portraitSecondary",ScreenOrientationType::LandscapePrimary=>"landscapePrimary",ScreenOrientationType::LandscapeSecondary=>"landscapeSecondary"},"angle":orientation.angle});
        }
        if let Some(scale) = self.scale {
            params["scale"] = json!(scale);
        }
        params
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserAgentBrandVersion {
    pub brand: String,
    pub version: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAgentMetadata {
    pub brands: Vec<UserAgentBrandVersion>,
    pub full_version_list: Vec<UserAgentBrandVersion>,
    pub platform: String,
    pub platform_version: String,
    pub architecture: String,
    pub model: String,
    pub mobile: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wow64: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub form_factors: Vec<String>,
}
#[derive(Debug, Clone, Default)]
pub struct UserAgentOptions {
    /// An empty user agent without other fields clears all UA overrides.
    pub user_agent: String,
    pub accept_language: Option<String>,
    pub platform: Option<String>,
    pub metadata: Option<UserAgentMetadata>,
    pub operation: OperationOptions,
}
fn validate_text(text: &str) -> E2eResult<()> {
    if text.len() > 4096 || text.chars().any(char::is_control) {
        Err(E2eError::Config(
            "emulation strings must have at most 4096 UTF-8 bytes and no control characters".into(),
        ))
    } else {
        Ok(())
    }
}
fn validate_hint(text: &str) -> E2eResult<()> {
    validate_text(text)?;
    if !text.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(E2eError::Config(
            "UA client-hint metadata must use printable ASCII".into(),
        ));
    }
    Ok(())
}
impl UserAgentOptions {
    fn params(&self) -> E2eResult<Value> {
        validate_text(&self.user_agent)?;
        if self.user_agent.is_empty()
            && (self.accept_language.is_some()
                || self.platform.is_some()
                || self.metadata.is_some())
        {
            return Err(E2eError::Config(
                "empty user agent clears overrides and cannot include extra UA fields".into(),
            ));
        }
        let mut params = json!({"userAgent":self.user_agent});
        if let Some(language) = &self.accept_language {
            validate_text(language)?;
            params["acceptLanguage"] = json!(language);
        }
        if let Some(platform) = &self.platform {
            validate_text(platform)?;
            params["platform"] = json!(platform);
        }
        if let Some(metadata) = &self.metadata {
            if metadata.brands.len() > 32
                || metadata.full_version_list.len() > 32
                || metadata.form_factors.len() > 8
            {
                return Err(E2eError::Config(
                    "UA metadata allows at most 32 brands/full versions and eight form factors"
                        .into(),
                ));
            }
            for text in [
                &metadata.platform,
                &metadata.platform_version,
                &metadata.architecture,
                &metadata.model,
            ]
            .into_iter()
            .chain(metadata.bitness.iter())
            .chain(metadata.form_factors.iter())
            {
                validate_hint(text)?;
            }
            for brand in metadata.brands.iter().chain(&metadata.full_version_list) {
                validate_hint(&brand.brand)?;
                validate_hint(&brand.version)?;
            }
            params["userAgentMetadata"] = serde_json::to_value(metadata)?;
        }
        if serde_json::to_vec(&params)?.len() > 65536 {
            return Err(E2eError::Config(
                "UA override exceeds 64 KiB encoded metadata budget".into(),
            ));
        }
        Ok(params)
    }
}
impl Page {
    fn require_emulation(&self) -> E2eResult<()> {
        if self.browser_kind() == BrowserKind::Chromium {
            Ok(())
        } else {
            Err(E2eError::Config(
                "extended emulation requires Chromium; Firefox overrides are not supported".into(),
            ))
        }
    }
    pub async fn emulate_media_with(&self, options: MediaOptions) -> E2eResult<()> {
        if options.changes() {
            self.require_emulation()?;
        }
        let page = self.operation_page(&options.operation);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "media emulation",
            async {
                if !options.changes() {
                    return Ok(());
                }
                let mut state = page.emulation_state.lock().await;
                // Preserve intended overrides across an uncertain native timeout; a subsequent
                // patch sends the complete desired profile again, without claiming rollback.
                state.media.patch(&options);
                page.call("Emulation.setEmulatedMedia", state.media.params())
                    .await?;
                Ok(())
            },
        ))
        .await
    }
    pub async fn emulate_device_metrics(&self, options: DeviceMetricsOptions) -> E2eResult<()> {
        options.validate()?;
        self.require_emulation()?;
        let page = self.operation_page(&options.operation);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "device metrics emulation",
            async {
                let _state = page.emulation_state.lock().await;
                page.call("Emulation.setDeviceMetricsOverride", options.params())
                    .await?;
                let points = options.touch_points.unwrap_or(0);
                let touch = if points == 0 {
                    json!({"enabled":false})
                } else {
                    json!({"enabled":true,"maxTouchPoints":points})
                };
                page.call("Emulation.setTouchEmulationEnabled", touch)
                    .await?;
                Ok(())
            },
        ))
        .await
    }
    pub async fn reset_device_metrics(&self, options: OperationOptions) -> E2eResult<()> {
        self.require_emulation()?;
        let page = self.operation_page(&options);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "reset device metrics",
            async {
                let _state = page.emulation_state.lock().await;
                page.call("Emulation.clearDeviceMetricsOverride", json!({}))
                    .await?;
                page.call(
                    "Emulation.setTouchEmulationEnabled",
                    json!({"enabled":false}),
                )
                .await?;
                Ok(())
            },
        ))
        .await
    }
    pub async fn set_user_agent_with(&self, options: UserAgentOptions) -> E2eResult<()> {
        let params = options.params()?;
        self.require_emulation()?;
        let page = self.operation_page(&options.operation);
        page.run_operation(crate::operation::Deadline::new(page.timeout()).run(
            "user-agent emulation",
            async {
                let _state = page.emulation_state.lock().await;
                page.call("Emulation.setUserAgentOverride", params).await?;
                Ok(())
            },
        ))
        .await
    }
    pub async fn reset_user_agent(&self, operation: OperationOptions) -> E2eResult<()> {
        self.set_user_agent_with(UserAgentOptions {
            operation,
            ..Default::default()
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_keep_and_individual_reset_preserve_other_features() {
        let mut profile = MediaProfile::default();
        profile.patch(&MediaOptions {
            media: EmulationOverride::Set(MediaType::Print),
            color_scheme: EmulationOverride::Set(MediaColorScheme::Dark),
            forced_colors: EmulationOverride::Set(ForcedColors::Active),
            ..Default::default()
        });
        profile.patch(&MediaOptions {
            color_scheme: EmulationOverride::Reset,
            contrast: EmulationOverride::Set(ContrastPreference::More),
            ..Default::default()
        });
        assert_eq!(profile.media, Some(MediaType::Print));
        assert_eq!(profile.color_scheme, None);
        assert_eq!(profile.forced_colors, Some(ForcedColors::Active));
        profile.patch(&MediaOptions::reset());
        assert_eq!(profile.params()["media"], json!(""));
        assert!(profile.params()["features"]
            .as_array()
            .unwrap()
            .iter()
            .all(|feature| feature["value"] == json!("")));
    }
    #[test]
    fn metric_validation_rejects_nonfinite_out_of_native_range_and_touch() {
        for factor in [f64::NAN, f64::INFINITY, -1.0] {
            let mut options = DeviceMetricsOptions::new(320, 240);
            options.device_scale_factor = factor;
            assert!(options.validate().is_err());
        }
        let mut options = DeviceMetricsOptions::new(10_000_001, 240);
        assert!(options.validate().is_err());
        options.viewport.width = 320;
        options.touch_points = Some(17);
        assert!(options.validate().is_err());
        options.touch_points = Some(0);
        options.orientation = Some(ScreenOrientation {
            kind: ScreenOrientationType::PortraitPrimary,
            angle: 360,
        });
        assert!(options.validate().is_err());
        options.orientation = None;
        options.scale = Some(0.0);
        assert!(options.validate().is_err());
        options.scale = Some(10.01);
        assert!(options.validate().is_err());
        options.scale = None;
        options.position = Some(ScreenPosition { x: 1, y: 0 });
        assert!(options.validate().is_err());
        options.screen = Some(Viewport {
            width: 1,
            height: 1,
        });
        options.validate().unwrap();
        options.position = Some(ScreenPosition { x: 2, y: 0 });
        assert!(options.validate().is_err());
        options.position = None;
        options.device_scale_factor = 0.0;
        options.validate().unwrap();
        assert_eq!(options.params()["deviceScaleFactor"], json!(0.0));
    }
    #[test]
    fn ua_validation_caps_vectors_strings_and_encoded_budget() {
        let mut options = UserAgentOptions {
            user_agent: "Ferrite\r\nInjected: true".into(),
            ..Default::default()
        };
        assert!(options.params().is_err());
        options.user_agent = "x".repeat(4097);
        assert!(options.params().is_err());
        options = UserAgentOptions {
            platform: Some("platform".into()),
            ..Default::default()
        };
        assert!(options.params().is_err());
        let metadata = UserAgentMetadata {
            brands: vec![
                UserAgentBrandVersion {
                    brand: "brand".into(),
                    version: "1".into()
                };
                33
            ],
            ..Default::default()
        };
        options = UserAgentOptions {
            user_agent: "Ferrite/1".into(),
            metadata: Some(metadata),
            ..Default::default()
        };
        assert!(options.params().is_err());
        options.metadata.as_mut().unwrap().brands.clear();
        options.metadata.as_mut().unwrap().platform = "非ASCII".into();
        assert!(options.params().is_err());
        options.metadata.as_mut().unwrap().platform = "OS".into();
        options.metadata.as_mut().unwrap().brands = vec![
            UserAgentBrandVersion {
                brand: "x".repeat(4096),
                version: "1".into()
            };
            32
        ];
        assert!(options.params().is_err());
        assert_eq!(
            UserAgentOptions::default().params().unwrap(),
            json!({"userAgent":""})
        );
    }
}
