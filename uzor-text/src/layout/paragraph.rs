//! [`layout_paragraph`] — Phase 2 multi-run entry point: rich spans +
//! [`crate::model::InlineBox`] + baseline pass over a [`Paragraph`].

use crate::linebreak::knuth_plass::GLUE_SHRINK_RATIO;
use crate::linebreak::BreakStrategy;
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
///
/// `paragraph.break_strategy` (Phase 5) picks which packer turns the shared
/// atom stream into line groups — [`BreakStrategy::Greedy`] (the default)
/// is Phase 1/2's original packer, unchanged; [`BreakStrategy::KnuthPlass`]
/// swaps in `crate::linebreak::knuth_plass`'s total-fit breaker. Every
/// downstream step below (baseline resolution, alignment/justify, box
/// placement) is identical either way.
pub fn layout_paragraph(paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> ParagraphLayout {
    let atoms = greedy::build_atom_stream(paragraph, shaper);
    let packed = match paragraph.break_strategy {
        BreakStrategy::Greedy => greedy::pack_lines(atoms, paragraph.max_width),
        BreakStrategy::KnuthPlass => crate::linebreak::knuth_plass::pack_lines(atoms, paragraph, shaper),
    };

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
        // Shrink (never stretch) applies regardless of alignment: a
        // `BreakStrategy::KnuthPlass` line may be chosen slightly over
        // `max_width` on the strength of its interword glue's shrink
        // capacity (see `crate::linebreak::knuth_plass`'s badness model) —
        // without actually compressing that glue here, the line would
        // render wider than `max_width` regardless of `align`. Greedy's
        // own `pack_lines` never produces an over-full non-last line with
        // glue on it, so this branch is unreachable for Greedy output
        // (Phase 1/2's regression floor is unaffected).
        let needs_shrink =
            !can_justify && line_index != last_index && glue_count > 0 && paragraph.max_width.is_finite() && natural_width > paragraph.max_width;
        // Shrink is capped at `GLUE_SHRINK_RATIO` of the line's *narrowest*
        // glue atom — the same fraction `crate::linebreak::knuth_plass`'s
        // own badness model assumes is available — never the full glue
        // width. Capping any tighter than the cost model assumed would
        // make a line KP scored as "shrink covers the gap" collapse its
        // interword spaces to nothing (unreadable, words touching) while
        // still not actually reaching `max_width` any better than a
        // shallower, legible compression would have.
        let min_glue_shrink = || {
            line_atoms
                .iter()
                .filter_map(|a| if let Atom::Text(t) = a { t.is_glue.then_some(t.width) } else { None })
                .fold(f64::MAX, f64::min)
                * GLUE_SHRINK_RATIO
        };
        let extra_per_glue = if can_justify {
            let raw = (paragraph.max_width - natural_width) / glue_count as f64;
            if raw < 0.0 { raw.max(-min_glue_shrink()) } else { raw }
        } else if needs_shrink {
            ((paragraph.max_width - natural_width) / glue_count as f64).max(-min_glue_shrink())
        } else {
            0.0
        };
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
    use crate::linebreak::Hyphenation;
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

    /// `BreakStrategy::Greedy` is the default: constructing a `Paragraph`
    /// via `new()` (never touching `break_strategy`) must produce exactly
    /// the same layout as an explicit `.with_break_strategy(Greedy)` — the
    /// Phase 5 regression guard for every prior phase's caller.
    #[test]
    fn default_paragraph_is_byte_identical_to_an_explicit_greedy_strategy() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let runs = [StyledRun::new(text, font)];
        let shaper = CosmicShaper::headless();

        let default_paragraph = Paragraph::new(&runs, 220.0);
        let explicit_greedy = Paragraph::new(&runs, 220.0).with_break_strategy(BreakStrategy::Greedy);

        let a = layout_paragraph(&default_paragraph, &shaper);
        let b = layout_paragraph(&explicit_greedy, &shaper);
        assert_eq!(a, b);
    }

    /// Every `KnuthPlass` line must stay within `max_width` (+ tolerance),
    /// with a chosen discretionary-hyphen break's glyph width folded into
    /// that measurement (not drawn "for free").
    #[test]
    fn knuth_plass_lines_stay_within_max_width_with_hyphen_widths_counted() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "An understanding of wonderful hyphenation helps a beautiful \
            narrow column of business text stay even instead of ragged.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 260.0;
        let paragraph = Paragraph::new(&runs, max_width)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        for line in &layout.lines {
            assert!(
                line.content_width <= max_width + 1.0,
                "line {} width {} exceeds max_width {max_width}",
                line.line_index,
                line.content_width
            );
        }
        assert!(
            layout.glyphs.iter().any(|g| g.cluster == "-"),
            "this fixture at this width must hit at least one hyphenation break"
        );
    }

    /// The hyphen glyph is drawn **only** where a break actually lands —
    /// the same paragraph laid out wide enough to never wrap must produce
    /// zero `-` glyphs, even with `Hyphenation::English` turned on.
    #[test]
    fn hyphenation_only_draws_a_hyphen_glyph_when_a_break_actually_lands_there() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "An understanding of wonderful hyphenation.";
        let runs = [StyledRun::new(text, font)];
        let shaper = CosmicShaper::headless();

        let narrow = Paragraph::new(&runs, 70.0)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        let narrow_layout = layout_paragraph(&narrow, &shaper);
        assert!(narrow_layout.lines.len() > 1, "fixture must wrap at this width");
        assert!(narrow_layout.glyphs.iter().any(|g| g.cluster == "-"), "narrow column should force a visible hyphen");

        let wide = Paragraph::new(&runs, 1000.0)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English);
        let wide_layout = layout_paragraph(&wide, &shaper);
        assert_eq!(wide_layout.lines.len(), 1, "fixture must fit unwrapped at this width");
        assert!(!wide_layout.glyphs.iter().any(|g| g.cluster == "-"), "no break landed, so no hyphen should be drawn");
    }

    /// `Justify` + `KnuthPlass` + `Hyphenation::Russian` compose cleanly on
    /// Cyrillic text — a narrow column forces at least one real mid-word
    /// break (a `-` glyph appears) and the paragraph wraps to several lines,
    /// with every glyph landing at a finite, valid position (no panic, no
    /// NaN) — the same "KP + hyphenation compose through the existing
    /// Justify code path with no special-casing" guarantee the English
    /// fixtures above already prove, now for a non-Latin script. (A single
    /// glue-less over-long line occasionally still exceeds `max_width` under
    /// justify — the documented "overfull hbox" fallback every KP line can
    /// hit — so this test doesn't additionally assert a per-line width
    /// bound; `knuth_plass_lines_stay_within_max_width_with_hyphen_widths_counted`
    /// above already covers that guarantee for the common case.)
    #[test]
    fn knuth_plass_hyphenates_and_justifies_a_russian_paragraph() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "показательный документ подтверждает поддержку кириллического текста";
        let runs = [StyledRun::new(text, font)];
        let max_width = 130.0;
        let paragraph = Paragraph::new(&runs, max_width)
            .with_align(ParagraphAlign::Justify)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::Russian);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines at this narrow width");
        assert!(layout.glyphs.iter().any(|g| g.cluster == "-"), "a narrow Cyrillic column must hit at least one hyphenation break");
        assert!(layout.glyphs.iter().all(|g| g.x.is_finite() && g.y.is_finite()), "every glyph must land at a finite position");
    }

    /// `Justify` + `KnuthPlass` compose cleanly: every non-last line still
    /// reaches `max_width` after redistribution, exactly like `Justify`
    /// already does over the greedy breaker.
    #[test]
    fn justify_still_reaches_max_width_under_knuth_plass() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let runs = [StyledRun::new(text, font)];
        let max_width = 220.0;
        let paragraph = Paragraph::new(&runs, max_width)
            .with_align(ParagraphAlign::Justify)
            .with_break_strategy(BreakStrategy::KnuthPlass);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        let last_index = layout.lines.len() - 1;

        for line in &layout.lines {
            if line.line_index == last_index {
                continue;
            }
            assert!(
                (line.content_width - max_width).abs() < 1.0,
                "line {} should reach max_width {max_width} under Justify+KnuthPlass, got {}",
                line.line_index,
                line.content_width
            );
        }
    }
}
