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

pub mod backend;
pub mod detect;
pub mod metrics;
pub mod factory;
pub mod submit;
pub mod runtime;
pub mod surface;
pub mod factories;

pub use backend::RenderBackend;
pub use detect::{detect_backend, default_perf, detect, GpuInfo, PerfDefaults, RecommendedBackend};
pub use metrics::RenderMetrics;
pub use factory::{BackendContext, GpuDevicePool, WindowRenderState};
pub use submit::{submit_frame, SubmitOutcome, SubmitParams};
pub use runtime::RuntimeBackend;
pub use surface::{RenderSurfaceFactory, SurfaceError, SurfaceSize};
pub use factories::{
    Canvas2dSurfaceFactory,
    TinySkiaSurfaceFactory,
    VelloCpuSurfaceFactory,
    VelloGpuSurfaceFactory,
    VelloHybridSurfaceFactory,
    WgpuInstancedSurfaceFactory,
};
