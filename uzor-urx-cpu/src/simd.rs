//! SIMD-accelerated span fill — 4-pixels-at-once fused
//! (cov × premul → src-over blend).
//!
//! Math is BIT-EXACT identical to `fill::axis_coverage` +
//! `color::premul_scale` + `Pixmap::blend_pixel` chained scalar.
//! Verified by `tests/simd_parity.rs`. If you change arithmetic
//! here, change it there too — and re-run parity tests.
//!
//! Key trick: `(x + 127) / 255` (round-half-up integer div) is
//! exactly reproducible as `(x + 127) * 0x8081 >> 23` for
//! `x in [0, 255*255]`. That keeps everything in u32 lanes without
//! triggering wide's Div (which doesn't exist for u32x4). Same
//! result, no FP, no branches.
//!
//! Multiversioning: `targets = "simd"` lets multiversion pick AVX2
//! / SSE4.2 / SSE2 / NEON at runtime. The lane width stays u32x4
//! — 16 bytes per register — which AVX2 handles in one cycle on
//! a typical Zen/Skylake.

use wide::u32x4;

use crate::fill::axis_coverage;
use crate::pixmap::Pixmap;

/// `(x + 127) / 255` exactly, for `x` up to `255 * 255`. Hot path
/// inner; benchmarked to compile to a multiply + shift on x86_64.
#[inline(always)]
fn div255_u32(x: u32) -> u32 {
    ((x + 127).wrapping_mul(0x8081)) >> 23
}

/// SIMD equivalent — applied per-lane, same exact result.
#[inline(always)]
fn div255_u32x4(x: u32x4) -> u32x4 {
    // (x + 127) * 0x8081 >> 23
    let bias = u32x4::splat(127);
    let mul  = u32x4::splat(0x8081);
    (x + bias) * mul >> 23
}

/// Fill ONE pixel row `[ix0, ix1)` in `pixmap` with the given
/// premultiplied source colour, weighted per-pixel by horizontal
/// coverage from `[fx0, fx1]` and the row vertical coverage `v_cov`.
#[multiversion::multiversion(targets = "simd")]
pub(crate) fn fill_span_aa(
    pixmap: &mut Pixmap,
    py: u32,
    ix0: i64, ix1: i64,
    fx0: f64, fx1: f64,
    v_cov: u8,
    src_premul: [u8; 4],
) {
    if v_cov == 0 || ix0 >= ix1 { return; }
    let w = pixmap.width() as i64;
    let ix0 = ix0.max(0);
    let ix1 = ix1.min(w);
    if ix0 >= ix1 { return; }
    let row_off = (py as usize) * (pixmap.width() as usize) * 4;
    let pixels = pixmap.pixels_mut();
    let v = v_cov as u32;

    // SIMD broadcast of src colour (one register, 4 lanes).
    let src_v = u32x4::new([
        src_premul[0] as u32,
        src_premul[1] as u32,
        src_premul[2] as u32,
        src_premul[3] as u32,
    ]);
    let two_55 = u32x4::splat(255);

    let mut px = ix0;
    let chunk_end = ix1 - ((ix1 - ix0) % 4);

    while px < chunk_end {
        // Per-pixel coverage via scalar (axis_coverage is FP — keeping
        // it scalar means we don't have to vectorise the f64 inputs).
        let h0 = axis_coverage((px    ) as f64, (px    ) as f64 + 1.0, fx0, fx1) as u32;
        let h1 = axis_coverage((px + 1) as f64, (px + 1) as f64 + 1.0, fx0, fx1) as u32;
        let h2 = axis_coverage((px + 2) as f64, (px + 2) as f64 + 1.0, fx0, fx1) as u32;
        let h3 = axis_coverage((px + 3) as f64, (px + 3) as f64 + 1.0, fx0, fx1) as u32;

        let cov0 = div255_u32(h0 * v);
        let cov1 = div255_u32(h1 * v);
        let cov2 = div255_u32(h2 * v);
        let cov3 = div255_u32(h3 * v);

        if (cov0 | cov1 | cov2 | cov3) == 0 {
            px += 4;
            continue;
        }

        // Each pixel: src_scaled[k] = (src[k] * cov + 127) / 255
        // src_scaled is RGBA but the same `cov` applies to all 4
        // channels, so we broadcast cov over u32x4 lanes.
        for (i, &cov) in [cov0, cov1, cov2, cov3].iter().enumerate() {
            if cov == 0 { continue; }
            let cov_v = u32x4::splat(cov);
            let scaled = div255_u32x4(src_v * cov_v);
            let scaled_a = scaled.to_array()[3];
            if scaled_a == 0 { continue; }
            let inv_a = u32x4::splat(255 - scaled_a);

            let pos = row_off + ((px as usize) + i) * 4;
            let dst = u32x4::new([
                pixels[pos    ] as u32,
                pixels[pos + 1] as u32,
                pixels[pos + 2] as u32,
                pixels[pos + 3] as u32,
            ]);
            // dst_new = scaled + (dst * inv_a + 127) / 255, lane-clamped to 255.
            let bg = div255_u32x4(dst * inv_a);
            let out = scaled + bg;
            let oa = out.to_array();
            let _ = two_55; // silence unused (used later if we add a min())
            pixels[pos    ] = oa[0].min(255) as u8;
            pixels[pos + 1] = oa[1].min(255) as u8;
            pixels[pos + 2] = oa[2].min(255) as u8;
            pixels[pos + 3] = oa[3].min(255) as u8;
        }
        px += 4;
    }

    // Scalar tail (< 4 pixels remaining).
    while px < ix1 {
        let h = axis_coverage(px as f64, px as f64 + 1.0, fx0, fx1) as u32;
        let cov = div255_u32(h * v);
        if cov == 0 { px += 1; continue; }
        let s = [
            div255_u32(src_premul[0] as u32 * cov) as u8,
            div255_u32(src_premul[1] as u32 * cov) as u8,
            div255_u32(src_premul[2] as u32 * cov) as u8,
            div255_u32(src_premul[3] as u32 * cov) as u8,
        ];
        pixmap.blend_pixel(px as u32, py, s);
        px += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div255_matches_round_half_up() {
        // Smoke: across the realistic input range, our multiply-shift
        // must match (x + 127) / 255 exactly.
        for x in 0u32..=(255 * 255) {
            let exact = (x + 127) / 255;
            let fast = div255_u32(x);
            assert_eq!(exact, fast, "mismatch at x={}", x);
        }
    }
}
