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

pub mod math;
pub mod scene;
pub mod dirty;
pub mod region;
pub mod skeleton;
pub mod metrics_keys;
pub mod recorder;
pub mod validate;
pub mod config;

pub use math::{Affine, BezPath, Point, Rect, Size, Vec2};
pub use scene::{DrawCommand, FillRule, Glyph, ImageId, Scene, Stroke};
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
