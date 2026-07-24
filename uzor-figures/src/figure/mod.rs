//! Composed figures — thin, in-crate compositions of scale + coord + mark
//! + guide. Each figure is a pure `render(&self, ctx, rect, theme)` (V1)
//! or `render_with(&self, ctx, rect, theme, overlay: &FigureOverlay)` (V2)
//! over borrowed data; a figure never retains interaction state itself —
//! [`FigureOverlay`] is borrowed per-frame input, not owned (design law #3).
//! A real registry/IR (mlc's `ChartTypeDef`+`DrawOps` two-table pattern)
//! is a later milestone — see the crate-root docs.
//!
//! `pie`/`waterfall`/`heatmap` (V4, business-chart set) apply the FT/
//! Economist chart-hygiene rules from
//! `nemo/docs/uzor-engines/research_dataviz_sota_2026.md` §6 as DEFAULT
//! behavior, not opt-in flags — see each module's own docs.
//!
//! `scatter`/`boxplot`/`kpi` (typography-gap WAVE 4, statistical/business
//! set) round out the crate: [`scatter::ScatterFigure`] (x/y point cloud,
//! deterministic uniform-stride thinning — NOT LTTB, see its own module
//! docs for why), [`boxplot::BoxplotFigure`] (Tukey `1.5 * IQR` quartile
//! summary), and [`kpi::KpiFigure`] (dashboard number tile) — all three
//! opt into [`crate::guide::annotation`]'s reference lines/bands/callouts
//! where applicable (curve/bars gained the SAME opt-in `annotations`
//! field this wave).
//!
//! `dag` (arc B wave 2 shelf item): [`dag::DagFigure`] — a layered DAG
//! figure over [`dag::layering`], the CANONICAL Kahn longest-path +
//! barycenter layering algorithm for this workspace (this crate must not
//! depend on `uzor-graph` — see `dag`'s own module docs for the full
//! provenance and its documented v1 scope boundaries). `uzor-graph`
//! re-points its own `layout::layering` onto this module (2026-07-22)
//! instead of keeping a duplicate copy.

pub mod bars;
pub mod boxplot;
pub mod curve;
pub mod dag;
pub mod heatmap;
pub mod histogram;
pub mod kpi;
pub mod pie;
pub mod sankey;
pub mod scatter;
pub mod timeline;
pub mod waterfall;

pub use bars::{BarFigure, BarMode, BarSeries};
pub use boxplot::{boxplot_stats, quartile, BoxplotFigure, BoxplotStats, WHISKER_IQR_MULTIPLIER};
pub use curve::{CurveFigure, CurveSeries};
pub use dag::{hit_test_dag_node, layout_dag, DagEdge, DagEdgeGeom, DagFigure, DagLayout, DagNode};
pub use heatmap::{hit_test_cell, layout_heatmap, HeatmapCell, HeatmapFigure, HeatmapLayout};
pub use histogram::{bin, Bin, HistogramFigure};
pub use kpi::KpiFigure;
pub use pie::{hit_test_slice, layout_pie, resolve_slices, PieFigure, PieLayout, PieSlice, PieSliceGeom};
pub use sankey::{hit_test_node, layout_sankey, LabelSide, RibbonGeom, SankeyFigure, SankeyLayout, SankeyLink, SankeyNode};
pub use scatter::{uniform_thin_indices, PointRadius, ScatterFigure, ScatterPoint};
pub use timeline::{layout_point_labels, PointLabelInput, TimelineEvent, TimelineFigure};
pub use waterfall::{
    compute_steps, layout_bars, layout_connectors, WaterfallBar, WaterfallConnector, WaterfallFigure, WaterfallItem, WaterfallKind,
    WaterfallStep,
};

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::interact::focus::FocusSet;
use crate::theme::FigureTheme;

/// Borrowed per-frame interaction state a figure may optionally react to.
/// The default (`hover_px: None, brush: None, focus: None`) reproduces
/// exactly the old stateless V1 render — `render(ctx, rect, theme)` is
/// literally `render_with(ctx, rect, theme, &FigureOverlay::default())`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FigureOverlay<'a> {
    /// Cursor position in the SAME absolute pixel space as the `rect`
    /// passed to `render_with`. A figure hit-tests this against its own
    /// [`crate::coord::PlotArea`] internally (via
    /// [`crate::interact::hit::hit_zone`]) — a position outside that
    /// figure's own plot rect is simply ignored, so fanning the same
    /// value out to every panel in a multi-figure layout is harmless.
    pub hover_px: Option<(f64, f64)>,
    /// Domain-space `(min, max)` interval — e.g. the committed value from
    /// [`crate::interact::brush::BrushState::domain_interval`] on
    /// whichever figure owns the drag. A figure that supports
    /// brush-linked highlighting re-derives ITS OWN screen position for
    /// this interval through its own scale — figures may live in entirely
    /// different domains (see the linked-brush demo), so the dragging
    /// figure's raw pixels are never threaded through directly.
    pub brush: Option<(f64, f64)>,
    /// Optional persistent hover/selection set (click-to-pin, future
    /// cross-figure linking). `None` for figures that don't use it.
    pub focus: Option<&'a FocusSet>,
}

/// How a figure's own auto-computed Y domain treats the zero baseline —
/// shared vocabulary so a caller reasons about "does this figure force
/// zero" identically regardless of which figure it's configuring.
/// [`crate::figure::CurveFigure::with_y_domain_policy`] is currently the
/// only builder exposing it (default [`YDomainPolicy::ForceZero`]
/// reproduces this crate's own pre-existing forced-zero behavior
/// byte-for-byte — see that builder's own doc comment for why zero
/// forcing was never actually optional before this).
///
/// [`crate::figure::ScatterFigure`]/[`crate::figure::BoxplotFigure`]'s own
/// Y domains are ALREADY, unconditionally, [`YDomainPolicy::FitData`]
/// (their own module docs explain why a bar/area's height-from-zero
/// convention doesn't apply to a point/box's position) — named here as
/// the SAME vocabulary for documentation purposes; neither figure has a
/// builder to change it, since nothing today asks for the opposite on
/// either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YDomainPolicy {
    /// Always widen the domain to include `0.0` — a bar/filled-area's
    /// height encodes area-from-zero.
    #[default]
    ForceZero,
    /// Fit the domain to the data's own extent only, never pulling in
    /// zero — a line/point/box's position does not encode area-from-zero.
    FitData,
}

/// Left inset (px) of a figure's own title from `rect.x` — see
/// [`draw_title`]'s own doc comment for why this is larger than the
/// pre-existing `8.0`.
const TITLE_LEFT_INSET: f64 = 14.0;
/// Top inset (px) of a figure's own title from `rect.y`.
const TITLE_TOP_INSET: f64 = 6.0;

/// Shared title-bar draw shared by every figure in this module — top-left,
/// one line, `theme.label_font`/`theme.label_color`.
///
/// **Owner defect report investigated (report, not silently fixed by
/// guesswork):** "the scatter figure's title has its first glyph(s)
/// clipped by the figure rect's left edge" at high PDF zoom. Root-caused
/// via a pixel-level decode of the regenerated raster proof PNGs (this
/// crate's own `figures_scatter.png`, and an isolated `uzor-typeset`
/// repro matching the showcase's exact heading+spacer+figure block
/// sequence) — in BOTH, the title's own ink starts with a clean,
/// unclipped gap before the figure rect's left edge (verified by decoding
/// the PNG's own raw pixel rows: no ink at all in the pre-title columns).
/// **No literal glyph-clipping bug exists in this code path.** The
/// pre-existing `8.0`px inset was nonetheless visibly TIGHTER than every
/// other left-side chrome margin this crate's figures use (`MARGIN_LEFT`
/// on an axis-bearing figure runs 48-90px; even the annotation guide's own
/// `REFERENCE_LABEL_GAP` inside the plot is a comparable few px past a
/// much larger existing inset) — at the zoom level a "crop tightly around
/// just the figure" screenshot implies, an 8px gap reads as "hugging the
/// edge," which is almost certainly what prompted the report. Bumped to a
/// named, more generous constant for real visual breathing room, not a
/// blind "add a few more px" guess.
pub(crate) fn draw_title(ctx: &mut dyn RenderContext, rect: Rect, title: &str, theme: &FigureTheme) {
    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Top);
    ctx.fill_text(title, rect.x + TITLE_LEFT_INSET, rect.y + TITLE_TOP_INSET);
}
