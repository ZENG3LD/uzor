//! Per-window persistent render state + per-frame context construction.
//!
//! Mirrors the layout `mylittlechart`'s `PerWindow` uses:
//! - GPU-side `Renderer` instances are **lazy + persistent** (created on first
//!   use, kept across frames).
//! - The per-frame scene / pixel buffer / instance-command list is **rebuilt
//!   every frame** by the caller.
//!
//! Hub owns the persistent half (`WindowRenderState`); the caller owns the
//! per-frame half and feeds it into `submit_frame`.

use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
use uzor_render_vello_cpu::VelloCpuRenderContext;
use uzor_render_vello_gpu::VelloGpuRenderContext;
use uzor_render_vello_hybrid::VelloHybridRenderContext;
use uzor_render_wgpu_instanced::{DrawCmd, InstancedRenderContext, InstancedRenderer};
use vello::{Renderer as VelloRenderer, Scene};

use crate::backend::RenderBackend;

// ─── Per-frame context — caller-built, hub-consumed ──────────────────────────

/// Backend-specific render context the caller fills each frame.
///
/// `VelloGpu` borrows the vello scene; the others are owned. Build via
/// the `BackendContext::*` constructors below.
pub enum BackendContext<'a> {
    VelloGpu(VelloGpuRenderContext<'a>),
    VelloHybrid(VelloHybridRenderContext),
    Instanced(InstancedRenderContext),
    VelloCpu(VelloCpuRenderContext),
    TinySkia(TinySkiaCpuRenderContext),
}

impl<'a> BackendContext<'a> {
    pub fn vello_gpu(scene: &'a mut Scene, offset_x: f64, offset_y: f64) -> Self {
        Self::VelloGpu(VelloGpuRenderContext::new(scene, offset_x, offset_y))
    }
    pub fn vello_hybrid(dpr: f64) -> Self {
        Self::VelloHybrid(VelloHybridRenderContext::new(dpr))
    }
    pub fn instanced(screen_w: f32, screen_h: f32, offset_x: f32, offset_y: f32) -> Self {
        Self::Instanced(InstancedRenderContext::new(screen_w, screen_h, offset_x, offset_y))
    }
    pub fn vello_cpu(dpr: f64) -> Self {
        Self::VelloCpu(VelloCpuRenderContext::new(dpr))
    }
    pub fn tiny_skia(width: u32, height: u32, dpr: f64) -> Self {
        Self::TinySkia(TinySkiaCpuRenderContext::new(width, height, dpr))
    }
}

// ─── Persistent per-window render state ──────────────────────────────────────

/// Persistent backend state owned per-window.
///
/// Holds the lazy renderers (vello, vello_hybrid, instanced) and per-frame
/// "last produced" artifacts that GPU submit consumes (vello scene, cpu
/// pixel buffer, instanced draw-cmd list, hybrid context). Mirrors the
/// per-window state in `mlc/chart-app-vello/src/main.rs`.
///
/// The vello renderer is required (used both for the `VelloGpu` backend and
/// to power surface-blit on CPU backends), so it is constructed eagerly in
/// `new`. The other renderers are constructed lazily on first use.
pub struct WindowRenderState {
    /// Active backend for this window.
    pub backend: RenderBackend,

    /// vello GPU renderer — used by `VelloGpu` backend and by `vello`'s
    /// `BlitPipeline` to copy CPU target textures to the swapchain.
    pub vello_renderer: VelloRenderer,

    /// Per-frame vello scene (rebuilt each frame for `VelloGpu`).
    pub scene: Scene,

    /// Lazily-created instanced renderer.
    pub instanced_renderer: Option<InstancedRenderer>,
    /// Per-frame instanced draw-command list.
    pub instanced_commands: Vec<DrawCmd>,

    /// Lazily-created vello-hybrid renderer.
    pub hybrid_renderer: Option<vello_hybrid::Renderer>,
    /// Per-frame hybrid context (filled by caller during scene build).
    pub hybrid_ctx: Option<VelloHybridRenderContext>,

    /// CPU-rendered pixels (RGBA8) for `VelloCpu` / `TinySkia` backends.
    pub cpu_pixels: Vec<u8>,
    /// `(width, height)` of the CPU pixel buffer.
    pub cpu_dims: (u32, u32),
}

impl WindowRenderState {
    /// Build the persistent per-window state.
    ///
    /// `vello_renderer` is the eagerly-created vello renderer. The caller is
    /// responsible for constructing it via `vello::Renderer::new` because
    /// `RendererOptions` is opaque and may need app-specific tuning
    /// (`use_cpu`, `antialiasing_support`, `pipeline_cache`, …).
    pub fn new(backend: RenderBackend, vello_renderer: VelloRenderer) -> Self {
        Self {
            backend,
            vello_renderer,
            scene: Scene::new(),
            instanced_renderer: None,
            instanced_commands: Vec::new(),
            hybrid_renderer: None,
            hybrid_ctx: None,
            cpu_pixels: Vec::new(),
            cpu_dims: (0, 0),
        }
    }

    /// Reset all per-frame artifacts. Call at the start of every frame.
    pub fn begin_frame(&mut self) {
        self.scene.reset();
        self.instanced_commands.clear();
        self.hybrid_ctx = None;
        // CPU pixel buffer is reused in-place (resized as needed by the
        // caller during the build phase).
    }

    /// Switch backend at runtime.
    ///
    /// Invalidates lazy renderers tied to the previous backend so they get
    /// re-built on the next submit.
    pub fn set_backend(&mut self, backend: RenderBackend) {
        if self.backend == backend {
            return;
        }
        self.backend = backend;
        // Drop renderers that are specific to the previous backend; they'll
        // be lazily re-created on next submit.
        self.instanced_renderer = None;
        self.hybrid_renderer = None;
    }
}
