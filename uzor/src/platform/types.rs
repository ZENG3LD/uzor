//! Platform-specific types

use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WindowId(usize);

impl WindowId {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Window not found")]
    WindowNotFound,
    #[error("Failed to create window: {0}")]
    CreationFailed(String),
    #[error("Platform operation not supported")]
    NotSupported,
    #[error("System error: {0}")]
    SystemError(String),
}

pub trait RenderSurface: Send + Sync {
    fn size(&self) -> (u32, u32);
}

pub trait SystemIntegration {
    fn get_clipboard(&self) -> Option<String>;
    fn set_clipboard(&self, text: &str);
    fn get_system_theme(&self) -> Option<crate::input::core::SystemTheme>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    Continue,
    Redraw,
    Exit,
}
