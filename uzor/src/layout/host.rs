//! `WindowHost` — capabilities the L3 `LayoutManager` asks the host
//! (window manager / runtime) to perform.
//!
//! Implemented by L4 runtimes (uzor-desktop, uzor-web, uzor-mobile).
//! L3 calls this trait rather than touching winit / platform APIs directly.

use crate::platform::types::ResizeDirection;
use crate::framework::multi_window::WindowSpec;

/// Capabilities a window manager must provide to the L3 layout layer.
///
/// Default implementations are no-ops so that stub / test hosts can
/// implement only the methods they care about.
pub trait WindowHost {
    /// Begin an OS-level window drag from the current cursor position.
    fn drag_window(&mut self) {}

    /// Begin an OS-level resize drag in the given direction.
    fn drag_resize_window(&mut self, _dir: ResizeDirection) {}

    /// Set the window minimized state.
    fn set_minimized(&mut self, _on: bool) {}

    /// Set the window maximized state.
    fn set_maximized(&mut self, _on: bool) {}

    /// Query whether the window is currently maximized.
    fn is_maximized(&self) -> bool { false }

    /// Request that this window be closed.
    fn close_window(&mut self) {}

    /// Request that the entire application be closed (all windows).
    fn close_app(&mut self) {}

    /// Request that the runtime spawn a new window from a `WindowSpec`.
    fn request_spawn_window(&mut self, _spec: WindowSpec) {}

    /// Ask the OS to schedule a redraw for this window.
    fn request_redraw(&mut self) {}
}
