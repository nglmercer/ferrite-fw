use serde_json::{json, Value};

/// Constructor for a programmatically dispatched DOM event. These events are
/// synthetic; trusted input is provided by the existing mouse/keyboard APIs.
#[derive(Debug, Clone, Copy, Default)]
pub enum DomEventKind {
    #[default]
    Auto,
    Event,
    Custom,
    Mouse,
    Keyboard,
    Focus,
    Input,
    Pointer,
    Wheel,
    Drag,
}
impl DomEventKind {
    pub(crate) fn constructor(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Event => "Event",
            Self::Custom => "CustomEvent",
            Self::Mouse => "MouseEvent",
            Self::Keyboard => "KeyboardEvent",
            Self::Focus => "FocusEvent",
            Self::Input => "InputEvent",
            Self::Pointer => "PointerEvent",
            Self::Wheel => "WheelEvent",
            Self::Drag => "DragEvent",
        }
    }
}

/// JSON event initialization. Native event flags default to true and can be
/// overridden in `init` or with the flag builders. Live handles are not accepted.
#[derive(Debug, Clone)]
pub struct DispatchEventOptions {
    pub kind: DomEventKind,
    pub init: Value,
}
impl Default for DispatchEventOptions {
    fn default() -> Self {
        Self {
            kind: DomEventKind::Auto,
            init: json!({}),
        }
    }
}
impl DispatchEventOptions {
    pub fn kind(mut self, kind: DomEventKind) -> Self {
        self.kind = kind;
        self
    }
    pub fn init(mut self, init: Value) -> Self {
        self.init = init;
        self
    }
    fn flag(mut self, name: &str, value: bool) -> Self {
        if let Some(init) = self.init.as_object_mut() {
            init.insert(name.into(), value.into());
        }
        self
    }
    pub fn bubbles(self, value: bool) -> Self {
        self.flag("bubbles", value)
    }
    pub fn cancelable(self, value: bool) -> Self {
        self.flag("cancelable", value)
    }
    pub fn composed(self, value: bool) -> Self {
        self.flag("composed", value)
    }
}
