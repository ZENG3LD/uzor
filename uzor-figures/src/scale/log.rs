//! `LogScale` — log10 domain mapping, guarded against non-positive
//! domains.
//!
//! Ported and generalized from `PriceScale`'s `Logarithmic` mode dispatch
//! (`mlc-core/src/chart/types/price_scale.rs:502-519` for the map,
//! `604-635` for tick generation) — same log10-space linear interpolation,
//! same non-positive-domain floor, "price" vocabulary removed.

use super::linear::format_value;
use super::{Scale, Tick, TickPriority};

/// Floor applied to any domain value before taking its log10 — guards
/// `log(0)`/`log(negative)` without needing callers to pre-validate their
/// data. Deliberately much smaller than mlc's `0.0001` price floor (which
/// was itself a price-specific magnitude): this is a generic value axis
/// and may legitimately hold small positive numbers.
const LOG_SCALE_MIN_POSITIVE: f64 = 1e-9;

/// Continuous logarithmic (base-10) scale over `[min, max]`.
///
/// `domain()` returns the caller-supplied bounds verbatim (even if
/// non-positive) — all log-space math clamps internally via
/// [`LogScale::safe_min`]/[`LogScale::safe_max`], so a domain that dips to
/// or below zero degrades to "everything maps near the floor" instead of
/// panicking or producing NaN.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogScale {
    pub min: f64,
    pub max: f64,
}

impl LogScale {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    fn safe_min(&self) -> f64 {
        self.min.max(LOG_SCALE_MIN_POSITIVE)
    }

    fn safe_max(&self) -> f64 {
        self.max.max(self.safe_min() + LOG_SCALE_MIN_POSITIVE)
    }

    fn log_min(&self) -> f64 {
        self.safe_min().log10()
    }

    fn log_max(&self) -> f64 {
        self.safe_max().log10()
    }
}

impl Scale for LogScale {
    fn domain(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    fn map(&self, v: f64) -> f64 {
        let log_range = self.log_max() - self.log_min();
        if log_range <= 0.0 {
            return 0.5;
        }
        let safe_v = v.max(LOG_SCALE_MIN_POSITIVE);
        (safe_v.log10() - self.log_min()) / log_range
    }

    fn invert(&self, t: f64) -> f64 {
        let log_range = self.log_max() - self.log_min();
        10.0_f64.powf(self.log_min() + t * log_range)
    }

    fn ticks(&self, target_count: usize) -> Vec<Tick> {
        let log_min = self.log_min();
        let log_max = self.log_max();
        if !(log_max > log_min) {
            let v = self.safe_min();
            return vec![Tick { value: v, label: format_value(v, v) }];
        }

        let decade_start = log_min.floor() as i64;
        let decade_end = log_max.ceil() as i64;
        let num_decades = (decade_end - decade_start).max(1) as usize;

        // Few decades in view -> subdivide each decade at 2x/5x so there's
        // still a reasonable tick density; many decades -> decade marks
        // alone already meet or exceed the target, so skip subdivision.
        let subdivisions: &[f64] =
            if num_decades >= target_count.max(1) { &[1.0] } else { &[1.0, 2.0, 5.0] };

        // Filter against the SAFE bounds (same ones `log_min`/`log_max`
        // were derived from), not the raw `self.min`/`self.max` — a
        // non-positive raw domain has already been floored into log
        // space, so filtering against the original (possibly negative)
        // bounds would reject every candidate tick.
        let filter_min = self.safe_min();
        let filter_max = self.safe_max();

        let mut out = Vec::new();
        for decade in decade_start..=decade_end {
            let base = 10.0_f64.powi(decade as i32);
            for &mult in subdivisions {
                let value = base * mult;
                if value + 1e-12 < filter_min || value - 1e-12 > filter_max {
                    continue;
                }
                out.push(Tick { value, label: format_value(value, base) });
            }
        }
        out
    }

    /// A DECADE boundary (an exact power of ten — `mult == 1.0` in
    /// [`LogScale::ticks`]' own subdivision loop above) is
    /// [`TickPriority::Major`]; a 2x/5x subdivision tick is
    /// [`TickPriority::Minor`]. No [`TickPriority::Critical`] tier here —
    /// a log scale has no zero-crossing concept (`log(0)` is undefined,
    /// see [`LogScale::safe_min`]).
    fn tick_priority(&self, v: f64) -> TickPriority {
        if v <= 0.0 || !v.is_finite() {
            return TickPriority::Minor;
        }
        let log10 = v.log10();
        if (log10 - log10.round()).abs() < 1e-6 {
            TickPriority::Major
        } else {
            TickPriority::Minor
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_and_invert_round_trip() {
        let scale = LogScale::new(1.0, 1000.0);
        for v in [1.0, 10.0, 100.0, 1000.0] {
            let t = scale.map(v);
            let back = scale.invert(t);
            assert!((back - v).abs() / v < 1e-9);
        }
    }

    #[test]
    fn non_positive_domain_does_not_panic_or_nan() {
        let scale = LogScale::new(-10.0, 0.0);
        let t = scale.map(-5.0);
        assert!(t.is_finite());
        let ticks = scale.ticks(5);
        assert!(!ticks.is_empty());
        for tick in &ticks {
            assert!(tick.value.is_finite());
        }
    }

    #[test]
    fn ticks_land_on_decade_boundaries_for_wide_domain() {
        let scale = LogScale::new(1.0, 100_000.0);
        let ticks = scale.ticks(4);
        // Wide domain (5 decades) at a small target -> decade-only ticks.
        for tick in &ticks {
            let log10 = tick.value.log10();
            assert!((log10 - log10.round()).abs() < 1e-9);
        }
    }

    #[test]
    fn ticks_subdivide_for_narrow_domain() {
        let scale = LogScale::new(1.0, 10.0);
        let ticks = scale.ticks(6);
        // One decade with a target above the decade count -> subdivisions
        // (1, 2, 5) should appear alongside the decade boundary itself.
        assert!(ticks.iter().any(|t| (t.value - 2.0).abs() < 1e-9));
        assert!(ticks.iter().any(|t| (t.value - 5.0).abs() < 1e-9));
    }

    // ── tick_priority (decade labels survive densification) ────────────

    #[test]
    fn decade_boundaries_report_major_priority() {
        let scale = LogScale::new(1.0, 100_000.0);
        for v in [1.0, 10.0, 100.0, 1_000.0, 10_000.0, 100_000.0] {
            assert_eq!(scale.tick_priority(v), TickPriority::Major, "decade boundary {v} must report Major priority");
        }
    }

    #[test]
    fn subdivision_ticks_report_minor_priority() {
        let scale = LogScale::new(1.0, 10.0);
        for v in [2.0, 5.0] {
            assert_eq!(scale.tick_priority(v), TickPriority::Minor, "a 2x/5x subdivision tick {v} must report Minor priority");
        }
    }

    #[test]
    fn non_positive_value_reports_minor_priority_never_panics() {
        let scale = LogScale::new(1.0, 100.0);
        assert_eq!(scale.tick_priority(-5.0), TickPriority::Minor);
        assert_eq!(scale.tick_priority(0.0), TickPriority::Minor);
    }

    #[test]
    fn a_dense_log_axis_generated_tick_set_contains_both_major_and_minor_priorities() {
        let scale = LogScale::new(1.0, 10.0);
        let ticks = scale.ticks(6);
        let priorities: Vec<TickPriority> = ticks.iter().map(|t| scale.tick_priority(t.value)).collect();
        assert!(priorities.contains(&TickPriority::Major), "expected at least one Major decade tick, got {priorities:?}");
        assert!(priorities.contains(&TickPriority::Minor), "expected at least one Minor subdivision tick, got {priorities:?}");
    }
}
