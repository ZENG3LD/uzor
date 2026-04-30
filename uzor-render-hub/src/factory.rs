//! Per-window persistent render state.
//!
//! [`WindowRenderState`] is an enum that owns **everything** required for the
//! frame lifecycle of its backend variant:
//!
//! - [`WindowRenderState::Gpu`] — vello GPU pipeline.  Owns the
//!   [`GpuDevicePool`] (wgpu instance + device pool — vello's own
//!   `RenderContext` type, aliased here to avoid a name clash with
//!   `uzor::render::RenderContext`), the `RenderSurface` (wgpu swapchain),
//!   the `Renderer`, and the per-frame `Scene`.  The surface is never dropped
//!   after init — this is the primary goal of the refactor.
//!
//! - [`WindowRenderState::Cpu`] — pure-CPU rasterizer.  Owns a
//!   `TinySkiaCpuRenderContext` (which itself wraps a `tiny_skia::Pixmap`).
//!   No wgpu dependency; no GPU initialisation cost.
//!
//! # Breaking change
//!
//! Prior to this refactor `WindowRenderState` was a flat struct with public
//! fields (`backend`, `vello_renderer`, `scene`, `cpu_pixels`, etc.).  Those
//! fields are gone.  Callers should use the variant-specific accessors:
//! [`WindowRenderState::scene_mut`], [`WindowRenderState::cpu_ctx_mut`], etc.
//!
//! The [`BackendContext`] enum (per-frame context built by callers) is
//! unchanged.

use uzor_render_tiny_skia::TinySkiaCpuRenderContext;
use uzor_render_vello_cpu::VelloCpuRenderContext;
use uzor_render_vello_gpu::VelloGpuRenderContext;
use uzor_render_vello_hybrid::VelloHybridRenderContext;
use uzor_render_wgpu_instanced::InstancedRenderContext;
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
///
/// # Breaking change (v2)
///
/// The previously flat struct is now an enum.  Old code that accessed public
/// fields like `state.vello_renderer`, `state.scene`, `state.cpu_pixels`, etc.
/// must be updated to use the methods below or to match on the enum variant.
pub enum WindowRenderState {
    /// GPU-backed vello rendering pipeline.
    ///
    /// Owns the [`GpuDevicePool`] (wgpu instance + device pool), the
    /// `RenderSurface` (swapchain tied to the OS window), the vello
    /// `Renderer`, and the per-frame `Scene`.
    ///
    /// The `RenderSurface` has a `'static` lifetime bound because both the
    /// surface and the device pool that owns the device are stored together
    /// in this variant — the surface cannot outlive its device, and both are
    /// dropped together when the variant is dropped.
    Gpu {
        /// wgpu instance + device pool.  Kept alongside the surface so the
        /// surface lifetime constraint (`device must outlive surface`) is
        /// satisfied by construction.
        gpu_pool: GpuDevicePool,
        /// wgpu swapchain bound to the OS window.  `'static` because the
        /// window (`Arc<winit::window::Window>`) that the handle points to is
        /// kept alive by the `WinitWindowProvider` for the entire runtime
        /// duration — the surface will be dropped before the window.
        surface: RenderSurface<'static>,
        /// vello GPU renderer.
        renderer: VelloRenderer,
        /// Per-frame vello scene (reset at `begin_frame`).
        scene: Scene,
        /// Device index into `gpu_pool.devices` (set at init, stable across
        /// frames since we never add/remove devices after startup).
        dev_id: usize,
    },
    /// Pure-CPU rasterizer (tiny-skia).  No wgpu, no GPU init cost.
    Cpu {
        /// tiny-skia CPU render context — owns the pixel buffer.
        ctx: TinySkiaCpuRenderContext,
    },
}

impl WindowRenderState {
    /// Construct a GPU-backed render state.
    ///
    /// All GPU resources are moved in; ownership is transferred to this
    /// variant.  The `RenderSurface<'static>` must have been created from a
    /// window whose `Arc<Window>` outlives the runtime (which is always true
    /// when `WinitWindowProvider` is used).
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

    /// Returns the active [`RenderBackend`] for this state.
    pub fn backend(&self) -> RenderBackend {
        match self {
            Self::Gpu { .. } => RenderBackend::VelloGpu,
            Self::Cpu { .. } => RenderBackend::TinySkia,
        }
    }

    // ── Frame lifecycle ───────────────────────────────────────────────────────

    /// Reset all per-frame artifacts.  Call at the **top** of each frame
    /// before issuing draw commands.
    pub fn begin_frame(&mut self) {
        match self {
            Self::Gpu { scene, .. } => scene.reset(),
            Self::Cpu { .. } => {
                // CPU pixel buffer is rebuilt by the caller during the draw
                // phase; no reset needed here.
            }
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    /// Mutable reference to the vello `Scene` (GPU variant only).
    ///
    /// Returns `None` for CPU backends.
    pub fn scene_mut(&mut self) -> Option<&mut Scene> {
        match self {
            Self::Gpu { scene, .. } => Some(scene),
            Self::Cpu { .. } => None,
        }
    }

    /// Shared reference to the vello `Scene` (GPU variant only).
    pub fn scene(&self) -> Option<&Scene> {
        match self {
            Self::Gpu { scene, .. } => Some(scene),
            Self::Cpu { .. } => None,
        }
    }

    /// Mutable reference to the CPU render context (CPU variant only).
    ///
    /// Returns `None` for GPU backends.
    pub fn cpu_ctx_mut(&mut self) -> Option<&mut TinySkiaCpuRenderContext> {
        match self {
            Self::Cpu { ctx } => Some(ctx),
            Self::Gpu { .. } => None,
        }
    }

    /// Shared reference to the CPU render context (CPU variant only).
    pub fn cpu_ctx(&self) -> Option<&TinySkiaCpuRenderContext> {
        match self {
            Self::Cpu { ctx } => Some(ctx),
            Self::Gpu { .. } => None,
        }
    }
}
