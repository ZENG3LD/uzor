//! Pairwise pixel-diff comparator + the divergence-policy tolerance.
//!
//! Same algorithm as `uzor-examples/src/parity_harness.rs`'s own
//! `compare_tight` (itself reusing `uzor-urx-wgpu/tests/parity.rs`'s
//! proven per-primitive tolerance shape) — generalized to any two
//! equal-length premultiplied-RGBA8 buffers.
//!
//! **This is NOT a byte-tight parity gate.** tiny-skia, vello-cpu, and
//! urx-cpu are three independent rasterizer implementations with their
//! own AA algorithms, gamma handling, and (for text) shaping/hinting —
//! real, legitimate visual differences are expected on every real proof
//! scene and are NOT bugs. The one thing this comparator's default
//! tolerance ([`ChannelTolerance::default`]) IS tuned to catch is a
//! STRUCTURAL defect: a whole primitive (a mark, a run of text, a
//! gradient fill) silently missing on one leg — which drives the
//! differing-pixel fraction far above what any AA/text-rasterization
//! difference alone produces. See [`STRUCTURAL_DEFECT_FRACTION`]'s own
//! doc comment for the empirical basis of the actual number.

/// Tolerance for [`compare_tight`].
#[derive(Debug, Clone, Copy)]
pub struct ChannelTolerance {
    /// A pixel counts as "differing" when its max per-channel absolute
    /// diff (0-255) exceeds this. Generous by design — ordinary AA-edge
    /// and text-rasterization differences between three independent
    /// rasterizers routinely reach this magnitude on real content
    /// without indicating any defect.
    pub edge: i32,
    /// The whole-image differing-pixel-fraction budget
    /// [`DiffReport::within_budget`] is checked against.
    pub max_differing_fraction: f64,
}

/// Per-channel diff magnitude below which a pixel is EXPECTED to differ
/// between three independent CPU rasterizers on ordinary AA-edge/text
/// content (measured empirically across the converted proofs — see this
/// crate's own divergence-findings report; a bare solid-fill scene with
/// no AA edges at all measures 0 here across every leg pair).
pub const STRUCTURAL_EDGE_TOLERANCE: i32 = 40;

/// The generous whole-image differing-fraction gate this crate's proof
/// tests use by default (via [`ChannelTolerance::default`]).
///
/// **Reasoning**: measured pairwise `differing_fraction` at
/// [`STRUCTURAL_EDGE_TOLERANCE`] across every converted `uzor-figures`
/// proof (GapPolicy/NaN, label-rotation, a text-heavy KPI tile row, a
/// gradient-heavy heatmap) never exceeded roughly a tenth of the canvas
/// even on the densest text/gradient content — AA fringes and font
/// rasterization differences are confined to edges/glyph outlines, a
/// small minority of any real figure's own pixels. A genuinely MISSING
/// primitive (an entire mark, an entire text run, an entire gradient
/// fill silently absent on one leg) instead flips a LARGE, contiguous
/// fraction of the canvas from "differs" to "matches the background" —
/// structurally a different failure shape, not a bigger version of the
/// same one. `0.35` sits well above the highest real AA/text measurement
/// seen and well below what any single missing primitive on a
/// non-trivial figure produces, so it fires on the latter and stays
/// silent on the former.
pub const STRUCTURAL_DEFECT_FRACTION: f64 = 0.35;

impl Default for ChannelTolerance {
    fn default() -> Self {
        Self { edge: STRUCTURAL_EDGE_TOLERANCE, max_differing_fraction: STRUCTURAL_DEFECT_FRACTION }
    }
}

/// Result of [`compare_tight`].
#[derive(Debug, Clone, Copy)]
pub struct DiffReport {
    pub differing_fraction: f64,
    pub max_channel_diff: i32,
    pub within_budget: bool,
}

fn max_channel_diff_at(a: &[u8], b: &[u8]) -> i32 {
    a.iter().zip(b.iter()).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0)
}

/// Whole-image comparator — generalizes `uzor-examples/src/
/// parity_harness.rs`'s own `compare_tight`/`check_whole_image_budget` to
/// any two equal-length premultiplied-RGBA8 buffers, so a proof reuses
/// the SAME proven algorithm rather than a new one.
pub fn compare_tight(a: &[u8], b: &[u8], tol: ChannelTolerance) -> DiffReport {
    debug_assert_eq!(a.len(), b.len(), "compare_tight: buffers must be the same length");
    let total_pixels = a.len() / 4;
    if total_pixels == 0 {
        return DiffReport { differing_fraction: 0.0, max_channel_diff: 0, within_budget: true };
    }
    let mut differing = 0usize;
    let mut max_diff = 0i32;
    for i in 0..total_pixels {
        let idx = i * 4;
        let d = max_channel_diff_at(&a[idx..idx + 4], &b[idx..idx + 4]);
        max_diff = max_diff.max(d);
        if d > tol.edge {
            differing += 1;
        }
    }
    let differing_fraction = differing as f64 / total_pixels as f64;
    DiffReport { differing_fraction, max_channel_diff: max_diff, within_budget: differing_fraction <= tol.max_differing_fraction }
}

/// Every pairwise [`DiffReport`] across the three legs of a
/// [`crate::render::ThreeLegRender`].
#[derive(Debug, Clone, Copy)]
pub struct ThreeLegDiff {
    pub tiny_skia_vs_vello_cpu: DiffReport,
    pub tiny_skia_vs_urx_cpu: DiffReport,
    pub vello_cpu_vs_urx_cpu: DiffReport,
}

impl ThreeLegDiff {
    /// Compute every pairwise [`compare_tight`] across `render`'s three
    /// legs.
    pub fn compute(render: &crate::render::ThreeLegRender, tol: ChannelTolerance) -> Self {
        Self {
            tiny_skia_vs_vello_cpu: compare_tight(&render.tiny_skia, &render.vello_cpu, tol),
            tiny_skia_vs_urx_cpu: compare_tight(&render.tiny_skia, &render.urx_cpu, tol),
            vello_cpu_vs_urx_cpu: compare_tight(&render.vello_cpu, &render.urx_cpu, tol),
        }
    }

    /// `true` only when EVERY pairwise comparison is within its own
    /// [`ChannelTolerance::max_differing_fraction`] budget.
    pub fn all_within_budget(&self) -> bool {
        self.tiny_skia_vs_vello_cpu.within_budget && self.tiny_skia_vs_urx_cpu.within_budget && self.vello_cpu_vs_urx_cpu.within_budget
    }

    /// One human-readable line per pair — printed by every converted
    /// proof test (`cargo test -- --nocapture`) so the measured numbers
    /// surface directly, without a human having to re-derive them.
    pub fn report_lines(&self) -> [String; 3] {
        [
            format!(
                "tiny-skia vs vello-cpu: differing_fraction={:.4} max_channel_diff={}",
                self.tiny_skia_vs_vello_cpu.differing_fraction, self.tiny_skia_vs_vello_cpu.max_channel_diff
            ),
            format!(
                "tiny-skia vs urx-cpu:   differing_fraction={:.4} max_channel_diff={}",
                self.tiny_skia_vs_urx_cpu.differing_fraction, self.tiny_skia_vs_urx_cpu.max_channel_diff
            ),
            format!(
                "vello-cpu vs urx-cpu:   differing_fraction={:.4} max_channel_diff={}",
                self.vello_cpu_vs_urx_cpu.differing_fraction, self.vello_cpu_vs_urx_cpu.max_channel_diff
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_tight_identical_buffers_are_within_budget() {
        let a = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
        let report = compare_tight(&a, &a, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 0);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(report.within_budget);
    }

    #[test]
    fn compare_tight_flags_a_large_difference_outside_the_edge_tolerance() {
        let a = vec![0u8, 0, 0, 255];
        let b = vec![255u8, 255, 255, 255];
        let report = compare_tight(&a, &b, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 255);
        assert_eq!(report.differing_fraction, 1.0);
        assert!(!report.within_budget);
    }

    #[test]
    fn compare_tight_tolerates_a_small_difference_within_the_edge_tolerance() {
        let a = vec![100u8, 100, 100, 255];
        let b = vec![110u8, 100, 100, 255]; // diff = 10, well under the default edge of 40
        let report = compare_tight(&a, &b, ChannelTolerance::default());
        assert_eq!(report.max_channel_diff, 10);
        assert_eq!(report.differing_fraction, 0.0);
        assert!(report.within_budget);
    }

    #[test]
    fn compare_tight_empty_buffers_are_within_budget() {
        let report = compare_tight(&[], &[], ChannelTolerance::default());
        assert_eq!(report.differing_fraction, 0.0);
        assert!(report.within_budget);
    }
}
