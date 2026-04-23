//! Platform backend abstractions

use super::types::{PlatformError, WindowId, SystemIntegration};
use super::{PlatformEvent, SystemTheme, WindowConfig};

pub trait PlatformBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError>;
    fn close_window(&mut self, id: WindowId) -> Result<(), PlatformError>;
    fn primary_window(&self) -> Option<WindowId>;
    fn poll_events(&mut self) -> Vec<PlatformEvent>;
    fn request_redraw(&self, id: WindowId);
}

struct MockWindow {
    id: WindowId,
}

pub struct MockPlatform {
    windows: Vec<MockWindow>,
    events: Vec<PlatformEvent>,
    system_theme: SystemTheme,
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
        let _ = config;
        self.windows.push(MockWindow { id });
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

    fn request_redraw(&self, _id: WindowId) {}
}

impl SystemIntegration for MockPlatform {
    fn get_clipboard(&self) -> Option<String> {
        None
    }
    fn set_clipboard(&self, _text: &str) {}
    fn get_system_theme(&self) -> Option<SystemTheme> {
        Some(self.system_theme)
    }
}
