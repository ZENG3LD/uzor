//! URX Engine — cross-backend façade with per-region dirty tracking.
//!
//! ## What it owns
//!
//! - `regions: HashMap<RegionId, RegionState>` — one entry per
//!   independently-renderable region (consumer-declared).
//! - Per-region `DirtyState` (`Clean | TransformOnly | Content`) +
//!   `Affine` transform (so transform-only changes never trigger
//!   rasterisation).
//! - `dirty_union: DirtyRect` — union of all regions' dirty bboxes
//!   since last clear. Driver calls `needs_paint()` to read it.
//! - `RenderCadence` per region (Static / LowHz / HighHz / Forced).
//!
//! ## What it doesn't own
//!
//! - Cadence policy (Always / OnInput / Fps{N} / Manual) — that lives
//!   in the driver/kernel layer (consumer's `winit::ApplicationHandler`
//!   or equivalent). The engine just exposes `needs_paint()` so the
//!   driver can short-circuit idle frames.
//! - Backend choice — passed in at construction (`Cpu` / `Wgpu` /
//!   `Hybrid` after Phase 7).
//!
//! ## Contract
//!
//! ```ignore
//! let mut engine = UrxEngine::new_cpu(width, height);
//! engine.upsert_region(region_id, scene, bounds, Cadence::Static);
//! // ... consumer state changes ...
//! engine.mark_dirty(region_id);
//!
//! // driver per frame:
//! if let Some(rect) = engine.needs_paint() {
//!     engine.render_cpu(&mut pixmap);
//!     // present pixmap; engine resets dirty state internally.
//! }
//! ```

pub mod cache;
pub mod cadence;
pub mod engine;
pub mod region_state;

pub use cache::DEFAULT_CACHE_BUDGET_BYTES;
pub use cadence::RenderCadence;
pub use engine::{Backend, RenderTarget, UrxEngine};
pub use region_state::RegionState;
