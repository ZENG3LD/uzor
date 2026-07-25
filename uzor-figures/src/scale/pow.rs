//! `PowScale` — exponent-based domain: normalized position is proportional
//! to `|v|^exponent` (sign-preserving, so a domain crossing zero still
//! maps monotonically), not `v` itself. [`PowScale::sqrt`] (`exponent =
//! 0.5`) is the common case — the standard convention for a scale whose
//! MARK area (not radius/length) should encode the value linearly, e.g. a
//! bubble-chart radius, where a linear radius scale would visually
//! over-emphasize large values (area grows with the SQUARE of radius).

use super::linear::LinearScale;
use super::{Scale, Tick, TickPriority};

/// `exponent = 1.0` (the fallback [`PowScale::new`] uses for a non-finite
/// caller argument) reproduces plain linear mapping exactly — a
/// degenerate-safe default, not a special case this scale's `map`/`invert`
/// need to branch on.
const DEFAULT_EXPONENT: f64 = 1.0;

/// Sign-preserving power scale over `[min, max]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowScale {
    pub min: f64,
    pub max: f64,
    /// The exponent — `0.5` ([`PowScale::sqrt`]) for area-to-linear-value
    /// mapping, `2.0` for the inverse (emphasize large values), `1.0` for
    /// plain linear.
    pub exponent: f64,
}

/// `sign(v) * |v|^exponent` — the sign-preserving power transform this
/// scale's `map`/`invert` both apply, so a domain that crosses zero (e.g.
/// a signed delta with `exponent: 0.5`) still transforms monotonically
/// through zero instead of hitting `(-x)^0.5` (undefined for real numbers).
fn signed_pow(v: f64, exponent: f64) -> f64 {
    v.signum() * v.abs().powf(exponent)
}

/// Inverse of [`signed_pow`] — `sign(y) * |y|^(1/exponent)`.
fn signed_unpow(y: f64, exponent: f64) -> f64 {
    y.signum() * y.abs().powf(1.0 / exponent)
}

impl PowScale {
    /// Non-finite `exponent` falls back to [`DEFAULT_EXPONENT`] (plain
    /// linear) rather than propagating NaN through every `map`/`invert`
    /// call. `exponent == 0.0` is guarded the same way inside `map`/
    /// `invert` themselves (see their own doc comments) — a caller passing
    /// `0.0` here still gets a finite, if degenerate, scale.
    pub fn new(min: f64, max: f64, exponent: f64) -> Self {
        let exponent = if exponent.is_finite() { exponent } else { DEFAULT_EXPONENT };
        Self { min, max, exponent }
    }

    /// The common case — `exponent = 0.5`, area-to-linear-value mapping
    /// (e.g. a bubble-chart radius scale).
    pub fn sqrt(min: f64, max: f64) -> Self {
        Self::new(min, max, 0.5)
    }
}

impl Scale for PowScale {
    fn domain(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    fn map(&self, v: f64) -> f64 {
        let (p_min, p_max) = (signed_pow(self.min, self.exponent), signed_pow(self.max, self.exponent));
        let range = p_max - p_min;
        if range.abs() < f64::EPSILON {
            return 0.5;
        }
        (signed_pow(v, self.exponent) - p_min) / range
    }

    fn invert(&self, t: f64) -> f64 {
        // `exponent == 0.0` makes `1.0 / exponent` infinite — guard by
        // falling back to plain linear interpolation over the RAW domain
        // (never NaN/inf out of a public API), matching every other
        // degenerate guard in this crate's scale layer.
        if self.exponent.abs() < f64::EPSILON {
            return self.min + t * (self.max - self.min);
        }
        let (p_min, p_max) = (signed_pow(self.min, self.exponent), signed_pow(self.max, self.exponent));
        signed_unpow(p_min + t * (p_max - p_min), self.exponent)
    }

    /// Nice-number ticks in DOMAIN space — deliberately delegates to
    /// [`LinearScale::ticks`] over the SAME `[min, max]` rather than
    /// generating ticks in the power-transformed space: an axis needs
    /// readable, evenly-reasoned domain values (`"0, 25, 50, 75, 100"`),
    /// the same convention d3's own `scalePow`/`scaleSqrt` use (both share
    /// `scaleLinear`'s `.nice()`/`.ticks()` implementation) — only the
    /// screen-space MAPPING is nonlinear, not the tick placement.
    fn ticks(&self, target_count: usize) -> Vec<Tick> {
        LinearScale::new(self.min, self.max).ticks(target_count)
    }

    /// Delegates to the SAME [`LinearScale`] over `[min, max]` used by
    /// [`PowScale::ticks`] above — zero is [`TickPriority::Major`] when
    /// it's an actually-generated tick AND the domain spans it, matching
    /// [`LinearScale::tick_priority`]'s own opt-in reasoning.
    fn tick_priority(&self, v: f64) -> TickPriority {
        LinearScale::new(self.min, self.max).tick_priority(v)
    }

    /// Preserves this scale's own `exponent` (a per-scale CONFIG choice,
    /// not part of the domain being windowed) — see [`Scale::windowed`]'s
    /// own doc comment for the seam this serves.
    fn windowed(&self, min: f64, max: f64) -> Option<Box<dyn Scale>> {
        Some(Box::new(PowScale::new(min, max, self.exponent)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_convenience_constructor_sets_exponent_one_half() {
        let scale = PowScale::sqrt(0.0, 100.0);
        assert!((scale.exponent - 0.5).abs() < 1e-9);
    }

    #[test]
    fn boundary_values_map_to_exactly_zero_and_one() {
        let scale = PowScale::sqrt(0.0, 100.0);
        assert!((scale.map(0.0) - 0.0).abs() < 1e-9);
        assert!((scale.map(100.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn map_and_invert_round_trip_for_several_exponents() {
        for exponent in [0.5, 1.0, 2.0, 3.0] {
            let scale = PowScale::new(1.0, 1000.0, exponent);
            for v in [1.0, 10.0, 250.0, 1000.0] {
                let t = scale.map(v);
                let back = scale.invert(t);
                assert!((back - v).abs() / v < 1e-6, "exponent={exponent}: round trip failed for {v}, got {back}");
            }
        }
    }

    #[test]
    fn sqrt_scale_is_monotonically_increasing() {
        let scale = PowScale::sqrt(0.0, 10_000.0);
        let mapped: Vec<f64> = (0..=20).map(|i| scale.map(i as f64 * 500.0)).collect();
        for w in mapped.windows(2) {
            assert!(w[1] > w[0], "PowScale::sqrt must be strictly increasing, got {mapped:?}");
        }
    }

    #[test]
    fn negative_domain_uses_sign_preserving_power_and_round_trips() {
        let scale = PowScale::new(-1000.0, 1000.0, 0.5);
        for v in [-1000.0, -1.0, 0.0, 1.0, 1000.0] {
            let t = scale.map(v);
            assert!(t.is_finite(), "map({v}) produced a non-finite value under a signed sqrt scale");
            let back = scale.invert(t);
            assert!((back - v).abs() < 1e-3, "round trip failed for {v}, got {back}");
        }
        // A domain crossing zero must still map monotonically through it.
        assert!(scale.map(-500.0) < scale.map(0.0));
        assert!(scale.map(0.0) < scale.map(500.0));
    }

    #[test]
    fn degenerate_domain_does_not_panic() {
        let scale = PowScale::sqrt(5.0, 5.0);
        assert_eq!(scale.map(5.0), 0.5);
        let ticks = scale.ticks(5);
        assert_eq!(ticks.len(), 1);
    }

    #[test]
    fn zero_exponent_does_not_panic_or_produce_nan() {
        let scale = PowScale::new(0.0, 100.0, 0.0);
        let t = scale.map(50.0);
        assert!(t.is_finite());
        let back = scale.invert(t);
        assert!(back.is_finite());
    }

    #[test]
    fn non_finite_exponent_falls_back_to_the_default() {
        let scale = PowScale::new(0.0, 100.0, f64::NAN);
        assert_eq!(scale.exponent, DEFAULT_EXPONENT);
    }

    #[test]
    fn ticks_match_a_plain_linear_scale_over_the_same_domain() {
        let pow = PowScale::sqrt(0.0, 1000.0);
        let linear = LinearScale::new(0.0, 1000.0);
        assert_eq!(pow.ticks(5), linear.ticks(5), "PowScale ticks must be identical to LinearScale ticks over the same domain — only the mapping differs");
    }

    #[test]
    fn tick_priority_matches_a_plain_linear_scale_over_the_same_domain() {
        let pow = PowScale::new(-100.0, 100.0, 0.5);
        let linear = LinearScale::new(-100.0, 100.0);
        assert_eq!(pow.tick_priority(0.0), linear.tick_priority(0.0));
        assert_eq!(pow.tick_priority(0.0), TickPriority::Major);
        assert_eq!(pow.tick_priority(50.0), TickPriority::Minor);
    }

    #[test]
    fn windowed_preserves_the_exponent() {
        let scale = PowScale::sqrt(0.0, 1000.0);
        let windowed = scale.windowed(10.0, 20.0).expect("PowScale supports windowing");
        assert_eq!(windowed.domain(), (10.0, 20.0));
        // `map`'s own shape (not just the domain bounds) must reflect the
        // SAME exponent — a windowed scale that silently reset to a
        // linear (exponent 1.0) mapping would be a real regression.
        let rebuilt = PowScale::new(10.0, 20.0, 0.5);
        assert!((windowed.map(15.0) - rebuilt.map(15.0)).abs() < 1e-12);
    }
}
