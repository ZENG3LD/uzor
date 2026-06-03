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
//! - [`is_finite_rect`] / [`is_finite_affine`] / [`is_finite_vec2`] —
//!   `O(1)` checks that callers can use at the backend entry point to
//!   silently skip non-finite primitives and bump a metrics counter.
//! - [`Scene::validate`] (via inherent method on the type) — opt-in
//!   pre-flight that returns a list of bad primitives without
//!   rendering. Useful in tests / fuzz harnesses; never called on the
//!   hot path.
//!
//! Design rule: validation **never panics**. A NaN that slips past
//! these checks is upstream's bug, not ours, but our policy is
//! "silent skip + counter" rather than "crash the frame".

use crate::math::{Affine, Rect, RoundedRect, Vec2};
use crate::scene::{DrawCommand, Scene};

/// True iff every coordinate of the rect is finite (no NaN, no ±Inf).
#[inline]
pub fn is_finite_rect(r: Rect) -> bool {
    r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite()
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
            {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::FillPath { path: _, rule: _, brush: _, transform }
        | DrawCommand::StrokePath { path: _, stroke: _, brush: _, transform } => {
            if !is_finite_affine(*transform) {
                return Err(ValidationIssue::NonFinite);
            }
            Ok(())
        }
        DrawCommand::GlyphRun { glyphs, font: _, font_size, brush: _, transform } => {
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
            brush: Brush::Solid(Color::rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_flags_degenerate() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(10.0, 10.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::DegenerateGeometry));
    }

    #[test]
    fn validate_command_flags_nan_radii() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radii: Some([1.0, f32::NAN, 1.0, 1.0]),
            brush: Brush::Solid(Color::rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert_eq!(validate_command(&cmd), Err(ValidationIssue::NonFinite));
    }

    #[test]
    fn validate_command_accepts_normal() {
        let cmd = DrawCommand::FillRect {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            radii: None,
            brush: Brush::Solid(Color::rgba8(0, 0, 0, 255)),
            transform: Affine::IDENTITY,
        };
        assert!(validate_command(&cmd).is_ok());
    }

    #[test]
    fn scene_validate_reports_all_issues() {
        let mut s = Scene::new();
        s.fill_rect_solid(Rect::new(0.0, 0.0, 1.0, 1.0), Color::rgba8(0, 0, 0, 255));
        s.fill_rect_solid(Rect::new(f64::NAN, 0.0, 1.0, 1.0), Color::rgba8(0, 0, 0, 255));
        s.fill_rect_solid(Rect::new(5.0, 5.0, 5.0, 5.0), Color::rgba8(0, 0, 0, 255));
        let issues = s.validate();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0], (1, ValidationIssue::NonFinite));
        assert_eq!(issues[1], (2, ValidationIssue::DegenerateGeometry));
    }
}
