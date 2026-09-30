//! uzor-render-hub: unified rendering backend hub.
//!
//! Single abstraction layer over uzor's render backends.  Apps (and
//! `uzor-framework`) talk only to this crate — they never depend on
//! `uzor-backend-*` directly.
//!
//! Responsibilities:
//! - **Detect**: pick a backend from a wgpu adapter.
//! - **Init**: per-backend wgpu device features / limits / MSAA / fps defaults.
//! - **Create**: instantiate the right `WindowRenderState` via a factory.
//! - **Submit**: dispatch frame submission across backends.
//! - **Metrics**: collect frame_time / gpu_submit / draw_calls.
//! - **Hub**: unified `RenderHub` owning pool + settings + metrics.
//!
//! # Cargo features
//!
//! Every backend sits behind its own feature; `default` enables all of
//! them. `tiny-skia` and `vello-cpu` are pure CPU (software presentation
//! only, unless `gpu` is also on); `vello-gpu`, `vello-hybrid`,
//! `wgpu-instanced`, `urx` and `urx-3d` each imply the internal `gpu`
//! feature (wgpu + vello device pool + winit surface plumbing).
//! [`RenderBackend`] keeps every variant in every build; use
//! [`is_backend_compiled`] to ask whether a variant is usable.

pub mod backend;
pub mod detect;
pub mod hub;
pub mod metrics;
pub mod factory;
pub mod submit;
#[cfg(feature = "urx")]
pub mod submit_urx;
#[cfg(feature = "urx-3d")]
pub mod compose;
pub mod runtime;
pub mod surface;
pub mod factories;
pub mod retained;
#[cfg(feature = "urx")]
pub mod urx_engine_handle;

pub use backend::{is_backend_compiled, RenderBackend, RenderFamily};
pub use detect::{
    default_perf, no_adapter_backend_for_family, resolve_render_family,
    resolve_render_family_from_process_env, PerfDefaults, RecommendedBackend,
};
#[cfg(feature = "gpu")]
pub use detect::{detect, detect_backend, detect_backend_for_family, detect_backend_urx, GpuInfo};

// URX cold-start skeleton types — re-exported so consumers
// (tessera-window etc.) don't need a direct `uzor-urx-core` dep
// just to pass a spec into `WindowRenderState::paint_skeleton`.
pub use uzor_urx_core::{SkeletonSpec, SkeletonFrame};
pub use hub::{BackendPool, HubError, PerfSettings, RenderHub};
pub use metrics::RenderMetrics;
pub use factory::{BackendContext, Submit3DError, WindowRenderState};
#[cfg(feature = "gpu")]
pub use factory::GpuDevicePool;
#[cfg(feature = "urx-3d")]
pub use factory::UrxCapture3D;
pub use retained::RetainedCache;
#[cfg(feature = "urx-3d")]
pub use compose::{
    CachedOverlayJob, Compose3DJob, ComposeParticlesJob, ComposedOutcome,
    submit_urx_composed, submit_urx_composed_with_scene, submit_particles_composed,
};
#[cfg(feature = "urx")]
pub use urx_engine_handle::UrxEngineHandle;
pub use submit::{submit_frame, SubmitOutcome, SubmitParams};
#[cfg(feature = "urx")]
pub use submit_urx::{submit_urx_regions, RegionSubmitOutcome};
pub use uzor::layout::window::SoftwarePresenter;
pub use runtime::RuntimeBackend;
pub use surface::{RenderSurfaceFactory, SurfaceError, SurfaceSize};
pub use factories::Canvas2dSurfaceFactory;

#[cfg(all(not(target_arch = "wasm32"), feature = "tiny-skia"))]
pub use factories::TinySkiaSurfaceFactory;
#[cfg(all(not(target_arch = "wasm32"), feature = "vello-cpu"))]
pub use factories::VelloCpuSurfaceFactory;
#[cfg(all(not(target_arch = "wasm32"), feature = "vello-gpu"))]
pub use factories::{VelloGpuSurfaceFactory, GpuPrewarm, prewarm_vello_gpu, build_renderer};
#[cfg(all(not(target_arch = "wasm32"), feature = "vello-hybrid"))]
pub use factories::VelloHybridSurfaceFactory;
#[cfg(all(not(target_arch = "wasm32"), feature = "wgpu-instanced"))]
pub use factories::WgpuInstancedSurfaceFactory;
#[cfg(all(not(target_arch = "wasm32"), feature = "urx"))]
pub use factories::UrxSurfaceFactory;
#[cfg(all(not(target_arch = "wasm32"), feature = "gpu"))]
pub use factories::{GpuDeviceReady, prewarm_device, build_surface_from_device};
