//! Mobile backend for uzor supporting iOS and Android
//!
//! This crate provides the mobile platform implementation for uzor,
//! supporting iOS and Android devices with touch-first design.
//!
//! # Architecture
//!
//! The mobile backend provides:
//! - Touch input handling (multi-touch)
//! - Virtual keyboard integration (IME)
//! - Mobile-specific features (haptics, orientation, safe areas)
//! - Native clipboard integration
//! - System theme detection
//!
//! # Platform-Specific Notes
//!
//! ## iOS
//! - Uses UIKit for native UI integration
//! - Clipboard via UIPasteboard
//! - Theme detection via UITraitCollection
//! - Safe area insets for notch/home indicator
//!
//! ## Android
//! - Uses Android NDK and JNI
//! - Clipboard via ClipboardManager
//! - Theme detection via Configuration.UI_MODE_NIGHT
//! - System bars handling
//!
//! # Feature Flags
//!
//! - `android`: Enable Android-specific functionality
//! - `ios`: Enable iOS-specific functionality
//!
//! # Example
//!
//! ```ignore
//! use uzor_mobile::MobilePlatform;
//! use uzor_core::platform::{PlatformBackend, WindowConfig};
//!
//! fn main() {
//!     let mut platform = MobilePlatform::new().unwrap();
//!
//!     let window = platform.create_window(
//!         WindowConfig::new("My Mobile App")
//!     ).unwrap();
//!
//!     // Handle touch events
//!     while let Some(event) = platform.poll_event() {
//!         match event {
//!             PlatformEvent::TouchStart { id, x, y } => {
//!                 // Handle touch start
//!             }
//!             PlatformEvent::TouchMove { id, x, y } => {
//!                 // Handle touch move
//!             }
//!             PlatformEvent::TouchEnd { id, x, y } => {
//!                 // Handle touch end
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```

pub use uzor_core;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use uzor_core::platform::{
    backends::{PlatformBackend, PlatformError, RenderSurface, WindowId},
    Clipboard, ClipboardError, CursorError, CursorIcon, CursorManagement, ImeSupport,
    PlatformCapabilities, PlatformEvent, SystemError, SystemIntegration, SystemTheme,
    WindowConfig,
};

// Platform-specific modules
#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
use android::AndroidBackend;

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "ios")]
use ios::IosBackend;

mod common;

// =============================================================================
// Mobile Platform
// =============================================================================

/// Mobile platform backend for uzor
///
/// Provides a unified interface for iOS and Android platforms.
/// Uses platform-specific implementations internally based on target OS.
pub struct MobilePlatform {
    state: Arc<Mutex<MobileState>>,
}

struct MobileState {
    /// Active window (mobile apps typically have only one)
    window: Option<MobileWindow>,

    /// Platform-specific backend
    #[cfg(target_os = "android")]
    backend: AndroidBackend,
    #[cfg(target_os = "ios")]
    backend: IosBackend,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    backend: StubBackend,

    /// Event queue
    event_queue: VecDeque<PlatformEvent>,

    /// IME state
    ime_position: (f64, f64),
    ime_allowed: bool,

    /// Cursor state (tracked for compatibility, but not displayed on mobile)
    cursor_icon: CursorIcon,
}

struct MobileWindow {
    id: WindowId,
    config: WindowConfig,
    width: u32,
    height: u32,
    scale_factor: f64,
}

impl MobilePlatform {
    /// Create a new mobile platform instance
    ///
    /// # Errors
    ///
    /// Returns an error if the platform-specific backend fails to initialize.
    pub fn new() -> Result<Self, PlatformError> {
        #[cfg(target_os = "android")]
        let backend = AndroidBackend::new()
            .map_err(|e| PlatformError::Other(format!("Android backend init failed: {}", e)))?;

        #[cfg(target_os = "ios")]
        let backend = IosBackend::new()
            .map_err(|e| PlatformError::Other(format!("iOS backend init failed: {}", e)))?;

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let backend = StubBackend::new();

        Ok(Self {
            state: Arc::new(Mutex::new(MobileState {
                window: None,
                backend,
                event_queue: VecDeque::new(),
                ime_position: (0.0, 0.0),
                ime_allowed: false,
                cursor_icon: CursorIcon::Default,
            })),
        })
    }

    /// Get safe area insets (for notch, home indicator, etc.)
    ///
    /// Returns (top, right, bottom, left) insets in physical pixels.
    pub fn safe_area_insets(&self) -> (f64, f64, f64, f64) {
        let state = self.state.lock().unwrap();
        state.backend.safe_area_insets()
    }

    /// Get current screen orientation
    pub fn orientation(&self) -> ScreenOrientation {
        let state = self.state.lock().unwrap();
        state.backend.orientation()
    }

    /// Trigger haptic feedback
    ///
    /// # Arguments
    ///
    /// * `style` - The haptic feedback style to trigger
    pub fn haptic_feedback(&mut self, style: HapticStyle) {
        let mut state = self.state.lock().unwrap();
        state.backend.haptic_feedback(style);
    }
}

impl Default for MobilePlatform {
    fn default() -> Self {
        Self::new().expect("Failed to create mobile platform")
    }
}

// =============================================================================
// PlatformBackend Implementation
// =============================================================================

impl PlatformBackend for MobilePlatform {
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities::mobile()
    }

    fn create_window(&mut self, config: WindowConfig) -> Result<WindowId, PlatformError> {
        let mut state = self.state.lock().unwrap();

        // Mobile apps typically have only one window
        if state.window.is_some() {
            return Err(PlatformError::Other(
                "Mobile platform supports only one window".to_string(),
            ));
        }

        let window_id = WindowId::PRIMARY;

        // Get screen size from backend
        let (width, height) = state.backend.screen_size();
        let scale_factor = state.backend.scale_factor();

        let window = MobileWindow {
            id: window_id,
            config: config.clone(),
            width,
            height,
            scale_factor,
        };

        state.window = Some(window);
        state.event_queue.push_back(PlatformEvent::WindowCreated);

        Ok(window_id)
    }

    fn request_redraw(&mut self, window_id: WindowId) {
        let mut state = self.state.lock().unwrap();

        if let Some(window) = &state.window {
            if window.id == window_id {
                state.event_queue.push_back(PlatformEvent::RedrawRequested);
            }
        }
    }

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        let mut state = self.state.lock().unwrap();

        // First, poll backend for any native events
        if let Some(event) = state.backend.poll_event() {
            return Some(event);
        }

        // Then, return queued events
        state.event_queue.pop_front()
    }

    fn set_window_title(&mut self, window_id: WindowId, title: &str) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();

        let window = state
            .window
            .as_mut()
            .filter(|w| w.id == window_id)
            .ok_or(PlatformError::WindowNotFound(window_id))?;

        window.config.title = title.to_string();
        state.backend.set_title(title);

        Ok(())
    }

    fn window_size(&self, window_id: WindowId) -> Result<(u32, u32), PlatformError> {
        let state = self.state.lock().unwrap();

        let window = state
            .window
            .as_ref()
            .filter(|w| w.id == window_id)
            .ok_or(PlatformError::WindowNotFound(window_id))?;

        Ok((window.width, window.height))
    }

    fn scale_factor(&self, window_id: WindowId) -> Result<f64, PlatformError> {
        let state = self.state.lock().unwrap();

        let window = state
            .window
            .as_ref()
            .filter(|w| w.id == window_id)
            .ok_or(PlatformError::WindowNotFound(window_id))?;

        Ok(window.scale_factor)
    }

    fn close_window(&mut self, window_id: WindowId) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();

        if let Some(window) = &state.window {
            if window.id == window_id {
                state.window = None;
                state.event_queue.push_back(PlatformEvent::WindowDestroyed);
                return Ok(());
            }
        }

        Err(PlatformError::WindowNotFound(window_id))
    }
}

// =============================================================================
// Clipboard Implementation
// =============================================================================

impl Clipboard for MobilePlatform {
    fn get_text(&self) -> Option<String> {
        let state = self.state.lock().unwrap();
        state.backend.get_clipboard_text()
    }

    fn set_text(&self, text: &str) -> Result<(), ClipboardError> {
        let mut state = self.state.lock().unwrap();
        state
            .backend
            .set_clipboard_text(text)
            .map_err(|e| ClipboardError::Unknown(e))
    }
}

// =============================================================================
// SystemIntegration Implementation
// =============================================================================

impl SystemIntegration for MobilePlatform {
    fn open_url(&self, url: &str) -> Result<(), SystemError> {
        let state = self.state.lock().unwrap();
        state
            .backend
            .open_url(url)
            .map_err(|e| SystemError::Failed(e))
    }

    fn get_system_theme(&self) -> Option<SystemTheme> {
        let state = self.state.lock().unwrap();
        state.backend.system_theme()
    }

    fn get_scale_factor(&self) -> f64 {
        let state = self.state.lock().unwrap();
        state.backend.scale_factor()
    }
}

// =============================================================================
// CursorManagement Implementation
// =============================================================================

impl CursorManagement for MobilePlatform {
    fn set_cursor(&mut self, cursor: CursorIcon) {
        // Mobile devices don't have cursors, but track state for compatibility
        let mut state = self.state.lock().unwrap();
        state.cursor_icon = cursor;
    }

    fn set_cursor_visible(&mut self, _visible: bool) {
        // No-op on mobile (no cursor)
    }

    fn set_cursor_locked(&mut self, _locked: bool) -> Result<(), CursorError> {
        // Not supported on mobile
        Err(CursorError::NotSupported)
    }
}

// =============================================================================
// ImeSupport Implementation
// =============================================================================

impl ImeSupport for MobilePlatform {
    fn set_ime_position(&mut self, x: f64, y: f64) {
        let mut state = self.state.lock().unwrap();
        state.ime_position = (x, y);
        state.backend.set_ime_position(x, y);
    }

    fn set_ime_allowed(&mut self, allowed: bool) {
        let mut state = self.state.lock().unwrap();
        state.ime_allowed = allowed;

        if allowed {
            state.backend.show_keyboard();
        } else {
            state.backend.hide_keyboard();
        }
    }

    fn set_ime_cursor_area(&mut self, x: f64, y: f64, _width: f64, _height: f64) {
        // Mobile platforms use simple position-based IME
        self.set_ime_position(x, y);
    }
}

// =============================================================================
// RenderSurface Implementation
// =============================================================================

impl RenderSurface for MobilePlatform {
    fn raw_window_handle(&self) -> Option<&dyn std::any::Any> {
        // Platform-specific window handles should be accessed differently on mobile
        // This would require platform-specific APIs (e.g., CAMetalLayer for iOS,
        // ANativeWindow for Android)
        None
    }

    fn surface_size(&self) -> (u32, u32) {
        let state = self.state.lock().unwrap();
        if let Some(window) = &state.window {
            (
                (window.width as f64 * window.scale_factor) as u32,
                (window.height as f64 * window.scale_factor) as u32,
            )
        } else {
            (0, 0)
        }
    }

    fn surface_scale_factor(&self) -> f64 {
        let state = self.state.lock().unwrap();
        state.window.as_ref().map(|w| w.scale_factor).unwrap_or(1.0)
    }
}

// =============================================================================
// Mobile-Specific Types
// =============================================================================

/// Screen orientation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenOrientation {
    /// Portrait (vertical)
    Portrait,
    /// Landscape (horizontal)
    Landscape,
    /// Portrait upside down
    PortraitUpsideDown,
    /// Landscape right
    LandscapeRight,
}

/// Haptic feedback style
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HapticStyle {
    /// Light impact (subtle)
    Light,
    /// Medium impact
    Medium,
    /// Heavy impact
    Heavy,
    /// Selection feedback (tick)
    Selection,
    /// Success feedback
    Success,
    /// Warning feedback
    Warning,
    /// Error feedback
    Error,
}

// =============================================================================
// Stub Backend (for non-mobile platforms during development)
// =============================================================================

#[cfg(not(any(target_os = "android", target_os = "ios")))]
struct StubBackend;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl StubBackend {
    fn new() -> Self {
        StubBackend
    }

    fn screen_size(&self) -> (u32, u32) {
        (800, 600)
    }

    fn scale_factor(&self) -> f64 {
        1.0
    }

    fn safe_area_insets(&self) -> (f64, f64, f64, f64) {
        (0.0, 0.0, 0.0, 0.0)
    }

    fn orientation(&self) -> ScreenOrientation {
        ScreenOrientation::Portrait
    }

    fn haptic_feedback(&mut self, _style: HapticStyle) {}

    fn poll_event(&mut self) -> Option<PlatformEvent> {
        None
    }

    fn set_title(&mut self, _title: &str) {}

    fn get_clipboard_text(&self) -> Option<String> {
        None
    }

    fn set_clipboard_text(&mut self, _text: &str) -> Result<(), String> {
        Err("Clipboard not available on stub backend".to_string())
    }

    fn open_url(&self, _url: &str) -> Result<(), String> {
        Err("URL opening not available on stub backend".to_string())
    }

    fn system_theme(&self) -> Option<SystemTheme> {
        Some(SystemTheme::Light)
    }

    fn set_ime_position(&mut self, _x: f64, _y: f64) {}

    fn show_keyboard(&mut self) {}

    fn hide_keyboard(&mut self) {}
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_capabilities() {
        let caps = PlatformCapabilities::mobile();

        assert!(caps.has_clipboard);
        assert!(caps.has_touch);
        assert!(!caps.has_mouse);
        assert!(caps.has_keyboard);
        assert!(!caps.has_file_drop);
        assert!(!caps.has_cursor_management);
        assert!(caps.has_ime);
        assert_eq!(caps.max_touch_points, 10);
    }

    #[test]
    fn test_haptic_style_variants() {
        let styles = vec![
            HapticStyle::Light,
            HapticStyle::Medium,
            HapticStyle::Heavy,
            HapticStyle::Selection,
            HapticStyle::Success,
            HapticStyle::Warning,
            HapticStyle::Error,
        ];

        assert_eq!(styles.len(), 7);
    }

    #[test]
    fn test_screen_orientation_variants() {
        let orientations = vec![
            ScreenOrientation::Portrait,
            ScreenOrientation::Landscape,
            ScreenOrientation::PortraitUpsideDown,
            ScreenOrientation::LandscapeRight,
        ];

        assert_eq!(orientations.len(), 4);
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[test]
    fn test_stub_backend() {
        let backend = StubBackend::new();

        assert_eq!(backend.screen_size(), (800, 600));
        assert_eq!(backend.scale_factor(), 1.0);
        assert_eq!(backend.safe_area_insets(), (0.0, 0.0, 0.0, 0.0));
        assert_eq!(backend.orientation(), ScreenOrientation::Portrait);
        assert_eq!(backend.get_clipboard_text(), None);
        assert_eq!(backend.system_theme(), Some(SystemTheme::Light));
    }
}
