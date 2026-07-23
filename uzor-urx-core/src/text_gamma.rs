//! Pure text-gamma coverage-adjustment math — shared by
//! `uzor-urx-glyph::draw_glyph_run` (CPU) and `uzor-urx-wgpu`'s native
//! glyph pipeline (GPU), so a LUT built from the identical curve is
//! byte-identical on both backends (same "shared pure function, no
//! cross-backend fork" doctrine as `gradient_lut.rs`'s own module doc).
//!
//! `docs/uzor-engines/plans/urx-text-gamma-compositing-design-2026-07-26.md`
//! §2.2/§2.3, Commit 1. Closes the "URX text renders visibly lighter/
//! thinner than vello" gap found by the family-parity plan's Wave 6
//! close note — NOT via linear-space compositing (§1.1/§1.2 of that
//! design confirm neither URX nor vello actually does that), but via a
//! coverage-space gamma lift applied at glyph composite time:
//! `cov' = cov ^ (1 / γ(fg_luma))`. `γ` is a pure function of the
//! foreground color only (never the destination/background — the GPU
//! fixed-function blend unit can't read dst, so any dst-aware scheme is
//! structurally impossible on that side, design §2.1), bucketed into a
//! small, fixed row count exactly like `gradient_lut`'s own "a handful
//! of discrete slots, not a smooth per-pixel formula" shape.
//!
//! This module owns ONLY the pure math. It does not read `UrxConfig`,
//! does not cache anything, and does not know which backend is calling
//! it — each backend's own OnceLock/config-resolution site (
//! `uzor-urx-glyph::configured_text_gamma_lut`, `uzor-urx-cpu`'s
//! `CpuBackend` GlyphRun arm, `uzor-urx-wgpu`'s `NativeGlyphAtlas`)
//! decides WHEN to build/cache the table; this file only decides WHAT
//! the table contains for a given curve.

/// LUT row count — one per foreground-luma bucket. Row 0 (darkest fg)
/// is ALWAYS gamma 1.0 by convention (exact passthrough) — enforced by
/// never changing `TEXT_GAMMA_CURVE[0]` away from `1.0`, not by any
/// special-casing inside [`build_text_gamma_lut`] itself (which is a
/// fully generic function of whatever curve it's handed — see its own
/// doc comment). Kept small (a LUT row count, not a smooth curve) —
/// matches `gradient_lut`'s "handful of discrete slots" shape, not a
/// per-pixel-continuous formula.
pub const TEXT_GAMMA_BINS: usize = 2;

/// Coverage-axis resolution — same 256-entry convention as
/// `gradient_lut::LUT_SIZE` (one column per u8 coverage value; GPU
/// samples via exact `textureLoad`, never bilinear — see the design's
/// §2.5(b), "never `pow()` in-shader").
pub const TEXT_GAMMA_LUT_SIZE: usize = 256;

/// A pre-built lookup table — `TEXT_GAMMA_BINS` rows of
/// `TEXT_GAMMA_LUT_SIZE` adjusted-coverage bytes.
pub type TextGammaLut = [[u8; TEXT_GAMMA_LUT_SIZE]; TEXT_GAMMA_BINS];

/// Per-bin gamma exponent. Row 0 is `1.0` (exact passthrough — dark-
/// on-light text, not the reported problem, never changes). Row 1 is
/// a **placeholder** until the calibration sweep (design §3) picks a
/// real value — `1.0` here means the WHOLE pass is a byte-exact no-op
/// even with `UrxConfig::text_gamma_enabled` on, which is exactly
/// Commit 1's own acceptance bar ("mechanism, inert" — design §6
/// Commit 1).
pub const TEXT_GAMMA_CURVE: [f32; TEXT_GAMMA_BINS] = [1.0, /* CALIBRATED */ 1.0];

/// Foreground-luma bucket index in `[0, TEXT_GAMMA_BINS)`, via integer
/// Rec.601 luma (`(299*r + 587*g + 114*b) / 1000`, all `u32` — no
/// floats, no cross-language rounding-order concern since only Rust
/// call sites ever evaluate this: CPU's `draw_glyph_run` and GPU's
/// `encode_glyph_run` each call it independently at THEIR OWN encode
/// site on the identical `[u8;4]` straight color — same "shared pure
/// function, no cross-backend data threading" doctrine as
/// `subpixel_bin_for_x`/`GlyphKey::new`, `uzor-urx-glyph/src/lib.rs:55-66`).
///
/// Boundary: `luma >= 128` (the standard 8-bit midpoint) selects bin 1
/// ("light fg"); everything below selects bin 0 ("dark fg"). The
/// design doc names the two buckets ("dark-on-light" / "light-on-dark")
/// but leaves the exact numeric split to implementation; `TEXT_GAMMA_BINS
/// = 2` keeps this the ONLY boundary in the whole table (design risk 1:
/// "bin-boundary flicker"), and the calibration review (design §3.3)
/// re-checks this choice against real UI text colors before the curve
/// is finalized in Commit 2.
///
/// `fg[3]` (alpha) is intentionally ignored — bucket selection is a
/// pure function of RGB weight, alpha already scales the whole glyph
/// via the existing premultiply step (design risk 5).
pub fn luma_bin(fg: [u8; 4]) -> u8 {
    let luma = (299 * fg[0] as u32 + 587 * fg[1] as u32 + 114 * fg[2] as u32) / 1000;
    if luma >= 128 { 1 } else { 0 }
}

/// Build a `TextGammaLut` from an explicit curve (pure — no static
/// state, no `OnceLock` inside this function itself; callers own
/// caching). Row `i`, column `c`: `round(255 * (c/255)^(1/curve[i]))`,
/// clamped `0..=255`. Parameterized (not hardcoded to
/// `TEXT_GAMMA_CURVE` internally) so the calibration sweep (design §3)
/// can build many candidate LUTs in one process without touching the
/// production constant.
pub fn build_text_gamma_lut(curve: &[f32; TEXT_GAMMA_BINS]) -> TextGammaLut {
    let mut lut: TextGammaLut = [[0u8; TEXT_GAMMA_LUT_SIZE]; TEXT_GAMMA_BINS];
    for (row, gamma) in lut.iter_mut().zip(curve.iter()) {
        let inv_gamma = 1.0_f32 / *gamma;
        for (c, slot) in row.iter_mut().enumerate() {
            let t = c as f32 / 255.0;
            let v = t.powf(inv_gamma) * 255.0;
            *slot = v.round().clamp(0.0, 255.0) as u8;
        }
    }
    lut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bin_0_curve_is_identity_for_the_production_constant() {
        let lut = build_text_gamma_lut(&TEXT_GAMMA_CURVE);
        for c in 0..TEXT_GAMMA_LUT_SIZE {
            assert_eq!(lut[0][c], c as u8, "row 0 (gamma 1.0) must be an exact passthrough at c={c}");
        }
    }

    #[test]
    fn identity_curve_is_identity_on_every_row() {
        let lut = build_text_gamma_lut(&[1.0, 1.0]);
        for row in &lut {
            for (c, &v) in row.iter().enumerate() {
                assert_eq!(v, c as u8, "gamma=1.0 must be an exact passthrough at c={c}");
            }
        }
    }

    #[test]
    fn gamma_greater_than_one_raises_every_non_endpoint_coverage_value() {
        let lut = build_text_gamma_lut(&[1.0, 1.8]);
        // Endpoints are fixed regardless of gamma (0^x = 0, 1^x = 1).
        assert_eq!(lut[1][0], 0);
        assert_eq!(lut[1][255], 255);
        // Every interior value must be lifted (cov^(1/gamma) > cov for gamma > 1, 0 < cov < 1).
        for c in 1..255 {
            assert!(
                lut[1][c] >= c as u8,
                "gamma=1.8 must not lower coverage at c={c}: got {}",
                lut[1][c]
            );
        }
        // At least one interior value must be STRICTLY raised — proves
        // the curve actually did something, not just clamped to a no-op.
        assert!(
            (1..255).any(|c| lut[1][c] > c as u8),
            "gamma=1.8 must strictly raise at least one interior coverage value"
        );
    }

    #[test]
    fn build_text_gamma_lut_is_monotonic_in_coverage() {
        let lut = build_text_gamma_lut(&[1.0, 2.2]);
        for row in &lut {
            for w in row.windows(2) {
                assert!(w[1] >= w[0], "LUT must be non-decreasing in coverage: {} then {}", w[0], w[1]);
            }
        }
    }

    #[test]
    fn build_text_gamma_lut_is_pure_and_deterministic() {
        let a = build_text_gamma_lut(&[1.0, 1.5]);
        let b = build_text_gamma_lut(&[1.0, 1.5]);
        assert_eq!(a, b, "same curve must produce byte-identical tables across calls");
    }

    #[test]
    fn luma_bin_classifies_black_and_white_correctly() {
        assert_eq!(luma_bin([0, 0, 0, 255]), 0, "black fg is bin 0 (dark)");
        assert_eq!(luma_bin([255, 255, 255, 255]), 1, "white fg is bin 1 (light)");
    }

    #[test]
    fn luma_bin_boundary_is_at_128() {
        assert_eq!(luma_bin([127, 127, 127, 255]), 0, "luma 127 must still be bin 0");
        assert_eq!(luma_bin([128, 128, 128, 255]), 1, "luma 128 must be bin 1");
    }

    #[test]
    fn luma_bin_ignores_alpha() {
        assert_eq!(luma_bin([255, 255, 255, 0]), luma_bin([255, 255, 255, 255]), "alpha must not affect bucket selection");
        assert_eq!(luma_bin([0, 0, 0, 0]), luma_bin([0, 0, 0, 255]));
    }

    #[test]
    fn luma_bin_weighs_green_the_most_per_rec601() {
        // Pure green is brighter (per Rec.601 weights) than pure red or
        // blue at the same channel value — proves the weighted formula
        // is actually wired, not e.g. a flat average. At channel value
        // 230: green's 0.587 weight alone crosses the 128 threshold
        // (587*230/1000 = 135), red's 0.299 and blue's 0.114 don't
        // (68 and 26 respectively).
        assert_eq!(luma_bin([0, 230, 0, 255]), 1, "pure green at 230 must cross into bin 1");
        assert_eq!(luma_bin([230, 0, 0, 255]), 0, "pure red at 230 must stay in bin 0");
        assert_eq!(luma_bin([0, 0, 230, 255]), 0, "pure blue at 230 must stay in bin 0 (lowest Rec.601 weight)");
    }
}
