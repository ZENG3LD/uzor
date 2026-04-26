//! Window + surface + renderer creation.
//!
//! Lifted from `mylittlechart/crates/chart-app-vello/src/main.rs` lines 1135-1287.

use std::sync::Arc;

use vello::{
    util::{RenderContext, RenderSurface},
    AaSupport, Renderer, RendererOptions, Scene,
};
use vello::wgpu::PresentMode;
use winit::window::Window;

use super::{
    config::WindowConfig,
    state::WindowState,
};
use crate::screenshot::add_copy_src_to_target_texture;

/// Error variants for window creation.
#[derive(Debug)]
pub enum WindowCreateError {
    /// winit failed to create the OS window.
    WindowCreate(winit::error::OsError),
    /// Vello surface creation failed.
    SurfaceCreate(String),
    /// Vello renderer creation failed.
    RendererCreate(String),
    /// No wgpu adapter was available after surface creation.
    NoAdapter,
}

impl std::fmt::Display for WindowCreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WindowCreate(e) => write!(f, "window create: {e}"),
            Self::SurfaceCreate(e) => write!(f, "surface create: {e}"),
            Self::RendererCreate(e) => write!(f, "renderer create: {e}"),
            Self::NoAdapter => write!(f, "no wgpu adapter available"),
        }
    }
}

impl std::error::Error for WindowCreateError {}

impl From<winit::error::OsError> for WindowCreateError {
    fn from(e: winit::error::OsError) -> Self {
        Self::WindowCreate(e)
    }
}

/// Create a winit window, vello surface, and vello renderer from `config`.
///
/// `app_init` is called after the window, surface, and renderer are ready so
/// business code can inspect GPU capabilities before constructing its payload.
/// Returns a fully initialised [`WindowState<A>`].
///
/// # Errors
/// Returns [`WindowCreateError`] if any OS, surface, or GPU step fails.
pub fn create_window<A, F>(
    event_loop: &winit::event_loop::ActiveEventLoop,
    render_cx: &mut RenderContext,
    config: WindowConfig,
    app_init: F,
) -> Result<WindowState<A>, WindowCreateError>
where
    F: FnOnce(&Arc<Window>, &RenderSurface<'static>, &wgpu::AdapterInfo) -> A,
{
    // ---- Build WindowAttributes from config ---------------------------------
    let mut attrs = Window::default_attributes()
        .with_title(&config.title)
        .with_inner_size(winit::dpi::LogicalSize::new(
            config.initial_size.0,
            config.initial_size.1,
        ))
        .with_decorations(config.decorations)
        .with_visible(config.start_visible);

    if let Some((w, h)) = config.min_size {
        attrs = attrs.with_min_inner_size(winit::dpi::LogicalSize::new(w, h));
    }

    if let Some(icon) = config.icon {
        attrs = attrs.with_window_icon(Some(icon));
    }

    // Windows: enable drop-shadow even on undecorated windows.
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_undecorated_shadow(true);
    }

    // Apply geometry in priority order: restore > initial_position > cascade.
    if let Some(geom) = config.restore_geom {
        attrs = attrs.with_position(winit::dpi::Position::Physical(
            winit::dpi::PhysicalPosition::new(geom.x, geom.y),
        ));
        attrs = attrs.with_inner_size(winit::dpi::PhysicalSize::new(geom.width, geom.height));
    } else if let Some((px, py)) = config.initial_position {
        attrs = attrs.with_position(winit::dpi::Position::Physical(
            winit::dpi::PhysicalPosition::new(px, py),
        ));
    }
    // cascade_from is applied later once we have access to existing windows;
    // callers resolve it before calling create_window or set initial_position.

    // ---- Create winit window -----------------------------------------------
    let window = Arc::new(event_loop.create_window(attrs)?);

    let size = window.inner_size();

    // ---- Create vello surface ----------------------------------------------
    let mut surface = pollster::block_on(render_cx.create_surface(
        window.clone(),
        size.width,
        size.height,
        PresentMode::AutoNoVsync,
    ))
    .map_err(|e| WindowCreateError::SurfaceCreate(e.to_string()))?;

    // ---- Resolve backend ---------------------------------------------------
    let dev_id = surface.dev_id;
    let adapter_info = render_cx.devices[dev_id].adapter().get_info();
    let backend = config
        .backend_hint
        .unwrap_or_else(|| uzor_autodetect::detect_backend(&adapter_info));

    // ---- Patch COPY_SRC on target texture (needed for screenshot readback) --
    let device = &render_cx.devices[dev_id].device;
    add_copy_src_to_target_texture(&mut surface, device);

    // ---- Create vello renderer ---------------------------------------------
    let renderer = Renderer::new(
        device,
        RendererOptions {
            use_cpu: false,
            antialiasing_support: AaSupport::all(),
            num_init_threads: std::num::NonZeroUsize::new(1),
            pipeline_cache: None,
        },
    )
    .map_err(|e| WindowCreateError::RendererCreate(e.to_string()))?;

    // ---- Win32 HWND cache --------------------------------------------------
    #[cfg(target_os = "windows")]
    let hwnd = {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        window
            .window_handle()
            .ok()
            .and_then(|handle| {
                if let RawWindowHandle::Win32(h) = handle.as_ref() {
                    Some(h.hwnd.get())
                } else {
                    None
                }
            })
    };

    // ---- App-specific payload (called after GPU is ready) ------------------
    let app = app_init(&window, &surface, &adapter_info);

    Ok(WindowState {
        window,
        surface,
        renderer,
        backend,
        scene: Scene::new(),
        gpu_scene: Scene::new(),
        last_mouse_pos: (0.0, 0.0),
        mouse_pressed: false,
        drag_start_pos: None,
        last_drag_pos: None,
        last_click: None,
        modifiers: winit::keyboard::ModifiersState::empty(),
        close_requested: false,
        visible_set: false,
        was_minimized: false,
        screenshot_pending: false,
        #[cfg(target_os = "windows")]
        hwnd,
        app,
    })
}
