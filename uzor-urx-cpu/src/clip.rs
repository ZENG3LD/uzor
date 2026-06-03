//! Clip stack — axis-aligned rect + rounded-rect (via A8 mask).
//!
//! Each entry in the stack contributes coverage at every pixel.
//! Rect entries: 0 or 255 (binary inside/outside, scissor-style).
//! Mask entries: sample the cached A8 mask at the pixel offset for
//! sub-pixel AA on rounded corners.

use uzor_urx_core::math::{Affine, Rect, RoundedRect};

use crate::rounded::{rounded_clip_to_mask, AlphaMask};

#[derive(Debug, Clone)]
pub(crate) enum ClipEntry {
    Rect(Rect),
    Mask {
        /// Bounds (screen space) — used for pixel-loop early-out.
        bounds: Rect,
        /// Top-left of the mask in screen coords.
        origin: (i64, i64),
        mask:   AlphaMask,
    },
}

impl ClipEntry {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect(r) => *r,
            Self::Mask { bounds, .. } => *bounds,
        }
    }

    /// Sample coverage at a pixel center. Returns 0..=255.
    #[inline]
    pub fn coverage(&self, px: i64, py: i64) -> u8 {
        match self {
            Self::Rect(r) => {
                if (px as f64) < r.x0 || (px as f64) >= r.x1
                    || (py as f64) < r.y0 || (py as f64) >= r.y1 { 0 } else { 255 }
            }
            Self::Mask { origin, mask, bounds } => {
                if (px as f64) < bounds.x0 || (px as f64) >= bounds.x1
                    || (py as f64) < bounds.y0 || (py as f64) >= bounds.y1 { return 0; }
                let mx = (px - origin.0) as i64;
                let my = (py - origin.1) as i64;
                if mx < 0 || my < 0 { return 0; }
                mask.sample(mx as u32, my as u32)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClipStack {
    /// Always non-empty. Bottom = pixmap bounds (Rect).
    entries: Vec<ClipEntry>,
}

impl ClipStack {
    pub fn new(bounds: Rect) -> Self {
        Self { entries: vec![ClipEntry::Rect(bounds)] }
    }

    /// Current active clip bounding rect (= intersection of all entry
    /// bounding boxes). Used by primitives for early-out.
    pub fn current(&self) -> Rect {
        let mut r = self.entries[0].bounds();
        for e in &self.entries[1..] {
            r = r.intersect(e.bounds());
        }
        r
    }

    pub fn push_rect(&mut self, r: Rect, transform: &Affine) {
        let r_screen = transform_axis_aligned(*transform, r);
        let isect = self.current().intersect(r_screen);
        let normalized = if isect.width() <= 0.0 || isect.height() <= 0.0 {
            Rect::new(0.0, 0.0, 0.0, 0.0)
        } else {
            isect
        };
        self.entries.push(ClipEntry::Rect(normalized));
    }

    pub fn push_rounded_rect(&mut self, rrect: RoundedRect, transform: &Affine) {
        let (mask, origin, screen_rect) = rounded_clip_to_mask(rrect, transform);
        // Intersect bounds with current clip rect so early-out works.
        let cur = self.current();
        let visible = screen_rect.intersect(cur);
        if visible.width() <= 0.0 || visible.height() <= 0.0 {
            self.entries.push(ClipEntry::Rect(Rect::new(0.0, 0.0, 0.0, 0.0)));
            return;
        }
        self.entries.push(ClipEntry::Mask {
            bounds: visible,
            origin,
            mask,
        });
    }

    pub fn pop(&mut self) {
        if self.entries.len() > 1 {
            self.entries.pop();
        }
    }

    /// Sample combined coverage at a pixel (product of all clip
    /// coverages). 0 = fully clipped out, 255 = fully passes.
    /// Used by primitives that want to honor rounded clips per-pixel.
    #[inline]
    pub fn pixel_coverage(&self, px: i64, py: i64) -> u8 {
        let mut cov: u32 = 255;
        for e in &self.entries {
            cov = (cov * e.coverage(px, py) as u32 + 127) / 255;
            if cov == 0 { return 0; }
        }
        cov as u8
    }

    /// Fast check: are ALL clip entries plain rects (no mask)?
    /// Lets primitives skip per-pixel `pixel_coverage()` and stay on
    /// the analytic-AA fast path when no rounded clip is active.
    #[inline]
    pub fn all_rect(&self) -> bool {
        self.entries.iter().all(|e| matches!(e, ClipEntry::Rect(_)))
    }
}

/// Apply transform to a rect and snap to axis-aligned bounding box.
pub(crate) fn transform_axis_aligned(t: Affine, r: Rect) -> Rect {
    let c = t.as_coeffs();
    let (sx, sy, tx, ty) = (c[0], c[3], c[4], c[5]);
    let x0 = r.x0 * sx + tx;
    let y0 = r.y0 * sy + ty;
    let x1 = r.x1 * sx + tx;
    let y1 = r.y1 * sy + ty;
    Rect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}
