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
pub struct WindowState<A> {
    // ---- Window + GPU -------------------------------------------------------
    pub window: Arc<Window>,
    pub surface: RenderSurface<'static>,
    pub renderer: Renderer,
    pub backend: uzor_render_hub::RenderBackend,

    // ---- Scene double-buffer ------------------------------------------------
    pub scene: Scene,
    pub gpu_scene: Scene,

    // ---- Per-window input snapshot ------------------------------------------
    pub last_mouse_pos: (f64, f64),
    pub mouse_pressed: bool,
    pub drag_start_pos: Option<(f64, f64)>,
    pub last_drag_pos: Option<(f64, f64)>,
    pub last_click: Option<(std::time::Instant, f64, f64)>,
    pub modifiers: winit::keyboard::ModifiersState,

    // ---- Lifecycle flags ----------------------------------------------------
    pub close_requested: bool,
    pub visible_set: bool,
    pub was_minimized: bool,
    pub screenshot_pending: bool,

    // ---- Win32 HWND cache ---------------------------------------------------
    #[cfg(target_os = "windows")]
    pub hwnd: Option<isize>,

    // ---- App-specific payload -----------------------------------------------
    pub app: A,
}

impl<A> WindowState<A> {
    pub fn id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    pub fn size(&self) -> (u32, u32) {
        (self.surface.config.width, self.surface.config.height)
    }
}
