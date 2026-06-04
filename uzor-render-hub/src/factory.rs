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
#[cfg(not(target_arch = "wasm32"))]
use uzor::layout::window::SoftwarePresenter;
use vello::util::{RenderContext as VelloRenderContext, RenderSurface};
use vello::{Renderer as VelloRenderer, Scene};

use crate::backend::RenderBackend;

#[cfg(target_arch = "wasm32")]
use uzor_render_canvas2d::Canvas2dRenderContext;

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
///
/// On `wasm32` the `Canvas2d` variant is used for DOM canvas rendering.
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
    /// DOM canvas surface (wasm32 only).
    ///
    /// The [`Canvas2dRenderContext`] draws directly into the HTML canvas element.
    /// No GPU swapchain or software buffer is needed — the browser presents the
    /// canvas automatically after each RAF callback.
    #[cfg(target_arch = "wasm32")]
    Canvas2d {
        /// The HTML canvas element being rendered into.
        canvas: web_sys::HtmlCanvasElement,
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
    /// Wgpu instanced per-frame draw context — walker writes `DrawCmd`s into
    /// `draw_commands`, `submit_instanced` pulls them. Lazy-init on first
    /// `with_render_context` call when `active = InstancedWgpu`. Wiring this
    /// slot lights up the InstancedWgpu path end-to-end (before this, hub
    /// returned `None` from `with_render_context` and `submit_instanced`
    /// passed `&[]` — net no-op clear-only frame).
    pub(crate) instanced_ctx: Option<InstancedRenderContext>,

    // ── CPU renderer slots ────────────────────────────────────────────────────
    /// vello CPU render context.
    pub(crate) vello_cpu_ctx: Option<VelloCpuRenderContext>,
    /// tiny-skia CPU render context.
    pub(crate) tiny_skia_ctx: Option<TinySkiaCpuRenderContext>,

    // ── URX render-family slots ──────────────────────────────────────────────
    /// Shared URX paint context — captures consumer's RenderContext calls
    /// into a `urx_core::Scene`. Used by ALL four URX backends; the choice
    /// of backend happens at submit time (the `Scene` is universal).
    pub(crate) urx_ctx: Option<uzor_render_urx::UrxRenderContext>,
    /// URX CPU backend (own scanline rasteriser). Lazy-init on first submit.
    pub(crate) urx_cpu_backend: Option<uzor_urx_cpu::CpuBackend>,
    /// URX CPU output pixmap — same role as `tiny_skia_ctx`'s buffer.
    pub(crate) urx_cpu_pixmap: Option<uzor_urx_cpu::Pixmap>,
    /// URX instanced-wgpu backend. Lazy-init on first GPU-side submit.
    /// Phase B will read this; Phase A leaves it `None`.
    #[allow(dead_code)]
    pub(crate) urx_wgpu_backend: Option<uzor_urx_wgpu::UrxWgpuBackend>,
    /// URX hybrid backend. Phase B; allocated slot only in Phase A.
    #[allow(dead_code)]
    pub(crate) urx_hybrid_backend: Option<uzor_urx_hybrid::HybridBackend>,

    // ── Canvas 2D context (wasm32 only) ───────────────────────────────────────
    /// HTML Canvas 2D render context.  Only populated when `active` is
    /// [`RenderBackend::Canvas2d`].
    #[cfg(target_arch = "wasm32")]
    pub(crate) canvas2d_ctx: Option<Canvas2dRenderContext>,

    // ── Shared vello scene ────────────────────────────────────────────────────
    /// Per-frame vello scene, reset each frame.  Shared by GPU and Hybrid.
    pub(crate) scene: Scene,

    // ── VelloHybrid per-frame context ─────────────────────────────────────────
    /// vello-hybrid per-frame render context (rebuilt each frame).
    pub(crate) vello_hybrid_ctx: VelloHybridRenderContext,

    // ── Active backend ────────────────────────────────────────────────────────
    /// Currently active 2D-scene backend (set by `RenderHub::set_active`).
    /// One of `Scene2DBackend` (renamed legacy `RenderBackend`).
    pub(crate) active: RenderBackend,
    /// Currently active URX-channel backend (set by `RenderHub::set_active_urx`).
    /// When `Some`, consumers using the `with_urx_engine` channel are routed
    /// through this backend's submit path; when `None`, the channel is idle
    /// and consumers fall back to the 2D path via `active`. Stage 1
    /// of urx-full-integration introduces this slot; Stage 3 will wire
    /// `UrxEngine` lifetime + RegionMixer behind it.
    pub(crate) active_urx: Option<uzor::UrxBackend>,
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
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            #[cfg(target_arch = "wasm32")]
            canvas2d_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::VelloGpu,
        }
    }

    /// Build a GPU-mode state with the swapchain ready but no vello
    /// renderer yet.  Used by the cold-start skeleton path: the
    /// `WindowRenderState` is built when the device is ready (~600ms),
    /// and the renderer is slotted in later via
    /// [`Self::attach_vello_renderer`] once `vello::Renderer::new`
    /// finishes on the background thread.
    pub fn new_gpu_skeleton(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
    ) -> Self {
        Self {
            surface: SurfaceMode::Gpu { gpu_pool, surface, dev_id },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            #[cfg(target_arch = "wasm32")]
            canvas2d_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::VelloGpu,
        }
    }

    /// Slot a freshly-built vello `Renderer` into a skeleton state.
    pub fn attach_vello_renderer(&mut self, renderer: VelloRenderer) {
        self.vello_gpu_renderer = Some(renderer);
    }

    /// Borrow the underlying `SurfaceMode` mutably so the skeleton
    /// painter can reach `gpu_pool` / `surface` directly.  This is
    /// the only public hook into the private `surface` field.
    pub fn surface_mut_for_skeleton(&mut self) -> &mut SurfaceMode {
        &mut self.surface
    }

    /// `true` if the vello GPU renderer is wired up and ready to submit.
    pub fn has_vello_gpu_renderer(&self) -> bool {
        self.vello_gpu_renderer.is_some()
    }

    /// Current configured surface dimensions `(width, height)` in physical
    /// pixels — the size the swapchain / software target is sized at, which is
    /// also what `submit_frame`'s `render_to_texture` uses. Lets the driver
    /// detect a surface-vs-window size desync (diagnostics + self-heal).
    pub fn surface_config_size(&self) -> (u32, u32) {
        match &self.surface {
            SurfaceMode::Gpu { surface, .. } => (surface.config.width, surface.config.height),
            SurfaceMode::Software { width, height, .. } => (*width, *height),
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
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            #[cfg(target_arch = "wasm32")]
            canvas2d_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(dpr),
            active,
        }
    }

    /// Build a CPU-only (tiny-skia) state with a software presenter.
    ///
    /// `presenter` is the [`SoftwarePresenter`] obtained from
    /// [`WindowProvider::create_software_presenter`](uzor::layout::window::WindowProvider::create_software_presenter).
    /// It is called once per frame to blit the CPU-rasterized pixels to the OS window.
    ///
    /// Available on native targets only — use the Canvas 2D path on wasm32.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_cpu(width: u32, height: u32, presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self {
            surface: SurfaceMode::Software { presenter, width, height },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: Some(TinySkiaCpuRenderContext::new(width, height, 1.0)),
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::TinySkia,
        }
    }

    /// Build a CPU-only (vello-cpu) state with a software presenter.
    ///
    /// `presenter` is the [`SoftwarePresenter`] obtained from
    /// [`WindowProvider::create_software_presenter`](uzor::layout::window::WindowProvider::create_software_presenter).
    /// It is called once per frame to blit the CPU-rasterized pixels to the OS window.
    ///
    /// Available on native targets only — use the Canvas 2D path on wasm32.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new_vello_cpu(dpr: f64, presenter: Box<dyn SoftwarePresenter>) -> Self {
        Self {
            surface: SurfaceMode::Software { presenter, width: 0, height: 0 },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: Some(VelloCpuRenderContext::new(dpr)),
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(dpr),
            active: RenderBackend::VelloCpu,
        }
    }

    // (helper above the constructor)
}

/// Recreate `surface.target_texture` with the usage flags CPU
/// backends and the screenshot path require: `COPY_SRC | COPY_DST |
/// RENDER_ATTACHMENT` on top of the vello defaults.
///
/// Same shape as the `add_copy_src_to_target_texture` helper used by
/// the screenshot endpoint — kept here so the swapchain comes up
/// with the right usage from the very first frame, not lazily.
#[cfg(not(target_arch = "wasm32"))]
fn recreate_target_with_cpu_usage(
    surface: &mut RenderSurface<'static>,
    device: &wgpu::Device,
    width: u32,
    height: u32,
) {
    let size = wgpu::Extent3d {
        width: width.max(1),
        height: height.max(1),
        depth_or_array_layers: 1,
    };
    let new_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("target_texture_cpu_swapchain"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let new_view = new_texture.create_view(&wgpu::TextureViewDescriptor::default());
    surface.target_texture = new_texture;
    surface.target_view = new_view;
}

impl WindowRenderState {
    /// Build a GPU-mode state for tiny-skia (CPU rasteriser, GPU
    /// swapchain).  Each frame the tiny-skia pixmap is uploaded to
    /// `surface.target_texture` via `queue.write_texture` and blitted
    /// to the swapchain — same path mlc uses.
    pub fn new_tiny_skia_gpu(
        gpu_pool: GpuDevicePool,
        mut surface: RenderSurface<'static>,
        dev_id: usize,
    ) -> Self {
        let (w, h) = (surface.config.width.max(1), surface.config.height.max(1));
        // Vello creates `target_texture` with TEXTURE_BINDING | STORAGE_BINDING
        // only; CPU upload needs COPY_DST and the screenshot path needs
        // COPY_SRC.  Recreate the texture with the right usage flags
        // BEFORE handing the surface to the new state.
        recreate_target_with_cpu_usage(&mut surface, &gpu_pool.devices[dev_id].device, w, h);
        Self {
            surface: SurfaceMode::Gpu { gpu_pool, surface, dev_id },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: Some(TinySkiaCpuRenderContext::new(w, h, 1.0)),
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            #[cfg(target_arch = "wasm32")]
            canvas2d_ctx: None,
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::TinySkia,
        }
    }

    /// Build a GPU-mode state for vello-cpu (CPU rasteriser, GPU
    /// swapchain).  Mirror of `new_tiny_skia_gpu`.
    pub fn new_vello_cpu_gpu(
        gpu_pool: GpuDevicePool,
        mut surface: RenderSurface<'static>,
        dev_id: usize,
        dpr: f64,
    ) -> Self {
        let (cw, ch) = (surface.config.width.max(1), surface.config.height.max(1));
        recreate_target_with_cpu_usage(&mut surface, &gpu_pool.devices[dev_id].device, cw, ch);
        Self {
            surface: SurfaceMode::Gpu { gpu_pool, surface, dev_id },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: Some(VelloCpuRenderContext::new(dpr)),
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            #[cfg(target_arch = "wasm32")]
            canvas2d_ctx: None,
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

    /// Build a Canvas 2D state for DOM canvas rendering (wasm32 only).
    ///
    /// The `canvas` element is the HTML canvas being rendered into.
    /// The `ctx` is a [`Canvas2dRenderContext`] wrapping the
    /// `CanvasRenderingContext2d` obtained from `canvas.getContext("2d")`.
    #[cfg(target_arch = "wasm32")]
    pub fn new_canvas2d(
        canvas: web_sys::HtmlCanvasElement,
        ctx: Canvas2dRenderContext,
    ) -> Self {
        Self {
            surface: SurfaceMode::Canvas2d { canvas },
            vello_gpu_renderer: None,
            vello_hybrid_renderer: None,
            instanced_renderer: None,
            instanced_ctx: None,
            vello_cpu_ctx: None,
            tiny_skia_ctx: None,
            urx_ctx: None,
            urx_cpu_backend: None,
            urx_cpu_pixmap: None,
            urx_wgpu_backend: None,
            urx_hybrid_backend: None,
            active_urx: None,
            canvas2d_ctx: Some(ctx),
            scene: Scene::new(),
            vello_hybrid_ctx: VelloHybridRenderContext::new(1.0),
            active: RenderBackend::Canvas2d,
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// The active [`RenderBackend`] for this window.
    pub fn backend(&self) -> RenderBackend {
        self.active
    }

    /// Borrow the wgpu device + queue + render surface tuple, if this
    /// window is GPU-backed.  Returns `None` for software-presented
    /// windows (TinySkia / VelloCpu in headless GPU mode) and on web.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn gpu_handles(&self) -> Option<(&wgpu::Device, &wgpu::Queue, &vello::util::RenderSurface<'static>)> {
        match &self.surface {
            SurfaceMode::Gpu { gpu_pool, surface, dev_id } => {
                let dh = gpu_pool.devices.get(*dev_id)?;
                Some((&dh.device, &dh.queue, surface))
            }
            #[cfg(not(target_arch = "wasm32"))]
            SurfaceMode::Software { .. } => None,
            #[cfg(target_arch = "wasm32")]
            SurfaceMode::Canvas2d { .. } => None,
        }
    }

    /// Mutable variant of [`gpu_handles`] — returns the surface as
    /// `&mut` so callers can patch its texture (e.g. add COPY_SRC for
    /// screenshots).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn gpu_handles_mut(
        &mut self,
    ) -> Option<(&wgpu::Device, &wgpu::Queue, &mut vello::util::RenderSurface<'static>)> {
        match &mut self.surface {
            SurfaceMode::Gpu { gpu_pool, surface, dev_id } => {
                let dh = gpu_pool.devices.get(*dev_id)?;
                Some((&dh.device, &dh.queue, surface))
            }
            #[cfg(not(target_arch = "wasm32"))]
            SurfaceMode::Software { .. } => None,
            #[cfg(target_arch = "wasm32")]
            SurfaceMode::Canvas2d { .. } => None,
        }
    }

    /// Set the active backend (live switching).
    ///
    /// Calls [`ensure_backend_slot`] internally so the matching
    /// renderer / context is ready before the next frame.
    pub fn set_active(&mut self, backend: RenderBackend) {
        self.active = backend;
        self.ensure_backend_slot(backend);
    }

    /// Lazily create whatever renderer / CPU context the given
    /// backend needs.  No-op if the slot is already populated or if
    /// the backend's slot is created on first submit anyway
    /// (`VelloHybrid`, `InstancedWgpu`).  Used after live backend
    /// switching to wake up a previously-cold path on this window.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn ensure_backend_slot(&mut self, backend: RenderBackend) {
        match backend {
            RenderBackend::VelloGpu => {
                if self.vello_gpu_renderer.is_none() {
                    if let Some((device, _, _)) = self.gpu_handles() {
                        match vello::Renderer::new(
                            device,
                            vello::RendererOptions {
                                use_cpu: false,
                                antialiasing_support: vello::AaSupport::all(),
                                num_init_threads: std::num::NonZeroUsize::new(1),
                                pipeline_cache: None,
                            },
                        ) {
                            Ok(r) => self.vello_gpu_renderer = Some(r),
                            Err(e) => eprintln!("[render-hub] VelloGpu renderer init failed: {e}"),
                        }
                    }
                }
            }
            RenderBackend::VelloCpu => {
                if self.vello_cpu_ctx.is_none() {
                    self.vello_cpu_ctx = Some(VelloCpuRenderContext::new(1.0));
                }
            }
            RenderBackend::TinySkia => {
                if self.tiny_skia_ctx.is_none() {
                    let (w, h) = self.gpu_handles()
                        .map(|(_, _, s)| (s.config.width.max(1), s.config.height.max(1)))
                        .unwrap_or((1, 1));
                    self.tiny_skia_ctx = Some(TinySkiaCpuRenderContext::new(w, h, 1.0));
                }
            }
            // Lazy on first submit.
            RenderBackend::VelloHybrid | RenderBackend::InstancedWgpu => {}
            #[cfg(target_arch = "wasm32")]
            RenderBackend::Canvas2d => {}
            #[cfg(not(target_arch = "wasm32"))]
            RenderBackend::Canvas2d => {}

            // URX family — `urx_ctx` is the only shared slot; per-backend
            // backends (CpuBackend / WgpuBackend / HybridBackend) lazy-init
            // on first submit because they need the surface format.
            RenderBackend::UrxCpu
            | RenderBackend::UrxWgpu
            | RenderBackend::UrxHybrid
            | RenderBackend::UrxWgpuFull => {
                if self.urx_ctx.is_none() {
                    self.urx_ctx = Some(uzor_render_urx::UrxRenderContext::new(1.0));
                }
            }
        }
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

    /// Mutable reference to the Canvas 2D render context (wasm32 only).
    ///
    /// Returns `Some` only when `active` is [`RenderBackend::Canvas2d`] and
    /// the context has been initialized.
    #[cfg(target_arch = "wasm32")]
    pub fn canvas2d_ctx_mut(&mut self) -> Option<&mut Canvas2dRenderContext> {
        if matches!(self.active, RenderBackend::Canvas2d) {
            self.canvas2d_ctx.as_mut()
        } else {
            None
        }
    }

    /// Shared reference to the Canvas 2D render context (wasm32 only).
    #[cfg(target_arch = "wasm32")]
    pub fn canvas2d_ctx(&self) -> Option<&Canvas2dRenderContext> {
        if matches!(self.active, RenderBackend::Canvas2d) {
            self.canvas2d_ctx.as_ref()
        } else {
            None
        }
    }

    // ── Unified render-context accessor ──────────────────────────────────────

    /// Call `f` with a `&mut dyn RenderContext` wired to the active backend.
    ///
    /// This is the ergonomic entry point for user code inside `App::ui`.
    /// Widget L3 helpers (e.g. `register_layout_manager_button`) accept
    /// `&mut dyn RenderContext`; pass the argument you receive here directly
    /// to those helpers:
    ///
    /// ```rust,ignore
    /// fn ui(&mut self, layout: &mut LayoutManager<NoPanel>, state: &mut WindowRenderState) {
    ///     state.with_render_context(|render| {
    ///         register_layout_manager_button(layout, render, "btn", rect, &layer, &view, &settings);
    ///     });
    /// }
    /// ```
    ///
    /// Returns `None` only when the active backend is `InstancedWgpu` (which
    /// does not expose a `RenderContext`-compatible draw API) or when the
    /// corresponding context slot is uninitialised.
    /// Like [`with_render_context`] but writes into a caller-supplied
    /// `vello::Scene` rather than the per-window main scene.  Used by the
    /// per-region paint scheduler so each region can build into its own
    /// cached scene and the runtime then composites them.
    ///
    /// VelloGpu / VelloHybrid only — returns `None` on backends that don't
    /// expose a vello-style `Scene` (Canvas2d / TinySkia / VelloCpu).
    pub fn with_scene_render_context<R>(
        &mut self,
        scene: &mut Scene,
        f: impl FnOnce(&mut dyn uzor::render::RenderContext) -> R,
    ) -> Option<R> {
        match self.active {
            RenderBackend::VelloGpu => {
                let mut ctx = VelloGpuRenderContext::new(scene, 0.0, 0.0);
                Some(f(&mut ctx))
            }
            RenderBackend::VelloHybrid => {
                // VelloHybrid uses its own context type backed by vello-hybrid::Scene,
                // not vello::Scene. Per-region caching for hybrid is a future patch;
                // for now fall through and let the caller use with_render_context.
                None
            }
            _ => None,
        }
    }

    /// Append a previously-built region scene into the main per-window
    /// scene.  No-op on non-vello backends.
    pub fn append_region_scene(&mut self, region_scene: &Scene) {
        if matches!(self.active, RenderBackend::VelloGpu | RenderBackend::VelloHybrid) {
            self.scene.append(region_scene, None);
        }
    }

    pub fn with_render_context<R>(
        &mut self,
        f: impl FnOnce(&mut dyn uzor::render::RenderContext) -> R,
    ) -> Option<R> {
        match self.active {
            RenderBackend::VelloGpu => {
                let mut ctx = VelloGpuRenderContext::new(&mut self.scene, 0.0, 0.0);
                Some(f(&mut ctx))
            }
            RenderBackend::VelloHybrid => {
                // Route through the dedicated hybrid context; submit_vello_hybrid
                // reads from the same context. Routing through VelloGpuRenderContext
                // would write to the wrong scene and the swapchain stays blank.
                Some(f(&mut self.vello_hybrid_ctx))
            }
            RenderBackend::VelloCpu => {
                let (w, h) = self.gpu_handles()
                    .map(|(_, _, s)| (s.config.width.max(1), s.config.height.max(1)))
                    .unwrap_or((1, 1));
                self.vello_cpu_ctx.as_mut().map(|c| {
                    c.begin_frame(w, h);
                    f(c)
                })
            }
            RenderBackend::TinySkia => {
                let (w, h) = self.gpu_handles()
                    .map(|(_, _, s)| (s.config.width.max(1), s.config.height.max(1)))
                    .unwrap_or((1, 1));
                self.tiny_skia_ctx.as_mut().map(|c| {
                    if c.width() != w || c.height() != h {
                        c.resize(w, h);
                    }
                    f(c)
                })
            }
            RenderBackend::InstancedWgpu => {
                // Lazy-init the per-frame context. Lives across frames; we
                // call `clear()` at the start of each frame to reset
                // `draw_commands` while reusing the Vec allocation.
                //
                // Dimensions come from the GPU surface config (physical
                // pixels). On software surfaces InstancedWgpu is not
                // supported; we return None there.
                let (w, h) = self.gpu_handles()
                    .map(|(_, _, s)| (s.config.width.max(1), s.config.height.max(1)))
                    .unwrap_or((1, 1));
                if self.instanced_ctx.is_none() {
                    self.instanced_ctx = Some(InstancedRenderContext::new(
                        w as f32, h as f32, 0.0, 0.0,
                    ));
                }
                self.instanced_ctx.as_mut().map(|c| {
                    // Reset draw_commands for the new frame. (Walker is
                    // about to push fresh commands into it.)
                    c.clear();
                    // Recreate the context if the surface resized — the
                    // screen_w/screen_h are baked into the transform and
                    // root clip rect at construction time, so a stale
                    // context paints at the old size.
                    //
                    // NOTE: InstancedRenderContext has no setter for
                    // these fields today, so we replace the whole
                    // context when the size changes. Add a `resize()`
                    // method upstream if this becomes a hot path.
                    let _ = (w, h); // size-aware resize TBD
                    f(c)
                })
            }
            #[cfg(target_arch = "wasm32")]
            RenderBackend::Canvas2d => {
                self.canvas2d_ctx.as_mut().map(|c| f(c))
            }
            #[cfg(not(target_arch = "wasm32"))]
            RenderBackend::Canvas2d => None,

            // ── URX family ─────────────────────────────────────────────────
            // All four URX backends share one `urx_ctx`: the consumer paints
            // into a `urx_core::Scene` via UrxRenderContext. The backend
            // (Cpu / Wgpu / Hybrid / WgpuFull) is dispatched at submit time
            // and consumes that same Scene.
            RenderBackend::UrxCpu
            | RenderBackend::UrxWgpu
            | RenderBackend::UrxHybrid
            | RenderBackend::UrxWgpuFull => {
                let (w, h) = self.gpu_handles()
                    .map(|(_, _, s)| (s.config.width.max(1), s.config.height.max(1)))
                    .unwrap_or_else(|| match &self.surface {
                        #[cfg(not(target_arch = "wasm32"))]
                        SurfaceMode::Software { width, height, .. } => (*width, *height),
                        _ => (1, 1),
                    });
                if self.urx_ctx.is_none() {
                    self.urx_ctx = Some(uzor_render_urx::UrxRenderContext::new(1.0));
                }
                self.urx_ctx.as_mut().map(|c| {
                    c.begin_frame(w, h);
                    f(c)
                })
            }
        }
    }

    // ── Surface lifecycle ─────────────────────────────────────────────────────

    /// Block the CPU until every GPU submission on this surface's
    /// device has completed.  Used by animations that want to be
    /// certain a freshly-presented frame is *on the screen* before
    /// they touch the OS window (e.g. atomic SetWindowPos right
    /// after the corresponding swapchain present — without the
    /// wait DWM may composite a stale backbuffer onto the new
    /// outer rect for one vblank).  No-op on non-GPU backends.
    pub fn wait_gpu_idle(&self) {
        if let SurfaceMode::Gpu { gpu_pool, dev_id, .. } = &self.surface {
            let device = &gpu_pool.devices[*dev_id].device;
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        }
    }

    /// Resize the underlying surface to match the window's new physical size.
    ///
    /// For GPU surfaces this re-creates the wgpu swapchain. For software
    /// surfaces it forwards to `SoftwarePresenter::resize` and updates the
    /// stored `width`/`height`.  Caller is responsible for ensuring no GPU
    /// frame is in flight when this is called.
    pub fn resize_surface(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        match &mut self.surface {
            SurfaceMode::Gpu { gpu_pool, surface, dev_id } => {
                gpu_pool.resize_surface(surface, width, height);
                // VelloHybrid renderer caches the target dimensions when it
                // was first created. After a swapchain resize we must drop
                // it so the next submit re-creates it with the new size —
                // otherwise the GPU draws into a stale render target and
                // the swapchain shows blank / stretched content.
                if matches!(self.active, RenderBackend::VelloHybrid) {
                    self.vello_hybrid_renderer = None;
                }
                // Vello always recreates the target with
                // STORAGE_BINDING | TEXTURE_BINDING only on resize.
                // We need COPY_SRC (screenshot) and COPY_DST (live
                // backend swap into a CPU rasteriser) on every
                // GPU-backed surface regardless of which renderer is
                // currently active — the user can flip at runtime.
                // Pass the NEW size — `old.size()` would re-create
                // the texture at the previous swapchain dimensions
                // and the next frame's blit would scale / clip
                // (visibly anchoring the scene to one corner when
                // the swapchain grew).
                let device = &gpu_pool.devices[*dev_id].device;
                recreate_target_with_cpu_usage(surface, device, width, height);
            }
            #[cfg(not(target_arch = "wasm32"))]
            SurfaceMode::Software { presenter, width: w, height: h } => {
                presenter.resize(width, height);
                *w = width;
                *h = height;
                // Resize the CPU pixmap / vello-cpu render context so the
                // submit path's `cw == width` check succeeds and present()
                // sends a non-empty frame.
                if let Some(ref mut ts) = self.tiny_skia_ctx {
                    ts.resize(width, height);
                }
                // VelloCpuRenderContext's pixmap is rebuilt on each
                // render_to_pixmap_rgba8 call from the buffer the submit
                // path provides — no explicit resize needed here.
            }
            #[cfg(target_arch = "wasm32")]
            SurfaceMode::Canvas2d { .. } => {
                // The canvas element resize is handled by the DOM layout —
                // nothing to do here.
            }
        }
    }

    // ── Frame lifecycle ───────────────────────────────────────────────────────

    /// Reset per-frame artifacts.  Call at the top of each frame.
    pub fn begin_frame(&mut self) {
        match self.active {
            RenderBackend::VelloGpu => self.scene.reset(),
            RenderBackend::VelloHybrid => {
                // Re-create / reset the hybrid scene with the current swapchain
                // size so the caller can paint into it.  Without this the
                // hybrid scene stays None and the swapchain stays blank.
                if let SurfaceMode::Gpu { ref surface, .. } = self.surface {
                    self.vello_hybrid_ctx
                        .begin_frame(surface.config.width, surface.config.height);
                }
            }
            RenderBackend::VelloCpu
            | RenderBackend::TinySkia
            | RenderBackend::InstancedWgpu => {
                // CPU pixel buffers and instanced commands are rebuilt by caller.
            }
            RenderBackend::Canvas2d => {
                // Canvas 2D draw calls are issued directly via canvas2d_ctx_mut().
                // No per-frame reset needed — the browser auto-clears as needed.
            }
            RenderBackend::UrxCpu
            | RenderBackend::UrxWgpu
            | RenderBackend::UrxHybrid
            | RenderBackend::UrxWgpuFull => {
                // urx_ctx::begin_frame is called by `with_render_context` (it
                // needs the surface size). Nothing per-frame here.
            }
        }
    }

    // ── URX channel (2026-06-05 dual-enum) ───────────────────────────────────
    //
    // Stage 1 ships the channel skeleton only: the handle exposes `render_ctx`
    // (re-using the existing `UrxRenderContext` Scene-buffering path) and frame
    // metadata. Engine / 3D / physics / particles slots are NOT yet on the
    // handle — they land in Stages 3/4 along with their backing slots on
    // `WindowRenderState`.

    /// Flip the URX channel's active backend. Setting `Some(b)` arms the
    /// `with_urx_engine` channel; the 2D channel (`with_render_context`)
    /// keeps working in parallel for now. Stage 3 will gate the two
    /// channels so they don't both try to paint the swapchain in the
    /// same frame.
    pub fn set_active_urx(&mut self, backend: Option<uzor::UrxBackend>) {
        self.active_urx = backend;
    }

    /// Read the URX channel's current backend selection (if any).
    pub fn active_urx(&self) -> Option<uzor::UrxBackend> {
        self.active_urx
    }

    /// Call `f` with a `UrxEngineHandle` bound to this window's URX
    /// channel. Returns `None` if the URX channel isn't armed
    /// (`active_urx == None`) or the surface has zero area.
    ///
    /// Stage 1 wiring: handle's `render_ctx` is the same
    /// `UrxRenderContext` Scene buffer the existing `with_render_context`
    /// path uses when one of the URX `Scene2DBackend` variants is active.
    /// Submitting at frame-end goes through the same `submit_urx_*`
    /// dispatcher; the only difference vs the legacy path is which
    /// backend is selected (via `active_urx` rather than `active`).
    pub fn with_urx_engine<R>(
        &mut self,
        f: impl FnOnce(&mut crate::urx_engine_handle::UrxEngineHandle<'_>) -> R,
    ) -> Option<R> {
        // Channel must be armed.
        let _backend = self.active_urx?;

        // Resolve surface dimensions.
        let (width, height) = match &self.surface {
            SurfaceMode::Gpu { surface, .. } => (surface.config.width, surface.config.height),
            #[cfg(not(target_arch = "wasm32"))]
            SurfaceMode::Software { width, height, .. } => (*width, *height),
            #[cfg(target_arch = "wasm32")]
            SurfaceMode::Canvas2d { .. } => (0, 0),
        };
        if width == 0 || height == 0 {
            return None;
        }

        // Lazy-init shared URX paint context.
        let dpr = 1.0_f64; // TODO Stage 3: real DPR from window.
        if self.urx_ctx.is_none() {
            self.urx_ctx = Some(uzor_render_urx::UrxRenderContext::new(dpr));
        }
        let ctx = self.urx_ctx.as_mut()?;
        ctx.begin_frame(width, height);

        let mut handle = crate::urx_engine_handle::UrxEngineHandle {
            render_ctx: ctx as &mut dyn uzor::render::RenderContext,
            width,
            height,
            dpr,
            frame_idx: 0, // Stage 3 wires real counter.
        };
        Some(f(&mut handle))
    }
}
