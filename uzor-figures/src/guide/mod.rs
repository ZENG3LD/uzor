//! Axis/grid/crosshair/tooltip guides — read [`crate::scale::Scale::ticks`]
//! and [`crate::theme::FigureTheme`], draw through the same
//! [`crate::coord::PlotArea`] transform every mark uses.
//!
//! `crosshair`/`tooltip` are the V2 interaction-plane additions (design
//! doc §3, `guide/` block) — both take an already-resolved position from
//! [`crate::interact`], never their own hit-testing. `legend` is the
//! multi-series addition — measured BEFORE plot layout so a figure can
//! shrink its own plot rect by the legend's exact measured size (see
//! `legend`'s own module docs, design law #1).

pub mod axis;
pub mod crosshair;
pub mod grid;
pub mod legend;
pub mod tooltip;
