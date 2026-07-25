//! Group-by-key rollup — the "categorical -> values" reduction a report
//! figure recomputes by hand at the call site today (a caller building a
//! [`crate::figure::BarFigure`] from raw transaction-level rows has no
//! shared helper to collapse them into one value per category first).
//!
//! Group order is DETERMINISTIC — first-seen key order, never a `HashMap`
//! iteration order — since the whole point of a rollup here is usually
//! "feed the result straight into a categorical axis," where a caller
//! reasonably expects the SAME input to always produce the SAME category
//! ordering across runs.

use super::{apply_policy, quantile::quantile, MissingDataPolicy, TransformError};

/// A built-in reduction [`rollup`] applies to each group's own value
/// list. For anything else, use [`rollup_with`] with a closure directly —
/// a `fn(&[f64]) -> f64` inside this enum would need `Debug`/`Clone`
/// bounds this crate doesn't otherwise need to carry around, and a plain
/// closure parameter is the more idiomatic Rust "custom reducer" shape
/// anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reducer {
    Sum,
    Mean,
    Min,
    Max,
    /// Number of values in the group AFTER `missing` has been applied
    /// (e.g. under [`MissingDataPolicy::Skip`], a non-finite entry is not
    /// counted — it was dropped before reaching the reducer at all).
    Count,
    /// Linear-interpolation median — [`super::quantile::quantile`] at
    /// `q = 0.5`.
    Median,
}

/// Group `values[i]` by `keys[i]` and reduce each group with `reducer`.
/// Sugar for [`rollup_with`] with the matching built-in closure — see
/// that function's own doc comment for the full contract (group order,
/// missing-data handling, mismatched-length behavior, empty/degenerate
/// cases).
pub fn rollup(keys: &[String], values: &[f64], reducer: Reducer, missing: MissingDataPolicy) -> Result<Vec<(String, f64)>, TransformError> {
    match reducer {
        Reducer::Sum => rollup_with(keys, values, |g| g.iter().sum(), missing),
        Reducer::Mean => rollup_with(keys, values, super::mean_kahan, missing),
        Reducer::Min => rollup_with(keys, values, safe_min, missing),
        Reducer::Max => rollup_with(keys, values, safe_max, missing),
        Reducer::Count => rollup_with(keys, values, |g| g.len() as f64, missing),
        Reducer::Median => rollup_with(keys, values, |g| quantile(g, 0.5, MissingDataPolicy::Skip).unwrap_or(f64::NAN), missing),
    }
}

/// `f64::min` silently IGNORES a `NaN` operand (IEEE-754 minNum
/// semantics) rather than propagating it — the exact implicit-NaN-
/// handling trap this module's own doctrine exists to avoid. Under
/// [`MissingDataPolicy::Propagate`] a group's own values can legitimately
/// still contain `NaN` (that policy deliberately doesn't filter), so this
/// needs an explicit short-circuit to actually propagate one instead of
/// quietly folding around it. `Empty` group -> `NaN` (no minimum of
/// nothing).
fn safe_min(group: &[f64]) -> f64 {
    if group.is_empty() || super::contains_nan(group) {
        return f64::NAN;
    }
    group.iter().copied().fold(f64::INFINITY, f64::min)
}

/// [`safe_min`]'s counterpart for `f64::max` — same reasoning, same
/// empty/`NaN` handling.
fn safe_max(group: &[f64]) -> f64 {
    if group.is_empty() || super::contains_nan(group) {
        return f64::NAN;
    }
    group.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

/// Group `values[i]` by `keys[i]` (`keys`/`values` zipped — a length
/// mismatch silently truncates to the SHORTER of the two, `Iterator::zip`'s
/// own well-known, non-panicking behavior; never a panic, always
/// documented rather than an implicit surprise) and reduce each group
/// with `reduce`.
///
/// - **Group order**: first-seen key order (NOT alphabetical, NOT a
///   `HashMap`'s unspecified iteration order) — deterministic across runs
///   for the same input, matching this crate's own "reproducible
///   screenshots/diffs" convention (design law 8 elsewhere in this
///   workspace).
/// - **Missing data** (`missing`): applied to EACH GROUP's own value list
///   independently, before `reduce` ever sees it — [`MissingDataPolicy::Skip`]
///   drops non-finite entries from that group only;
///   [`MissingDataPolicy::Propagate`] leaves them in (so `reduce` itself
///   decides how a `NaN` affects its own result — every built-in
///   [`Reducer`] documented above propagates `NaN` correctly, including
///   `Min`/`Max`, which need an explicit check since raw `f64::min`/`max`
///   otherwise silently ignore a `NaN` operand);
///   [`MissingDataPolicy::Error`] rejects the WHOLE call at the first
///   non-finite value found in `values` (checked up front, over the full
///   `values` slice, before any grouping — `index` in the returned
///   [`TransformError::NonFinite`] is that value's own index in `values`,
///   not an index within its group).
/// - **Empty input** (`keys`/`values` both empty, or one of them empty) ->
///   `Ok(Vec::new())`, no groups.
/// - **A group with zero REMAINING values** after `missing` has filtered
///   it (e.g. every entry for that key was non-finite under `Skip`) still
///   appears in the output — `reduce` is called on an empty slice for
///   that group, and every built-in [`Reducer`] defines a sane empty
///   answer (`Sum`/`Count` -> `0.0`; `Mean`/`Min`/`Max`/`Median` -> `NaN`,
///   an honest "no data" sentinel, never a panic or a fabricated `0.0`
///   that would misleadingly read as a real zero-valued measurement).
pub fn rollup_with<F: Fn(&[f64]) -> f64>(
    keys: &[String],
    values: &[f64],
    reduce: F,
    missing: MissingDataPolicy,
) -> Result<Vec<(String, f64)>, TransformError> {
    if missing == MissingDataPolicy::Error {
        if let Some(index) = values.iter().position(|v| !v.is_finite()) {
            return Err(TransformError::NonFinite { index });
        }
    }

    let mut order: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<f64>> = Vec::new();
    for (key, &value) in keys.iter().zip(values.iter()) {
        let group_index = match order.iter().position(|k| k == key) {
            Some(i) => i,
            None => {
                order.push(key.clone());
                groups.push(Vec::new());
                order.len() - 1
            }
        };
        groups[group_index].push(value);
    }

    let mut out = Vec::with_capacity(order.len());
    for (key, raw_group) in order.into_iter().zip(groups.into_iter()) {
        // `apply_policy` never errors here (the up-front `Error` check
        // above already rejected the call if that policy is active) —
        // `unwrap_or_default` is a safe, documented fallback, not a
        // load-bearing branch.
        let group = apply_policy(&raw_group, missing).unwrap_or_default();
        out.push((key, reduce(&group)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn groups_by_first_seen_key_order() {
        let keys = cats(&["b", "a", "b", "c", "a"]);
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let out = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Skip).unwrap();
        let order: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(order, vec!["b", "a", "c"], "must reflect FIRST-SEEN order, not alphabetical or insertion-hash order");
    }

    #[test]
    fn sum_reducer_matches_hand_computed_totals() {
        let keys = cats(&["a", "b", "a", "b", "a"]);
        let values = vec![1.0, 10.0, 2.0, 20.0, 3.0];
        let out = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![("a".to_owned(), 6.0), ("b".to_owned(), 30.0)]);
    }

    #[test]
    fn mean_reducer_matches_hand_computed_averages() {
        let keys = cats(&["a", "a", "a", "b", "b"]);
        let values = vec![2.0, 4.0, 6.0, 10.0, 20.0];
        let out = rollup(&keys, &values, Reducer::Mean, MissingDataPolicy::Skip).unwrap();
        assert!((out[0].1 - 4.0).abs() < 1e-9);
        assert!((out[1].1 - 15.0).abs() < 1e-9);
    }

    #[test]
    fn min_max_reducers_match_hand_computed_extremes() {
        let keys = cats(&["a", "a", "a", "b"]);
        let values = vec![5.0, -3.0, 9.0, 42.0];
        let mins = rollup(&keys, &values, Reducer::Min, MissingDataPolicy::Skip).unwrap();
        let maxs = rollup(&keys, &values, Reducer::Max, MissingDataPolicy::Skip).unwrap();
        assert_eq!(mins[0].1, -3.0);
        assert_eq!(maxs[0].1, 9.0);
        assert_eq!(mins[1].1, 42.0);
        assert_eq!(maxs[1].1, 42.0);
    }

    #[test]
    fn count_reducer_counts_entries_after_missing_data_filtering() {
        let keys = cats(&["a", "a", "a"]);
        let values = vec![1.0, f64::NAN, 3.0];
        let skip_count = rollup(&keys, &values, Reducer::Count, MissingDataPolicy::Skip).unwrap();
        let propagate_count = rollup(&keys, &values, Reducer::Count, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(skip_count[0].1, 2.0, "Skip must exclude the NaN entry from the count");
        assert_eq!(propagate_count[0].1, 3.0, "Propagate must still count the NaN entry as a present (if poisoned) observation");
    }

    #[test]
    fn median_reducer_matches_the_linear_interpolation_golden() {
        let keys = cats(&["a", "a", "a", "a"]);
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let out = rollup(&keys, &values, Reducer::Median, MissingDataPolicy::Skip).unwrap();
        assert!((out[0].1 - 2.5).abs() < 1e-9);
    }

    #[test]
    fn custom_reducer_via_rollup_with() {
        let keys = cats(&["a", "a", "b"]);
        let values = vec![2.0, 4.0, 100.0];
        // A custom "range" (max - min) reducer.
        let out = rollup_with(&keys, &values, |g| g.iter().copied().fold(f64::NEG_INFINITY, f64::max) - g.iter().copied().fold(f64::INFINITY, f64::min), MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[0].1, 2.0);
        assert_eq!(out[1].1, 0.0);
    }

    #[test]
    fn empty_input_is_empty_output_never_a_panic() {
        assert_eq!(rollup(&[], &[], Reducer::Sum, MissingDataPolicy::Skip).unwrap(), Vec::new());
    }

    #[test]
    fn mismatched_lengths_truncate_to_the_shorter_slice() {
        let keys = cats(&["a", "b", "c"]);
        let values = vec![1.0, 2.0];
        let out = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out.len(), 2, "the third key has no matching value and must not appear");
    }

    #[test]
    fn skip_policy_drops_non_finite_values_from_their_own_group_only() {
        let keys = cats(&["a", "a", "b"]);
        let values = vec![1.0, f64::NAN, 5.0];
        let out = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Skip).unwrap();
        assert_eq!(out[0].1, 1.0, "group a's NaN must be dropped, leaving only 1.0");
        assert_eq!(out[1].1, 5.0, "group b is untouched by group a's own NaN");
    }

    #[test]
    fn propagate_policy_poisons_only_the_touched_group() {
        let keys = cats(&["a", "a", "b"]);
        let values = vec![1.0, f64::NAN, 5.0];
        let out = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Propagate).unwrap();
        assert!(out[0].1.is_nan(), "group a must be poisoned");
        assert_eq!(out[1].1, 5.0, "group b must be unaffected");
    }

    #[test]
    fn error_policy_rejects_the_whole_call_at_the_first_non_finite_value() {
        let keys = cats(&["a", "b", "a"]);
        let values = vec![1.0, f64::NAN, 3.0];
        let err = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Error).unwrap_err();
        assert_eq!(err, TransformError::NonFinite { index: 1 });
    }

    #[test]
    fn a_group_emptied_entirely_by_skip_still_appears_with_a_defined_answer() {
        let keys = cats(&["a", "b"]);
        let values = vec![f64::NAN, 5.0];
        let sums = rollup(&keys, &values, Reducer::Sum, MissingDataPolicy::Skip).unwrap();
        let means = rollup(&keys, &values, Reducer::Mean, MissingDataPolicy::Skip).unwrap();
        assert_eq!(sums[0].1, 0.0, "an emptied group's SUM is the additive identity, not a panic");
        assert!(means[0].1.is_nan(), "an emptied group's MEAN is an honest NaN, never a fabricated 0.0");
    }
}
