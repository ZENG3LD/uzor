//! Axis guides — baseline + tick marks + tick labels, driven entirely by
//! [`Scale::ticks`] and drawn through the same [`PlotArea`] transform
//! every mark uses.
//!
//! [`BandScale`] needs no special case here: its `ticks()` already
//! returns one tick per category, and its `map()` already resolves an
//! index to the band CENTER (not the band edge) — so
//! `area.x(band_scale, tick.value)` lands the label exactly where a
//! category-axis label belongs, through the exact same call every
//! continuous-scale axis uses.
//!
//! Label collision is a simple greedy left-to-right (x-axis) / top-to-
//! bottom (y-axis) skip: if the next label would overlap the last DRAWN
//! label's extent, it's dropped rather than crowded — cheap and gives a
//! readable axis without a real constraint solver.

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::coord::PlotArea;
use crate::mark::text::draw_label_right_aligned;
use crate::scale::time::TickMarkWeight;
use crate::scale::{NumberFormat, Scale, Tick, TickPriority};
use crate::theme::FigureTheme;

const TICK_LENGTH: f64 = 4.0;
const LABEL_GAP: f64 = 4.0;

/// Visual policy for [`draw_x_axis_weighted`]/[`draw_y_axis_weighted`] —
/// how a tick's own [`Scale::tick_weight`] classification (only
/// [`crate::scale::TimeScale`] reports one today; every other scale's
/// ticks stay `None` and render EXACTLY like the unweighted
/// [`draw_x_axis`]/[`draw_y_axis`], regardless of this style) changes its
/// drawn tick length, stroke width, and (for a major tick only) label
/// color. A configurable field set, not a hardcoded constant — every
/// behavioral knob here is a real option: [`AxisTickWeightStyle::default`]
/// renders a visibly distinguishable major > medium > minor hierarchy
/// (the "tested hierarchy computed and thrown away" audit finding this
/// closes — [`crate::scale::time::TickMarkWeight`] was fully built and
/// unit-tested but never consulted by this shared guide);
/// [`AxisTickWeightStyle::flat`] reproduces the unweighted draw's own
/// single style for every tick regardless of weight, reachable through
/// the weighted entry point without switching call sites back to
/// `draw_x_axis`/`draw_y_axis`.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTickWeightStyle {
    /// Tick-mark length (px) for a major tick ([`TickMarkWeight::is_major`]
    /// — Year/Month calendar boundaries).
    pub major_tick_length: f64,
    /// Tick-mark length (px) for a medium tick ([`TickMarkWeight::
    /// is_medium`] — Day boundaries).
    pub medium_tick_length: f64,
    /// Tick-mark length (px) for every other (minor) tick — matches
    /// [`TICK_LENGTH`], the unweighted draw's own fixed value.
    pub minor_tick_length: f64,
    /// Stroke width (px) for a major tick's own mark.
    pub major_stroke_width: f64,
    /// Stroke width (px) for a medium tick's own mark.
    pub medium_stroke_width: f64,
    /// Stroke width (px) for a minor tick's own mark — matches the
    /// unweighted draw's own fixed `1.0`.
    pub minor_stroke_width: f64,
    /// Label color for a major tick's own text — `None` falls back to
    /// `theme.label_color` (same as every tick under the unweighted
    /// draw; medium/minor ticks always use `theme.label_color`).
    pub major_label_color: Option<String>,
}

impl Default for AxisTickWeightStyle {
    /// A visibly distinguishable major > medium > minor hierarchy —
    /// roughly double tick length + heavier stroke for a major boundary,
    /// a modest bump for a medium one, unchanged for minor.
    fn default() -> Self {
        Self {
            major_tick_length: TICK_LENGTH * 2.0,
            medium_tick_length: TICK_LENGTH * 1.5,
            minor_tick_length: TICK_LENGTH,
            major_stroke_width: 1.75,
            medium_stroke_width: 1.25,
            minor_stroke_width: 1.0,
            major_label_color: None,
        }
    }
}

impl AxisTickWeightStyle {
    /// Every tick draws identically regardless of its own weight — the
    /// SAME output [`draw_x_axis`]/[`draw_y_axis`] (the unweighted draw)
    /// already produce, reachable through the weighted entry point
    /// without switching call sites.
    pub fn flat() -> Self {
        Self {
            major_tick_length: TICK_LENGTH,
            medium_tick_length: TICK_LENGTH,
            minor_tick_length: TICK_LENGTH,
            major_stroke_width: 1.0,
            medium_stroke_width: 1.0,
            minor_stroke_width: 1.0,
            major_label_color: None,
        }
    }
}

/// Resolve `(tick_length, stroke_width)` for `weight` under `style` — a
/// tick with no weight (`None`, every non-[`crate::scale::TimeScale`]
/// scale, or a `TimeScale` tick this crate's own classifier never marks
/// major/medium) resolves to the MINOR tier.
pub(crate) fn resolve_tick_style(weight: Option<TickMarkWeight>, style: &AxisTickWeightStyle) -> (f64, f64) {
    match weight {
        Some(w) if w.is_major() => (style.major_tick_length, style.major_stroke_width),
        Some(w) if w.is_medium() => (style.medium_tick_length, style.medium_stroke_width),
        _ => (style.minor_tick_length, style.minor_stroke_width),
    }
}

// ── Priority-aware label-collision skip ─────────────────────────────────
//
// Defect fix: the pre-existing single-pass greedy skip (still exactly
// reproduced below when every tick shares one priority tier) has no
// notion that some ticks matter more than others — on a dense
// `crate::scale::SymlogScale` axis this silently dropped the axis's own
// `0.0` zero-crossing label in favor of a merely-intermediate neighbor.

/// One label's own screen-space collision footprint along whichever
/// SINGLE axis [`resolve_label_priority`] is checking — `center`/
/// `half_extent` are x/half-width for [`draw_x_axis`]'s horizontal skip,
/// y/half-row-height for [`draw_y_axis`]'s vertical skip — plus its
/// [`TickPriority`] (from [`Scale::tick_priority`]).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LabelFootprint {
    pub center: f64,
    pub half_extent: f64,
    pub priority: TickPriority,
}

/// Symmetric 1D overlap-with-margin test (`gap` px of required
/// clearance) — for two footprints that are strictly ordered by
/// position (the pre-existing single-pass regime), this is provably
/// equivalent to the ORIGINAL pairwise test each axis direction used
/// (`x - half_w < last_right + gap` / the Y-axis's own top/bottom
/// mirror), see [`resolve_label_priority`]'s own doc comment for the
/// proof sketch.
fn footprints_collide(a: &LabelFootprint, b: &LabelFootprint, gap: f64) -> bool {
    (a.center - b.center).abs() < a.half_extent + b.half_extent + gap
}

/// Accept `footprints[idx]` (mark it visible + remember its own
/// footprint) unless it collides with ANY already-`accepted` footprint —
/// tick counts on a real axis are small (rarely more than a few dozen),
/// so a full linear scan against everything accepted so far stays cheap
/// even though processing order is no longer purely positional once
/// [`TickPriority::Critical`]/`Major` tiers reorder ahead of `Minor`.
fn accept_if_clear(footprints: &[LabelFootprint], accepted: &mut Vec<LabelFootprint>, visible: &mut [bool], idx: usize, gap: f64) {
    let candidate = footprints[idx];
    if accepted.iter().any(|a| footprints_collide(a, &candidate, gap)) {
        return;
    }
    visible[idx] = true;
    accepted.push(candidate);
}

/// Resolve which of `footprints` (in ASCENDING screen-position order,
/// matching [`Scale::ticks`]' own ascending domain order) get their own
/// label DRAWN — the priority-aware generalization of this crate's
/// pre-existing plain left-to-right greedy skip, driving
/// [`draw_x_axis`]/[`draw_y_axis`]/[`draw_x_axis_overflow`]'s
/// [`LabelOverflow::Skip`] branch.
///
/// Three tiers, processed in this order:
/// 1. [`TickPriority::Critical`] — position order (in practice at most
///    one per axis, e.g. a symlog scale's own zero crossing, so order
///    rarely matters).
/// 2. [`TickPriority::Major`] — the two EXTREMES of the major set
///    (leftmost/topmost and rightmost/bottommost by position) are tried
///    FIRST, then the rest in ascending position order — the "prefer
///    keeping the extremes [and zero]" degrade rule for the rare case
///    where majors alone still collide (zero itself is never a `Major`
///    tick on a scale that also reports it `Critical`, e.g.
///    [`crate::scale::SymlogScale`], so it's already locked in by tier
///    1 before this tier even runs).
/// 3. [`TickPriority::Minor`] — plain ascending position order.
///
/// **Byte-identical-to-today guarantee**: when EVERY footprint shares
/// the same tier (every scale's own default, `Minor` — see
/// [`Scale::tick_priority`]'s own doc comment), tiers 1/2 are empty and
/// tier 3 alone runs in plain ascending order, testing each candidate
/// against the full `accepted` list — but since accepted footprints in
/// this ascending/single-tier regime are, by construction, pairwise
/// non-overlapping and monotonically increasing in position, a new
/// (further-right) candidate can only ever possibly collide with the
/// LAST-accepted one (every earlier accepted footprint sits even
/// further away) — so this reduces EXACTLY to the original
/// nearest-neighbor-only test, proven by
/// `priority_skip_with_uniform_priority_matches_the_plain_greedy_skip`
/// below.
pub(crate) fn resolve_label_priority(footprints: &[LabelFootprint], gap: f64) -> Vec<bool> {
    let n = footprints.len();
    let mut visible = vec![false; n];
    let mut accepted: Vec<LabelFootprint> = Vec::with_capacity(n);

    for (i, f) in footprints.iter().enumerate() {
        if f.priority == TickPriority::Critical {
            accept_if_clear(footprints, &mut accepted, &mut visible, i, gap);
        }
    }

    let major_indices: Vec<usize> =
        footprints.iter().enumerate().filter(|(_, f)| f.priority == TickPriority::Major).map(|(i, _)| i).collect();
    if let (Some(&first), Some(&last)) = (major_indices.first(), major_indices.last()) {
        accept_if_clear(footprints, &mut accepted, &mut visible, first, gap);
        if last != first {
            accept_if_clear(footprints, &mut accepted, &mut visible, last, gap);
        }
    }
    for &i in &major_indices {
        if !visible[i] {
            accept_if_clear(footprints, &mut accepted, &mut visible, i, gap);
        }
    }

    for (i, f) in footprints.iter().enumerate() {
        if f.priority == TickPriority::Minor {
            accept_if_clear(footprints, &mut accepted, &mut visible, i, gap);
        }
    }

    visible
}

/// Measure the pixel gutter a Y axis over `scale` (drawn via
/// [`draw_y_axis`]) actually needs — `TICK_LENGTH + LABEL_GAP` plus the
/// widest generated tick label's own measured text width. A figure under
/// [`crate::figure::MarginPolicy::Measured`] widens its own fixed left
/// margin to `max(fixed, this)` so a label can never draw past the plot
/// rect's own left edge (the audit's own B2 finding: [`draw_y_axis`]
/// draws every label right-aligned with NO width check against the
/// margin reserved for it). Empty `scale.ticks(target_ticks)` measures to
/// `0.0` (nothing to reserve).
pub fn measure_y_axis_gutter(ctx: &mut dyn RenderContext, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) -> f64 {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return 0.0;
    }
    ctx.set_font(&theme.label_font);
    let widest = ticks.iter().map(|t| ctx.measure_text(&t.label)).fold(0.0_f64, f64::max);
    TICK_LENGTH + LABEL_GAP + widest
}

/// Axis bounding-box `(width, height)` of a `width` x `height` label
/// rectangle rotated by `degrees` around its own anchor — the standard
/// AABB-of-a-rotated-rectangle formula, used by [`LabelOverflow::Rotate`]/
/// `Auto`'s own margin sizing ([`measure_rotated_x_axis_gutter`]) and unit
/// tested in isolation from any render call.
pub fn rotated_label_extent(width: f64, height: f64, degrees: f64) -> (f64, f64) {
    let (s, c) = degrees.to_radians().sin_cos();
    ((width * c).abs() + (height * s).abs(), (width * s).abs() + (height * c).abs())
}

/// How [`draw_x_axis_overflow`] handles a tick label that would collide
/// with its neighbor under the plain greedy left-to-right skip
/// [`draw_x_axis`] always uses — see this module's own top-level doc
/// comment for that skip's exact rule. A `BarFigure`/`HistogramFigure`
/// X-axis with many categories or long category names silently drops
/// most of its own labels under that rule with no fallback — the audit's
/// own B4 finding.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LabelOverflow {
    /// DEFAULT — reproduces [`draw_x_axis`]'s own greedy-skip rule exactly
    /// (a colliding label is dropped, never crowded).
    #[default]
    Skip,
    /// Rotate EVERY tick label by `degrees` (matching
    /// [`uzor::render::RenderContext::rotate`]'s own sign convention)
    /// around its own tick anchor instead of skipping a colliding one —
    /// via `ctx.save()`/`translate()`/`rotate()`/`restore()` (already
    /// exposed by [`RenderContext`], never called from this module before
    /// this item). A rotated label's own horizontal footprint shrinks
    /// (see [`rotated_label_extent`]), so every label draws — no
    /// collision skip needed.
    Rotate(f64),
    /// Try the plain greedy-skip pass first (via a dry run, not an actual
    /// draw); if it would drop more than [`AUTO_ROTATE_DROP_THRESHOLD`] of
    /// the tick set, rotate the WHOLE axis at [`AUTO_ROTATE_DEGREES`]
    /// instead. A caller wanting an exact angle should use
    /// [`LabelOverflow::Rotate`] directly.
    Auto,
}

/// The rotation angle (degrees) [`LabelOverflow::Auto`] falls back to when
/// the plain greedy-skip pass would drop too many labels — exported so a
/// caller sizing its own bottom margin for `Auto` (which cannot know in
/// advance whether a rotate will actually trigger — see
/// [`measure_rotated_x_axis_gutter`]'s own doc comment) can reserve
/// against the SAME angle this module resolves to.
pub const AUTO_ROTATE_DEGREES: f64 = 45.0;

/// The fraction of ticks the plain greedy-skip pass must drop before
/// [`LabelOverflow::Auto`] switches to a rotated draw.
pub const AUTO_ROTATE_DROP_THRESHOLD: f64 = 0.5;

/// Extra bottom-margin (px) an X axis over `scale` needs to fit its OWN
/// widest tick label rotated by `degrees` — [`measure_y_axis_gutter`]'s
/// bottom-axis counterpart, used by a figure sizing its own `MARGIN_BOTTOM`
/// under [`LabelOverflow::Rotate`]/`Auto`.
///
/// For [`LabelOverflow::Auto`] specifically: since whether a rotate
/// actually triggers depends on the plot's own rendered width (only known
/// once the margin itself is already committed — a chicken-and-egg a
/// figure's own margin-sizing pass cannot resolve), a figure reserves
/// this SAME rotated extent conservatively whenever `Auto` is configured,
/// regardless of whether the eventual draw ends up rotating or not.
/// Reserving slightly more than strictly needed is a harmless, safe
/// over-allocation; reserving too little (assuming `Auto` will never
/// rotate) risks the exact clipping this item exists to prevent.
pub fn measure_rotated_x_axis_gutter(ctx: &mut dyn RenderContext, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, degrees: f64) -> f64 {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return 0.0;
    }
    ctx.set_font(&theme.label_font);
    let widest_w = ticks.iter().map(|t| ctx.measure_text(&t.label)).fold(0.0_f64, f64::max);
    let row_h = ticks.iter().map(|t| ctx.text_bounds(&t.label, &theme.label_font).h).fold(0.0_f64, f64::max).max(1.0);
    let (_, rotated_h) = rotated_label_extent(widest_w, row_h, degrees);
    TICK_LENGTH + LABEL_GAP + rotated_h + LABEL_GAP
}

/// `(left_overhang, right_overhang)` (px, each `>= 0.0`) an X axis over
/// `scale` needs so its own FIRST/LAST tick label — center-aligned under
/// its own tick, per [`draw_x_axis`]'s convention — never draws past the
/// plot rect's own left/right edge.
///
/// Distinct from [`measure_y_axis_gutter`] (which sizes the LEFT margin
/// for the PERPENDICULAR Y axis's own RIGHT-aligned tick labels — a
/// margin an X-tick's CENTER-aligned label never interacts with, since a
/// right-aligned label's own text always sits entirely on one side of its
/// anchor). An X-tick's label straddles its own tick on BOTH sides — an
/// INTERIOR tick's overhang only eats into inter-tick whitespace
/// (harmless, self-correcting via [`resolve_label_priority`]'s own
/// collision skip), but nothing protects the FIRST tick's own LEFT half
/// or the LAST tick's own RIGHT half from extending straight past the
/// plot rect's own boundary — confirmed as a real, previously-unguarded
/// gap (a wide extreme label, e.g. a [`crate::scale::SymlogScale`] axis's
/// own `"-100,000.00"`, visibly clips past a narrow figure's own left
/// edge with nothing reserving room for it).
pub fn measure_x_axis_extreme_overhang(ctx: &mut dyn RenderContext, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) -> (f64, f64) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return (0.0, 0.0);
    }
    ctx.set_font(&theme.label_font);
    let left_overhang = ctx.measure_text(&ticks[0].label) / 2.0;
    let right_overhang = ctx.measure_text(&ticks[ticks.len() - 1].label) / 2.0;
    (left_overhang, right_overhang)
}

/// Dry-run the plain greedy-skip pass over `ticks` (already positioned via
/// `area`/`scale`) and return the fraction that would be DROPPED — shared
/// by [`draw_x_axis_overflow`]'s own [`LabelOverflow::Auto`] branch. Reads
/// `ctx.measure_text` only (no drawing), so it's safe to call before
/// committing to a final draw pass.
fn greedy_skip_drop_fraction(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, ticks: &[Tick]) -> f64 {
    if ticks.is_empty() {
        return 0.0;
    }
    let mut drawn = 0usize;
    let mut last_label_right = f64::MIN;
    for tick in ticks {
        let x = area.x(scale, tick.value);
        let half_w = ctx.measure_text(&tick.label) / 2.0;
        if x - half_w < last_label_right + LABEL_GAP {
            continue;
        }
        drawn += 1;
        last_label_right = x + half_w;
    }
    1.0 - (drawn as f64 / ticks.len() as f64)
}

/// Same as [`draw_x_axis`], but resolves per-label collision through
/// `overflow` instead of always skipping — see [`LabelOverflow`]'s own
/// docs. [`LabelOverflow::Skip`] (the default) renders byte-identically
/// to [`draw_x_axis`].
pub fn draw_x_axis_overflow(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, overflow: LabelOverflow) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }

    let axis_y = area.rect.bottom();
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(area.rect.x, axis_y);
    ctx.line_to(area.rect.right(), axis_y);
    ctx.stroke();

    ctx.set_font(&theme.label_font);

    let rotate_degrees = match overflow {
        LabelOverflow::Skip => None,
        LabelOverflow::Rotate(degrees) => Some(degrees),
        LabelOverflow::Auto => {
            if greedy_skip_drop_fraction(ctx, area, scale, &ticks) > AUTO_ROTATE_DROP_THRESHOLD {
                Some(AUTO_ROTATE_DEGREES)
            } else {
                None
            }
        }
    };

    match rotate_degrees {
        None => {
            // Byte-identical to `draw_x_axis`'s own priority-aware
            // skip — same footprint-building + `resolve_label_priority`
            // call, so `LabelOverflow::Skip` can never drift from the
            // plain `draw_x_axis` entry point's own behavior.
            ctx.set_text_align(TextAlign::Center);
            ctx.set_text_baseline(TextBaseline::Top);
            let footprints: Vec<LabelFootprint> = ticks
                .iter()
                .map(|t| LabelFootprint { center: area.x(scale, t.value), half_extent: ctx.measure_text(&t.label) / 2.0, priority: scale.tick_priority(t.value) })
                .collect();
            let visible = resolve_label_priority(&footprints, LABEL_GAP);
            for (i, tick) in ticks.iter().enumerate() {
                let x = area.x(scale, tick.value);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.begin_path();
                ctx.move_to(x, axis_y);
                ctx.line_to(x, axis_y + TICK_LENGTH);
                ctx.stroke();
                if !visible[i] {
                    continue;
                }
                ctx.set_fill_color(&theme.label_color);
                ctx.fill_text(&tick.label, x, axis_y + TICK_LENGTH + LABEL_GAP);
            }
        }
        Some(degrees) => {
            // Every label rotates, anchored at its own tick x — no
            // collision skip needed (see `LabelOverflow::Rotate`'s own
            // doc comment).
            // Anchor at the label's own TOP-RIGHT corner (`Right` align +
            // `Top` baseline, both evaluated in the LOCAL pre-rotation
            // frame) — after the `-degrees` rotation this corner sits
            // closest to the tick, with the rest of the label swinging
            // down and to the left, entirely BELOW `axis_y` (never
            // overlapping the plot's own baseline/gridline). The
            // conventional D3/Chart.js rotated-tick-label anchor.
            ctx.set_text_align(TextAlign::Right);
            ctx.set_text_baseline(TextBaseline::Top);
            for tick in &ticks {
                let x = area.x(scale, tick.value);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.begin_path();
                ctx.move_to(x, axis_y);
                ctx.line_to(x, axis_y + TICK_LENGTH);
                ctx.stroke();

                ctx.save();
                ctx.translate(x, axis_y + TICK_LENGTH + LABEL_GAP);
                ctx.rotate(-degrees.to_radians());
                ctx.set_fill_color(&theme.label_color);
                ctx.fill_text(&tick.label, 0.0, 0.0);
                ctx.restore();
            }
        }
    }
}

/// Best-effort tick "step" derived from the first two ticks — same
/// "derive a display step from the scale's own ticks" convention
/// [`Scale::format_value`]'s default impl and [`crate::guide::crosshair`]'s
/// own `axis_step` helper already use, reused here so a
/// [`NumberFormat`]-formatted label picks the same sane decimal precision
/// an unformatted tick label would. Falls back to `1.0` for a degenerate/
/// single-tick set.
fn ticks_step(ticks: &[Tick]) -> f64 {
    if ticks.len() >= 2 {
        (ticks[1].value - ticks[0].value).abs().max(f64::EPSILON)
    } else {
        1.0
    }
}

/// Draw the bottom x-axis: baseline, downward tick marks, and tick labels
/// centered under each tick.
pub fn draw_x_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, None, None);
}

/// Same as [`draw_x_axis`], but re-formats every tick's own label through
/// `format` (e.g. [`NumberFormat::Si`]/`Percent`/`Currency`) instead of the
/// scale's own default [`Tick::label`] — the tick VALUES, positions, and
/// greedy-skip collision logic are entirely UNCHANGED (design law #1: one
/// tick-generation/collision algorithm; only the display STRING differs).
pub fn draw_x_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, Some(format), None);
}

/// Same as [`draw_x_axis`], but styles each tick's own mark/label per
/// `scale.tick_weight(tick.value)` under `style` — see
/// [`AxisTickWeightStyle`]'s own doc comment. A `scale` that never
/// reports a weight (every scale except [`crate::scale::TimeScale`])
/// renders byte-identically to [`draw_x_axis`] regardless of `style`.
pub fn draw_x_axis_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &AxisTickWeightStyle) {
    draw_x_axis_impl(ctx, area, scale, theme, target_ticks, None, Some(style));
}

fn draw_x_axis_impl(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    scale: &dyn Scale,
    theme: &FigureTheme,
    target_ticks: usize,
    format: Option<NumberFormat>,
    weight_style: Option<&AxisTickWeightStyle>,
) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    let step = ticks_step(&ticks);

    let axis_y = area.rect.bottom();
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(area.rect.x, axis_y);
    ctx.line_to(area.rect.right(), axis_y);
    ctx.stroke();

    ctx.set_font(&theme.label_font);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Top);

    // Pass 1: resolve every tick's own display label + screen-space
    // footprint + `Scale::tick_priority`, then resolve which labels are
    // actually drawn — priority-aware (see `resolve_label_priority`'s
    // own doc comment for the byte-identical-to-today guarantee when
    // `scale` never overrides `tick_priority`).
    let labels: Vec<String> = ticks.iter().map(|t| match format { Some(f) => f.format(t.value, step), None => t.label.clone() }).collect();
    let footprints: Vec<LabelFootprint> = ticks
        .iter()
        .zip(labels.iter())
        .map(|(t, label)| LabelFootprint { center: area.x(scale, t.value), half_extent: ctx.measure_text(label) / 2.0, priority: scale.tick_priority(t.value) })
        .collect();
    let visible = resolve_label_priority(&footprints, LABEL_GAP);

    // Pass 2: draw every tick's own mark (never skipped — a tick mark
    // survives densification unconditionally, only its LABEL is ever
    // dropped) and, where `visible`, its label.
    for (i, tick) in ticks.iter().enumerate() {
        let x = area.x(scale, tick.value);

        // Only a weighted call site ever changes the tick's own stroke
        // width — the unweighted path leaves it at whatever the baseline
        // above already set (`1.0`), byte-identical to before this fix.
        let (tick_len, major_label_color) = match weight_style {
            Some(style) => {
                let weight = scale.tick_weight(tick.value);
                let (len, stroke_w) = resolve_tick_style(weight, style);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.set_stroke_width(stroke_w);
                let major_color = weight.filter(|w| w.is_major()).and_then(|_| style.major_label_color.as_deref());
                (len, major_color)
            }
            None => {
                ctx.set_stroke_color(&theme.axis_color);
                (TICK_LENGTH, None)
            }
        };
        ctx.begin_path();
        ctx.move_to(x, axis_y);
        ctx.line_to(x, axis_y + tick_len);
        ctx.stroke();

        if !visible[i] {
            continue; // would collide with a higher-or-equal-priority label
        }
        ctx.set_fill_color(major_label_color.unwrap_or(&theme.label_color));
        ctx.fill_text(&labels[i], x, axis_y + tick_len + LABEL_GAP);
    }
}

/// Draw the left y-axis: baseline, leftward tick marks, and right-aligned
/// tick labels.
pub fn draw_y_axis(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, None, None);
}

/// Same as [`draw_y_axis`], but re-formats every tick's own label through
/// `format` — see [`draw_x_axis_formatted`]'s own docs (identical
/// reasoning, Y side).
pub fn draw_y_axis_formatted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, format: NumberFormat) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, Some(format), None);
}

/// Same as [`draw_y_axis`], but styles each tick's own mark/label per
/// `scale.tick_weight(tick.value)` under `style` — see
/// [`draw_x_axis_weighted`]'s own docs (identical reasoning, Y side).
pub fn draw_y_axis_weighted(ctx: &mut dyn RenderContext, area: &PlotArea, scale: &dyn Scale, theme: &FigureTheme, target_ticks: usize, style: &AxisTickWeightStyle) {
    draw_y_axis_impl(ctx, area, scale, theme, target_ticks, None, Some(style));
}

fn draw_y_axis_impl(
    ctx: &mut dyn RenderContext,
    area: &PlotArea,
    scale: &dyn Scale,
    theme: &FigureTheme,
    target_ticks: usize,
    format: Option<NumberFormat>,
    weight_style: Option<&AxisTickWeightStyle>,
) {
    let ticks = scale.ticks(target_ticks);
    if ticks.is_empty() {
        return;
    }
    let step = ticks_step(&ticks);

    let axis_x = area.rect.x;
    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(axis_x, area.rect.y);
    ctx.line_to(axis_x, area.rect.bottom());
    ctx.stroke();

    let labels: Vec<String> = ticks.iter().map(|t| match format { Some(f) => f.format(t.value, step), None => t.label.clone() }).collect();

    // Row height for the vertical greedy-skip check — text_bounds (not
    // just measure_text) because the y-axis's collision axis is height,
    // not width.
    let row_height = labels
        .iter()
        .map(|label| ctx.text_bounds(label, &theme.label_font).h)
        .fold(0.0_f64, f64::max)
        .max(1.0);

    // `ticks` is ascending by value, and `area.y` is INVERTED (larger
    // value -> smaller y) — so screen y monotonically DECREASES as this
    // list advances. `resolve_label_priority`'s own symmetric
    // center-distance overlap test doesn't care which screen direction
    // that maps to (see its own doc comment for the proof it reduces
    // exactly to this axis's pre-existing top/bottom pairwise test in
    // the uniform-priority default case).
    let footprints: Vec<LabelFootprint> =
        ticks.iter().map(|t| LabelFootprint { center: area.y(scale, t.value), half_extent: row_height / 2.0, priority: scale.tick_priority(t.value) }).collect();
    let visible = resolve_label_priority(&footprints, LABEL_GAP);

    for (i, (tick, label)) in ticks.iter().zip(labels.iter()).enumerate() {
        let y = area.y(scale, tick.value);

        // Only a weighted call site ever changes the tick's own stroke
        // width — the unweighted path leaves it at whatever the baseline
        // above already set (`1.0`), byte-identical to before this fix.
        let (tick_len, major_label_color) = match weight_style {
            Some(style) => {
                let weight = scale.tick_weight(tick.value);
                let (len, stroke_w) = resolve_tick_style(weight, style);
                ctx.set_stroke_color(&theme.axis_color);
                ctx.set_stroke_width(stroke_w);
                let major_color = weight.filter(|w| w.is_major()).and_then(|_| style.major_label_color.as_deref());
                (len, major_color)
            }
            None => {
                ctx.set_stroke_color(&theme.axis_color);
                (TICK_LENGTH, None)
            }
        };
        ctx.begin_path();
        ctx.move_to(axis_x - tick_len, y);
        ctx.line_to(axis_x, y);
        ctx.stroke();

        if !visible[i] {
            continue; // would collide with a higher-or-equal-priority label
        }
        draw_label_right_aligned(ctx, label, axis_x - tick_len - LABEL_GAP, y, major_label_color.unwrap_or(&theme.label_color), &theme.label_font);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use crate::theme::FigureTheme;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(40.0, 10.0, 300.0, 150.0))
    }

    #[test]
    fn ticks_step_derives_from_the_first_two_ticks() {
        let ticks = vec![
            Tick { value: 0.0, label: "0".to_owned() },
            Tick { value: 10.0, label: "10".to_owned() },
            Tick { value: 20.0, label: "20".to_owned() },
        ];
        assert_eq!(ticks_step(&ticks), 10.0);
    }

    #[test]
    fn ticks_step_falls_back_to_one_for_a_single_tick() {
        let ticks = vec![Tick { value: 5.0, label: "5".to_owned() }];
        assert_eq!(ticks_step(&ticks), 1.0);
    }

    #[test]
    fn draw_x_axis_formatted_renders_without_panicking_for_every_variant() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 5000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for format in [NumberFormat::Plain, NumberFormat::Thousands, NumberFormat::Si, NumberFormat::Percent, NumberFormat::Currency("$")] {
            let result = render_to_png(&spec, |ctx| {
                draw_x_axis_formatted(ctx, &area(), &scale, &theme, 5, format);
            });
            assert!(result.is_ok(), "draw_x_axis_formatted must render cleanly for {format:?}");
        }
    }

    #[test]
    fn draw_y_axis_formatted_renders_without_panicking_for_every_variant() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(-500.0, 500.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for format in [NumberFormat::Plain, NumberFormat::Thousands, NumberFormat::Si, NumberFormat::Percent, NumberFormat::Currency("$")] {
            let result = render_to_png(&spec, |ctx| {
                draw_y_axis_formatted(ctx, &area(), &scale, &theme, 5, format);
            });
            assert!(result.is_ok(), "draw_y_axis_formatted must render cleanly for {format:?}");
        }
    }

    #[test]
    fn formatted_with_thousands_default_is_the_same_shape_as_unformatted() {
        // Not a byte-for-byte draw-op capture (this crate's own headless
        // proof convention doesn't expose one for `guide::axis`), but both
        // paths must at minimum render successfully over the SAME scale —
        // `NumberFormat::default()` resolving to `Thousands` is proven at
        // the pure-formatter level in `scale::format`'s own tests
        // (`default_matches_current_thousands_output`).
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let unformatted = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 5));
        let formatted = render_to_png(&spec, |ctx| draw_x_axis_formatted(ctx, &area(), &scale, &theme, 5, NumberFormat::default()));
        assert!(unformatted.is_ok() && formatted.is_ok());
    }

    // ── TimeScale tick-weight wiring (item A5) ──────────────────────────

    #[test]
    fn resolve_tick_style_distinguishes_every_weight_tier() {
        let style = AxisTickWeightStyle::default();
        let major = resolve_tick_style(Some(TickMarkWeight::Year), &style);
        let medium = resolve_tick_style(Some(TickMarkWeight::Day), &style);
        let minor = resolve_tick_style(Some(TickMarkWeight::Minute1), &style);
        let none = resolve_tick_style(None, &style);

        assert_ne!(major, medium, "a major tick must resolve differently from a medium one");
        assert_ne!(medium, minor, "a medium tick must resolve differently from a minor one");
        assert_eq!(minor, none, "no weight (every non-TimeScale scale) resolves the SAME as an explicit minor tick");
        assert!(major.0 > medium.0 && medium.0 > minor.0, "tick length must strictly increase major > medium > minor");
        assert!(major.1 > medium.1 && medium.1 > minor.1, "stroke width must strictly increase major > medium > minor");
    }

    #[test]
    fn flat_style_matches_minor_for_every_weight_tier() {
        let flat = AxisTickWeightStyle::flat();
        let major = resolve_tick_style(Some(TickMarkWeight::Year), &flat);
        let minor = resolve_tick_style(None, &flat);
        assert_eq!(major, minor, "AxisTickWeightStyle::flat must draw every tick identically regardless of weight");
        assert_eq!(major, (TICK_LENGTH, 1.0), "flat must match the unweighted draw's own fixed tick length/stroke width");
    }

    #[test]
    fn weighted_draw_over_a_non_time_scale_renders_byte_identical_to_the_unweighted_draw() {
        // The safety property the whole wiring depends on: a scale that
        // never reports a tick weight (every scale except TimeScale) must
        // be COMPLETELY unaffected by switching a call site from
        // `draw_x_axis`/`draw_y_axis` to the weighted entry point.
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };

        let unweighted = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 6)).expect("unweighted x-axis render");
        let weighted = render_to_png(&spec, |ctx| draw_x_axis_weighted(ctx, &area(), &scale, &theme, 6, &AxisTickWeightStyle::default()))
            .expect("weighted x-axis render over a non-TimeScale scale");
        assert_eq!(unweighted, weighted, "a non-TimeScale x-axis must render identically through the weighted entry point");

        let unweighted_y = render_to_png(&spec, |ctx| draw_y_axis(ctx, &area(), &scale, &theme, 6)).expect("unweighted y-axis render");
        let weighted_y = render_to_png(&spec, |ctx| draw_y_axis_weighted(ctx, &area(), &scale, &theme, 6, &AxisTickWeightStyle::default()))
            .expect("weighted y-axis render over a non-TimeScale scale");
        assert_eq!(unweighted_y, weighted_y, "a non-TimeScale y-axis must render identically through the weighted entry point");
    }

    #[test]
    fn a_month_spanning_time_axis_resolves_distinguishable_major_and_minor_tick_styles() {
        // Item A5's own explicit gate: "test that a month-spanning time
        // axis renders distinguishable major vs. minor ticks." Builds a
        // real `TimeScale` over ~2 months (the same regime
        // `scale::time`'s own `ticks_over_two_months_pick_day_scale_
        // cadence` fixture uses — day-cadence ticks, some of which land on
        // a real month/day-1 boundary) and proves the RESOLVED per-tick
        // style genuinely differs across the generated tick set, not just
        // in isolated unit fixtures above.
        use crate::scale::TimeScale;

        let jan1_2024 = 1_704_067_200.0; // 2024-01-01T00:00:00Z
        const DAY_SECS: f64 = 86_400.0;
        let scale = TimeScale::new(jan1_2024, jan1_2024 + 62.0 * DAY_SECS);
        let ticks = scale.ticks(6);
        assert!(ticks.len() >= 2, "fixture must produce a real multi-tick set");

        let style = AxisTickWeightStyle::default();
        let resolved: Vec<(f64, f64)> =
            ticks.iter().map(|t| resolve_tick_style(scale.tick_weight(t.value), &style)).collect();
        let mut unique = resolved.clone();
        unique.sort_by(|a, b| a.partial_cmp(b).unwrap());
        unique.dedup();
        assert!(
            unique.len() >= 2,
            "a month-spanning time axis must resolve at least 2 distinct tick styles (major vs. minor), got {resolved:?}"
        );

        // And the actual render must succeed end to end.
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 600, height_px: 250, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| draw_x_axis_weighted(ctx, &area(), &scale, &theme, 6, &style));
        assert!(result.is_ok());
    }

    // ── Measured Y-axis gutter (item 2) ─────────────────────────────────

    #[test]
    fn measure_y_axis_gutter_grows_for_a_deliberately_wide_label() {
        let theme = FigureTheme::dark();
        let narrow_scale = LinearScale::new(0.0, 9.0);
        let wide_scale = LinearScale::new(0.0, 999_999_999.0);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut narrow_gutter = 0.0_f64;
        let mut wide_gutter = 0.0_f64;
        render_to_png(&spec, |ctx| {
            narrow_gutter = measure_y_axis_gutter(ctx, &narrow_scale, &theme, 5);
            wide_gutter = measure_y_axis_gutter(ctx, &wide_scale, &theme, 5);
        })
        .expect("render");
        assert!(wide_gutter > narrow_gutter, "a wider tick label must measure a larger gutter (narrow={narrow_gutter}, wide={wide_gutter})");
    }

    #[test]
    fn measure_y_axis_gutter_is_zero_for_an_empty_tick_set() {
        let theme = FigureTheme::dark();
        let band = crate::scale::BandScale::new(Vec::new(), 0.1);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut gutter = -1.0_f64;
        render_to_png(&spec, |ctx| gutter = measure_y_axis_gutter(ctx, &band, &theme, 5)).expect("render");
        assert_eq!(gutter, 0.0);
    }

    // ── X-axis extreme-tick overhang (leftmost/rightmost label clipping) ─

    #[test]
    fn measure_x_axis_extreme_overhang_is_zero_for_an_empty_tick_set() {
        let theme = FigureTheme::dark();
        let band = crate::scale::BandScale::new(Vec::new(), 0.1);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut overhang = (-1.0_f64, -1.0_f64);
        render_to_png(&spec, |ctx| overhang = measure_x_axis_extreme_overhang(ctx, &band, &theme, 5)).expect("render");
        assert_eq!(overhang, (0.0, 0.0));
    }

    #[test]
    fn measure_x_axis_extreme_overhang_grows_for_a_deliberately_wide_extreme_label() {
        let theme = FigureTheme::dark();
        let narrow_scale = LinearScale::new(0.0, 9.0);
        let wide_scale = LinearScale::new(-999_999_999.0, 999_999_999.0);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut narrow_overhang = (0.0_f64, 0.0_f64);
        let mut wide_overhang = (0.0_f64, 0.0_f64);
        render_to_png(&spec, |ctx| {
            narrow_overhang = measure_x_axis_extreme_overhang(ctx, &narrow_scale, &theme, 5);
            wide_overhang = measure_x_axis_extreme_overhang(ctx, &wide_scale, &theme, 5);
        })
        .expect("render");
        assert!(wide_overhang.0 > narrow_overhang.0, "a wider leftmost tick label must measure a larger left overhang");
        assert!(wide_overhang.1 > narrow_overhang.1, "a wider rightmost tick label must measure a larger right overhang");
    }

    #[test]
    fn measure_x_axis_extreme_overhang_is_half_the_extreme_labels_own_measured_width() {
        let theme = FigureTheme::dark();
        let scale = crate::scale::SymlogScale::new(-1_000_000.0, 1_000_000.0);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut overhang = (0.0_f64, 0.0_f64);
        let mut first_label_w = 0.0_f64;
        let mut last_label_w = 0.0_f64;
        render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            let ticks = scale.ticks(6);
            first_label_w = ctx.measure_text(&ticks[0].label);
            last_label_w = ctx.measure_text(&ticks[ticks.len() - 1].label);
            overhang = measure_x_axis_extreme_overhang(ctx, &scale, &theme, 6);
        })
        .expect("render");
        assert!((overhang.0 - first_label_w / 2.0).abs() < 1e-6);
        assert!((overhang.1 - last_label_w / 2.0).abs() < 1e-6);
    }

    // ── Rotated label extent + overflow (item 4) ────────────────────────

    #[test]
    fn rotated_label_extent_at_zero_degrees_is_the_original_box() {
        let (w, h) = rotated_label_extent(40.0, 12.0, 0.0);
        assert!((w - 40.0).abs() < 1e-9);
        assert!((h - 12.0).abs() < 1e-9);
    }

    #[test]
    fn rotated_label_extent_at_ninety_degrees_swaps_width_and_height() {
        let (w, h) = rotated_label_extent(40.0, 12.0, 90.0);
        assert!((w - 12.0).abs() < 1e-6);
        assert!((h - 40.0).abs() < 1e-6);
    }

    #[test]
    fn rotated_label_extent_at_forty_five_degrees_is_narrower_than_the_full_width() {
        let (w, _h) = rotated_label_extent(40.0, 12.0, 45.0);
        assert!(w < 40.0, "a 45-degree rotation must reduce the label's own horizontal footprint below its unrotated width");
    }

    #[test]
    fn draw_x_axis_overflow_skip_renders_byte_identical_to_draw_x_axis() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 1_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let plain = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 6)).expect("plain render");
        let overflow = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &scale, &theme, 6, LabelOverflow::Skip)).expect("overflow render");
        assert_eq!(plain, overflow, "LabelOverflow::Skip must render byte-identically to draw_x_axis");
    }

    #[test]
    fn draw_x_axis_overflow_rotate_renders_without_panicking_and_differs_from_skip() {
        let theme = FigureTheme::dark();
        // Long category-like labels over a narrow band so the plain
        // greedy-skip pass would drop most of them.
        let categories: Vec<String> = (0..12).map(|i| format!("category-name-{i}")).collect();
        let band = crate::scale::BandScale::new(categories, 0.1);
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let skip = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &band, &theme, band.len(), LabelOverflow::Skip)).expect("skip render");
        let rotated =
            render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &band, &theme, band.len(), LabelOverflow::Rotate(45.0))).expect("rotate render");
        assert_ne!(rotated, skip, "a rotated draw must render visibly differently from the plain skip draw");
    }

    #[test]
    fn draw_x_axis_overflow_auto_rotates_when_labels_would_mostly_collide() {
        let theme = FigureTheme::dark();
        let categories: Vec<String> = (0..12).map(|i| format!("category-name-{i}")).collect();
        let band = crate::scale::BandScale::new(categories, 0.1);
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let auto = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &band, &theme, band.len(), LabelOverflow::Auto)).expect("auto render");
        let rotated =
            render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &band, &theme, band.len(), LabelOverflow::Rotate(AUTO_ROTATE_DEGREES))).expect("rotate render");
        assert_eq!(auto, rotated, "Auto must fall back to Rotate(AUTO_ROTATE_DEGREES) when most labels would collide");
    }

    #[test]
    fn draw_x_axis_overflow_auto_matches_skip_when_labels_comfortably_fit() {
        let theme = FigureTheme::dark();
        let scale = LinearScale::new(0.0, 5.0);
        let spec = ExportSpec { width_px: 600, height_px: 200, dpr: 1.0, background: None };
        let auto = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &scale, &theme, 5, LabelOverflow::Auto)).expect("auto render");
        let skip = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &scale, &theme, 5, LabelOverflow::Skip)).expect("skip render");
        assert_eq!(auto, skip, "Auto must resolve to the plain skip draw when few, short labels already fit");
    }

    #[test]
    fn measure_rotated_x_axis_gutter_grows_with_the_rotation_angle_up_to_ninety_degrees() {
        let theme = FigureTheme::dark();
        let categories: Vec<String> = (0..3).map(|i| format!("category-name-{i}")).collect();
        let band = crate::scale::BandScale::new(categories, 0.1);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut flat = 0.0_f64;
        let mut rotated = 0.0_f64;
        render_to_png(&spec, |ctx| {
            flat = measure_rotated_x_axis_gutter(ctx, &band, &theme, band.len(), 0.0);
            rotated = measure_rotated_x_axis_gutter(ctx, &band, &theme, band.len(), 90.0);
        })
        .expect("render");
        assert!(rotated > flat, "a 90-degree rotated gutter must reserve more height than an unrotated (0-degree) one");
    }

    /// The item's own explicit gate: a figure sizing its bottom margin
    /// under `LabelOverflow::Rotate(45.0)` (the actual angle used by
    /// `BarFigure`/`HistogramFigure`'s own before/after proof) must
    /// reserve MORE height than the flat `TICK_LENGTH + LABEL_GAP`
    /// baseline every unrotated axis reserves — so a rotated label's own
    /// bounding box never overflows past a margin sized as if it were
    /// still horizontal.
    #[test]
    fn measure_rotated_x_axis_gutter_at_forty_five_degrees_exceeds_the_unrotated_baseline() {
        let theme = FigureTheme::dark();
        let categories: Vec<String> = (0..8).map(|i| format!("category-{i}")).collect();
        let band = crate::scale::BandScale::new(categories, 0.1);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut unrotated_baseline = 0.0_f64;
        let mut rotated_45 = 0.0_f64;
        render_to_png(&spec, |ctx| {
            unrotated_baseline = TICK_LENGTH + LABEL_GAP;
            rotated_45 = measure_rotated_x_axis_gutter(ctx, &band, &theme, band.len(), 45.0);
        })
        .expect("render");
        assert!(
            rotated_45 > unrotated_baseline,
            "a 45-degree rotated gutter ({rotated_45}) must reserve more height than the flat unrotated baseline ({unrotated_baseline})"
        );
    }

    // ── Priority-aware label-collision skip (defect fix: symlog's own
    // zero crossing was being dropped) ──────────────────────────────────

    /// The core "nothing changes for a scale that doesn't opt in"
    /// guarantee, checked directly against a hand-rolled reimplementation
    /// of the OLD single-pass nearest-neighbor-only algorithm — not just
    /// asserted in prose.
    #[test]
    fn priority_skip_with_uniform_priority_matches_the_plain_greedy_skip() {
        let gap = 4.0;
        // Deliberately irregular spacing/half-widths so a nearest-neighbor-
        // only vs. full-scan difference would actually show up if the
        // reduction proof in `resolve_label_priority`'s own doc comment
        // were wrong.
        let footprints = vec![
            LabelFootprint { center: 0.0, half_extent: 5.0, priority: TickPriority::Minor },
            LabelFootprint { center: 8.0, half_extent: 5.0, priority: TickPriority::Minor }, // collides with 0.0
            LabelFootprint { center: 30.0, half_extent: 3.0, priority: TickPriority::Minor },
            LabelFootprint { center: 34.0, half_extent: 3.0, priority: TickPriority::Minor }, // collides with 30.0
            LabelFootprint { center: 60.0, half_extent: 8.0, priority: TickPriority::Minor },
        ];

        let visible = resolve_label_priority(&footprints, gap);

        // Manual re-implementation of the ORIGINAL plain nearest-neighbor
        // greedy skip (position order, test against the LAST accepted
        // only).
        let mut expected = vec![false; footprints.len()];
        let mut last_right = f64::MIN;
        for (i, f) in footprints.iter().enumerate() {
            let left = f.center - f.half_extent;
            if left < last_right + gap {
                continue;
            }
            expected[i] = true;
            last_right = f.center + f.half_extent;
        }

        assert_eq!(visible, expected, "uniform-priority input must degrade to EXACTLY the original nearest-neighbor-only greedy skip");
    }

    /// The item's own explicit gate: when MAJORS alone still collide,
    /// dropping must degrade sanely — a `Critical` tick is never dropped
    /// in favor of a `Major`, and among competing majors the two EXTREMES
    /// (by position) are preferred over an interior one.
    #[test]
    fn majors_colliding_among_themselves_degrade_by_preferring_extremes_and_critical() {
        let gap = 4.0;
        let footprints = vec![
            LabelFootprint { center: 0.0, half_extent: 10.0, priority: TickPriority::Major },    // leftmost extreme
            LabelFootprint { center: 15.0, half_extent: 10.0, priority: TickPriority::Major },   // interior
            LabelFootprint { center: 30.0, half_extent: 10.0, priority: TickPriority::Critical }, // zero
            LabelFootprint { center: 45.0, half_extent: 10.0, priority: TickPriority::Major },   // interior
            LabelFootprint { center: 60.0, half_extent: 10.0, priority: TickPriority::Major },   // rightmost extreme
        ];
        // Every adjacent pair is 15 apart, well inside the 24-unit
        // collision threshold (10 + 10 + 4) — a genuinely dense, fully
        // colliding fixture.
        let visible = resolve_label_priority(&footprints, gap);

        assert!(visible[2], "the Critical (zero) tick must never be dropped in favor of a Major");
        assert!(visible[0], "the leftmost MAJOR extreme must be preferred over an interior major");
        assert!(visible[4], "the rightmost MAJOR extreme must be preferred over an interior major");
        assert!(!visible[1], "an interior major must be the one dropped once extremes+zero already claim the space");
        assert!(!visible[3], "an interior major must be the one dropped once extremes+zero already claim the space");
    }

    /// The item's own headline regression: a `SymlogScale` axis dense
    /// enough that its own zero-crossing label would collide with a
    /// neighbor must still keep it visible — reproduces the exact defect
    /// reported from `figures_wave4a_symlog_backends.png` (a real
    /// generated tick set through the real `resolve_label_priority` call,
    /// not a synthetic footprint list).
    #[test]
    fn a_dense_symlog_axis_keeps_the_zero_label_visible() {
        use crate::scale::SymlogScale;

        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        let theme = FigureTheme::dark();
        let a = area();
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let mut zero_visible = false;
        let mut had_a_collision_to_resolve = false;
        render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            let ticks = scale.ticks(6);
            let footprints: Vec<LabelFootprint> = ticks
                .iter()
                .map(|t| LabelFootprint { center: a.x(&scale, t.value), half_extent: ctx.measure_text(&t.label) / 2.0, priority: scale.tick_priority(t.value) })
                .collect();
            let visible = resolve_label_priority(&footprints, LABEL_GAP);
            had_a_collision_to_resolve = visible.iter().any(|&v| !v);
            let zero_idx = ticks.iter().position(|t| t.value.abs() < 1e-9).expect("a domain spanning zero must generate a zero tick");
            zero_visible = visible[zero_idx];
        })
        .expect("render");
        assert!(had_a_collision_to_resolve, "fixture must actually be dense enough to force at least one label to drop — otherwise this test proves nothing");
        assert!(zero_visible, "the zero-crossing label must survive a dense symlog axis's own label-collision skip");
    }

    /// The item's own second explicit gate: a dense `LogScale` axis must
    /// keep every decade (Major) label while dropping intermediate 2x/5x
    /// subdivision (Minor) labels.
    #[test]
    fn a_dense_log_axis_keeps_decade_labels_while_dropping_intermediate_subdivisions() {
        use crate::scale::LogScale;

        let scale = LogScale::new(1.0, 100_000.0);
        let theme = FigureTheme::dark();
        let a = area();
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let mut every_major_visible = true;
        let mut any_minor_dropped = false;
        render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            let ticks = scale.ticks(8); // 5 decades < target 8 -> 2x/5x subdivisions included too
            let footprints: Vec<LabelFootprint> = ticks
                .iter()
                .map(|t| LabelFootprint { center: a.x(&scale, t.value), half_extent: ctx.measure_text(&t.label) / 2.0, priority: scale.tick_priority(t.value) })
                .collect();
            let visible = resolve_label_priority(&footprints, LABEL_GAP);
            for (i, t) in ticks.iter().enumerate() {
                match scale.tick_priority(t.value) {
                    TickPriority::Major if !visible[i] => every_major_visible = false,
                    TickPriority::Minor if !visible[i] => any_minor_dropped = true,
                    _ => {}
                }
            }
        })
        .expect("render");
        assert!(any_minor_dropped, "fixture must actually be dense enough to force at least one Minor subdivision to drop — otherwise this test proves nothing");
        assert!(every_major_visible, "every decade (Major) label must survive even when the axis is dense enough to drop minor subdivisions");
    }

    #[test]
    fn draw_x_axis_overflow_skip_still_renders_byte_identical_to_draw_x_axis_after_the_priority_fix() {
        // Regression guard: both entry points now build footprints and
        // call `resolve_label_priority` independently — this proves they
        // can never drift apart, over a scale that actually reports
        // non-uniform priorities (not just the pre-existing non-Time-scale
        // check).
        use crate::scale::SymlogScale;

        let theme = FigureTheme::dark();
        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let plain = render_to_png(&spec, |ctx| draw_x_axis(ctx, &area(), &scale, &theme, 6)).expect("plain render");
        let overflow = render_to_png(&spec, |ctx| draw_x_axis_overflow(ctx, &area(), &scale, &theme, 6, LabelOverflow::Skip)).expect("overflow render");
        assert_eq!(plain, overflow, "LabelOverflow::Skip must render byte-identically to draw_x_axis even for a scale with non-uniform tick priority");
    }
}
