//! `SymlogScale` — symmetric-log domain: linear near zero (within a
//! configurable [`SymlogScale::linear_threshold`]), logarithmic beyond it,
//! on BOTH sides of zero.
//!
//! [`LogScale`](super::LogScale) cannot represent data that spans zero or
//! goes negative (it floors every value at a tiny positive epsilon,
//! collapsing an entire negative/near-zero range to one screen position) —
//! exactly the shape of a money-flow, P&L, or delta series that swings
//! from large negative to large positive across orders of magnitude. This
//! scale is the standard fix (d3's own `scaleSymlog`, Bostock's
//! ["A Better Symlog Scale for D3"](https://observablehq.com/@d3/symlog)):
//! the domain <-> normalized-range transform is
//! `f(v) = sign(v) * ln(1 + |v| / C)`, where `C` is
//! [`SymlogScale::linear_threshold`] — inside `[-C, C]` the transform is
//! nearly linear (`ln(1+x) ≈ x` for small `x`), outside it grows
//! logarithmically, and `f` is continuous and strictly increasing through
//! zero (no floor, no epsilon, no undefined `log(negative)`).
//!
//! Tick generation splits the domain into up to three zones — a negative
//! log tail (`< -C`), a linear middle (`[-C, C] ∩ [min, max]`), and a
//! positive log tail (`> C`) — decade ticks ([`decade_ticks`]) in the log
//! zones (matching [`LogScale`](super::LogScale)'s own decade-boundary
//! convention), nice-number ticks ([`linear_zone_ticks`], reusing
//! [`super::linear::nice_step`]) in the linear zone, and always includes
//! `0.0` when the domain actually spans it (`min <= 0.0 <= max`) — the one
//! tick a symlog axis must never omit, since it's the whole reason this
//! scale kind exists.

use super::linear::{format_value, nice_step};
use super::{Scale, Tick};

/// Default `C` (the linear/log crossover magnitude) — matches d3's own
/// `scaleSymlog` default of `1`.
const DEFAULT_LINEAR_THRESHOLD: f64 = 1.0;

/// Symmetric-log scale over `[min, max]` (may straddle zero, may be
/// entirely negative, may be entirely positive — every case is handled,
/// see this module's own tick-zone reasoning above).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymlogScale {
    pub min: f64,
    pub max: f64,
    /// The linear/log crossover magnitude `C` — `|v| < linear_threshold`
    /// transforms near-linearly, `|v| >= linear_threshold` transforms
    /// logarithmically. Always a finite positive number (a non-positive or
    /// non-finite constructor argument falls back to
    /// [`DEFAULT_LINEAR_THRESHOLD`] — see [`SymlogScale::with_linear_threshold`]).
    pub linear_threshold: f64,
}

impl SymlogScale {
    /// `linear_threshold` = [`DEFAULT_LINEAR_THRESHOLD`] (`1.0`, d3's own default).
    pub fn new(min: f64, max: f64) -> Self {
        Self::with_linear_threshold(min, max, DEFAULT_LINEAR_THRESHOLD)
    }

    /// Explicit linear/log crossover magnitude — e.g. a P&L series in
    /// whole dollars might want `linear_threshold: 100.0` so swings under
    /// $100 read on a linear scale and only larger swings compress
    /// logarithmically.
    pub fn with_linear_threshold(min: f64, max: f64, linear_threshold: f64) -> Self {
        let linear_threshold = if linear_threshold.is_finite() && linear_threshold > 0.0 { linear_threshold } else { DEFAULT_LINEAR_THRESHOLD };
        Self { min, max, linear_threshold }
    }

    fn transform(&self, v: f64) -> f64 {
        v.signum() * (1.0 + v.abs() / self.linear_threshold).ln()
    }

    fn inverse_transform(&self, t: f64) -> f64 {
        t.signum() * (t.abs().exp() - 1.0) * self.linear_threshold
    }
}

impl Scale for SymlogScale {
    fn domain(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    fn map(&self, v: f64) -> f64 {
        let (t_min, t_max) = (self.transform(self.min), self.transform(self.max));
        let range = t_max - t_min;
        if range.abs() < f64::EPSILON {
            return 0.5;
        }
        (self.transform(v) - t_min) / range
    }

    fn invert(&self, t: f64) -> f64 {
        let (t_min, t_max) = (self.transform(self.min), self.transform(self.max));
        self.inverse_transform(t_min + t * (t_max - t_min))
    }

    fn ticks(&self, target_count: usize) -> Vec<Tick> {
        let (min, max) = (self.min, self.max);
        if !min.is_finite() || !max.is_finite() || min >= max {
            let v = if min.is_finite() { min } else { 0.0 };
            return vec![Tick { value: v, label: format_value(v, 1.0) }];
        }
        let lt = self.linear_threshold;
        let mut values: Vec<f64> = Vec::new();

        // Negative log tail: the portion of the domain strictly beyond
        // -C on the negative side. `neg_zone_hi` is the LESS-negative
        // (closer to zero) edge of that tail — either -C itself, or the
        // domain's own `max` when the WHOLE domain sits deep in negative
        // territory (max < -C).
        let neg_zone_hi = max.min(-lt);
        if min < neg_zone_hi {
            values.extend(decade_ticks(neg_zone_hi.abs(), min.abs(), -1.0));
        }

        // Positive log tail: the mirror image.
        let pos_zone_lo = min.max(lt);
        if max > pos_zone_lo {
            values.extend(decade_ticks(pos_zone_lo, max, 1.0));
        }

        // Linear middle zone: `[-C, C]` intersected with `[min, max]`.
        let lin_lo = min.max(-lt);
        let lin_hi = max.min(lt);
        if lin_hi > lin_lo {
            values.extend(linear_zone_ticks(lin_lo, lin_hi, target_count));
        } else if (lin_hi - lin_lo).abs() < 1e-12 {
            values.push(lin_lo);
        }

        // The zero crossing is always present when the domain actually
        // spans it — the one tick this scale kind exists to never omit.
        if min <= 0.0 && max >= 0.0 && !values.iter().any(|v| v.abs() < 1e-9) {
            values.push(0.0);
        }

        values.retain(|v| *v >= min - 1e-9 && *v <= max + 1e-9);
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        values.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        values.into_iter().map(|v| Tick { value: v, label: format_value(v, tick_step_for(v, lt)) }).collect()
    }
}

/// Decimal precision for a tick at magnitude `v` — inside the linear zone
/// (`|v| < C`), format at `C`'s own precision; in a log zone, format at
/// the precision of that value's own decade (`10^floor(log10(|v|))`),
/// matching [`super::LogScale::ticks`]'s own `format_value(value, base)`
/// convention.
fn tick_step_for(v: f64, linear_threshold: f64) -> f64 {
    if v == 0.0 || v.abs() < linear_threshold {
        linear_threshold.max(f64::EPSILON)
    } else {
        10.0_f64.powf(v.abs().log10().floor())
    }
}

/// Decade tick magnitudes (`1, 10, 100, ...`) landing within
/// `[lo_abs, hi_abs]` (both `>= 0`, caller-guaranteed `hi_abs >= lo_abs`),
/// each multiplied by `sign` (`1.0`/`-1.0`) — [`SymlogScale::ticks`]'s own
/// log-zone generator, deliberately decade-only (no 2x/5x subdivision like
/// [`super::LogScale::ticks`] adds for a narrow domain) since a symlog log
/// tail is a secondary zone alongside the linear middle, not the whole
/// axis.
fn decade_ticks(lo_abs: f64, hi_abs: f64, sign: f64) -> Vec<f64> {
    if lo_abs <= 0.0 || hi_abs < lo_abs {
        return Vec::new();
    }
    let d0 = lo_abs.log10().floor() as i32;
    let d1 = hi_abs.log10().ceil() as i32;
    let mut out = Vec::new();
    for d in d0..=d1 {
        let mag = 10.0_f64.powi(d);
        if mag + 1e-9 >= lo_abs && mag - 1e-9 <= hi_abs {
            out.push(sign * mag);
        }
    }
    out
}

/// Nice-number ticks over `[lo, hi]` (the linear zone) — the same
/// index-based `nice_step` walk [`super::LinearScale::ticks`] uses,
/// bounded to this sub-range rather than the whole domain.
fn linear_zone_ticks(lo: f64, hi: f64, target_count: usize) -> Vec<f64> {
    let range = hi - lo;
    if range <= 0.0 {
        return vec![lo];
    }
    let step = nice_step(range, target_count.max(1) as f64);
    if step <= 0.0 {
        return vec![lo, hi];
    }
    let first = (lo / step).ceil() * step;
    let count = (((hi - first) / step).ceil() as i64 + 1).max(0);
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let value = first + i as f64 * step;
        if value > hi + step * 1e-9 {
            break;
        }
        out.push(value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_domain_maps_zero_to_the_exact_center() {
        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        assert!((scale.map(0.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn map_and_invert_round_trip_across_a_wide_signed_domain() {
        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        for v in [-1_000_000.0, -1_000.0, -1.0, 0.0, 1.0, 1_000.0, 1_000_000.0] {
            let t = scale.map(v);
            let back = scale.invert(t);
            assert!((back - v).abs() < 1e-6, "round trip failed for {v}: got {back} via t={t}");
        }
    }

    #[test]
    fn map_is_monotonically_increasing_across_the_whole_signed_domain() {
        let scale = SymlogScale::new(-500_000.0, 500_000.0);
        let sample_points: Vec<f64> = (-20..=20).map(|i| (i as f64) * 25_000.0).collect();
        let mapped: Vec<f64> = sample_points.iter().map(|&v| scale.map(v)).collect();
        for w in mapped.windows(2) {
            assert!(w[1] > w[0], "map() must be strictly increasing through zero, got {mapped:?}");
        }
    }

    #[test]
    fn negative_only_domain_does_not_panic_and_round_trips() {
        let scale = SymlogScale::new(-10_000.0, -1.0);
        for v in [-10_000.0, -100.0, -1.0] {
            let t = scale.map(v);
            assert!(t.is_finite());
            let back = scale.invert(t);
            assert!((back - v).abs() < 1e-6);
        }
    }

    #[test]
    fn positive_only_domain_behaves_like_a_smooth_log_like_ramp() {
        let scale = SymlogScale::new(1.0, 10_000.0);
        assert!((scale.map(1.0) - 0.0).abs() < 1e-9);
        assert!((scale.map(10_000.0) - 1.0).abs() < 1e-9);
        assert!(scale.map(100.0) > 0.0 && scale.map(100.0) < 1.0);
    }

    #[test]
    fn degenerate_domain_does_not_panic() {
        let scale = SymlogScale::new(5.0, 5.0);
        assert_eq!(scale.map(5.0), 0.5);
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
    }

    #[test]
    fn empty_or_reversed_domain_does_not_panic() {
        let scale = SymlogScale::new(10.0, -10.0); // min > max
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
        assert!(scale.map(0.0).is_finite());
    }

    #[test]
    fn non_positive_linear_threshold_falls_back_to_the_default() {
        let scale = SymlogScale::with_linear_threshold(-100.0, 100.0, -5.0);
        assert_eq!(scale.linear_threshold, DEFAULT_LINEAR_THRESHOLD);
        let scale = SymlogScale::with_linear_threshold(-100.0, 100.0, f64::NAN);
        assert_eq!(scale.linear_threshold, DEFAULT_LINEAR_THRESHOLD);
    }

    #[test]
    fn zero_crossing_tick_is_always_present_when_the_domain_spans_zero() {
        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        let ticks = scale.ticks(6);
        assert!(ticks.iter().any(|t| t.value.abs() < 1e-9), "expected a 0.0 tick, got {ticks:?}");
    }

    #[test]
    fn zero_crossing_tick_is_absent_when_the_domain_does_not_span_zero() {
        let scale = SymlogScale::new(10.0, 1_000.0);
        let ticks = scale.ticks(6);
        assert!(!ticks.iter().any(|t| t.value.abs() < 1e-9), "domain [10, 1000] never contains 0, no tick should land there");
    }

    #[test]
    fn wide_domain_produces_decade_ticks_in_both_log_tails() {
        let scale = SymlogScale::new(-1_000_000.0, 1_000_000.0);
        let ticks = scale.ticks(6);
        let has_positive_decade = ticks.iter().any(|t| (t.value - 1_000.0).abs() < 1e-6);
        let has_negative_decade = ticks.iter().any(|t| (t.value - (-1_000.0)).abs() < 1e-6);
        assert!(has_positive_decade, "expected a positive decade tick (1000), got {ticks:?}");
        assert!(has_negative_decade, "expected a negative decade tick (-1000), got {ticks:?}");
    }

    #[test]
    fn narrow_domain_within_the_linear_threshold_produces_only_linear_zone_ticks() {
        let scale = SymlogScale::with_linear_threshold(-0.5, 0.5, 1.0);
        let ticks = scale.ticks(5);
        assert!(!ticks.is_empty());
        for t in &ticks {
            assert!(t.value.abs() <= 0.5 + 1e-9, "a domain entirely inside the linear threshold must produce only linear-zone ticks, got {t:?}");
        }
    }

    #[test]
    fn every_tick_lands_within_the_scale_domain() {
        let scale = SymlogScale::new(-250.0, 900_000.0);
        for t in scale.ticks(8) {
            assert!(t.value >= scale.min - 1e-6 && t.value <= scale.max + 1e-6, "tick {t:?} escapes domain [{}, {}]", scale.min, scale.max);
        }
    }

    #[test]
    fn tick_weight_defaults_to_none() {
        let scale = SymlogScale::new(-100.0, 100.0);
        assert_eq!(scale.tick_weight(0.0), None);
    }
}
