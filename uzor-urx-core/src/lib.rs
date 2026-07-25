//! URX render-family core — shared types and metrics facade.
//!
//! Every URX backend (uzor-urx-wgpu, uzor-urx-cpu, uzor-urx-hybrid)
//! consumes the same [`Scene`] of [`DrawCommand`]s + emits metrics
//! through this crate's [`metrics`] facade. urx-core itself has no
//! GPU or CPU rasterisation code — only the data model + telemetry.
//!
//! ## Layout
//!
//! - [`math`] — re-export of `kurbo` (geometry) and `peniko` (paint),
//!   plus a few thin convenience aliases. Consumers depend only on
//!   `uzor_urx_core::math` so we can swap the underlying crates
//!   without breaking the URL.
//! - [`scene`] — `Scene`, `DrawCommand`, `Glyph`, `ImageId`. The
//!   one shared scene encoding all backends walk.
//! - [`dirty`] — `DirtyState` (`Clean | TransformOnly | Content`),
//!   `DirtyRect`. The three-state contract every retained-mode
//!   research bucket converged on (cc, Flutter, GTK4).
//! - [`region`] — `RegionId`, `CacheKey`, `CachedRegion`. Per-region
//!   identity + texture cache used by the hybrid retained mode.
//! - [`skeleton`] — `SkeletonFrame` + `SkeletonSpec`. Cold-start
//!   first-frame painter (CPU, 0 GPU deps) — every WGPU backend
//!   must impl it so users never see a blank window while shaders
//!   compile.
//! - [`metrics_keys`] — flat catalog of all KEY_* names emitted by
//!   URX backends. Single source of truth so consumers can build
//!   dashboards / regression alerts without grepping each crate.
//! - [`recorder`] — `UrxRecorder` snapshot impl of `metrics::Recorder`
//!   (counters / gauges / ring-buffer histograms). Same pattern
//!   tessera uses; install once per process via `install_recorder()`.
//! - [`gradient_lut`] — pure gradient-LUT math (build/sample/hash),
//!   extracted from `uzor-urx-cpu::gradient` (URX Wave 4 design §2.2/
//!   §10 Commit 1) so a GPU `GradientLutAtlas` (Wave 4 Commit 2+) and
//!   CPU's own LUT cache build byte-identical tables from identical
//!   stops — no per-backend fork of the LUT math.
//! - [`text_gamma`] — pure text-gamma coverage-adjustment math (build/
//!   luma-bucket), shared by `uzor-urx-glyph::draw_glyph_run` (CPU) and
//!   `uzor-urx-wgpu`'s native glyph pipeline (GPU) so a LUT built from
//!   the same curve is byte-identical on both backends (URX text-gamma
//!   compositing design, 2026-07-26, §2.3 Commit 1).
//! - [`dash`] — shared dash-pattern geometry expansion (`Stroke.dash` ->
//!   a multi-subpath `BezPath`), reused by `uzor-urx-cpu`'s capsule
//!   stroker and `uzor-urx-wgpu`'s lyon tessellation path so both
//!   backends dash off the exact same `kurbo::dash`-derived geometry.

pub mod math;
pub mod scene;
pub mod dirty;
pub mod region;
pub mod skeleton;
pub mod metrics_keys;
pub mod recorder;
pub mod validate;
pub mod config;
pub mod gradient_lut;
pub mod text_gamma;
pub mod dash;

/// wgpu::PipelineCache disk persistence helpers (opt-in feature
/// `pipeline-cache`). Adds wgpu as a direct dep when enabled.
#[cfg(feature = "pipeline-cache")]
pub mod pipeline_cache;
// Non-wgpu helpers (path key) always available, even without the feature.
#[cfg(not(feature = "pipeline-cache"))]
pub mod pipeline_cache;

pub use math::{Affine, BezPath, Point, Rect, Size, Vec2};
pub use scene::{Dash, DrawCommand, FillRule, Glyph, ImageId, Scene, Stroke};
pub use dash::dash_path;
pub use dirty::{DirtyRect, DirtyState};
pub use region::{CacheKey, CachedRegion, RegionId};
pub use skeleton::{SkeletonFrame, SkeletonSpec};
pub use recorder::{install_recorder, metrics_snapshot, metrics_reset, UrxRecorder, MetricsSnapshot};
pub use metrics_keys::METRIC_CATALOG;
pub use validate::{
    ValidationIssue, validate_command,
    is_finite_rect, is_finite_affine, is_finite_vec2,
    is_finite_rounded_rect, is_finite_radii_opt,
};
pub use config::{UrxConfig, UrxConfigBuilder, ConfigError, SimdLevel, DirtyStrategy};
