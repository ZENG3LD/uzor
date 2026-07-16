//! `paragraph_intrinsic_size` — the tessera measure seam.
//!
//! Shaped to match tessera's existing width-only `TextMeasure` seam
//! (`tessera-kernel/src/engine/text/measure.rs`) so tessera's later
//! extension onto this crate is a signature widen, not a rewrite.

use crate::layout::layout_text;
use crate::model::FontSpec;
use crate::shape::LineShaper;

/// Intrinsic `(width, height)` of `text` in `font`, word-wrapped to
/// `max_width`, via `shaper`. A thin wrapper over [`layout_text`] — the
/// same measure path every draw/hit-test/kinetics sample uses (design law
/// 1).
pub fn paragraph_intrinsic_size(text: &str, font: &FontSpec, max_width: f64, shaper: &dyn LineShaper) -> (f64, f64) {
    let layout = layout_text(text, font, max_width, shaper);
    (layout.width, layout.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    #[test]
    fn intrinsic_size_matches_layout_text_bbox() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let text = "Some sample paragraph text for the measure seam.";
        let max_width = 120.0;

        let (w, h) = paragraph_intrinsic_size(text, &font, max_width, &shaper);
        let layout = layout_text(text, &font, max_width, &shaper);

        assert_eq!(w, layout.width);
        assert_eq!(h, layout.height);
        assert!(w > 0.0 && w <= max_width + 1.0);
        assert!(h > 0.0);
    }

    #[test]
    fn intrinsic_size_of_empty_text_is_zero() {
        let font = FontSpec::default();
        let shaper = CosmicShaper::headless();
        assert_eq!(paragraph_intrinsic_size("", &font, 100.0, &shaper), (0.0, 0.0));
    }
}
