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

/// N distinct hues for N unordered CATEGORIES — the discrete counterpart
/// to [`ColorScale`]'s continuous sequential/diverging ramps. Deliberately
/// NOT folded into [`ColorScale`] itself: `ColorScale`'s whole job is
/// OKLCH-interpolating an intermediate hue BETWEEN two anchors for a value
/// between two breakpoints, and a categorical palette must never do that —
/// an interpolated blend between "category A"'s color and "category B"'s
/// color has no meaning (category count has no ordering a blend could
/// respect). This is a plain index -> color lookup with a documented
/// cycling policy, not a ramp.
#[derive(Debug, Clone)]
pub struct CategoricalScale {
    colors: Vec<String>,
}

/// The built-in default palette: Okabe, M. & Ito, K. (2008), "Color
/// Universal Design (CUD) — How to make figures and presentations that are
/// friendly to Colorblind people," Color Universal Design Organization
/// (<https://jfly.uni-koeln.de/color/>) — the de-facto standard 8-hue
/// qualitative palette for colorblind-safe categorical charts (the same
/// set underlies R's `scales::pal_okabe_ito`/`ggthemes::okabe_ito` and is
/// widely re-published as THE reference colorblind-safe qualitative
/// palette in dataviz literature). Verified safe against protanopia,
/// deuteranopia (the two red-green forms, together the large majority of
/// color-vision deficiency), and tritanopia (blue-yellow) by the original
/// publication; this crate's own test suite additionally runs an
/// independent deuteranopia-simulation check (see `tests::
/// default_palette_stays_pairwise_distinguishable_under_deuteranopia_simulation`
/// below) as a second, locally-computed confirmation.
const OKABE_ITO: [&str; 8] = [
    "#000000", // black
    "#E69F00", // orange
    "#56B4E9", // sky blue
    "#009E73", // bluish green
    "#F0E442", // yellow
    "#0072B2", // blue
    "#D55E00", // vermillion
    "#CC79A7", // reddish purple
];

/// Neutral fallback for [`CategoricalScale::color_for`] on an
/// (unreachable through the public constructors, but defensively guarded)
/// empty palette — a mid-gray rather than a panic on `% 0`.
const EMPTY_PALETTE_FALLBACK: &str = "#808080";

impl CategoricalScale {
    /// The built-in colour-blind-safe default — see [`OKABE_ITO`]'s own
    /// doc comment for the source and safety claim.
    pub fn default_palette() -> Self {
        Self { colors: OKABE_ITO.iter().map(|&s| s.to_owned()).collect() }
    }

    /// A caller-supplied palette (CSS hex strings). An EMPTY list falls
    /// back to [`CategoricalScale::default_palette`] (a zero-color palette
    /// can't assign anything); any non-empty list — including a single
    /// color, or one shorter than the caller's own category count — is
    /// honored as given (a caller relying on the cycling policy documented
    /// on [`CategoricalScale::color_for`] is a legitimate choice this
    /// constructor doesn't second-guess).
    pub fn new(colors: Vec<String>) -> Self {
        if colors.is_empty() {
            Self::default_palette()
        } else {
            Self { colors }
        }
    }

    /// This palette's own color count.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Color for category `index` — CYCLES via `index % len()` once
    /// `index` exceeds the palette's own length. This is the ONE, documented
    /// cycling policy for this crate's categorical color assignment —
    /// matches the SAME `%`-modulo convention every pre-existing
    /// `theme.palette[i % theme.palette.len()]` call site already uses for
    /// the crate's own 10-color default palette (`BarFigure`/`PieFigure`/
    /// `DagFigure`/`SankeyFigure`'s own category-color resolution): a
    /// category beyond the Nth distinct hue silently repeats colors from
    /// the start rather than growing the palette, blending, or erroring.
    pub fn color_for(&self, index: usize) -> &str {
        if self.colors.is_empty() {
            return EMPTY_PALETTE_FALLBACK;
        }
        self.colors[index % self.colors.len()].as_str()
    }
}

impl Default for CategoricalScale {
    /// The colour-blind-safe Okabe-Ito default — see
    /// [`CategoricalScale::default_palette`].
    fn default() -> Self {
        Self::default_palette()
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

    // ── CategoricalScale ─────────────────────────────────────────────

    #[test]
    fn default_palette_has_eight_okabe_ito_entries() {
        let palette = CategoricalScale::default_palette();
        assert_eq!(palette.len(), 8);
        assert_eq!(palette.color_for(0), "#000000");
    }

    #[test]
    fn new_with_user_colors_is_honored_verbatim() {
        let palette = CategoricalScale::new(vec!["#111111".to_owned(), "#222222".to_owned()]);
        assert_eq!(palette.len(), 2);
        assert_eq!(palette.color_for(0), "#111111");
        assert_eq!(palette.color_for(1), "#222222");
    }

    #[test]
    fn new_with_empty_colors_falls_back_to_the_default_palette() {
        let palette = CategoricalScale::new(Vec::new());
        assert_eq!(palette.len(), 8);
        assert_eq!(palette.color_for(0), "#000000");
    }

    #[test]
    fn color_for_cycles_via_modulo_past_the_palette_length() {
        let palette = CategoricalScale::new(vec!["#aa0000".to_owned(), "#00bb00".to_owned(), "#0000cc".to_owned()]);
        assert_eq!(palette.color_for(3), palette.color_for(0));
        assert_eq!(palette.color_for(4), palette.color_for(1));
        assert_eq!(palette.color_for(100), palette.color_for(100 % 3));
    }

    #[test]
    fn default_impl_matches_default_palette() {
        assert_eq!(CategoricalScale::default().color_for(0), CategoricalScale::default_palette().color_for(0));
    }

    /// Deuteranopia (red-green colorblindness) simulation via the Machado,
    /// Oliveira & Fairchild (2009), "A Physiologically-based Model for
    /// Simulation of Color Vision Deficiency" (IEEE TVCG 15(6)) linear-RGB
    /// transform matrix — the same matrix underlying Chromium DevTools'
    /// own "Rendering > Emulate vision deficiencies > Deuteranopia" mode.
    /// An INDEPENDENT, locally-computed second confirmation of the
    /// Okabe-Ito palette's own published colorblind-safety claim (see
    /// [`OKABE_ITO`]'s own doc comment for the citation) — not a
    /// replacement for it.
    fn simulate_deuteranopia(hex: &str) -> (f64, f64, f64) {
        let color = Color::from_hex(hex).unwrap_or(Color::BLACK);
        let (r, g, b) = color.to_linear();
        let r2 = 0.367_322 * r + 0.860_646 * g - 0.227_968 * b;
        let g2 = 0.280_085 * r + 0.672_501 * g + 0.047_413 * b;
        let b2 = -0.011_820 * r + 0.042_940 * g + 0.968_881 * b;
        (r2, g2, b2)
    }

    fn euclidean_distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
    }

    #[test]
    fn default_palette_stays_pairwise_distinguishable_under_deuteranopia_simulation() {
        let palette = CategoricalScale::default_palette();
        let simulated: Vec<(f64, f64, f64)> = (0..palette.len()).map(|i| simulate_deuteranopia(palette.color_for(i))).collect();
        // A locally-computed pass over this exact palette measures its
        // closest simulated pair (bluish-green #009E73 vs. vermillion
        // #D55E00) at ~0.204 linear-RGB Euclidean distance — 0.15 is a
        // generous floor well below that measured minimum, so this
        // assertion has real margin without being tuned to just barely
        // pass.
        const MIN_DISTANCE: f64 = 0.15;
        for i in 0..simulated.len() {
            for j in (i + 1)..simulated.len() {
                let d = euclidean_distance(simulated[i], simulated[j]);
                assert!(
                    d > MIN_DISTANCE,
                    "palette entries {i} ({}) and {j} ({}) collapse under deuteranopia simulation: distance {d:.4}",
                    palette.color_for(i),
                    palette.color_for(j)
                );
            }
        }
    }
}
