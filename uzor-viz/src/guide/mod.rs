//! Axis/grid/crosshair/tooltip guides — read [`crate::scale::Scale::ticks`]
//! and [`crate::theme::VizTheme`], draw through the same
//! [`crate::coord::PlotArea`] transform every mark uses.
//!
//! `crosshair`/`tooltip` are the V2 interaction-plane additions (design
//! doc §3, `guide/` block) — both take an already-resolved position from
//! [`crate::interact`], never their own hit-testing.

pub mod axis;
pub mod crosshair;
pub mod grid;
pub mod tooltip;
