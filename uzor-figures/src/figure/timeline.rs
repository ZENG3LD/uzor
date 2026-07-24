//! `TimelineFigure` — an event strip/ribbon over a calendar time axis: one
//! horizontal lane per named track, point events drawn as circular markers
//! with adjacent (collision-skipped) labels, interval events drawn as
//! rounded bars spanning `[ts, end_ts]` with an inside-or-beside label.
//!
//! Report figure #2 (owner-requested timeline, verbatim: "Таймлайн всей
//! операции... единой лентой") and #11 (infra-wave timeline) both consume
//! this one generic figure — no case-specific vocabulary lives here, only
//! `lane`/`kind`/`label` indices the caller assigns meaning to.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::labeler::{self, OccupancyBitmap};
use crate::guide::{axis, tooltip};
use crate::interact::hit::{self, HitZone};
use crate::mark::text::{draw_label_centered, draw_label_left_aligned, draw_label_right_aligned};
use crate::scale::time::boundary_weight;
use crate::scale::{BandScale, Scale, TimeScale};
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 90.0;
const MARGIN_RIGHT: f64 = 16.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const TARGET_X_TICKS: usize = 6;
/// Inner padding (fraction of each lane's row height) between adjacent
/// lane strips — same convention as [`crate::figure::bars::BarFigure`]'s
/// `BAND_PADDING`, rotated onto the row axis.
const LANE_PADDING: f64 = 0.15;
/// Fraction of the raw `[min_ts, max_ts]` event span added as padding on
/// each side of [`TimelineFigure::time_scale`]'s domain, so edge events
/// never sit exactly on the plot's left/right border.
const TIME_PADDING_FRACTION: f64 = 0.04;
/// Minimum padding (seconds, 12h) applied even to a near-zero-span/single-
/// event domain, so a lone event still gets visible breathing room instead
/// of a degenerate zero-width scale.
const MIN_TIME_PADDING_SECS: f64 = 43_200.0;
/// Radius (px) of a point-event marker.
const POINT_RADIUS: f64 = 5.0;
/// Height (px) of an interval-event bar within its lane strip (clamped to
/// the strip's own height when the strip is narrower).
const BAR_HEIGHT: f64 = 14.0;
const BAR_CORNER_RADIUS: f64 = 4.0;
/// Horizontal gap (px) between a marker/bar edge and its beside-label.
const LABEL_GAP: f64 = 6.0;
const HOVER_HIGHLIGHT_ALPHA: f64 = 0.28;
const SELECTED_STROKE_WIDTH: f64 = 2.0;
/// Screen-pixel slack added around a marker/bar's own geometry for
/// [`hit_event_at`]'s hover hit-test — a marker's true radius or a bar's
/// exact edge is an unforgivingly small target otherwise.
const HIT_TOLERANCE: f64 = 3.0;
const CROSSHAIR_DASH: [f64; 2] = [4.0, 3.0];
const CROSSHAIR_LABEL_PAD: f64 = 3.0;

/// One timeline event: a point (`end_ts: None`) or an interval
/// (`end_ts: Some(..)`) placed on lane `lane` (an index into
/// [`TimelineFigure::lane_names`]) at unix-second timestamp(s). `kind`
/// indexes [`FigureTheme::palette`] for this event's color.
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub ts: f64,
    pub end_ts: Option<f64>,
    pub lane: usize,
    pub label: String,
    pub kind: usize,
}

impl TimelineEvent {
    pub fn is_interval(&self) -> bool {
        self.end_ts.is_some()
    }
}

/// An event-strip/ribbon timeline: one horizontal lane per name in
/// `lane_names`, events placed by unix-second timestamp on a calendar
/// [`TimeScale`] x-axis.
pub struct TimelineFigure {
    pub events: Vec<TimelineEvent>,
    pub lane_names: Vec<String>,
    pub title: String,
}

impl TimelineFigure {
    pub fn new(events: Vec<TimelineEvent>, lane_names: Vec<String>) -> Self {
        Self { events, lane_names, title: String::new() }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    fn plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_empty() { 0.0 } else { TITLE_HEIGHT };
        Rect::new(
            rect.x + MARGIN_LEFT,
            rect.y + title_h,
            (rect.width - MARGIN_LEFT - MARGIN_RIGHT).max(0.0),
            (rect.height - title_h - MARGIN_BOTTOM).max(0.0),
        )
    }

    /// This figure's plot-area transform for `rect` — exposed for the
    /// same reason as [`crate::figure::CurveFigure::plot_area`]: a caller
    /// driving hover/click routing from outside needs to hit-test through
    /// the EXACT same transform this figure renders with.
    pub fn plot_area(&self, rect: Rect) -> PlotArea {
        PlotArea::new(self.plot_rect(rect))
    }

    /// This figure's own lane row scale (always constructible, even for
    /// zero lanes) — exposed for the same reason as
    /// [`TimelineFigure::plot_area`].
    pub fn lane_scale(&self) -> BandScale {
        BandScale::new(self.lane_names.clone(), LANE_PADDING)
    }

    /// This figure's nice-padded [`TimeScale`] domain over every event's
    /// `ts`/`end_ts` — `None` when there are no events (nothing to plot).
    pub fn time_scale(&self) -> Option<TimeScale> {
        time_domain(&self.events).map(|(min_ts, max_ts)| TimeScale::new(min_ts, max_ts))
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over an
    /// event draws a vertical time crosshair + brightens that event +
    /// shows a lane/label/time tooltip; `overlay.focus`-selected events
    /// (keyed by their index into [`TimelineFigure::events`]) get a
    /// persistent accent outline, the same convention
    /// [`crate::figure::BarFigure`] uses keyed by category index.
    /// `overlay.brush` is not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let area = self.plot_area(rect);

        if let Some(time_scale) = self.time_scale() {
            let lanes = self.lane_scale();

            self.draw_time_grid(ctx, &area, &time_scale, theme);
            self.draw_lane_dividers(ctx, &area, &lanes, theme);
            self.draw_intervals(ctx, &area, &time_scale, &lanes, theme);
            self.draw_points(ctx, &area, &time_scale, &lanes, theme);

            if let Some(focus) = overlay.focus {
                let accent = palette_color(theme, 1);
                for (i, e) in self.events.iter().enumerate() {
                    if e.lane >= lanes.len() || !focus.is_selected(i as u64) {
                        continue;
                    }
                    let (top, bottom) = area.y_band(&lanes, e.lane);
                    ctx.set_stroke_color(accent);
                    ctx.set_stroke_width(SELECTED_STROKE_WIDTH);
                    match e.end_ts {
                        Some(end_ts) => {
                            let (left, right, bar_y, bar_h) = interval_bar_geometry(&area, &time_scale, e.ts, end_ts, top, bottom);
                            ctx.stroke_rounded_rect(left, bar_y, (right - left).max(0.0), bar_h, BAR_CORNER_RADIUS);
                        }
                        None => {
                            let cy = (top + bottom) / 2.0;
                            let x = area.x(&time_scale, e.ts);
                            ctx.begin_path();
                            ctx.arc(x, cy, POINT_RADIUS + 2.0, 0.0, std::f64::consts::TAU);
                            ctx.stroke();
                        }
                    }
                }
            }

            if let Some((hx, hy)) = overlay.hover_px {
                if hit::hit_zone(&area, hx, hy) == HitZone::Plot {
                    self.draw_hover(ctx, &area, &time_scale, &lanes, theme, hx, hy);
                }
            }

            axis::draw_x_axis(ctx, &area, &time_scale, theme, TARGET_X_TICKS);
            self.draw_lane_labels(ctx, &area, &lanes, theme);
        }

        if !self.title.is_empty() {
            crate::figure::draw_title(ctx, rect, &self.title, theme);
        }
    }

    /// Vertical calendar gridlines at each [`TimeScale`] tick — like
    /// [`crate::guide::grid::draw_x_grid`], but a major calendar boundary
    /// (year/month, via [`boundary_weight`]) draws a touch stronger than a
    /// minor one, since the tick's own weight is already cheaply available
    /// right here (the shared `grid::draw_x_grid` has no notion of tick
    /// weight — only [`TimeScale`] carries one).
    fn draw_time_grid(&self, ctx: &mut dyn RenderContext, area: &PlotArea, time_scale: &TimeScale, theme: &FigureTheme) {
        let ticks = time_scale.ticks(TARGET_X_TICKS);
        if ticks.is_empty() {
            return;
        }
        ctx.set_line_dash(&[]);
        for tick in &ticks {
            let major = boundary_weight(tick.value as i64).is_major();
            ctx.set_stroke_color(&theme.grid_color);
            ctx.set_stroke_width(if major { 1.5 } else { 1.0 });
            let x = area.x(time_scale, tick.value);
            ctx.begin_path();
            ctx.move_to(x, area.rect.y);
            ctx.line_to(x, area.rect.bottom());
            ctx.stroke();
        }
    }

    /// One faint horizontal divider per lane boundary (`lane_names.len() +
    /// 1` lines total) — plain equal-height rows, not the padded
    /// [`BandScale::band_range`] extent (that padding is for event
    /// geometry, drawn separately; the dividers mark the FULL row).
    fn draw_lane_dividers(&self, ctx: &mut dyn RenderContext, area: &PlotArea, lanes: &BandScale, theme: &FigureTheme) {
        if lanes.is_empty() {
            return;
        }
        ctx.set_stroke_color(&theme.grid_color);
        ctx.set_stroke_width(1.0);
        ctx.set_line_dash(&[]);
        let step = area.rect.height / lanes.len() as f64;
        for i in 0..=lanes.len() {
            let y = area.rect.y + step * i as f64;
            ctx.begin_path();
            ctx.move_to(area.rect.x, y);
            ctx.line_to(area.rect.right(), y);
            ctx.stroke();
        }
    }

    fn draw_lane_labels(&self, ctx: &mut dyn RenderContext, area: &PlotArea, lanes: &BandScale, theme: &FigureTheme) {
        for (i, label) in self.lane_names.iter().enumerate() {
            let (top, bottom) = area.y_band(lanes, i);
            let cy = (top + bottom) / 2.0;
            draw_label_right_aligned(ctx, label, area.rect.x - LABEL_GAP, cy, &theme.label_color, &theme.label_font);
        }
    }

    fn draw_intervals(&self, ctx: &mut dyn RenderContext, area: &PlotArea, time_scale: &TimeScale, lanes: &BandScale, theme: &FigureTheme) {
        ctx.set_font(&theme.label_font);
        for e in &self.events {
            let Some(end_ts) = e.end_ts else { continue };
            if e.lane >= lanes.len() {
                continue;
            }
            let (top, bottom) = area.y_band(lanes, e.lane);
            let (left, right, bar_y, bar_h) = interval_bar_geometry(area, time_scale, e.ts, end_ts, top, bottom);
            let width = (right - left).max(0.0);

            ctx.set_fill_color(palette_color(theme, e.kind));
            ctx.fill_rounded_rect(left, bar_y, width, bar_h, BAR_CORNER_RADIUS);

            if e.label.is_empty() {
                continue;
            }
            let label_w = ctx.measure_text(&e.label);
            let cy = bar_y + bar_h / 2.0;
            match inside_label_placement(left, right, area.rect.x, area.rect.right(), label_w, LABEL_GAP) {
                Some(InsideLabelPlacement::Centered(cx)) => {
                    // Fits inside the bar's own FULL span — background-tinted
                    // text reads cleanly against any palette accent, same
                    // convention as a filled-button label.
                    draw_label_centered(ctx, &e.label, cx, cy, &theme.background, &theme.label_font);
                }
                Some(InsideLabelPlacement::LeftAligned(lx)) => {
                    // The bar itself starts left of the visible plot area
                    // (or its FULL span is otherwise off-plot) but the
                    // VISIBLE portion is wide enough — anchor at the
                    // visible portion's own left edge, never at the bar's
                    // own off-plot start (see `inside_label_placement`'s
                    // own doc comment).
                    draw_label_left_aligned(ctx, &e.label, lx, cy, &theme.background, &theme.label_font);
                }
                None => {
                    // Fix D: a beside-label naturally drawn past the bar's own
                    // right edge would clip past the PLOT's right edge for an
                    // interval ending near it — mirror to the bar's own LEFT
                    // edge instead when that would happen.
                    let beside_left = beside_label_left_edge(right + LABEL_GAP, label_w, left - LABEL_GAP, area.rect.right());
                    draw_label_left_aligned(ctx, &e.label, beside_left, cy, &theme.label_color, &theme.label_font);
                }
            }
        }
    }

    /// Point-event markers + labels. Label placement is the bitmap-
    /// occupancy 2D placer ([`labeler`], research doc §2, arXiv
    /// 2405.10953): ONE [`OccupancyBitmap`] spans the whole plot rect,
    /// pre-marked with every interval bar's AND every point marker's own
    /// footprint (so an alternate label position never lands on top of a
    /// mark either, not just another label), then each label tries its
    /// existing Fix-D-resolved natural position FIRST — reproducing the
    /// pre-labeler behavior exactly when nothing else is contested — and
    /// only reaches for [`labeler::anchor_candidates`]'s alternate
    /// offsets (mirrored side, above, below, diagonals) when that natural
    /// slot is already claimed. A label with every candidate exhausted
    /// still degrades to a skip, same never-overlap convention this
    /// crate's greedy collision passes have always used — augmenting,
    /// not replacing, the skip-is-the-last-resort rule.
    ///
    /// Processing stays lane-by-lane, ts-ascending (same order as
    /// before) — the shared bitmap is threaded across lanes too, so a
    /// later lane's label additionally avoids an earlier lane's
    /// already-placed label, a correctness improvement the old
    /// per-lane-only greedy skip couldn't offer.
    fn draw_points(&self, ctx: &mut dyn RenderContext, area: &PlotArea, time_scale: &TimeScale, lanes: &BandScale, theme: &FigureTheme) {
        ctx.set_font(&theme.label_font);

        let mut occupancy = OccupancyBitmap::new(area.rect, labeler::DEFAULT_CELL_PX);
        for e in &self.events {
            if e.lane >= lanes.len() {
                continue;
            }
            let (top, bottom) = area.y_band(lanes, e.lane);
            match e.end_ts {
                Some(end_ts) => {
                    let (left, right, bar_y, bar_h) = interval_bar_geometry(area, time_scale, e.ts, end_ts, top, bottom);
                    occupancy.mark(Rect::new(left, bar_y, (right - left).max(0.0), bar_h));
                }
                None => {
                    let cy = (top + bottom) / 2.0;
                    let x = area.x(time_scale, e.ts);
                    occupancy.mark(Rect::new(x - POINT_RADIUS, cy - POINT_RADIUS, POINT_RADIUS * 2.0, POINT_RADIUS * 2.0));
                }
            }
        }

        for lane_idx in 0..lanes.len() {
            let mut indices: Vec<usize> = self
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| e.lane == lane_idx && !e.is_interval())
                .map(|(i, _)| i)
                .collect();
            indices.sort_by(|&a, &b| self.events[a].ts.partial_cmp(&self.events[b].ts).unwrap_or(std::cmp::Ordering::Equal));
            if indices.is_empty() {
                continue;
            }

            let (top, bottom) = area.y_band(lanes, lane_idx);
            let cy = (top + bottom) / 2.0;

            // Fix D: resolve each label's FINAL (post-right-edge-flip)
            // left edge as the candidate list's OWN first (natural)
            // entry, exactly as before — a label that already fits there
            // paints identically to pre-labeler output.
            let inputs: Vec<PointLabelInput> = indices
                .iter()
                .map(|&i| {
                    let x = area.x(time_scale, self.events[i].ts);
                    let natural_left = x + POINT_RADIUS + LABEL_GAP;
                    let label_width = ctx.measure_text(&self.events[i].label);
                    let label_left = beside_label_left_edge(natural_left, label_width, x - POINT_RADIUS - LABEL_GAP, area.rect.right());
                    PointLabelInput { label_left, label_width }
                })
                .collect();
            let label_height = indices
                .iter()
                .map(|&i| ctx.text_bounds(&self.events[i].label, &theme.label_font).h)
                .fold(0.0_f64, f64::max)
                .max(1.0);
            let natural_rects: Vec<Rect> =
                inputs.iter().map(|inp| Rect::new(inp.label_left, cy - label_height / 2.0, inp.label_width, label_height)).collect();

            let placed = labeler::place_labels(&mut occupancy, &natural_rects, |pos, natural| {
                let x = area.x(time_scale, self.events[indices[pos]].ts);
                let mut candidates = vec![natural];
                candidates.extend(labeler::anchor_candidates((x, cy), (natural.width, natural.height), POINT_RADIUS, LABEL_GAP));
                candidates
            });

            for (pos, &i) in indices.iter().enumerate() {
                let e = &self.events[i];
                let x = area.x(time_scale, e.ts);
                ctx.set_fill_color(palette_color(theme, e.kind));
                ctx.begin_path();
                ctx.arc(x, cy, POINT_RADIUS, 0.0, std::f64::consts::TAU);
                ctx.fill();

                if let Some(rect) = placed[pos] {
                    if !e.label.is_empty() {
                        draw_label_left_aligned(ctx, &e.label, rect.x, rect.center_y(), &theme.label_color, &theme.label_font);
                    }
                }
            }
        }
    }

    /// Vertical hover hairline + time-cursor label + hovered-event
    /// highlight + tooltip. A reduced form of
    /// [`crate::guide::crosshair::draw_crosshair`] (which crosses two
    /// CONTINUOUS scales): a lane axis has no continuous y-value to cross
    /// against, so only the vertical half + axis-cursor label apply here.
    fn draw_hover(&self, ctx: &mut dyn RenderContext, area: &PlotArea, time_scale: &TimeScale, lanes: &BandScale, theme: &FigureTheme, hx: f64, hy: f64) {
        let x = hx.clamp(area.rect.x, area.rect.right());

        ctx.set_stroke_color(&theme.label_color);
        ctx.set_stroke_width(1.0);
        ctx.set_line_dash(&CROSSHAIR_DASH);
        ctx.begin_path();
        ctx.move_to(x, area.rect.y);
        ctx.line_to(x, area.rect.bottom());
        ctx.stroke();
        ctx.set_line_dash(&[]);

        let t = ((x - area.rect.x) / area.rect.width.max(1e-9)).clamp(0.0, 1.0);
        let cursor_label = time_scale.format_value(time_scale.invert(t));
        ctx.set_font(&theme.label_font);
        ctx.set_fill_color(&theme.label_color);
        ctx.set_text_align(TextAlign::Center);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(&cursor_label, x, area.rect.bottom() + CROSSHAIR_LABEL_PAD);

        let Some(i) = hit_event_at(&self.events, area, time_scale, lanes, hx, hy) else { return };
        let e = &self.events[i];
        let (top, bottom) = area.y_band(lanes, e.lane);

        ctx.set_fill_color(&theme.highlight);
        ctx.set_global_alpha(HOVER_HIGHLIGHT_ALPHA);
        match e.end_ts {
            Some(end_ts) => {
                let (left, right, bar_y, bar_h) = interval_bar_geometry(area, time_scale, e.ts, end_ts, top, bottom);
                ctx.fill_rounded_rect(left, bar_y, (right - left).max(0.0), bar_h, BAR_CORNER_RADIUS);
            }
            None => {
                let cy = (top + bottom) / 2.0;
                let ex = area.x(time_scale, e.ts);
                ctx.begin_path();
                ctx.arc(ex, cy, POINT_RADIUS + 3.0, 0.0, std::f64::consts::TAU);
                ctx.fill();
            }
        }
        ctx.set_global_alpha(1.0);

        let lane_name = self.lane_names.get(e.lane).cloned().unwrap_or_default();
        let mut lines = vec![("lane".to_owned(), lane_name), ("label".to_owned(), e.label.clone())];
        match e.end_ts {
            Some(end_ts) => {
                lines.push(("start".to_owned(), time_scale.format_value(e.ts)));
                lines.push(("end".to_owned(), time_scale.format_value(end_ts)));
            }
            None => lines.push(("time".to_owned(), time_scale.format_value(e.ts))),
        }
        tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, area.rect);
    }
}

/// `theme.palette[kind % palette.len()]`, guarded against a degenerate
/// empty custom theme (falls back to `axis_color` rather than panicking on
/// a `% 0`).
fn palette_color<'a>(theme: &'a FigureTheme, kind: usize) -> &'a str {
    if theme.palette.is_empty() {
        return &theme.axis_color;
    }
    &theme.palette[kind % theme.palette.len()]
}

/// Shared bar geometry for an interval event — screen-pixel `(left, right,
/// bar_y, bar_h)` — computed once and reused by the plain draw pass, the
/// focus-outline pass, and the hover-highlight pass so all three agree
/// pixel-for-pixel (design law #1).
fn interval_bar_geometry(area: &PlotArea, time_scale: &TimeScale, ts: f64, end_ts: f64, lane_top: f64, lane_bottom: f64) -> (f64, f64, f64, f64) {
    let x0 = area.x(time_scale, ts);
    let x1 = area.x(time_scale, end_ts);
    let (left, right) = (x0.min(x1), x0.max(x1));
    let band_h = (lane_bottom - lane_top).max(0.0);
    let bar_h = BAR_HEIGHT.min(band_h);
    let bar_y = lane_top + (band_h - bar_h) / 2.0;
    (left, right, bar_y, bar_h)
}

/// Raw (unpadded) `[min, max]` timestamp extent across every event's
/// `ts`/`end_ts` — `None` for an empty event list.
fn raw_time_extent(events: &[TimelineEvent]) -> Option<(f64, f64)> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for e in events {
        min = min.min(e.ts);
        max = max.max(e.ts);
        if let Some(end) = e.end_ts {
            min = min.min(end);
            max = max.max(end);
        }
    }
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    Some((min, max))
}

/// Nice-padded time domain for [`TimelineFigure::time_scale`]: adds
/// [`TIME_PADDING_FRACTION`] of the raw span (floored at
/// [`MIN_TIME_PADDING_SECS`]) on each side, so edge events never sit
/// exactly on the plot's border and a single-event/zero-span domain still
/// widens to something renderable.
fn time_domain(events: &[TimelineEvent]) -> Option<(f64, f64)> {
    raw_time_extent(events).map(|(min, max)| {
        let span = (max - min).max(0.0);
        let pad = (span * TIME_PADDING_FRACTION).max(MIN_TIME_PADDING_SECS);
        (min - pad, max + pad)
    })
}

/// Row index (`0..lanes.len()`) whose pixel band (via [`PlotArea::y_band`])
/// contains `py` — the y-axis analogue of
/// [`crate::interact::hit::bar_index_at`], which only covers the x
/// direction ([`PlotArea::x_band`]). `None` when `py` falls in an
/// inter-lane padding gap or outside the whole scale.
fn lane_index_at(area: &PlotArea, lanes: &BandScale, py: f64) -> Option<usize> {
    (0..lanes.len()).find(|&i| {
        let (top, bottom) = area.y_band(lanes, i);
        py >= top && py <= bottom
    })
}

/// Index into `events` nearest `(px, py)` in screen space, restricted to
/// events on whichever lane `py` falls in (if any) — this figure's own
/// hover hit-test, playing the role [`crate::interact::hit::bar_index_at`]/
/// [`crate::interact::hit::nearest_point_x`] play for bars/curves. An
/// interval event hits anywhere along its bar's pixel span (plus
/// [`HIT_TOLERANCE`]); a point event hits within [`POINT_RADIUS`] (plus
/// tolerance) of its marker — nearest wins among point-event candidates.
fn hit_event_at(events: &[TimelineEvent], area: &PlotArea, time_scale: &TimeScale, lanes: &BandScale, px: f64, py: f64) -> Option<usize> {
    let lane = lane_index_at(area, lanes, py)?;
    let mut best: Option<(usize, f64)> = None;
    for (i, e) in events.iter().enumerate() {
        if e.lane != lane {
            continue;
        }
        match e.end_ts {
            Some(end_ts) => {
                let x0 = area.x(time_scale, e.ts);
                let x1 = area.x(time_scale, end_ts);
                let (lo, hi) = (x0.min(x1), x0.max(x1));
                if px >= lo - HIT_TOLERANCE && px <= hi + HIT_TOLERANCE {
                    return Some(i); // an interval hit is unambiguous — its whole span is the target
                }
            }
            None => {
                let x0 = area.x(time_scale, e.ts);
                let dist = (x0 - px).abs();
                if dist <= POINT_RADIUS + HIT_TOLERANCE && best.map_or(true, |(_, best_dist)| dist < best_dist) {
                    best = Some((i, dist));
                }
            }
        }
    }
    best.map(|(i, _)| i)
}

/// Pure input to [`layout_point_labels`]: one candidate label's screen-
/// space left edge (already offset from its own marker by the caller —
/// this fn only resolves collisions BETWEEN labels, not between a label
/// and its own marker) and measured width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLabelInput {
    pub label_left: f64,
    pub label_width: f64,
}

/// Fix D — right-edge label flip: resolve a beside-label's own paint LEFT
/// edge, given its NATURAL (unflipped, drawn to the right of its own
/// marker/bar) left-aligned position (`natural_left`), the label's
/// measured `width`, this same event's own MIRRORED anchor (the position
/// that keeps the identical gap on the OTHER side of the marker/bar —
/// e.g. `marker_x - POINT_RADIUS - LABEL_GAP` for a point event, or
/// `bar_left - LABEL_GAP` for an interval bar's beside-label), and the
/// plot's own right bound.
///
/// If painting left-aligned at `natural_left` would clip past
/// `plot_right`, the label mirrors to the LEFT of the marker/bar instead
/// (same `gap`, on the other side) — the caller always paints the
/// returned left edge via [`draw_label_left_aligned`], never a separate
/// right-aligned call site, so there is exactly one paint call per label
/// regardless of which side it ultimately lands on.
fn beside_label_left_edge(natural_left: f64, width: f64, mirrored_anchor: f64, plot_right: f64) -> f64 {
    if natural_left + width > plot_right {
        mirrored_anchor - width
    } else {
        natural_left
    }
}

/// Where (if anywhere) an interval bar's own inside-bar label should land,
/// given the bar's FULL screen-pixel span `[left, right]` (which may start
/// or end off-plot — nothing in this figure's own domain computation
/// currently produces that, since [`time_domain`] always pads to cover
/// every event, but a caller-overridden scale or a future degenerate
/// padding edge case could) and the plot's own `[plot_left, plot_right]`
/// bounds.
///
/// Owner defect report: the pre-fix code checked "does the label fit the
/// bar's FULL width" and, if so, always centered at the bar's own FULL
/// center (`left + width / 2.0`) — correct only when the bar itself is
/// entirely on-plot. A bar whose `left` sits before `plot_left` has a FULL
/// center that can ALSO sit before `plot_left`, silently painting the label
/// (or its leading glyphs) off the visible plot area even though the check
/// that let it through only ever measured the bar's own (partly invisible)
/// width, never where any of that width is actually visible.
///
/// Three outcomes:
/// - the bar is fully on-plot (`left >= plot_left`) AND its own FULL width
///   fits the label: [`InsideLabelPlacement::Centered`] at the bar's own
///   full center — byte-identical to this figure's pre-fix behavior for
///   every bar that was already entirely on-plot (the common case).
/// - otherwise, if the VISIBLE portion (`[left, right]` clamped to
///   `[plot_left, plot_right]`) is wide enough: [`InsideLabelPlacement::
///   LeftAligned`] at the visible portion's own left edge + `pad` — never
///   anchored at the bar's own (possibly off-plot) `left`.
/// - neither: `None` — the caller falls back to its existing beside-the-bar
///   placement (`beside_label_left_edge`), unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
enum InsideLabelPlacement {
    /// Centered at this screen-pixel X.
    Centered(f64),
    /// Left-aligned starting at this screen-pixel X.
    LeftAligned(f64),
}

fn inside_label_placement(left: f64, right: f64, plot_left: f64, plot_right: f64, label_w: f64, pad: f64) -> Option<InsideLabelPlacement> {
    let width = (right - left).max(0.0);
    if left >= plot_left && label_w + pad * 2.0 <= width {
        return Some(InsideLabelPlacement::Centered(left + width / 2.0));
    }

    let visible_left = left.max(plot_left);
    let visible_right = right.min(plot_right);
    let visible_width = (visible_right - visible_left).max(0.0);
    if label_w + pad * 2.0 <= visible_width {
        return Some(InsideLabelPlacement::LeftAligned(visible_left + pad));
    }

    None
}

/// Greedy left-to-right label collision within one lane, resolved in
/// INPUT order (not resorted by position): if the next label's left edge
/// would overlap the last VISIBLE label's extent (plus `gap`), it's
/// skipped rather than crowded — same convention as
/// [`crate::guide::axis::draw_x_axis`]'s tick-label collision (skip only,
/// never stagger — no other guide in this crate staggers either).
///
/// Callers get correct behavior by passing `inputs` already sorted
/// ascending by screen position (e.g. events sorted by timestamp, since
/// [`TimeScale`]'s map is monotonic) — this fn does not sort them itself,
/// since the "which one wins a contested slot" tie-break is a caller
/// policy (earliest-first here), not something a generic collision
/// resolver should decide.
pub fn layout_point_labels(inputs: &[PointLabelInput], gap: f64) -> Vec<bool> {
    let mut visible = Vec::with_capacity(inputs.len());
    let mut last_label_right = f64::MIN;
    for input in inputs {
        if input.label_left < last_label_right + gap {
            visible.push(false);
            continue;
        }
        visible.push(true);
        last_label_right = input.label_left + input.label_width;
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`layout_point_labels`]'s own gap parameter, exercised directly by
    /// its unit tests below — the render path (`draw_points`) no longer
    /// calls this pure fn itself (superseded by the bitmap-occupancy
    /// placer, see `draw_points`'s own module docs), but the fn/gap
    /// convention stays exported+tested for external callers.
    const LABEL_COLLISION_GAP: f64 = 4.0;

    fn evt(ts: f64, end_ts: Option<f64>, lane: usize, label: &str, kind: usize) -> TimelineEvent {
        TimelineEvent { ts, end_ts, lane, label: label.to_owned(), kind }
    }

    fn lanes(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("lane-{i}")).collect()
    }

    // ── Lane band math (requirement 1) ──────────────────────────────────

    #[test]
    fn events_land_within_their_own_lane_strip() {
        let events = vec![evt(100.0, None, 0, "a", 0), evt(200.0, None, 1, "b", 0), evt(300.0, None, 2, "c", 0)];
        let figure = TimelineFigure::new(events, lanes(3));
        let rect = Rect::new(0.0, 0.0, 400.0, 300.0);
        let area = figure.plot_area(rect);
        let band = figure.lane_scale();

        for e in &figure.events {
            let (top, bottom) = area.y_band(&band, e.lane);
            assert!(top < bottom, "lane strip must have positive height");
            let cy = (top + bottom) / 2.0;
            assert!(cy >= area.rect.y && cy <= area.rect.bottom(), "lane center must sit inside the plot rect");
        }
        // Distinct lane strips must not overlap, and must read top-down in
        // lane order (lane 0 above lane 1 above lane 2).
        let (_, b0) = area.y_band(&band, 0);
        let (t1, b1) = area.y_band(&band, 1);
        let (t2, _) = area.y_band(&band, 2);
        assert!(b0 <= t1, "lane 0 strip must sit entirely above lane 1's");
        assert!(b1 <= t2, "lane 1 strip must sit entirely above lane 2's");
    }

    // ── Interval bar spans (requirement 2) ───────────────────────────────

    #[test]
    fn interval_bar_spans_map_ts_to_map_end_ts() {
        let events = vec![evt(1_000.0, Some(5_000.0), 0, "span", 0)];
        let figure = TimelineFigure::new(events, lanes(1));
        let rect = Rect::new(0.0, 0.0, 400.0, 200.0);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("events present");
        let band = figure.lane_scale();

        let e = &figure.events[0];
        let end_ts = e.end_ts.expect("fixture event is an interval");
        let (top, bottom) = area.y_band(&band, e.lane);
        let (left, right, bar_y, bar_h) = interval_bar_geometry(&area, &time_scale, e.ts, end_ts, top, bottom);

        assert!((left - area.x(&time_scale, e.ts)).abs() < 1e-9);
        assert!((right - area.x(&time_scale, end_ts)).abs() < 1e-9);
        assert!(right > left, "end must map to the right of start");
        assert!(bar_y >= top - 1e-9 && bar_y + bar_h <= bottom + 1e-9, "bar must fit inside its lane strip");
    }

    #[test]
    fn interval_bar_geometry_handles_a_reversed_ts_end_ts_pair() {
        // Malformed input (end before start) must still produce a valid,
        // ordered span rather than a negative width.
        let area = PlotArea::new(Rect::new(0.0, 0.0, 400.0, 100.0));
        let scale = TimeScale::new(0.0, 100.0);
        let (left, right, _, _) = interval_bar_geometry(&area, &scale, 80.0, 20.0, 0.0, 100.0);
        assert!(left < right);
    }

    // ── Label collision (requirement 3) ──────────────────────────────────

    #[test]
    fn label_collision_skips_the_second_of_two_close_labels() {
        let inputs = vec![
            PointLabelInput { label_left: 10.0, label_width: 40.0 }, // occupies [10, 50]
            PointLabelInput { label_left: 30.0, label_width: 40.0 }, // starts inside [10, 50] -> collides
        ];
        assert_eq!(layout_point_labels(&inputs, LABEL_COLLISION_GAP), vec![true, false]);
    }

    #[test]
    fn label_collision_keeps_labels_with_enough_gap() {
        let inputs = vec![
            PointLabelInput { label_left: 0.0, label_width: 20.0 },  // [0, 20]
            PointLabelInput { label_left: 40.0, label_width: 20.0 }, // starts well past 20 + gap
        ];
        assert_eq!(layout_point_labels(&inputs, LABEL_COLLISION_GAP), vec![true, true]);
    }

    #[test]
    fn label_collision_resumes_after_a_skip() {
        let inputs = vec![
            PointLabelInput { label_left: 0.0, label_width: 30.0 },   // [0, 30] visible
            PointLabelInput { label_left: 10.0, label_width: 30.0 },  // collides -> skipped
            PointLabelInput { label_left: 100.0, label_width: 20.0 }, // far clear of [0, 30] -> visible
        ];
        assert_eq!(layout_point_labels(&inputs, LABEL_COLLISION_GAP), vec![true, false, true]);
    }

    // ── Right-edge label flip (Fix D) ─────────────────────────────────────

    #[test]
    fn beside_label_stays_at_its_natural_position_when_it_already_fits() {
        // Natural placement [10, 60] comfortably inside a 100px-right plot.
        let left_edge = beside_label_left_edge(10.0, 50.0, -40.0, 100.0);
        assert_eq!(left_edge, 10.0, "a label that already fits must never move");
    }

    #[test]
    fn beside_label_mirrors_to_the_other_side_when_it_would_clip_the_plot_right_edge() {
        // Natural placement [90, 140] clips a 100px-wide plot; the mirrored
        // anchor (this event's own left-side edge) is 20.0, so the flipped
        // label's own right edge must land exactly there.
        let width = 50.0;
        let mirrored_anchor = 20.0;
        let left_edge = beside_label_left_edge(90.0, width, mirrored_anchor, 100.0);
        assert!((left_edge - (mirrored_anchor - width)).abs() < 1e-9, "flipped label's right edge must sit exactly at the mirrored anchor");
        assert!(left_edge + width <= 100.0, "flipped label must fit within the plot's own right edge");
    }

    // ── Off-plot inside-bar label clamp (report fix) ─────────────────────

    #[test]
    fn inside_label_placement_centers_in_the_bar_when_fully_on_plot_and_fitting() {
        // A bar comfortably inside the plot, wide enough for the label —
        // byte-identical to the pre-fix centered behavior.
        let placement = inside_label_placement(20.0, 120.0, 0.0, 200.0, 40.0, LABEL_GAP);
        assert_eq!(placement, Some(InsideLabelPlacement::Centered(70.0)));
    }

    #[test]
    fn inside_label_placement_clamps_to_the_visible_start_when_the_bar_begins_off_plot() {
        // Bar spans [-80, 60] (starts 80px before the plot's own left edge
        // at x=0). Its FULL width (140) trivially "fits" a 30px label, but
        // `left < plot_left`, so the fully-on-plot fast path (which would
        // have centered on the FULL, partly-invisible span) is skipped;
        // the VISIBLE width (60 - 0 = 60) still fits, so it clamps to the
        // visible portion's own start instead — never anchoring at the
        // bar's own off-plot `left`.
        let placement = inside_label_placement(-80.0, 60.0, 0.0, 200.0, 30.0, LABEL_GAP);
        assert_eq!(placement, Some(InsideLabelPlacement::LeftAligned(0.0 + LABEL_GAP)));
    }

    #[test]
    fn inside_label_placement_never_centers_past_the_plot_left_edge() {
        // Bar spans [-300, 60] — FULL width (360) trivially fits a 40px
        // label under the OLD (pre-fix) check, and the FULL center
        // (-300+180=-120) sits WAY off-plot to the left. The visible
        // portion is [0, 60] (width 60), which also fits — must resolve to
        // LeftAligned at the visible start (0 + pad), never Centered at a
        // negative x.
        let placement = inside_label_placement(-300.0, 60.0, 0.0, 500.0, 40.0, LABEL_GAP);
        assert_eq!(placement, Some(InsideLabelPlacement::LeftAligned(LABEL_GAP)));
    }

    #[test]
    fn inside_label_placement_falls_back_to_none_when_even_the_visible_portion_is_too_narrow() {
        // Bar starts off-plot, and its visible sliver (only 5px) can't fit
        // any reasonably-sized label — caller must fall back to its
        // existing beside-the-bar placement.
        let placement = inside_label_placement(-100.0, 5.0, 0.0, 500.0, 40.0, LABEL_GAP);
        assert_eq!(placement, None);
    }

    #[test]
    fn inside_label_placement_falls_back_to_none_for_an_ordinary_too_narrow_on_plot_bar() {
        // A fully on-plot bar too narrow for its own label — the ordinary,
        // pre-existing "fall back to beside" case, unaffected by this fix.
        let placement = inside_label_placement(20.0, 40.0, 0.0, 500.0, 100.0, LABEL_GAP);
        assert_eq!(placement, None);
    }

    /// End-to-end proof against REAL geometry (not just the pure helper in
    /// isolation): a point event placed near the tail of a genuinely wide
    /// seeded time domain (so its own marker sits close to the plot's
    /// right edge) with a long label — an UNFLIPPED, right-of-marker
    /// placement would clip well past the plot's own right bound; the
    /// resolved (flipped) placement must not.
    #[test]
    fn point_event_label_near_the_right_edge_does_not_clip_past_the_plot_right_edge() {
        let long_label = "resting balance identified at custodial cold storage pending review";
        const DAY_SECS: f64 = 86_400.0;
        let near_edge_ts = 30.0 * DAY_SECS;
        let events = vec![evt(0.0, None, 0, "start", 0), evt(near_edge_ts, None, 0, long_label, 0)];
        let figure = TimelineFigure::new(events, lanes(1));
        let rect = Rect::new(0.0, 0.0, 500.0, 150.0);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("events present");
        let theme = FigureTheme::dark();

        let x = area.x(&time_scale, near_edge_ts);
        let natural_left = x + POINT_RADIUS + LABEL_GAP;

        let spec = uzor_export::ExportSpec { width_px: 500, height_px: 150, dpr: 1.0, background: None };
        let mut label_width = 0.0_f64;
        uzor_export::render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            label_width = ctx.measure_text(long_label);
        })
        .expect("probe render must succeed");

        // Sanity: this fixture's own natural (unflipped) placement WOULD
        // clip past the plot's right edge — otherwise the flip branch
        // below is never actually exercised by this test.
        assert!(
            natural_left + label_width > area.rect.right(),
            "fixture must exercise the flip (tune the near-edge ts/label if this ever fails): \
             natural_left={natural_left}, label_width={label_width}, plot_right={}",
            area.rect.right()
        );

        let left_edge = beside_label_left_edge(natural_left, label_width, x - POINT_RADIUS - LABEL_GAP, area.rect.right());
        assert!(
            left_edge + label_width <= area.rect.right() + 1e-6,
            "flipped label must fit within the plot's own right edge: left_edge={left_edge}, width={label_width}, plot_right={}",
            area.rect.right()
        );
        assert!(left_edge < x, "the flipped label must move to the LEFT of the marker, not stay in place");
    }

    // ── Empty / single event (requirement 4) ─────────────────────────────

    #[test]
    fn time_domain_is_none_for_empty_events() {
        assert!(time_domain(&[]).is_none());
        let figure = TimelineFigure::new(Vec::new(), lanes(2));
        assert!(figure.time_scale().is_none());
    }

    #[test]
    fn single_event_widens_to_a_non_degenerate_padded_domain() {
        let events = vec![evt(1_000.0, None, 0, "only", 0)];
        let figure = TimelineFigure::new(events, lanes(1));
        let scale = figure.time_scale().expect("one event should still produce a domain");
        assert!(scale.min_ts.is_finite() && scale.max_ts.is_finite());
        assert!(scale.max_ts > scale.min_ts, "padding must widen a single-point domain");
        assert!(scale.min_ts < 1_000.0 && scale.max_ts > 1_000.0);
    }

    #[test]
    fn empty_events_render_without_panicking() {
        let figure = TimelineFigure::new(Vec::new(), lanes(2));
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok(), "empty-events timeline must render without panicking");
    }

    #[test]
    fn single_event_renders_without_panicking() {
        let events = vec![evt(1_000.0, None, 0, "only", 0)];
        let figure = TimelineFigure::new(events, lanes(1)).with_title("single");
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok(), "single-event timeline must render without panicking");
    }

    #[test]
    fn zero_lanes_and_zero_events_render_without_panicking() {
        let figure = TimelineFigure::new(Vec::new(), Vec::new());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 120.0), &theme);
        });
        assert!(result.is_ok(), "a figure with no lanes and no events must still render without panicking");
    }

    // ── Hover hit-test ────────────────────────────────────────────────────

    #[test]
    fn hit_event_at_finds_a_point_event_near_its_marker_and_misses_far_away() {
        let events = vec![evt(1_000.0, None, 0, "a", 0)];
        let figure = TimelineFigure::new(events, lanes(1));
        let rect = Rect::new(0.0, 0.0, 400.0, 200.0);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("events present");
        let band = figure.lane_scale();
        let x = area.x(&time_scale, 1_000.0);
        let (top, bottom) = area.y_band(&band, 0);
        let y = (top + bottom) / 2.0;

        assert_eq!(hit_event_at(&figure.events, &area, &time_scale, &band, x, y), Some(0));
        assert_eq!(hit_event_at(&figure.events, &area, &time_scale, &band, x + 500.0, y), None);
    }

    #[test]
    fn hit_event_at_finds_an_interval_event_anywhere_along_its_span() {
        let events = vec![evt(1_000.0, Some(5_000.0), 0, "span", 0)];
        let figure = TimelineFigure::new(events, lanes(1));
        let rect = Rect::new(0.0, 0.0, 400.0, 200.0);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("events present");
        let band = figure.lane_scale();
        let (top, bottom) = area.y_band(&band, 0);
        let y = (top + bottom) / 2.0;
        let x_mid = (area.x(&time_scale, 1_000.0) + area.x(&time_scale, 5_000.0)) / 2.0;

        assert_eq!(hit_event_at(&figure.events, &area, &time_scale, &band, x_mid, y), Some(0));
    }

    #[test]
    fn hit_event_at_respects_lane_boundaries() {
        let events = vec![evt(1_000.0, None, 0, "a", 0), evt(1_000.0, None, 1, "b", 0)];
        let figure = TimelineFigure::new(events, lanes(2));
        let rect = Rect::new(0.0, 0.0, 400.0, 200.0);
        let area = figure.plot_area(rect);
        let time_scale = figure.time_scale().expect("events present");
        let band = figure.lane_scale();
        let x = area.x(&time_scale, 1_000.0);

        let (top0, bottom0) = area.y_band(&band, 0);
        assert_eq!(hit_event_at(&figure.events, &area, &time_scale, &band, x, (top0 + bottom0) / 2.0), Some(0));
        let (top1, bottom1) = area.y_band(&band, 1);
        assert_eq!(hit_event_at(&figure.events, &area, &time_scale, &band, x, (top1 + bottom1) / 2.0), Some(1));
    }
}
