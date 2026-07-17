//! `PieFigure` — pie and donut charts over weighted [`PieSlice`]s.
//!
//! FT/Economist "part-to-whole" hygiene defaults (business-chart research
//! doc §6) are baked in as the ONLY behavior, not opt-in flags: slices are
//! always sorted descending starting at 12 o'clock, sweeping clockwise
//! ([`layout_pie`]); [`resolve_slices`] can additionally fold a long tail
//! into a single "Other" bucket (`top_n_plus_other`, research doc's own
//! harvest item #7) via [`PieFigure::top_n`]; a slice under 3% never gets
//! its own label (its share is small enough that the legend, not the pie
//! itself, is the readable place to see it).
//!
//! Geometry ([`layout_pie`]) is pure and separate from painting (design
//! law #1, same split [`crate::figure::sankey::layout_sankey`] uses) — a
//! hover ([`hit_test_slice`]) resolves through the EXACT SAME angle/radius
//! math the wedges were drawn with, so paint and hit-test can never
//! disagree.
//!
//! **Wedge paths are hand-built cubic beziers ([`append_arc`]), not
//! [`uzor::render::RenderContext`]'s `Painter::arc` primitive** — a real
//! primitive exists and was tried first, but the shared
//! `uzor-render-tiny-skia` backend's own `arc_to_cubics` helper computes
//! its bezier control-point magnitude (`kappa`) as `4/3 * tan(seg_angle /
//! 2)`; the correct constant for a cubic-bezier arc approximation is `4/3
//! * tan(seg_angle / 4)` (the well-known `~0.5523` constant at a 90°
//! quarter-turn, `tan(22.5°)`, not `tan(45°)`) — the shipped formula is
//! roughly 2x too large and visibly turns any wide arc sweep into a
//! faceted, near-polygonal shape rather than a smooth circle (confirmed
//! by direct render: a 6-slice pie came out looking like an irregular
//! hexagon). This is a pre-existing bug in `uzor` core (affects
//! `Painter::arc`/`Painter::ellipse` generally, e.g.
//! `ShapeHelpers::rounded_rect`'s own corner arcs too), out of this
//! crate's own scope to fix (`uzor-figures` only consumes `uzor`, never
//! edits it) — flagged for a follow-up fix in `uzor-render-tiny-skia`
//! itself. Worked around here per this figure's own task brief's fallback
//! clause ("approximate with beziers — standard ... circle math") with a
//! small, self-contained, CORRECT bezier-arc builder ([`append_arc`],
//! 45°-per-segment subdivision, real `4/3 * tan(theta/4)` kappa) that
//! bypasses `Painter::arc` entirely for this figure's own wedge geometry.

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use uzor::render::RenderContext;
use uzor::types::Rect;

use crate::figure::FigureOverlay;
use crate::guide::legend::{self, LegendEntry, LegendPosition};
use crate::guide::tooltip;
use crate::mark::text::{draw_label_centered, draw_label_left_aligned, draw_label_right_aligned};
use crate::scale::linear::format_value;
use crate::theme::FigureTheme;

const MARGIN: f64 = 12.0;
const TITLE_HEIGHT: f64 = 24.0;
const LEGEND_GAP: f64 = 8.0;
/// Room (px) reserved inside the plot rect's own half-extent so an
/// outside-the-pie label never clips the figure's own edge.
const OUTSIDE_LABEL_RESERVE: f64 = 44.0;
/// A slice under this share never gets its own inside/outside label — the
/// legend (when shown) carries it instead (FT hygiene default).
const LABEL_MIN_PCT: f64 = 3.0;
const OUTSIDE_LABEL_GAP: f64 = 10.0;
const INSIDE_LABEL_PAD: f64 = 6.0;
/// Inside labels are drawn in a fixed light color regardless of theme —
/// every palette swatch is mid-to-dark saturation, so white reads reliably
/// on all of them (the usual pie-chart convention), unlike the
/// theme-dependent OUTSIDE label color which sits on the plain background.
const INSIDE_LABEL_COLOR: &str = "#ffffff";
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.22;
const SELECTED_STROKE_WIDTH: f64 = 2.0;
/// Default donut inner-ratio ceiling — never so thin the outer ring
/// collapses to a hairline.
const MAX_DONUT_RATIO: f64 = 0.85;

/// One wedge of caller data: `label` + non-negative `value`. A negative or
/// non-finite `value` is a caller data bug — loudly caught via
/// `debug_assert` in dev builds, degraded to a `0.0`-weight (invisible)
/// slice in release, same convention as
/// [`crate::figure::sankey::SankeyLink`]'s weight guard.
#[derive(Debug, Clone)]
pub struct PieSlice {
    pub label: String,
    pub value: f64,
}

fn clamped_value(v: f64) -> f64 {
    debug_assert!(v.is_finite() && v >= 0.0, "PieSlice value must be finite and non-negative, got {v}");
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

/// FT hygiene transform: sort `slices` descending by value; when `top_n`
/// is `Some(n)` and there are more than `n` slices, keep the top `n`
/// individually and fold the remainder into one trailing `"Other"` slice
/// (appended LAST, never re-sorted into the middle — "Other" is a
/// deliberate catch-all bucket, not a data value competing on its own
/// merits). A `top_n` at or above the input length is a no-op beyond the
/// sort. Pure and independently testable (design law #1's "separate from
/// painting" extended to this pre-layout data transform too).
pub fn resolve_slices(slices: &[PieSlice], top_n: Option<usize>) -> Vec<PieSlice> {
    let mut sorted: Vec<PieSlice> = slices.to_vec();
    sorted.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    match top_n {
        Some(n) if sorted.len() > n => {
            let mut head: Vec<PieSlice> = sorted[..n].to_vec();
            let other_sum: f64 = sorted[n..].iter().map(|s| clamped_value(s.value)).sum();
            if other_sum > 0.0 {
                head.push(PieSlice { label: "Other".to_owned(), value: other_sum });
            }
            head
        }
        _ => sorted,
    }
}

/// One resolved slice's own angular geometry — `start_angle`/`end_angle`
/// in radians, `-FRAC_PI_2` (12 o'clock) as the baseline, increasing
/// CLOCKWISE (matches every other arc call already in this crate, e.g.
/// [`uzor::render::painter::Painter::rounded_rect`]'s corner arcs: screen
/// space has y pointing down, so an increasing angle sweeps clockwise).
#[derive(Debug, Clone)]
pub struct PieSliceGeom {
    pub label: String,
    pub value: f64,
    /// Share of the whole, `0.0..=100.0`.
    pub pct: f64,
    pub start_angle: f64,
    pub end_angle: f64,
}

/// Pure layout geometry for a [`PieFigure`] — see the module docs for why
/// this is separate from painting.
#[derive(Debug, Clone)]
pub struct PieLayout {
    pub cx: f64,
    pub cy: f64,
    pub r_outer: f64,
    pub r_inner: f64,
    pub slices: Vec<PieSliceGeom>,
}

/// Lay `slices` (already [`resolve_slices`]-resolved by the caller — this
/// function does NOT sort/fold, it only turns values into angles) out as a
/// pie (`donut_inner_ratio <= 0.0`) or donut, centered in `rect`.
/// `slices` summing to `0.0` (all-zero or empty input) produces an empty
/// slice list rather than dividing by zero.
pub fn layout_pie(slices: &[PieSlice], donut_inner_ratio: f64, rect: Rect) -> PieLayout {
    let cx = rect.center_x();
    let cy = rect.center_y();
    let r_outer = (rect.width.min(rect.height) / 2.0).max(0.0);
    let r_inner = r_outer * donut_inner_ratio.clamp(0.0, MAX_DONUT_RATIO);

    let total: f64 = slices.iter().map(|s| clamped_value(s.value)).sum();
    if total <= 0.0 {
        return PieLayout { cx, cy, r_outer, r_inner, slices: Vec::new() };
    }

    let mut cumulative = -FRAC_PI_2;
    let geoms = slices
        .iter()
        .map(|s| {
            let v = clamped_value(s.value);
            let frac = v / total;
            let start = cumulative;
            let end = start + frac * TAU;
            cumulative = end;
            PieSliceGeom { label: s.label.clone(), value: v, pct: frac * 100.0, start_angle: start, end_angle: end }
        })
        .collect();

    PieLayout { cx, cy, r_outer, r_inner, slices: geoms }
}

/// Index of the slice under screen point `(px, py)` — resolves through the
/// EXACT SAME angle convention [`layout_pie`] produced (design law #1), so
/// a caller's own hover routing can never disagree with what got drawn.
/// `None` outside the ring (past `r_outer`, or inside `r_inner` for a
/// donut).
pub fn hit_test_slice(layout: &PieLayout, px: f64, py: f64) -> Option<usize> {
    let dx = px - layout.cx;
    let dy = py - layout.cy;
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > layout.r_outer || dist < layout.r_inner {
        return None;
    }
    let raw = dy.atan2(dx);
    let base = -FRAC_PI_2;
    let angle = base + (raw - base).rem_euclid(TAU);
    layout.slices.iter().position(|s| angle >= s.start_angle - 1e-9 && angle < s.end_angle + 1e-9)
}

/// How [`append_arc`] connects its arc's own start point to whatever the
/// current path point already is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArcConnect {
    /// Fresh path (or subpath) — jump to the arc's start with no line.
    MoveTo,
    /// Continue the current path with a straight line to the arc's start
    /// (e.g. the radial connector from a donut's outer end point to its
    /// inner ring's own start point).
    LineTo,
}

/// Cap on how much angle (radians) one cubic-bezier segment covers — small
/// enough that the CORRECT kappa formula (see the module docs) stays a
/// faithful circle approximation regardless of how wide the slice's own
/// total sweep is.
const MAX_SEGMENT_ANGLE: f64 = FRAC_PI_4;

/// Append a circular arc from `start_angle` to `end_angle` (any sign of
/// sweep — a donut's inner ring is walked backward, `end_angle <
/// start_angle`) around `(cx, cy)` at `radius`, as one or more
/// [`Painter::bezier_curve_to`](uzor::render::painter::Painter::bezier_curve_to)
/// calls with a CORRECT `4/3 * tan(theta/4)` kappa per segment — see the
/// module docs for why this hand-rolled builder exists instead of
/// `Painter::arc`. A zero-magnitude sweep is a no-op (no degenerate
/// zero-length curve emitted).
fn append_arc(ctx: &mut dyn RenderContext, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64, connect: ArcConnect) {
    let sweep = end_angle - start_angle;
    if sweep.abs() < 1e-9 {
        return;
    }
    let n_segs = ((sweep.abs() / MAX_SEGMENT_ANGLE).ceil() as usize).max(1);
    let seg_angle = sweep / n_segs as f64;
    let kappa = (4.0 / 3.0) * (seg_angle / 4.0).tan();

    let start_x = cx + radius * start_angle.cos();
    let start_y = cy + radius * start_angle.sin();
    match connect {
        ArcConnect::MoveTo => ctx.move_to(start_x, start_y),
        ArcConnect::LineTo => ctx.line_to(start_x, start_y),
    }

    let mut a = start_angle;
    for _ in 0..n_segs {
        let a1 = a + seg_angle;
        let (cos_a, sin_a) = (a.cos(), a.sin());
        let (cos_a1, sin_a1) = (a1.cos(), a1.sin());
        let p0x = cx + radius * cos_a;
        let p0y = cy + radius * sin_a;
        let p3x = cx + radius * cos_a1;
        let p3y = cy + radius * sin_a1;
        let cp1x = p0x - kappa * radius * sin_a;
        let cp1y = p0y + kappa * radius * cos_a;
        let cp2x = p3x + kappa * radius * sin_a1;
        let cp2y = p3y - kappa * radius * cos_a1;
        ctx.bezier_curve_to(cp1x, cp1y, cp2x, cp2y, p3x, p3y);
        a = a1;
    }
}

/// Append (no fill/stroke yet) one slice's own closed wedge path — a full
/// pie wedge (center -> outer arc -> back to center) when `r_inner <= 0`,
/// or a donut annulus wedge (outer arc forward, radial connector, inner
/// arc backward, radial connector closing back to the outer start) when
/// `r_inner > 0`.
fn build_wedge_path(ctx: &mut dyn RenderContext, layout: &PieLayout, slice: &PieSliceGeom) {
    ctx.begin_path();
    if layout.r_inner > 0.0 {
        append_arc(ctx, layout.cx, layout.cy, layout.r_outer, slice.start_angle, slice.end_angle, ArcConnect::MoveTo);
        append_arc(ctx, layout.cx, layout.cy, layout.r_inner, slice.end_angle, slice.start_angle, ArcConnect::LineTo);
    } else {
        ctx.move_to(layout.cx, layout.cy);
        append_arc(ctx, layout.cx, layout.cy, layout.r_outer, slice.start_angle, slice.end_angle, ArcConnect::LineTo);
    }
    ctx.close_path();
}

/// A pie (or, via [`PieFigure::donut`], donut) chart: weighted [`PieSlice`]s
/// swept clockwise from 12 o'clock, descending by value.
pub struct PieFigure {
    pub slices: Vec<PieSlice>,
    donut_inner_ratio: f64,
    legend_position: Option<LegendPosition>,
    title: Option<String>,
    top_n: Option<usize>,
}

impl PieFigure {
    pub fn new(slices: Vec<PieSlice>) -> Self {
        Self { slices, donut_inner_ratio: 0.0, legend_position: None, title: None, top_n: None }
    }

    /// Draw as a donut with `inner_ratio` (clamped `0.0..=0.85`) of
    /// `r_outer` cut out of the center.
    pub fn donut(mut self, inner_ratio: f64) -> Self {
        self.donut_inner_ratio = inner_ratio.clamp(0.0, MAX_DONUT_RATIO);
        self
    }

    pub fn with_legend(mut self, position: LegendPosition) -> Self {
        self.legend_position = Some(position);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Fold every slice beyond the top `n` (by value) into a trailing
    /// "Other" bucket — see [`resolve_slices`].
    pub fn top_n(mut self, n: usize) -> Self {
        self.top_n = Some(n);
        self
    }

    /// This figure's own resolved (sorted + top-N-folded) slice list —
    /// exposed so a caller driving hover/legend from outside computes the
    /// SAME order this figure paints with.
    pub fn resolved_slices(&self) -> Vec<PieSlice> {
        resolve_slices(&self.slices, self.top_n)
    }

    fn base_plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
        Rect::new(
            rect.x + MARGIN,
            rect.y + title_h + MARGIN,
            (rect.width - MARGIN * 2.0).max(0.0),
            (rect.height - title_h - MARGIN * 2.0).max(0.0),
        )
    }

    /// This figure's plot rect for `rect` — same caveat as
    /// [`crate::figure::BarFigure::plot_area`]: does NOT account for a
    /// legend (measuring one needs live text metrics); `render_with`
    /// shrinks the SAME base rect internally via its own `ctx`, so a
    /// legend-bearing figure's own hover/tooltip/legend stay mutually
    /// consistent within one `render_with` call regardless.
    pub fn plot_rect(&self, rect: Rect) -> Rect {
        self.base_plot_rect(rect)
    }

    /// This figure's own layout geometry for `rect` (using
    /// [`PieFigure::plot_rect`], i.e. WITHOUT accounting for a legend —
    /// same caveat as [`PieFigure::plot_rect`] itself).
    pub fn layout(&self, rect: Rect) -> PieLayout {
        layout_pie(&self.resolved_slices(), self.donut_inner_ratio, self.plot_rect(rect))
    }

    fn legend_entries(&self, resolved: &[PieSlice], theme: &FigureTheme) -> Vec<LegendEntry> {
        resolved
            .iter()
            .enumerate()
            .map(|(i, s)| LegendEntry { label: s.label.clone(), color: theme.palette[i % theme.palette.len()].clone() })
            .collect()
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a
    /// wedge brightens it and shows a label/value/percentage tooltip;
    /// `overlay.focus`-selected slices (keyed by index into
    /// [`PieFigure::resolved_slices`]) get a persistent accent outline.
    /// `overlay.brush` is not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let resolved = self.resolved_slices();
        let base_rect = self.base_plot_rect(rect);
        let legend_position = self.legend_position;
        let legend_entries = if legend_position.is_some() { self.legend_entries(&resolved, theme) } else { Vec::new() };

        let (plot_rect, legend_rect) = match legend_position {
            Some(pos) if !legend_entries.is_empty() => {
                let size = legend::measure_legend(ctx, theme, &legend_entries, pos, base_rect.width);
                match pos {
                    LegendPosition::Top => {
                        let reserved = size.height + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y + reserved, base_rect.width, (base_rect.height - reserved).max(0.0)),
                            Some((Rect::new(base_rect.x, base_rect.y, base_rect.width, size.height), pos)),
                        )
                    }
                    LegendPosition::Bottom => {
                        let reserved = size.height + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y, base_rect.width, (base_rect.height - reserved).max(0.0)),
                            Some((Rect::new(base_rect.x, base_rect.bottom() - size.height, base_rect.width, size.height), pos)),
                        )
                    }
                    LegendPosition::Right => {
                        let reserved = size.width + LEGEND_GAP;
                        (
                            Rect::new(base_rect.x, base_rect.y, (base_rect.width - reserved).max(0.0), base_rect.height),
                            Some((Rect::new(base_rect.right() - size.width, base_rect.y, size.width, base_rect.height), pos)),
                        )
                    }
                }
            }
            _ => (base_rect, None),
        };

        // Reserve breathing room for outside labels — shrink the disc
        // itself, not the plot rect, so the legend math above stays
        // simple (plot_rect already accounts for the legend band).
        let disc_rect = Rect::new(
            plot_rect.x + OUTSIDE_LABEL_RESERVE.min(plot_rect.width / 2.0),
            plot_rect.y + OUTSIDE_LABEL_RESERVE.min(plot_rect.height / 2.0),
            (plot_rect.width - OUTSIDE_LABEL_RESERVE.min(plot_rect.width / 2.0) * 2.0).max(0.0),
            (plot_rect.height - OUTSIDE_LABEL_RESERVE.min(plot_rect.height / 2.0) * 2.0).max(0.0),
        );

        let layout = layout_pie(&resolved, self.donut_inner_ratio, disc_rect);
        let hovered = overlay.hover_px.and_then(|(hx, hy)| hit_test_slice(&layout, hx, hy));

        for (i, slice) in layout.slices.iter().enumerate() {
            ctx.set_fill_color(theme.palette[i % theme.palette.len()].as_str());
            build_wedge_path(ctx, &layout, slice);
            ctx.fill();
        }

        for (i, slice) in layout.slices.iter().enumerate() {
            draw_slice_label(ctx, &layout, slice, theme);

            if hovered == Some(i) {
                build_wedge_path(ctx, &layout, slice);
                ctx.set_fill_color("#ffffff");
                ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
                ctx.fill();
                ctx.set_global_alpha(1.0);
            }

            if let Some(focus) = overlay.focus {
                if focus.is_selected(i as u64) {
                    build_wedge_path(ctx, &layout, slice);
                    ctx.set_stroke_color(&theme.palette[1 % theme.palette.len()]);
                    ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                    ctx.stroke();
                }
            }
        }

        if let (Some(i), Some((hx, hy))) = (hovered, overlay.hover_px) {
            if let Some(slice) = layout.slices.get(i) {
                let lines = vec![
                    ("label".to_owned(), slice.label.clone()),
                    ("value".to_owned(), format_value(slice.value, 1.0)),
                    ("pct".to_owned(), format!("{:.1}%", slice.pct)),
                ];
                tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, plot_rect);
            }
        }

        if let Some((legend_rect, pos)) = legend_rect {
            legend::draw_legend(ctx, legend_rect, theme, &legend_entries, pos);
        }

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}

/// Draw slice's own percentage label — inside (centered at the ring's mid
/// radius) when the measured text fits the slice's own chord width at
/// that radius, else outside beside the wedge with NO leader line (FT
/// hygiene default: a leader-line-free outside label, not a pointer).
/// Slices under [`LABEL_MIN_PCT`] draw no label at all (legend carries
/// them).
fn draw_slice_label(ctx: &mut dyn RenderContext, layout: &PieLayout, slice: &PieSliceGeom, theme: &FigureTheme) {
    if slice.pct < LABEL_MIN_PCT {
        return;
    }
    let mid_angle = (slice.start_angle + slice.end_angle) / 2.0;
    let text = format!("{:.1}%", slice.pct);

    ctx.set_font(&theme.label_font);
    let text_w = ctx.measure_text(&text);

    let mid_r = (layout.r_inner + layout.r_outer) / 2.0;
    let half_span = (slice.end_angle - slice.start_angle) / 2.0;
    let chord = 2.0 * mid_r * half_span.sin().abs();

    if text_w + INSIDE_LABEL_PAD <= chord {
        let x = layout.cx + mid_r * mid_angle.cos();
        let y = layout.cy + mid_r * mid_angle.sin();
        draw_label_centered(ctx, &text, x, y, INSIDE_LABEL_COLOR, &theme.label_font);
    } else {
        let r = layout.r_outer + OUTSIDE_LABEL_GAP;
        let x = layout.cx + r * mid_angle.cos();
        let y = layout.cy + r * mid_angle.sin();
        if mid_angle.cos() >= 0.0 {
            draw_label_left_aligned(ctx, &text, x, y, &theme.label_color, &theme.label_font);
        } else {
            draw_label_right_aligned(ctx, &text, x, y, &theme.label_color, &theme.label_font);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(label: &str, value: f64) -> PieSlice {
        PieSlice { label: label.to_owned(), value }
    }

    #[test]
    fn resolve_slices_sorts_descending_by_value() {
        let input = vec![slice("a", 5.0), slice("b", 20.0), slice("c", 10.0)];
        let resolved = resolve_slices(&input, None);
        let values: Vec<f64> = resolved.iter().map(|s| s.value).collect();
        assert_eq!(values, vec![20.0, 10.0, 5.0]);
    }

    #[test]
    fn resolve_slices_top_n_folds_the_tail_into_a_trailing_other_bucket() {
        let input = vec![slice("a", 50.0), slice("b", 30.0), slice("c", 10.0), slice("d", 6.0), slice("e", 4.0)];
        let resolved = resolve_slices(&input, Some(2));
        assert_eq!(resolved.len(), 3, "top 2 + one Other bucket");
        assert_eq!(resolved[0].label, "a");
        assert_eq!(resolved[1].label, "b");
        assert_eq!(resolved[2].label, "Other");
        assert!((resolved[2].value - 20.0).abs() < 1e-9, "Other must sum the folded tail (10+6+4)");
    }

    #[test]
    fn resolve_slices_top_n_at_or_above_length_is_a_pure_sort_no_other_bucket() {
        let input = vec![slice("a", 5.0), slice("b", 20.0)];
        let resolved = resolve_slices(&input, Some(5));
        assert_eq!(resolved.len(), 2);
        assert!(resolved.iter().all(|s| s.label != "Other"));
    }

    #[test]
    fn layout_pie_slice_angles_sum_to_a_full_turn() {
        let slices = vec![slice("a", 1.0), slice("b", 2.0), slice("c", 3.0)];
        let layout = layout_pie(&slices, 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        let total_span: f64 = layout.slices.iter().map(|s| s.end_angle - s.start_angle).sum();
        assert!((total_span - TAU).abs() < 1e-9);
    }

    #[test]
    fn layout_pie_first_slice_starts_exactly_at_12_oclock() {
        let slices = vec![slice("a", 1.0), slice("b", 1.0)];
        let layout = layout_pie(&slices, 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!((layout.slices[0].start_angle - (-FRAC_PI_2)).abs() < 1e-9);
    }

    #[test]
    fn layout_pie_descending_input_places_the_largest_slice_first_clockwise() {
        // resolve_slices + layout_pie composed: the biggest slice owns the
        // angular range starting AT 12 o'clock.
        let resolved = resolve_slices(&[slice("small", 10.0), slice("big", 90.0)], None);
        let layout = layout_pie(&resolved, 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(layout.slices[0].label, "big");
        assert!((layout.slices[0].start_angle - (-FRAC_PI_2)).abs() < 1e-9);
        // "big" is 90% of the total -> spans 0.9 * TAU.
        assert!((layout.slices[0].end_angle - layout.slices[0].start_angle - 0.9 * TAU).abs() < 1e-6);
    }

    #[test]
    fn layout_pie_empty_or_all_zero_input_never_panics() {
        let layout = layout_pie(&[], 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!(layout.slices.is_empty());
        let layout = layout_pie(&[slice("a", 0.0), slice("b", 0.0)], 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!(layout.slices.is_empty());
    }

    #[test]
    fn donut_inner_ratio_is_clamped_and_produces_a_nonzero_inner_radius() {
        let slices = vec![slice("a", 1.0)];
        let layout = layout_pie(&slices, 0.5, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!(layout.r_inner > 0.0 && layout.r_inner < layout.r_outer);
        // Absurd input clamps rather than collapsing the ring.
        let layout = layout_pie(&slices, 5.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert!(layout.r_inner / layout.r_outer <= MAX_DONUT_RATIO + 1e-9);
    }

    #[test]
    fn hit_test_slice_at_each_slices_own_mid_angle_and_mid_radius_matches_the_drawn_slice() {
        let slices = vec![slice("a", 1.0), slice("b", 1.0), slice("c", 2.0)];
        let layout = layout_pie(&slices, 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        for (i, s) in layout.slices.iter().enumerate() {
            let mid_angle = (s.start_angle + s.end_angle) / 2.0;
            let mid_r = layout.r_outer * 0.5;
            let px = layout.cx + mid_r * mid_angle.cos();
            let py = layout.cy + mid_r * mid_angle.sin();
            assert_eq!(hit_test_slice(&layout, px, py), Some(i), "point at slice {i}'s own mid-angle/mid-radius must resolve to slice {i}");
        }
    }

    #[test]
    fn hit_test_slice_outside_the_outer_radius_is_none() {
        let slices = vec![slice("a", 1.0)];
        let layout = layout_pie(&slices, 0.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(hit_test_slice(&layout, layout.cx + layout.r_outer * 5.0, layout.cy), None);
    }

    #[test]
    fn hit_test_slice_inside_the_inner_radius_of_a_donut_is_none() {
        let slices = vec![slice("a", 1.0)];
        let layout = layout_pie(&slices, 0.5, Rect::new(0.0, 0.0, 200.0, 200.0));
        assert_eq!(hit_test_slice(&layout, layout.cx, layout.cy), None, "the donut's own hole must never hit-test to a slice");
    }

    #[test]
    fn empty_figure_renders_without_panicking() {
        let figure = PieFigure::new(Vec::new());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 200, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 200.0), &theme);
        });
        assert!(result.is_ok(), "an empty pie figure must render without panicking");
    }
}
