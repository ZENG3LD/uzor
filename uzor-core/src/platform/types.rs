//! Platform-specific types for windowing and system integration

use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique identifier for a window
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

impl WindowId {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self(0)
    }
}

/// Errors that can occur in platform backends
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

/// Abstraction over a platform-native render surface
pub trait RenderSurface: Send + Sync {
    fn size(&self) -> (u32, u32);
}

/// System-level operations (re-export or define here)
pub trait SystemIntegration {
    fn get_clipboard(&self) -> Option<String>;
    fn set_clipboard(&self, text: &str);
    fn get_system_theme(&self) -> Option<crate::platform::SystemTheme>;
}

/// Result of processing a platform event
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    /// Continue running the event loop
    Continue,
    /// Request a redraw
    Redraw,
    /// Exit the application
    Exit,
}
