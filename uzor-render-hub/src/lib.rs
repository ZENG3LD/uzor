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

pub mod backend;
pub mod detect;
pub mod hub;
pub mod metrics;
pub mod factory;
pub mod submit;
pub mod submit_urx;
pub mod compose;
pub mod runtime;
pub mod surface;
pub mod factories;
pub mod urx_engine_handle;

pub use backend::RenderBackend;
pub use detect::{detect_backend, default_perf, detect, GpuInfo, PerfDefaults, RecommendedBackend};

// URX cold-start skeleton types — re-exported so consumers
// (tessera-window etc.) don't need a direct `uzor-urx-core` dep
// just to pass a spec into `WindowRenderState::paint_skeleton`.
pub use uzor_urx_core::{SkeletonSpec, SkeletonFrame};
pub use hub::{BackendPool, HubError, PerfSettings, RenderHub};
pub use metrics::RenderMetrics;
pub use factory::{BackendContext, GpuDevicePool, Submit3DError, UrxCapture3D, WindowRenderState};
pub use compose::{Compose3DJob, ComposedOutcome, submit_urx_composed};
pub use urx_engine_handle::UrxEngineHandle;
pub use submit::{submit_frame, SubmitOutcome, SubmitParams};
pub use submit_urx::{submit_urx_regions, RegionSubmitOutcome};
pub use uzor::layout::window::SoftwarePresenter;
pub use runtime::RuntimeBackend;
pub use surface::{RenderSurfaceFactory, SurfaceError, SurfaceSize};
pub use factories::Canvas2dSurfaceFactory;

#[cfg(not(target_arch = "wasm32"))]
pub use factories::{
    TinySkiaSurfaceFactory,
    VelloCpuSurfaceFactory,
    VelloGpuSurfaceFactory,
    VelloHybridSurfaceFactory,
    WgpuInstancedSurfaceFactory,
    GpuPrewarm,
    prewarm_vello_gpu,
    GpuDeviceReady,
    prewarm_device,
    build_renderer,
    build_surface_from_device,
};
