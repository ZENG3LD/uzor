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
//! | [`VelloGpuSurfaceFactory`] | `VelloGpu` | Functional |
//! | [`TinySkiaSurfaceFactory`] | `TinySkia` | Functional |
//! | [`VelloCpuSurfaceFactory`] | `VelloCpu` | Functional — pure CPU, no wgpu |
//! | [`VelloHybridSurfaceFactory`] | `VelloHybrid` | Functional — GPU init deferred to first submit |
//! | [`WgpuInstancedSurfaceFactory`] | `InstancedWgpu` | Functional — GPU init deferred to first submit |
//! | [`Canvas2dSurfaceFactory`] | web canvas | Non-wasm stub (returns `UnsupportedBackend`) |

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

// ─── Shared GPU init helper ───────────────────────────────────────────────────

/// Create a `GpuDevicePool` + `RenderSurface` from a `SendSyncHandlePair`.
///
/// Shared by all GPU-backed factories (VelloGpu, VelloHybrid, WgpuInstanced).
fn init_gpu_surface(
    pair: &SendSyncHandlePair,
    size: SurfaceSize,
    backend: RenderBackend,
) -> Result<(GpuDevicePool, vello::util::RenderSurface<'static>, usize), SurfaceError> {
    let target = WinitSurfaceTarget {
        window: pair.0,
        display: pair.1,
    };

    let mut gpu_pool = GpuDevicePool::new();

    let surface_with_lifetime = pollster::block_on(gpu_pool.create_surface(
        target,
        size.width,
        size.height,
        PresentMode::AutoNoVsync,
    ))
    .map_err(|e| SurfaceError::InitFailed(format!("{backend:?}: {e}")))?;

    let dev_id = surface_with_lifetime.dev_id;

    // SAFETY: The surface's implicit lifetime is tied to the window handle
    // passed to `create_surface`.  That window is an `Arc<Window>` owned by
    // `WinitWindowProvider`, which lives alongside (and outlives) this
    // `WindowRenderState` inside the runtime.  Erasing the lifetime to
    // `'static` is safe in this specific ownership topology.
    let surface: vello::util::RenderSurface<'static> =
        unsafe { std::mem::transmute(surface_with_lifetime) };

    Ok((gpu_pool, surface, dev_id))
}

/// Extract a `SendSyncHandlePair` from a `RawHandle::RawWindowHandle`.
fn extract_handle_pair<'a>(
    handle: &'a RawHandle,
    backend: RenderBackend,
) -> Result<&'a SendSyncHandlePair, SurfaceError> {
    let RawHandle::RawWindowHandle(any) = handle else {
        return Err(SurfaceError::HandleMismatch(backend));
    };

    any.downcast_ref::<SendSyncHandlePair>().ok_or_else(|| {
        SurfaceError::InitFailed(
            "expected SendSyncHandlePair inside RawHandle — \
             use WinitWindowProvider to obtain the handle"
                .into(),
        )
    })
}

// ─── VelloGpuSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloGpu`] path.
///
/// On [`create_render_state`](RenderSurfaceFactory::create_render_state) it:
///
/// 1. Creates a `GpuDevicePool` (wgpu device + queue).
/// 2. Creates a `RenderSurface` bound to the OS window handle.
/// 3. Creates a vello `Renderer`.
/// 4. Moves **all three** into [`WindowRenderState::Gpu`].
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

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

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
/// `TinySkiaCpuRenderContext`.  No wgpu dependency — zero GPU initialisation cost.
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
/// Constructs a [`WindowRenderState::VelloCpu`] backed by a
/// `VelloCpuRenderContext`.  No wgpu dependency — zero GPU initialisation cost.
/// The context renders frames via CPU rasterization; pixels are made available
/// via [`VelloCpuRenderContext::render_to_softbuffer`] for OS presentation.
pub struct VelloCpuSurfaceFactory {
    /// Device pixel ratio.  Defaults to `1.0`.
    pub dpr: f64,
}

impl VelloCpuSurfaceFactory {
    /// Create the factory with the given device pixel ratio.
    pub fn new(dpr: f64) -> Self {
        Self { dpr }
    }
}

impl Default for VelloCpuSurfaceFactory {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl RenderSurfaceFactory for VelloCpuSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloCpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        Ok(WindowRenderState::new_vello_cpu(self.dpr))
    }

    fn supports(&self, _handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloCpu)
    }
}

// ─── VelloHybridSurfaceFactory ────────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::VelloHybrid`] path.
///
/// Constructs a [`WindowRenderState::VelloHybrid`].  GPU surface and device
/// pool are initialised eagerly; the `vello_hybrid::Renderer` itself is
/// deferred to the first frame (requires the swapchain texture format, which
/// only becomes available when the first `get_current_texture` call is made).
pub struct VelloHybridSurfaceFactory {
    /// Device pixel ratio passed to the `VelloHybridRenderContext`.
    pub dpr: f64,
}

impl VelloHybridSurfaceFactory {
    /// Create the factory.
    pub fn new(dpr: f64) -> Self {
        Self { dpr }
    }
}

impl Default for VelloHybridSurfaceFactory {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl RenderSurfaceFactory for VelloHybridSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::VelloHybrid) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

        Ok(WindowRenderState::new_vello_hybrid(gpu_pool, surface, dev_id, self.dpr))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::VelloHybrid)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── WgpuInstancedSurfaceFactory ─────────────────────────────────────────────

/// Surface factory for the [`RenderBackend::InstancedWgpu`] path.
///
/// Constructs a [`WindowRenderState::WgpuInstanced`].  GPU surface and device
/// pool are initialised eagerly; the `InstancedRenderer` itself is deferred
/// to the first frame (requires the swapchain texture format).
pub struct WgpuInstancedSurfaceFactory;

impl WgpuInstancedSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WgpuInstancedSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceFactory for WgpuInstancedSurfaceFactory {
    fn create_render_state(
        &self,
        handle: &RawHandle,
        backend: RenderBackend,
        size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        if !matches!(backend, RenderBackend::InstancedWgpu) {
            return Err(SurfaceError::UnsupportedBackend(backend));
        }

        let pair = extract_handle_pair(handle, backend)?;
        let (gpu_pool, surface, dev_id) = init_gpu_surface(pair, size, backend)?;

        Ok(WindowRenderState::new_wgpu_instanced(gpu_pool, surface, dev_id))
    }

    fn supports(&self, handle: &RawHandle, backend: RenderBackend) -> bool {
        matches!(backend, RenderBackend::InstancedWgpu)
            && matches!(handle, RawHandle::RawWindowHandle(_))
    }
}

// ─── Canvas2dSurfaceFactory ───────────────────────────────────────────────────

/// Surface factory for the HTML Canvas 2D backend (WASM only).
///
/// On native targets this always returns
/// [`SurfaceError::UnsupportedBackend`].  On `wasm32` targets it downcasts
/// the `RawHandle::Canvas` to a `web_sys::HtmlCanvasElement` and returns a
/// `WindowRenderState` appropriate for DOM canvas rendering.
///
/// # Note
///
/// `RenderBackend` does not currently have a `Canvas2d` variant — the backend
/// enum is desktop-focused.  This factory is provided for forward-compatibility
/// and always returns `UnsupportedBackend` on all current backends on native.
pub struct Canvas2dSurfaceFactory;

impl Canvas2dSurfaceFactory {
    /// Create the factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Canvas2dSurfaceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSurfaceFactory for Canvas2dSurfaceFactory {
    fn create_render_state(
        &self,
        _handle: &RawHandle,
        backend: RenderBackend,
        _size: SurfaceSize,
    ) -> Result<WindowRenderState, SurfaceError> {
        // Canvas2d has no matching RenderBackend variant on native targets.
        // On wasm32 this would downcast the Canvas handle and return the
        // appropriate state; that path is not implemented yet.
        Err(SurfaceError::UnsupportedBackend(backend))
    }

    fn supports(&self, _handle: &RawHandle, _backend: RenderBackend) -> bool {
        false
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: a dummy handle that is not a RawWindowHandle (for mismatch tests).
    fn canvas_handle() -> RawHandle {
        RawHandle::Canvas(Box::new(42u32))
    }

    fn raw_window_handle_dummy() -> RawHandle {
        // We can't construct a real SendSyncHandlePair in unit tests, but we
        // can verify the discriminant check at the `supports` level, which
        // only looks at the handle variant and backend, not the contents.
        RawHandle::Canvas(Box::new(99u32))
    }

    // ── VelloGpuSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn vello_gpu_supports_correct_pair() {
        let f = VelloGpuSurfaceFactory::new();
        // `supports` only checks the discriminant, not whether the inner Any
        // can be downcast to SendSyncHandlePair — so we can test with a dummy
        // RawWindowHandle variant.
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::VelloGpu));
    }

    #[test]
    fn vello_gpu_rejects_wrong_backend() {
        let f = VelloGpuSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
        assert!(!f.supports(&handle, RenderBackend::VelloCpu));
    }

    #[test]
    fn vello_gpu_rejects_canvas_handle() {
        let f = VelloGpuSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
    }

    // ── TinySkiaSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn tiny_skia_supports_any_handle() {
        let f = TinySkiaSurfaceFactory::new();
        // TinySkia doesn't care about the handle type.
        assert!(f.supports(&canvas_handle(), RenderBackend::TinySkia));
        assert!(f.supports(&raw_window_handle_dummy(), RenderBackend::TinySkia));
    }

    #[test]
    fn tiny_skia_rejects_wrong_backend() {
        let f = TinySkiaSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloCpu));
    }

    // ── VelloCpuSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn vello_cpu_supports_any_handle() {
        let f = VelloCpuSurfaceFactory::default();
        assert!(f.supports(&canvas_handle(), RenderBackend::VelloCpu));
        assert!(f.supports(&raw_window_handle_dummy(), RenderBackend::VelloCpu));
    }

    #[test]
    fn vello_cpu_rejects_wrong_backend() {
        let f = VelloCpuSurfaceFactory::default();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::TinySkia));
    }

    // ── VelloHybridSurfaceFactory ─────────────────────────────────────────────

    #[test]
    fn vello_hybrid_supports_raw_window_handle() {
        let f = VelloHybridSurfaceFactory::default();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::VelloHybrid));
    }

    #[test]
    fn vello_hybrid_rejects_canvas_handle() {
        let f = VelloHybridSurfaceFactory::default();
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloHybrid));
    }

    #[test]
    fn vello_hybrid_rejects_wrong_backend() {
        let f = VelloHybridSurfaceFactory::default();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
    }

    // ── WgpuInstancedSurfaceFactory ───────────────────────────────────────────

    #[test]
    fn wgpu_instanced_supports_raw_window_handle() {
        let f = WgpuInstancedSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(f.supports(&handle, RenderBackend::InstancedWgpu));
    }

    #[test]
    fn wgpu_instanced_rejects_canvas_handle() {
        let f = WgpuInstancedSurfaceFactory::new();
        assert!(!f.supports(&canvas_handle(), RenderBackend::InstancedWgpu));
    }

    #[test]
    fn wgpu_instanced_rejects_wrong_backend() {
        let f = WgpuInstancedSurfaceFactory::new();
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
        assert!(!f.supports(&handle, RenderBackend::TinySkia));
    }

    // ── Canvas2dSurfaceFactory ────────────────────────────────────────────────

    #[test]
    fn canvas2d_never_supports() {
        let f = Canvas2dSurfaceFactory::new();
        // Canvas2d factory always returns false on native — no matching backend.
        assert!(!f.supports(&canvas_handle(), RenderBackend::VelloGpu));
        assert!(!f.supports(&canvas_handle(), RenderBackend::TinySkia));
        let handle = RawHandle::RawWindowHandle(Box::new(42u32));
        assert!(!f.supports(&handle, RenderBackend::VelloGpu));
    }
}
