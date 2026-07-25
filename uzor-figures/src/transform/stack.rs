//! Stacking — extracted from [`crate::figure::bars`], where it used to be
//! baked directly into `BarFigure::{stack_totals, stacked_segments_px}`
//! with no reuse path. [`stack`] is the pure, domain-space (not pixel-
//! space) computation both those private methods now delegate to.
//!
//! [`crate::figure::BarFigure`]'s own pre-existing convention — positive
//! values accumulate upward from a zero baseline, negative values
//! accumulate downward from it, INDEPENDENTLY (never combined into one
//! running total) — is [`StackOffset::Diverging`], and stays the DEFAULT
//! (byte-identical to every pre-existing stacked-bar render). [`stack`]
//! additionally offers the two other standard offset conventions
//! (d3's own `stackOffsetNone`/`stackOffsetExpand`) as OPT-IN options —
//! [`crate::figure::BarFigure::with_stack_offset`] — plus a `StackOrder`
//! choice for which series contributes the innermost/outermost segment.

use super::{MissingDataPolicy, TransformError};

/// Which series-accumulation convention [`stack`] uses to turn raw
/// per-category values into cumulative `(bottom, top)` segment pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackOffset {
    /// Positive values accumulate upward from `0.0`, negative values
    /// accumulate downward from `0.0`, the two running totals kept
    /// entirely independent — [`crate::figure::BarFigure`]'s own
    /// pre-existing, and still default, convention (the standard
    /// finance-chart "gains stack up, losses stack down" reading).
    #[default]
    Diverging,
    /// Sequential accumulation regardless of sign (d3's own
    /// `stackOffsetNone`) — series `n`'s own segment starts exactly where
    /// series `n - 1`'s own segment ended, so a negative value SHRINKS
    /// the running total instead of starting a separate downward stack.
    Zero,
    /// [`StackOffset::Zero`]'s own accumulation, then every boundary in a
    /// category divided by that category's own value TOTAL (d3's own
    /// `stackOffsetExpand`) — the "100% stacked" / percent-of-total
    /// reading. A category whose total is exactly `0.0` produces `NaN`
    /// boundaries for that category (a genuine `0 / 0`, matching this
    /// crate's own `percent_of_total` convention — see that function's
    /// own doc comment for the same reasoning).
    Expand,
}

/// Which order [`stack`] accumulates `series` in (the FIRST series in
/// stacking order contributes the segment closest to the baseline). The
/// OUTPUT is always indexed by the ORIGINAL `series` order regardless of
/// this choice — only the cumulative POSITION each series lands at
/// changes, matching d3's own "order changes z-index, not identity"
/// convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackOrder {
    /// Stack in the exact order `series` was given — [`crate::figure::
    /// BarFigure`]'s own pre-existing, and still default, convention.
    #[default]
    AsGiven,
    /// The reverse of `series`' own given order.
    Reverse,
    /// Ascending by each series' own value TOTAL (under `missing`'s own
    /// policy — see [`stack`]'s own doc comment) — the series contributing
    /// the LEAST overall ends up closest to the baseline.
    AscendingSum,
    /// Descending by each series' own value total — the series
    /// contributing the MOST overall ends up closest to the baseline.
    DescendingSum,
}

/// Stack `series` (each `series[j]` a value slice indexed by category,
/// slices may have DIFFERENT lengths — a category index missing from a
/// shorter series is treated as `0.0`, matching [`crate::figure::
/// BarFigure`]'s own pre-existing "missing = no contribution" convention)
/// across `category_count` categories, per `order`/`offset`.
///
/// Returns `Vec<Vec<(f64, f64)>>` — OUTER indexed by series in `series`'
/// OWN original order (never reordered, regardless of `order` — see
/// [`StackOrder`]'s own doc comment), INNER indexed by category, each
/// entry the `(bottom, top)` DOMAIN-space (not pixel) segment boundary
/// for that series/category.
///
/// **Missing data** (`missing`): applied PER VALUE as it's folded into
/// the running accumulator (not pre-filtered across the whole series,
/// since a stack is inherently positional/cumulative — see this module's
/// own top-level doc comment for the general "windowed function" framing
/// this shares with [`super::rolling`]):
/// - [`MissingDataPolicy::Skip`] — a non-finite value contributes `0.0`
///   (no change to either accumulator), same effective treatment as a
///   missing category index.
/// - [`MissingDataPolicy::Propagate`] — **the DEFAULT for
///   [`crate::figure::BarFigure`]'s own internal calls**, chosen
///   specifically because it reproduces this crate's pre-existing
///   (undocumented until now) behavior byte-for-byte: the old
///   `stacked_segments_px` branched on `value >= 0.0` to choose which
///   accumulator to add a value into, and `NaN >= 0.0` is ALWAYS `false`
///   in IEEE-754 — so a `NaN` series value always fell into the NEGATIVE
///   accumulator, poisoning every subsequent NEGATIVE segment in that
///   category from that series onward (positive segments in the SAME
///   category were unaffected). [`StackOffset::Diverging`] reproduces
///   this exact branch selection; [`StackOffset::Zero`]/`Expand` poison
///   forward from that value's own position in accumulation order
///   instead (there is no positive/negative split to preserve there).
/// - [`MissingDataPolicy::Error`] — [`TransformError::NonFinite`] at
///   `series_index * category_count + category_index` (a 2-D input has
///   no single natural flat index; this module documents its own
///   flattening rather than reusing another function's 1-D convention).
///
/// **Empty input**: `series.is_empty()` or `category_count == 0` ->
/// `Ok(vec![Vec::new(); series.len()])` (every series present with zero
/// categories, never a panic).
pub fn stack(
    series: &[&[f64]],
    category_count: usize,
    order: StackOrder,
    offset: StackOffset,
    missing: MissingDataPolicy,
) -> Result<Vec<Vec<(f64, f64)>>, TransformError> {
    let mut out: Vec<Vec<(f64, f64)>> = vec![Vec::with_capacity(category_count); series.len()];
    if series.is_empty() || category_count == 0 {
        return Ok(out);
    }

    if missing == MissingDataPolicy::Error {
        for (j, s) in series.iter().enumerate() {
            for (i, &v) in s.iter().enumerate() {
                if !v.is_finite() {
                    return Err(TransformError::NonFinite { index: j * category_count + i });
                }
            }
        }
    }

    let stack_order = resolve_order(series, category_count, order, missing);

    for category in 0..category_count {
        let mut pos_acc = 0.0_f64;
        let mut neg_acc = 0.0_f64;
        let mut zero_acc = 0.0_f64;
        let mut category_total = 0.0_f64;
        let mut raw: Vec<(usize, f64, f64)> = Vec::with_capacity(stack_order.len()); // (series_index, bottom, top) in stack order

        for &series_index in &stack_order {
            let raw_value = series[series_index].get(category).copied().unwrap_or(0.0);
            let value = resolve_value(raw_value, missing);
            category_total += value;

            let (bottom, top) = match offset {
                StackOffset::Diverging => {
                    // `value >= 0.0` is `false` for a `NaN` value — see
                    // this function's own doc comment for why that's the
                    // deliberate, behavior-preserving branch selection.
                    if value >= 0.0 {
                        let bottom = pos_acc;
                        pos_acc += value;
                        (bottom, pos_acc)
                    } else {
                        let top = neg_acc;
                        neg_acc += value;
                        (neg_acc, top)
                    }
                }
                StackOffset::Zero | StackOffset::Expand => {
                    let bottom = zero_acc;
                    zero_acc += value;
                    (bottom, zero_acc)
                }
            };
            raw.push((series_index, bottom, top));
        }

        for (series_index, bottom, top) in raw {
            let (bottom, top) = if offset == StackOffset::Expand {
                if category_total == 0.0 {
                    (f64::NAN, f64::NAN)
                } else {
                    (bottom / category_total, top / category_total)
                }
            } else {
                (bottom, top)
            };
            out[series_index].push((bottom, top));
        }
    }

    Ok(out)
}

/// Apply `missing` to a single value already selected out of a series
/// (Skip -> `0.0`, matching a missing-index's own contribution; every
/// other policy passes the raw value through unchanged — `Propagate`
/// lets it poison the accumulator per this function's own doc comment,
/// `Error` has already been rejected up front by [`stack`] itself before
/// this is ever called).
fn resolve_value(value: f64, missing: MissingDataPolicy) -> f64 {
    match missing {
        MissingDataPolicy::Skip if !value.is_finite() => 0.0,
        _ => value,
    }
}

/// The series-index PERMUTATION [`stack`] accumulates in, per `order` —
/// the function's own output stays indexed by ORIGINAL series order
/// regardless (see [`stack`]'s own doc comment); this is purely the
/// internal accumulation sequence.
fn resolve_order(series: &[&[f64]], category_count: usize, order: StackOrder, missing: MissingDataPolicy) -> Vec<usize> {
    match order {
        StackOrder::AsGiven => (0..series.len()).collect(),
        StackOrder::Reverse => (0..series.len()).rev().collect(),
        StackOrder::AscendingSum => sort_index_by_value(&series_totals(series, category_count, missing), false),
        StackOrder::DescendingSum => sort_index_by_value(&series_totals(series, category_count, missing), true),
    }
}

/// Each series' own value total across every category, `missing`-aware
/// the same way [`stack`]'s own per-value resolution is (`Skip` treats a
/// non-finite entry as `0.0`; `Propagate` lets it poison that series'
/// own total to `NaN`, which then sorts to one end via
/// [`sort_index_by_value`]'s own documented `NaN`-last convention).
fn series_totals(series: &[&[f64]], category_count: usize, missing: MissingDataPolicy) -> Vec<f64> {
    series
        .iter()
        .map(|s| (0..category_count).map(|i| resolve_value(s.get(i).copied().unwrap_or(0.0), missing)).sum())
        .collect()
}

/// Item 8 — a small sorting/ordering helper that fell out naturally while
/// implementing [`StackOrder::AscendingSum`]/[`DescendingSum`]: the
/// indices of `values`, sorted by VALUE (not by index), ascending or
/// descending. Exposed publicly since it's directly useful beyond
/// stacking too — the common "sort a categorical axis by its own value"
/// need (a caller reorders its OWN `categories`/`values` slices by
/// indexing through the returned permutation).
///
/// `NaN` values sort LAST regardless of `descending` (there is no total
/// order to place a `NaN` correctly relative to a real number — pushing
/// every `NaN` to the end, rather than letting the comparator's own
/// `unwrap_or(Equal)` fallback scatter them unpredictably through the
/// middle, is the one deterministic, well-defined placement). Stable for
/// equal values (ties keep their original relative order, `slice::sort_by`'s
/// own guarantee).
pub fn sort_index_by_value(values: &[f64], descending: bool) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_by(|&a, &b| {
        let (va, vb) = (values[a], values[b]);
        match (va.is_nan(), vb.is_nan()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            (false, false) => {
                let ord = va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal);
                if descending {
                    ord.reverse()
                } else {
                    ord
                }
            }
        }
    });
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diverging_default_reproduces_positive_up_negative_down_independently() {
        let a: &[f64] = &[10.0, -10.0];
        let b: &[f64] = &[20.0, -20.0];
        let c: &[f64] = &[5.0, 5.0];
        let segments = stack(&[a, b, c], 2, StackOrder::AsGiven, StackOffset::Diverging, MissingDataPolicy::Propagate).unwrap();
        // Category 0: a=10 (pos 0->10), b=20 (pos 10->30), c=5 (pos 30->35).
        assert_eq!(segments[0][0], (0.0, 10.0));
        assert_eq!(segments[1][0], (10.0, 30.0));
        assert_eq!(segments[2][0], (30.0, 35.0));
        // Category 1: a=-10 (neg -10->0), b=-20 (neg -30->-10), c=5 (pos 0->5).
        assert_eq!(segments[0][1], (-10.0, 0.0));
        assert_eq!(segments[1][1], (-30.0, -10.0));
        assert_eq!(segments[2][1], (0.0, 5.0));
    }

    #[test]
    fn output_is_always_indexed_by_original_series_order_regardless_of_stack_order() {
        let a: &[f64] = &[1.0];
        let b: &[f64] = &[100.0];
        let as_given = stack(&[a, b], 1, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        let reversed = stack(&[a, b], 1, StackOrder::Reverse, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        // out[0] is ALWAYS series `a`'s own segments regardless of order.
        assert_eq!(as_given[0][0], (0.0, 1.0), "AsGiven: a stacks first, [0,1]");
        assert_eq!(reversed[0][0], (100.0, 101.0), "Reverse: a stacks SECOND now, but out[0] is still a's own segment");
        assert_eq!(reversed[1][0], (0.0, 100.0), "Reverse: b stacks first, [0,100]");
    }

    #[test]
    fn zero_offset_accumulates_sequentially_regardless_of_sign() {
        let a: &[f64] = &[10.0];
        let b: &[f64] = &[-3.0];
        let c: &[f64] = &[5.0];
        let segments = stack(&[a, b, c], 1, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(segments[0][0], (0.0, 10.0));
        assert_eq!(segments[1][0], (10.0, 7.0), "a negative value SHRINKS the running total under Zero, never starts a new downward stack");
        assert_eq!(segments[2][0], (7.0, 12.0));
    }

    #[test]
    fn expand_offset_normalizes_each_category_to_sum_to_one() {
        let a: &[f64] = &[10.0, 1.0];
        let b: &[f64] = &[30.0, 1.0];
        let segments = stack(&[a, b], 2, StackOrder::AsGiven, StackOffset::Expand, MissingDataPolicy::Propagate).unwrap();
        // Category 0: total 40 -> a is [0, 0.25], b is [0.25, 1.0].
        assert!((segments[0][0].1 - 0.25).abs() < 1e-9);
        assert!((segments[1][0].1 - 1.0).abs() < 1e-9);
        // Category 1: total 2 -> a is [0, 0.5], b is [0.5, 1.0].
        assert!((segments[0][1].1 - 0.5).abs() < 1e-9);
        assert!((segments[1][1].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn expand_offset_with_a_zero_total_category_produces_nan_not_a_panic() {
        let a: &[f64] = &[0.0];
        let b: &[f64] = &[0.0];
        let segments = stack(&[a, b], 1, StackOrder::AsGiven, StackOffset::Expand, MissingDataPolicy::Propagate).unwrap();
        assert!(segments[0][0].0.is_nan() && segments[0][0].1.is_nan());
    }

    #[test]
    fn ascending_and_descending_sum_order_the_stack_by_each_series_own_total() {
        let small: &[f64] = &[1.0, 1.0];
        let big: &[f64] = &[50.0, 50.0];
        let medium: &[f64] = &[10.0, 10.0];
        let ascending = stack(&[small, big, medium], 2, StackOrder::AscendingSum, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        // Ascending: small (total 2) stacks first [0,1], medium (20) next [1,11], big (100) last [11,61].
        assert_eq!(ascending[0][0], (0.0, 1.0), "small must stack first (closest to baseline) under AscendingSum");
        assert_eq!(ascending[2][0], (1.0, 11.0), "medium must stack second");
        assert_eq!(ascending[1][0], (11.0, 61.0), "big must stack last (farthest from baseline)");

        let descending = stack(&[small, big, medium], 2, StackOrder::DescendingSum, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(descending[1][0], (0.0, 50.0), "big must stack first under DescendingSum");
    }

    #[test]
    fn a_category_index_missing_from_a_shorter_series_contributes_zero() {
        let short: &[f64] = &[10.0];
        let long: &[f64] = &[5.0, 5.0];
        let segments = stack(&[short, long], 2, StackOrder::AsGiven, StackOffset::Diverging, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(segments[0][1], (0.0, 0.0), "short's own missing category-1 entry contributes nothing");
        assert_eq!(segments[1][1], (0.0, 5.0));
    }

    #[test]
    fn empty_series_or_zero_categories_never_panics() {
        let empty_series: &[&[f64]] = &[];
        assert_eq!(stack(empty_series, 3, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap(), Vec::<Vec<(f64, f64)>>::new());
        let a: &[f64] = &[1.0, 2.0];
        let out = stack(&[a], 0, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Propagate).unwrap();
        assert_eq!(out, vec![Vec::new()]);
    }

    #[test]
    fn skip_policy_treats_a_non_finite_value_as_zero_contribution() {
        let a: &[f64] = &[f64::NAN];
        let b: &[f64] = &[5.0];
        let segments = stack(&[a, b], 1, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Skip).unwrap();
        assert_eq!(segments[0][0], (0.0, 0.0));
        assert_eq!(segments[1][0], (0.0, 5.0));
    }

    #[test]
    fn propagate_policy_nan_falls_into_the_negative_accumulator_under_diverging_the_documented_pre_existing_quirk() {
        let a: &[f64] = &[f64::NAN];
        let segments = stack(&[a], 1, StackOrder::AsGiven, StackOffset::Diverging, MissingDataPolicy::Propagate).unwrap();
        let (bottom, top) = segments[0][0];
        assert!(bottom.is_nan() && top == 0.0, "NaN >= 0.0 is false, so it must land in the NEGATIVE accumulator branch, matching the pre-refactor behavior");
    }

    #[test]
    fn error_policy_reports_the_flattened_series_and_category_index() {
        let a: &[f64] = &[1.0, 2.0];
        let b: &[f64] = &[3.0, f64::NAN];
        let err = stack(&[a, b], 2, StackOrder::AsGiven, StackOffset::Zero, MissingDataPolicy::Error).unwrap_err();
        // series_index=1, category_count=2, category_index=1 -> flat index 3.
        assert_eq!(err, TransformError::NonFinite { index: 3 });
    }

    // ── sort_index_by_value ──────────────────────────────────────────

    #[test]
    fn sort_index_by_value_ascending_and_descending() {
        let values = [30.0, 10.0, 20.0];
        assert_eq!(sort_index_by_value(&values, false), vec![1, 2, 0]);
        assert_eq!(sort_index_by_value(&values, true), vec![0, 2, 1]);
    }

    #[test]
    fn sort_index_by_value_pushes_nan_to_the_end_regardless_of_direction() {
        let values = [5.0, f64::NAN, 1.0];
        assert_eq!(sort_index_by_value(&values, false), vec![2, 0, 1]);
        assert_eq!(sort_index_by_value(&values, true), vec![0, 2, 1]);
    }

    #[test]
    fn sort_index_by_value_empty_is_empty() {
        assert!(sort_index_by_value(&[], false).is_empty());
    }
}
