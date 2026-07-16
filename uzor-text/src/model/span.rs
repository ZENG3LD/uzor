//! [`StyledRun`]/[`Paragraph`]/[`ParagraphAlign`] — the Phase 2 rich-span
//! input model (parley's simplest "ranged_builder" tier: a flat run list,
//! no tree-builder, no nested-style support — none is needed yet).

use super::inline_box::InlineBoxSlot;
use super::font_spec::FontSpec;

/// One styled run of text within a [`Paragraph`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledRun<'a> {
    pub text: &'a str,
    pub font: FontSpec,
    /// Fill color packed `0xRRGGBBAA`; `None` defers to whatever default
    /// color [`crate::draw::draw_paragraph`]'s caller passes in.
    pub color: Option<u32>,
}

impl<'a> StyledRun<'a> {
    /// A run with no color override (paints with the caller's default).
    pub fn new(text: &'a str, font: FontSpec) -> Self {
        Self { text, font, color: None }
    }

    /// Builder: set this run's fill color (packed `0xRRGGBBAA`).
    pub fn with_color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }
}

/// Horizontal paragraph alignment.
///
/// Supersedes [`crate::layout::Align`] as the richer, `Paragraph`-facing
/// alignment type (`crate::layout::Align`/`align_lines` remain, unchanged,
/// as [`crate::layout::layout_text`]'s own single-run alignment path — see
/// this crate's `CLAUDE.md` for why both coexist rather than one replacing
/// the other).
///
/// `Justify` is implemented starting this phase using the greedy breaker
/// (inter-word space redistribution on every line except the last) — the
/// design doc's Phase 5 section additionally revisits it once Knuth-Plass
/// breaking exists, for higher-quality breakpoints; the *redistribution*
/// itself does not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// A multi-run, word-wrapped block of text, optionally interleaved with
/// [`crate::model::InlineBox`]es.
///
/// Pure input to [`crate::layout::layout_paragraph`] (design law 3):
/// nothing here is retained/mutated by layout.
#[derive(Debug, Clone, Copy)]
pub struct Paragraph<'a> {
    pub runs: &'a [StyledRun<'a>],
    /// Boxes spliced into `runs` at a `(run_index, byte_offset)` anchor —
    /// see [`InlineBoxSlot`]. Empty slice for a plain rich-text paragraph.
    pub inline_boxes: &'a [InlineBoxSlot],
    pub align: ParagraphAlign,
    /// Fixed line height overriding the natural per-line max-ascent +
    /// max-descent fold (`None` = natural, matches every run's own
    /// metrics — see `crate::layout`'s baseline pass).
    pub line_height: Option<f64>,
    pub max_width: f64,
}

impl<'a> Paragraph<'a> {
    /// A plain, left-aligned, natural-line-height paragraph with no
    /// inline boxes.
    pub fn new(runs: &'a [StyledRun<'a>], max_width: f64) -> Self {
        Self { runs, inline_boxes: &[], align: ParagraphAlign::default(), line_height: None, max_width }
    }

    pub fn with_align(mut self, align: ParagraphAlign) -> Self {
        self.align = align;
        self
    }

    pub fn with_inline_boxes(mut self, inline_boxes: &'a [InlineBoxSlot]) -> Self {
        self.inline_boxes = inline_boxes;
        self
    }

    pub fn with_line_height(mut self, line_height: f64) -> Self {
        self.line_height = Some(line_height);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;

    #[test]
    fn styled_run_new_has_no_color_override() {
        let font = FontSpec::default();
        let run = StyledRun::new("hi", font);
        assert_eq!(run.color, None);
    }

    #[test]
    fn styled_run_with_color_sets_the_override() {
        let font = FontSpec::default();
        let run = StyledRun::new("hi", font).with_color(0x11223344);
        assert_eq!(run.color, Some(0x11223344));
    }

    #[test]
    fn paragraph_new_defaults_to_left_no_boxes_no_line_height_override() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("hi", font)];
        let p = Paragraph::new(&runs, 200.0);
        assert_eq!(p.align, ParagraphAlign::Left);
        assert!(p.inline_boxes.is_empty());
        assert_eq!(p.line_height, None);
        assert_eq!(p.max_width, 200.0);
    }

    #[test]
    fn paragraph_builders_set_the_expected_fields() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("hi", font)];
        let p = Paragraph::new(&runs, 200.0).with_align(ParagraphAlign::Justify).with_line_height(30.0);
        assert_eq!(p.align, ParagraphAlign::Justify);
        assert_eq!(p.line_height, Some(30.0));
    }
}
