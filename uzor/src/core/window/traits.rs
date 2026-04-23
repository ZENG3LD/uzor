//! Window management trait

use super::types::*;

/// Platform-agnostic window manager
///
/// Backends implement this to provide window control.
/// uzor core defines the contract, backends provide the implementation.
pub trait WindowManager {
    /// Get current window state
    fn state(&self) -> &WindowState;

    /// Set window title
    fn set_title(&mut self, title: &str);

    /// Resize window
    fn set_size(&mut self, size: WindowSize);

    /// Move window
    fn set_position(&mut self, position: WindowPosition);

    /// Minimize window
    fn set_minimized(&mut self, minimized: bool);

    /// Maximize/restore window
    fn set_maximized(&mut self, maximized: bool);

    /// Enter/exit fullscreen
    fn set_fullscreen(&mut self, fullscreen: bool);

    /// Show/hide window
    fn set_visible(&mut self, visible: bool);

    /// Start interactive window drag (user is dragging title bar)
    fn drag_window(&self);

    /// Start interactive resize from an edge/corner
    fn drag_resize(&self, direction: ResizeDirection);

    /// Apply chrome configuration
    fn set_chrome(&mut self, config: ChromeConfig);

    /// Apply border configuration (platform-specific, may be no-op)
    fn set_border(&mut self, config: BorderConfig);

    /// Close the window
    fn close(&mut self);
}
