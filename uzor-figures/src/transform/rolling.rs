//! Rolling-window smoothing — [`rolling_mean`], [`rolling_median`],
//! [`ema`] (exponential moving average). Every reference implementation
//! (pandas, numpy, TA-Lib, every charting library's own SMA/EMA
//! indicator) disagrees on ONE specific point: what to report at the
//! LEADING EDGE, before a full window's worth of data has accumulated —
//! [`EdgePolicy`] is the explicit, named answer this module requires
//! instead of silently picking one.
//!
//! **Complexity note**: each window is reduced FRESH (no incremental
//! running-sum/running-median data structure) — `O(n * window)` worst
//! case. A deliberate, documented simplicity-over-asymptotic-optimality
//! choice: this crate's own data scale is figure-rendering-sized (a few
//! thousand points, a window in the tens-to-low-hundreds), not a
//! high-frequency streaming pipeline where an `O(n)` incremental
//! algorithm would matter.
//!
//! **`Skip` shifts positions, like every other function in this module**:
//! [`MissingDataPolicy::Skip`] filters non-finite values out of the WHOLE
//! series before windowing (the same global-then-compute convention
//! [`super::cumulative`] uses) — a caller whose rolling window must stay
//! POSITION-ALIGNED with the original series (e.g. an X-axis of
//! timestamps) should use [`MissingDataPolicy::Propagate`] instead (same
//! length output, `NaN` marking the poisoned windows) rather than `Skip`.

use super::{apply_policy, mean_kahan, quantile::quantile, MissingDataPolicy, TransformError};

/// How a rolling/EMA function treats its own LEADING EDGE, before it has
/// seen enough data to compute a "real," fully-supported value — see
/// each consuming function's own doc comment for exactly what this means
/// for THAT function (the three functions in this module do not share
/// identical leading-edge mechanics — [`ema`]'s own doc comment explains
/// how its alpha-derived "implied period" substitutes for `rolling_mean`/
/// `rolling_median`'s literal `window` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EdgePolicy {
    /// No output until a full window is available — `None` for every
    /// index short of it. THE DEFAULT: matches pandas' own un-configured
    /// `rolling().mean()` (`min_periods` defaults to the window size) and
    /// R's `zoo::rollmean()` default — the most common "don't report a
    /// number I can't stand behind yet" convention across reference
    /// implementations.
    #[default]
    None,
    /// Compute over however many samples ARE available — a genuinely
    /// SHRINKING window at the leading edge (pandas' own
    /// `min_periods=1`). Every index gets a real, if less-supported,
    /// value.
    Partial,
    /// Pad the leading edge with `window - avail` copies of the FIRST
    /// available sample before reducing — every index gets a real,
    /// FULL-WIDTH-window value, biased toward the series' own opening
    /// value more strongly than [`EdgePolicy::Partial`] would (a common
    /// charting-library "warm-up via edge replication" convention).
    Pad,
}

/// Simple moving average over a sliding `window` — see [`EdgePolicy`]'s
/// own doc comment for the 3-way leading-edge contract this function
/// follows literally (`window - avail` values short of a full window at
/// index `i` -> `None`/shrinking-window/padded per policy).
///
/// - `window` is floored at `1` (a window of 1 is simply the series
///   itself, every index `Some`).
/// - Empty `values` -> `Ok(Vec::new())`.
/// - Uses [`mean_kahan`] (Kahan-compensated summation) for each window's
///   own mean — the same numerically-stable mean this module uses
///   everywhere else (see `transform::mod`'s own doc comment).
pub fn rolling_mean(values: &[f64], window: usize, edge: EdgePolicy, missing: MissingDataPolicy) -> Result<Vec<Option<f64>>, TransformError> {
    rolling_reduce(values, window, edge, missing, mean_kahan)
}

/// [`rolling_mean`]'s counterpart using the LINEAR-INTERPOLATION MEDIAN
/// (this crate's ONE canonical [`quantile`](super::quantile::quantile)
/// method at `q = 0.5`) instead of the mean — robust to a single outlier
/// sample within the window, unlike [`rolling_mean`]. Same `window`/
/// `edge`/`missing` contract.
pub fn rolling_median(values: &[f64], window: usize, edge: EdgePolicy, missing: MissingDataPolicy) -> Result<Vec<Option<f64>>, TransformError> {
    // `Propagate` here is a per-WINDOW re-check (not a re-filter — the
    // outer `apply_policy` call inside `rolling_reduce` already resolved
    // `missing` once, globally): if the outer policy was itself
    // `Propagate` and a NaN landed inside THIS window, `quantile` reports
    // `NaN` for this window specifically (the windowed "poisons forward"
    // contract this module's own top doc names); if the outer policy was
    // `Skip`/`Error`, no NaN can reach here at all, so this is a no-op.
    rolling_reduce(values, window, edge, missing, |w| quantile(w, 0.5, MissingDataPolicy::Propagate).unwrap_or(f64::NAN))
}

fn rolling_reduce(
    values: &[f64],
    window: usize,
    edge: EdgePolicy,
    missing: MissingDataPolicy,
    reduce: impl Fn(&[f64]) -> f64,
) -> Result<Vec<Option<f64>>, TransformError> {
    let working = apply_policy(values, missing)?;
    let window = window.max(1);
    let n = working.len();
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        let avail = i + 1;
        let value = if avail >= window {
            Some(reduce(&working[i + 1 - window..=i]))
        } else {
            match edge {
                EdgePolicy::None => None,
                EdgePolicy::Partial => Some(reduce(&working[..avail])),
                EdgePolicy::Pad => {
                    let pad_count = window - avail;
                    let first = working[0];
                    let mut padded: Vec<f64> = std::iter::repeat(first).take(pad_count).collect();
                    padded.extend_from_slice(&working[..avail]);
                    Some(reduce(&padded))
                }
            }
        };
        out.push(value);
    }
    Ok(out)
}

/// Exponential moving average — `ema[i] = alpha * value[i] + (1 - alpha)
/// * ema[i - 1]`, `alpha` clamped to `(0, 1]` (the conventional EMA
/// smoothing-factor range; a value outside it is silently clamped, not
/// rejected — documented here, not implicit).
///
/// **`EdgePolicy` for EMA, explained** (EMA's own recursion has no
/// literal `window` — this function derives an "implied period" from
/// `alpha` via the standard technical-analysis identity `alpha = 2 / (N +
/// 1)`, i.e. `N = 2 / alpha - 1`, and uses THAT as its own leading-edge
/// span):
/// - [`EdgePolicy::None`] — delayed start: no output reported until index
///   `N - 1` (an "N-period-equivalent EMA shouldn't speak for itself
///   before N samples," the TA-Lib-style convention), even though the
///   recursion itself is already running underneath from index `0`.
/// - [`EdgePolicy::Partial`] — seed the recursion directly from
///   `values[0]` and report every index immediately (the most common
///   charting-library default — every index has a value, it just hasn't
///   converged yet in the earliest bars).
/// - [`EdgePolicy::Pad`] — seed the recursion from the SIMPLE MEAN of the
///   first `N` values (a smoothed anchor) instead of the raw
///   `values[0]`, then run the SAME recursion from index `0` — every
///   index still gets a value, but the earliest ones start from a
///   better-informed seed than `Partial`'s single raw sample.
///
/// This crate does not claim bit-for-bit parity with any specific
/// external library's own EMA seeding (TA-Lib, pandas' `ewm`, etc.) —
/// each of the three variants above is a real, named, industry-standard
/// convention, but the exact recursion-restart semantics at the seed
/// point are this crate's own documented choice, not a verified
/// reproduction of a particular reference implementation.
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - A single value -> `Some(values[0])` for every `edge` variant except
///   `None` when the implied period exceeds `1` (an alpha implying a
///   multi-sample warm-up over a 1-sample series never reports anything).
pub fn ema(values: &[f64], alpha: f64, edge: EdgePolicy, missing: MissingDataPolicy) -> Result<Vec<Option<f64>>, TransformError> {
    let working = apply_policy(values, missing)?;
    let n = working.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    // `f64::clamp` is a no-op on a NaN `self` (neither `<` nor `>`
    // comparison against a NaN is ever true, so its own branches never
    // fire) — handled explicitly here rather than left as a silent gap:
    // a NaN alpha falls back to `1.0` (maximally reactive — `ema[i] ==
    // value[i]`), the same "an extreme/undefined smoothing request still
    // produces a finite, defined series" spirit every degenerate case in
    // this module follows.
    let alpha = if alpha.is_nan() { 1.0 } else { alpha.clamp(f64::EPSILON, 1.0) };
    let implied_period = ((2.0 / alpha) - 1.0).round().max(1.0) as usize;

    let seed = match edge {
        EdgePolicy::Pad => mean_kahan(&working[..implied_period.min(n)]),
        EdgePolicy::None | EdgePolicy::Partial => working[0],
    };

    let mut out = Vec::with_capacity(n);
    let mut current = seed;
    for (i, &v) in working.iter().enumerate() {
        if i == 0 {
            current = seed;
        } else {
            current = alpha * v + (1.0 - alpha) * current;
        }
        let emit = match edge {
            EdgePolicy::None => i + 1 >= implied_period,
            EdgePolicy::Partial | EdgePolicy::Pad => true,
        };
        out.push(if emit { Some(current) } else { None });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── rolling_mean ─────────────────────────────────────────────────

    #[test]
    fn rolling_mean_matches_hand_computed_windows() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        let out = rolling_mean(&values, 3, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![None, None, Some(2.0), Some(3.0), Some(4.0)]);
    }

    #[test]
    fn rolling_mean_edge_none_leading_gap_is_exactly_window_minus_one() {
        let values = [1.0; 10];
        let out = rolling_mean(&values, 4, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[..3], [None, None, None]);
        assert!(out[3..].iter().all(Option::is_some));
    }

    #[test]
    fn rolling_mean_edge_partial_shrinking_window_every_index_defined() {
        let values = [2.0, 4.0, 6.0, 8.0];
        let out = rolling_mean(&values, 3, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[0], Some(2.0), "avail=1: mean of [2.0]");
        assert_eq!(out[1], Some(3.0), "avail=2: mean of [2.0, 4.0]");
        assert_eq!(out[2], Some(4.0), "avail=3: full window [2,4,6]");
    }

    #[test]
    fn rolling_mean_edge_pad_biases_toward_the_first_value_more_than_partial() {
        let values = [10.0, 100.0];
        let partial = rolling_mean(&values, 3, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        let pad = rolling_mean(&values, 3, EdgePolicy::Pad, MissingDataPolicy::Skip).unwrap();
        // index 1: Partial mean([10,100]) = 55; Pad mean([10(pad),10,100]) = 40.
        assert_eq!(partial[1], Some(55.0));
        assert_eq!(pad[1], Some(40.0));
    }

    #[test]
    fn rolling_mean_window_floors_at_one() {
        let values = [3.0, 5.0, 7.0];
        let out = rolling_mean(&values, 0, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![Some(3.0), Some(5.0), Some(7.0)]);
    }

    #[test]
    fn rolling_mean_empty_is_empty() {
        assert!(rolling_mean(&[], 3, EdgePolicy::None, MissingDataPolicy::Skip).unwrap().is_empty());
    }

    #[test]
    fn rolling_mean_all_equal_stays_constant() {
        let out = rolling_mean(&[5.0; 8], 3, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert!(out.iter().all(|v| *v == Some(5.0)));
    }

    #[test]
    fn rolling_mean_propagate_poisons_only_windows_touching_the_non_finite_value() {
        let values = [1.0, 2.0, f64::NAN, 4.0, 5.0];
        let out = rolling_mean(&values, 2, EdgePolicy::Partial, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(out[0], Some(1.0));
        assert_eq!(out[1], Some(1.5));
        assert!(out[2].unwrap().is_nan(), "window [2, NaN] must be poisoned");
        assert!(out[3].unwrap().is_nan(), "window [NaN, 4] must be poisoned");
        assert_eq!(out[4], Some(4.5), "window [4, 5] never touched the NaN, must be clean");
    }

    #[test]
    fn rolling_mean_error_rejects_up_front() {
        let err = rolling_mean(&[1.0, f64::NAN], 2, EdgePolicy::None, MissingDataPolicy::Error).unwrap_err();
        assert_eq!(err, TransformError::NonFinite { index: 1 });
    }

    // ── rolling_median ───────────────────────────────────────────────

    #[test]
    fn rolling_median_matches_hand_computed_windows() {
        let values = [1.0, 5.0, 2.0, 8.0, 3.0];
        let out = rolling_median(&values, 3, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[2], Some(2.0), "median of [1,5,2] sorted [1,2,5] -> 2");
        assert_eq!(out[3], Some(5.0), "median of [5,2,8] sorted [2,5,8] -> 5");
    }

    #[test]
    fn rolling_median_is_robust_to_a_single_outlier_unlike_rolling_mean() {
        let values = [4.0, 5.0, 6.0, 1000.0, 5.0];
        let mean_out = rolling_mean(&values, 3, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        let median_out = rolling_median(&values, 3, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert!(mean_out[3].unwrap() > 100.0, "mean gets dragged by the outlier");
        assert!(median_out[3].unwrap() < 10.0, "median stays near the non-outlier values");
    }

    // ── ema ───────────────────────────────────────────────────────────

    #[test]
    fn ema_partial_seeds_from_the_first_value_and_recurses() {
        let values = [10.0, 20.0, 30.0];
        let alpha = 0.5;
        let out = ema(&values, alpha, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[0], Some(10.0));
        assert_eq!(out[1], Some(15.0), "0.5*20 + 0.5*10");
        assert_eq!(out[2], Some(22.5), "0.5*30 + 0.5*15");
    }

    #[test]
    fn ema_none_delays_output_until_the_implied_period() {
        // alpha = 2/(N+1) => N = 2/alpha - 1. alpha=0.5 -> N=3.
        let values = [10.0, 20.0, 30.0, 40.0];
        let out = ema(&values, 0.5, EdgePolicy::None, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[0], None);
        assert_eq!(out[1], None);
        assert!(out[2].is_some(), "index 2 is the (implied_period=3)rd sample, must emit");
        assert!(out[3].is_some());
    }

    #[test]
    fn ema_pad_seeds_from_the_sma_of_the_implied_period_not_the_raw_first_value() {
        let values = [10.0, 100.0, 100.0];
        let alpha = 1.0; // implied_period = 2/1 - 1 = 1 -> Pad seeds from mean of first 1 value == Partial.
        let pad = ema(&values, alpha, EdgePolicy::Pad, MissingDataPolicy::Skip).unwrap();
        let partial = ema(&values, alpha, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert_eq!(pad, partial, "an implied period of 1 makes Pad's own SMA seed degenerate to the same single raw value Partial uses");

        let alpha2 = 2.0 / 4.0; // N = 2/0.5 - 1 = 3.
        let pad2 = ema(&values, alpha2, EdgePolicy::Pad, MissingDataPolicy::Skip).unwrap();
        let partial2 = ema(&values, alpha2, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert_ne!(pad2[0], partial2[0], "a real (>1) implied period must make Pad's own smoothed seed differ from Partial's raw-first-value seed");
    }

    #[test]
    fn ema_empty_is_empty() {
        assert!(ema(&[], 0.5, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap().is_empty());
    }

    #[test]
    fn ema_single_value_is_itself_under_partial_and_pad() {
        assert_eq!(ema(&[9.0], 0.3, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap(), vec![Some(9.0)]);
        assert_eq!(ema(&[9.0], 0.3, EdgePolicy::Pad, MissingDataPolicy::Skip).unwrap(), vec![Some(9.0)]);
    }

    #[test]
    fn ema_alpha_is_clamped_to_a_sane_range_never_panics_and_never_produces_nan_output() {
        let values = [1.0, 2.0, 3.0];
        for bad_alpha in [-5.0, 0.0, 3.7, f64::NAN, f64::INFINITY] {
            let out = ema(&values, bad_alpha, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
            assert_eq!(out.len(), 3);
            assert!(out.iter().all(|v| v.is_some_and(|x| x.is_finite())), "a degenerate alpha must clamp to a finite result, got {out:?}");
        }
    }

    #[test]
    fn ema_all_equal_stays_constant() {
        let out = ema(&[4.0; 6], 0.4, EdgePolicy::Partial, MissingDataPolicy::Skip).unwrap();
        assert!(out.iter().all(|v| *v == Some(4.0)));
    }
}
