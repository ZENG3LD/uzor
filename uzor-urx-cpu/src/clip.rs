//! Clip stack — axis-aligned rectangular clipping only (Phase 3).
//!
//! Each `PushClipRect` pushes a new clip; every subsequent draw is
//! intersected against the TOP-OF-STACK clip before rasterisation.
//! `PopClip` pops. No rounded-rect clip yet (deferred to Phase 3.5
//! when we add stencil-style coverage masking).

use uzor_urx_core::math::{Affine, Rect};

#[derive(Debug, Clone)]
pub(crate) struct ClipStack {
    /// Always non-empty. Bottom = pixmap bounds. Top = current
    /// active clip = intersection of every push so far.
    rects: Vec<Rect>,
}

impl ClipStack {
    pub fn new(bounds: Rect) -> Self { Self { rects: vec![bounds] } }

    /// Current active clip rect (= top of stack).
    pub fn current(&self) -> Rect { *self.rects.last().unwrap() }

    pub fn push_rect(&mut self, r: Rect, transform: &Affine) {
        let r_screen = transform_axis_aligned(*transform, r);
        let cur = self.current();
        let isect = cur.intersect(r_screen);
        // If intersection is empty (negative width/height), push an
        // empty rect. Primitives clipped against it will do nothing.
        let normalized = if isect.width() <= 0.0 || isect.height() <= 0.0 {
            Rect::new(0.0, 0.0, 0.0, 0.0)
        } else {
            isect
        };
        self.rects.push(normalized);
    }

    pub fn pop(&mut self) {
        // Never pop the bottom (pixmap bounds).
        if self.rects.len() > 1 {
            self.rects.pop();
        }
    }
}

/// Apply transform to a rect and snap to axis-aligned bounding box.
/// We only support translation + scale (no rotation) for clip rects
/// in Phase 3 — rotated clips would need full path clipping which is
/// deferred.
pub(crate) fn transform_axis_aligned(t: Affine, r: Rect) -> Rect {
    let c = t.as_coeffs();
    let (sx, sy, tx, ty) = (c[0], c[3], c[4], c[5]);
    let x0 = r.x0 * sx + tx;
    let y0 = r.y0 * sy + ty;
    let x1 = r.x1 * sx + tx;
    let y1 = r.y1 * sy + ty;
    Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}
