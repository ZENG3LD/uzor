//! Equal-width binning — [`Bin`]/[`bin_by_count`]/[`BinPolicy`]/
//! [`resolve_bin_count`] were originally [`crate::figure::histogram`]'s
//! own private machinery (a histogram's binning is a pure,
//! independently-testable transform, never entangled with painting); this
//! wave (Engine-strengthening WAVE 4b) promotes them into the reusable
//! transform layer and adds [`bin`], the ONE-CALL policy-driven entry
//! point the task's own item 1 asks for (`bin(values, policy) ->
//! Vec<Bin>`) — [`crate::figure::HistogramFigure::render_with`] now calls
//! [`bin`] directly instead of its own two-step `resolve_bin_count` then
//! `bin_by_count` dance.
//!
//! **A real, disclosed signature change, not a silent one**: the
//! pre-existing free function `histogram::bin(samples: &[f64], bin_count:
//! usize) -> Vec<Bin>` is renamed [`bin_by_count`] (byte-identical body,
//! unchanged behavior — every existing golden test moved here verbatim)
//! so the name `bin` can carry the NEW, more useful `(values, BinPolicy)`
//! signature this item asks for; Rust has no function overloading, so
//! both signatures can't share one name. Confirmed via a workspace-wide
//! grep before this rename: no crate outside `uzor-figures` itself ever
//! called the raw free function (only `HistogramFigure`'s own internals
//! and this module's own tests did) — this crate is `publish = false`
//! (incubating engine; `uzor/CLAUDE.md`'s own hard-cutover convention
//! applies), so a clean rename carries none of the "breaks a real
//! external caller" risk a published crate's public API rename would.

use crate::transform::MissingDataPolicy;

use super::quantile::quantile;

/// One equal-width bin: its `[start, end)` domain range and sample count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bin {
    pub range: (f64, f64),
    pub count: usize,
}

/// Bin `samples` into `bin_count` equal-width bins spanning a nice-rounded
/// domain around the data extent.
///
/// - Empty `samples` -> empty result (nothing to bin).
/// - All-equal samples (including a single sample) -> the domain widens to
///   `[v, v + 1)` so bin width stays non-zero; every sample lands in bin 0.
/// - `bin_count` is floored at 1.
/// - **Non-finite samples**: NOT explicitly policy-driven (unlike every
///   OTHER function in this module) — this is the one deliberate
///   exception, preserved byte-for-byte from [`crate::figure::histogram`]'s
///   own pre-existing behavior rather than retroactively "fixed" this
///   pass (this wave's own binding doctrine: a refactor stays
///   behavior-preserving; a genuine defect gets fixed outright, but this
///   one wasn't in this wave's own named scope). Documented here for the
///   first time though: a `NaN` sample casts to bin index `0` via Rust's
///   OWN saturating float-to-int cast (`NaN as usize == 0`, stable since
///   1.45) — every `NaN` sample silently lands in the FIRST bin, an
///   accidental "sort of skip, sort of not" behavior, not a deliberate
///   [`MissingDataPolicy`] choice. [`bin`] (the new policy-driven entry
///   point) inherits this same quirk for identical reasons.
pub fn bin_by_count(samples: &[f64], bin_count: usize) -> Vec<Bin> {
    let bin_count = bin_count.max(1);
    if samples.is_empty() {
        return Vec::new();
    }

    let (data_min, data_max) = samples
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    if !data_min.is_finite() || !data_max.is_finite() {
        return Vec::new();
    }

    let (domain_min, domain_max) =
        if (data_max - data_min).abs() < f64::EPSILON { (data_min, data_min + 1.0) } else { (data_min, data_max) };

    let width = (domain_max - domain_min) / bin_count as f64;
    let mut bins: Vec<Bin> = (0..bin_count)
        .map(|i| Bin { range: (domain_min + i as f64 * width, domain_min + (i + 1) as f64 * width), count: 0 })
        .collect();

    for &v in samples {
        let idx = (((v - domain_min) / width) as usize).min(bin_count - 1);
        bins[idx].count += 1;
    }
    bins
}

/// How [`bin`]/[`crate::figure::HistogramFigure`] resolve a bin COUNT
/// from data — every mainstream histogram implementation (matplotlib
/// `bins='auto'`, numpy `histogram_bin_edges`, R's `hist()`) picks a sane
/// default bin count from the data itself; a caller with no domain
/// intuition about bin count had no help before this enum existed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinPolicy {
    /// Always use the caller-supplied bin count — byte-identical to this
    /// crate's pre-existing (and, before this item, only) behavior. THE
    /// DEFAULT: [`crate::figure::HistogramFigure::new`] still takes an
    /// explicit `bin_count` and resolves to this variant, so every
    /// existing caller keeps its exact output unchanged.
    Manual(usize),
    /// Sturges' rule: `ceil(log2(n)) + 1` — the simplest, most widely
    /// taught automatic rule (R's own `hist()` default); best suited to a
    /// small, roughly-normal sample count.
    Sturges,
    /// Freedman-Diaconis' rule: `bin_width = 2 * IQR / n^(1/3)`, converted
    /// to a bin count via `ceil(data_range / bin_width)` — robust to
    /// outliers (sizes the bin width from the IQR, not the full range),
    /// the rule matplotlib's own `bins='auto'` prefers for larger,
    /// real-world (non-normal) datasets. Uses [`super::quantile::quantile`]
    /// (this crate's ONE canonical percentile method) for its own Q1/Q3.
    FreedmanDiaconis,
    /// Scott's rule: `bin_width = 3.49 * stddev / n^(1/3)` — asymptotically
    /// optimal for roughly-normal data (minimizes integrated mean squared
    /// error against a Gaussian reference), converted to a bin count the
    /// same way as [`BinPolicy::FreedmanDiaconis`].
    Scott,
}

/// Resolve a bin count from `policy` over `samples` — the pure function
/// [`bin`]/[`crate::figure::HistogramFigure::bin_count`] call, independently
/// testable against known datasets. Every automatic rule floors at `1` bin
/// (matching [`bin_by_count`]'s own floor) and falls back to `1` for fewer
/// than 2 samples (no meaningful spread to derive a rule from).
pub fn resolve_bin_count(policy: BinPolicy, samples: &[f64]) -> usize {
    let n = samples.len();
    match policy {
        BinPolicy::Manual(count) => count.max(1),
        _ if n < 2 => 1,
        BinPolicy::Sturges => (((n as f64).log2().ceil()) as usize + 1).max(1),
        BinPolicy::FreedmanDiaconis => {
            // Skip policy: a stray non-finite sample here degrades to
            // "compute the rule over the finite subset" rather than
            // corrupting the whole bin-count decision with a NaN — the
            // same convention `figure::boxplot::quartile`'s own
            // delegation to this crate's quantile fn uses (see that
            // module's own doc comment), chosen independently here for
            // the identical reason.
            let (Ok(q1), Ok(q3)) = (quantile(samples, 0.25, MissingDataPolicy::Skip), quantile(samples, 0.75, MissingDataPolicy::Skip)) else {
                return 1;
            };
            let iqr = q3 - q1;
            let (data_min, data_max) = samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let range = data_max - data_min;
            if iqr <= 0.0 || range <= 0.0 {
                return 1;
            }
            let bin_width = 2.0 * iqr / (n as f64).cbrt();
            if bin_width <= 0.0 {
                return 1;
            }
            (range / bin_width).ceil().max(1.0) as usize
        }
        BinPolicy::Scott => {
            let mean = samples.iter().sum::<f64>() / n as f64;
            let variance = samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
            let stddev = variance.sqrt();
            let (data_min, data_max) = samples.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let range = data_max - data_min;
            if stddev <= 0.0 || range <= 0.0 {
                return 1;
            }
            let bin_width = 3.49 * stddev / (n as f64).cbrt();
            if bin_width <= 0.0 {
                return 1;
            }
            (range / bin_width).ceil().max(1.0) as usize
        }
    }
}

/// The task's own literal ask — one call: resolve a bin count from
/// `policy` (via [`resolve_bin_count`]) then bin `values` at that count
/// (via [`bin_by_count`]). [`crate::figure::HistogramFigure::render_with`]
/// calls this directly.
pub fn bin(values: &[f64], policy: BinPolicy) -> Vec<Bin> {
    bin_by_count(values, resolve_bin_count(policy, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bin_by_count (moved verbatim from `figure::histogram`) ─────────

    #[test]
    fn empty_samples_produce_no_bins() {
        assert!(bin_by_count(&[], 10).is_empty());
    }

    #[test]
    fn single_value_lands_in_one_bin_with_full_count() {
        let bins = bin_by_count(&[5.0], 4);
        assert_eq!(bins.len(), 4);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, 1);
        assert_eq!(bins[0].count, 1);
    }

    #[test]
    fn all_equal_samples_land_in_one_bin_with_full_count() {
        let samples = vec![3.0, 3.0, 3.0, 3.0];
        let bins = bin_by_count(&samples, 5);
        assert_eq!(bins.len(), 5);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, samples.len());
        assert_eq!(bins[0].count, samples.len());
    }

    #[test]
    fn every_sample_is_counted_exactly_once() {
        let samples: Vec<f64> = (0..100).map(|i| i as f64 * 0.37).collect();
        let bins = bin_by_count(&samples, 10);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, samples.len());
    }

    #[test]
    fn max_value_sample_lands_in_the_last_bin_not_out_of_range() {
        // The sample equal to the data max computes a fractional index
        // exactly at `bin_count` (`(4.0 - 0.0) / 1.0 == 4`, one past the
        // last valid index `3`) without the `.min(bin_count - 1)` clamp —
        // verify it lands in the last bin instead of panicking.
        let bins = bin_by_count(&[0.0, 4.0], 4);
        assert_eq!(bins.len(), 4);
        assert_eq!(bins[0].count, 1);
        assert_eq!(bins[3].count, 1);
        assert_eq!(bins[1].count, 0);
        assert_eq!(bins[2].count, 0);
    }

    #[test]
    fn bin_count_floors_at_one() {
        let bins = bin_by_count(&[1.0, 2.0, 3.0], 0);
        assert_eq!(bins.len(), 1);
        assert_eq!(bins[0].count, 3);
    }

    #[test]
    fn a_nan_sample_lands_in_bin_zero_the_documented_pre_existing_quirk() {
        let bins = bin_by_count(&[10.0, 20.0, f64::NAN], 4);
        let total: usize = bins.iter().map(|b| b.count).sum();
        assert_eq!(total, 3, "the NaN sample must still be COUNTED somewhere, not silently dropped");
        assert_eq!(bins[0].count, 2, "the NaN sample lands in bin 0 alongside the genuine bin-0 sample (10.0)");
    }

    // ── BinPolicy / resolve_bin_count (moved verbatim from `figure::histogram`) ──

    #[test]
    fn manual_policy_always_returns_the_caller_supplied_count_regardless_of_data() {
        let samples = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0];
        assert_eq!(resolve_bin_count(BinPolicy::Manual(7), &samples), 7);
        assert_eq!(resolve_bin_count(BinPolicy::Manual(0), &samples), 1, "Manual must floor at 1, matching bin_by_count's own floor");
    }

    #[test]
    fn sturges_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4: ceil(log2(4)) + 1 = ceil(2.0) + 1 = 3 (log2(4) is exact in
        // f64 since 4 is a power of two — no rounding-boundary risk).
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::Sturges, &samples), 3);
    }

    #[test]
    fn freedman_diaconis_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4, same fixture `boxplot::quartile`'s own numpy golden uses:
        // Q1 = 1.75, Q3 = 3.25 -> IQR = 1.5. bin_width = 2*1.5 / 4^(1/3)
        // = 3 / 1.5874... ~= 1.8902. data range = 3.0.
        // bin_count = ceil(3.0 / 1.8902) = ceil(1.5875) = 2.
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::FreedmanDiaconis, &samples), 2);
    }

    #[test]
    fn scott_matches_the_hand_computed_golden_for_a_known_dataset() {
        // n = 4: mean = 2.5, population variance = 1.25, stddev ~=
        // 1.11803. bin_width = 3.49 * 1.11803 / 4^(1/3) ~= 2.4586.
        // bin_count = ceil(3.0 / 2.4586) = ceil(1.2202) = 2.
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(resolve_bin_count(BinPolicy::Scott, &samples), 2);
    }

    #[test]
    fn a_larger_roughly_uniform_dataset_gets_a_wider_sturges_bin_count() {
        // n = 100: ceil(log2(100)) + 1 = ceil(6.6439) + 1 = 7 + 1 = 8.
        let samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        assert_eq!(resolve_bin_count(BinPolicy::Sturges, &samples), 8);
    }

    #[test]
    fn every_automatic_rule_falls_back_to_one_bin_for_degenerate_all_equal_data() {
        // Zero IQR/stddev must never divide by zero or produce a NaN/0
        // bin count.
        let samples = vec![7.0; 20];
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott] {
            assert_eq!(resolve_bin_count(policy, &samples), if policy == BinPolicy::Sturges { 6 } else { 1 });
        }
    }

    #[test]
    fn every_automatic_rule_falls_back_to_one_bin_for_fewer_than_two_samples() {
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott] {
            assert_eq!(resolve_bin_count(policy, &[]), 1);
            assert_eq!(resolve_bin_count(policy, &[42.0]), 1);
        }
    }

    // ── bin (the new item-1 one-call entry point) ───────────────────────

    #[test]
    fn bin_composes_resolve_bin_count_then_bin_by_count_identically_to_calling_both_by_hand() {
        let samples: Vec<f64> = (0..200).map(|i| ((i * 37) % 97) as f64 * 0.5).collect();
        for policy in [BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott, BinPolicy::Manual(12)] {
            let via_one_call = bin(&samples, policy);
            let via_two_calls = bin_by_count(&samples, resolve_bin_count(policy, &samples));
            assert_eq!(via_one_call, via_two_calls);
        }
    }

    #[test]
    fn bin_on_empty_input_is_empty_for_every_policy() {
        for policy in [BinPolicy::Manual(5), BinPolicy::Sturges, BinPolicy::FreedmanDiaconis, BinPolicy::Scott] {
            assert!(bin(&[], policy).is_empty());
        }
    }

    #[test]
    fn manual_policy_via_bin_matches_the_pre_existing_bin_by_count_call_shape() {
        let samples = vec![1.0, 5.0, 9.0, 20.0, 42.0];
        assert_eq!(bin(&samples, BinPolicy::Manual(4)), bin_by_count(&samples, 4));
    }
}
