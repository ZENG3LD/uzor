//! Stateless mark draw functions — `(ctx, area, scale(s), data, style) ->
//! ()`. Every function only writes to `ctx`; [`crate::coord::PlotArea`]
//! plus the scale(s) passed in are the only positioning inputs (design
//! laws #1 and #3 — one shared transform, pure draw over borrowed data).
//!
//! `BatchPainter` is used wherever the shape repeats per-datapoint
//! (`draw_line_batch`/`draw_circle_batch`/`stroke_polyline`) — same rule
//! `uzor-graph::render` already follows.

pub mod area;
pub mod line;
pub mod point;
pub mod rect;
pub mod text;

/// How [`area::draw_area`]/[`line::draw_polyline`] handle a non-finite
/// (`NaN`/`±infinity`) coordinate inside their own `points` slice.
///
/// Before this enum existed, neither function had ANY non-finite
/// handling at all — every backend's own path builder was fed the raw
/// coordinate directly, with backend-dependent (and in `tiny-skia`'s
/// case, whole-path-dropping) results. That is a genuine defect, not a
/// debatable default: [`GapPolicy::Break`] (this enum's `Default`) is the
/// fix, not a preserved "old behavior" — there was no correct old
/// behavior to preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GapPolicy {
    /// DEFAULT. Split `points` into maximal runs of consecutive finite
    /// pairs at every non-finite coordinate; draw/fill each run
    /// independently — a visible break in the line/area exactly where a
    /// real sample is missing. The conventional "gap in the line"
    /// behavior every serious charting library (D3, Highcharts,
    /// matplotlib, the TradingView engine this crate's own scale math was
    /// ported from) implements as its own default (`connectNulls: false`).
    #[default]
    Break,
    /// Drop every non-finite point from `points` and draw ONE continuous
    /// stroke/fill spanning the remaining finite points — a straight
    /// bridge directly connecting the two points that flank any gap
    /// (Highcharts' `connectNulls: true`). Never shows a break.
    Connect,
    /// A conservative all-or-nothing policy: if ANY point in `points` is
    /// non-finite, draw NOTHING at all this call — never guess at or
    /// silently patch over incomplete data by showing only the valid
    /// stretches. Suited to a context where a partial render could read
    /// as a COMPLETE (and therefore misleading) picture — e.g. a
    /// forensic/financial series where "some samples are missing" should
    /// block the whole mark, not quietly thin it.
    Skip,
}

/// Split `points` into the runs [`area::draw_area`]/[`line::draw_polyline`]
/// each draw/fill independently under `policy` — see [`GapPolicy`]'s own
/// docs for exactly what each variant produces. A run shorter than 2
/// points is still returned (both call sites already guard `run.len() <
/// 2` themselves before drawing, matching every other mark's own
/// `points.len() < 2` no-op convention).
pub(crate) fn gap_runs(points: &[(f64, f64)], policy: GapPolicy) -> Vec<Vec<(f64, f64)>> {
    let is_finite = |&(px, py): &(f64, f64)| px.is_finite() && py.is_finite();

    if policy == GapPolicy::Skip && points.iter().any(|p| !is_finite(p)) {
        return Vec::new();
    }

    match policy {
        GapPolicy::Skip | GapPolicy::Connect => {
            let run: Vec<(f64, f64)> = points.iter().copied().filter(is_finite).collect();
            if run.is_empty() {
                Vec::new()
            } else {
                vec![run]
            }
        }
        GapPolicy::Break => {
            let mut runs = Vec::new();
            let mut current: Vec<(f64, f64)> = Vec::new();
            for &p in points {
                if is_finite(&p) {
                    current.push(p);
                } else if !current.is_empty() {
                    runs.push(std::mem::take(&mut current));
                }
            }
            if !current.is_empty() {
                runs.push(current);
            }
            runs
        }
    }
}

/// Line-cap style applied by [`line::draw_polyline`] before stroking — the
/// same three values [`uzor::render::RenderContext::set_line_cap`] already
/// accepts. Mirrors `uzor-export`'s own `PdfLineCap` (`Butt`/`Round`/
/// `Square`), not reused directly since that type is `pub(crate)` to the
/// PDF backend and this crate must not depend on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// DEFAULT — matches every render backend's own compiled-in default
    /// (e.g. `tiny-skia`'s `LineCap::Butt`) and this crate's pre-existing
    /// rendered output byte-for-byte: no figure ever called
    /// `set_line_cap` before this item, so every stroke rendered at
    /// whatever the backend happened to default to.
    #[default]
    Butt,
    Round,
    Square,
}

impl LineCap {
    pub fn as_str(self) -> &'static str {
        match self {
            LineCap::Butt => "butt",
            LineCap::Round => "round",
            LineCap::Square => "square",
        }
    }
}

/// Line-join style applied by [`line::draw_polyline`] before stroking —
/// see [`LineCap`]'s own docs (identical reasoning).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// DEFAULT — matches every render backend's own compiled-in default
    /// (e.g. `tiny-skia`'s `LineJoin::Miter`) and this crate's
    /// pre-existing rendered output byte-for-byte (see [`LineCap::Butt`]'s
    /// own doc comment for the same reasoning).
    #[default]
    Miter,
    Round,
    Bevel,
}

impl LineJoin {
    pub fn as_str(self) -> &'static str {
        match self {
            LineJoin::Miter => "miter",
            LineJoin::Round => "round",
            LineJoin::Bevel => "bevel",
        }
    }
}

/// Minimal per-mark visual style.
///
/// `color` is a CSS hex string (`"#rrggbb"` / `"#rrggbbaa"`), matching
/// every `set_fill_color`/`set_stroke_color` call across the uzor render
/// stack (same convention as `uzor-graph::render::category_color`).
#[derive(Debug, Clone)]
pub struct MarkStyle {
    pub color: String,
    pub stroke_width: f64,
    pub fill_alpha: f64,
    /// Missing-data (non-finite coordinate) handling — consulted by
    /// [`line::draw_polyline`]/[`area::draw_area`] only (every other mark
    /// in this module never receives a non-finite coordinate from any
    /// figure in this crate). See [`GapPolicy`]'s own docs. Defaults to
    /// [`GapPolicy::Break`] — a genuine defect fix, not a
    /// preserved-old-default option (this crate's line/area marks had
    /// ZERO non-finite handling before this item).
    pub gap_policy: GapPolicy,
    /// Line-cap style [`line::draw_polyline`] applies before stroking —
    /// see [`LineCap`]'s own docs. Defaults to [`LineCap::Butt`],
    /// byte-identical to this crate's pre-existing rendered output (no
    /// figure ever called `set_line_cap` before this item).
    pub line_cap: LineCap,
    /// Line-join style [`line::draw_polyline`] applies before stroking —
    /// see [`LineJoin`]'s own docs. Defaults to [`LineJoin::Miter`],
    /// byte-identical to this crate's pre-existing rendered output.
    pub line_join: LineJoin,
}

impl Default for MarkStyle {
    fn default() -> Self {
        Self {
            color: "#4d90fe".to_owned(),
            stroke_width: 1.5,
            fill_alpha: 1.0,
            gap_policy: GapPolicy::default(),
            line_cap: LineCap::default(),
            line_join: LineJoin::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nan_point() -> (f64, f64) {
        (f64::NAN, 1.0)
    }

    #[test]
    fn gap_runs_break_splits_at_every_non_finite_point() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), nan_point(), (3.0, 3.0), (4.0, 4.0)];
        let runs = gap_runs(&points, GapPolicy::Break);
        assert_eq!(runs.len(), 2, "a single interior gap must split into exactly 2 runs");
        assert_eq!(runs[0], vec![(0.0, 0.0), (1.0, 1.0)]);
        assert_eq!(runs[1], vec![(3.0, 3.0), (4.0, 4.0)]);
    }

    #[test]
    fn gap_runs_connect_filters_non_finite_and_returns_one_run() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), nan_point(), (3.0, 3.0), (4.0, 4.0)];
        let runs = gap_runs(&points, GapPolicy::Connect);
        assert_eq!(runs.len(), 1, "Connect must bridge the gap into a single continuous run");
        assert_eq!(runs[0], vec![(0.0, 0.0), (1.0, 1.0), (3.0, 3.0), (4.0, 4.0)]);
    }

    #[test]
    fn gap_runs_skip_draws_nothing_when_any_point_is_non_finite() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), nan_point(), (3.0, 3.0)];
        assert!(gap_runs(&points, GapPolicy::Skip).is_empty(), "Skip must draw NOTHING when the series has any gap");
    }

    #[test]
    fn gap_runs_skip_matches_break_when_there_is_no_gap_at_all() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)];
        assert_eq!(gap_runs(&points, GapPolicy::Skip), gap_runs(&points, GapPolicy::Break));
        assert_eq!(gap_runs(&points, GapPolicy::Skip), gap_runs(&points, GapPolicy::Connect));
    }

    #[test]
    fn gap_runs_leading_and_trailing_non_finite_points_are_trimmed_for_every_policy() {
        let points = vec![nan_point(), (1.0, 1.0), (2.0, 2.0), nan_point()];
        for policy in [GapPolicy::Break, GapPolicy::Connect] {
            let runs = gap_runs(&points, policy);
            assert_eq!(runs.len(), 1);
            assert_eq!(runs[0], vec![(1.0, 1.0), (2.0, 2.0)]);
        }
        assert!(gap_runs(&points, GapPolicy::Skip).is_empty());
    }

    #[test]
    fn gap_runs_all_non_finite_produces_no_runs_for_any_policy() {
        let points = vec![nan_point(), (f64::NAN, f64::NAN), (f64::INFINITY, 1.0)];
        for policy in [GapPolicy::Break, GapPolicy::Connect, GapPolicy::Skip] {
            assert!(gap_runs(&points, policy).is_empty(), "{policy:?} must produce no runs when every point is non-finite");
        }
    }

    #[test]
    fn gap_runs_single_valid_point_among_gaps_never_produces_a_drawable_run() {
        // A lone finite sample surrounded by non-finite ones on both sides
        // is still fewer than 2 points once filtered — every policy's own
        // runs are either empty or a single-element run, both of which
        // the calling draw functions already skip (run.len() < 2).
        let points = vec![nan_point(), (5.0, 5.0), nan_point()];
        for policy in [GapPolicy::Break, GapPolicy::Connect] {
            let runs = gap_runs(&points, policy);
            assert!(runs.iter().all(|r| r.len() < 2));
        }
        assert!(gap_runs(&points, GapPolicy::Skip).is_empty());
    }

    #[test]
    fn line_cap_and_line_join_as_str_match_the_render_context_string_convention() {
        assert_eq!(LineCap::Butt.as_str(), "butt");
        assert_eq!(LineCap::Round.as_str(), "round");
        assert_eq!(LineCap::Square.as_str(), "square");
        assert_eq!(LineJoin::Miter.as_str(), "miter");
        assert_eq!(LineJoin::Round.as_str(), "round");
        assert_eq!(LineJoin::Bevel.as_str(), "bevel");
    }

    #[test]
    fn mark_style_default_preserves_todays_rendered_behavior() {
        let style = MarkStyle::default();
        assert_eq!(style.gap_policy, GapPolicy::Break);
        assert_eq!(style.line_cap, LineCap::Butt);
        assert_eq!(style.line_join, LineJoin::Miter);
    }
}
