//! Shared dash-pattern geometry expansion — SVG/Canvas2D "walk the
//! on/off pattern along the path's own arc length" semantics, applied
//! in LOCAL (pre-transform) space so the active CTM scales the whole
//! dashed result exactly the way it scales stroke width and path
//! geometry (matches `tiny-skia`'s own `StrokeDash` — see
//! [`crate::scene::Dash`]'s own doc comment for the full reasoning).
//!
//! Reuses `kurbo::dash` (already a transitive dependency of every URX
//! backend via `crate::math`, and pinned to the identical `kurbo =
//! "0.13"` in both `uzor-urx-cpu` and `uzor-urx-wgpu`'s own
//! `Cargo.toml`) rather than a hand-rolled arc-length polyline splitter
//! or a new `lyon_algorithms` dependency:
//!
//! - `kurbo::dash` is an OFFICIAL, tested part of kurbo's own stroke
//!   module — its own doc comment: "expected to be useful when doing
//!   stroke expansion on GPU," precisely this crate's own two
//!   consumers' situation.
//! - It walks CURVES by real arc length (`ParamCurveArclen`), no
//!   pre-flatten pass needed — a hand-rolled polyline splitter would
//!   either need its own flatten step first (losing precision/adding a
//!   second tolerance knob to keep in sync with each backend's own) or
//!   only support straight lines.
//! - It emits a flat `PathEl` stream with a fresh `MoveTo` at the start
//!   of every "on" run — exactly the multi-subpath `BezPath` shape both
//!   consumers already handle with ZERO other changes:
//!   `uzor-urx-cpu::path::stroke_path_aa` already walks `MoveTo`-
//!   delimited chains (one per subpath) into its own capsule stroker,
//!   and `uzor-urx-wgpu::tessellate::build_lyon_path` already opens a
//!   fresh lyon subpath on every `MoveTo`.
//! - `lyon_algorithms` (which does carry its own dash iterator) was
//!   deliberately NOT added: `uzor-urx-wgpu` already depends on
//!   `lyon_path`/`lyon_tessellation` but not `lyon_algorithms`, and
//!   `uzor-urx-cpu` has no lyon dependency at all — `kurbo::dash`
//!   covers both consumers from a dependency ALREADY pinned identically
//!   in every crate that needs it, at zero new `Cargo.lock` surface.

use crate::math::BezPath;
use crate::scene::Dash;

/// Split `path`'s own geometry into its dashed "on"-run subpaths per
/// `dash.pattern`/`dash.phase`, still in `path`'s own (pre-transform)
/// coordinate space. The result is a normal multi-subpath `BezPath` —
/// hand it straight to whatever (undashed) stroke pipeline would have
/// consumed the original `path`; the caller's own `Stroke` should carry
/// `dash: None` from that point on (the dashing has already been
/// "spent" into the returned geometry, so nothing downstream needs to
/// see the pattern again).
///
/// Degrades to `path.clone()` (i.e. "no dashing applied," the ORIGINAL
/// undashed geometry) rather than panicking or looping forever on a
/// malformed pattern — see [`is_valid_pattern`]'s own doc comment for
/// exactly which patterns qualify. Every real `uzor-figures` guide call
/// site passes a small positive-length pattern, so this fallback is a
/// defensive backstop, not a path any production caller is expected to
/// hit.
pub fn dash_path(path: &BezPath, dash: &Dash) -> BezPath {
    if !is_valid_pattern(&dash.pattern) {
        return path.clone();
    }
    let pattern: Vec<f64> = dash.pattern.iter().map(|&v| v as f64).collect();
    kurbo::dash(path.elements().iter().copied(), dash.phase as f64, &pattern).collect()
}

/// A pattern is usable when non-empty, every entry is finite and
/// non-negative, and at least one entry is strictly positive. An empty
/// pattern would panic `kurbo::dash` (it indexes `dashes[0]`
/// unconditionally); an all-zero pattern has a zero period, which
/// `kurbo::dash`'s own offset normalization (`dash_offset.rem_euclid
/// (period)`) would turn into `NaN` state instead of looping forever —
/// caught here before either happens, matching Canvas2D's own
/// `setLineDash` spec ("if any value in the list is negative, infinite,
/// or NaN, then the method must return without updating the dash
/// list" — this function's fallback is the rendering-side mirror of
/// that same "ignore the invalid list" outcome).
fn is_valid_pattern(pattern: &[f32]) -> bool {
    !pattern.is_empty()
        && pattern.iter().all(|v| v.is_finite() && *v >= 0.0)
        && pattern.iter().any(|v| *v > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point;
    use kurbo::PathEl;

    fn straight_line(len: f64) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(len, 0.0));
        p
    }

    fn move_to_count(path: &BezPath) -> usize {
        path.elements().iter().filter(|e| matches!(e, PathEl::MoveTo(_))).count()
    }

    #[test]
    fn dash_split_a_straight_line_produces_multiple_disjoint_runs() {
        let path = straight_line(100.0);
        let dash = Dash { pattern: vec![10.0, 10.0], phase: 0.0 };
        let out = dash_path(&path, &dash);
        let moves = move_to_count(&out);
        assert!(moves >= 4, "expected several disjoint dash runs, got {moves} MoveTo ops: {out:?}");
    }

    #[test]
    fn dash_split_respects_phase_offset() {
        let path = straight_line(100.0);
        let no_phase = dash_path(&path, &Dash { pattern: vec![10.0, 10.0], phase: 0.0 });
        let with_phase = dash_path(&path, &Dash { pattern: vec![10.0, 10.0], phase: 5.0 });
        assert_ne!(no_phase.elements(), with_phase.elements(), "a phase offset must shift where the runs start");
    }

    #[test]
    fn dash_split_empty_pattern_is_a_noop() {
        let path = straight_line(50.0);
        let out = dash_path(&path, &Dash { pattern: vec![], phase: 0.0 });
        assert_eq!(out.elements(), path.elements());
    }

    #[test]
    fn dash_split_all_zero_pattern_does_not_hang_or_panic() {
        let path = straight_line(50.0);
        let out = dash_path(&path, &Dash { pattern: vec![0.0, 0.0], phase: 0.0 });
        assert_eq!(out.elements(), path.elements());
    }

    #[test]
    fn dash_split_negative_entry_degrades_to_solid() {
        let path = straight_line(50.0);
        let out = dash_path(&path, &Dash { pattern: vec![10.0, -5.0], phase: 0.0 });
        assert_eq!(out.elements(), path.elements());
    }

    #[test]
    fn dash_split_nan_entry_degrades_to_solid() {
        let path = straight_line(50.0);
        let out = dash_path(&path, &Dash { pattern: vec![10.0, f32::NAN], phase: 0.0 });
        assert_eq!(out.elements(), path.elements());
    }

    #[test]
    fn dash_split_odd_length_pattern_still_produces_multiple_runs() {
        let path = straight_line(90.0);
        let dash = Dash { pattern: vec![10.0, 5.0, 5.0], phase: 0.0 };
        let out = dash_path(&path, &dash);
        let moves = move_to_count(&out);
        assert!(moves >= 2, "odd-length pattern must still dash (doubled internally by kurbo::dash): {out:?}");
    }

    #[test]
    fn dash_split_a_multi_subpath_path_dashes_each_subpath_independently() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(40.0, 0.0));
        path.move_to(Point::new(0.0, 20.0));
        path.line_to(Point::new(40.0, 20.0));
        let dash = Dash { pattern: vec![10.0, 10.0], phase: 0.0 };
        let out = dash_path(&path, &dash);
        // Two source subpaths of 40 units each, dash period 20 -> 2 "on"
        // runs per subpath -> at least 4 MoveTo total.
        assert!(move_to_count(&out) >= 4, "{out:?}");
    }

    #[test]
    fn dash_split_a_curve_dashes_by_real_arc_length() {
        let mut path = BezPath::new();
        path.move_to(Point::new(0.0, 0.0));
        path.curve_to(Point::new(0.0, 50.0), Point::new(50.0, 50.0), Point::new(50.0, 0.0));
        let dash = Dash { pattern: vec![10.0, 10.0], phase: 0.0 };
        let out = dash_path(&path, &dash);
        assert!(move_to_count(&out) >= 2, "a curve with a dash pattern shorter than its own arc length must still split: {out:?}");
    }
}
