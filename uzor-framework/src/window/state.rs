//! Per-window framework state.
//!
//! `WindowState<A>` owns all window/GPU/input infrastructure.
//! App-specific business state lives in the generic `app: A` field.

use std::sync::Arc;

use vello::{util::RenderSurface, Renderer, Scene};
use winit::window::Window;

/// Per-window state owned by the framework.
///
/// `A` is the app-specific payload (e.g. `chart_app::ChartApp` for mlc).
/// Framework owns infrastructure; the app owns business logic.
pub struct WindowState<A> {
    // ---- Window + GPU -------------------------------------------------------
    /// The underlying winit window.
    pub window: Arc<Window>,
    /// Vello render surface (tied to the window's lifetime).
    pub surface: RenderSurface<'static>,
    /// Vello renderer instance for this window's device.
    pub renderer: Renderer,
    /// The active rendering backend for this window.
    pub backend: uzor_autodetect::RenderBackend,

    // ---- Scene double-buffer ------------------------------------------------
    /// Scene being built on the main thread for the *next* GPU frame.
    pub scene: Scene,
    /// Scene ready for the GPU thread (swapped with `scene` before submission).
    pub gpu_scene: Scene,

    // ---- Per-window input snapshot ------------------------------------------
    /// Last known cursor position in logical pixels.
    pub last_mouse_pos: (f64, f64),
    /// Whether the primary mouse button is currently held.
    pub mouse_pressed: bool,
    /// Position where the current drag started, if a drag is in progress.
    pub drag_start_pos: Option<(f64, f64)>,
    /// Last cursor position during a drag.
    pub last_drag_pos: Option<(f64, f64)>,
    /// Timestamp + position of the last click for double-click detection.
    pub last_click: Option<(std::time::Instant, f64, f64)>,
    /// Current keyboard modifier state.
    pub modifiers: winit::keyboard::ModifiersState,

    // ---- Lifecycle flags ----------------------------------------------------
    /// Set when the OS or app chrome requests window closure.
    /// Checked in `about_to_wait`; triggers save-and-exit logic.
    pub close_requested: bool,
    /// `false` until the first GPU frame has been presented.  The window is
    /// kept invisible until this becomes `true` to avoid a white-flash.
    pub visible_set: bool,
    /// `true` while the window is minimized (detected via zero-size resize).
    pub was_minimized: bool,
    /// When `true` the next render pass should capture a screenshot.
    pub screenshot_pending: bool,

    // ---- Win32 HWND cache ---------------------------------------------------
    /// Cached Win32 HWND extracted at creation time (avoids repeated FFI calls).
    #[cfg(target_os = "windows")]
    pub hwnd: Option<isize>,

    // ---- App-specific payload -----------------------------------------------
    /// Business state provided by the consuming app.
    pub app: A,
}

impl<A> WindowState<A> {
    /// The winit [`WindowId`] for this window.
    pub fn id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    /// Current surface dimensions `(width, height)` in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }
}
