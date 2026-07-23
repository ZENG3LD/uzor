//! Pure gradient-LUT math — shared by `uzor-urx-cpu::gradient` and (Wave
//! 4+) `uzor-urx-wgpu`'s native `GradientLutAtlas`, so a LUT built from
//! identical stops is byte-identical on both backends.
//!
//! `docs/uzor-engines/plans/urx-wave4-vello-parity-design-2026-07-25.md`
//! §2.2/§10 Commit 1. Extracted VERBATIM from
//! `uzor-urx-cpu/src/gradient.rs` (mechanical relocation, zero logic
//! change — same "share the canonical implementation, don't fork"
//! doctrine as Wave 2's `uzor-urx-glyph` `GlyphKey`/`subpixel_bin_for_x`
//! extraction). `uzor-urx-cpu::gradient` is refactored to call these
//! instead of keeping its own copies; its existing gradient-behaviour
//! (LUT shape, hash stability, spread folding) is unchanged — only
//! WHERE the code lives moved.
//!
//! Deliberately NOT extracted (stay CPU-side — not pure functions, or
//! not needed by a GPU consumer):
//! - `LutCache` / the process-global `LUT_CACHE` / `get_lut` — CPU's
//!   OWN caching policy (an LRU keyed by [`hash_stops`]'s result); the
//!   GPU-side `GradientLutAtlas` (Wave 4 Commit 2) has its OWN,
//!   texture-row-shaped caching mechanism, not this one.
//! - `lut_sample` — CPU's own exact-index LUT read against an
//!   in-memory `GradientLut`. GPU reads via `textureLoad` directly in
//!   the fragment shader (design §2.3), never calling into Rust code
//!   per-fragment.

use crate::math::{ColorStop, Extend};

/// LUT resolution — 256 entries, matching CPU's existing scanline
/// rasteriser (`uzor-urx-cpu::gradient`) and (Wave 4+) the GPU atlas's
/// texture width (`GradientLutAtlas`, `256 x N Rgba8Unorm`).
pub const LUT_SIZE: usize = 256;

/// A pre-built lookup table — `LUT_SIZE` entries of premultiplied
/// RGBA8, built by lerping consecutive stops in premultiplied space.
pub type GradientLut = [[u8; 4]; LUT_SIZE];

/// Straight (non-premultiplied) sRGB byte quad for a stop.
// peniko 0.6: ColorStop.color is `DynamicColor`. Convert to sRGB byte
// quad at the stop boundary so the rest of the gradient math (premul +
// lerp) stays in u8 space exactly as before.
#[inline]
pub fn stop_rgba8(s: &ColorStop) -> [u8; 4] {
    let p = s.color.to_alpha_color::<peniko::color::Srgb>().to_rgba8();
    [p.r, p.g, p.b, p.a]
}

/// Deterministic FNV-1a 64-bit hash over the stop bytes + extend mode.
/// Stable across processes (unlike `DefaultHasher`) so cache hits work
/// reliably (on EITHER backend's own cache) and collisions are rare
/// for the `ColorStop` counts this family expects. Byte-identical
/// hashing on both backends is what lets a GPU `GradientLutAtlas` row
/// and a CPU `LutCache` entry, built from the SAME stops, be
/// recognised as "the same gradient" even though they're two entirely
/// separate cache implementations.
pub fn hash_stops(stops: &[ColorStop], extend: Extend) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001b3;
    let mut h = FNV_OFFSET;
    let mut feed = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    };
    for s in stops {
        for b in s.offset.to_bits().to_le_bytes() {
            feed(b);
        }
        let [r, g, b8, a] = stop_rgba8(s);
        feed(r);
        feed(g);
        feed(b8);
        feed(a);
    }
    feed(extend as u8);
    h
}

/// Build a `LUT_SIZE`-entry RGBA8 LUT by linear-premul interpolation
/// between consecutive color stops. Implementation note (carried over
/// verbatim from the pre-extraction CPU comment): interpolates in
/// **sRGB-premul** space (matches HTML Canvas2D / this family's
/// existing WGPU blend convention) — linear-space interpolation would
/// give more physically-correct mid-tones but breaks parity with the
/// WGPU adapter, which doesn't do sRGB->linear conversion before
/// `mix()` either. Doing both together is a later quality pass, not
/// this wave's scope.
pub fn build_lut(stops: &[ColorStop]) -> GradientLut {
    let mut lut: GradientLut = [[0; 4]; LUT_SIZE];
    if stops.is_empty() {
        return lut;
    }
    if stops.len() == 1 {
        let c = premul(&stops[0]);
        for slot in lut.iter_mut() {
            *slot = c;
        }
        return lut;
    }
    for i in 0..LUT_SIZE {
        let t = (i as f32) / (LUT_SIZE as f32 - 1.0);
        lut[i] = sample_stops(stops, t);
    }
    lut
}

pub fn premul(s: &ColorStop) -> [u8; 4] {
    let [r, g, b, a] = stop_rgba8(s);
    let aw = a as u32;
    [
        ((r as u32 * aw + 127) / 255) as u8,
        ((g as u32 * aw + 127) / 255) as u8,
        ((b as u32 * aw + 127) / 255) as u8,
        a,
    ]
}

/// Sample a stop sequence at parameter `t ∈ [0, 1]` and return
/// premultiplied RGBA8 via lerp between bracketing stops.
pub fn sample_stops(stops: &[ColorStop], t: f32) -> [u8; 4] {
    // Stops are normally sorted by `offset`; bracket-search.
    if t <= stops[0].offset {
        return premul(&stops[0]);
    }
    if t >= stops[stops.len() - 1].offset {
        return premul(&stops[stops.len() - 1]);
    }
    for w in stops.windows(2) {
        let s0 = &w[0];
        let s1 = &w[1];
        if t >= s0.offset && t <= s1.offset {
            let span = s1.offset - s0.offset;
            let local = if span < 1e-9 { 0.0 } else { (t - s0.offset) / span };
            let c0 = premul(s0);
            let c1 = premul(s1);
            return [
                lerp_u8(c0[0], c1[0], local),
                lerp_u8(c0[1], c1[1], local),
                lerp_u8(c0[2], c1[2], local),
                lerp_u8(c0[3], c1[3], local),
            ];
        }
    }
    premul(&stops[stops.len() - 1])
}

#[inline]
pub fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let r = (a as f32) * (1.0 - t) + (b as f32) * t;
    r.round().clamp(0.0, 255.0) as u8
}

/// Fold a raw `t` into `[0, 1]` per the spread/extend mode.
#[inline]
pub fn apply_spread(t: f32, mode: Extend) -> f32 {
    match mode {
        Extend::Pad => t.clamp(0.0, 1.0),
        Extend::Repeat => {
            let f = t - t.floor();
            if f < 0.0 {
                f + 1.0
            } else {
                f
            }
        }
        Extend::Reflect => {
            let m = (t.rem_euclid(2.0) - 1.0).abs();
            // Convert [-1,0,1] reflection to [0,1].
            1.0 - m
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Color;

    fn stop(offset: f32, r: u8, g: u8, b: u8, a: u8) -> ColorStop {
        ColorStop { offset, color: Color::from_rgba8(r, g, b, a).into() }
    }

    #[test]
    fn hash_stops_is_deterministic_across_calls() {
        let stops = vec![stop(0.0, 255, 0, 0, 255), stop(1.0, 0, 0, 255, 255)];
        assert_eq!(hash_stops(&stops, Extend::Pad), hash_stops(&stops, Extend::Pad));
    }

    #[test]
    fn hash_stops_differs_by_extend_mode() {
        let stops = vec![stop(0.0, 255, 0, 0, 255), stop(1.0, 0, 0, 255, 255)];
        assert_ne!(hash_stops(&stops, Extend::Pad), hash_stops(&stops, Extend::Repeat));
    }

    #[test]
    fn build_lut_endpoints_match_stop_colors() {
        let stops = vec![stop(0.0, 255, 0, 0, 255), stop(1.0, 0, 0, 255, 255)];
        let lut = build_lut(&stops);
        assert_eq!(lut[0], premul(&stops[0]));
        assert_eq!(lut[LUT_SIZE - 1], premul(&stops[1]));
    }

    #[test]
    fn build_lut_single_stop_fills_every_entry() {
        let stops = vec![stop(0.5, 10, 20, 30, 255)];
        let lut = build_lut(&stops);
        let c = premul(&stops[0]);
        assert!(lut.iter().all(|&entry| entry == c));
    }

    #[test]
    fn apply_spread_pad_clamps() {
        assert_eq!(apply_spread(-0.5, Extend::Pad), 0.0);
        assert_eq!(apply_spread(1.5, Extend::Pad), 1.0);
    }

    #[test]
    fn apply_spread_repeat_wraps() {
        assert!((apply_spread(1.25, Extend::Repeat) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn lerp_u8_midpoint() {
        assert_eq!(lerp_u8(0, 255, 0.5), 128);
    }
}
