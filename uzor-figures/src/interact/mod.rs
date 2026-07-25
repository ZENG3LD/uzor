//! Interaction plane — V2 milestone. Harvested from `mylittlechart`'s
//! input pipeline (`engine/input/**`, `nemo/docs/uzor-engines/
//! mlc_harvest_inventory.md` §4) and `uzor-graph`'s `FocusSet`
//! (`uzor-graph/src/interaction/focus.rs`), generalized: see
//! `nemo/docs/uzor-engines/uzor_figures_engine_architecture.md` §3 (`interact/`
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
//!
//! [`viewport`] (engine-strengthening arc Wave 5, 2026-07-26) adds a THIRD
//! gesture family — pan/zoom/fit over a scale's own DOMAIN (not MLC's
//! bar-index) — following the exact same "small, independently-testable,
//! caller-owned state a figure only ever BORROWS" shape [`brush::BrushState`]/
//! [`focus::FocusSet`] already establish; see that module's own top-level
//! docs for the full design.

pub mod action;
pub mod brush;
pub mod focus;
pub mod hit;
pub mod link;
pub mod viewport;

pub use action::{FigureInputAction, FigureOutputAction};
pub use brush::BrushState;
pub use focus::FocusSet;
pub use hit::HitZone;
pub use link::{HoverInfo, SelectionBus};
pub use viewport::{windowed_scale, OverscrollPolicy, Viewport, ViewportConfig};
