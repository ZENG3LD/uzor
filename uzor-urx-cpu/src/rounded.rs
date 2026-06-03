//! Rounded-rect clip implementation — cached A8 mask via SDF.
//!
//! Per research-round2/04-rounded-clip.md: cached A8 mask keyed on
//! `(quantised_w, quantised_h, quantised_radii[4])`, generated via
//! per-pixel SDF (`sdRoundedBox` from Inigo Quilez). Pixel parity
//! with GPU shader via identical `smoothstep(-0.5, 0.5, d)` math.

use std::sync::{Arc, RwLock};

use uzor_urx_core::math::{Affine, Rect, RoundedRect};

const MAX_MASK_DIM: u32 = 4096;
const MASK_CACHE_CAP: usize = 256;

/// Cache key — quantise to 0.5px so visually-identical rounded rects
/// at slightly different sub-pixel positions hit the same cache slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MaskKey {
    width_2x:  u32,
    height_2x: u32,
    r_2x:      [u32; 4], // tl, tr, br, bl, all × 2 quantised
}

/// One cached A8 alpha mask. Same layout as Pixmap but 1 byte/pixel.
#[derive(Debug)]
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

pub(crate) type AlphaMaskArc = Arc<AlphaMask>;

#[derive(Default)]
struct MaskCache {
    entries: Vec<(MaskKey, AlphaMaskArc, u64)>, // key, mask, last_used_tick
    tick:    u64,
}

impl MaskCache {
    fn get(&mut self, key: MaskKey) -> Option<AlphaMaskArc> {
        self.tick = self.tick.wrapping_add(1);
        for entry in self.entries.iter_mut() {
            if entry.0 == key {
                entry.2 = self.tick;
                return Some(entry.1.clone());
            }
        }
        None
    }

    fn insert(&mut self, key: MaskKey, mask: AlphaMaskArc) {
        self.tick = self.tick.wrapping_add(1);
        if self.entries.len() >= MASK_CACHE_CAP {
            if let Some((idx, _)) = self.entries
                .iter().enumerate()
                .min_by_key(|(_, e)| e.2)
            {
                self.entries.swap_remove(idx);
            }
        }
        self.entries.push((key, mask, self.tick));
    }
}

static MASK_CACHE: RwLock<Option<MaskCache>> = RwLock::new(None);

fn quantise(v: f64) -> u32 {
    if !v.is_finite() { return 0; }
    (v * 2.0).round().max(0.0).min(u32::MAX as f64) as u32
}

/// Get-or-build an A8 mask for a rounded rect of given dimensions.
/// `radii` order matches kurbo: top-left, top-right, bottom-right,
/// bottom-left. Width/height in pixel units. Caller-supplied dims
/// are clamped to `MAX_MASK_DIM` to avoid runaway allocations.
pub(crate) fn get_or_build_mask(
    width:  f64,
    height: f64,
    radii:  [f64; 4],
) -> AlphaMaskArc {
    let key = MaskKey {
        width_2x:  quantise(width),
        height_2x: quantise(height),
        r_2x: [
            quantise(radii[0]), quantise(radii[1]),
            quantise(radii[2]), quantise(radii[3]),
        ],
    };
    {
        let mut guard = MASK_CACHE.write().unwrap();
        let cache = guard.get_or_insert_with(MaskCache::default);
        if let Some(m) = cache.get(key) {
            return m;
        }
        let mask = Arc::new(build_mask_sdf(width, height, radii));
        cache.insert(key, mask.clone());
        mask
    }
}

/// Per-pixel SDF mask generator. The Inigo Quilez 4-corner rounded box
/// SDF: pick corner radius by quadrant, eval distance, smoothstep
/// over 1 pixel for AA coverage.
fn build_mask_sdf(width: f64, height: f64, radii: [f64; 4]) -> AlphaMask {
    let w_u = width.ceil().max(1.0).min(MAX_MASK_DIM as f64) as u32;
    let h_u = height.ceil().max(1.0).min(MAX_MASK_DIM as f64) as u32;
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

/// Helper for engine ClipStack — produce a screen-space `(mask, origin,
/// screen_rect)` triple for a RoundedRect clip given its transform.
///
/// **Scope (Phase 1)**: translation+scale only. Shear/rotation collapse
/// to AABB silently and a `KEY_RENDER_PRIMITIVES{kind=rounded_clip_rotated_degraded}`
/// counter is bumped so dashboards see the lossy path. Proper rotated
/// rrect clip needs a tessellated tri-strip alpha mask — deferred to
/// Phase 9c+ when a consumer needs it.
pub(crate) fn rounded_clip_to_mask(
    rect:      RoundedRect,
    transform: &Affine,
) -> (AlphaMaskArc, (i64, i64), Rect) {
    let r = rect.rect();
    let radii = rect.radii();
    let coeffs = transform.as_coeffs();
    let (sx, sy) = (coeffs[0], coeffs[3]);
    let (shear_a, shear_b) = (coeffs[1], coeffs[2]);
    let (tx, ty) = (coeffs[4], coeffs[5]);
    if shear_a.abs() > 1e-6 || shear_b.abs() > 1e-6 {
        metrics::counter!(
            uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
            "kind" => "rounded_clip_rotated_degraded"
        ).increment(1);
    }
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

/// Test-only helper: flush the mask cache.
#[doc(hidden)]
pub fn _clear_cache_for_tests() {
    let mut g = MASK_CACHE.write().unwrap();
    if let Some(c) = g.as_mut() { c.entries.clear(); c.tick = 0; }
}
