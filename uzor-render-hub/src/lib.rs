//! uzor-render-hub: unified rendering backend hub.
//!
//! Single abstraction layer over uzor's 5 render backends. Apps (and
//! `uzor-framework`) talk only to this crate — they never depend on
//! `uzor-backend-*` directly.
//!
//! Responsibilities:
//! - **Detect**: pick a backend from a wgpu adapter.
//! - **Init**: per-backend wgpu device features / limits / MSAA / fps defaults.
//! - **Create**: instantiate the right `RenderContext` impl for the active backend.
//! - **Submit**: dispatch frame submission across backends.
//! - **Metrics**: collect frame_time / gpu_submit / draw_calls without rendering UI.

pub mod backend;
pub mod detect;
pub mod metrics;
pub mod factory;
pub mod submit;

pub use backend::RenderBackend;
pub use detect::{detect_backend, default_perf, detect, GpuInfo, PerfDefaults, RecommendedBackend};
pub use metrics::RenderMetrics;
pub use factory::{BackendContext, WindowRenderState};
pub use submit::{submit_frame, SubmitOutcome, SubmitParams};
