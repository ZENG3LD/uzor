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

use crate::model::{FontSpec, InlineBox, InlineBoxSlot, Paragraph};
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
                push_text_segment(&mut atoms, run_index, &text[cursor..offset], run.font, shaper);
            }
            atoms.push(Atom::Box(slot.inline_box));
            cursor = offset;
        }
        if cursor < text.len() {
            push_text_segment(&mut atoms, run_index, &text[cursor..], run.font, shaper);
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
    font: FontSpec,
    shaper: &dyn LineShaper,
) {
    if segment.is_empty() {
        return;
    }
    // Matches `crate::layout::glyph_layout`'s own fallback-height
    // convention (cosmic-text's `Metrics::new(font_size, font_size *
    // 1.2)`) — reused here, not reinvented.
    let fallback_height = (font.size_px * 1.2).max(1.0);
    let wrapped_lines = shaper.shape_wrapped(segment, &font, f64::MAX);
    for (i, line) in wrapped_lines.iter().enumerate() {
        if i > 0 {
            atoms.push(Atom::Break);
        }
        let ascent = line.baseline_y.max(0.0);
        let descent = (fallback_height - ascent).max(0.0);
        push_glyph_atoms(atoms, run_index, &line.glyphs, ascent, descent);
    }
}

fn push_glyph_atoms(atoms: &mut Vec<Atom>, run_index: usize, glyphs: &[GlyphMetric], ascent: f64, descent: f64) {
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
        let end_x = group.last().map(|g| g.x_offset + g.advance).unwrap_or(base_x);
        let atom_glyphs = group
            .iter()
            .map(|g| AtomGlyph {
                cluster: g.cluster.clone(),
                x: g.x_offset - base_x,
                y_offset: g.y_offset,
                advance: g.advance,
                width: g.width,
            })
            .collect();
        atoms.push(Atom::Text(TextAtom {
            run_index,
            glyphs: atom_glyphs,
            width: end_x - base_x,
            ascent,
            descent,
            is_glue: is_ws,
            hyphen_break: false,
        }));
    }
}

fn is_whitespace_cluster(cluster: &str) -> bool {
    !cluster.is_empty() && cluster.chars().all(char::is_whitespace)
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
        let paragraph = Paragraph { runs: &runs, inline_boxes: &slots, align: Default::default(), line_height: None, max_width: 1000.0, break_strategy: Default::default(), hyphenation: Default::default() };
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
        let paragraph = Paragraph { runs: &runs, inline_boxes: &[], align: Default::default(), line_height: None, max_width: 40.0, break_strategy: Default::default(), hyphenation: Default::default() };
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
        let paragraph = Paragraph { runs: &runs, inline_boxes: &[], align: Default::default(), line_height: None, max_width: 30.0, break_strategy: Default::default(), hyphenation: Default::default() };
        let shaper = CosmicShaper::headless();

        let atoms = build_atom_stream(&paragraph, &shaper);
        let lines = pack_lines(atoms, 30.0);
        assert!(lines.len() >= 2, "the overflow word and \"short\" must land on separate lines");
    }
}
