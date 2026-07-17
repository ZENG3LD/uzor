//! `ColorScale` — continuous domain value -> CSS hex color, interpolated in
//! OKLCH (perceptual, hue-preserving) space rather than raw sRGB.
//!
//! Reuses `uzor::ui::animation::math::color::Color` verbatim — this OKLCH
//! lerp machinery already exists in uzor core (`Color::lerp_oklch`,
//! `Color::to_hex`/`Color::from_hex`), per
//! `nemo/docs/uzor-engines/research_dataviz_sota_2026.md` §5: interpolating
//! straight-line in `L`/`C`/`H` avoids the classic "muddy midpoint" RGB
//! gradients produce (e.g. red->green through RGB passes through a dull
//! brown; through OKLCH it stays a clean, evenly-bright arc). No color math
//! is reimplemented here — this module is a thin domain-mapping layer
//! (breakpoints + which two anchors to interpolate between) over that core
//! primitive.
//!
//! Two constructors, both usable with the built-in default anchors or an
//! explicit override (`_colors` variants) pulled from a [`crate::theme::FigureTheme`]
//! or supplied literally:
//! - [`ColorScale::sequential`] / [`ColorScale::sequential_colors`] — one
//!   continuous ramp, `min` -> `max`.
//! - [`ColorScale::diverging`] / [`ColorScale::diverging_colors`] — two
//!   ramps either side of an explicit `mid` value (not necessarily centered
//!   between `min`/`max`).
//!
//! [`ColorScale::stops`] samples `n` evenly-spaced (by domain value) colors
//! for a colorbar/gradient legend — see [`crate::guide::colorbar`].

use uzor::ui::animation::math::Color;

/// Default 3-anchor sequential ramp: dark navy -> the crate's own accent
/// blue (`FigureTheme::palette[0]`) -> warm gold — reads as a single
/// "low to high" arc on both the dark and light theme backgrounds.
const DEFAULT_SEQUENTIAL: [&str; 3] = ["#1b2540", "#4d90fe", "#f5c66b"];
/// Default diverging low/mid/high anchors — cool blue -> neutral slate ->
/// warm red, the classic diverging-scale shape (distinct hues either side
/// of a desaturated midpoint, so "which side of `mid`" reads at a glance).
const DEFAULT_DIVERGING_LOW: &str = "#3c7fd9";
const DEFAULT_DIVERGING_MID: &str = "#6b7280";
const DEFAULT_DIVERGING_HIGH: &str = "#e0555a";

/// Continuous value -> color ramp. Internally just ascending domain
/// `breaks` (len N) paired with N [`Color`] anchors — `color_at` finds the
/// bracketing pair and OKLCH-lerps between them. Invariant maintained by
/// every constructor: `breaks.len() == anchors.len() >= 2`.
#[derive(Debug, Clone)]
pub struct ColorScale {
    breaks: Vec<f64>,
    anchors: Vec<Color>,
}

/// Guard a possibly-degenerate `(min, max)` domain the same way
/// [`crate::scale::linear::nice_domain`] does: non-finite or `min >= max`
/// falls back to a unit domain anchored at `min` (or `0.0`), so a ramp
/// never divides by a zero-width span.
fn guarded_domain(min: f64, max: f64) -> (f64, f64) {
    if !min.is_finite() || !max.is_finite() || min >= max {
        let lo = if min.is_finite() { min } else { 0.0 };
        (lo, lo + 1.0)
    } else {
        (min, max)
    }
}

/// Parse `colors` as hex strings, falling back to opaque black for any
/// entry that fails to parse — a malformed caller-supplied hex string
/// degrades to a visibly-wrong-but-non-panicking color, never a crash.
fn parse_colors(colors: &[&str]) -> Vec<Color> {
    colors.iter().map(|c| Color::from_hex(c).unwrap_or(Color::BLACK)).collect()
}

/// `n` breakpoints evenly spaced (by domain value) across `[lo, hi]`,
/// `n >= 2` (the only shape every constructor produces).
fn even_breaks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let n = n.max(2);
    (0..n).map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64).collect()
}

impl ColorScale {
    /// Sequential ramp over `[min, max]` through the built-in default
    /// 3-anchor palette — see [`ColorScale::sequential_colors`] to supply
    /// your own anchors (e.g. pulled from a [`crate::theme::FigureTheme`]).
    pub fn sequential(min: f64, max: f64) -> Self {
        Self::sequential_colors(min, max, &DEFAULT_SEQUENTIAL)
    }

    /// Sequential ramp over `[min, max]` through `colors` (2+ CSS hex
    /// strings, OKLCH-interpolated between consecutive anchors evenly
    /// spaced across the domain). Fewer than 2 colors falls back to the
    /// built-in default palette — a ramp needs at least two anchors to
    /// interpolate between.
    pub fn sequential_colors(min: f64, max: f64, colors: &[&str]) -> Self {
        let (lo, hi) = guarded_domain(min, max);
        let anchors = if colors.len() >= 2 { parse_colors(colors) } else { parse_colors(&DEFAULT_SEQUENTIAL) };
        let breaks = even_breaks(lo, hi, anchors.len());
        Self { breaks, anchors }
    }

    /// Diverging ramp: `[min, mid]` interpolates low->mid color, `[mid,
    /// max]` interpolates mid->high color — built-in default anchors. `mid`
    /// is clamped into `[min, max]` (a `mid` outside the data range would
    /// otherwise collapse one side of the ramp to zero width).
    pub fn diverging(min: f64, mid: f64, max: f64) -> Self {
        Self::diverging_colors(min, mid, max, DEFAULT_DIVERGING_LOW, DEFAULT_DIVERGING_MID, DEFAULT_DIVERGING_HIGH)
    }

    /// Diverging ramp with explicit `low`/`mid_color`/`high` CSS hex
    /// anchors — same clamping/guard behavior as [`ColorScale::diverging`].
    pub fn diverging_colors(min: f64, mid: f64, max: f64, low: &str, mid_color: &str, high: &str) -> Self {
        let (lo, hi) = guarded_domain(min, max);
        let mid = if mid.is_finite() { mid.clamp(lo, hi) } else { (lo + hi) / 2.0 };
        let anchors = parse_colors(&[low, mid_color, high]);
        let breaks = vec![lo, mid, hi];
        Self { breaks, anchors }
    }

    /// This scale's own `(min, max)` domain — the first and last
    /// breakpoint, regardless of how many anchors sit between them.
    pub fn domain(&self) -> (f64, f64) {
        let lo = self.breaks[0];
        let hi = *self.breaks.last().unwrap_or(&lo);
        (lo, hi)
    }

    /// Map domain value `v` to a CSS hex color — clamped into this scale's
    /// own domain, then OKLCH-interpolated between the two anchors
    /// bracketing `v`.
    pub fn color_at(&self, v: f64) -> String {
        debug_assert!(self.breaks.len() >= 2 && self.breaks.len() == self.anchors.len(), "ColorScale invariant: >= 2 breaks, one anchor per break");
        let (lo, hi) = self.domain();
        let v = v.clamp(lo.min(hi), lo.max(hi));
        let last_segment = self.breaks.len().saturating_sub(2);
        let i = self.breaks.windows(2).position(|w| v <= w[1]).unwrap_or(last_segment);
        let (d0, d1) = (self.breaks[i], self.breaks[i + 1]);
        let t = if (d1 - d0).abs() < f64::EPSILON { 0.0 } else { ((v - d0) / (d1 - d0)).clamp(0.0, 1.0) };
        self.anchors[i].lerp_oklch(&self.anchors[i + 1], t).to_hex()
    }

    /// `n` (clamped to `>= 2`) evenly-spaced-by-domain-value `(value, hex)`
    /// samples from `min` to `max`, ascending — the colorbar gradient/tick
    /// source (see [`crate::guide::colorbar::draw_colorbar`]).
    pub fn stops(&self, n: usize) -> Vec<(f64, String)> {
        let n = n.max(2);
        let (lo, hi) = self.domain();
        (0..n)
            .map(|i| {
                let t = i as f64 / (n - 1) as f64;
                let v = lo + (hi - lo) * t;
                (v, self.color_at(v))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_endpoints_are_exact_anchor_colors() {
        let scale = ColorScale::sequential_colors(0.0, 10.0, &["#112233", "#ffcc00"]);
        let expected_lo = Color::from_hex("#112233").unwrap_or(Color::BLACK).to_hex();
        let expected_hi = Color::from_hex("#ffcc00").unwrap_or(Color::BLACK).to_hex();
        assert_eq!(scale.color_at(0.0), expected_lo);
        assert_eq!(scale.color_at(10.0), expected_hi);
        // Clamping: values outside the domain stick to the nearest endpoint.
        assert_eq!(scale.color_at(-5.0), expected_lo);
        assert_eq!(scale.color_at(50.0), expected_hi);
    }

    #[test]
    fn sequential_midpoint_matches_the_oklch_lerp_function_itself() {
        let scale = ColorScale::sequential_colors(0.0, 10.0, &["#112233", "#ffcc00"]);
        let c0 = Color::from_hex("#112233").unwrap_or(Color::BLACK);
        let c1 = Color::from_hex("#ffcc00").unwrap_or(Color::BLACK);
        let expected_mid = c0.lerp_oklch(&c1, 0.5).to_hex();
        assert_eq!(scale.color_at(5.0), expected_mid);
    }

    #[test]
    fn diverging_hits_the_exact_mid_color_at_the_mid_value() {
        let scale = ColorScale::diverging_colors(-10.0, 0.0, 20.0, "#2222ff", "#eeeeee", "#ff2222");
        let expected_mid = Color::from_hex("#eeeeee").unwrap_or(Color::BLACK).to_hex();
        assert_eq!(scale.color_at(0.0), expected_mid);
        let expected_low = Color::from_hex("#2222ff").unwrap_or(Color::BLACK).to_hex();
        let expected_high = Color::from_hex("#ff2222").unwrap_or(Color::BLACK).to_hex();
        assert_eq!(scale.color_at(-10.0), expected_low);
        assert_eq!(scale.color_at(20.0), expected_high);
    }

    #[test]
    fn diverging_mid_off_center_still_lands_exactly_at_mid_value() {
        // mid much closer to max than min — the two sides are NOT
        // symmetric ramps, but `mid` must still resolve to the exact mid
        // anchor color regardless of where it sits in [min, max].
        let scale = ColorScale::diverging_colors(0.0, 90.0, 100.0, "#000000", "#808080", "#ffffff");
        let expected_mid = Color::from_hex("#808080").unwrap_or(Color::BLACK).to_hex();
        assert_eq!(scale.color_at(90.0), expected_mid);
    }

    #[test]
    fn stops_are_ascending_and_bracket_the_domain() {
        let scale = ColorScale::sequential(0.0, 100.0);
        let stops = scale.stops(5);
        assert_eq!(stops.len(), 5);
        assert!((stops[0].0 - 0.0).abs() < 1e-9);
        assert!((stops[4].0 - 100.0).abs() < 1e-9);
        for w in stops.windows(2) {
            assert!(w[1].0 > w[0].0, "stops must be strictly ascending by domain value");
        }
        assert_eq!(stops[0].1, scale.color_at(0.0));
        assert_eq!(stops[4].1, scale.color_at(100.0));
    }

    #[test]
    fn degenerate_domain_does_not_panic() {
        let scale = ColorScale::sequential(5.0, 5.0);
        // Falls back to a unit domain anchored at min — must still resolve
        // to SOME color without panicking or dividing by zero.
        let _ = scale.color_at(5.0);
        let _ = scale.domain();
        let _ = scale.stops(4);
    }

    #[test]
    fn fewer_than_two_explicit_colors_falls_back_to_the_default_palette() {
        let scale = ColorScale::sequential_colors(0.0, 10.0, &["#123456"]);
        assert_eq!(scale.color_at(0.0), Color::from_hex(DEFAULT_SEQUENTIAL[0]).unwrap_or(Color::BLACK).to_hex());
    }
}
