//! Same max-channel-diff + differing-fraction algorithm as
//! `uzor-proof-harness/src/compare.rs`'s `compare_tight`, reused in SHAPE
//! (re-implemented at a fraction of the size, not imported — see the
//! design-decision doc comment in the H0 harness design doc,
//! `docs/uzor/plans/h0-harness-and-a11y-design-2026-09-24.md` §1) with a
//! tolerance tuned for same-backend regression testing (one renderer
//! against its own committed reference) instead of cross-backend
//! structural-defect detection (three independent rasterizers against
//! each other).

/// Tolerance for a golden-PNG pixel comparison.
#[derive(Debug, Clone, Copy)]
pub struct GoldenTolerance {
    /// Per-channel abs diff (0-255) below which a pixel counts as
    /// matching. Same renderer, same backend, same embedded fonts — only
    /// needs to absorb float-rounding jitter at AA edges, not cross-
    /// rasterizer AA differences, so this is far tighter than
    /// `uzor-proof-harness::compare::STRUCTURAL_EDGE_TOLERANCE` (40).
    pub edge: i32,
    /// Fraction (0.0-1.0) of pixels allowed to exceed `edge` before the
    /// comparison counts as a mismatch.
    pub max_differing_fraction: f64,
}

impl Default for GoldenTolerance {
    fn default() -> Self {
        Self { edge: 2, max_differing_fraction: 0.001 }
    }
}

/// Numeric summary of a [`diff`] call.
#[derive(Debug, Clone, Copy)]
pub struct GoldenReport {
    /// Fraction of pixels whose max per-channel diff exceeded
    /// [`GoldenTolerance::edge`].
    pub differing_fraction: f64,
    /// The single largest per-channel diff seen anywhere in the image.
    pub max_channel_diff: i32,
}

/// Compares two premultiplied RGBA8 buffers of identical length, `a`
/// (golden) against `b` (actual). Returns a report plus whether the
/// comparison is within `tol`.
pub(super) fn diff(a: &[u8], b: &[u8], tol: GoldenTolerance) -> (GoldenReport, bool) {
    let pixel_count = a.len() / 4;
    if pixel_count == 0 {
        return (GoldenReport { differing_fraction: 0.0, max_channel_diff: 0 }, true);
    }

    let mut differing = 0usize;
    let mut max_channel_diff = 0i32;

    for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let pixel_max = pa
            .iter()
            .zip(pb.iter())
            .map(|(x, y)| (*x as i32 - *y as i32).abs())
            .max()
            .unwrap_or(0);
        max_channel_diff = max_channel_diff.max(pixel_max);
        if pixel_max > tol.edge {
            differing += 1;
        }
    }

    let differing_fraction = differing as f64 / pixel_count as f64;
    let within_tolerance = differing_fraction <= tol.max_differing_fraction;
    (GoldenReport { differing_fraction, max_channel_diff }, within_tolerance)
}

/// Builds a visual diff image, same size as `a`/`b`: any pixel that
/// differs at all (any per-channel diff > 0, independent of
/// [`GoldenTolerance::edge`] — this is for a human looking at the PNG,
/// not another tolerance gate) is painted opaque red; matching pixels are
/// left fully transparent.
pub(super) fn diff_image(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; a.len()];
    for ((pa, pb), po) in a.chunks_exact(4).zip(b.chunks_exact(4)).zip(out.chunks_exact_mut(4)) {
        let differs = pa.iter().zip(pb.iter()).any(|(x, y)| x != y);
        if differs {
            // Opaque red, premultiplied — alpha 255 means premultiplied
            // and straight are identical for this color.
            po[0] = 255;
            po[1] = 0;
            po[2] = 0;
            po[3] = 255;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_identical_buffers_are_within_tolerance() {
        let a = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let (report, within) = diff(&a, &a, GoldenTolerance::default());
        assert_eq!(report.max_channel_diff, 0);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(within);
    }

    #[test]
    fn diff_flags_a_large_difference_outside_the_edge_tolerance() {
        let a = vec![0u8, 0, 0, 255];
        let b = vec![255u8, 255, 255, 255];
        let (report, within) = diff(&a, &b, GoldenTolerance::default());
        assert_eq!(report.max_channel_diff, 255);
        assert_eq!(report.differing_fraction, 1.0);
        assert!(!within);
    }

    #[test]
    fn diff_tolerates_a_small_difference_within_the_edge_tolerance() {
        let a = vec![100u8, 100, 100, 255];
        let b = vec![101u8, 100, 100, 255]; // diff = 1, under the default edge of 2
        let (report, within) = diff(&a, &b, GoldenTolerance::default());
        assert_eq!(report.max_channel_diff, 1);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(within);
    }

    #[test]
    fn diff_empty_buffers_are_within_tolerance() {
        let (report, within) = diff(&[], &[], GoldenTolerance::default());
        assert_eq!(report.differing_fraction, 0.0);
        assert!(within);
    }

    #[test]
    fn diff_image_paints_only_the_differing_pixels_red() {
        let a = vec![0u8, 0, 0, 255, /**/ 10, 10, 10, 255];
        let b = vec![0u8, 0, 0, 255, /**/ 200, 10, 10, 255];
        let out = diff_image(&a, &b);
        assert_eq!(&out[0..4], &[0, 0, 0, 0], "matching pixel must stay transparent");
        assert_eq!(&out[4..8], &[255, 0, 0, 255], "differing pixel must be painted opaque red");
    }
}
