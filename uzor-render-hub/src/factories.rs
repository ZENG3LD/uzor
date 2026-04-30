//! Concrete [`RenderSurfaceFactory`] implementations for each backend.
//!
//! These factories live here (in the hub) rather than in the individual
//! backend crates to avoid circular dependencies: each backend crate is a
//! dependency of `uzor-render-hub`, so the backends cannot also depend on
//! `uzor-render-hub`.
//!
//! # Available factories
//!
//! | Factory | Backend | Status |
//! |---------|---------|--------|
//! | [`VelloGpuSurfaceFactory`] | `VelloGpu` | Functional — `RenderSurface` now kept alive inside `WindowRenderState::Gpu` |
//! | [`TinySkiaSurfaceFactory`] | `TinySkia` | Functional — pure CPU, zero GPU init |
//! | [`VelloCpuSurfaceFactory`] | `VelloCpu` | Stubbed — returns `UnsupportedBackend` |
//! | [`VelloHybridSurfaceFactory`] | `VelloHybrid` | Stubbed — returns `UnsupportedBackend` |

use vello::{AaSupport, Renderer, RendererOptions};

use crate::factory::GpuDevicePool;
use vello::wgpu::PresentMode;
use winit::raw_window_handle::{RawWindowHandle, RawDisplayHandle};

use uzor_window_hub::lifecycle::RawHandle;
use uzor_window_hub::winit_provider::SendSyncHandlePair;

use crate::{RenderBackend, RenderSurfaceFactory, SurfaceError, SurfaceSize, WindowRenderState};

// ─── Internal surface target helper ──────────────────────────────────────────

/// Minimal `HasWindowHandle + HasDisplayHandle` wrapper around raw handles.
///
/// Allows calling `GpuDevicePool::create_surface` from a copied
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

// SAFETY: raw handles are plain integer/pointer values.  The underlying OS
// window and display are guaranteed to outlive the factory call (the
// `Arc<Window>` in `WinitWindowProvider` keeps them alive for the entire
// runtime duration).  No thread-local state is accessed during wgpu surface
// creation on desktop platforms (Win32, X11, Wayland).
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
/// On [`create_render_state`](RenderSurfaceFactory::create_render_state) it:
///
/// 1. Creates a `GpuDevicePool` (wgpu device + queue).
/// 2. Creates a `RenderSurface` bound to the OS window handle.
/// 3. Creates a vello `Renderer`.
/// 4. Moves **all three** into [`WindowRenderState::Gpu`] — the surface is
///    no longer dropped after init (this was the previous architecture gap).
///
/// The `GpuDevicePool` is consumed per call (not shared across windows).
pub struct VelloGpuSurfaceFactory;

impl VelloGpuSurfaceFactory {
    /// Create a new factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VelloGpuSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

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

        let mut gpu_pool = GpuDevicePool::new();

        // SAFETY: `target` wraps the raw handles of an `Arc<Window>` that the
        // `WinitWindowProvider` keeps alive for the entire runtime lifetime.
        // Transmuting the surface to `'static` is sound because the device pool
        // (which owns the device the surface depends on) and the surface itself are
        // both moved into the `WindowRenderState::Gpu` variant together — neither
        // can be dropped independently.
        let surface_with_lifetime = pollster::block_on(gpu_pool.create_surface(
            target,
            size.width,
            size.height,
            PresentMode::AutoNoVsync,
        ))
        .map_err(|e| SurfaceError::InitFailed(e.to_string()))?;

        let dev_id = surface_with_lifetime.dev_id;

        // SAFETY: The surface's implicit lifetime is tied to the window handle
        // passed to `create_surface`.  That window is an `Arc<Window>` owned by
        // `WinitWindowProvider`, which lives alongside (and outlives) this
        // `WindowRenderState` inside the runtime.  Erasing the lifetime to
        // `'static` is safe in this specific ownership topology.
        let surface: vello::util::RenderSurface<'static> =
            unsafe { std::mem::transmute(surface_with_lifetime) };

        let device = &gpu_pool.devices[dev_id].device;

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

        Ok(WindowRenderState::new_gpu(gpu_pool, surface, renderer, dev_id))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloGpu)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── TinySkiaSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::TinySkia`] CPU software path.
///
/// Constructs a [`WindowRenderState::Cpu`] backed by a
/// `TinySkiaCpuRenderContext`.  No wgpu dependency — zero GPU initialisation
/// cost.
pub struct TinySkiaSurfaceFactory;

impl TinySkiaSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for TinySkiaSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceFactory for TinySkiaSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::TinySkia) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        Ok(WindowRenderState::new_cpu(size.width, size.height))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::TinySkia)
    }
}

// ─── VelloCpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloCpu`] path.
///
/// # Status: stubbed
///
/// `vello_cpu` renders entirely on the CPU but requires its own renderer
/// setup that does not yet integrate with [`WindowRenderState::Cpu`].
/// Returns [`SurfaceError::UnsupportedBackend`] until that path is added.
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
/// Integrating the `vello_hybrid::Renderer` with [`WindowRenderState`]
/// requires a dedicated submit path.  Returns [`SurfaceError::UnsupportedBackend`]
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
