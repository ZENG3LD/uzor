//! Rounded-rect clip implementation — cached A8 mask via SDF.
//!
//! Per research-round2/04-rounded-clip.md: cached A8 mask keyed on
//! `(quantised_w, quantised_h, quantised_radii[4])`, generated via
//! per-pixel SDF (`sdRoundedBox` from Inigo Quilez). Pixel parity
//! with GPU shader via identical `smoothstep(-0.5, 0.5, d)` math.

use std::collections::HashMap;
use std::sync::Mutex;

use uzor_urx_core::math::{Affine, Rect, RoundedRect};

/// Cache key — quantise to 0.5px so visually-identical rounded rects
/// at slightly different sub-pixel positions hit the same cache slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MaskKey {
    width_2x:  u32,
    height_2x: u32,
    r_2x:      [u32; 4], // tl, tr, br, bl, all × 2 quantised
}

/// One cached A8 alpha mask. Same layout as Pixmap but 1 byte/pixel.
#[derive(Debug, Clone)]
pub(crate) struct AlphaMask {
    pub width:  u32,
    pub height: u32,
    pub alpha:  Vec<u8>,
}

impl AlphaMask {
    #[inline]
    pub fn sample(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height { return 0; }
        let i = (y as usize) * (self.width as usize) + (x as usize);
        self.alpha[i]
    }
}

/// Process-global mask cache. Tiny by default; rounded rects with the
/// same radii+dims are extremely common in UI (every button).
static MASK_CACHE: Mutex<Option<HashMap<MaskKey, AlphaMask>>> = Mutex::new(None);

fn quantise(v: f64) -> u32 {
    (v * 2.0).round().max(0.0) as u32
}
fn quantise_f32(v: f32) -> u32 {
    (v * 2.0).round().max(0.0) as u32
}

/// Get-or-build an A8 mask for a rounded rect of given dimensions.
/// `radii` order matches kurbo: top-left, top-right, bottom-right,
/// bottom-left. Width/height in pixel units.
pub(crate) fn get_or_build_mask(
    width:  f64,
    height: f64,
    radii:  [f64; 4],
) -> AlphaMask {
    let key = MaskKey {
        width_2x:  quantise(width),
        height_2x: quantise(height),
        r_2x: [
            quantise(radii[0]), quantise(radii[1]),
            quantise(radii[2]), quantise(radii[3]),
        ],
    };
    {
        let guard = MASK_CACHE.lock().unwrap();
        if let Some(cache) = guard.as_ref() {
            if let Some(m) = cache.get(&key) {
                return m.clone();
            }
        }
    }
    let mask = build_mask_sdf(width, height, radii);
    let mut guard = MASK_CACHE.lock().unwrap();
    let cache = guard.get_or_insert_with(HashMap::new);
    cache.insert(key, mask.clone());
    mask
}

/// Per-pixel SDF mask generator. The Inigo Quilez 4-corner rounded box
/// SDF: pick corner radius by quadrant, eval distance, smoothstep
/// over 1 pixel for AA coverage.
fn build_mask_sdf(width: f64, height: f64, radii: [f64; 4]) -> AlphaMask {
    let w_u = width.ceil().max(1.0) as u32;
    let h_u = height.ceil().max(1.0) as u32;
    let mut alpha = vec![0u8; (w_u as usize) * (h_u as usize)];
    let hw = width as f32 * 0.5;
    let hh = height as f32 * 0.5;
    // Clamp radii to fit the rect.
    let max_r = hw.min(hh).max(0.0);
    let r = [
        (radii[0] as f32).clamp(0.0, max_r),
        (radii[1] as f32).clamp(0.0, max_r),
        (radii[2] as f32).clamp(0.0, max_r),
        (radii[3] as f32).clamp(0.0, max_r),
    ];
    for py in 0..h_u {
        let cy = py as f32 + 0.5 - hh;
        for px in 0..w_u {
            let cx = px as f32 + 0.5 - hw;
            // Pick corner radius based on quadrant:
            //  +x +y → bottom-right (radii[2])
            //  +x -y → top-right    (radii[1])
            //  -x +y → bottom-left  (radii[3])
            //  -x -y → top-left     (radii[0])
            let radius = if cx >= 0.0 {
                if cy >= 0.0 { r[2] } else { r[1] }
            } else {
                if cy >= 0.0 { r[3] } else { r[0] }
            };
            // sdRoundedBox formula.
            let qx = cx.abs() - hw + radius;
            let qy = cy.abs() - hh + radius;
            let outside = ((qx.max(0.0)).powi(2) + (qy.max(0.0)).powi(2)).sqrt();
            let inside  = qx.max(qy).min(0.0);
            let d = outside + inside - radius;
            // smoothstep over 1 pixel (AA edge band).
            // coverage = 1 - smoothstep(-0.5, 0.5, d)
            let cov = if d <= -0.5 { 1.0 }
                      else if d >= 0.5 { 0.0 }
                      else {
                          let t = (d + 0.5).clamp(0.0, 1.0);
                          let s = t * t * (3.0 - 2.0 * t);
                          1.0 - s
                      };
            alpha[(py as usize) * (w_u as usize) + (px as usize)] = (cov * 255.0).round() as u8;
        }
    }
    AlphaMask { width: w_u, height: h_u, alpha }
}

/// Helper for engine ClipStack — produce a screen-space (mask, origin)
/// pair for a RoundedRect clip given its transform.
pub(crate) fn rounded_clip_to_mask(
    rect:      RoundedRect,
    transform: &Affine,
) -> (AlphaMask, (i64, i64), Rect) {
    let r = rect.rect();
    let radii = rect.radii();
    let coeffs = transform.as_coeffs();
    // Phase 1: translation-only transforms.
    let (sx, sy) = (coeffs[0], coeffs[3]);
    let (tx, ty) = (coeffs[4], coeffs[5]);
    let w = (r.width() * sx.abs()).max(1.0);
    let h = (r.height() * sy.abs()).max(1.0);
    let x0 = r.x0 * sx + tx;
    let y0 = r.y0 * sy + ty;
    let screen_rect = Rect::new(x0, y0, x0 + w, y0 + h);
    let mask = get_or_build_mask(w, h, [
        radii.top_left,
        radii.top_right,
        radii.bottom_right,
        radii.bottom_left,
    ]);
    (mask, (x0.round() as i64, y0.round() as i64), screen_rect)
}

/// Compose a source pixel through a clip-mask coverage. Multiplies
/// the source's premultiplied alpha by the mask coverage.
/// (Currently unused — ClipStack::pixel_coverage handles composition.
/// Kept as a public helper for future direct-blend use cases.)
#[allow(dead_code)]
#[inline]
pub(crate) fn apply_mask(src: [u8; 4], mask_a: u8) -> [u8; 4] {
    if mask_a == 255 { return src; }
    if mask_a == 0 { return [0; 4]; }
    let m = mask_a as u32;
    [
        ((src[0] as u32 * m + 127) / 255) as u8,
        ((src[1] as u32 * m + 127) / 255) as u8,
        ((src[2] as u32 * m + 127) / 255) as u8,
        ((src[3] as u32 * m + 127) / 255) as u8,
    ]
}

/// Test-only helper: flush the mask cache.
#[doc(hidden)]
pub fn _clear_cache_for_tests() {
    let mut g = MASK_CACHE.lock().unwrap();
    if let Some(c) = g.as_mut() { c.clear(); }
}

// Silence unused-import warnings.
#[allow(dead_code)]
fn _u() { let _ = quantise_f32(0.0); }
