//! Greedy word-wrap over a [`Paragraph`]'s runs + spliced [`InlineBox`]es.
//!
//! **Divergence from the design doc's Phase 2 sketch** (recorded here and
//! in this crate's `CLAUDE.md`): the doc describes switching
//! `CosmicShaper`'s internal call from `buf.set_text` to cosmic-text's own
//! `buf.set_rich_text`, letting cosmic-text's line breaker handle
//! cross-run wrap directly. This module instead shapes each run/segment
//! in isolation via the existing [`LineShaper::shape_wrapped`] (Phase 1's
//! own trait method — unchanged) and re-flows the results itself,
//! word-by-word, because:
//!
//! 1. `uzor::shaper` has no rich (multi-span) entry today, and adding one
//!    would mean either a new core surface on the most-used subsystem in
//!    the crate, or hand-rolling `InlineBox` atomic-reservation on top of
//!    it anyway (the doc itself says boxes must be "layered on top" of
//!    cosmic-text, since it has no inline-object concept) — so some
//!    custom wrap-accumulator work is unavoidable regardless.
//! 2. Doing it all here keeps Phase 2 entirely inside `uzor-text`, at zero
//!    risk to `uzor::shaper.rs` (read by ~100+ call sites across 4
//!    backends).
//!
//! Trade-off accepted: no cross-run kerning at a style-boundary that falls
//! *inside* a word (rare, and a run boundary usually falls on whitespace
//! anyway) — kerning within one run/word is still exact, since each
//! segment is still shaped for real by cosmic-text.

use uzor::render::GlyphMetric;

use crate::model::{FontSpec, InlineBox, InlineBoxSlot, Paragraph, StyledRun};
use crate::shape::LineShaper;

/// One glyph cluster, positioned relative to its own [`Atom`]'s local
/// origin (rebased to `0.0` at the atom's first glyph).
#[derive(Debug, Clone)]
pub(crate) struct AtomGlyph {
    pub cluster: String,
    pub x: f64,
    pub y_offset: f64,
    pub advance: f64,
    pub width: f64,
}

/// A contiguous run of same-whitespace-class glyph clusters from one
/// [`crate::model::StyledRun`], shaped and kept together as one
/// unbreakable unit.
#[derive(Debug, Clone)]
pub(crate) struct TextAtom {
    pub run_index: usize,
    pub glyphs: Vec<AtomGlyph>,
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
    /// The font this atom's glyphs were actually SHAPED at — matches the
    /// owning run's own `font` unchanged under `VerticalAlign::Baseline`,
    /// or that font shrunk by `VerticalAlign::shape_font` under `Super`/
    /// `Sub` (typography-gap WAVE 2). Carried per-atom (not re-derived from
    /// `paragraph.runs[run_index].font` at placement time) so
    /// [`crate::layout::glyph_layout::GlyphLayout::font`] always reports
    /// the font glyphs were genuinely measured at — a downstream painter
    /// that trusts `GlyphLayout::font` for `set_font` never mismatches a
    /// shrunk sub/superscript glyph's already-resolved position/width.
    pub shape_font: FontSpec,
    /// `true` for a whitespace-only atom — a wrap opportunity; dropped
    /// when it would trail a finished line.
    pub is_glue: bool,
    /// `true` when this atom is one hyphenation fragment (of a longer word
    /// split by [`crate::linebreak::hyphenate::expand_hyphenation`]) that
    /// may end a line — [`crate::linebreak::knuth_plass`]'s discretionary
    /// breakpoint flag. Always `false` for every atom this module itself
    /// produces (Phase 1/2's own greedy packer never reads this field, so
    /// its output is unaffected either way — Phase 5's regression floor).
    pub hyphen_break: bool,
}

/// One token in the paragraph's linear content stream.
#[derive(Debug, Clone)]
pub(crate) enum Atom {
    Text(TextAtom),
    Box(InlineBox),
    /// A forced line break (a literal `\n` inside a run's text).
    Break,
}

/// This atom's contribution to its line's total advance width.
pub(crate) fn atom_width(atom: &Atom) -> f64 {
    match atom {
        Atom::Text(t) => t.width,
        Atom::Box(b) => b.width(),
        Atom::Break => 0.0,
    }
}

/// `(ascent, descent)` this atom contributes to its line's shared
/// baseline fold (see [`super::baseline::resolve_line_metrics`]).
pub(crate) fn atom_metrics(atom: &Atom) -> (f64, f64) {
    match atom {
        Atom::Text(t) => (t.ascent, t.descent),
        Atom::Box(b) => (b.ascent(), b.descent()),
        Atom::Break => (0.0, 0.0),
    }
}

/// Flatten `paragraph`'s runs + spliced inline boxes into one ordered
/// [`Atom`] stream, shaping each run segment in isolation via `shaper`.
pub(crate) fn build_atom_stream(paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> Vec<Atom> {
    let mut boxes_by_run: Vec<Vec<&InlineBoxSlot>> = vec![Vec::new(); paragraph.runs.len()];
    for slot in paragraph.inline_boxes {
        if let Some(bucket) = boxes_by_run.get_mut(slot.run_index) {
            bucket.push(slot);
        }
        // A slot pointing at an out-of-range run index is silently
        // dropped — no fallible API surface on the hot path.
    }
    for bucket in &mut boxes_by_run {
        bucket.sort_by_key(|slot| slot.byte_offset);
    }

    let mut atoms = Vec::new();
    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let text = run.text;
        let mut cursor = 0usize;
        for slot in &boxes_by_run[run_index] {
            let offset = floor_char_boundary(text, slot.byte_offset);
            if offset < cursor {
                // Out-of-order after flooring (e.g. two slots landing on
                // the same grapheme) — drop rather than go backwards.
                continue;
            }
            if offset > cursor {
                push_text_segment(&mut atoms, run_index, &text[cursor..offset], *run, shaper);
            }
            atoms.push(Atom::Box(slot.inline_box));
            cursor = offset;
        }
        if cursor < text.len() {
            push_text_segment(&mut atoms, run_index, &text[cursor..], *run, shaper);
        }
    }
    atoms
}

fn floor_char_boundary(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn push_text_segment(
    atoms: &mut Vec<Atom>,
    run_index: usize,
    segment: &str,
    run: StyledRun<'_>,
    shaper: &dyn LineShaper,
) {
    if segment.is_empty() {
        return;
    }
    // Typography-gap WAVE 2: a `Super`/`Sub` run shapes at a genuinely
    // smaller font (`VerticalAlign::shape_font`) — see `TextAtom::
    // shape_font`'s own doc comment for why this is carried per-atom
    // rather than re-derived from `paragraph.runs[run_index].font` at
    // placement time.
    let shape_font = run.vertical_align.shape_font(run.font);
    // Matches `crate::layout::glyph_layout`'s own fallback-height
    // convention (cosmic-text's `Metrics::new(font_size, font_size *
    // 1.2)`) — reused here, not reinvented.
    let fallback_height = (shape_font.size_px * 1.2).max(1.0);
    let wrapped_lines = shaper.shape_wrapped(segment, &shape_font, f64::MAX);
    for (i, line) in wrapped_lines.iter().enumerate() {
        if i > 0 {
            atoms.push(Atom::Break);
        }
        let ascent = line.baseline_y.max(0.0);
        let descent = (fallback_height - ascent).max(0.0);
        push_glyph_atoms(atoms, run_index, &line.glyphs, ascent, descent, shape_font, run.letter_spacing);
    }
}

fn push_glyph_atoms(
    atoms: &mut Vec<Atom>,
    run_index: usize,
    glyphs: &[GlyphMetric],
    ascent: f64,
    descent: f64,
    shape_font: FontSpec,
    letter_spacing: f64,
) {
    let mut i = 0;
    while i < glyphs.len() {
        let is_ws = is_whitespace_cluster(&glyphs[i].cluster);
        let start = i;
        while i < glyphs.len() && is_whitespace_cluster(&glyphs[i].cluster) == is_ws {
            i += 1;
        }
        let group = &glyphs[start..i];
        let Some(first) = group.first() else {
            continue;
        };
        let base_x = first.x_offset;
        // Typography-gap WAVE 2 (letter-spacing): `extra` accumulates the
        // spacing already inserted before this glyph — the shaper's own
        // `x_offset` knows nothing about it, so every glyph after the
        // first in this group is re-based by however much spacing already
        // preceded it. Applied per shaped cluster (never inside a
        // ligature — cosmic-text's own ligature merge already collapsed a
        // multi-codepoint ligature into ONE `GlyphMetric`/cluster before
        // this loop ever sees it, see `uzor::shaper::shape_uncached`'s own
        // merge logic), so a ligature gets exactly one spacing bump, not
        // one per source character.
        let mut extra = 0.0_f64;
        let mut atom_glyphs = Vec::with_capacity(group.len());
        let mut end_x = 0.0_f64; // always overwritten below — `group` is non-empty here
        for g in group {
            let x = (g.x_offset - base_x) + extra;
            let advance = g.advance + letter_spacing;
            atom_glyphs.push(AtomGlyph { cluster: g.cluster.clone(), x, y_offset: g.y_offset, advance, width: g.width });
            end_x = x + advance;
            extra += letter_spacing;
        }
        atoms.push(Atom::Text(TextAtom {
            run_index,
            glyphs: atom_glyphs,
            width: end_x,
            ascent,
            descent,
            shape_font,
            is_glue: is_ws,
            hyphen_break: false,
        }));
    }
}

/// `true` for a NO-BREAK space character (U+00A0 NO-BREAK SPACE, U+202F
/// NARROW NO-BREAK SPACE) — bug fix, typography-gap WAVE 2. Both carry
/// Unicode's `White_Space` property (so `char::is_whitespace` alone
/// returns `true` for them, same as an ordinary space), but their own
/// Unicode semantics forbid a line break there. `uzor::shaper` still
/// shapes one with a real space-width glyph advance (cosmic-text treats it
/// identically to a plain space for shaping purposes) — only its
/// classification as a wrap opportunity changes.
fn is_no_break_space(ch: char) -> bool {
    matches!(ch, '\u{00A0}' | '\u{202F}')
}

/// A cluster counts as breakable glue when every one of its characters is
/// whitespace AND none of them is a no-break space ([`is_no_break_space`]).
/// A no-break-space cluster therefore reads as "not whitespace" here,
/// which glues it into the SAME non-breaking [`TextAtom`] as its
/// neighboring word glyphs (see [`push_glyph_atoms`]'s own same-`is_ws`
/// grouping loop) — non-stretching (never touched by
/// [`crate::layout::paragraph::glue_extras_for_line`]'s own Justify/shrink
/// redistribution, since no glue atom for it exists to redistribute onto),
/// fixed-width, and never a break opportunity for [`pack_lines`] or
/// [`crate::linebreak::knuth_plass`] (neither ever inspects a non-glue
/// atom's own interior for a mid-atom breakpoint).
fn is_whitespace_cluster(cluster: &str) -> bool {
    !cluster.is_empty() && cluster.chars().all(|c| c.is_whitespace() && !is_no_break_space(c))
}

/// Greedily pack `atoms` into visual lines no wider than `max_width`
/// (non-finite widths behave as unconstrained). A single atom wider than
/// `max_width` is still placed alone on its own line rather than dropped
/// or panicking (matches `uzor::shaper::measure_glyphs_wrapped`'s own
/// convention). Trailing whitespace atoms are trimmed from each finished
/// line — never rendered, never counted toward content width.
pub(crate) fn pack_lines(atoms: Vec<Atom>, max_width: f64) -> Vec<Vec<Atom>> {
    let max_width = if max_width.is_finite() { max_width.max(1.0) } else { f64::MAX };

    let mut lines = Vec::new();
    let mut current: Vec<Atom> = Vec::new();
    let mut current_width = 0.0_f64;

    for atom in atoms {
        match atom {
            Atom::Break => {
                flush_line(&mut current, &mut lines);
                current_width = 0.0;
            }
            Atom::Text(ref t) if t.is_glue => {
                if current.is_empty() {
                    continue; // never start a line with leading whitespace
                }
                current_width += t.width;
                current.push(atom);
            }
            _ => {
                let width = atom_width(&atom);
                if !current.is_empty() && current_width + width > max_width {
                    flush_line(&mut current, &mut lines);
                    current_width = 0.0;
                }
                current_width += width;
                current.push(atom);
            }
        }
    }
    if !current.is_empty() {
        flush_line(&mut current, &mut lines);
    }
    lines
}

fn flush_line(current: &mut Vec<Atom>, lines: &mut Vec<Vec<Atom>>) {
    while matches!(current.last(), Some(Atom::Text(t)) if t.is_glue) {
        current.pop();
    }
    lines.push(std::mem::take(current));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StyledRun;
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    #[test]
    fn build_atom_stream_splices_a_box_between_two_text_segments() {
        use crate::model::{InlineBox, InlineBoxSlot};

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("before", font), StyledRun::new("after", font)];
        // Splice right after run 0's full text ("before"), before run 1
        // ("after") starts.
        let slots = [InlineBoxSlot::new(0, "before".len(), InlineBox::in_flow(1, 10.0, 10.0))];
        let paragraph = Paragraph { runs: &runs, inline_boxes: &slots, align: Default::default(), line_height: None, max_width: 1000.0, break_strategy: Default::default(), hyphenation: Default::default(), max_consecutive_hyphens: Default::default() };
        let shaper = CosmicShaper::headless();

        let atoms = build_atom_stream(&paragraph, &shaper);
        // "before" -> one Text atom, then the spliced Box, then "after"
        // -> one Text atom.
        assert_eq!(atoms.len(), 3);
        assert!(matches!(&atoms[0], Atom::Text(t) if t.run_index == 0 && !t.is_glue));
        assert!(matches!(&atoms[1], Atom::Box(b) if b.id == 1));
        assert!(matches!(&atoms[2], Atom::Text(t) if t.run_index == 1 && !t.is_glue));
    }

    #[test]
    fn pack_lines_never_starts_a_line_with_leading_whitespace() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("one two three", font)];
        let paragraph = Paragraph { runs: &runs, inline_boxes: &[], align: Default::default(), line_height: None, max_width: 40.0, break_strategy: Default::default(), hyphenation: Default::default(), max_consecutive_hyphens: Default::default() };
        let shaper = CosmicShaper::headless();

        let atoms = build_atom_stream(&paragraph, &shaper);
        let lines = pack_lines(atoms, 40.0);
        assert!(lines.len() > 1);
        for line in &lines {
            if let Some(Atom::Text(t)) = line.first() {
                assert!(!t.is_glue, "line must not start with a glue atom");
            }
            if let Some(Atom::Text(t)) = line.last() {
                assert!(!t.is_glue, "line must not end with a trimmed-away glue atom");
            }
        }
    }

    #[test]
    fn pack_lines_places_a_single_overflowing_atom_alone_rather_than_panicking() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("Supercalifragilisticexpialidocious short", font)];
        let paragraph = Paragraph { runs: &runs, inline_boxes: &[], align: Default::default(), line_height: None, max_width: 30.0, break_strategy: Default::default(), hyphenation: Default::default(), max_consecutive_hyphens: Default::default() };
        let shaper = CosmicShaper::headless();

        let atoms = build_atom_stream(&paragraph, &shaper);
        let lines = pack_lines(atoms, 30.0);
        assert!(lines.len() >= 2, "the overflow word and \"short\" must land on separate lines");
    }

    /// Typography-gap WAVE 2 bug fix: two words joined by U+00A0 NO-BREAK
    /// SPACE must land in ONE non-glue [`TextAtom`] (never split), while an
    /// ordinary space at the same width DOES split — the NBSP fixture must
    /// therefore wrap to FEWER lines than the plain-space fixture at an
    /// identical narrow width (the whole "hello<NBSP>world" unit either
    /// fits together or overflows alone onto its own line; it never
    /// straddles a line boundary the way "hello world" freely does).
    #[test]
    fn nbsp_glues_two_words_into_one_non_breaking_atom_never_split_across_lines() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let nbsp_runs = [StyledRun::new("hello\u{00A0}world foo bar", font)];
        let space_runs = [StyledRun::new("hello world foo bar", font)];
        let max_width = 60.0;

        let nbsp_paragraph = Paragraph::new(&nbsp_runs, max_width);
        let space_paragraph = Paragraph::new(&space_runs, max_width);
        let shaper = CosmicShaper::headless();

        let nbsp_atoms = build_atom_stream(&nbsp_paragraph, &shaper);
        // "hello<NBSP>world" must be ONE non-glue Text atom (never two atoms
        // split by a glue atom) — exactly 3 atoms total: the glued
        // compound, a real glue (the plain space before "foo"), "foo", a
        // real glue, "bar" -> but grouped: [compound, glue, "foo", glue, "bar"].
        let text_atom_count = nbsp_atoms.iter().filter(|a| matches!(a, Atom::Text(t) if !t.is_glue)).count();
        assert_eq!(text_atom_count, 3, "hello<NBSP>world must merge into ONE non-glue atom, plus foo/bar = 3 total");
        let compound = nbsp_atoms.iter().find_map(|a| match a {
            Atom::Text(t) if !t.is_glue => Some(t),
            _ => None,
        });
        assert!(compound.is_some(), "the glued compound must exist as a real Text atom");
        let compound_text: String = compound.unwrap().glyphs.iter().map(|g| g.cluster.as_str()).collect();
        assert!(compound_text.contains('\u{00A0}'), "the compound atom must still carry the NBSP cluster itself");

        let nbsp_lines = pack_lines(nbsp_atoms, max_width);
        let space_atoms = build_atom_stream(&space_paragraph, &shaper);
        let space_lines = pack_lines(space_atoms, max_width);

        assert!(
            space_lines.len() > nbsp_lines.len(),
            "an ordinary space at this width must wrap MORE than the NBSP-glued fixture \
             (NBSP prevents the hello/world split entirely): space={} nbsp={}",
            space_lines.len(),
            nbsp_lines.len()
        );

        // The glued compound is never split across two lines: every line
        // that contains ANY glyph from the compound's own cluster set must
        // contain the WHOLE compound (both "hello" and "world" halves).
        for line in &nbsp_lines {
            let has_hello = line.iter().any(|a| matches!(a, Atom::Text(t) if t.glyphs.iter().any(|g| g.cluster == "h")));
            let has_world = line.iter().any(|a| matches!(a, Atom::Text(t) if t.glyphs.iter().any(|g| g.cluster == "w")));
            assert_eq!(has_hello, has_world, "hello/world must never land on different lines when joined by NBSP");
        }
    }

    /// U+202F NARROW NO-BREAK SPACE gets the identical non-breaking
    /// treatment as U+00A0 (both carry Unicode's `White_Space` property but
    /// forbid a line break).
    #[test]
    fn narrow_nbsp_is_also_non_breaking() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("12\u{202F}345 rest of the line", font)];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();

        let atoms = build_atom_stream(&paragraph, &shaper);
        let first_text_atom = atoms.iter().find_map(|a| match a {
            Atom::Text(t) if !t.is_glue => Some(t),
            _ => None,
        });
        let text: String = first_text_atom.unwrap().glyphs.iter().map(|g| g.cluster.as_str()).collect();
        assert!(text.contains('\u{202F}'), "the first non-glue atom must carry the narrow-NBSP-glued compound \"12 345\", got {text:?}");
    }

    /// Justify still works on a line containing an NBSP-glued compound —
    /// the compound itself is never stretched (no glue atom exists inside
    /// it to redistribute onto), but the line's OTHER, real interword
    /// glues still reach `max_width` under `Justify`, exactly like any
    /// other line.
    #[test]
    fn justify_still_works_on_a_line_with_an_nbsp_glued_compound() {
        use crate::model::ParagraphAlign;

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "one two three\u{00A0}four five six seven eight nine ten";
        let runs = [StyledRun::new(text, font)];
        let max_width = 220.0;
        let paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify);
        let shaper = CosmicShaper::headless();

        let layout = crate::layout::layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must wrap to multiple lines");
        let last_index = layout.lines.len() - 1;
        for line in &layout.lines {
            if line.line_index == last_index {
                continue;
            }
            assert!(
                (line.content_width - max_width).abs() < 1.0,
                "non-last line {} must still reach max_width {max_width} under Justify despite the NBSP compound, got {}",
                line.line_index,
                line.content_width
            );
        }
    }

    /// Letter-spacing (typography-gap WAVE 2): measured width grows
    /// linearly with the spacing value, applied per shaped glyph cluster
    /// (never doubled inside a ligature).
    #[test]
    fn letter_spacing_grows_measured_width_linearly() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "Spacing";

        let base_runs = [StyledRun::new(text, font)];
        let base_paragraph = Paragraph::new(&base_runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let base_layout = crate::layout::layout_paragraph(&base_paragraph, &shaper);

        let spaced_runs = [StyledRun::new(text, font).with_letter_spacing(4.0)];
        let spaced_paragraph = Paragraph::new(&spaced_runs, f64::MAX);
        let spaced_layout = crate::layout::layout_paragraph(&spaced_paragraph, &shaper);

        let glyph_count = base_layout.glyphs.len();
        assert_eq!(glyph_count, spaced_layout.glyphs.len(), "letter-spacing must not change the glyph count");
        let expected_extra = 4.0 * glyph_count as f64;
        assert!(
            (spaced_layout.width - base_layout.width - expected_extra).abs() < 0.5,
            "spaced width {} must exceed base width {} by ~{expected_extra} (one spacing bump per glyph cluster)",
            spaced_layout.width,
            base_layout.width
        );
    }

    /// Sub/superscript (typography-gap WAVE 2): a `Super`/`Sub` run's
    /// glyphs shape at a genuinely smaller font (`GlyphLayout::font.size_px`
    /// shrinks) and paint at a shifted baseline `y` relative to a plain
    /// baseline run sharing the same line.
    #[test]
    fn superscript_and_subscript_shrink_the_shape_font_and_shift_the_baseline() {
        use crate::model::VerticalAlign;

        let font = FontSpec::new(FontFamily::Roboto, 20.0);
        let base_run = StyledRun::new("x", font);
        let sup_run = StyledRun::new("2", font).with_vertical_align(VerticalAlign::Super);
        let sub_run = StyledRun::new("n", font).with_vertical_align(VerticalAlign::Sub);
        let runs = [base_run, sup_run, sub_run];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();

        let layout = crate::layout::layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.lines.len(), 1);

        let base_glyph = layout.glyphs.iter().find(|g| g.run_index == 0).expect("base glyph");
        let sup_glyph = layout.glyphs.iter().find(|g| g.run_index == 1).expect("superscript glyph");
        let sub_glyph = layout.glyphs.iter().find(|g| g.run_index == 2).expect("subscript glyph");

        assert!(sup_glyph.font.size_px < base_glyph.font.size_px, "superscript must shape at a smaller font size");
        assert!(sub_glyph.font.size_px < base_glyph.font.size_px, "subscript must shape at a smaller font size");
        assert!(sup_glyph.y < base_glyph.y, "superscript must paint ABOVE the baseline run (smaller y)");
        assert!(sub_glyph.y > base_glyph.y, "subscript must paint BELOW the baseline run (larger y)");
    }
}
