//! Gradient rasterisation — linear / radial scanline math + stop LUT cache.
//!
//! Pipeline:
//! 1. Build a 256-entry RGBA8 lookup table from `peniko::ColorStops`
//!    by interpolating in **linear-premul** space (matches GPU hw
//!    blending and our WGSL shader output for cross-backend parity).
//! 2. Per scanline, compute per-pixel `t ∈ [0,1]` via gradient math
//!    (linear: 1 mul+add per pixel, no sqrt; radial: 1 sqrt per pixel
//!    with incremental r²).
//! 3. Apply spread (Pad/Repeat/Reflect) to fold `t` into [0,1].
//! 4. Sample LUT at `t * 255`, blend src-over premul into pixmap.
//!
//! Phase 1 vocab: Linear + Radial (concentric). Sweep + focal radial
//! deferred to consumer demand.

use std::collections::HashMap;
use std::sync::Mutex;

use uzor_urx_core::math::{
    Brush, Color, ColorStop, Extend, Gradient, GradientKind, Rect,
};

use crate::clip::ClipStack;
use crate::pixmap::Pixmap;

const LUT_SIZE: usize = 256;

/// Pre-built lookup table — `LUT_SIZE` entries of premultiplied RGBA8.
type GradientLut = [[u8; 4]; LUT_SIZE];

/// Process-global LUT cache. Key: hash of stops + spread mode. Built
/// once per (gradient, dpr) tuple; reused across frames. LRU eviction
/// would land here when the cache grows; current impl is unbounded
/// since gradient count per app is typically low (< 50).
static LUT_CACHE: Mutex<Option<HashMap<u64, GradientLut>>> = Mutex::new(None);

fn hash_stops(stops: &[ColorStop], extend: Extend) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    for s in stops {
        // ColorStop is Color + offset; hash all bytes.
        let c = s.color;
        s.offset.to_bits().hash(&mut h);
        c.r.hash(&mut h);
        c.g.hash(&mut h);
        c.b.hash(&mut h);
        c.a.hash(&mut h);
    }
    (extend as u8).hash(&mut h);
    h.finish()
}

/// Get or build the LUT for a gradient.
fn get_lut(stops: &[ColorStop], extend: Extend) -> GradientLut {
    let key = hash_stops(stops, extend);
    let mut cache_guard = LUT_CACHE.lock().unwrap();
    let cache = cache_guard.get_or_insert_with(HashMap::new);
    if let Some(lut) = cache.get(&key) {
        return *lut;
    }
    let built = build_lut(stops);
    cache.insert(key, built);
    built
}

/// Build a 256-entry RGBA8 LUT by linear-premul interpolation
/// between consecutive color stops. Implementation note: we
/// interpolate in **sRGB-premul** for now (matches HTML Canvas2D /
/// our existing WGPU blend). Linear-space interpolation gives more
/// physically-correct mid-tones but breaks parity with the WGPU
/// adapter which doesn't yet do sRGB→linear conversion before
/// `mix()` either. Doing both together is a later quality pass.
fn build_lut(stops: &[ColorStop]) -> GradientLut {
    let mut lut: GradientLut = [[0; 4]; LUT_SIZE];
    if stops.is_empty() {
        return lut;
    }
    if stops.len() == 1 {
        let c = premul(stops[0].color);
        for slot in lut.iter_mut() { *slot = c; }
        return lut;
    }
    for i in 0..LUT_SIZE {
        let t = (i as f32) / (LUT_SIZE as f32 - 1.0);
        lut[i] = sample_stops(stops, t);
    }
    lut
}

fn premul(c: Color) -> [u8; 4] {
    let a = c.a as u32;
    [
        ((c.r as u32 * a + 127) / 255) as u8,
        ((c.g as u32 * a + 127) / 255) as u8,
        ((c.b as u32 * a + 127) / 255) as u8,
        c.a,
    ]
}

/// Sample a stop sequence at parameter `t ∈ [0, 1]` and return
/// premultiplied RGBA8 via lerp between bracketing stops.
fn sample_stops(stops: &[ColorStop], t: f32) -> [u8; 4] {
    // Stops are normally sorted by `offset`; bracket-search.
    if t <= stops[0].offset { return premul(stops[0].color); }
    if t >= stops[stops.len() - 1].offset { return premul(stops[stops.len() - 1].color); }
    for w in stops.windows(2) {
        let s0 = &w[0];
        let s1 = &w[1];
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            let local = if span < 1e-9 { 0.0 } else { (t - s0.offset) / span };
            let c0 = premul(s0.color);
            let c1 = premul(s1.color);
            return [
                lerp_u8(c0[0], c1[0], local),
                lerp_u8(c0[1], c1[1], local),
                lerp_u8(c0[2], c1[2], local),
                lerp_u8(c0[3], c1[3], local),
            ];
        }
    }
    premul(stops[stops.len() - 1].color)
}

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let r = (a as f32) * (1.0 - t) + (b as f32) * t;
    r.round().clamp(0.0, 255.0) as u8
}

/// Fold a raw `t` into [0, 1] per the spread/extend mode.
#[inline]
fn apply_spread(t: f32, mode: Extend) -> f32 {
    match mode {
        Extend::Pad => t.clamp(0.0, 1.0),
        Extend::Repeat => {
            let f = t - t.floor();
            if f < 0.0 { f + 1.0 } else { f }
        }
        Extend::Reflect => {
            let m = (t.rem_euclid(2.0) - 1.0).abs();
            // Convert [-1,0,1] reflection to [0,1].
            1.0 - m
        }
    }
}

#[inline]
fn lut_sample(lut: &GradientLut, t: f32) -> [u8; 4] {
    let idx = (t * (LUT_SIZE - 1) as f32).round().clamp(0.0, (LUT_SIZE - 1) as f32) as usize;
    lut[idx]
}

/// Fill a rect with a gradient brush. Walker entry — dispatches to
/// linear/radial scanline routines. Returns true if the brush was
/// recognised and rendered.
pub(crate) fn fill_rect_gradient_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    rect:   Rect,
    gradient: &Gradient,
    transform: &uzor_urx_core::math::Affine,
) -> bool {
    let r_screen = crate::clip::transform_axis_aligned(*transform, rect);
    let cur_clip = clip.current();
    let visible = r_screen.intersect(cur_clip);
    if visible.width() <= 0.0 || visible.height() <= 0.0 { return true; }

    let w = pixmap.width()  as i64;
    let h = pixmap.height() as i64;
    let ix0 = (visible.x0.floor() as i64).max(0);
    let iy0 = (visible.y0.floor() as i64).max(0);
    let ix1 = (visible.x1.ceil()  as i64).min(w);
    let iy1 = (visible.y1.ceil()  as i64).min(h);
    if ix0 >= ix1 || iy0 >= iy1 { return true; }

    let lut = get_lut(&gradient.stops, gradient.extend);

    match gradient.kind {
        GradientKind::Linear { start, end } => {
            // dot(d, d) — squared gradient axis length.
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 < 1e-9 {
                // Degenerate — fill with stop[0].
                let c = premul(gradient.stops.first().map(|s| s.color).unwrap_or(Color::rgba8(0, 0, 0, 0)));
                fill_solid(pixmap, ix0, iy0, ix1, iy1, &visible, c);
                return true;
            }
            let inv_d2 = 1.0 / d2;
            for py in iy0 .. iy1 {
                let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
                if v_cov == 0 { continue; }
                let cy = py as f32 + 0.5;
                let py_dy = cy - start.y as f32;
                for px in ix0 .. ix1 {
                    let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
                    if h_cov == 0 { continue; }
                    let cx = px as f32 + 0.5;
                    let px_dx = cx - start.x as f32;
                    let t_raw = (px_dx * dx as f32 + py_dy * dy as f32) * inv_d2;
                    let t = apply_spread(t_raw, gradient.extend);
                    let mut sample = lut_sample(&lut, t);
                    let cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
                    if cov < 255 {
                        sample = scale_premul(sample, cov);
                    }
                    pixmap.blend_pixel(px as u32, py as u32, sample);
                }
            }
        }
        GradientKind::Radial { start_center, start_radius: _, end_center, end_radius } => {
            // Concentric variant only (start ≈ end). For focal
            // variants we'd add the two-point conical solver.
            let cx = end_center.x as f32;
            let cy = end_center.y as f32;
            let radius = end_radius.max(1e-3);
            let inv_r = 1.0 / radius;
            let _ = start_center;
            for py in iy0 .. iy1 {
                let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
                if v_cov == 0 { continue; }
                let pcy = py as f32 + 0.5 - cy;
                for px in ix0 .. ix1 {
                    let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
                    if h_cov == 0 { continue; }
                    let pcx = px as f32 + 0.5 - cx;
                    let dist = (pcx * pcx + pcy * pcy).sqrt();
                    let t_raw = dist * inv_r;
                    let t = apply_spread(t_raw, gradient.extend);
                    let mut sample = lut_sample(&lut, t);
                    let cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
                    if cov < 255 {
                        sample = scale_premul(sample, cov);
                    }
                    pixmap.blend_pixel(px as u32, py as u32, sample);
                }
            }
        }
        GradientKind::Sweep { center: _, start_angle: _, end_angle: _ } => {
            // Phase 2 deferred — needs per-pixel atan2.
            return false;
        }
    }
    true
}

#[inline]
fn scale_premul(rgba: [u8; 4], cov: u8) -> [u8; 4] {
    let c = cov as u32;
    [
        ((rgba[0] as u32 * c + 127) / 255) as u8,
        ((rgba[1] as u32 * c + 127) / 255) as u8,
        ((rgba[2] as u32 * c + 127) / 255) as u8,
        ((rgba[3] as u32 * c + 127) / 255) as u8,
    ]
}

fn fill_solid(pixmap: &mut Pixmap, ix0: i64, iy0: i64, ix1: i64, iy1: i64, visible: &Rect, color: [u8; 4]) {
    for py in iy0 .. iy1 {
        let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
        if v_cov == 0 { continue; }
        for px in ix0 .. ix1 {
            let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
            if h_cov == 0 { continue; }
            let cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
            let src = scale_premul(color, cov);
            pixmap.blend_pixel(px as u32, py as u32, src);
        }
    }
}

/// Top-level entry — checks if the Brush is a Gradient and dispatches.
/// Returns `Some(true)` if a gradient was rendered, `Some(false)` if
/// it was a degenerate / unsupported variant (caller falls back to
/// the first stop), `None` if Brush is not a gradient.
pub(crate) fn try_fill_rect_gradient(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    rect:   Rect,
    brush:  &Brush,
    transform: &uzor_urx_core::math::Affine,
) -> Option<bool> {
    match brush {
        Brush::Gradient(g) => Some(fill_rect_gradient_aa(pixmap, clip, rect, g, transform)),
        _ => None,
    }
}
