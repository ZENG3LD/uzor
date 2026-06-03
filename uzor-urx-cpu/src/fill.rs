//! Filled rect rasterisation with analytic edge coverage AA.
//!
//! Algorithm: for each pixel row that the rect intersects, compute the
//! horizontal coverage (0..=255) of the left + right edges, full
//! coverage in the middle. For each pixel column, vertical coverage
//! of the top + bottom edges. Per-pixel coverage = `h_cov * v_cov / 255`.
//!
//! This is the same approach Skia + tiny_skia + AGG use for axis-aligned
//! rects. Pixel-identical to a supersampled reference at integer
//! sample density.
//!
//! No corner radii in Phase 3 — those need an SDF eval per pixel
//! (cheap, will land in 3.5 when consumers ask for rounded fills).

use crate::clip::{transform_axis_aligned, ClipStack};
use crate::color::{color_to_premul, premul_scale};
use crate::pixmap::Pixmap;
use uzor_urx_core::math::{Affine, Color, Rect};

/// Fill an axis-aligned rect (post-transform) into the pixmap with
/// analytic AA along all four edges. Brush is resolved to a single
/// color (gradient stub'd in Phase 3).
pub(crate) fn fill_rect_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    rect:   Rect,
    color:  Color,
    transform: &Affine,
) {
    let r_screen = transform_axis_aligned(*transform, rect);
    // Intersect against current clip.
    let c = clip.current();
    let visible = r_screen.intersect(c);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return; }

    // Final fragment rect — clipped, with sub-pixel edges preserved.
    let fx0 = visible.x0;
    let fy0 = visible.y0;
    let fx1 = visible.x1;
    let fy1 = visible.y1;

    // Pixel range we touch (inclusive). Pixel `i` covers `[i, i+1)`.
    let ix0 = fx0.floor() as i64;
    let iy0 = fy0.floor() as i64;
    let ix1 = (fx1.ceil() as i64).max(ix0 + 1);
    let iy1 = (fy1.ceil() as i64).max(iy0 + 1);

    let w = pixmap.width()  as i64;
    let h = pixmap.height() as i64;
    let ix0 = ix0.max(0);
    let iy0 = iy0.max(0);
    let ix1 = ix1.min(w);
    let iy1 = iy1.min(h);
    if ix0 >= ix1 || iy0 >= iy1 { return; }

    let premul = color_to_premul(color);
    let use_mask = !clip.all_rect();

    for py in iy0 .. iy1 {
        let v_cov = axis_coverage(py as f64, py as f64 + 1.0, fy0, fy1);
        if v_cov == 0 { continue; }
        if !use_mask {
            // SIMD fast path: 4-pixel-wide fused (cov × premul → blend).
            // Math is bit-identical to the scalar branch below.
            crate::simd::fill_span_aa(
                pixmap, py as u32,
                ix0, ix1, fx0, fx1,
                v_cov, premul,
            );
            continue;
        }
        for px in ix0 .. ix1 {
            let h_cov = axis_coverage(px as f64, px as f64 + 1.0, fx0, fx1);
            if h_cov == 0 { continue; }
            let mut cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
            let mask_cov = clip.pixel_coverage(px, py);
            cov = ((cov as u32 * mask_cov as u32 + 127) / 255) as u8;
            if cov == 0 { continue; }
            let src = premul_scale(premul, cov);
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}

/// Coverage (0..=255) of the integer interval `[i, i+1]` covered by
/// the float interval `[lo, hi]`. Used per-axis; final per-pixel
/// coverage = (h * v + 127) / 255.
#[inline]
pub fn axis_coverage(pi: f64, pi1: f64, lo: f64, hi: f64) -> u8 {
    let a = pi.max(lo);
    let b = pi1.min(hi);
    if b <= a { 0 } else { ((b - a) * 255.0 + 0.5).clamp(0.0, 255.0) as u8 }
}
