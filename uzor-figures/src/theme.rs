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
            palette: PALETTE.iter().map(|&s| s.to_owned()).collect(),
            positive: POSITIVE_COLOR.to_owned(),
            negative: NEGATIVE_COLOR.to_owned(),
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
            palette: PALETTE.iter().map(|&s| s.to_owned()).collect(),
            positive: POSITIVE_COLOR.to_owned(),
            negative: NEGATIVE_COLOR.to_owned(),
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
    }

    #[test]
    fn default_is_dark() {
        assert_eq!(FigureTheme::default().background, FigureTheme::dark().background);
    }
}
