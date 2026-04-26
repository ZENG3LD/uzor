//! Toast variant catalog. Layout (rect) is layout-layer concern.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToastType {
    pub severity: ToastSeverity,
    pub message: String,
    /// How long (ms) the toast stays before fading out.
    pub duration_ms: u32,
}

impl WidgetCapabilities for ToastType {
    fn sense(&self) -> Sense {
        Sense::HOVER
    }
}

impl ToastType {
    pub fn info(message: impl Into<String>)    -> Self { Self { severity: ToastSeverity::Info,    message: message.into(), duration_ms: 3000 } }
    pub fn success(message: impl Into<String>) -> Self { Self { severity: ToastSeverity::Success, message: message.into(), duration_ms: 3000 } }
    pub fn warning(message: impl Into<String>) -> Self { Self { severity: ToastSeverity::Warning, message: message.into(), duration_ms: 4000 } }
    pub fn error(message: impl Into<String>)   -> Self { Self { severity: ToastSeverity::Error,   message: message.into(), duration_ms: 5000 } }
}
