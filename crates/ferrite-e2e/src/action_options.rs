use crate::{E2eResult, MouseButton, Page};
use std::time::Duration;

/// CSS pixels relative to an element's padding-box top-left.
#[derive(Debug, Clone, Copy)]
pub struct ActionPosition {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardModifier {
    Alt,
    Control,
    Meta,
    Shift,
}
impl KeyboardModifier {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Alt => "Alt",
            Self::Control => "Control",
            Self::Meta => "Meta",
            Self::Shift => "Shift",
        }
    }
}
/// Shared pointer action controls. Trial checks readiness without mouse/key
/// input; scrolling to the element is still allowed. Zero timeout disables it.
#[derive(Debug, Clone, Default)]
pub struct ActionOptions {
    pub force: bool,
    pub position: Option<ActionPosition>,
    pub modifiers: Vec<KeyboardModifier>,
    pub trial: bool,
    pub timeout: Option<Duration>,
}
impl ActionOptions {
    pub fn force(mut self, value: bool) -> Self {
        self.force = value;
        self
    }
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.position = Some(ActionPosition { x, y });
        self
    }
    pub fn modifiers(mut self, values: &[KeyboardModifier]) -> Self {
        self.modifiers = values.into();
        self
    }
    pub fn trial(mut self, value: bool) -> Self {
        self.trial = value;
        self
    }
    pub fn timeout(mut self, value: Duration) -> Self {
        self.timeout = Some(value);
        self
    }
}
#[derive(Debug, Clone)]
pub struct DragOptions {
    pub action: ActionOptions,
    pub target_position: Option<ActionPosition>,
    pub steps: u32,
}
impl Default for DragOptions {
    fn default() -> Self {
        Self {
            action: ActionOptions::default(),
            target_position: None,
            steps: 20,
        }
    }
}
impl DragOptions {
    pub fn action(mut self, value: ActionOptions) -> Self {
        self.action = value;
        self
    }
    pub fn target_position(mut self, x: f64, y: f64) -> Self {
        self.target_position = Some(ActionPosition { x, y });
        self
    }
    pub fn steps(mut self, value: u32) -> Self {
        self.steps = value;
        self
    }
}

/// Releases only modifiers acquired by this action, preserving keys already
/// held through Ferrite. Drop cleanup uses lifecycle-scoped uncanceled input.
pub(crate) struct InputGuard {
    page: Page,
    keys: Vec<KeyboardModifier>,
    mouse: Option<(MouseButton, f64, f64)>,
}
impl InputGuard {
    pub(crate) fn new(page: Page) -> Self {
        Self {
            page: page.with_cancellation(crate::CancellationToken::new()),
            keys: Vec::new(),
            mouse: None,
        }
    }
    pub(crate) async fn press(
        &mut self,
        page: &Page,
        modifiers: &[KeyboardModifier],
    ) -> E2eResult<()> {
        for &modifier in modifiers {
            if self.keys.contains(&modifier) || page.driver.modifier_held(modifier.key()) {
                continue;
            }
            self.keys.push(modifier);
            page.key_down(modifier.key()).await?;
        }
        Ok(())
    }
    pub(crate) fn mouse(&mut self, button: MouseButton, x: f64, y: f64) {
        self.mouse = Some((button, x, y));
    }
    pub(crate) fn mouse_completed(&mut self) {
        self.mouse = None;
    }
    pub(crate) async fn release(&mut self) -> E2eResult<()> {
        let mut failed = None;
        if let Some((button, x, y)) = self.mouse.take() {
            if let Err(error) = self.page.driver.release_mouse_button(button, x, y).await {
                failed = Some(error);
            }
        }
        for key in self.keys.drain(..).rev() {
            if let Err(error) = self.page.key_up(key.key()).await {
                if failed.is_none() {
                    failed = Some(error);
                }
            }
        }
        failed.map_or(Ok(()), Err)
    }
}
impl Drop for InputGuard {
    fn drop(&mut self) {
        if self.keys.is_empty() && self.mouse.is_none() {
            return;
        }
        let mut cleanup = Self {
            page: self.page.clone(),
            keys: std::mem::take(&mut self.keys),
            mouse: self.mouse.take(),
        };
        tokio::spawn(async move {
            let _ = cleanup.release().await;
        });
    }
}
