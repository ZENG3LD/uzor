//! Binning scales — map a CONTINUOUS value to a DISCRETE class, the
//! foundation for risk/class colouring and for a legend that reads as
//! classes ("low / medium / high") rather than a continuous ramp.
//!
//! Three flavors, matching d3's own `scaleQuantize`/`scaleThreshold`/
//! `scaleQuantile` (the standard three-way split every binning-scale
//! design in the field settles on):
//! - [`QuantizeScale`] — `n` UNIFORM-WIDTH bins over a continuous
//!   `[min, max]` domain (equal-interval classification).
//! - [`ThresholdScale`] — explicit, caller-supplied cut points (arbitrary
//!   business thresholds, e.g. risk-tier boundaries that don't follow any
//!   formula).
//! - [`QuantileScale`] — bins derived from the DATA's own distribution
//!   (equal-COUNT classification — each class holds roughly the same
//!   number of samples, not the same value width), via the same
//!   linear-interpolation percentile method [`crate::figure::boxplot::
//!   quartile`] documents (numpy `'linear'` / R `'type 7'`) — reimplemented
//!   locally (not called cross-module) to keep `scale` beneath `figure` in
//!   this crate's own layering (see `scale/mod.rs`'s own module doc: scale
//!   is the foundation other layers build on, not the reverse).
//!
//! ## Output type: a class INDEX, not a generic range value
//!
//! Every real consumer of a binning scale in this crate already resolves
//! "which of N things" by INDEXING into its own `Vec<T>` — a color
//! (`theme.palette[i % len]`, [`super::color::CategoricalScale::color_for`]),
//! a legend swatch, a discrete colorbar tier. A `ClassScale<T>` generic
//! over the range type would add a type parameter this trait's one real
//! use (color-by-class) never needs; a plain `usize` lets any caller
//! resolve a class into whatever range it likes with the SAME
//! `%`-cycled-index idiom this crate already uses everywhere for its
//! categorical palette. [`ClassScale::class_label`] additionally supplies
//! a human-readable bound description (`"10 – 20"`, `"< 5"`, `">= 90"`) so
//! a legend/colorbar caller never has to re-derive bin edges itself.

use super::linear::format_value;
use std::cmp::Ordering;

/// Value -> discrete CLASS mapping — the shared shape [`QuantizeScale`],
/// [`ThresholdScale`], and [`QuantileScale`] all implement. See this
/// module's own top-level doc comment for why the output is a class INDEX
/// rather than a generic range value.
pub trait ClassScale {
    /// Number of distinct classes this scale partitions its domain into
    /// (always `>= 1`, even for a degenerate/empty construction).
    fn class_count(&self) -> usize;

    /// Which class (`0..class_count()`) `v` falls into. Non-finite input
    /// resolves to class `0` (never panics, never returns an out-of-range
    /// index).
    fn class_index(&self, v: f64) -> usize;

    /// A human-readable bound description for class `index` (e.g.
    /// `"10 – 20"`, `"< 5"`, `">= 90"`) — the label a discrete legend/
    /// colorbar swatch shows. An out-of-range `index` degrades to an empty
    /// string rather than panicking.
    fn class_label(&self, index: usize) -> String;
}

/// Sensible display-precision step for a label spanning `[lo, hi]` — a
/// tenth of the span, guarded against a zero/degenerate span (falls back
/// to `1.0`, [`format_value`]'s own default precision).
fn label_step(lo: f64, hi: f64) -> f64 {
    let span = (hi - lo).abs();
    if span > f64::EPSILON {
        span / 10.0
    } else {
        1.0
    }
}

/// `"{lo} – {hi}"` label for a class whose bounds are both KNOWN (used by
/// [`QuantizeScale`]/[`QuantileScale`], which always have a real domain
/// extent) — `lo`/`hi` resolved from `thresholds[index - 1]`/
/// `thresholds[index]`, falling back to `domain_min`/`domain_max` at the
/// two open ends.
fn bounded_class_label(domain_min: f64, domain_max: f64, thresholds: &[f64], index: usize) -> String {
    let lo = if index == 0 { domain_min } else { *thresholds.get(index - 1).unwrap_or(&domain_min) };
    let hi = *thresholds.get(index).unwrap_or(&domain_max);
    let step = label_step(lo, hi);
    format!("{} – {}", format_value(lo, step), format_value(hi, step))
}

// ── QuantizeScale ────────────────────────────────────────────────────────

/// `n` UNIFORM-WIDTH bins over a continuous `[min, max]` domain.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizeScale {
    pub min: f64,
    pub max: f64,
    n: usize,
}

impl QuantizeScale {
    /// `n` is floored at `1` (a single class trivially contains the whole
    /// domain — never zero classes).
    pub fn new(min: f64, max: f64, n: usize) -> Self {
        Self { min, max, n: n.max(1) }
    }

    fn width(&self) -> f64 {
        let range = self.max - self.min;
        if range.abs() < f64::EPSILON || !range.is_finite() {
            0.0
        } else {
            range / self.n as f64
        }
    }

    /// The `n - 1` internal boundary values, ascending — empty for a
    /// degenerate (`min >= max`, non-finite) domain or `n == 1`.
    pub fn thresholds(&self) -> Vec<f64> {
        let w = self.width();
        if w <= 0.0 {
            return Vec::new();
        }
        (1..self.n).map(|i| self.min + w * i as f64).collect()
    }
}

impl ClassScale for QuantizeScale {
    fn class_count(&self) -> usize {
        self.n
    }

    fn class_index(&self, v: f64) -> usize {
        if !v.is_finite() || self.max <= self.min || !self.min.is_finite() || !self.max.is_finite() {
            return 0;
        }
        let w = self.width();
        if w <= 0.0 {
            return 0;
        }
        let idx = ((v - self.min) / w).floor();
        if idx < 0.0 {
            0
        } else {
            (idx as usize).min(self.n - 1)
        }
    }

    fn class_label(&self, index: usize) -> String {
        bounded_class_label(self.min, self.max, &self.thresholds(), index)
    }
}

// ── ThresholdScale ───────────────────────────────────────────────────────

/// Explicit, caller-supplied cut points — `n` thresholds partition the
/// (conceptually unbounded) real line into `n + 1` classes: `< t0`,
/// `[t0, t1)`, ..., `>= t(n-1)`.
#[derive(Debug, Clone)]
pub struct ThresholdScale {
    thresholds: Vec<f64>,
}

impl ThresholdScale {
    /// `thresholds` is defensively sorted ascending and deduplicated
    /// (within a tight float-equality tolerance) — a caller passing an
    /// unsorted or duplicate-laden list still gets a well-formed scale
    /// rather than silently-wrong class boundaries.
    pub fn new(thresholds: Vec<f64>) -> Self {
        let mut thresholds: Vec<f64> = thresholds.into_iter().filter(|v| v.is_finite()).collect();
        thresholds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        thresholds.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
        Self { thresholds }
    }

    /// The sorted, deduplicated cut points this scale was built from.
    pub fn thresholds(&self) -> &[f64] {
        &self.thresholds
    }
}

impl ClassScale for ThresholdScale {
    fn class_count(&self) -> usize {
        self.thresholds.len() + 1
    }

    fn class_index(&self, v: f64) -> usize {
        if !v.is_finite() {
            return 0;
        }
        self.thresholds.iter().filter(|&&t| v >= t).count()
    }

    fn class_label(&self, index: usize) -> String {
        let t = &self.thresholds;
        if t.is_empty() {
            return "all".to_owned();
        }
        let step = label_step(t[0], *t.last().unwrap_or(&t[0])).max(f64::EPSILON);
        if index == 0 {
            format!("< {}", format_value(t[0], step))
        } else if index >= t.len() {
            format!(">= {}", format_value(t[t.len() - 1], step))
        } else {
            format!("{} – {}", format_value(t[index - 1], step), format_value(t[index], step))
        }
    }
}

// ── QuantileScale ────────────────────────────────────────────────────────

/// Linear-interpolation percentile — the SAME method (numpy `'linear'` /
/// R `'type 7'`) [`crate::figure::boxplot::quartile`]'s own doc comment
/// documents choosing, for identical small-`n` reasoning (unambiguous at
/// `n == 1`/`2`/`3`, never an undefined "median of an empty half"). `p` is
/// clamped to `[0, 1]`; `sorted` must already be ascending. Not shared
/// cross-module with `figure::boxplot`'s own private copy — see this
/// module's own top-level doc comment for why.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 {
        return f64::NAN;
    }
    if n == 1 {
        return sorted[0];
    }
    let rank = p.clamp(0.0, 1.0) * (n - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

/// Bins derived from the DATA's own distribution — each of `n` classes
/// holds (as close as the percentile interpolation allows) an equal COUNT
/// of samples, not an equal value width.
#[derive(Debug, Clone)]
pub struct QuantileScale {
    breaks: Vec<f64>,
    data_min: f64,
    data_max: f64,
}

impl QuantileScale {
    /// `n` is floored at `1`. Non-finite samples are dropped before
    /// computing breakpoints. Empty (or all-non-finite) `data`, a SINGLE
    /// remaining sample, or `n <= 1`, produces a single trivial class
    /// (`breaks` empty) — `class_index` then always resolves to `0`, never
    /// panics, divides by an empty sorted slice, nor manufactures `n - 1`
    /// duplicate breakpoints all equal to one sample's own value.
    pub fn new(data: &[f64], n: usize) -> Self {
        let n = n.max(1);
        let mut sorted: Vec<f64> = data.iter().copied().filter(|v| v.is_finite()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        // Fewer than 2 samples can't support a real percentile INTERPOLATION
        // (a single sample is its own 0th..100th percentile) — collapse to
        // one trivial class the same way `n <= 1` already does, rather than
        // manufacturing `n - 1` duplicate breakpoints all equal to that one
        // value.
        if sorted.len() <= 1 || n <= 1 {
            let anchor = sorted.first().copied().unwrap_or(0.0);
            let top = sorted.last().copied().unwrap_or(anchor + 1.0);
            return Self { breaks: Vec::new(), data_min: anchor, data_max: top };
        }
        let breaks: Vec<f64> = (1..n).map(|i| percentile(&sorted, i as f64 / n as f64)).collect();
        Self { breaks, data_min: sorted[0], data_max: *sorted.last().unwrap_or(&sorted[0]) }
    }
}

impl ClassScale for QuantileScale {
    fn class_count(&self) -> usize {
        self.breaks.len() + 1
    }

    fn class_index(&self, v: f64) -> usize {
        if !v.is_finite() {
            return 0;
        }
        self.breaks.iter().filter(|&&b| v >= b).count()
    }

    fn class_label(&self, index: usize) -> String {
        bounded_class_label(self.data_min, self.data_max, &self.breaks, index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── QuantizeScale ────────────────────────────────────────────────

    #[test]
    fn quantize_uniform_bins_split_the_domain_into_equal_widths() {
        let scale = QuantizeScale::new(0.0, 100.0, 4);
        assert_eq!(scale.thresholds(), vec![25.0, 50.0, 75.0]);
    }

    #[test]
    fn quantize_class_index_at_boundaries_and_interior() {
        let scale = QuantizeScale::new(0.0, 100.0, 4);
        assert_eq!(scale.class_index(0.0), 0);
        assert_eq!(scale.class_index(24.9), 0);
        assert_eq!(scale.class_index(25.0), 1);
        assert_eq!(scale.class_index(49.9), 1);
        assert_eq!(scale.class_index(75.0), 3);
        // The top edge (== max) must clamp into the LAST class, not overflow.
        assert_eq!(scale.class_index(100.0), 3);
        assert_eq!(scale.class_index(1_000_000.0), 3);
    }

    #[test]
    fn quantize_negative_domain_classifies_correctly() {
        let scale = QuantizeScale::new(-100.0, 100.0, 2);
        assert_eq!(scale.class_index(-50.0), 0);
        assert_eq!(scale.class_index(50.0), 1);
        assert_eq!(scale.class_index(0.0), 1); // boundary rounds into the upper class
    }

    #[test]
    fn quantize_degenerate_domain_never_panics_and_stays_in_class_zero() {
        let scale = QuantizeScale::new(5.0, 5.0, 4);
        assert_eq!(scale.class_index(5.0), 0);
        assert_eq!(scale.class_count(), 4);
        assert!(scale.thresholds().is_empty());
        assert!(!scale.class_label(0).is_empty());
    }

    #[test]
    fn quantize_n_zero_floors_to_one_class() {
        let scale = QuantizeScale::new(0.0, 10.0, 0);
        assert_eq!(scale.class_count(), 1);
        assert_eq!(scale.class_index(5.0), 0);
    }

    #[test]
    fn quantize_class_label_reflects_bin_bounds() {
        let scale = QuantizeScale::new(0.0, 100.0, 4);
        assert!(scale.class_label(0).contains('0'));
        assert!(scale.class_label(3).contains("100"));
    }

    // ── ThresholdScale ───────────────────────────────────────────────

    #[test]
    fn threshold_explicit_cut_points_partition_into_n_plus_one_classes() {
        let scale = ThresholdScale::new(vec![10.0, 20.0, 30.0]);
        assert_eq!(scale.class_count(), 4);
        assert_eq!(scale.class_index(5.0), 0);
        assert_eq!(scale.class_index(10.0), 1);
        assert_eq!(scale.class_index(25.0), 2);
        assert_eq!(scale.class_index(30.0), 3);
        assert_eq!(scale.class_index(1000.0), 3);
    }

    #[test]
    fn threshold_defensively_sorts_and_dedups_unsorted_input() {
        let scale = ThresholdScale::new(vec![30.0, 10.0, 20.0, 10.0]);
        assert_eq!(scale.thresholds(), &[10.0, 20.0, 30.0]);
    }

    #[test]
    fn threshold_empty_thresholds_is_one_trivial_class() {
        let scale = ThresholdScale::new(Vec::new());
        assert_eq!(scale.class_count(), 1);
        assert_eq!(scale.class_index(42.0), 0);
        assert_eq!(scale.class_label(0), "all");
    }

    #[test]
    fn threshold_negative_and_mixed_sign_cut_points_classify_correctly() {
        let scale = ThresholdScale::new(vec![-10.0, 0.0, 10.0]);
        assert_eq!(scale.class_index(-20.0), 0);
        assert_eq!(scale.class_index(-5.0), 1);
        assert_eq!(scale.class_index(5.0), 2);
        assert_eq!(scale.class_index(15.0), 3);
    }

    #[test]
    fn threshold_labels_use_open_ended_bounds_at_the_extremes() {
        let scale = ThresholdScale::new(vec![10.0, 20.0]);
        assert!(scale.class_label(0).starts_with('<'));
        assert!(scale.class_label(2).starts_with(">="));
        assert!(scale.class_label(1).contains('–'));
    }

    // ── QuantileScale ────────────────────────────────────────────────

    #[test]
    fn quantile_breaks_match_the_linear_interpolation_percentile_method() {
        // Same golden fixture `figure::boxplot`'s own quartile test uses
        // for its own quartile method — here as 4-quantile (n=4) breaks,
        // which ARE the quartile boundaries (p=0.25/0.5/0.75).
        let scale = QuantileScale::new(&[1.0, 2.0, 3.0, 4.0], 4);
        assert_eq!(scale.class_count(), 4);
        let breaks = &scale.breaks;
        assert!((breaks[0] - 1.75).abs() < 1e-9);
        assert!((breaks[1] - 2.5).abs() < 1e-9);
        assert!((breaks[2] - 3.25).abs() < 1e-9);
    }

    #[test]
    fn quantile_empty_data_is_one_trivial_class_and_never_panics() {
        let scale = QuantileScale::new(&[], 5);
        assert_eq!(scale.class_count(), 1);
        assert_eq!(scale.class_index(42.0), 0);
        assert!(!scale.class_label(0).is_empty());
    }

    #[test]
    fn quantile_single_sample_is_one_trivial_class() {
        let scale = QuantileScale::new(&[7.0], 5);
        assert_eq!(scale.class_count(), 1);
        assert_eq!(scale.class_index(7.0), 0);
    }

    #[test]
    fn quantile_all_equal_samples_never_panics_and_is_deterministic() {
        let data = vec![5.0; 20];
        let scale = QuantileScale::new(&data, 4);
        // Every break collapses to the same constant — every value lands
        // in the SAME (last) class deterministically, not a panic/NaN.
        let idx = scale.class_index(5.0);
        for _ in 0..5 {
            assert_eq!(scale.class_index(5.0), idx);
        }
        assert!(idx < scale.class_count());
    }

    #[test]
    fn quantile_n_one_or_zero_is_one_trivial_class() {
        let data: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let scale = QuantileScale::new(&data, 1);
        assert_eq!(scale.class_count(), 1);
        let scale0 = QuantileScale::new(&data, 0);
        assert_eq!(scale0.class_count(), 1);
    }

    #[test]
    fn quantile_non_finite_samples_are_dropped_before_computing_breaks() {
        let data = vec![1.0, 2.0, f64::NAN, 3.0, 4.0, f64::INFINITY];
        let scale = QuantileScale::new(&data, 4);
        for b in &scale.breaks {
            assert!(b.is_finite());
        }
    }

    #[test]
    fn quantile_roughly_equalizes_class_membership_counts() {
        // 100 evenly spread samples into 4 quantile classes should land
        // close to 25 samples per class (equal-COUNT, not equal-width).
        let data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let scale = QuantileScale::new(&data, 4);
        let mut counts = [0usize; 4];
        for &v in &data {
            counts[scale.class_index(v)] += 1;
        }
        for count in counts {
            assert!((20..=30).contains(&count), "expected roughly equal class membership, got {counts:?}");
        }
    }
}
