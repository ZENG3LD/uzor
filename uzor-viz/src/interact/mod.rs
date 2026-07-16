//! Interaction plane — V2 milestone. Harvested from `mylittlechart`'s
//! input pipeline (`engine/input/**`, `nemo/docs/uzor-viz/
//! mlc_harvest_inventory.md` §4) and `uzor-graph`'s `FocusSet`
//! (`uzor-graph/src/interaction/focus.rs`), generalized: see
//! `nemo/docs/uzor-viz/uzor_viz_engine_architecture.md` §3 (`interact/`
//! block) and §4 (design laws).
//!
//! Shape kept from mlc: **input action (semantic) -> small per-concern
//! handler -> output action**. Dropped: one monolithic
//! `DefaultChartInputHandler` god-object juggling pan/zoom/kinetic-scroll/
//! primitive-drag/undo/sub-panes all at once. V2's scope is exactly two
//! gestures — hover (crosshair/tooltip) and a 1D X-brush — so each gets
//! its own small, independently-testable state machine instead
//! ([`brush::BrushState`], [`focus::FocusSet`]) rather than a single
//! struct owning every concern mlc's `ChartInputState` did (pane
//! separators, drawing-primitive control points, kinetic-scroll velocity,
//! price/time-scale drag-mode disambiguation — none of that applies to a
//! composed-figure engine with no chart-specific chrome).
//!
//! Every hit-test in [`hit`] goes through [`crate::coord::PlotArea`] —
//! design law #1, the same transform every mark/guide draws with.

pub mod action;
pub mod brush;
pub mod focus;
pub mod hit;
pub mod link;

pub use action::{VizInputAction, VizOutputAction};
pub use brush::BrushState;
pub use focus::FocusSet;
pub use hit::HitZone;
pub use link::{HoverInfo, SelectionBus};
