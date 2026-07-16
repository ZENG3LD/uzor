//! [`layout_paragraph`] — Phase 2 multi-run entry point: rich spans +
//! [`crate::model::InlineBox`] + baseline pass over a [`Paragraph`].

use crate::model::{InlineBox, Paragraph, ParagraphAlign};
use crate::shape::LineShaper;

use super::baseline::resolve_line_metrics;
use super::glyph_layout::{GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};
use super::greedy::{self, Atom};

/// Lay out `paragraph`'s runs (+ any spliced [`InlineBox`]es), word-wrapped
/// to `paragraph.max_width`, via `shaper`.
///
/// Pure function (design law 3): every call recomputes from
/// `(paragraph, shaper)` — no retained state. [`crate::layout::layout_text`]
/// is a thin wrapper over this for the single-run case (Phase 1 regression
/// guard: its own 4-argument signature is unchanged).
pub fn layout_paragraph(paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> ParagraphLayout {
    let atoms = greedy::build_atom_stream(paragraph, shaper);
    let packed = greedy::pack_lines(atoms, paragraph.max_width);

    if packed.is_empty() {
        return ParagraphLayout::default();
    }

    let mut glyphs = Vec::new();
    let mut lines = Vec::with_capacity(packed.len());
    let mut boxes = Vec::new();
    let mut y_top = 0.0_f64;
    let last_index = packed.len() - 1;

    for (line_index, line_atoms) in packed.into_iter().enumerate() {
        let natural_width: f64 = line_atoms.iter().map(greedy::atom_width).sum();
        let line_metrics =
            resolve_line_metrics(line_atoms.iter().map(greedy::atom_metrics), paragraph.line_height);

        let glue_count = line_atoms.iter().filter(|a| matches!(a, Atom::Text(t) if t.is_glue)).count();
        let can_justify = paragraph.align == ParagraphAlign::Justify
            && line_index != last_index
            && glue_count > 0
            && paragraph.max_width.is_finite();
        let extra_per_glue =
            if can_justify { ((paragraph.max_width - natural_width) / glue_count as f64).max(0.0) } else { 0.0 };
        let content_width = natural_width + extra_per_glue * glue_count as f64;

        let align_shift = match paragraph.align {
            ParagraphAlign::Left | ParagraphAlign::Justify => 0.0,
            ParagraphAlign::Center => {
                if paragraph.max_width.is_finite() { (paragraph.max_width - content_width) / 2.0 } else { 0.0 }
            }
            ParagraphAlign::Right => {
                if paragraph.max_width.is_finite() { paragraph.max_width - content_width } else { 0.0 }
            }
        };

        let baseline_y = y_top + line_metrics.ascent;
        let mut pen_x = align_shift;

        for atom in &line_atoms {
            match atom {
                Atom::Text(t) => {
                    let run = &paragraph.runs[t.run_index];
                    for g in &t.glyphs {
                        glyphs.push(GlyphLayout {
                            cluster: g.cluster.clone(),
                            run_index: t.run_index,
                            line_index,
                            x: pen_x + g.x,
                            y: baseline_y + g.y_offset,
                            advance: g.advance,
                            width: g.width,
                            font: run.font,
                            color: run.color,
                        });
                    }
                    pen_x += t.width;
                    if t.is_glue {
                        pen_x += extra_per_glue;
                    }
                }
                Atom::Box(inline_box) => {
                    boxes.push(placed_box(*inline_box, line_index, pen_x, baseline_y));
                    pen_x += inline_box.width();
                }
                Atom::Break => {}
            }
        }

        lines.push(LineBox { line_index, y_top, baseline_y, height: line_metrics.height, content_width });
        y_top += line_metrics.height;
    }

    let width = lines.iter().map(|l| l.content_width).fold(0.0_f64, f64::max);
    let height = lines.last().map(|l| l.y_top + l.height).unwrap_or(0.0);

    ParagraphLayout { glyphs, lines, boxes, width, height }
}

fn placed_box(inline_box: InlineBox, line_index: usize, x: f64, baseline_y: f64) -> PlacedInlineBox {
    PlacedInlineBox {
        id: inline_box.id,
        line_index,
        x,
        y_top: baseline_y - inline_box.height(),
        width: inline_box.width(),
        height: inline_box.height(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::GlyphLayout;
    use crate::model::{FontSpec, InlineBox, InlineBoxSlot, StyledRun};
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    /// Mixed-size runs on the same line must share one baseline (every
    /// glyph's `y` equal), and that line's height must reflect the
    /// *taller* run, not the smaller one.
    #[test]
    fn mixed_size_runs_share_one_baseline_and_line_height_is_max_run_height() {
        let small = FontSpec::new(FontFamily::Roboto, 14.0);
        let big = FontSpec::new(FontFamily::Roboto, 28.0);
        let runs = [StyledRun::new("small ", small), StyledRun::new("BIG", big)];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.lines.len(), 1, "fixture must fit on one line");

        let ys: Vec<f64> = layout.glyphs.iter().map(|g| g.y).collect();
        assert!(!ys.is_empty());
        for y in &ys {
            assert!((y - ys[0]).abs() < 1e-6, "mixed-size runs must share one baseline, got {ys:?}");
        }

        let small_only = crate::layout::layout_text("small only", &small, 1000.0, &shaper);
        assert!(
            layout.lines[0].height > small_only.lines[0].height,
            "line height must reflect the taller run"
        );
    }

    /// `Justify` stretches every line except the last so it reaches
    /// `max_width` exactly; the last line is never stretched.
    #[test]
    fn justify_stretches_every_line_except_the_last() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let runs = [StyledRun::new(text, font)];
        let max_width = 220.0;
        let paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        let last_index = layout.lines.len() - 1;

        for line in &layout.lines {
            let glyphs_on_line: Vec<&GlyphLayout> =
                layout.glyphs.iter().filter(|g| g.line_index == line.line_index).collect();
            let Some(last_glyph) = glyphs_on_line.last() else { continue };
            let rendered_extent = last_glyph.x + last_glyph.advance;
            if line.line_index == last_index {
                assert!(rendered_extent < max_width - 1.0, "last line must NOT be stretched, got {rendered_extent}");
            } else {
                assert!(
                    (rendered_extent - max_width).abs() < 1.0,
                    "line {} should reach max_width {max_width}, got {rendered_extent}",
                    line.line_index
                );
                assert!((line.content_width - max_width).abs() < 1.0);
            }
        }
    }

    /// A single-word line under `Justify` must not divide by zero or panic.
    #[test]
    fn justify_single_word_line_does_not_divide_by_zero() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("Solo", font)];
        let paragraph = Paragraph::new(&runs, 300.0).with_align(ParagraphAlign::Justify);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.lines[0].content_width.is_finite());
        assert!(layout.lines[0].content_width < 300.0, "a single word must not be force-stretched");
    }

    /// An in-flow `InlineBox` reserves its width out of the text flow (no
    /// glyph before it overlaps it, no glyph after it starts before its
    /// reserved width ends) and its bottom edge sits on the line baseline.
    #[test]
    fn inline_box_reserves_width_and_places_at_correct_x_and_baseline() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("Look ", font), StyledRun::new(" after the icon.", font)];
        let box_width = 40.0;
        let box_height = 20.0;
        let inline_box = InlineBox::in_flow(1, box_width, box_height);
        let slots = [InlineBoxSlot::new(0, "Look ".len(), inline_box)];
        let paragraph = Paragraph::new(&runs, 1000.0).with_inline_boxes(&slots);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.boxes.len(), 1);
        let placed = layout.boxes[0];
        assert_eq!(placed.id, 1);
        assert_eq!(placed.width, box_width);
        assert_eq!(placed.height, box_height);

        for glyph in &layout.glyphs {
            if glyph.run_index == 0 {
                assert!(glyph.x + glyph.advance <= placed.x + 0.5, "text before the box must not overlap it");
            } else {
                assert!(
                    glyph.x + 0.5 >= placed.x + placed.width,
                    "text after the box must continue past its reserved width"
                );
            }
        }

        let line = &layout.lines[placed.line_index];
        assert!(
            (placed.y_top + placed.height - line.baseline_y).abs() < 1e-6,
            "box's bottom edge must sit on the line baseline"
        );
    }

    /// An `InlineBox` taller than the surrounding text grows the line.
    #[test]
    fn inline_box_taller_than_text_grows_the_line() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("short text", font)];
        let tall_box = InlineBox::in_flow(7, 10.0, 200.0);
        let slots = [InlineBoxSlot::new(0, "short ".len(), tall_box)];
        let paragraph = Paragraph::new(&runs, 1000.0).with_inline_boxes(&slots);
        let shaper = CosmicShaper::headless();

        let with_box = layout_paragraph(&paragraph, &shaper);
        let without_box = crate::layout::layout_text("short text", &font, 1000.0, &shaper);

        assert!(with_box.lines[0].height > without_box.lines[0].height);
    }
}
