//! `LinearScale` — continuous linear domain, plus the "nice number" tick
//! algorithm this whole crate's tick generation is built on.
//!
//! Ported and generalized from `mylittlechart`'s `PriceScale` nice-number
//! ladder (`mlc-core/src/chart/types/price_scale.rs:113-207`) — same
//! `[2, 2.5, 2]` multiplier walk, same index-based tick loop (avoids
//! float-accumulation drift over many ticks), with all "price" vocabulary
//! removed: this is generic axis-tick math, not a trading concept.

use super::{Scale, Tick, TickPriority};

/// Multiplier ladder for "nice" tick steps: walking `base * [2, 2.5, 2]`
/// repeatedly produces the familiar 1, 2, 5, 10, 20, 50, 100, ... cadence.
pub const NICE_MULTIPLIERS: [f64; 3] = [2.0, 2.5, 2.0];

/// Round `value` up to the nearest "nice" number using the `[2, 2.5, 2]`
/// multiplier ladder (produces intervals like 1, 2, 5, 10, 20, 50).
///
/// Non-positive input has no meaningful nice step — returns `1.0`.
pub fn nice_number(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 1.0;
    }

    let exp = value.log10().floor();
    let base = 10.0_f64.powf(exp);

    let mut current = base;
    let mut idx = 0;
    while current < value {
        current *= NICE_MULTIPLIERS[idx % 3];
        idx += 1;
        if idx > 10 {
            break; // safety valve — unreachable for any finite f64 magnitude
        }
    }

    // Prefer the step one rung down if it's still close enough (within 20%)
    // to the target — avoids consistently overshooting into a coarser step
    // than the data actually needs.
    if idx > 0 {
        let prev_idx = idx - 1;
        let mut check = base;
        for i in 0..prev_idx {
            check *= NICE_MULTIPLIERS[i % 3];
        }
        if check >= value * 0.8 {
            return check;
        }
    }

    current
}

/// Nice step size for `range` split into roughly `target_ticks` intervals.
pub fn nice_step(range: f64, target_ticks: f64) -> f64 {
    let target_ticks = if target_ticks > 0.0 { target_ticks } else { 1.0 };
    nice_number(range / target_ticks)
}

/// Expand `[data_min, data_max]` to nice round bounds that comfortably
/// contain it, sized for roughly `target_ticks` tick marks.
///
/// Degenerate input (`min >= max`, or non-finite) falls back to a unit
/// domain anchored at `data_min` (or `0.0`) so callers never see NaN/inf.
pub fn nice_domain(data_min: f64, data_max: f64, target_ticks: usize) -> (f64, f64) {
    if !data_min.is_finite() || !data_max.is_finite() || data_min >= data_max {
        let anchor = if data_min.is_finite() { data_min } else { 0.0 };
        return (anchor, anchor + 1.0);
    }
    let step = nice_step(data_max - data_min, target_ticks.max(1) as f64);
    if step <= 0.0 {
        return (data_min, data_max);
    }
    let nice_min = (data_min / step).floor() * step;
    let nice_max = (data_max / step).ceil() * step;
    (nice_min, nice_max)
}

/// Decimal precision to display a value stepping by `step`.
///
/// Formula-equivalent port of `price_precision`'s explicit match ladder:
/// any step `>= 0.01` shows 2 decimals; each decade below that adds one
/// more digit, capped at 12 (guards against a pathologically tiny step
/// producing an absurd label).
pub fn decimal_precision(step: f64) -> usize {
    if step <= 0.0 || !step.is_finite() || step >= 0.01 {
        return 2;
    }
    let exp = step.log10().floor();
    ((-exp) as usize).min(12)
}

/// Group the integer part of a formatted decimal string with `,` every 3
/// digits (`"1234567.89"` -> `"1,234,567.89"`). Handles an optional
/// leading `-`.
fn group_thousands(formatted: &str) -> String {
    let (sign, rest) = match formatted.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", formatted),
    };
    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };

    let bytes = int_part.as_bytes();
    let mut grouped = String::with_capacity(bytes.len() + bytes.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }

    match frac_part {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Format `value` at the decimal precision implied by tick `step`, with
/// thousands separators on the integer part.
pub fn format_value(value: f64, step: f64) -> String {
    let precision = decimal_precision(step);
    let formatted = format!("{value:.precision$}");
    group_thousands(&formatted)
}

/// Continuous linear scale over `[min, max]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearScale {
    pub min: f64,
    pub max: f64,
}

impl LinearScale {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// Build a scale over a nice-rounded domain that comfortably contains
    /// `[data_min, data_max]` — the usual way to construct an axis scale
    /// from raw data extents.
    pub fn nice(data_min: f64, data_max: f64, target_ticks: usize) -> Self {
        let (min, max) = nice_domain(data_min, data_max, target_ticks);
        Self { min, max }
    }

    fn range(&self) -> f64 {
        self.max - self.min
    }
}

impl Scale for LinearScale {
    fn domain(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    fn map(&self, v: f64) -> f64 {
        let range = self.range();
        if range.abs() < f64::EPSILON {
            return 0.5;
        }
        (v - self.min) / range
    }

    fn invert(&self, t: f64) -> f64 {
        self.min + t * self.range()
    }

    fn ticks(&self, target_count: usize) -> Vec<Tick> {
        let range = self.range();
        if range.abs() < f64::EPSILON {
            return vec![Tick { value: self.min, label: format_value(self.min, 1.0) }];
        }

        let step = nice_step(range, target_count.max(1) as f64);
        if step <= 0.0 {
            return Vec::new();
        }

        // Index-based iteration (not `price += step` accumulation) — same
        // discipline as the mlc source, keeps ticks evenly spaced even
        // after many steps.
        let first = (self.min / step).ceil() * step;
        let count = (((self.max - first) / step).ceil() as i64 + 1).max(0);
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let value = first + (i as f64) * step;
            if value > self.max + step * 1e-9 {
                break;
            }
            out.push(Tick { value, label: format_value(value, step) });
        }
        out
    }

    /// `0.0` is [`TickPriority::Major`] WHEN it's an actually-generated
    /// tick value AND this scale's own domain genuinely spans zero — the
    /// zero baseline is a real anchor a caller may reasonably not want
    /// silently dropped, but (unlike [`crate::scale::SymlogScale`], whose
    /// entire reason for existing is keeping zero legible) this is an
    /// OPT-IN-by-construction protection, not a guaranteed-present tick:
    /// a `LinearScale` never manufactures a zero tick that its own
    /// `ticks()` wouldn't otherwise generate. Every other value is
    /// [`TickPriority::Minor`] — this override changes NOTHING for a
    /// domain that never crosses zero, or for any tick other than zero
    /// itself.
    fn tick_priority(&self, v: f64) -> TickPriority {
        if v.abs() < 1e-9 && self.min <= 0.0 && self.max >= 0.0 {
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
    fn nice_number_ladder_values() {
        // 7 rounds up into the 5..10 rung, 23 into the 20..25 rung — same
        // assertions as the source mlc test, generalized naming only.
        let nice = nice_number(7.0);
        assert!((5.0..=10.0).contains(&nice));
        let nice = nice_number(23.0);
        assert!((20.0..=25.0).contains(&nice));
        // Non-positive input has no nice step.
        assert_eq!(nice_number(0.0), 1.0);
        assert_eq!(nice_number(-5.0), 1.0);
    }

    #[test]
    fn decimal_precision_matches_step_magnitude() {
        assert_eq!(decimal_precision(10.0), 2);
        assert_eq!(decimal_precision(1.0), 2);
        assert_eq!(decimal_precision(0.5), 2);
        assert_eq!(decimal_precision(0.05), 2);
        assert_eq!(decimal_precision(0.005), 3);
        assert_eq!(decimal_precision(0.0005), 4);
    }

    #[test]
    fn format_value_groups_thousands() {
        assert_eq!(format_value(1_234_567.891, 1.0), "1,234,567.89");
        assert_eq!(format_value(-1_234.5, 1.0), "-1,234.50");
        assert_eq!(format_value(42.0, 1.0), "42.00");
    }

    #[test]
    fn linear_scale_ticks_count_and_rounding() {
        let scale = LinearScale::new(0.0, 100.0);
        let ticks = scale.ticks(5);
        assert!(!ticks.is_empty());
        // Every tick must land within the scale's own domain bounds.
        for t in &ticks {
            assert!(t.value >= scale.min - 1e-9);
            assert!(t.value <= scale.max + 1e-9);
        }
        // Roughly the requested density — not an exact count contract
        // (nice-number rounding can land on 5..11 ticks for a 100-wide
        // range targeting 5), but it must not degenerate to 0/1.
        assert!(ticks.len() >= 2);
    }

    #[test]
    fn linear_scale_map_and_invert_round_trip() {
        let scale = LinearScale::new(-50.0, 150.0);
        for v in [-50.0, 0.0, 42.5, 150.0] {
            let t = scale.map(v);
            let back = scale.invert(t);
            assert!((back - v).abs() < 1e-9);
        }
    }

    #[test]
    fn nice_scale_contains_data_extent() {
        let scale = LinearScale::nice(3.0, 97.0, 5);
        assert!(scale.min <= 3.0);
        assert!(scale.max >= 97.0);
    }

    #[test]
    fn degenerate_domain_does_not_panic() {
        let scale = LinearScale::new(5.0, 5.0);
        assert_eq!(scale.map(5.0), 0.5);
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
    }

    // ── tick_priority (opt-in zero anchor) ──────────────────────────────

    #[test]
    fn zero_reports_major_priority_when_the_domain_spans_it() {
        let scale = LinearScale::new(-100.0, 100.0);
        assert_eq!(scale.tick_priority(0.0), TickPriority::Major);
    }

    #[test]
    fn zero_reports_minor_priority_when_the_domain_does_not_span_it() {
        let scale = LinearScale::new(10.0, 100.0);
        assert_eq!(scale.tick_priority(0.0), TickPriority::Minor, "zero is not even in this domain — must not be elevated");
    }

    #[test]
    fn non_zero_values_always_report_minor_priority() {
        let scale = LinearScale::new(-100.0, 100.0);
        assert_eq!(scale.tick_priority(50.0), TickPriority::Minor);
        assert_eq!(scale.tick_priority(-50.0), TickPriority::Minor);
    }
}
