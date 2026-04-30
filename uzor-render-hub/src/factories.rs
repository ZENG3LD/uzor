//! Concrete [`RenderSurfaceFactory`] implementations for each backend.
//!
//! These factories live here (in the hub) rather than in the individual backend
//! crates to avoid circular dependencies: each backend crate is a dependency of
//! `uzor-render-hub`, so the backends cannot also depend on `uzor-render-hub`.
//!
//! # Available factories
//!
//! | Factory | Backend | Status |
//! |---------|---------|--------|
//! | [`VelloGpuSurfaceFactory`] | `VelloGpu` | Functional — creates Renderer; RenderSurface dropped (see note) |
//! | [`TinySkiaSurfaceFactory`] | `TinySkia` | Functional — CPU path, creates GPU Renderer for blit pipeline |
//! | [`VelloCpuSurfaceFactory`] | `VelloCpu` | Stubbed — returns `UnsupportedBackend` |
//! | [`VelloHybridSurfaceFactory`] | `VelloHybrid` | Stubbed — returns `UnsupportedBackend` |
//!
//! # RenderSurface gap
//!
//! [`uzor_render_hub::WindowRenderState`] stores a `vello::Renderer` but not a
//! `vello::util::RenderSurface` (the wgpu swapchain handle). The GPU factories
//! here create the surface during init to obtain a device, then drop it. Full
//! GPU frame submission requires the `Runtime` to carry the `RenderSurface`
//! alongside `WindowRenderState`.

use std::sync::Mutex;

use vello::{util::RenderContext, AaSupport, Renderer, RendererOptions};
use vello::wgpu::PresentMode;
use winit::raw_window_handle::{RawWindowHandle, RawDisplayHandle};

use uzor_window_hub::lifecycle::RawHandle;
use uzor_window_hub::winit_provider::SendSyncHandlePair;

use crate::{RenderBackend, RenderSurfaceFactory, SurfaceError, SurfaceSize, WindowRenderState};

// ─── Internal surface target helper ──────────────────────────────────────────

/// Minimal `HasWindowHandle + HasDisplayHandle` wrapper around raw handles.
///
/// Allows calling `vello::util::RenderContext::create_surface` from a copied
/// `(RawWindowHandle, RawDisplayHandle)` pair without holding a live
/// `Arc<Window>`.
///
/// # Safety
///
/// The caller must guarantee that the underlying OS window outlives every
/// `wgpu::Surface` created from this target.
struct WinitSurfaceTarget {
    window: RawWindowHandle,
    display: RawDisplayHandle,
}

// SAFETY: raw handles are plain integer/pointer values. The underlying OS
// window and display are guaranteed to outlive the factory call (the Arc<Window>
// in WinitWindowProvider keeps them alive). No thread-local state is accessed
// during wgpu surface creation on desktop platforms (Win32, X11, Wayland).
unsafe impl Send for WinitSurfaceTarget {}
unsafe impl Sync for WinitSurfaceTarget {}

impl winit::raw_window_handle::HasWindowHandle for WinitSurfaceTarget {
    fn window_handle(
        &self,
    ) -> Result<winit::raw_window_handle::WindowHandle<'_>, winit::raw_window_handle::HandleError>
    {
        // SAFETY: caller guarantees the underlying window is still alive.
        Ok(unsafe { winit::raw_window_handle::WindowHandle::borrow_raw(self.window) })
    }
}

impl winit::raw_window_handle::HasDisplayHandle for WinitSurfaceTarget {
    fn display_handle(
        &self,
    ) -> Result<winit::raw_window_handle::DisplayHandle<'_>, winit::raw_window_handle::HandleError>
    {
        // SAFETY: caller guarantees the underlying display is still alive.
        Ok(unsafe { winit::raw_window_handle::DisplayHandle::borrow_raw(self.display) })
    }
}

// ─── VelloGpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloGpu`] path.
///
/// Owns a `vello::util::RenderContext` (wgpu instance + device state).
/// On `create_render_state` it initialises a wgpu surface, picks an adapter,
/// creates a `vello::Renderer`, and returns a ready [`WindowRenderState`].
///
/// The `RenderSurface` produced during init is dropped after `Renderer` creation.
/// Full GPU frame submission requires the calling `Runtime` to hold the
/// `RenderSurface` alongside the `WindowRenderState` — see module-level docs.
///
/// # Thread safety
///
/// Interior mutability via `Mutex<RenderContext>` satisfies the `&self` constraint
/// of [`RenderSurfaceFactory`] while still mutating the device list on first use.
pub struct VelloGpuSurfaceFactory {
    render_cx: Mutex<RenderContext>,
}

impl VelloGpuSurfaceFactory {
    /// Create a factory with a freshly initialised wgpu `RenderContext`.
    pub fn new() -> Self {
        Self {
            render_cx: Mutex::new(RenderContext::new()),
        }
    }
}

impl Default for VelloGpuSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: RenderContext is Send. Mutex provides exclusive access.
unsafe impl Send for VelloGpuSurfaceFactory {}
unsafe impl Sync for VelloGpuSurfaceFactory {}

impl RenderSurfaceFactory for VelloGpuSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloGpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let RawHandle::RawWindowHandle(any) = handle else {
            return Err(SurfaceError::HandleMismatch(backend));
        };

        let pair = any
            .downcast_ref::<SendSyncHandlePair>()
            .ok_or_else(|| {
                SurfaceError::InitFailed(
                    "expected SendSyncHandlePair inside RawHandle — \
                     use WinitWindowProvider to obtain the handle"
                        .into(),
                )
            })?;

        let target = WinitSurfaceTarget {
            window: pair.0,
            display: pair.1,
        };

        let mut cx = self
            .render_cx
            .lock()
            .map_err(|_| SurfaceError::InitFailed("RenderContext mutex poisoned".into()))?;

        // Create a wgpu surface to select the adapter and device.
        // SAFETY: the window referenced by `target` is owned by the Arc<Window>
        // inside WinitWindowProvider and outlives this surface for the duration
        // the Runtime runs.
        let surface = pollster::block_on(cx.create_surface(
            target,
            size.width,
            size.height,
            PresentMode::AutoNoVsync,
        ))
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;

        let dev_id = surface.dev_id;
        let device = &cx.devices[dev_id].device;

        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::all(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;

        // Drop the surface here. Runtime must be refactored to carry
        // RenderSurface alongside WindowRenderState for GPU presentation.
        drop(surface);

        Ok(WindowRenderState::new(backend, renderer))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloGpu)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── TinySkiaSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::TinySkia`] CPU software path.
///
/// Creates a vello `Renderer` with `use_cpu = true` so the GPU blit pipeline
/// is available for presenting the CPU pixel buffer to the swapchain.
///
/// Once `uzor-render-hub` gains a CPU-only `WindowRenderState` constructor
/// (backed by `softbuffer` or similar), this factory can drop the GPU dependency.
pub struct TinySkiaSurfaceFactory {
    render_cx: Mutex<RenderContext>,
}

impl TinySkiaSurfaceFactory {
    /// Create a factory with a freshly initialised wgpu `RenderContext`.
    pub fn new() -> Self {
        Self {
            render_cx: Mutex::new(RenderContext::new()),
        }
    }
}

impl Default for TinySkiaSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: RenderContext is Send. Mutex provides exclusive access.
unsafe impl Send for TinySkiaSurfaceFactory {}
unsafe impl Sync for TinySkiaSurfaceFactory {}

impl RenderSurfaceFactory for TinySkiaSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::TinySkia) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let RawHandle::RawWindowHandle(any) = handle else {
            return Err(SurfaceError::HandleMismatch(backend));
        };

        let pair = any
            .downcast_ref::<SendSyncHandlePair>()
            .ok_or_else(|| {
                SurfaceError::InitFailed(
                    "expected SendSyncHandlePair inside RawHandle — \
                     use WinitWindowProvider to obtain the handle"
                        .into(),
                )
            })?;

        let target = WinitSurfaceTarget {
            window: pair.0,
            display: pair.1,
        };

        let mut cx = self
            .render_cx
            .lock()
            .map_err(|_| SurfaceError::InitFailed("RenderContext mutex poisoned".into()))?;

        // Create surface to select adapter; it will be dropped after renderer init.
        let surface = pollster::block_on(cx.create_surface(
            target,
            size.width,
            size.height,
            PresentMode::AutoNoVsync,
        ))
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;

        let dev_id = surface.dev_id;
        let device = &cx.devices[dev_id].device;

        // use_cpu = true so the blit pipeline uses the software path.
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: true,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;

        drop(surface);

        Ok(WindowRenderState::new(backend, renderer))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::TinySkia)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── VelloCpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloCpu`] path.
///
/// # Status: stubbed
///
/// `vello_cpu` renders entirely on the CPU without wgpu, but
/// `WindowRenderState::new` requires a `vello::Renderer` (GPU object).
/// Add `WindowRenderState::new_cpu_only()` in `uzor-render-hub` to unblock.
pub struct VelloCpuSurfaceFactory;

impl VelloCpuSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VelloCpuSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceFactory for VelloCpuSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        Err(SurfaceError::UnsupportedBackend(backend))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloCpu)
    }
}

// ─── VelloHybridSurfaceFactory ────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloHybrid`] path.
///
/// # Status: stubbed
///
/// Integrating the `vello_hybrid::Renderer` with `WindowRenderState` requires
/// a dedicated submit path in `uzor-render-hub`. Returns `UnsupportedBackend`
/// until that path is added.
pub struct VelloHybridSurfaceFactory;

impl VelloHybridSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VelloHybridSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceFactory for VelloHybridSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        Err(SurfaceError::UnsupportedBackend(backend))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloHybrid)
    }
}
