//! Largest-Triangle-Three-Buckets (LTTB) downsampling — Steinarsson 2013,
//! "Downsampling Time Series for Visual Representation". Ported from the
//! algorithm description in
//! `nemo/docs/uzor-engines/research_dataviz_sota_2026.md` (LTTB section,
//! cross-referenced §3/§8) — no external crate (`lttb-rs` et al.), the
//! whole thing is ~40 lines of pure math.
//!
//! **What it optimizes for**: visual shape preservation. The first and
//! last input points are always kept verbatim; every interior output
//! point is the ACTUAL input point (never an interpolated/synthetic one)
//! that maximizes the triangle area formed with the previously-chosen
//! point and the next bucket's own centroid — peaks/valleys/sharp
//! reversals survive because they maximize that area, unlike naive
//! decimation (take every Nth point) or bucket-averaging, both of which
//! flatten exactly those features.
//!
//! **Complexity**: `O(n)`, single left-to-right pass, no sorting, no
//! extra data structures, no RNG — fully deterministic (same input +
//! `threshold` always produces the identical output), which matters for
//! this crate's own "screenshots/diffs must be reproducible" convention
//! (design law 8 elsewhere in this workspace).

/// Downsample `points` to at most `threshold` points via LTTB, preserving
/// visual shape (peaks/valleys survive) far better than naive decimation
/// or bucket-averaging.
///
/// - `threshold >= points.len()` (or `points.len() <= 2`): identity —
///   returns `points` verbatim, nothing to reduce.
/// - `threshold == 0`: empty (nothing requested).
/// - `threshold == 1`: just the first point (a single point is not
///   meaningfully "the shape" of a series, but this degrades gracefully
///   rather than panicking on the `threshold - 2` bucket math below,
///   which needs at least one interior bucket).
/// - `threshold == 2`: just the first and last point.
/// - Otherwise (`3 <= threshold < points.len()`): the real algorithm —
///   first and last kept as fixed anchors, `threshold - 2` interior
///   buckets each contribute the one input point that maximizes the
///   triangle area against the previously-chosen anchor and the NEXT
///   bucket's own centroid (mean, not a specific point).
pub fn lttb(points: &[(f64, f64)], threshold: usize) -> Vec<(f64, f64)> {
    let n = points.len();
    if n == 0 || threshold == 0 {
        return Vec::new();
    }
    if threshold >= n {
        return points.to_vec();
    }
    if threshold == 1 {
        return vec![points[0]];
    }
    if threshold == 2 {
        return vec![points[0], points[n - 1]];
    }

    let bucket_count = threshold - 2;
    let bucket_size = (n - 2) as f64 / bucket_count as f64;

    let mut sampled = Vec::with_capacity(threshold);
    sampled.push(points[0]);
    let mut anchor_index = 0usize;

    for i in 0..bucket_count {
        // This bucket's own [start, end) range of candidate points
        // (offset by +1 — index 0 is the fixed first-point anchor,
        // never a candidate).
        let range_start = (i as f64 * bucket_size) as usize + 1;
        let range_end = (((i + 1) as f64 * bucket_size) as usize + 1).min(n);

        // The NEXT bucket's own range, averaged into a centroid "next
        // anchor" — never a specific point, per the algorithm.
        let next_start = (((i + 1) as f64) * bucket_size) as usize + 1;
        let next_end = (((i + 2) as f64) * bucket_size) as usize + 1;
        let next_end = next_end.min(n);
        let next_anchor = if next_start < next_end {
            let (sum_x, sum_y) = points[next_start..next_end].iter().fold((0.0, 0.0), |(sx, sy), &(x, y)| (sx + x, sy + y));
            let len = (next_end - next_start) as f64;
            (sum_x / len, sum_y / len)
        } else {
            // Degenerate last bucket (integer rounding pushed it empty)
            // — the true last point is the only sane centroid.
            points[n - 1]
        };

        let (anchor_x, anchor_y) = points[anchor_index];
        let mut best_area = -1.0_f64;
        let mut best_index = range_start.min(n - 1);
        for j in range_start..range_end {
            let (px, py) = points[j];
            // Shoelace/cross-product triangle area (unsigned; the
            // constant 1/2 factor doesn't affect which candidate is the
            // max, so it's dropped).
            let area = ((anchor_x - next_anchor.0) * (py - anchor_y) - (anchor_x - px) * (next_anchor.1 - anchor_y)).abs();
            if area > best_area {
                best_area = area;
                best_index = j;
            }
        }

        sampled.push(points[best_index]);
        anchor_index = best_index;
    }

    sampled.push(points[n - 1]);
    sampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_at_or_above_len_is_identity() {
        let points: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, (i as f64).sin())).collect();
        assert_eq!(lttb(&points, 10), points);
        assert_eq!(lttb(&points, 50), points);
    }

    #[test]
    fn empty_and_degenerate_thresholds_never_panic() {
        assert_eq!(lttb(&[], 10), Vec::<(f64, f64)>::new());
        let points = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)];
        assert_eq!(lttb(&points, 0), Vec::<(f64, f64)>::new());
        assert_eq!(lttb(&points, 1), vec![(0.0, 0.0)]);
        assert_eq!(lttb(&points, 2), vec![(0.0, 0.0), (2.0, 4.0)]);
    }

    #[test]
    fn keeps_first_and_last_point_verbatim() {
        let points: Vec<(f64, f64)> = (0..1000).map(|i| (i as f64, ((i as f64) * 0.01).sin() * 10.0)).collect();
        let out = lttb(&points, 50);
        assert_eq!(out.len(), 50);
        assert_eq!(out[0], points[0]);
        assert_eq!(out[out.len() - 1], points[points.len() - 1]);
    }

    /// A synthetic spike (one point far outside the smooth baseline)
    /// must survive a 1000 -> 50 downsample — the whole point of
    /// maximizing triangle area over naive decimation/averaging, which
    /// would flatten it.
    #[test]
    fn a_synthetic_spike_survives_1000_to_50_downsample() {
        const N: usize = 1000;
        const SPIKE_INDEX: usize = 417; // interior, not first/last
        const SPIKE_Y: f64 = 500.0;
        let points: Vec<(f64, f64)> = (0..N)
            .map(|i| {
                let baseline = ((i as f64) * 0.02).sin() * 5.0;
                if i == SPIKE_INDEX { (i as f64, SPIKE_Y) } else { (i as f64, baseline) }
            })
            .collect();

        let out = lttb(&points, 50);
        assert_eq!(out.len(), 50);
        assert!(
            out.iter().any(|&(x, y)| x == SPIKE_INDEX as f64 && y == SPIKE_Y),
            "the spike point must be one of the 50 selected points, got: {out:?}"
        );
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let points: Vec<(f64, f64)> = (0..733).map(|i| (i as f64, ((i as f64) * 0.037).cos() * 3.0 + (i as f64 % 17.0))).collect();
        let a = lttb(&points, 40);
        let b = lttb(&points, 40);
        assert_eq!(a, b);
    }

    #[test]
    fn output_length_matches_threshold_when_downsampling() {
        let points: Vec<(f64, f64)> = (0..200).map(|i| (i as f64, i as f64 * 0.5)).collect();
        for threshold in [3usize, 10, 51, 100] {
            let out = lttb(&points, threshold);
            assert_eq!(out.len(), threshold, "threshold={threshold}");
            assert_eq!(out[0], points[0]);
            assert_eq!(*out.last().unwrap(), *points.last().unwrap());
        }
    }
}
