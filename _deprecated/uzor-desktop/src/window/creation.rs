//! Window + surface + renderer creation.

use std::path::PathBuf;
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
use crate::utils::screenshot::add_copy_src_to_target_texture;

/// Error variants for window creation.
#[derive(Debug)]
pub enum WindowCreateError {
    WindowCreate(winit::error::OsError),
    SurfaceCreate(String),
    RendererCreate(String),
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

/// Resolve this platform's base cache directory — the host-side half
/// of `uzor_urx_core::pipeline_cache`'s "the library never picks its
/// own directory" contract (plan rev 2 §2). Feed the result into
/// `uzor_urx_core::config::UrxConfig::pipeline_cache_dir`;
/// `pipeline_cache::cache_path_for_adapter` appends its own
/// `<app_id>/pipeline-cache/...` subpath on top, so this returns only
/// the OS base — no app-specific segment.
///
/// - Windows: `%LOCALAPPDATA%`
/// - macOS:   `$HOME/Library/Caches`
/// - other:   `$XDG_CACHE_HOME`, or `$HOME/.cache`
///
/// Returns `None` when the relevant environment variable isn't set.
/// Callers should treat that as "disk persistence unavailable" and
/// pass `None` straight through to `pipeline_cache_dir` rather than
/// guessing a fallback location — `uzor_urx_core::pipeline_cache`
/// already treats `None` as "skip disk I/O, in-memory cache only".
pub fn platform_pipeline_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
            Some(PathBuf::from(xdg))
        } else {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))
        }
    }
}

/// Create a winit window, vello surface, and vello renderer from `config`.
pub fn create_window<A, F>(
    event_loop: &winit::event_loop::ActiveEventLoop,
    render_cx: &mut RenderContext,
    config: WindowConfig,
    app_init: F,
) -> Result<WindowState<A>, WindowCreateError>
where
    F: FnOnce(&Arc<Window>, &RenderSurface<'static>, &wgpu::AdapterInfo) -> A,
{
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

    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_undecorated_shadow(true);
    }

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

    let window = Arc::new(event_loop.create_window(attrs)?);

    let size = window.inner_size();

    let mut surface = pollster::block_on(render_cx.create_surface(
        window.clone(),
        size.width,
        size.height,
        PresentMode::AutoNoVsync,
    ))
    .map_err(|e| WindowCreateError::SurfaceCreate(e.to_string()))?;

    let dev_id = surface.dev_id;
    let adapter_info = render_cx.devices[dev_id].adapter().get_info();
    // Precedence (owner decision 2026-07-24: no default flip, ever):
    // explicit `backend_hint` > `UZOR_RENDER_FAMILY` env var > `config`'s
    // own `render_family` > `RenderFamily::default()` (`Vello`). Mirrors
    // `uzor-desktop::Manager::from_built`'s own resolution exactly.
    let backend = config.backend_hint.unwrap_or_else(|| {
        let family = uzor_render_hub::resolve_render_family_from_process_env(Some(config.render_family));
        uzor_render_hub::detect_backend_for_family(&adapter_info, family)
    });

    let device = &render_cx.devices[dev_id].device;
    add_copy_src_to_target_texture(&mut surface, device);

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The real ambient environment always has a base cache dir set on
    /// every platform this crate targets (`LOCALAPPDATA` on Windows,
    /// `HOME` elsewhere) — a `None` here would mean the resolved value
    /// silently becomes "no disk persistence" for every window, which
    /// is worth catching in CI rather than only in the field.
    #[test]
    fn resolves_a_cache_dir_in_the_real_test_environment() {
        assert!(
            platform_pipeline_cache_dir().is_some(),
            "expected the ambient test environment to expose a base cache dir"
        );
    }
}
