//! [`StyledRun`]/[`Paragraph`]/[`ParagraphAlign`] — the Phase 2 rich-span
//! input model (parley's simplest "ranged_builder" tier: a flat run list,
//! no tree-builder, no nested-style support — none is needed yet).

use super::decoration::{TextDecoration, VerticalAlign};
use super::inline_box::InlineBoxSlot;
use super::font_spec::FontSpec;
use crate::linebreak::{BreakStrategy, Hyphenation};

/// One styled run of text within a [`Paragraph`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyledRun<'a> {
    pub text: &'a str,
    pub font: FontSpec,
    /// Fill color packed `0xRRGGBBAA`; `None` defers to whatever default
    /// color [`crate::draw::draw_paragraph`]'s caller passes in.
    pub color: Option<u32>,
    /// Underline/strikethrough (typography-gap WAVE 2). Default
    /// [`TextDecoration::NONE`] — every pre-wave caller is unaffected.
    pub decoration: TextDecoration,
    /// Extra advance (px) added after every shaped glyph cluster in this
    /// run (typography-gap WAVE 2) — see [`crate::layout::greedy`]'s own
    /// doc comment for exactly where it's applied (post-shaping, per
    /// cluster, never inside a ligature-merged cluster). Default `0.0`.
    pub letter_spacing: f64,
    /// Sub/superscript (typography-gap WAVE 2). Default
    /// [`VerticalAlign::Baseline`] — every pre-wave caller is unaffected.
    pub vertical_align: VerticalAlign,
}

impl<'a> StyledRun<'a> {
    /// A run with no color override, no decoration, no letter-spacing, and
    /// baseline vertical alignment (paints with the caller's default).
    pub fn new(text: &'a str, font: FontSpec) -> Self {
        Self { text, font, color: None, decoration: TextDecoration::NONE, letter_spacing: 0.0, vertical_align: VerticalAlign::Baseline }
    }

    /// Builder: set this run's fill color (packed `0xRRGGBBAA`).
    pub fn with_color(mut self, color: u32) -> Self {
        self.color = Some(color);
        self
    }

    /// Builder: set this run's underline/strikethrough flags.
    pub fn with_decoration(mut self, decoration: TextDecoration) -> Self {
        self.decoration = decoration;
        self
    }

    /// Builder: set this run's extra per-cluster letter-spacing (px).
    pub fn with_letter_spacing(mut self, letter_spacing: f64) -> Self {
        self.letter_spacing = letter_spacing;
        self
    }

    /// Builder: set this run's sub/superscript vertical alignment.
    pub fn with_vertical_align(mut self, vertical_align: VerticalAlign) -> Self {
        self.vertical_align = vertical_align;
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
    /// Which line-breaking algorithm [`crate::layout::layout_paragraph`]
    /// uses (Phase 5). Defaults to [`BreakStrategy::Greedy`] — every prior
    /// phase's caller gets byte-identical output without touching this
    /// field.
    pub break_strategy: BreakStrategy,
    /// Hyphenation strategy (Phase 5). Defaults to [`Hyphenation::None`];
    /// only consulted when `break_strategy` is [`BreakStrategy::KnuthPlass`]
    /// (see `crate::linebreak`'s module doc).
    pub hyphenation: Hyphenation,
    /// Hard cap on CONSECUTIVE lines allowed to end in a discretionary
    /// hyphen (typography-gap WAVE 3) — a genuine feasibility constraint on
    /// [`crate::linebreak::knuth_plass::pack_lines`]'s own dynamic program
    /// (a breakpoint sequence exceeding the limit is INFEASIBLE, never
    /// merely demerit-discouraged the way [`crate::linebreak::knuth_plass`]'s
    /// own `\doublehyphendemerits`-equivalent already discourages exactly
    /// two in a row), not consulted anywhere else. `Some(0)` forbids a
    /// hyphen break entirely (any hyphen-ending line already violates a
    /// zero-length allowed run). `None` (the default) disables the
    /// constraint — every pre-WAVE-3 caller's own layout is byte-for-byte
    /// unchanged (see that module's own doc comment for why the
    /// unconstrained code path is untouched, not merely reproduced).
    pub max_consecutive_hyphens: Option<u8>,
}

impl<'a> Paragraph<'a> {
    /// A plain, left-aligned, natural-line-height, greedy-wrapped,
    /// non-hyphenated paragraph with no inline boxes.
    pub fn new(runs: &'a [StyledRun<'a>], max_width: f64) -> Self {
        Self {
            runs,
            inline_boxes: &[],
            align: ParagraphAlign::default(),
            line_height: None,
            max_width,
            break_strategy: BreakStrategy::default(),
            hyphenation: Hyphenation::default(),
            max_consecutive_hyphens: None,
        }
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

    /// Builder: opt into [`BreakStrategy::KnuthPlass`] (or explicitly
    /// pin `Greedy`, the default).
    pub fn with_break_strategy(mut self, break_strategy: BreakStrategy) -> Self {
        self.break_strategy = break_strategy;
        self
    }

    /// Builder: opt into [`Hyphenation::English`] (or explicitly pin
    /// `None`, the default). Only takes effect under
    /// [`BreakStrategy::KnuthPlass`] — see `crate::linebreak`'s module doc.
    pub fn with_hyphenation(mut self, hyphenation: Hyphenation) -> Self {
        self.hyphenation = hyphenation;
        self
    }

    /// Builder: cap the number of CONSECUTIVE discretionary-hyphen lines
    /// [`crate::linebreak::knuth_plass::pack_lines`]'s DP may choose (a hard
    /// feasibility constraint, typography-gap WAVE 3) — see
    /// [`Paragraph::max_consecutive_hyphens`]'s own doc comment.
    pub fn with_max_consecutive_hyphens(mut self, max: u8) -> Self {
        self.max_consecutive_hyphens = Some(max);
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
        assert!(run.decoration.is_none(), "styled_run::new must default to no decoration");
        assert_eq!(run.letter_spacing, 0.0);
        assert_eq!(run.vertical_align, VerticalAlign::Baseline);
    }

    #[test]
    fn styled_run_builders_set_decoration_letter_spacing_and_vertical_align() {
        let font = FontSpec::default();
        let run = StyledRun::new("hi", font)
            .with_decoration(TextDecoration::underline())
            .with_letter_spacing(2.5)
            .with_vertical_align(VerticalAlign::Super);
        assert_eq!(run.decoration, TextDecoration::underline());
        assert_eq!(run.letter_spacing, 2.5);
        assert_eq!(run.vertical_align, VerticalAlign::Super);
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
        assert_eq!(p.break_strategy, BreakStrategy::Greedy, "Phase 5 regression floor: default stays Greedy");
        assert_eq!(p.hyphenation, Hyphenation::None);
    }

    #[test]
    fn paragraph_builders_set_the_expected_fields() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("hi", font)];
        let p = Paragraph::new(&runs, 200.0).with_align(ParagraphAlign::Justify).with_line_height(30.0);
        assert_eq!(p.align, ParagraphAlign::Justify);
        assert_eq!(p.line_height, Some(30.0));
    }

    #[test]
    fn paragraph_with_break_strategy_and_hyphenation_set_the_expected_fields() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("hi", font)];
        let p = Paragraph::new(&runs, 200.0)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        assert_eq!(p.break_strategy, BreakStrategy::KnuthPlass);
        assert_eq!(p.hyphenation, Hyphenation::English);
    }
}
