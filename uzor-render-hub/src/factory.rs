//! Per-window persistent render state.
//!
//! [`WindowRenderState`] is a **flat struct** that holds all initialized
//! renderer slots plus a [`SurfaceMode`] (GPU swapchain or CPU software
//! buffer).  The active backend is a runtime field — switching backends is
//! done by updating `state.active` through [`RenderHub::set_active`] rather
//! than by recreating the entire state.
//!
//! # Design rationale
//!
//! The old `WindowRenderState` was a 5-variant enum where each variant owned
//! exactly one renderer.  The flat struct replaces this with five `Option<>`
//! slots so multiple renderers can be initialized at the same time, enabling
//! live backend switching without a re-init cycle.
//!
//! ## Send + Sync story
//!
//! `WindowRenderState` is `Send` because:
//! - All wgpu types (`RenderSurface`, devices, queues) are `Send`.
//! - `vello::Renderer`, `vello_hybrid::Renderer`, `InstancedRenderer` are
//!   `Send`.
//! - CPU contexts (`VelloCpuRenderContext`, `TinySkiaCpuRenderContext`) are
//!   `Send`.
//!
//! `WindowRenderState` is **not** `Sync` — it contains interior-mutable GPU
//! state that is not safe to share across threads.  This is fine; the runtime
//! always drives one window from one thread.

use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
use uzor_render_vello_cpu::VelloCpuRenderContext;
use uzor_render_vello_gpu::VelloGpuRenderContext;
use uzor_render_vello_hybrid::VelloHybridRenderContext;
use uzor_render_wgpu_instanced::{InstancedRenderContext, InstancedRenderer};
use uzor_window_hub::lifecycle::SoftwarePresenter;
use vello::util::{RenderContext as VelloRenderContext, RenderSurface};
use vello::{Renderer as VelloRenderer, Scene};

use crate::backend::RenderBackend;

/// Local alias for vello's GPU device pool.
///
/// vello calls this type `RenderContext`, but that name collides with our
/// public `uzor::render::RenderContext` (the widget draw trait).
pub type GpuDevicePool = VelloRenderContext;

// ── SurfaceMode ───────────────────────────────────────────────────────────────

/// How the window surface is presented.
///
/// All backends that have a GPU adapter available share the same wgpu swapchain
/// — CPU backends upload their pixels via `queue.write_texture` into the same
/// `target_texture`, then the blitter copies to the swapchain (mlc pattern).
///
/// On machines with no GPU the `Software` variant is used instead: only
/// `TinySkia` and `VelloCpu` can run in this mode.  The `Software` variant is
/// not available on `wasm32` (softbuffer is desktop-only).
pub enum SurfaceMode {
    /// wgpu swapchain — used by all backends when a GPU adapter is available.
    ///
    /// CPU backends (VelloCpu, TinySkia) write their pixels via
    /// `queue.write_texture` into `surface.target_texture`, then the blitter
    /// copies the target texture to the swapchain (mlc gpu_submit.rs pattern,
    /// lines 235–258).
    Gpu {
        /// wgpu instance + device pool.
        gpu_pool: GpuDevicePool,
        /// wgpu swapchain bound to the OS window.  `'static` because the window
        /// is kept alive by `WinitWindowProvider` for the entire runtime duration.
        surface: RenderSurface<'static>,
        /// Device index into `gpu_pool.devices`.
        dev_id: usize,
    },
    /// softbuffer software surface — only valid when no GPU is present.
    ///
    /// Only `TinySkia` and `VelloCpu` can render in this mode.
    /// The `presenter` is called once per frame to push the CPU-rasterized
    /// pixel buffer to the OS window.
    #[cfg(not(target_arch = "wasm32"))]
    Software {
        /// Opaque OS window presenter (softbuffer on desktop).
        presenter: Box<dyn SoftwarePresenter>,
        /// Physical width of the software buffer in pixels.
        width: u32,
        /// Physical height of the software buffer in pixels.
        height: u32,
    },
}

// ── Per-frame context — caller-built, hub-consumed ────────────────────────────

/// Backend-specific render context the caller fills each frame.
///
/// `VelloGpu` borrows the vello scene; the others are owned.
pub enum BackendContext<'a> {
    /// GPU-backed vello scene context.
    VelloGpu(VelloGpuRenderContext<'a>),
    /// vello hybrid context.
    VelloHybrid(VelloHybridRenderContext),
    /// Wgpu instanced draw context.
    Instanced(InstancedRenderContext),
    /// vello CPU context.
    VelloCpu(VelloCpuRenderContext),
    /// tiny-skia CPU context.
    TinySkia(TinySkiaCpuRenderContext),
}

impl<'a> BackendContext<'a> {
    /// Build a `VelloGpu` context from a mutable scene reference.
    pub fn vello_gpu(scene: &'a mut Scene, offset_x: f64, offset_y: f64) -> Self {
        Self::VelloGpu(VelloGpuRenderContext::new(scene, offset_x, offset_y))
    }
    /// Build a `VelloHybrid` context.
    pub fn vello_hybrid(dpr: f64) -> Self {
        Self::VelloHybrid(VelloHybridRenderContext::new(dpr))
    }
    /// Build an `Instanced` context.
    pub fn instanced(screen_w: f32, screen_h: f32, offset_x: f32, offset_y: f32) -> Self {
        Self::Instanced(InstancedRenderContext::new(screen_w, screen_h, offset_x, offset_y))
    }
    /// Build a `VelloCpu` context.
    pub fn vello_cpu(dpr: f64) -> Self {
        Self::VelloCpu(VelloCpuRenderContext::new(dpr))
    }
    /// Build a `TinySkia` context with its own pixel buffer.
    pub fn tiny_skia(width: u32, height: u32, dpr: f64) -> Self {
        Self::TinySkia(TinySkiaCpuRenderContext::new(width, height, dpr))
    }
}

// ── WindowRenderState ─────────────────────────────────────────────────────────

/// Flat per-window render state that holds **all** initialized renderer slots.
///
/// At most one renderer per slot is initialized at any time; slots that are not
/// relevant to the current backend remain `None`.
///
/// # Frame lifecycle
///
/// 1. Call [`begin_frame`](Self::begin_frame) once at the start of each frame.
/// 2. Fill the scene / CPU buffer via the backend-specific accessor.
/// 3. Call [`crate::submit_frame`] to present.
///
/// # Live backend switching
///
/// Update `state.active` (via [`RenderHub::set_active`]) before the next frame.
/// The renderer for the new backend must already be initialized (`Some`); if it
/// is not, [`crate::submit_frame`] will emit a warning and skip the frame.
pub struct WindowRenderState {
    // ── Surface ───────────────────────────────────────────────────────────────
    /// How the frame is presented to the OS window.
    pub(crate) surface: SurfaceMode,

    // ── GPU renderer slots ────────────────────────────────────────────────────
    /// vello GPU renderer (initialized when `VelloGpu` is in the pool).
    pub(crate) vello_gpu_renderer: Option<VelloRenderer>,
    /// vello hybrid renderer (lazy-init on first submit; needs texture format).
    pub(crate) vello_hybrid_renderer: Option<vello_hybrid::Renderer>,
    /// Wgpu instanced renderer (lazy-init on first submit; needs texture format).
    pub(crate) instanced_renderer: Option<InstancedRenderer>,

    // ── CPU renderer slots ────────────────────────────────────────────────────
    /// vello CPU render context.
    pub(crate) vello_cpu_ctx: Option<VelloCpuRenderContext>,
    /// tiny-skia CPU render context.
    pub(crate) tiny_skia_ctx: Option<TinySkiaCpuRenderContext>,

    // ── Shared vello scene ────────────────────────────────────────────────────
    /// Per-frame vello scene, reset each frame.  Shared by GPU and Hybrid.
    pub(crate) scene: Scene,

    // ── VelloHybrid per-frame context ─────────────────────────────────────────
    /// vello-hybrid per-frame render context (rebuilt each frame).
    pub(crate) vello_hybrid_ctx: VelloHybridRenderContext,

    // ── Active backend ────────────────────────────────────────────────────────
    /// Currently active backend (set by `RenderHub::set_active`).
    pub(crate) active: RenderBackend,
}

impl WindowRenderState {
    // ── Constructors ──────────────────────────────────────────────────────────

    /// Build a GPU-mode state with vello GPU renderer initialized.
    pub fn new_gpu(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        renderer: VelloRenderer,
        dev_id: usize,
    ) -> Self {
        Self {
            surface: SurfaceMode::Gpu { gpu_pool, surface, dev_id },
            vello_gpu_renderer: Some(renderer),
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::VelloGpu,
        }
    }

    /// Build a GPU-mode state without a vello GPU renderer.
    ///
    /// Used for `VelloHybrid` and `WgpuInstanced` where the renderer is
    /// lazy-initialized on the first submit.
    pub fn new_gpu_no_vello(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
        active: RenderBackend,
        dpr: f64,
    ) -> Self {
        Self {
            surface: SurfaceMode::Gpu { gpu_pool, surface, dev_id },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(dpr),
            active,
        }
    }

    /// Build a CPU-only (tiny-skia) state with a software presenter.
    ///
    /// `presenter` is the [`SoftwarePresenter`] obtained from
    /// [`WindowProvider::create_software_presenter`](uzor_window_hub::lifecycle::WindowProvider::create_software_presenter).
    /// It is called once per frame to blit the CPU-rasterized pixels to the OS window.
    pub fn new_cpu(width: u32, height: u32, presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            surface: SurfaceMode::Software { presenter, width, height },
            #[cfg(target_arch = "wasm32")]
            // On wasm32 the Software variant is disabled; CPU rendering would
            // go through a canvas path that is not implemented yet.  The GPU
            // path is preferred on wasm anyway.
            surface: {
                // This constructor should not be reached on wasm32 in practice.
                compile_error!(
                    "WindowRenderState::new_cpu is not supported on wasm32 — \
                     use a GPU or canvas path instead"
                );
            },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: Some(TinySkiaCpuRenderContext::new(width, height, 1.0)),
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::TinySkia,
        }
    }

    /// Build a CPU-only (vello-cpu) state with a software presenter.
    ///
    /// `presenter` is the [`SoftwarePresenter`] obtained from
    /// [`WindowProvider::create_software_presenter`](uzor_window_hub::lifecycle::WindowProvider::create_software_presenter).
    /// It is called once per frame to blit the CPU-rasterized pixels to the OS window.
    pub fn new_vello_cpu(dpr: f64, presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            surface: SurfaceMode::Software { presenter, width: 0, height: 0 },
            #[cfg(target_arch = "wasm32")]
            surface: {
                compile_error!(
                    "WindowRenderState::new_vello_cpu is not supported on wasm32"
                );
            },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            vello_cpu_ctx: Some(VelloCpuRenderContext::new(dpr)),
            tiny_skia_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(dpr),
            active: RenderBackend::VelloCpu,
        }
    }

    /// Build a GPU-mode state for vello-hybrid (renderer lazy-init).
    pub fn new_vello_hybrid(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
        dpr: f64,
    ) -> Self {
        Self::new_gpu_no_vello(gpu_pool, surface, dev_id, RenderBackend::VelloHybrid, dpr)
    }

    /// Build a GPU-mode state for wgpu-instanced (renderer lazy-init).
    pub fn new_wgpu_instanced(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
    ) -> Self {
        Self::new_gpu_no_vello(gpu_pool, surface, dev_id, RenderBackend::InstancedWgpu, 1.0)
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// The active [`RenderBackend`] for this window.
    pub fn backend(&self) -> RenderBackend {
        self.active
    }

    /// Set the active backend (live switching).
    ///
    /// The caller is responsible for ensuring the corresponding renderer slot
    /// is initialized before the next frame.
    pub fn set_active(&mut self, backend: RenderBackend) {
        self.active = backend;
    }

    /// Mutable reference to the vello `Scene` (used by VelloGpu / VelloHybrid).
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        match self.active {
            RenderBackend::VelloGpu | RenderBackend::VelloHybrid => Some(&mut self.scene),
            _ => None,
        }
    }

    /// Shared reference to the vello `Scene`.
    pub fn scene(&self) -> Option<&Scene> {
        match self.active {
            RenderBackend::VelloGpu | RenderBackend::VelloHybrid => Some(&self.scene),
            _ => None,
        }
    }

    /// Mutable reference to the tiny-skia CPU context.
    pub fn cpu_ctx_mut(&mut self) -> Option<&mut TinySkiaCpuRenderContext> {
        self.tiny_skia_ctx.as_mut()
    }

    /// Shared reference to the tiny-skia CPU context.
    pub fn cpu_ctx(&self) -> Option<&TinySkiaCpuRenderContext> {
        self.tiny_skia_ctx.as_ref()
    }

    /// Mutable reference to the vello-cpu context.
    pub fn vello_cpu_ctx_mut(&mut self) -> Option<&mut VelloCpuRenderContext> {
        self.vello_cpu_ctx.as_mut()
    }

    /// Shared reference to the vello-cpu context.
    pub fn vello_cpu_ctx(&self) -> Option<&VelloCpuRenderContext> {
        self.vello_cpu_ctx.as_ref()
    }

    /// Mutable reference to the vello-hybrid per-frame context.
    pub fn vello_hybrid_ctx_mut(&mut self) -> Option<&mut VelloHybridRenderContext> {
        if matches!(self.active, RenderBackend::VelloHybrid) {
            Some(&mut self.vello_hybrid_ctx)
        } else {
            None
        }
    }

    // ── Frame lifecycle ───────────────────────────────────────────────────────

    /// Reset per-frame artifacts.  Call at the top of each frame.
    pub fn begin_frame(&mut self) {
        match self.active {
            RenderBackend::VelloGpu => self.scene.reset(),
            RenderBackend::VelloHybrid => {
                // vello_hybrid_ctx is rebuilt by the caller filling it each frame.
            }
            RenderBackend::VelloCpu
            | RenderBackend::TinySkia
            | RenderBackend::InstancedWgpu => {
                // CPU pixel buffers and instanced commands are rebuilt by caller.
            }
        }
    }
}
