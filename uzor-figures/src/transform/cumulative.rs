//! Cumulative/running reductions — [`cumsum`] (running total),
//! [`running_min`]/[`running_max`] (running extreme-so-far). All three
//! are position-indexed: output `i` is a function of inputs `0..=i` only
//! (a prefix reduction), never the whole series — so [`MissingDataPolicy::
//! Propagate`] poisons every output FROM a non-finite input's own index
//! ONWARD, not the whole result (a running total naturally stays whatever
//! it was before the poison hit; there is nothing to poison retroactively).

use super::{apply_policy, kahan_sum_running, MissingDataPolicy, TransformError};

/// Running total — `out[i]` is the sum of `values[0..=i]` (after
/// `missing`), via [`kahan_sum_running`] (Kahan-Neumaier compensated
/// summation — see that function's own doc comment; chosen here
/// specifically because a `cumsum` over a long series is exactly the
/// "many additions compound rounding error" case Kahan summation exists
/// for, and every prefix is reported, not just the final total, so a
/// RUNNING compensated sum — not a one-shot Kahan total recomputed `n`
/// times — is both the correct and the efficient choice).
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - **Property**: `cumsum(values)[last]` always equals the total of
///   `values` (after `missing`) — see this function's own test.
/// - [`MissingDataPolicy::Propagate`] with a non-finite value at index
///   `i` -> every output from index `i` onward is `NaN` (ordinary IEEE-754
///   `+` propagation — no special-casing needed, unlike `running_min`/
///   `running_max` below).
pub fn cumsum(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    let working = apply_policy(values, missing)?;
    Ok(kahan_sum_running(&working))
}

/// Running minimum — `out[i]` is `min(values[0..=i])` (after `missing`).
///
/// - Empty `values` -> `Ok(Vec::new())`.
/// - [`MissingDataPolicy::Propagate`] with a non-finite value at index
///   `i`: **explicitly poisons every output from `i` onward** — plain
///   `f64::min` would otherwise silently IGNORE a `NaN` operand
///   (IEEE-754 minNum semantics), which is exactly the implicit-NaN-
///   handling trap this module's own doctrine exists to avoid; this
///   function checks for it explicitly instead of relying on `f64::min`'s
///   own default behavior.
pub fn running_min(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    running_extreme(values, missing, f64::min)
}

/// [`running_min`]'s counterpart for the running maximum.
pub fn running_max(values: &[f64], missing: MissingDataPolicy) -> Result<Vec<f64>, TransformError> {
    running_extreme(values, missing, f64::max)
}

fn running_extreme(values: &[f64], missing: MissingDataPolicy, fold: fn(f64, f64) -> f64) -> Result<Vec<f64>, TransformError> {
    let working = apply_policy(values, missing)?;
    let mut out = Vec::with_capacity(working.len());
    let mut current: Option<f64> = None;
    for v in working {
        current = Some(match current {
            None => v,
            Some(c) if c.is_nan() || v.is_nan() => f64::NAN,
            Some(c) => fold(c, v),
        });
        out.push(current.unwrap_or(f64::NAN));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumsum_matches_hand_computed_running_total() {
        let out = cumsum(&[1.0, 2.0, 3.0, 4.0], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![1.0, 3.0, 6.0, 10.0]);
    }

    #[test]
    fn cumsum_last_element_equals_the_total_property() {
        let values: Vec<f64> = (1..=50).map(|i| (i as f64) * 0.37 - 3.0).collect();
        let out = cumsum(&values, MissingDataPolicy::Skip).unwrap();
        let naive_total: f64 = values.iter().sum();
        assert!((out.last().copied().unwrap_or(0.0) - naive_total).abs() < 1e-6);
    }

    #[test]
    fn cumsum_empty_is_empty() {
        assert_eq!(cumsum(&[], MissingDataPolicy::Skip).unwrap(), Vec::<f64>::new());
    }

    #[test]
    fn cumsum_single_element_is_itself() {
        assert_eq!(cumsum(&[7.0], MissingDataPolicy::Skip).unwrap(), vec![7.0]);
    }

    #[test]
    fn cumsum_skip_drops_non_finite_before_accumulating() {
        let out = cumsum(&[1.0, f64::NAN, 2.0], MissingDataPolicy::Skip).unwrap();
        assert_eq!(out, vec![1.0, 3.0]);
    }

    #[test]
    fn cumsum_propagate_poisons_from_the_non_finite_index_onward() {
        let out = cumsum(&[1.0, 2.0, f64::NAN, 4.0], MissingDataPolicy::Propagate).unwrap();
        assert_eq!(out[0], 1.0);
        assert_eq!(out[1], 3.0);
        assert!(out[2].is_nan());
        assert!(out[3].is_nan());
    }

    #[test]
    fn cumsum_error_rejects_up_front() {
        let err = cumsum(&[1.0, f64::NAN], MissingDataPolicy::Error).unwrap_err();
        assert_eq!(err, TransformError::NonFinite { index: 1 });
    }

    #[test]
    fn running_min_max_match_hand_computed_sequences() {
        let values = [5.0, 2.0, 8.0, 1.0, 9.0];
        assert_eq!(running_min(&values, MissingDataPolicy::Skip).unwrap(), vec![5.0, 2.0, 2.0, 1.0, 1.0]);
        assert_eq!(running_max(&values, MissingDataPolicy::Skip).unwrap(), vec![5.0, 5.0, 8.0, 8.0, 9.0]);
    }

    #[test]
    fn running_min_max_empty_and_single_element() {
        assert!(running_min(&[], MissingDataPolicy::Skip).unwrap().is_empty());
        assert_eq!(running_max(&[4.0], MissingDataPolicy::Skip).unwrap(), vec![4.0]);
    }

    #[test]
    fn running_min_max_all_equal_stays_constant() {
        let values = [3.0; 6];
        assert_eq!(running_min(&values, MissingDataPolicy::Skip).unwrap(), vec![3.0; 6]);
        assert_eq!(running_max(&values, MissingDataPolicy::Skip).unwrap(), vec![3.0; 6]);
    }

    #[test]
    fn running_min_max_propagate_poisons_from_the_non_finite_index_onward_unlike_naive_f64_min() {
        let values = [5.0, f64::NAN, 2.0, 1.0];
        let mins = running_min(&values, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(mins[0], 5.0);
        assert!(mins[1].is_nan(), "plain f64::min would have silently IGNORED the NaN here — this must not");
        assert!(mins[2].is_nan(), "poisoning must persist forward, not self-heal once real numbers resume");
        assert!(mins[3].is_nan());
    }

    #[test]
    fn running_min_max_skip_treats_non_finite_as_absent_never_poisons() {
        let values = [5.0, f64::NAN, 2.0];
        let mins = running_min(&values, MissingDataPolicy::Skip).unwrap();
        assert_eq!(mins, vec![5.0, 2.0]);
    }
}
