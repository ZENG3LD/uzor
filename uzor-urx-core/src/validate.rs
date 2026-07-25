//! Scene + primitive validation helpers.
//!
//! URX trusts its consumers in the hot path — every backend pre-filter
//! is a per-primitive cost. But adversarial / buggy upstream code
//! (NaN coordinates from a transform divide-by-zero; +Inf from an
//! unclipped logarithm; oversize values that overflow when cast to
//! integer pixel space) must NOT panic or corrupt the pixmap.
//!
//! This module provides:
//!
//! - [`is_finite_rect`] / [`is_finite_affine`] / [`is_finite_vec2`] /
//!   [`is_finite_point`] / [`is_finite_bezpath`] — `O(1)`/`O(path len)`
//!   checks that callers can use at the backend entry point to silently
//!   skip non-finite primitives and bump a metrics counter.
//! - [`Scene::validate`] (via inherent method on the type) — opt-in
//!   pre-flight that returns a list of bad primitives without
//!   rendering. Useful in tests / fuzz harnesses; never called on the
//!   hot path.
//!
//! Design rule: validation **never panics**. A NaN that slips past
//! these checks is upstream's bug, not ours, but our policy is
//! "silent skip + counter" rather than "crash the frame".

use crate::math::{Affine, BezPath, Point, Rect, RoundedRect, Vec2};
use crate::scene::{Dash, DrawCommand, Scene};

/// True iff every coordinate of the rect is finite (no NaN, no ±Inf).
#[inline]
pub fn is_finite_rect(r: Rect) -> bool {
    r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()
}

/// True iff both coordinates of the point are finite.
#[inline]
pub fn is_finite_point(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// True iff every control/end point of every element in the path is
/// finite. `FillPath`/`StrokePath`'s own path geometry used to be
/// completely UNCHECKED by [`validate_command`] (only their `transform`
/// was) — a NaN/Inf point INSIDE the path itself slipped straight
/// through to `uzor-urx-wgpu`'s lyon tessellator, which hard-`assert!`s
/// every point is finite and panics (`uzor-urx-cpu`'s scanline
/// rasteriser never hit this because it has no such assertion, so the
/// gap was invisible there).
#[inline]
pub fn is_finite_bezpath(path: &BezPath) -> bool {
    use kurbo::PathEl;
    path.elements().iter().all(|el| match *el {
        PathEl::MoveTo(p) | PathEl::LineTo(p) => is_finite_point(p),
        PathEl::QuadTo(c, p) => is_finite_point(c) && is_finite_point(p),
        PathEl::CurveTo(c1, c2, p) => is_finite_point(c1) && is_finite_point(c2) && is_finite_point(p),
        PathEl::ClosePath => true,
    })
}

/// True iff every coefficient of the affine matrix is finite.
#[inline]
pub fn is_finite_affine(a: Affine) -> bool {
    let c = a.as_coeffs();
    c[0].is_finite() && c[1].is_finite() && c[2].is_finite()
        && c[3].is_finite() && c[4].is_finite() && c[5].is_finite()
}

/// True iff both components of the vector are finite.
#[inline]
pub fn is_finite_vec2(v: Vec2) -> bool {
    v.x.is_finite() && v.y.is_finite()
}

/// True iff every coordinate of the rounded rect (incl. radii) is finite.
#[inline]
pub fn is_finite_rounded_rect(r: RoundedRect) -> bool {
    let inner = r.rect();
    if !is_finite_rect(inner) { return false; }
    let radii = r.radii();
    radii.top_left.is_finite()
        && radii.top_right.is_finite()
        && radii.bottom_left.is_finite()
        && radii.bottom_right.is_finite()
}

/// True iff every per-corner radius is finite (or `None`).
#[inline]
pub fn is_finite_radii_opt(r: &Option<[f32; 4]>) -> bool {
    match r {
        None => true,
        Some(arr) => arr.iter().all(|v| v.is_finite()),
    }
}

/// True iff `Stroke.dash` is either absent, or every `pattern` entry
/// plus `phase` is finite. Does NOT reject a negative/all-zero
/// `pattern` here — `uzor_urx_core::dash::dash_path`'s own
/// `is_valid_pattern` already degrades those to "no dashing" safely
/// (never panics), so this check stays scoped to the same NaN/±Inf
/// class every other `is_finite_*` helper in this module guards
/// against, not a duplicate of that separate, already-safe fallback.
#[inline]
pub fn is_finite_dash_opt(d: &Option<Dash>) -> bool {
    match d {
        None => true,
        Some(dash) => dash.phase.is_finite() && dash.pattern.iter().all(|v| v.is_finite()),
    }
}

/// Verdict for one primitive inspected by [`validate_command`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationIssue {
    /// One or more coordinates were NaN or ±Inf.
    NonFinite,
    /// Geometry is degenerate (zero area). Not an error per se — the
    /// backend would skip it anyway — but useful to surface in fuzz
    /// reports.
    DegenerateGeometry,
}

/// Inspect a single command. `Ok(())` = safe to render. `Err(issue)`
/// = backend should skip this command (and bump a counter).
pub fn validate_command(cmd: &DrawCommand) -> Result<(), ValidationIssue> {
    match cmd {
        DrawCommand::FillRect { rect, radii, brush: _, transform } => {
            if !is_finite_rect(*rect) || !is_finite_affine(*transform)
                || !is_finite_radii_opt(radii)
            {
                return Err(ValidationIssue::NonFinite);
            }
            if rect.x0 >= rect.x1 || rect.y0 >= rect.y1 {
                return Err(ValidationIssue::DegenerateGeometry);
            }
            Ok(())
        }
        DrawCommand::StrokeRect { rect, radii, stroke, brush: _, transform } => {
            if !is_finite_rect(*rect) || !is_finite_affine(*transform)
                || !is_finite_radii_opt(radii)
                || !stroke.width.is_finite()
                || !stroke.miter_limit.is_finite()
                || !is_finite_dash_opt(&stroke.dash)
            {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::Line { from, to, stroke, brush: _, transform } => {
            if !is_finite_vec2(*from) || !is_finite_vec2(*to)
                || !is_finite_affine(*transform)
                || !stroke.width.is_finite()
                || !stroke.miter_limit.is_finite()
                || !is_finite_dash_opt(&stroke.dash)
            {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::FillPath { path, rule: _, brush: _, transform } => {
            if !is_finite_affine(*transform) || !is_finite_bezpath(path) {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::StrokePath { path, stroke, brush: _, transform } => {
            if !is_finite_affine(*transform) || !is_finite_bezpath(path)
                || !stroke.width.is_finite() || !stroke.miter_limit.is_finite()
                || !is_finite_dash_opt(&stroke.dash)
            {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::GlyphRun { glyphs, font: _, font_size, brush: _, transform, text: _ } => {
            if !font_size.is_finite() || !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            for g in glyphs {
                if !g.x.is_finite() || !g.y.is_finite() {
                    return Err(ValidationIssue::NonFinite);
                }
            }
            Ok(())
        }
        DrawCommand::Image { src: _, src_rect, dest, transform } => {
            if !is_finite_rect(*dest) || !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            if let Some(sr) = src_rect {
                if !is_finite_rect(*sr) {
                    return Err(ValidationIssue::NonFinite);
                }
            }
            Ok(())
        }
        DrawCommand::PushClipRect { rect, transform } => {
            if !is_finite_rect(*rect) || !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::PushClipRoundedRect { rect, transform } => {
            if !is_finite_rounded_rect(*rect) || !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::PopClip => Ok(()),
        DrawCommand::PushBlendLayer { mode: _, alpha, transform } => {
            if !alpha.is_finite() || !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::PopBlendLayer => Ok(()),
    }
}

impl Scene {
    /// Inspect every primitive; return the indices + issues for any
    /// that would otherwise be silently skipped by the backend.
    ///
    /// Non-allocating zero-issue case (returns empty Vec).
    /// Strictly opt-in: not called from the render hot path.
    pub fn validate(&self) -> Vec<(usize, ValidationIssue)> {
        let mut out = Vec::new();
        for (i, c) in self.commands.iter().enumerate() {
            if let Err(issue) = validate_command(c) {
                out.push((i, issue));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Brush, Color};

    #[test]
    fn finite_rect_accepts_normal() {
        assert!(is_finite_rect(Rect::new(0.0, 0.0, 10.0, 10.0)));
    }

    #[test]
    fn finite_rect_rejects_nan() {
        assert!(!is_finite_rect(Rect::new(f64::NAN, 0.0, 10.0, 10.0)));
        assert!(!is_finite_rect(Rect::new(0.0, f64::INFINITY, 10.0, 10.0)));
        assert!(!is_finite_rect(Rect::new(0.0, 0.0, f64::NEG_INFINITY, 10.0)));
    }

    #[test]
    fn validate_command_flags_nan_rect() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(f64::NAN, 0.0, 1.0, 1.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_flags_degenerate() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::DegenerateGeometry));
    }

    #[test]
    fn validate_command_flags_nan_radii() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radii: Some([1.0, f32::NAN, 1.0, 1.0]),
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_accepts_normal() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert!(validate_command(&cmd).is_ok());
    }

    // ── FillPath/StrokePath path-point validation (bug fix: these used
    // to only check `transform`, letting a NaN/Inf point INSIDE the
    // path itself slip through to uzor-urx-wgpu's lyon tessellator,
    // which hard-panics — see `is_finite_bezpath`'s own doc comment) ──

    #[test]
    fn finite_bezpath_accepts_normal_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.curve_to(Point::new(10.0, 5.0), Point::new(5.0, 10.0), Point::new(0.0, 10.0));
        p.close_path();
        assert!(is_finite_bezpath(&p));
    }

    #[test]
    fn finite_bezpath_rejects_nan_line_to() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(f64::NAN, 10.0));
        assert!(!is_finite_bezpath(&p));
    }

    #[test]
    fn finite_bezpath_rejects_inf_in_a_curve_control_point() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.curve_to(Point::new(f64::INFINITY, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0));
        assert!(!is_finite_bezpath(&p));
    }

    #[test]
    fn finite_bezpath_rejects_nan_in_a_quad_end_point() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.quad_to(Point::new(5.0, 5.0), Point::new(f64::NAN, 10.0));
        assert!(!is_finite_bezpath(&p));
    }

    fn nan_fill_path() -> BezPath {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(f64::NAN, 10.0));
        p.line_to(Point::new(10.0, 10.0));
        p.close_path();
        p
    }

    #[test]
    fn validate_command_flags_nan_point_inside_a_fill_path() {
        let cmd = DrawCommand::FillPath {
            path: nan_fill_path(),
            rule: crate::scene::FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(
            validate_command(&cmd),
            Err(ValidationIssue::NonFinite),
            "a NaN point INSIDE the path (not just the transform) must be caught"
        );
    }

    #[test]
    fn validate_command_flags_nan_point_inside_a_stroke_path() {
        let cmd = DrawCommand::StrokePath {
            path: nan_fill_path(),
            stroke: crate::scene::Stroke::default(),
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_flags_nonfinite_stroke_width_on_a_stroke_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        let cmd = DrawCommand::StrokePath {
            path: p,
            stroke: crate::scene::Stroke { width: f32::NAN, ..crate::scene::Stroke::default() },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_flags_nan_in_a_dash_pattern_on_a_stroke_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        let cmd = DrawCommand::StrokePath {
            path: p,
            stroke: crate::scene::Stroke {
                dash: Some(crate::scene::Dash { pattern: vec![10.0, f32::NAN], phase: 0.0 }),
                ..crate::scene::Stroke::default()
            },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_accepts_a_finite_dash_pattern() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        let cmd = DrawCommand::StrokePath {
            path: p,
            stroke: crate::scene::Stroke {
                dash: Some(crate::scene::Dash { pattern: vec![5.0, 3.0], phase: 1.0 }),
                ..crate::scene::Stroke::default()
            },
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert!(validate_command(&cmd).is_ok());
    }

    #[test]
    fn validate_command_accepts_a_finite_fill_path() {
        let mut p = BezPath::new();
        p.move_to(Point::new(0.0, 0.0));
        p.line_to(Point::new(10.0, 0.0));
        p.line_to(Point::new(10.0, 10.0));
        p.close_path();
        let cmd = DrawCommand::FillPath {
            path: p,
            rule: crate::scene::FillRule::NonZero,
            brush: Brush::Solid(Color::from_rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert!(validate_command(&cmd).is_ok());
    }

    #[test]
    fn scene_validate_reports_all_issues() {
        let mut s = Scene::new();
        s.fill_rect_solid(Rect::new(0.0, 0.0, 1.0, 1.0), Color::from_rgba8(0, 0, 0, 255));
        s.fill_rect_solid(Rect::new(f64::NAN, 0.0, 1.0, 1.0), Color::from_rgba8(0, 0, 0, 255));
        s.fill_rect_solid(Rect::new(5.0, 5.0, 5.0, 5.0), Color::from_rgba8(0, 0, 0, 255));
        let issues = s.validate();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0], (1, ValidationIssue::NonFinite));
        assert_eq!(issues[1], (2, ValidationIssue::DegenerateGeometry));
    }
}
