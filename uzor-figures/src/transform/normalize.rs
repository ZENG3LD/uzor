//! Normalization — [`normalize`] (min-max to `[0, 1]`), [`z_score`]
//! (standard score), [`percent_of_total`] (share of a series' own sum).

use super::{apply_policy, kahan_sum, mean_kahan, MissingDataPolicy, TransformError};

/// Min-max normalize `values` into `[0, 1]`: `(v - min) / (max - min)`.
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - **Property**: the minimum input maps to exactly `0.0`, the maximum
///   maps to exactly `1.0` (see this function's own test) — modulo the
///   degenerate all-equal case below, where min == max and there is no
///   meaningful `0`/`1` extreme to hit.
/// - **All-equal input** (including a single value) -> every output is
///   `0.5` — a zero-width range has no meaningful position within
///   itself; `0.5` is the least-biased placement (neither falsely claims
///   "this is the minimum" nor "this is the maximum"), and avoids the
///   `NaN` an actual `0.0 / 0.0` division would otherwise produce.
/// - [`MissingDataPolicy::Propagate`] with any non-finite value present
///   -> the min/max extent itself becomes non-finite, so EVERY output is
///   `NaN` (a single poisoned extreme poisons the whole normalization,
///   not just the offending value's own position — unlike a windowed
///   function, `normalize` has no "window" to contain the damage to).
pub fn normalize(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    let working = apply_policy(values, missing)?;
    if working.is_empty() {
        return Ok(Vec::new());
    }
    if super::contains_nan(&working) {
        return Ok(vec![f64::NAN; working.len()]);
    }
    let (min, max) = working.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
    let span = max - min;
    if span == 0.0 {
        return Ok(vec![0.5; working.len()]);
    }
    Ok(working.iter().map(|&v| (v - min) / span).collect())
}

/// Standard score (`(v - mean) / population_stddev`) — POPULATION
/// standard deviation (divide by `n`, not `n - 1`), matching
/// [`crate::figure::histogram::BinPolicy::Scott`]'s own already-existing
/// population-variance convention elsewhere in this crate (consistency
/// over the sample/population choice, since this crate has no prior
/// convention favoring one over the other beyond that single precedent).
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - **All-equal input** (population stddev `== 0.0`) -> every output is
///   `0.0` (every sample sits exactly at the mean; avoids the `NaN` a
///   real `0.0 / 0.0` would produce).
/// - [`MissingDataPolicy::Propagate`] with any non-finite value present
///   -> the mean/stddev themselves become non-finite, so every output is
///   `NaN` (same whole-series-poisoning reasoning as [`normalize`]).
pub fn z_score(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    let working = apply_policy(values, missing)?;
    if working.is_empty() {
        return Ok(Vec::new());
    }
    if super::contains_nan(&working) {
        return Ok(vec![f64::NAN; working.len()]);
    }
    let mean = mean_kahan(&working);
    let variance = working.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / working.len() as f64;
    let stddev = variance.sqrt();
    if stddev == 0.0 {
        return Ok(vec![0.0; working.len()]);
    }
    Ok(working.iter().map(|&v| (v - mean) / stddev).collect())
}

/// Each value's own share of `values`' own TOTAL — `v / sum(values)`, via
/// [`kahan_sum`] (this module's own stable summation).
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - **A total of exactly `0.0`** (e.g. `[5.0, -5.0]`, or all-zero data)
///   -> every output is `NaN` — a genuine, undefined `0 / 0` (what is
///   "5's share of a 0 total"?), the SAME documented convention
///   [`crate::figure::KpiFigure::delta_pct`] already established in this
///   crate for "a percent change against a zero baseline is genuinely
///   undefined" — not a fabricated `0.0` that would misleadingly read as
///   "this value is 0% of the total."
/// - [`MissingDataPolicy::Propagate`] with any non-finite value present
///   -> the total itself becomes non-finite, so every output is `NaN`.
pub fn percent_of_total(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    let working = apply_policy(values, missing)?;
    if working.is_empty() {
        return Ok(Vec::new());
    }
    if super::contains_nan(&working) {
        return Ok(vec![f64::NAN; working.len()]);
    }
    let total = kahan_sum(&working);
    if total == 0.0 {
        return Ok(vec![f64::NAN; working.len()]);
    }
    Ok(working.iter().map(|&v| v / total).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize ────────────────────────────────────────────────────

    #[test]
    fn normalize_maps_min_to_zero_and_max_to_one_property() {
        let values = [30.0, 10.0, 50.0, 20.0, 40.0];
        let out = normalize(&values, MissingDataPolicy::Skip).unwrap();
        let min_index = values.iter().enumerate().min_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        let max_index = values.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        assert_eq!(out[min_index], 0.0);
        assert_eq!(out[max_index], 1.0);
        assert!(out.iter().all(|&v| (0.0..=1.0).contains(&v)));
    }

    #[test]
    fn normalize_matches_hand_computed_values() {
        let out = normalize(&[0.0, 5.0, 10.0], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn normalize_empty_is_empty() {
        assert!(normalize(&[], MissingDataPolicy::Skip).unwrap().is_empty());
    }

    #[test]
    fn normalize_all_equal_maps_to_the_midpoint_never_panics() {
        let out = normalize(&[7.0; 5], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![0.5; 5]);
    }

    #[test]
    fn normalize_single_value_maps_to_the_midpoint() {
        assert_eq!(normalize(&[42.0], MissingDataPolicy::Skip).unwrap(), vec![0.5]);
    }

    #[test]
    fn normalize_propagate_with_a_nan_poisons_every_output() {
        let out = normalize(&[1.0, f64::NAN, 3.0], MissingDataPolicy::Propagate).unwrap();
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn normalize_skip_drops_non_finite_before_computing_extent() {
        let out = normalize(&[0.0, f64::NAN, 10.0], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![0.0, 1.0]);
    }

    #[test]
    fn normalize_error_rejects_up_front() {
        let err = normalize(&[1.0, f64::NAN], MissingDataPolicy::Error).unwrap_err();
        assert_eq!(err, TransformError::NonFinite { index: 1 });
    }

    // ── z_score ──────────────────────────────────────────────────────

    #[test]
    fn z_score_mean_is_always_zero_and_symmetric_values_cancel() {
        let values = [10.0, 20.0, 30.0, 40.0, 50.0];
        let out = z_score(&values, MissingDataPolicy::Skip).unwrap();
        // Symmetric around the mean (30.0) -> z-scores are symmetric around 0.
        assert!((out[0] + out[4]).abs() < 1e-9);
        assert!((out[1] + out[3]).abs() < 1e-9);
        assert!(out[2].abs() < 1e-9, "the mean itself must z-score to 0");
    }

    #[test]
    fn z_score_empty_is_empty() {
        assert!(z_score(&[], MissingDataPolicy::Skip).unwrap().is_empty());
    }

    #[test]
    fn z_score_all_equal_is_zero_never_nan() {
        let out = z_score(&[3.0; 6], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![0.0; 6]);
    }

    #[test]
    fn z_score_propagate_with_a_nan_poisons_every_output() {
        let out = z_score(&[1.0, f64::NAN, 3.0], MissingDataPolicy::Propagate).unwrap();
        assert!(out.iter().all(|v| v.is_nan()));
    }

    // ── percent_of_total ─────────────────────────────────────────────

    #[test]
    fn percent_of_total_sums_to_one() {
        let values = [10.0, 20.0, 30.0, 40.0];
        let out = percent_of_total(&values, MissingDataPolicy::Skip).unwrap();
        let sum: f64 = out.iter().sum();
        assert!((sum - 1.0).abs() < 1e-9);
        assert!((out[0] - 0.1).abs() < 1e-9);
        assert!((out[3] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn percent_of_total_empty_is_empty() {
        assert!(percent_of_total(&[], MissingDataPolicy::Skip).unwrap().is_empty());
    }

    #[test]
    fn percent_of_total_a_zero_total_is_nan_not_a_panic() {
        let out = percent_of_total(&[5.0, -5.0], MissingDataPolicy::Skip).unwrap();
        assert!(out.iter().all(|v| v.is_nan()));
    }

    #[test]
    fn percent_of_total_propagate_with_a_nan_poisons_every_output() {
        let out = percent_of_total(&[1.0, f64::NAN, 3.0], MissingDataPolicy::Propagate).unwrap();
        assert!(out.iter().all(|v| v.is_nan()));
    }
}
