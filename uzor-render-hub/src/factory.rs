//! Per-window persistent render state.
//!
//! [`WindowRenderState`] is an enum that owns **everything** required for the
//! frame lifecycle of its backend variant:
//!
//! - [`WindowRenderState::Gpu`] — vello GPU pipeline.  Owns the
//!   [`GpuDevicePool`] (wgpu instance + device pool — vello's own
//!   `RenderContext` type, aliased here to avoid a name clash with
//!   `uzor::render::RenderContext`), the `RenderSurface` (wgpu swapchain),
//!   the `Renderer`, and the per-frame `Scene`.
//!
//! - [`WindowRenderState::Cpu`] — pure-CPU rasterizer.  Owns a
//!   `TinySkiaCpuRenderContext` (which itself wraps a `tiny_skia::Pixmap`).
//!   No wgpu dependency; no GPU initialisation cost.
//!
//! - [`WindowRenderState::VelloCpu`] — vello CPU renderer.  Owns a
//!   `VelloCpuRenderContext`.  No wgpu dependency.
//!
//! - [`WindowRenderState::VelloHybrid`] — vello hybrid CPU+GPU renderer.
//!   Owns `GpuDevicePool`, `RenderSurface`, and a `vello_hybrid::Renderer`.
//!
//! - [`WindowRenderState::WgpuInstanced`] — custom wgpu instanced renderer.
//!   Owns `GpuDevicePool`, `RenderSurface`, and an `InstancedRenderer`.

use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
use uzor_render_vello_cpu::VelloCpuRenderContext;
use uzor_render_vello_gpu::VelloGpuRenderContext;
use uzor_render_vello_hybrid::VelloHybridRenderContext;
use uzor_render_wgpu_instanced::{InstancedRenderContext, InstancedRenderer};
use vello::util::{RenderContext as VelloRenderContext, RenderSurface};
use vello::{Renderer as VelloRenderer, Scene};

use crate::backend::RenderBackend;

/// Local alias for vello's GPU device pool.
///
/// vello calls this type `RenderContext`, but that name collides with our
/// public `uzor::render::RenderContext` (the widget draw trait).  This alias
/// keeps the two universes apart inside the hub.
pub type GpuDevicePool = VelloRenderContext;

// ─── Per-frame context — caller-built, hub-consumed ──────────────────────────

/// Backend-specific render context the caller fills each frame.
///
/// `VelloGpu` borrows the vello scene; the others are owned.  Build via
/// the `BackendContext::*` constructors below.
pub enum BackendContext<'a> {
    VelloGpu(VelloGpuRenderContext<'a>),
    VelloHybrid(VelloHybridRenderContext),
    Instanced(InstancedRenderContext),
    VelloCpu(VelloCpuRenderContext),
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

// ─── Persistent per-window render state ──────────────────────────────────────

/// Persistent backend state owned per-window.
///
/// Each variant owns **all** resources required for GPU or CPU submission
/// without any external lifetime dependencies.  Constructed once per window
/// by a [`crate::RenderSurfaceFactory`] implementation; driven frame-by-frame
/// by the framework runtime.
pub enum WindowRenderState {
    /// GPU-backed vello rendering pipeline.
    ///
    /// Owns the [`GpuDevicePool`] (wgpu instance + device pool), the
    /// `RenderSurface` (swapchain tied to the OS window), the vello
    /// `Renderer`, and the per-frame `Scene`.
    Gpu {
        /// wgpu instance + device pool.
        gpu_pool: GpuDevicePool,
        /// wgpu swapchain bound to the OS window.  `'static` because the
        /// window is kept alive by the `WinitWindowProvider` for the entire
        /// runtime duration.
        surface: RenderSurface<'static>,
        /// vello GPU renderer.
        renderer: VelloRenderer,
        /// Per-frame vello scene (reset at `begin_frame`).
        scene: Scene,
        /// Device index into `gpu_pool.devices`.
        dev_id: usize,
    },
    /// Pure-CPU rasterizer (tiny-skia).  No wgpu, no GPU init cost.
    Cpu {
        /// tiny-skia CPU render context — owns the pixel buffer.
        ctx: TinySkiaCpuRenderContext,
    },
    /// vello CPU renderer — rasterizes on CPU, no wgpu dependency.
    ///
    /// Pixel output is available via `VelloCpuRenderContext::render_to_softbuffer`.
    VelloCpu {
        /// The vello-cpu render context — owns all CPU-side rasterization state.
        ctx: VelloCpuRenderContext,
    },
    /// vello hybrid — CPU strip encoding + GPU fine rasterization.
    ///
    /// Needs a wgpu surface (swapchain) for the GPU compositing phase,
    /// plus a `vello_hybrid::Renderer` for the GPU pass.
    VelloHybrid {
        /// wgpu instance + device pool.
        gpu_pool: GpuDevicePool,
        /// wgpu swapchain bound to the OS window.
        surface: RenderSurface<'static>,
        /// vello-hybrid GPU renderer (lazy-init on first submit).
        renderer: Option<vello_hybrid::Renderer>,
        /// Per-frame vello-hybrid context (reset at `begin_frame`).
        ctx: VelloHybridRenderContext,
        /// Device index into `gpu_pool.devices`.
        dev_id: usize,
    },
    /// Custom wgpu instanced renderer — no vello scene, no compute pipeline.
    ///
    /// Renders directly to the swapchain via `InstancedRenderer::render()`.
    WgpuInstanced {
        /// wgpu instance + device pool.
        gpu_pool: GpuDevicePool,
        /// wgpu swapchain bound to the OS window.
        surface: RenderSurface<'static>,
        /// Instanced renderer (lazy-init on first submit).
        renderer: Option<InstancedRenderer>,
        /// Device index into `gpu_pool.devices`.
        dev_id: usize,
    },
}

impl WindowRenderState {
    /// Construct a GPU-backed render state (vello GPU).
    pub fn new_gpu(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        renderer: VelloRenderer,
        dev_id: usize,
    ) -> Self {
        Self::Gpu {
            gpu_pool,
            surface,
            renderer,
            scene: Scene::new(),
            dev_id,
        }
    }

    /// Construct a CPU-backed render state backed by tiny-skia.
    pub fn new_cpu(width: u32, height: u32) -> Self {
        Self::Cpu {
            ctx: TinySkiaCpuRenderContext::new(width, height, 1.0),
        }
    }

    /// Construct a vello-cpu render state.
    pub fn new_vello_cpu(dpr: f64) -> Self {
        Self::VelloCpu {
            ctx: VelloCpuRenderContext::new(dpr),
        }
    }

    /// Construct a vello-hybrid render state.
    ///
    /// The `vello_hybrid::Renderer` is created lazily on the first frame
    /// submission (requires the swapchain texture format, available only then).
    pub fn new_vello_hybrid(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
        dpr: f64,
    ) -> Self {
        Self::VelloHybrid {
            gpu_pool,
            surface,
            renderer: None,
            ctx: VelloHybridRenderContext::new(dpr),
            dev_id,
        }
    }

    /// Construct a wgpu-instanced render state.
    ///
    /// The `InstancedRenderer` is created lazily on the first frame submission.
    pub fn new_wgpu_instanced(
        gpu_pool: GpuDevicePool,
        surface: RenderSurface<'static>,
        dev_id: usize,
    ) -> Self {
        Self::WgpuInstanced {
            gpu_pool,
            surface,
            renderer: None,
            dev_id,
        }
    }

    /// Returns the active [`RenderBackend`] for this state.
    pub fn backend(&self) -> RenderBackend {
        match self {
            Self::Gpu { .. }          => RenderBackend::VelloGpu,
            Self::Cpu { .. }          => RenderBackend::TinySkia,
            Self::VelloCpu { .. }     => RenderBackend::VelloCpu,
            Self::VelloHybrid { .. }  => RenderBackend::VelloHybrid,
            Self::WgpuInstanced { .. } => RenderBackend::InstancedWgpu,
        }
    }

    // ── Frame lifecycle ───────────────────────────────────────────────────────

    /// Reset all per-frame artifacts.  Call at the **top** of each frame
    /// before issuing draw commands.
    pub fn begin_frame(&mut self) {
        match self {
            Self::Gpu { scene, .. } => scene.reset(),
            Self::VelloHybrid { ctx, .. } => {
                // ctx is reset lazily via begin_frame inside VelloHybridRenderContext
                let _ = ctx; // reset is done by the caller filling a fresh context
            }
            Self::Cpu { .. } | Self::VelloCpu { .. } | Self::WgpuInstanced { .. } => {
                // CPU pixel buffer / instanced commands are rebuilt by the caller.
            }
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Mutable reference to the vello `Scene` (Gpu variant only).
    ///
    /// Returns `None` for all other backends.
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        match self {
            Self::Gpu { scene, .. } => Some(scene),
            _ => None,
        }
    }

    /// Shared reference to the vello `Scene` (Gpu variant only).
    pub fn scene(&self) -> Option<&Scene> {
        match self {
            Self::Gpu { scene, .. } => Some(scene),
            _ => None,
        }
    }

    /// Mutable reference to the tiny-skia CPU render context (Cpu variant only).
    ///
    /// Returns `None` for all other backends.
    pub fn cpu_ctx_mut(&mut self) -> Option<&mut TinySkiaCpuRenderContext> {
        match self {
            Self::Cpu { ctx } => Some(ctx),
            _ => None,
        }
    }

    /// Shared reference to the tiny-skia CPU render context (Cpu variant only).
    pub fn cpu_ctx(&self) -> Option<&TinySkiaCpuRenderContext> {
        match self {
            Self::Cpu { ctx } => Some(ctx),
            _ => None,
        }
    }

    /// Mutable reference to the vello-cpu render context (VelloCpu variant only).
    pub fn vello_cpu_ctx_mut(&mut self) -> Option<&mut VelloCpuRenderContext> {
        match self {
            Self::VelloCpu { ctx } => Some(ctx),
            _ => None,
        }
    }

    /// Mutable reference to the vello-hybrid render context (VelloHybrid variant only).
    pub fn vello_hybrid_ctx_mut(&mut self) -> Option<&mut VelloHybridRenderContext> {
        match self {
            Self::VelloHybrid { ctx, .. } => Some(ctx),
            _ => None,
        }
    }
}
