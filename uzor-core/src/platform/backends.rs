//! Platform backend abstractions
//!
//! This module defines traits that backends (Desktop, Web, Mobile) 
//! must implement to integrate with the uzor core.

use super::PlatformEvent;
use super::types::{PlatformError, WindowId, SystemIntegration};
use super::{SystemTheme, WindowConfig};

/// Core platform trait that backends must implement
pub trait PlatformBackend: Send + Sync {
    /// Get unique identifier for this platform
    fn name(&self) -> &'static str;

    /// Create a new window
    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError>;

    /// Close an existing window
    fn close_window(&mut self, id: WindowId) -> Result<(), PlatformError>;

    /// Get primary window ID
    fn primary_window(&self) -> Option<WindowId>;

    /// Poll for platform events
    fn poll_events(&mut self) -> Vec<PlatformEvent>;

    /// Request a redraw of a window
    fn request_redraw(&self, id: WindowId);
}

/// Mock platform for testing and headless use
pub struct MockPlatform {
    windows: Vec<MockWindow>,
    events: Vec<PlatformEvent>,
    system_theme: SystemTheme,
}

struct MockWindow {
    id: WindowId,
    #[allow(dead_code)]
    pub config: WindowConfig,
}

impl Default for MockPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPlatform {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            events: Vec::new(),
            system_theme: SystemTheme::Light,
        }
    }

    pub fn push_event(&mut self, event: PlatformEvent) {
        self.events.push(event);
    }

    pub fn set_system_theme(&mut self, theme: SystemTheme) {
        self.system_theme = theme;
    }
}

impl PlatformBackend for MockPlatform {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError> {
        let id = WindowId::new();
        self.windows.push(MockWindow {
            id,
            config,
        });
        Ok(id)
    }

    fn close_window(&mut self, id: WindowId) -> Result<(), PlatformError> {
        self.windows.retain(|w| w.id != id);
        Ok(())
    }

    fn primary_window(&self) -> Option<WindowId> {
        self.windows.first().map(|w| w.id)
    }

    fn poll_events(&mut self) -> Vec<PlatformEvent> {
        std::mem::take(&mut self.events)
    }

    fn request_redraw(&self, _id: WindowId) {
        // No-op in mock
    }
}

impl SystemIntegration for MockPlatform {
    fn get_clipboard(&self) -> Option<String> {
        None
    }

    fn set_clipboard(&self, _text: &str) {
    }

    fn get_system_theme(&self) -> Option<SystemTheme> {
        Some(self.system_theme)
    }
}
