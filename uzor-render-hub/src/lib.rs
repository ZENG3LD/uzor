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
pub mod runtime;
pub mod surface;
pub mod factories;

pub use backend::RenderBackend;
pub use detect::{detect_backend, default_perf, detect, GpuInfo, PerfDefaults, RecommendedBackend};
pub use hub::{BackendPool, HubError, PerfSettings, RenderHub};
pub use metrics::RenderMetrics;
pub use factory::{BackendContext, GpuDevicePool, WindowRenderState};
pub use submit::{submit_frame, SubmitOutcome, SubmitParams};
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
