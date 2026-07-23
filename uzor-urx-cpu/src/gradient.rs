//! Gradient rasterisation — linear / radial / sweep scanline math + stop LUT cache.
//!
//! Pipeline:
//! 1. Build a 256-entry RGBA8 lookup table from `peniko::ColorStops`
//!    by interpolating in **linear-premul** space (matches GPU hw
//!    blending and our WGSL shader output for cross-backend parity).
//! 2. Per scanline, compute per-pixel `t ∈ [0,1]` via gradient math
//!    (linear: 1 mul+add per pixel, no sqrt; radial: 1 sqrt per pixel
//!    with incremental r²; sweep: 1 atan2 per pixel).
//! 3. Apply spread (Pad/Repeat/Reflect) to fold `t` into [0,1].
//! 4. Sample LUT at `t * 255`, blend src-over premul into pixmap.
//!
//! All three kinds (Linear, Radial concentric, Sweep) are implemented
//! — the module used to say "Sweep + focal radial deferred to consumer
//! demand" here; that was STALE (URX Wave 4 design §0.1c/§3,
//! `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`):
//! Sweep has been a complete, working implementation since before this
//! correction, this doc comment just never caught up. Focal (two-point)
//! radial genuinely IS still an approximation (concentric fallback,
//! `gradient_radial_focal_degraded`, unchanged this wave — see the
//! `Radial` arm below) — that part of the old sentence was accurate
//! and remains true.
//!
//! ## Wave 4 Commit 1 fixes (§0.1)
//!
//! (a) **Radii-aware masking**: `fill_rect_gradient_aa` now folds
//! `clip.pixel_coverage(px, py)` into the per-pixel coverage in all
//! three `GradientKind` arms, mirroring `fill.rs`'s `use_mask`/
//! `pixel_coverage` pattern exactly (`fill.rs:57,76-78`) — previously a
//! `FillRect { radii: Some(_), brush: Gradient(_) }` pushed a real
//! rounded-clip mask (`backend.rs`'s `FillRect` arm,
//! `clip.push_rounded_rect`) that the gradient path never consulted,
//! silently painting square corners through a supposedly-rounded clip.
//!
//! (b) **Gradient-axis transform**: the gradient's own anchor points
//! (`start`/`end`, `end_center`, `center`) are now transformed through
//! the SAME `transform` as `rect` (via [`crate::clip::transform_point_full`],
//! extracted for exactly this reuse), radius scaled by the affine's
//! average axis-scale magnitude, Sweep's angles rotated by the affine's
//! rotation component. Previously the gradient axis was read directly
//! against screen-space pixel coordinates while ONLY `rect` was mapped
//! through the transform — correct by coincidence under
//! `Affine::IDENTITY` (every fixture before this wave), wrong under any
//! real transform, including a plain translation.
//!
//! Both fixes are provably inert on every pre-Wave-4 fixture: no
//! existing scene combines a gradient with `radii: Some(_)` or a
//! non-identity `transform` (Wave 1's own finding, carried into this
//! wave's design doc §0.1) — this file's existing test suite passing
//! UNMODIFIED is exactly that regression proof.
//!
//! ## Wave 4 Commit 1 extraction (§2.2/§10 Commit 1)
//!
//! `build_lut`/`sample_stops`/`premul`/`lerp_u8`/`apply_spread`/
//! `hash_stops`/`stop_rgba8`/the `GradientLut` type alias moved
//! VERBATIM to `uzor_urx_core::gradient_lut` (pure functions, no
//! state) — this module keeps only the CPU-specific process-global
//! `LutCache`/`LUT_CACHE`/`get_lut` (a caching POLICY, not shared math)
//! and calls the extracted functions. Same "share the canonical
//! implementation, don't fork" doctrine as Wave 2's `uzor-urx-glyph`
//! key-math extraction — this file's existing gradient tests passing
//! UNMODIFIED is the regression proof for the extraction too.

use std::sync::{Arc, RwLock};

use uzor_urx_core::gradient_lut::{apply_spread, build_lut, hash_stops, premul, GradientLut};
use uzor_urx_core::math::{
    Affine, Brush, ColorStop, Extend, Gradient, GradientKind, Rect,
};

use crate::clip::{transform_point_full, ClipStack};
use crate::pixmap::Pixmap;

const LUT_CACHE_CAP: usize = 256;

type GradientLutArc = Arc<GradientLut>;

#[derive(Default)]
struct LutCache {
    entries: Vec<(u64, GradientLutArc, u64)>, // key, lut, last_used_tick
    tick:    u64,
}

impl LutCache {
    fn get(&mut self, key: u64) -> Option<GradientLutArc> {
        self.tick = self.tick.wrapping_add(1);
        for entry in self.entries.iter_mut() {
            if entry.0 == key {
                entry.2 = self.tick;
                return Some(entry.1.clone());
            }
        }
        None
    }

    fn insert(&mut self, key: u64, lut: GradientLutArc) {
        self.tick = self.tick.wrapping_add(1);
        if self.entries.len() >= LUT_CACHE_CAP {
            // Evict least-recently-used.
            if let Some((idx, _)) = self.entries
                .iter().enumerate()
                .min_by_key(|(_, e)| e.2)
            {
                self.entries.swap_remove(idx);
            }
        }
        self.entries.push((key, lut, self.tick));
    }
}

static LUT_CACHE: RwLock<Option<LutCache>> = RwLock::new(None);

/// Get or build the LUT for a gradient. Returns `Arc<Lut>` — cheap clone
/// on the hot path.
fn get_lut(stops: &[ColorStop], extend: Extend) -> GradientLutArc {
    let key = hash_stops(stops, extend);
    {
        // Read-fast path: try the read lock first.
        let mut guard = LUT_CACHE.write().unwrap();
        let cache = guard.get_or_insert_with(LutCache::default);
        if let Some(lut) = cache.get(key) {
            return lut;
        }
        let built = Arc::new(build_lut(stops));
        cache.insert(key, built.clone());
        built
    }
}

/// Test helper — flush the gradient LUT cache.
#[doc(hidden)]
pub fn _clear_gradient_cache_for_tests() {
    let mut g = LUT_CACHE.write().unwrap();
    if let Some(c) = g.as_mut() { c.entries.clear(); c.tick = 0; }
}

#[inline]
fn lut_sample(lut: &GradientLut, t: f32) -> [u8; 4] {
    let idx = (t * (uzor_urx_core::gradient_lut::LUT_SIZE - 1) as f32).round().clamp(0.0, (uzor_urx_core::gradient_lut::LUT_SIZE - 1) as f32) as usize;
    lut[idx]
}

/// `(hypot(a,b), hypot(c,d))` — the affine's own per-axis scale
/// magnitude from its linear part, ignoring rotation/shear (Wave 4
/// §0.1b). Same "collapse anisotropic scale to one scalar via
/// `(sx+sy)*0.5`" convention this family already uses for stroke width
/// elsewhere (design §0.2).
#[inline]
fn affine_scale_factors(t: &Affine) -> (f64, f64) {
    let c = t.as_coeffs();
    (c[0].hypot(c[1]), c[2].hypot(c[3]))
}

/// `atan2(b, a)` — the affine's rotation component (Wave 4 §0.1b).
#[inline]
fn affine_rotation_angle(t: &Affine) -> f64 {
    let c = t.as_coeffs();
    c[1].atan2(c[0])
}

/// Fill a rect with a gradient brush. Walker entry — dispatches to
/// linear/radial/sweep scanline routines. Returns true if the brush was
/// recognised and rendered.
pub(crate) fn fill_rect_gradient_aa(
    pixmap: &mut Pixmap,
    clip:   &ClipStack,
    rect:   Rect,
    gradient: &Gradient,
    transform: &Affine,
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
    // Wave 4 §0.1(a): fold the active rounded-clip mask into per-pixel
    // coverage, exactly mirroring `fill.rs:57,76-78`'s `use_mask`
    // pattern — a `FillRect{radii: Some(_), brush: Gradient(_)}` pushes
    // a REAL mask (`backend.rs`'s `FillRect` arm), this path just never
    // consulted it before this fix.
    let use_mask = !clip.all_rect();

    match gradient.kind {
        // peniko 0.6: GradientKind variants are tuple-wrapped over position structs.
        GradientKind::Linear(peniko::LinearGradientPosition { start, end }) => {
            // Wave 4 §0.1(b): the gradient axis is transformed through
            // the SAME affine as `rect` — untransformed axis + a
            // transformed rect was the bug (silently correct only
            // under `Affine::IDENTITY`).
            let start = transform_point_full(transform, start);
            let end = transform_point_full(transform, end);
            // dot(d, d) — squared gradient axis length.
            let dx = end.x - start.x;
            let dy = end.y - start.y;
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 < 1e-9 {
                // Degenerate — fill with stop[0].
                let c = gradient.stops.first()
                    .map(premul)
                    .unwrap_or([0, 0, 0, 0]);
                fill_solid(pixmap, clip, use_mask, ix0, iy0, ix1, iy1, &visible, c);
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
                    let sample = lut_sample(&lut, t);
                    let mut cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
                    if use_mask {
                        let mask_cov = clip.pixel_coverage(px, py);
                        cov = ((cov as u32 * mask_cov as u32 + 127) / 255) as u8;
                        if cov == 0 { continue; }
                    }
                    let sample = if cov < 255 { scale_premul(sample, cov) } else { sample };
                    pixmap.blend_pixel(px as u32, py as u32, sample);
                }
            }
        }
        GradientKind::Radial(peniko::RadialGradientPosition { start_center, start_radius, end_center, end_radius }) => {
            // Honest scope: only concentric (start_center ≈ end_center
            // AND start_radius ≈ 0) is exact. For focal variants we
            // approximate as concentric on `end_center / end_radius`
            // and bump a "degraded" counter so dashboards see it. A
            // proper two-point conical solver lands when a consumer
            // produces real focal gradients (Lottie, SVG complex).
            let dx_c = (end_center.x - start_center.x).abs();
            let dy_c = (end_center.y - start_center.y).abs();
            let focal = dx_c > 0.5 || dy_c > 0.5 || start_radius.abs() > 0.5;
            if focal {
                metrics::counter!(
                    uzor_urx_core::metrics_keys::KEY_RENDER_PRIMITIVES,
                    "kind" => "gradient_radial_focal_degraded"
                ).increment(1);
            }
            // Wave 4 §0.1(b): transform end_center + scale end_radius
            // by the affine's average axis scale — same "collapse
            // anisotropic scale to one scalar" convention already used
            // for stroke width elsewhere in this family (design §0.2).
            let center = transform_point_full(transform, end_center);
            let (sx, sy) = affine_scale_factors(transform);
            let cx = center.x as f32;
            let cy = center.y as f32;
            let radius = ((end_radius as f64 * (sx + sy) * 0.5).max(1e-3)) as f32;
            let inv_r = 1.0 / radius;
            for py in iy0 .. iy1 {
                let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
                if v_cov == 0 { continue; }
                let pcy = py as f32 + 0.5 - cy;
                let pcy2 = pcy * pcy;
                for px in ix0 .. ix1 {
                    let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
                    if h_cov == 0 { continue; }
                    let pcx = px as f32 + 0.5 - cx;
                    let dist = (pcx * pcx + pcy2).sqrt();
                    let t_raw = dist * inv_r;
                    let t = apply_spread(t_raw, gradient.extend);
                    let sample = lut_sample(&lut, t);
                    let mut cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
                    if use_mask {
                        let mask_cov = clip.pixel_coverage(px, py);
                        cov = ((cov as u32 * mask_cov as u32 + 127) / 255) as u8;
                        if cov == 0 { continue; }
                    }
                    let sample = if cov < 255 { scale_premul(sample, cov) } else { sample };
                    pixmap.blend_pixel(px as u32, py as u32, sample);
                }
            }
        }
        GradientKind::Sweep(peniko::SweepGradientPosition { center, start_angle, end_angle }) => {
            // Angular gradient — t = (angle - start) / (end - start),
            // wrapped per spread mode. atan2 per pixel; not vectorised
            // yet but fine for typical sweep usage (small radial pies
            // / circular progress bars).
            //
            // Wave 4 §0.1(b): transform `center` and add the affine's
            // rotation component to both angles.
            let center = transform_point_full(transform, center);
            let rot = affine_rotation_angle(transform) as f32;
            let cx = center.x as f32;
            let cy = center.y as f32;
            let start_angle = start_angle + rot;
            let end_angle = end_angle + rot;
            let mut span = end_angle - start_angle;
            if span.abs() < 1e-6 {
                span = std::f32::consts::TAU; // full circle default
            }
            let inv_span = 1.0 / span;
            for py in iy0 .. iy1 {
                let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
                if v_cov == 0 { continue; }
                let pcy = py as f32 + 0.5 - cy;
                for px in ix0 .. ix1 {
                    let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
                    if h_cov == 0 { continue; }
                    let pcx = px as f32 + 0.5 - cx;
                    let ang = pcy.atan2(pcx);
                    let t_raw = (ang - start_angle) * inv_span;
                    let t = apply_spread(t_raw, gradient.extend);
                    let sample = lut_sample(&lut, t);
                    let mut cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
                    if use_mask {
                        let mask_cov = clip.pixel_coverage(px, py);
                        cov = ((cov as u32 * mask_cov as u32 + 127) / 255) as u8;
                        if cov == 0 { continue; }
                    }
                    let sample = if cov < 255 { scale_premul(sample, cov) } else { sample };
                    pixmap.blend_pixel(px as u32, py as u32, sample);
                }
            }
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

fn fill_solid(
    pixmap: &mut Pixmap,
    clip: &ClipStack,
    use_mask: bool,
    ix0: i64, iy0: i64, ix1: i64, iy1: i64,
    visible: &Rect,
    color: [u8; 4],
) {
    for py in iy0 .. iy1 {
        let v_cov = crate::fill::axis_coverage(py as f64, py as f64 + 1.0, visible.y0, visible.y1);
        if v_cov == 0 { continue; }
        for px in ix0 .. ix1 {
            let h_cov = crate::fill::axis_coverage(px as f64, px as f64 + 1.0, visible.x0, visible.x1);
            if h_cov == 0 { continue; }
            let mut cov = ((h_cov as u32 * v_cov as u32 + 127) / 255) as u8;
            if use_mask {
                let mask_cov = clip.pixel_coverage(px, py);
                cov = ((cov as u32 * mask_cov as u32 + 127) / 255) as u8;
                if cov == 0 { continue; }
            }
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
    transform: &Affine,
) -> Option<bool> {
    match brush {
        Brush::Gradient(g) => Some(fill_rect_gradient_aa(pixmap, clip, rect, g, transform)),
        _ => None,
    }
}

