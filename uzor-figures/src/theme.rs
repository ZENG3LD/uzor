//! `FigureTheme` — the small set of colors/font every guide and figure in
//! this crate reads. Palette matches `uzor-graph::render::category_color`'s
//! 10-color set so figures and the graph family share one visual identity
//! across the uzor demo suite.

/// Colors + font a figure needs to draw itself.
#[derive(Debug, Clone)]
pub struct FigureTheme {
    pub background: String,
    pub axis_color: String,
    pub grid_color: String,
    pub label_color: String,
    pub label_font: String,
    /// Bare font family (e.g. `"sans-serif"`, `"Georgia"`, `"'Times New
    /// Roman', serif"`) — no size/weight tokens. Exists so a figure that
    /// needs a DIFFERENT size/weight of the SAME family (today, only
    /// [`crate::figure::KpiFigure`]'s headline number) can compose a new
    /// CSS font string directly from this field instead of re-parsing
    /// [`FigureTheme::label_font`]'s own composed string — a real bug
    /// found in [`crate::figure::kpi::KpiFigure`]'s pre-existing
    /// `scaled_font` helper, which silently discarded the family on any
    /// `label_font` shape other than the exact `"<N>px <family>"` this
    /// crate's own [`FigureTheme::dark`]/`light` happen to produce (e.g.
    /// `"bold 11px Georgia"` degraded to a literal `"sans-serif"`).
    /// [`FigureTheme::label_font`] itself is unaffected — every other
    /// figure keeps using it verbatim.
    pub label_font_family: String,
    /// Deterministic per-series color assignment — figures index into
    /// this by series/category position, same convention as
    /// `uzor-graph::render::category_color`'s hashed lookup.
    pub palette: Vec<String>,
    /// Semantic "good"/increase color — [`crate::figure::WaterfallFigure`]'s
    /// positive-delta bars (additive field, business-chart hygiene default:
    /// FT/Economist convention colors a running-total INCREASE distinctly
    /// from a decrease, not by arbitrary palette index).
    pub positive: String,
    /// Semantic "bad"/decrease color — [`crate::figure::WaterfallFigure`]'s
    /// negative-delta bars.
    pub negative: String,
    /// Hover/selection highlight color — every figure's own translucent
    /// "brighten the hovered mark" overlay (bars/scatter/waterfall/
    /// boxplot/pie/timeline/heatmap) and hover-outline paint through THIS
    /// field instead of a hardcoded `"#ffffff"` literal (a real bug: white
    /// at low alpha over [`FigureTheme::light`]'s own white background was
    /// visually imperceptible — every figure was silently broken on hover
    /// under the light theme). [`FigureTheme::dark`] keeps the pre-existing
    /// white (still correct there — white already contrasted against a
    /// dark background); [`FigureTheme::light`] picks a value that
    /// contrasts against a WHITE background instead.
    pub highlight: String,
}

/// The 10-color categorical palette shared with `uzor-graph`.
const PALETTE: [&str; 10] = [
    "#4d90fe", "#e0703c", "#5cb87a", "#c94f7c", "#d9b64e", "#7e6bd9", "#3fb6c9", "#e0555a", "#8fbf5f", "#c78bd9",
];
/// Same green/red hues already in [`PALETTE`] (indices 2/7) — reused as the
/// named semantic increase/decrease colors so `positive`/`negative` read as
/// part of the SAME visual identity, not a second unrelated palette.
const POSITIVE_COLOR: &str = "#5cb87a";
const NEGATIVE_COLOR: &str = "#e0555a";
/// [`FigureTheme::dark`]'s own hover/selection highlight — unchanged from
/// the pre-existing hardcoded literal every figure used directly (white
/// still contrasts correctly against a dark background, so the dark
/// theme's own visual behavior is byte-identical after this fix).
const HIGHLIGHT_DARK: &str = "#ffffff";
/// [`FigureTheme::light`]'s own hover/selection highlight — a dark navy,
/// chosen to contrast against `FigureTheme::light`'s own `"#ffffff"`
/// background at the same low alpha every figure's hover overlay already
/// paints at (white-on-white was the actual bug this field fixes).
const HIGHLIGHT_LIGHT: &str = "#1a1a2e";
/// [`FigureTheme::high_contrast`]'s own hover/selection highlight — pure
/// yellow, the maximum-contrast choice against this theme's pure-black
/// background (same role [`HIGHLIGHT_DARK`]/[`HIGHLIGHT_LIGHT`] play for
/// their own backgrounds).
const HIGHLIGHT_HIGH_CONTRAST: &str = "#ffff00";

impl FigureTheme {
    /// Dark theme — matches `force-graph-demo`'s `#0d0f14` canvas
    /// background, the uzor demo suite's default aesthetic.
    pub fn dark() -> Self {
        Self {
            background: "#0d0f14".to_owned(),
            axis_color: "#3a3f4b".to_owned(),
            grid_color: "#22262f".to_owned(),
            label_color: "#9aa0ac".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_font_family: "sans-serif".to_owned(),
            palette: PALETTE.iter().map(|&s| s.to_owned()).collect(),
            positive: POSITIVE_COLOR.to_owned(),
            negative: NEGATIVE_COLOR.to_owned(),
            highlight: HIGHLIGHT_DARK.to_owned(),
        }
    }

    /// Light theme — white canvas, darker guide colors for contrast.
    pub fn light() -> Self {
        Self {
            background: "#ffffff".to_owned(),
            axis_color: "#c7ccd6".to_owned(),
            grid_color: "#eceff3".to_owned(),
            label_color: "#4a5060".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_font_family: "sans-serif".to_owned(),
            palette: PALETTE.iter().map(|&s| s.to_owned()).collect(),
            positive: POSITIVE_COLOR.to_owned(),
            negative: NEGATIVE_COLOR.to_owned(),
            highlight: HIGHLIGHT_LIGHT.to_owned(),
        }
    }

    /// High-contrast (accessibility) preset — pure-black background, pure-
    /// white axis/label ink, a maximum-contrast yellow hover/selection
    /// highlight. Closes the MLC-harvest gap: MLC ships `dark`/`light`/
    /// `high_contrast`/`cyberpunk` against this crate's own `dark`/`light`
    /// pair.
    ///
    /// `palette`/`positive`/`negative` are DELIBERATELY the SAME shared
    /// values [`FigureTheme::dark`]/[`FigureTheme::light`] already use —
    /// a figure's categorical-series identity stays consistent across
    /// every built-in theme (the same design choice `dark`/`light` already
    /// make relative to each other). This preset maximizes CHROME contrast
    /// (background/axis/grid/label/highlight), not categorical-hue
    /// distinguishability — a caller specifically wanting colour-blind-
    /// safe categorical hues alongside this theme should pair it with
    /// [`crate::scale::color::CategoricalScale::default_palette`] via a
    /// figure's own `with_category_palette` builder (e.g.
    /// [`crate::figure::BarFigure::with_category_palette`]), the two
    /// accessibility concerns (screen contrast vs. hue confusability) are
    /// independent knobs, not one setting.
    pub fn high_contrast() -> Self {
        Self {
            background: "#000000".to_owned(),
            axis_color: "#ffffff".to_owned(),
            grid_color: "#4d4d4d".to_owned(),
            label_color: "#ffffff".to_owned(),
            label_font: "11px sans-serif".to_owned(),
            label_font_family: "sans-serif".to_owned(),
            palette: PALETTE.iter().map(|&s| s.to_owned()).collect(),
            positive: POSITIVE_COLOR.to_owned(),
            negative: NEGATIVE_COLOR.to_owned(),
            highlight: HIGHLIGHT_HIGH_CONTRAST.to_owned(),
        }
    }
}

impl Default for FigureTheme {
    /// Dark — matches the rest of the uzor demo suite's default aesthetic.
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_and_light_both_carry_the_full_palette() {
        assert_eq!(FigureTheme::dark().palette.len(), PALETTE.len());
        assert_eq!(FigureTheme::light().palette.len(), PALETTE.len());
        assert_eq!(FigureTheme::high_contrast().palette.len(), PALETTE.len());
    }

    #[test]
    fn default_is_dark() {
        assert_eq!(FigureTheme::default().background, FigureTheme::dark().background);
    }

    #[test]
    fn highlight_color_differs_from_the_background_in_every_built_in_theme() {
        // The actual bug this field fixes: `FigureTheme::light`'s own
        // background is `"#ffffff"` — a hover overlay painted at the same
        // literal `"#ffffff"` (the pre-fix hardcoded value every figure
        // used directly) is imperceptible at low alpha. Every built-in
        // theme's own `highlight` must be a genuinely different color
        // from its own `background`.
        for theme in [FigureTheme::dark(), FigureTheme::light(), FigureTheme::high_contrast()] {
            assert_ne!(
                theme.highlight.to_ascii_lowercase(),
                theme.background.to_ascii_lowercase(),
                "highlight must contrast against the background, got highlight={} background={}",
                theme.highlight,
                theme.background
            );
        }
    }

    #[test]
    fn high_contrast_uses_pure_black_background_and_pure_white_chrome() {
        let theme = FigureTheme::high_contrast();
        assert_eq!(theme.background, "#000000");
        assert_eq!(theme.axis_color, "#ffffff");
        assert_eq!(theme.label_color, "#ffffff");
        assert_ne!(theme.grid_color, theme.background, "grid must remain visible against a pure-black background");
    }

    #[test]
    fn high_contrast_shares_the_same_palette_and_semantic_colors_as_dark_and_light() {
        // Categorical-series identity stays consistent across every
        // built-in theme — high_contrast maximizes CHROME contrast, not a
        // different palette.
        let dark = FigureTheme::dark();
        let hc = FigureTheme::high_contrast();
        assert_eq!(hc.palette, dark.palette);
        assert_eq!(hc.positive, dark.positive);
        assert_eq!(hc.negative, dark.negative);
    }

    #[test]
    fn dark_theme_highlight_is_unchanged_from_the_pre_fix_literal() {
        // The dark theme's own hover/selection color was never actually
        // broken (white already contrasts against a dark background) — a
        // regression guard that this fix doesn't change dark-theme output.
        assert_eq!(FigureTheme::dark().highlight, "#ffffff");
    }
}
