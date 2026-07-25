//! Data-space transforms applied BEFORE a figure hands points to its own
//! mark/hit-test pipeline — [`lttb`] (downsampling), plus (Engine-
//! strengthening WAVE 4b) the reusable aggregation/statistics layer a
//! grammar-of-graphics stack needs between raw data and marks: binning
//! ([`bin`]), grouping/rollup ([`groupby`]), stacking ([`stack`]),
//! cumulative/running reductions ([`cumulative`]), rolling-window
//! smoothing ([`rolling`]), normalization ([`normalize`]), and quantiles
//! ([`quantile`]). Kept separate from `scale`/`coord` (screen-space
//! mapping) and `mark` (pure draw) since a transform here operates on
//! DOMAIN points, before any [`crate::coord::PlotArea`] involvement.
//!
//! Every function in this module is pure (a slice in, an owned `Vec`/
//! scalar out — no I/O, no hidden allocation beyond the one obvious
//! output buffer, no runtime deps) and independently unit-tested,
//! matching this crate's own zero-runtime-dependency contract
//! (`Cargo.toml` — the only dependency is `uzor` core itself).
//!
//! ## Missing-data policy — the ONE shared contract every function here follows
//!
//! [`MissingDataPolicy`] is the single, explicit, documented answer to
//! "what happens when a non-finite (`NaN`/`+-Infinity`) value shows up in
//! the input" — every function in this module takes one rather than
//! silently picking a behavior. This crate's own engine-strengthening arc
//! notes this stack was "bitten twice this session by NaN handling being
//! implicit" (histogram binning silently routing a `NaN` sample into bin
//! `0` via a saturating float-to-int cast; `BarFigure::stacked_segments_px`
//! silently routing a `NaN` series value into the NEGATIVE accumulator
//! because `NaN >= 0.0` is always `false`) — both are real, and both stay
//! EXACTLY as they were (see [`bin::bin_by_count`]/[`stack`]'s own doc
//! comments for why: preserving a REFACTOR's pre-existing behavior takes
//! priority over retroactively "fixing" an implicit default this pass
//! didn't introduce), but every NEW function this wave adds names its own
//! non-finite handling explicitly instead of repeating that mistake.
//!
//! - [`MissingDataPolicy::Skip`] — drop non-finite values before
//!   computing; the result reflects only the finite subset. Matches the
//!   convention [`crate::figure::boxplot::quartile`]/
//!   [`crate::scale::bin::QuantileScale`] already use for percentile
//!   computation.
//! - [`MissingDataPolicy::Propagate`] — non-finite values flow through
//!   ordinary IEEE-754 arithmetic. For a single-scalar aggregate (sum,
//!   mean, a whole-slice reduction) the WHOLE result becomes `NaN` the
//!   moment any input is non-finite (this module enforces that uniformly
//!   — including for `min`/`max`, where naive `f64::min`/`f64::max`
//!   would otherwise silently IGNORE a `NaN` operand per IEEE-754 minNum
//!   semantics; see [`apply_policy`]'s own callers). For a
//!   position-indexed/windowed function (a running sum, a rolling mean)
//!   only the outputs whose own window touches the non-finite value
//!   become `NaN` — a natural "poisons forward from its own index"
//!   result, documented on each such function individually.
//! - [`MissingDataPolicy::Error`] — reject the call outright:
//!   [`TransformError::NonFinite`] naming the offending value's own
//!   index, checked BEFORE any computation runs.
//!
//! Every function additionally defines empty-input and degenerate-input
//! (single element, all-equal, all-non-finite) behavior explicitly in its
//! own doc comment — never a panic, never silent garbage.

pub mod bin;
pub mod cumulative;
pub mod groupby;
pub mod lttb;
pub mod normalize;
pub mod quantile;
pub mod rolling;
pub mod stack;

pub use bin::{bin, bin_by_count, resolve_bin_count, Bin, BinPolicy};
pub use cumulative::{cumsum, running_max, running_min};
pub use groupby::{rollup, rollup_with, Reducer};
pub use lttb::lttb;
pub use normalize::{normalize, percent_of_total, z_score};
pub use quantile::quantile;
pub use rolling::{ema, rolling_mean, rolling_median, EdgePolicy};
pub use stack::{sort_index_by_value, stack, StackOffset, StackOrder};

/// How every function in this module treats a non-finite (`NaN`/
/// `+-Infinity`) input value — see this module's own top-level doc
/// comment ("Missing-data policy") for the full contract each variant
/// implies, and each consuming function's own doc comment for which
/// variant IT defaults to and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingDataPolicy {
    /// Drop non-finite values before computing.
    Skip,
    /// Let a non-finite value flow through ordinary arithmetic, poisoning
    /// whatever aggregate/window it touches with `NaN` rather than
    /// vanishing silently.
    Propagate,
    /// Reject the call: [`TransformError::NonFinite`].
    Error,
}

/// The one error type every fallible function in this module returns —
/// deliberately small (two variants): a non-finite input under
/// [`MissingDataPolicy::Error`], or an input that has no finite values
/// left to compute over at all (distinct from "produced `NaN`" — this is
/// "there was nothing to reduce"). Neither variant is ever raised except
/// under conditions each function's own doc comment names explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    /// A non-finite value was found at `index` while
    /// [`MissingDataPolicy::Error`] was active. `index` is a flat index
    /// into the function's own 1-D input slice, EXCEPT [`stack`], which
    /// documents its own `series_index * category_count + category_index`
    /// flattening on its own doc comment (a 2-D input has no single
    /// natural 1-D index).
    NonFinite { index: usize },
    /// The input had no finite values left to compute over — either the
    /// slice was empty to begin with, or every value was non-finite and
    /// [`MissingDataPolicy::Skip`] filtered all of them away. Distinct
    /// from a defined-but-degenerate result (e.g. `quantile` of a single
    /// finite sample, which is `Ok`) — this is "there was nothing at all
    /// to reduce."
    EmptyInput,
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformError::NonFinite { index } => write!(f, "non-finite value at index {index}"),
            TransformError::EmptyInput => write!(f, "no finite values to compute over"),
        }
    }
}

impl std::error::Error for TransformError {}

/// Apply `policy` to `values`, returning either the working set to
/// compute over (`Skip`: a finite-only copy; `Propagate`: unchanged,
/// non-finite values included verbatim) or `Err` (`Error`: the first
/// non-finite value's own index, before any computation runs). The one
/// shared pre-processing step every function in this module funnels
/// through — see this module's own top-level "Missing-data policy" doc
/// section for the full contract.
pub(crate) fn apply_policy(values: &[f64], policy: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    match policy {
        MissingDataPolicy::Skip => Ok(values.iter().copied().filter(|v| v.is_finite()).collect()),
        MissingDataPolicy::Propagate => Ok(values.to_vec()),
        MissingDataPolicy::Error => match values.iter().position(|v| !v.is_finite()) {
            Some(index) => Err(TransformError::NonFinite { index }),
            None => Ok(values.to_vec()),
        },
    }
}

/// `true` if any element of `values` is `NaN` specifically (NOT
/// `+-Infinity` — a sum/min/max over an infinity is still a well-defined,
/// non-`NaN` IEEE-754 result; only `NaN` needs the explicit short-circuit
/// this helper exists for, since `NaN` fails every ordering comparison
/// `f64::min`/`f64::max`/`sort_by` rely on).
pub(crate) fn contains_nan(values: &[f64]) -> bool {
    values.iter().any(|v| v.is_nan())
}

/// Kahan-Neumaier compensated running sum — returns `values.len()` prefix
/// sums, each one the exact same value a `cumsum` call should report at
/// that index. Carries a running compensation term to recover the low-
/// order bits an ordinary `+=` loop loses over a long series, the
/// standard technique for summing many floats without accumulating error
/// (Kahan 1965). Used by [`cumulative::cumsum`] directly (every prefix IS
/// the function's own output) and by [`kahan_sum`]/[`mean_kahan`] (which
/// just read the LAST entry) — one shared implementation for every place
/// this module reduces a potentially-long series to a sum.
pub(crate) fn kahan_sum_running(values: &[f64]) -> Vec<f64> {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    let mut out = Vec::with_capacity(values.len());
    for &v in values {
        let y = v - compensation;
        let t = sum + y;
        compensation = (t - sum) - y;
        sum = t;
        out.push(sum);
    }
    out
}

/// The total (last prefix sum) of [`kahan_sum_running`] — `0.0` for an
/// empty slice (the identity element of addition, never a panic).
pub(crate) fn kahan_sum(values: &[f64]) -> f64 {
    kahan_sum_running(values).last().copied().unwrap_or(0.0)
}

/// Kahan-compensated mean — `kahan_sum(values) / values.len()`. `NaN` for
/// an empty slice (a mean of nothing is genuinely undefined, matching
/// this module's own "empty aggregate -> a defined sentinel, not a
/// panic" convention used elsewhere via `TransformError::EmptyInput` at
/// the PUBLIC function boundary; this private helper is only ever called
/// after its caller has already handled the empty case, so the `NaN`
/// fallback here is defensive, not a load-bearing contract).
pub(crate) fn mean_kahan(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    kahan_sum(values) / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_policy_skip_drops_non_finite_values() {
        let values = [1.0, f64::NAN, 2.0, f64::INFINITY, 3.0];
        let out = apply_policy(&values, MissingDataPolicy::Skip).expect("Skip never errors");
        assert_eq!(out, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn apply_policy_propagate_returns_values_unchanged() {
        let values = [1.0, f64::NAN, 2.0];
        let out = apply_policy(&values, MissingDataPolicy::Propagate).expect("Propagate never errors");
        assert_eq!(out.len(), 3);
        assert!(out[1].is_nan());
    }

    #[test]
    fn apply_policy_error_reports_the_first_non_finite_index() {
        let values = [1.0, 2.0, f64::NAN, f64::INFINITY];
        let err = apply_policy(&values, MissingDataPolicy::Error).expect_err("must reject a non-finite value");
        assert_eq!(err, TransformError::NonFinite { index: 2 });
    }

    #[test]
    fn apply_policy_on_empty_input_never_errors_under_any_policy() {
        for policy in [MissingDataPolicy::Skip, MissingDataPolicy::Propagate, MissingDataPolicy::Error] {
            assert_eq!(apply_policy(&[], policy), Ok(Vec::new()));
        }
    }

    #[test]
    fn contains_nan_ignores_infinities() {
        assert!(!contains_nan(&[1.0, f64::INFINITY, f64::NEG_INFINITY]));
        assert!(contains_nan(&[1.0, f64::NAN]));
        assert!(!contains_nan(&[]));
    }

    #[test]
    fn kahan_sum_matches_naive_sum_for_well_conditioned_data() {
        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        assert!((kahan_sum(&values) - 5050.0).abs() < 1e-9);
    }

    #[test]
    fn kahan_sum_running_last_element_equals_kahan_sum() {
        let values = vec![1.5, -2.5, 3.25, 7.0];
        let running = kahan_sum_running(&values);
        assert_eq!(running.len(), values.len());
        assert!((*running.last().unwrap_or(&0.0) - kahan_sum(&values)).abs() < 1e-12);
    }

    #[test]
    fn kahan_sum_of_empty_is_zero() {
        assert_eq!(kahan_sum(&[]), 0.0);
        assert!(kahan_sum_running(&[]).is_empty());
    }

    #[test]
    fn kahan_sum_is_more_accurate_than_naive_summation_for_a_classic_ill_conditioned_case() {
        // The textbook Kahan-summation demonstration: one large value
        // plus many small values whose sum is comparable in magnitude to
        // the large value's own last representable ULP — naive
        // sequential summation loses the small values entirely once the
        // running total's own precision can't represent them; Kahan's
        // compensation term recovers them.
        let mut values = vec![1.0e16_f64];
        values.extend(std::iter::repeat(1.0_f64).take(10_000));
        values.push(-1.0e16_f64);
        // True mathematical result: 10_000 (the two huge terms cancel).
        let naive: f64 = values.iter().sum();
        let kahan = kahan_sum(&values);
        assert!((kahan - 10_000.0).abs() < 1.0, "Kahan sum should recover the true total, got {kahan}");
        assert!((kahan - 10_000.0).abs() < (naive - 10_000.0).abs(), "Kahan sum must be at least as accurate as naive summation here");
    }

    #[test]
    fn mean_kahan_matches_hand_computed_average() {
        assert!((mean_kahan(&[2.0, 4.0, 6.0, 8.0]) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn mean_kahan_of_empty_is_nan_not_a_panic() {
        assert!(mean_kahan(&[]).is_nan());
    }

    #[test]
    fn transform_error_display_is_human_readable() {
        assert_eq!(TransformError::NonFinite { index: 3 }.to_string(), "non-finite value at index 3");
        assert_eq!(TransformError::EmptyInput.to_string(), "no finite values to compute over");
    }
}
