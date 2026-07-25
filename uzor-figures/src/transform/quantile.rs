//! [`quantile`] — the ONE canonical percentile implementation for this
//! crate: linear-interpolation percentile, numpy's default `'linear'`
//! method (R calls the same thing `'type 7'`). Sort the (policy-filtered)
//! samples, then linearly interpolate between the two bracketing order
//! statistics at fractional rank `q * (n - 1)`.
//!
//! Two consumers already implemented this exact method independently
//! before this wave — [`crate::figure::boxplot::quartile`] and
//! [`crate::scale::bin::QuantileScale`] (that module's own doc comment
//! explicitly notes it "reimplemented locally... to keep `scale` beneath
//! `figure` in this crate's own layering," the same one-directional-
//! dependency discipline this module now follows too: `transform` sits
//! BENEATH `figure` — [`crate::figure::CurveFigure::with_downsample`]
//! already depends on `transform::lttb`, establishing that direction —
//! so `figure::boxplot::quartile` can now depend on THIS function
//! (a lower layer), and has been refactored to do so; `scale::bin`'s own
//! copy was left untouched — this wave's task scope names only the
//! histogram-binning and bars-stacking refactors, not `scale::bin`).
//!
//! Chosen specifically for its unambiguous small-`n` behavior (over
//! Tukey's original median-of-halves hinge method): `n == 1` collapses
//! every quantile to the single sample; `n == 2`/`3` interpolate between
//! real order statistics; never an undefined "median of an empty half."

use super::{apply_policy, MissingDataPolicy, TransformError};

/// Linear-interpolation percentile at rank `q` (clamped to `[0, 1]`) over
/// `values`, after applying `missing` (see [`super::MissingDataPolicy`]).
///
/// - **Empty** `values`, or every value non-finite under
///   [`MissingDataPolicy::Skip`] (nothing left to compute over) ->
///   [`TransformError::EmptyInput`].
/// - **A single finite value** -> that value, for any `q` (no
///   interpolation possible or needed).
/// - **[`MissingDataPolicy::Propagate`] with any `NaN` present** -> `Ok(NaN)`
///   without attempting to sort (a `NaN` has no total order — sorting
///   around it would produce an arbitrary, non-reproducible result;
///   reporting `NaN` directly is the honest "this aggregate is poisoned"
///   answer this crate's own missing-data doctrine calls for). A
///   `+-Infinity` present under `Propagate` sorts and interpolates
///   normally (only `NaN` breaks total order, not infinity).
/// - **[`MissingDataPolicy::Error`] with a non-finite value present** ->
///   [`TransformError::NonFinite`] at that value's own index in `values`
///   (checked before any computation).
pub fn quantile(values: &[f64], q: f64, missing: MissingDataPolicy) -> Result<f64, TransformError> {
    let working = apply_policy(values, missing)?;
    if working.is_empty() {
        return Err(TransformError::EmptyInput);
    }
    if super::contains_nan(&working) {
        return Ok(f64::NAN);
    }
    let mut sorted = working;
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok(percentile_linear(&sorted, q))
}

/// The interpolation itself, over an ALREADY-sorted, ALREADY-finite
/// slice — `sorted.is_empty()` is never called by [`quantile`] (guarded
/// above), so this stays a private, non-fallible helper.
pub(crate) fn percentile_linear(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = q.clamp(0.0, 1.0) * (n - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        sorted[lo] * (1.0 - frac) + sorted[hi] * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_numpy_linear_method_golden() {
        // numpy.percentile([1,2,3,4], [25,50,75], method="linear") ==
        // [1.75, 2.5, 3.25] — the same golden `figure::boxplot::quartile`'s
        // own test uses.
        assert!((quantile(&[1.0, 2.0, 3.0, 4.0], 0.25, MissingDataPolicy::Skip).unwrap() - 1.75).abs() < 1e-9);
        assert!((quantile(&[1.0, 2.0, 3.0, 4.0], 0.5, MissingDataPolicy::Skip).unwrap() - 2.5).abs() < 1e-9);
        assert!((quantile(&[1.0, 2.0, 3.0, 4.0], 0.75, MissingDataPolicy::Skip).unwrap() - 3.25).abs() < 1e-9);
    }

    #[test]
    fn n_equals_1_collapses_to_the_single_sample_for_any_q() {
        for q in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(quantile(&[42.0], q, MissingDataPolicy::Skip).unwrap(), 42.0);
        }
    }

    #[test]
    fn n_equals_2_interpolates_between_the_two_real_samples() {
        assert!((quantile(&[1.0, 2.0], 0.25, MissingDataPolicy::Skip).unwrap() - 1.25).abs() < 1e-9);
        assert!((quantile(&[1.0, 2.0], 0.5, MissingDataPolicy::Skip).unwrap() - 1.5).abs() < 1e-9);
    }

    #[test]
    fn is_order_independent_sorts_internally() {
        let a = quantile(&[4.0, 1.0, 3.0, 2.0], 0.5, MissingDataPolicy::Skip).unwrap();
        let b = quantile(&[1.0, 2.0, 3.0, 4.0], 0.5, MissingDataPolicy::Skip).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn q_is_clamped_to_0_1() {
        let values = [1.0, 2.0, 3.0];
        assert_eq!(quantile(&values, -1.0, MissingDataPolicy::Skip).unwrap(), quantile(&values, 0.0, MissingDataPolicy::Skip).unwrap());
        assert_eq!(quantile(&values, 5.0, MissingDataPolicy::Skip).unwrap(), quantile(&values, 1.0, MissingDataPolicy::Skip).unwrap());
    }

    #[test]
    fn empty_input_is_an_explicit_error_never_a_panic() {
        assert_eq!(quantile(&[], 0.5, MissingDataPolicy::Skip), Err(TransformError::EmptyInput));
        assert_eq!(quantile(&[], 0.5, MissingDataPolicy::Propagate), Err(TransformError::EmptyInput));
    }

    #[test]
    fn skip_policy_filters_non_finite_values_before_computing() {
        let with_nan = quantile(&[1.0, f64::NAN, 2.0, 3.0, 4.0], 0.5, MissingDataPolicy::Skip).unwrap();
        let clean = quantile(&[1.0, 2.0, 3.0, 4.0], 0.5, MissingDataPolicy::Skip).unwrap();
        assert_eq!(with_nan, clean);
    }

    #[test]
    fn skip_policy_all_non_finite_is_empty_input_error() {
        assert_eq!(quantile(&[f64::NAN, f64::NAN], 0.5, MissingDataPolicy::Skip), Err(TransformError::EmptyInput));
    }

    #[test]
    fn propagate_policy_with_nan_present_returns_nan_without_panicking() {
        let result = quantile(&[1.0, f64::NAN, 3.0], 0.5, MissingDataPolicy::Propagate).unwrap();
        assert!(result.is_nan());
    }

    #[test]
    fn propagate_policy_with_only_infinity_still_interpolates_normally() {
        let result = quantile(&[1.0, f64::INFINITY, 3.0], 1.0, MissingDataPolicy::Propagate).unwrap();
        assert!(result.is_infinite() && result > 0.0);
    }

    #[test]
    fn error_policy_reports_the_offending_index() {
        let err = quantile(&[1.0, 2.0, f64::NAN], 0.5, MissingDataPolicy::Error).unwrap_err();
        assert_eq!(err, TransformError::NonFinite { index: 2 });
    }

    #[test]
    fn all_equal_samples_return_the_constant_value_for_every_q() {
        let values = [5.0; 10];
        for q in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(quantile(&values, q, MissingDataPolicy::Skip).unwrap(), 5.0);
        }
    }
}
