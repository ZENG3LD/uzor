//! Axis/grid/crosshair/tooltip guides — read [`crate::scale::Scale::ticks`]
//! and [`crate::theme::FigureTheme`], draw through the same
//! [`crate::coord::PlotArea`] transform every mark uses.
//!
//! `crosshair`/`tooltip` are the V2 interaction-plane additions (design
//! doc §3, `guide/` block) — both take an already-resolved position from
//! [`crate::interact`], never their own hit-testing. `legend` is the
//! multi-series addition — measured BEFORE plot layout so a figure can
//! shrink its own plot rect by the legend's exact measured size (see
//! `legend`'s own module docs, design law #1). `colorbar` is the
//! continuous-[`crate::scale::color::ColorScale`] counterpart of `legend`
//! — same measure-before-layout discipline, a gradient swatch instead of
//! a discrete list. `labeler` is the harvested bitmap-occupancy 2D label
//! placer (research doc §2, arXiv 2405.10953) — a generalization of
//! `axis`'s own 1D skip-only greedy collision to arbitrary screen-space
//! label rects, used by [`crate::figure::TimelineFigure`]'s point labels
//! and exposed generically for future figures.

pub mod axis;
pub mod colorbar;
pub mod crosshair;
pub mod grid;
pub mod labeler;
pub mod legend;
pub mod tooltip;
