//! SIMD-accelerated span fill — 4-pixels-at-once fused
//! (cov × premul → src-over blend).
//!
//! Bit-exact to scalar via `(x + 127) * 0x8081 >> 23` div255.
//! Verified by `tests/simd_parity.rs` + `simd::tests::div255_*`.

use wide::u32x4;

use crate::fill::axis_coverage;
use crate::pixmap::Pixmap;

/// Memset a contiguous run of `u32` pixels (premultiplied RGBA8
/// packed little-endian) with one solid color. Goes via
/// `multiversion`-dispatched 256-bit `u32x8` (8 pixels / iteration on
/// AVX2; 4 pixels via 2× NEON 128-bit on aarch64; etc).
///
/// `slice::fill` from libcore already lowers to SIMD on most release
/// builds, but the loop-unrolled `u32x8` variant lets us guarantee
/// the wide store regardless of LLVM's heuristics, AND it's the same
/// kernel we'll use for AVX2 8-pixel SOLID-FILL strides in the tile
/// pipeline (CPU-5).
///
/// Caller MUST ensure `dst` is u32-aligned (4-byte). The wider
/// SIMD store falls back to `slice::fill` for the head/tail when
/// `dst.len()` is not a multiple of 8 — bit-exact result.
#[multiversion::multiversion(targets = "simd")]
pub(crate) fn memset_u32_simd(dst: &mut [u32], word: u32) {
    use wide::u32x8;

    // Head: u32x8-aligned offset. dst is already u32-aligned; we just
    // need the slice start to be 32-byte aligned for the safest store.
    // But `wide::u32x8` stores via unaligned access so head=0 is fine.
    let n = dst.len();
    if n == 0 { return; }

    let splat = u32x8::splat(word);
    let chunk_count = n / 8;
    for i in 0..chunk_count {
        // SAFETY: slice indexing checked at len; chunk_count = n/8.
        let off = i * 8;
        let dst_chunk: &mut [u32; 8] = (&mut dst[off..off + 8]).try_into().unwrap();
        *dst_chunk = splat.to_array();
    }
    // Scalar tail (0..7 pixels).
    for slot in dst.iter_mut().skip(chunk_count * 8) {
        *slot = word;
    }
}

#[inline(always)]
fn div255_u32(x: u32) -> u32 {
    ((x + 127).wrapping_mul(0x8081)) >> 23
}

#[inline(always)]
fn div255_u32x4(x: u32x4) -> u32x4 {
    let bias = u32x4::splat(127);
    let mul  = u32x4::splat(0x8081);
    (x + bias) * mul >> 23
}

/// Pixmap-based entry — wrapper around `fill_span_into_slice`.
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
    fill_span_into_slice(pixels, row_off, ix0, ix1, fx0, fx1, v_cov, src_premul);
}

/// Slice variant. `row_off` is the byte offset to the start of the
/// target row within `pixels`. Caller is responsible for clamping
/// `ix0..ix1` to the slice's logical width.
#[multiversion::multiversion(targets = "simd")]
pub(crate) fn fill_span_into_slice(
    pixels:     &mut [u8],
    row_off:    usize,
    ix0:        i64,
    ix1:        i64,
    fx0:        f64,
    fx1:        f64,
    v_cov:      u8,
    src_premul: [u8; 4],
) {
    if v_cov == 0 || ix0 >= ix1 { return; }
    let v = v_cov as u32;
    let src_v = u32x4::new([
        src_premul[0] as u32,
        src_premul[1] as u32,
        src_premul[2] as u32,
        src_premul[3] as u32,
    ]);

    let mut px = ix0;
    let chunk_end = ix1 - ((ix1 - ix0) % 4);

    while px < chunk_end {
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

        for (i, &cov) in [cov0, cov1, cov2, cov3].iter().enumerate() {
            if cov == 0 { continue; }
            let cov_v = u32x4::splat(cov);
            let scaled = div255_u32x4(src_v * cov_v);
            let scaled_arr = scaled.to_array();
            let scaled_a = scaled_arr[3];
            if scaled_a == 0 { continue; }
            let inv_a = u32x4::splat(255 - scaled_a);
            let pos = row_off + ((px as usize) + i) * 4;
            let dst = u32x4::new([
                pixels[pos    ] as u32,
                pixels[pos + 1] as u32,
                pixels[pos + 2] as u32,
                pixels[pos + 3] as u32,
            ]);
            let bg = div255_u32x4(dst * inv_a);
            let out = scaled + bg;
            let oa = out.to_array();
            pixels[pos    ] = oa[0].min(255) as u8;
            pixels[pos + 1] = oa[1].min(255) as u8;
            pixels[pos + 2] = oa[2].min(255) as u8;
            pixels[pos + 3] = oa[3].min(255) as u8;
        }
        px += 4;
    }

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
        let pos = row_off + (px as usize) * 4;
        let inv_a = 255 - s[3] as u32;
        pixels[pos    ] = (s[0] as u32 + div255_u32(pixels[pos    ] as u32 * inv_a)).min(255) as u8;
        pixels[pos + 1] = (s[1] as u32 + div255_u32(pixels[pos + 1] as u32 * inv_a)).min(255) as u8;
        pixels[pos + 2] = (s[2] as u32 + div255_u32(pixels[pos + 2] as u32 * inv_a)).min(255) as u8;
        pixels[pos + 3] = (s[3] as u32 + div255_u32(pixels[pos + 3] as u32 * inv_a)).min(255) as u8;
        px += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn div255_matches_round_half_up() {
        for x in 0u32..=(255 * 255) {
            let exact = (x + 127) / 255;
            let fast = div255_u32(x);
            assert_eq!(exact, fast, "mismatch at x={}", x);
        }
    }
}
