//! Create a winit window + hub render surface from a [`WindowSpec`].

use std::path::PathBuf;
use std::sync::Arc;

use uzor::RgbaIcon;
use uzor_render_hub::{RenderBackend, RenderFamily, RenderHub, SurfaceSize};
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Icon, Window};

use super::mapper::WinitMapper;
use super::window::{apply_spawn_style, NativeWindow};
use super::win32;
use crate::types::error::FrameworkError;
use crate::types::ids::WindowId;
use crate::types::window::{SizePx, WindowSpec};

/// Resolve this platform's base cache directory (L-D4).
///
/// - Windows: `%LOCALAPPDATA%`
/// - macOS: `$HOME/Library/Caches`
/// - other: `$XDG_CACHE_HOME` or `$HOME/.cache`
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

/// Create the OS window and a hub render state for `spec`.
pub fn create_window(
    event_loop: &ActiveEventLoop,
    hub: &RenderHub,
    id: WindowId,
    spec: WindowSpec,
) -> Result<NativeWindow, FrameworkError> {
    let mut attrs = Window::default_attributes()
        .with_title(&spec.title)
        .with_inner_size(LogicalSize::new(
            f64::from(spec.inner_size.width),
            f64::from(spec.inner_size.height),
        ))
        .with_decorations(spec.decorations)
        .with_resizable(spec.resizable)
        .with_visible(false)
        .with_transparent(spec.transparent);

    if let Some(min) = spec.min_inner_size {
        attrs = attrs.with_min_inner_size(LogicalSize::new(
            f64::from(min.width),
            f64::from(min.height),
        ));
    }
    if let Some((x, y)) = spec.position {
        attrs = attrs.with_position(PhysicalPosition::new(x, y));
    }

    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs = attrs.with_movable_by_window_background(false);
    }

    let window = event_loop
        .create_window(attrs)
        .map_err(|e| FrameworkError::Host(format!("create_window: {e}")))?;
    let window = Arc::new(window);

    let dpr = window.scale_factor();
    let inner = window.inner_size();
    let size = SurfaceSize {
        width: inner.width.max(1),
        height: inner.height.max(1),
    };

    let raw = raw_handle_for(&window)
        .ok_or_else(|| FrameworkError::Host("no raw window handle".into()))?;

    let active = hub.active();
    let factory = hub
        .factory_for(active)
        .ok_or_else(|| FrameworkError::Host(format!("hub has no factory for {active:?}")))?;

    let mut render_state = factory
        .create_render_state(&raw, active, size)
        .map_err(|e| FrameworkError::Host(format!("create_render_state: {e}")))?;
    render_state.resize_surface(size.width, size.height);

    apply_spawn_style(&window, &spec);
    if !spec.decorations {
        win32::install_borderless_resize(&window);
    }

    if spec.visible {
        window.set_visible(true);
    }
    window.request_redraw();

    let key = spec.key.0.clone();
    Ok(NativeWindow {
        id,
        key,
        window,
        render_state,
        mapper: WinitMapper::new(dpr),
        decorations: spec.decorations,
    })
}

/// Physical inner size + DPR for a Created / Resized echo.
pub fn geometry_echo(window: &Window) -> (SizePx, f64, Option<(i32, i32)>) {
    let inner = window.inner_size();
    let dpr = window.scale_factor();
    let pos = window.outer_position().ok().map(|p| (p.x, p.y));
    (SizePx::new(inner.width, inner.height), dpr, pos)
}

fn raw_handle_for(window: &Arc<Window>) -> Option<uzor::layout::window::RawHandle> {
    use uzor::layout::window::RawHandle;
    use uzor_window_desktop::winit_provider::SendSyncHandlePair;
    use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

    let window_handle = window.window_handle().ok()?.as_raw();
    let display_handle = window.display_handle().ok()?.as_raw();
    // SAFETY: same contract as `WinitWindowProvider::raw_window_handle` —
    // the Arc<Window> outlives the surface created from these handles.
    let pair: Box<dyn std::any::Any + Send + Sync> =
        Box::new(SendSyncHandlePair(
            window_handle,
            display_handle,
            Some(window.clone()),
        ));
    Some(RawHandle::RawWindowHandle(pair))
}

/// Prefer an explicit backend, else autodetection.
pub fn build_hub(preferred: Option<RenderBackend>) -> RenderHub {
    match preferred {
        Some(b) => RenderHub::fixed(b),
        None => RenderHub::autodetect(RenderFamily::default()),
    }
}

/// Build a winit icon from an RGBA buffer, if valid.
pub fn icon_from_rgba(icon: &RgbaIcon) -> Option<Icon> {
    Icon::from_rgba(icon.pixels.clone(), icon.width, icon.height).ok()
}

