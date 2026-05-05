//! Window-level traits owned by `LayoutManager`.
//!
//! `LayoutManager` is the root of the application — it owns every window's
//! tree, every dock, every overlay, every separator.  But it cannot poke
//! the OS by itself: the **window manager** (e.g. `uzor_desktop::Manager`,
//! a winit-driven event loop) implements these traits so the layout
//! manager can ask for redraws, drag operations, presenters, surfaces.
//!
//! The window manager calls `LayoutManager::attach_window` to register
//! each OS window as a top-level branch in the layout tree.  From that
//! point on the layout manager addresses the window through its
//! `WindowKey` and routes commands back via the `WindowProvider` trait
//! object stored in the slot.
//!
//! These traits live here, not in a separate crate, because they are
//! the contract between the layout core and any platform layer.  Old
//! callers that imported them from `uzor-window-hub` should switch to
//! `uzor::layout::window::*`.

use crate::core::types::Rect;
use crate::input::PlatformEvent;

// Re-export from platform::types so callers can `use uzor::layout::window::*`
// and pick up everything window-related in one go.
pub use crate::platform::types::{CornerStyle, RgbaIcon, ResizeDirection};

// =============================================================================
// WindowKey — stable, app-supplied tag identifying a window across sessions.
// =============================================================================

/// Stable identifier the application uses to refer to one window.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct WindowKey(pub String);

impl WindowKey {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

impl From<&str>   for WindowKey { fn from(s: &str)   -> Self { Self(s.to_string()) } }
impl From<String> for WindowKey { fn from(s: String) -> Self { Self(s) } }

// =============================================================================
// SoftwarePresenter
// =============================================================================

/// Push CPU-rasterized pixels to an OS window without a GPU.
pub trait SoftwarePresenter: Send {
    fn present(&mut self, pixels: &[u8], width: u32, height: u32);
    fn resize(&mut self, width: u32, height: u32);
}

// =============================================================================
// RawHandle
// =============================================================================

/// Platform-specific window handle, erased to avoid forcing low-level
/// window dependencies on every consumer.
pub enum RawHandle {
    RawWindowHandle(Box<dyn std::any::Any + Send + Sync>),
    Canvas(Box<dyn std::any::Any + Send + Sync>),
    CALayer(Box<dyn std::any::Any + Send + Sync>),
}

// =============================================================================
// WindowProvider
// =============================================================================

/// Capabilities one OS window must expose so `LayoutManager` can drive it.
///
/// Implemented by:
/// - `uzor_window_desktop::WinitWindowProvider` (winit / native desktop)
/// - `uzor_window_web::WebWindowProvider` (DOM canvas)
/// - `uzor_window_mobile::*` (iOS / Android)
pub trait WindowProvider {
    fn poll_events(&mut self) -> Vec<PlatformEvent>;
    fn window_rect(&self) -> Rect;
    fn scale_factor(&self) -> f64;
    fn request_redraw(&mut self);
    fn should_close(&self) -> bool;
    fn raw_window_handle(&self) -> Option<RawHandle>;

    fn drag_window(&mut self) {}
    fn drag_resize_window(&mut self, _direction: ResizeDirection) {}
    fn set_minimized(&mut self, _on: bool) {}
    fn set_maximized(&mut self, _on: bool) {}
    fn is_maximized(&self) -> bool { false }
    fn request_close(&mut self) {}
    fn set_window_icon(&mut self, _rgba: Option<RgbaIcon>) {}
    fn set_title(&mut self, _title: &str) {}
    fn set_visible(&mut self, _visible: bool) {}

    fn create_software_presenter(&self) -> Option<Box<dyn SoftwarePresenter>> {
        None
    }

    /// Push a pre-translated platform event into the provider's queue.
    ///
    /// LM's window manager (`uzor-desktop::Manager` etc) maps native
    /// events to `PlatformEvent` and forwards them through the trait.
    /// `poll_events()` drains the queue once per frame.  Default no-op
    /// for providers that don't buffer (e.g. web).
    fn push_platform_event(&mut self, _ev: PlatformEvent) {}
}

// =============================================================================
// WindowDecorations
// =============================================================================

/// Optional OS-level window decoration controls (corner rounding, border
/// accent colour, drop shadow).  Implemented by providers that can talk
/// to the system compositor (Windows DWM, macOS NSWindow, etc.).
pub trait WindowDecorations {
    fn set_corner_style(&mut self, _style: CornerStyle) {}
    fn set_border_color(&mut self, _color: Option<u32>) {}
    fn set_shadow(&mut self, _on: bool) {}
}
