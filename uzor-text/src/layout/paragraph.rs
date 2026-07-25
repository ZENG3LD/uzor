//! [`layout_paragraph`] — Phase 2 multi-run entry point: rich spans +
//! [`crate::model::InlineBox`] + baseline pass over a [`Paragraph`].

use crate::linebreak::BreakStrategy;
use crate::model::{InlineBox, Paragraph, ParagraphAlign, ProtrusionTable, StyledRun};
use crate::shape::LineShaper;

use super::baseline::resolve_line_metrics;
use super::glyph_layout::{DecorationKind, DecorationSpan, GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};
use super::greedy::{self, Atom};

/// Justification-overshoot fix (typography track T6, 2026-07-25): whether a
/// [`layout_paragraph`]/[`layout_paragraph_diagnosed`] call had to render
/// at least one line wider than `paragraph.max_width` — always the honest
/// truth about the ACTUAL rendered output (measured post-shrink, from the
/// same `content_width` [`LineBox::content_width`] already carries), never
/// a proxy over [`crate::linebreak::knuth_plass`]'s own internal DP state.
///
/// A justified/`KnuthPlass` paragraph reaches `overfull_line_count > 0`
/// only when [`crate::linebreak::knuth_plass::pack_lines`]'s own PRIMARY,
/// feasibility-gated DP pass found NO fully feasible breaking for the whole
/// paragraph and fell back to the deliberate, TeX-style "overfull hbox"
/// pass (see that module's own doc comment) — every OTHER paragraph gets
/// `overfull_fallback_used: false` structurally (the fixed defect this
/// track closes: a candidate line whose shrink NEED exceeds its own
/// render-time shrink CAPACITY is now rejected as infeasible rather than
/// merely priced in, so a feasible breaking — when one exists — never
/// overshoots at all).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineBreakDiagnostics {
    /// `true` iff at least one returned line's rendered content exceeds
    /// `paragraph.max_width` — the deliberate overfull-hbox fallback fired
    /// (or, in principle, some other unrelated defect — this flag reports
    /// the OBSERVED symptom, not "the fallback specifically ran", though in
    /// this crate's current implementation the two coincide exactly for
    /// [`BreakStrategy::KnuthPlass`], and [`BreakStrategy::Greedy`] never
    /// sets it at all — see [`crate::layout::greedy::pack_lines`]'s own
    /// documented invariant that it never produces an over-full non-last
    /// line with glue on it).
    pub overfull_fallback_used: bool,
    /// How many of the returned layout's own lines are genuinely
    /// over-full — `0` unless `overfull_fallback_used`.
    pub overfull_line_count: usize,
}

/// Tolerance for [`LineBreakDiagnostics`]'s own "is this line over-full"
/// check — absorbs floating-point summation noise across the per-line
/// glue-extras accumulation below, not a visual/layout tolerance (compare
/// [`crate::linebreak::knuth_plass`]'s own `SHRINK_FEASIBILITY_EPSILON`,
/// the same order of magnitude for the identical reason).
const OVERFULL_EPSILON: f64 = 1e-6;

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
///
/// A thin wrapper over [`layout_paragraph_diagnosed`] that discards its own
/// [`LineBreakDiagnostics`] — kept as its OWN, byte-for-byte-unchanged
/// public function (never folded into a tuple-returning signature) because
/// this is the one function every existing caller across this workspace
/// already calls, including `uzor-typeset` (a sibling crate this task is
/// scoped to never edit) — see [`layout_paragraph_diagnosed`]'s own doc
/// comment for why the diagnostic is a NEW, additive entry point instead of
/// a new [`ParagraphLayout`] field (the same "sibling crate constructs this
/// public struct via an exhaustive field literal" constraint
/// [`apply_protrusion`]'s own doc comment already documents for
/// [`LineBox`]).
pub fn layout_paragraph(paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> ParagraphLayout {
    layout_paragraph_diagnosed(paragraph, shaper).0
}

/// [`layout_paragraph`], plus a [`LineBreakDiagnostics`] reporting whether
/// this call had to render an over-full line (typography track T6,
/// 2026-07-25 — see that type's own doc comment for the full mechanism).
///
/// A genuinely NEW, additive public entry point rather than a breaking
/// change to [`layout_paragraph`]'s own signature or a new field on
/// [`ParagraphLayout`]: `ParagraphLayout` is constructed via an EXHAUSTIVE
/// field-by-field literal in `uzor-typeset::compose::paragraph_split::
/// slice_layout_lines` (a sibling crate this task's own scope forbids
/// editing) — adding a field there would silently break that crate's own
/// build. Every existing caller of [`layout_paragraph`] (in this crate and
/// every downstream one) is completely unaffected; a caller that wants the
/// diagnostic opts in by calling this function instead.
pub fn layout_paragraph_diagnosed(paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> (ParagraphLayout, LineBreakDiagnostics) {
    let atoms = greedy::build_atom_stream(paragraph, shaper);
    let packed = match paragraph.break_strategy {
        BreakStrategy::Greedy => greedy::pack_lines(atoms, paragraph.max_width),
        BreakStrategy::KnuthPlass => crate::linebreak::knuth_plass::pack_lines(atoms, paragraph, shaper),
    };

    if packed.is_empty() {
        return (ParagraphLayout::default(), LineBreakDiagnostics::default());
    }

    let mut glyphs = Vec::new();
    let mut lines = Vec::with_capacity(packed.len());
    let mut boxes = Vec::new();
    let mut y_top = 0.0_f64;
    let last_index = packed.len() - 1;
    // Typography track T4 (protrusion): per-line "does this edge actually
    // sit flush against the margin" signal, indexed by `line_index` —
    // needed at the very end (`apply_protrusion`, after every line is
    // built) but computed HERE, inline, since it depends on `can_justify`
    // (below), which is itself only known per-line inside this loop. NOT
    // stored on `LineBox` itself: `LineBox` is a public struct a sibling
    // crate (`uzor-typeset::compose::paragraph_split::slice_layout_lines`)
    // constructs via an EXHAUSTIVE field-by-field literal (no `..`
    // functional-update spread) — adding a field there would be a breaking
    // change to a crate this task is explicitly forbidden from touching.
    // A local, parallel `Vec<bool>` carries the identical "property of the
    // line" information the coordinator asked for without that risk.
    let mut line_flush_start: Vec<bool> = Vec::with_capacity(packed.len());
    let mut line_flush_end: Vec<bool> = Vec::with_capacity(packed.len());
    // Typography track T6 (justification-overshoot fix): counts lines
    // whose OWN `content_width` (the same value pushed onto `LineBox`,
    // computed pre-protrusion — see this loop's own `LineBox::push` call
    // below) exceeds `paragraph.max_width`. Deliberately measured against
    // the ACTUAL rendered width, never a proxy over the DP's own internal
    // state, so this stays correct even for a mismatch this crate's own
    // `glue_extras_for_line` doc comment already flags (the one-letter-word
    // protected-glue exemption can make the REAL render-time shrink
    // capacity narrower than what `knuth_plass::span_metrics` assumed).
    let mut overfull_line_count = 0usize;

    for (line_index, line_atoms) in packed.into_iter().enumerate() {
        let natural_width: f64 = line_atoms.iter().map(greedy::atom_width).sum();
        let line_metrics =
            resolve_line_metrics(line_atoms.iter().map(greedy::atom_metrics), paragraph.line_height);

        let glue_count = line_atoms.iter().filter(|a| matches!(a, Atom::Text(t) if t.is_glue)).count();
        let can_justify = paragraph.align == ParagraphAlign::Justify
            && line_index != last_index
            && glue_count > 0
            && paragraph.max_width.is_finite();
        // Typography track T4: a margin-flush signal per edge, per line.
        // START (left): flush for every line under `Left`/`Justify` — both
        // always resolve `align_shift == 0.0` (see the match below),
        // INCLUDING a `Justify` paragraph's own ragged LAST line (Justify
        // never loosens the LEFT edge, only the right). `Right`/`Center`
        // shift the whole line away from the left margin, so neither is
        // ever start-flush. END (right): flush only for a line that was
        // ACTUALLY stretched to the measure (`can_justify` — already
        // excludes the last line and any line with no glue to stretch) or
        // for `Right` align (which, by construction, always sets every
        // line's right edge at exactly `max_width` when finite — there is
        // no "ragged" case under `Right`). A line that merely falls short
        // of `max_width` (the common `Left`/`Center` case, and a
        // `Justify` paragraph's own last line) is never end-flush —
        // hanging punctuation past a margin the line's own text never
        // reached would visibly detach it from its own word.
        let end_flush = match paragraph.align {
            ParagraphAlign::Right => paragraph.max_width.is_finite(),
            ParagraphAlign::Justify => can_justify,
            ParagraphAlign::Left | ParagraphAlign::Center => false,
        };
        let start_flush = matches!(paragraph.align, ParagraphAlign::Left | ParagraphAlign::Justify);
        line_flush_start.push(start_flush);
        line_flush_end.push(end_flush);
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
        let glue_extras = if can_justify || needs_shrink {
            glue_extras_for_line(
                &line_atoms,
                glue_count,
                (paragraph.max_width - natural_width) / glue_count as f64,
                paragraph.line_break_params.glue_shrink_ratio,
            )
        } else {
            vec![0.0; glue_count]
        };
        let content_width = natural_width + glue_extras.iter().sum::<f64>();

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
        let mut glue_ordinal = 0usize;

        for atom in &line_atoms {
            match atom {
                Atom::Text(t) => {
                    let run = &paragraph.runs[t.run_index];
                    // Typography-gap WAVE 2 (sub/superscript): shift is
                    // resolved against the run's own NOMINAL font size
                    // (never `t.shape_font`, which is already shrunk under
                    // `Super`/`Sub` — em-ratios are defined against the
                    // unscaled size, matching standard OpenType script-
                    // metric convention, see `VerticalAlign::baseline_shift`'s
                    // own doc comment).
                    let vshift = run.vertical_align.baseline_shift(run.font.size_px);
                    for g in &t.glyphs {
                        glyphs.push(GlyphLayout {
                            cluster: g.cluster.clone(),
                            run_index: t.run_index,
                            line_index,
                            x: pen_x + g.x,
                            y: baseline_y + g.y_offset + vshift,
                            advance: g.advance,
                            width: g.width,
                            // The font glyphs were ACTUALLY shaped at (see
                            // `TextAtom::shape_font`'s own doc comment) —
                            // never `run.font` directly, so a painter that
                            // trusts `GlyphLayout::font` for `set_font`
                            // never mismatches a shrunk sub/superscript
                            // glyph's already-resolved position/width.
                            font: t.shape_font,
                            color: run.color,
                        });
                    }
                    pen_x += t.width;
                    if t.is_glue {
                        pen_x += glue_extras[glue_ordinal];
                        glue_ordinal += 1;
                    }
                }
                Atom::Box(inline_box) => {
                    boxes.push(placed_box(*inline_box, line_index, pen_x, baseline_y));
                    pen_x += inline_box.width();
                }
                Atom::Break => {}
            }
        }

        if paragraph.max_width.is_finite() && content_width > paragraph.max_width + OVERFULL_EPSILON {
            overfull_line_count += 1;
        }

        lines.push(LineBox { line_index, y_top, baseline_y, height: line_metrics.height, content_width });
        y_top += line_metrics.height;
    }

    if let Some(table) = paragraph.protrusion {
        apply_protrusion(&mut glyphs, &lines, &line_flush_start, &line_flush_end, table);
    }

    let width = lines.iter().map(|l| l.content_width).fold(0.0_f64, f64::max);
    let height = lines.last().map(|l| l.y_top + l.height).unwrap_or(0.0);
    let decorations = build_decoration_spans(&glyphs, paragraph.runs);

    let layout = ParagraphLayout { glyphs, lines, boxes, decorations, width, height };
    let diagnostics = LineBreakDiagnostics { overfull_fallback_used: overfull_line_count > 0, overfull_line_count };
    (layout, diagnostics)
}

/// Typography track T4 (microtypography, hanging punctuation): shift each
/// line's own FIRST/LAST rendered glyph — if that glyph is a single
/// character present in `table`, AND that specific edge of that specific
/// line is actually set flush against the margin (`flush_start[line_index]`/
/// `flush_end[line_index]` — see [`layout_paragraph`]'s own inline
/// computation of both, immediately above where they're collected) — by
/// its own `start`/`end` protrusion factor, expressed as a fraction of
/// THAT glyph's own already-resolved `advance` (never a fixed px value —
/// a bold/larger run's wider comma hangs a proportionally wider amount
/// than a small caption's).
///
/// **The flush gate is load-bearing, not decorative** (found + fixed
/// after an owner review of the first version's own proof PNG, which
/// applied the shift unconditionally): a line whose text merely falls
/// short of `max_width` — the paragraph's own ragged LAST line under
/// `Justify`, or any line at all under plain `Left`/`Center` — has no
/// margin at that edge to hang past; shifting its own trailing glyph
/// there only pulls it away from the word it belongs to, visibly
/// detaching it (the exact defect: "line after line ." with a stray gap
/// before the period, instead of "line after line.").
///
/// A pure geometry post-process over the ALREADY fully-resolved `glyphs`
/// (baseline, justify/shrink, alignment — every earlier pass in this
/// function already ran): only the affected glyph's own `x` moves; every
/// OTHER glyph's `x` was computed independently of any other glyph's PAINT
/// position (only from its own run's advances), so this can never desync
/// anything downstream of it.
///
/// **Scoped, honestly reported** (see [`ProtrusionTable`]'s own doc
/// comment, and this function's own doc comment continuation below the
/// flush-gate note): this does NOT change which atoms
/// [`crate::layout::greedy::pack_lines`]/
/// [`crate::linebreak::knuth_plass::pack_lines`] chose to end/start a line
/// — those FIT decisions are untouched; only the already-decided edge
/// glyph's own PAINT position moves, AND — new limitation, reported
/// explicitly, not silently — the interword glue on a `Justify`-stretched
/// line was ALREADY distributed (by the per-line loop above, before this
/// function ever runs) against the FULL, un-discounted natural width. Real
/// TeX-style protrusion reduces a protruding edge character's EFFECTIVE
/// width before that stretch/shrink target is computed, so the remaining
/// glue absorbs a hair more/less. This function's own paint-time-only
/// shift does not do that: the glue on an affected line is stretched
/// exactly as if no character were about to hang, then the edge glyph is
/// shifted past the margin on top. The resulting per-gap discrepancy is
/// `(protruding character's own advance * factor) / glue_count` — for a
/// typical period/comma (~10-12px at 16px body text) across a realistic
/// 6-10-gap line, on the order of ~1-2px per gap, likely SUBLIMINAL at
/// ordinary body-text sizes but not zero, and potentially more noticeable
/// on a short, few-word justified line or at a larger display size.
/// Feeding protrusion into the justification step for real would mean
/// threading a per-line "discount the trailing/leading protruding
/// character's width by `factor * advance` before computing `raw` (the
/// Justify stretch target)" adjustment into the SAME per-line loop above —
/// straightforward there — AND into BOTH breaker algorithms' own fit/
/// badness math (`greedy::pack_lines`'s width comparison,
/// `knuth_plass::span_metrics`'s width/stretch/shrink), so a candidate
/// line's FIT decision also credits the discount consistently with how it
/// will eventually render — the deeper "applied during line breaking"
/// integration this crate's own protrusion doc comment already scoped OUT
/// of this pass for the same reason (real work across two independently-
/// evolving DP implementations, for a sub-pixel-scale visual refinement
/// on top of the already-delivered "hangs past the margin" win).
fn apply_protrusion(glyphs: &mut [GlyphLayout], lines: &[LineBox], flush_start: &[bool], flush_end: &[bool], table: &ProtrusionTable) {
    let mut first_idx: Vec<Option<usize>> = vec![None; lines.len()];
    let mut last_idx: Vec<Option<usize>> = vec![None; lines.len()];
    for (i, g) in glyphs.iter().enumerate() {
        if g.cluster.is_empty() {
            continue; // ligature continuation — carries no independent extent
        }
        if let Some(slot) = first_idx.get_mut(g.line_index) {
            slot.get_or_insert(i);
        }
        if let Some(slot) = last_idx.get_mut(g.line_index) {
            *slot = Some(i);
        }
    }

    for line_index in 0..lines.len() {
        if flush_start.get(line_index).copied().unwrap_or(false) {
            if let Some(i) = first_idx[line_index] {
                protrude_start(&mut glyphs[i], table);
            }
        }
        if flush_end.get(line_index).copied().unwrap_or(false) {
            if let Some(i) = last_idx[line_index] {
                protrude_end(&mut glyphs[i], table);
            }
        }
    }
}

fn protrude_start(glyph: &mut GlyphLayout, table: &ProtrusionTable) {
    if let Some(ch) = single_char(&glyph.cluster) {
        let factors = table.get(ch);
        if factors.start > 0.0 {
            glyph.x -= factors.start * glyph.advance;
        }
    }
}

fn protrude_end(glyph: &mut GlyphLayout, table: &ProtrusionTable) {
    if let Some(ch) = single_char(&glyph.cluster) {
        let factors = table.get(ch);
        if factors.end > 0.0 {
            glyph.x += factors.end * glyph.advance;
        }
    }
}

/// `Some(c)` iff `s` is exactly one character (never a multi-codepoint
/// cluster/ligature) — protrusion only ever applies to a single, literal
/// punctuation character, matching [`ProtrusionTable`]'s own `char` key.
fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_none() {
        Some(c)
    } else {
        None
    }
}

/// `true` for a non-glue [`Atom::Text`] spanning exactly one glyph — a
/// genuine one-letter word (e.g. Russian "а"/"и"/"в"/"с"/"у"/"о", the
/// single-letter prepositions/conjunctions this fn exists for).
/// [`crate::linebreak::hyphenate`]'s own `LEFT_MIN`/`RIGHT_MIN` guards
/// (never break inside the first 2 or last 3 letters of a word) mean a
/// real hyphenation FRAGMENT is never a single glyph either — so this
/// check can never misfire on a word fragment, only on an actual
/// one-letter word.
fn is_single_glyph_word(atom: &Atom) -> bool {
    matches!(atom, Atom::Text(t) if !t.is_glue && t.glyphs.len() == 1)
}
/// Which of `line_atoms`' own glue atoms sit immediately beside a
/// one-letter word (either side) — parallel to glue-occurrence order
/// (index `i` here corresponds to the `i`-th `Atom::Text` with
/// `is_glue == true`, walking `line_atoms` left to right), matching
/// [`glue_extras_for_line`]'s own indexing.
fn protected_glue_mask(line_atoms: &[Atom]) -> Vec<bool> {
    let mut mask = Vec::new();
    for (i, atom) in line_atoms.iter().enumerate() {
        if !matches!(atom, Atom::Text(t) if t.is_glue) {
            continue;
        }
        let before = i > 0 && is_single_glyph_word(&line_atoms[i - 1]);
        let after = i + 1 < line_atoms.len() && is_single_glyph_word(&line_atoms[i + 1]);
        mask.push(before || after);
    }
    mask
}

/// Per-glue `pen_x` adjustment for one packed line (Justify stretch, or
/// shrink for an over-full line regardless of alignment — see
/// [`layout_paragraph`]'s own call site docs) — `raw` is the naive
/// "spread `max_width - natural_width` evenly across every glue" value
/// the pre-fix code always applied uniformly.
///
/// **The one-letter-word fix**: a NEGATIVE `raw` (the line must shrink)
/// no longer shrinks every glue by the same amount. A glue immediately
/// beside a one-letter word ([`protected_glue_mask`]) is exempt —
/// pinned at its own full natural width — and the whole deficit is
/// redistributed across the REMAINING glues only (still capped at
/// `shrink_ratio` — [`crate::model::Paragraph::line_break_params`]'s own
/// `glue_shrink_ratio`, typography track T5 — of their own narrowest
/// natural width, same floor the pre-fix code already used, just scoped
/// to the regular glues now). Known, previously-unfixed bug this closes: uniformly
/// shrinking every glue by an equal fraction is fine in general, but for
/// a short, common one-letter conjunction ("а"/"и"/"в") sitting between
/// two ordinary, ink-dense letters, that SAME fractional shrink reads as
/// a full visual collapse (the two words touching, e.g. "текста, а
/// узкая" rendering as "текста, аузкая") even though the pixel math is
/// perfectly uniform with every other glue on the line — confirmed by
/// rendering `uzor-typeset`'s own `typography_wave` proof and inspecting
/// the resulting pixels directly (see this module's
/// `single_letter_word_glue_is_never_shrunk_even_on_a_tight_justified_line`
/// regression test, using the exact same fixture text/width). Positive
/// `raw` (stretch) is untouched: a WIDER gap around a short word never
/// collapses anything, so every glue — protected or not — stretches
/// identically, exactly like the pre-fix behavior.
fn glue_extras_for_line(line_atoms: &[Atom], glue_count: usize, raw: f64, shrink_ratio: f64) -> Vec<f64> {
    if raw >= 0.0 {
        return vec![raw; glue_count];
    }

    let protected = protected_glue_mask(line_atoms);
    debug_assert_eq!(protected.len(), glue_count);
    let glue_widths: Vec<f64> = line_atoms
        .iter()
        .filter_map(|a| if let Atom::Text(t) = a { t.is_glue.then_some(t.width) } else { None })
        .collect();

    let regular_count = protected.iter().filter(|&&p| !p).count();
    let deficit = raw * glue_count as f64; // == max_width - natural_width, negative
    if regular_count == 0 {
        // Degenerate: every glue on this line is one-letter-word-
        // adjacent (an extremely short line) — protecting all of them
        // would leave the deficit nowhere to go, so fall back to the
        // ORIGINAL uniform-shrink behavior rather than leaving an
        // over-full line completely un-shrunk.
        let floor = -(glue_widths.iter().copied().fold(f64::MAX, f64::min) * shrink_ratio);
        return vec![raw.max(floor); glue_count];
    }

    let min_regular_width =
        glue_widths.iter().zip(&protected).filter_map(|(&w, &p)| (!p).then_some(w)).fold(f64::MAX, f64::min);
    let floor = -(min_regular_width * shrink_ratio);
    let per_regular = (deficit / regular_count as f64).max(floor);

    protected.iter().map(|&p| if p { 0.0 } else { per_regular }).collect()
}

/// Underline offset below the baseline, in em (fraction of the owning
/// run's own `font.size_px`) — typography-gap WAVE 2. A sane fixed
/// fallback, not a real font-embedded `underlinePosition` (see
/// [`crate::model::VerticalAlign`]'s own doc comment for the identical
/// "no in-crate path to a real font's own metrics" reasoning — same
/// dependency-boundary law applies here).
const UNDERLINE_OFFSET_EM: f64 = 0.08;

/// Strikethrough offset ABOVE the baseline, in em — roughly mid x-height,
/// matching the common CSS/OpenType convention closely enough to read
/// correctly in every embedded font this workspace ships.
const STRIKETHROUGH_OFFSET_EM: f64 = 0.30;

/// Decoration rule thickness, in em, floored at [`MIN_DECORATION_THICKNESS_PX`].
const DECORATION_THICKNESS_EM: f64 = 0.06;

/// Minimum decoration rule thickness in px — keeps a rule visible even for
/// a very small font size.
const MIN_DECORATION_THICKNESS_PX: f64 = 1.0;

/// Build [`DecorationSpan`]s from `glyphs` (already placed, in the SAME
/// line-major, left-to-right order [`layout_paragraph`]'s own glyph-push
/// loop produces) + `runs` (for each glyph's own
/// [`crate::model::StyledRun::decoration`]/`font`/`color`).
///
/// A span is a maximal run of consecutive glyphs sharing `(line_index,
/// run_index)` under a non-[`crate::model::TextDecoration::is_none`] run,
/// WITH NO POSITION GAP between consecutive glyphs (`next.x` must equal
/// `prev.x + prev.advance`, within a small epsilon) — the gap check is
/// what correctly breaks a span across a spliced [`InlineBox`] landing
/// mid-run (the box itself produces no glyph, so two glyphs either side of
/// it share the same `(line_index, run_index)` but are NOT adjacent in x),
/// without this function needing any visibility into the atom stream's own
/// `Atom::Box` entries.
///
/// `y`/`thickness` are resolved directly from each span's own FIRST
/// glyph's already-absolute `y` (which already folds in
/// [`crate::model::VerticalAlign::baseline_shift`] for a `Super`/`Sub`
/// run — so a decorated superscript's own rule tracks ITS OWN raised
/// baseline, never the line's shared one) — no second baseline lookup.
fn build_decoration_spans(glyphs: &[GlyphLayout], runs: &[StyledRun<'_>]) -> Vec<DecorationSpan> {
    const GAP_EPSILON: f64 = 0.01;

    let mut spans = Vec::new();
    // (line_index, run_index, x_start, x_end, baseline_y — the span's own
    // first glyph's already-absolute y)
    let mut current: Option<(usize, usize, f64, f64, f64)> = None;

    for g in glyphs {
        if g.cluster.is_empty() {
            continue; // ligature continuation — carries no independent extent
        }
        let Some(run) = runs.get(g.run_index) else { continue };
        if run.decoration.is_none() {
            flush_decoration_span(current.take(), runs, &mut spans);
            continue;
        }

        let continues = current
            .is_some_and(|(line, run_idx, _, end_x, _)| line == g.line_index && run_idx == g.run_index && (g.x - end_x).abs() < GAP_EPSILON);

        if continues {
            if let Some(span) = &mut current {
                span.3 = g.x + g.advance;
            }
        } else {
            flush_decoration_span(current.take(), runs, &mut spans);
            current = Some((g.line_index, g.run_index, g.x, g.x + g.advance, g.y));
        }
    }
    flush_decoration_span(current.take(), runs, &mut spans);
    spans
}

/// Flush `current` (if any) into `spans` — looks the owning run back up by
/// `current`'s own stored `run_index` (never the caller's "whatever glyph
/// triggered this flush" run, which may belong to a DIFFERENT, just-
/// started span when a decorated run transitions directly into another
/// decorated run with no gap in between).
fn flush_decoration_span(current: Option<(usize, usize, f64, f64, f64)>, runs: &[StyledRun<'_>], spans: &mut Vec<DecorationSpan>) {
    let Some((line_index, run_index, x_start, x_end, baseline_y)) = current else { return };
    let Some(run) = runs.get(run_index) else { return };
    let size = run.font.size_px;
    let thickness = (size * DECORATION_THICKNESS_EM).max(MIN_DECORATION_THICKNESS_PX);

    if run.decoration.underline {
        spans.push(DecorationSpan {
            run_index,
            line_index,
            kind: DecorationKind::Underline,
            x_start,
            x_end,
            y: baseline_y + size * UNDERLINE_OFFSET_EM,
            thickness,
            color: run.color,
        });
    }
    if run.decoration.strikethrough {
        spans.push(DecorationSpan {
            run_index,
            line_index,
            kind: DecorationKind::Strikethrough,
            x_start,
            x_end,
            y: baseline_y - size * STRIKETHROUGH_OFFSET_EM,
            thickness,
            color: run.color,
        });
    }
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

    /// Regression fixture, RE-BUILT (typography track T6, 2026-07-25 — see
    /// `crate::linebreak::knuth_plass`'s own top doc comment for the
    /// justification-overshoot fix this file documents the fallout of).
    ///
    /// The ORIGINAL fixture here (a Russian paragraph mirroring
    /// `uzor-typeset`'s own `typography_wave_ru_and_en_hyphenation_in_a_
    /// narrow_justified_column` proof text/width verbatim) stopped
    /// exercising this mechanism once the T6 feasibility fix landed:
    /// probed directly (swept every 2px from 80 to 260), the specific
    /// line carrying the standalone one-letter "а" NEVER lands on a
    /// genuinely shrunk line anymore at ANY width in that range — the
    /// primary, feasibility-gated DP pass now systematically prefers a
    /// breaking where that particular segment is stretched (or an exact
    /// fit) rather than over-full-then-shrunk, since a strictly cheaper,
    /// fully feasible alternative exists nearby for THIS text. This is
    /// not a defect: shrink usage elsewhere in the SAME corpus is
    /// unaffected (confirmed directly — plenty of other lines in the
    /// same paragraph, at other widths, still shrink normally), it is
    /// simply that this ONE (text, width) pair no longer happens to
    /// route through the specific over-full line the old fixture relied
    /// on. A fresh, purpose-built ENGLISH fixture below (word lengths
    /// probed directly, not guessed) reliably reproduces the identical
    /// scenario this test exists to guard — a genuinely shrunk justified
    /// line containing a one-letter word — under the NEW, fixed DP.
    ///
    /// A shrunk, justified line whose single-letter word "a" sits between
    /// "along" and "road," must render with its OWN surrounding glue
    /// UNSHRUNK (`glue.advance`, exactly — the pre-existing, unrelated
    /// fix this test guards floors shrink at "0", never "less"), while at
    /// least one OTHER (non-adjacent) glue on the same line is still
    /// measurably compressed below its own natural width — proving the
    /// fix actually engages (redistributes the deficit elsewhere) rather
    /// than being a no-op that silently stopped shrinking anything.
    #[test]
    fn single_letter_word_glue_is_never_shrunk_even_on_a_tight_justified_line() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        // Exactly ONE standalone one-letter word ("a", between "along" and
        // "road,") — deliberately not "...to a store..." too, which would
        // give the `.find()` below two candidate lines to choose between
        // (an earlier version of this fixture had that ambiguity and
        // picked the WRONG — merely stretched, not shrunk — line).
        const TEXT: &str = "Walking to my store downtown continues further along a road, then home.";
        let runs = [StyledRun::new(TEXT, font)];
        let max_width = 250.0; // probed directly: line 1 renders genuinely shrunk elsewhere, with "a"'s own glue exempt
        let paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify).with_break_strategy(BreakStrategy::KnuthPlass);
        let shaper = CosmicShaper::headless();
        let (layout, diag) = crate::layout::layout_paragraph_diagnosed(&paragraph, &shaper);
        assert!(!diag.overfull_fallback_used, "this fixture must stay in the primary, feasibility-gated pass — a genuinely feasible, within-capacity shrink, not the deliberate overfull-hbox fallback");

        // Find the line carrying the standalone one-letter "a" (never the
        // "a" inside "along"/"road" etc — a glyph run whose OWN
        // line-relative neighbors are glue on both sides).
        let target_line = layout
            .lines
            .iter()
            .find(|line| {
                let glyphs: Vec<&GlyphLayout> = layout.glyphs.iter().filter(|g| g.line_index == line.line_index).collect();
                glyphs.windows(3).any(|w| w[0].cluster == " " && w[1].cluster == "a" && w[2].cluster == " ")
            })
            .expect("fixture must wrap the standalone \"a\" onto some line");

        let mut glyphs: Vec<&GlyphLayout> = layout.glyphs.iter().filter(|g| g.line_index == target_line.line_index).collect();
        glyphs.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        let a_pos = glyphs.windows(3).position(|w| w[0].cluster == " " && w[1].cluster == "a" && w[2].cluster == " ").unwrap();
        let glue_before = glyphs[a_pos];
        let a_glyph = glyphs[a_pos + 1];
        let glue_after = glyphs[a_pos + 2];
        let next_glyph = glyphs[a_pos + 3];

        // Both glues touching the one-letter word render at their own
        // full natural advance — the effective on-screen gap (next
        // glyph's x minus this glue's own x) must equal `glue.advance`
        // (not `glue.advance` minus a shrink amount).
        let gap_before = a_glyph.x - glue_before.x;
        let gap_after = next_glyph.x - glue_after.x;
        assert!(
            (gap_before - glue_before.advance).abs() < 1e-6,
            "glue before the one-letter word must render unshrunk: gap={gap_before} advance={}",
            glue_before.advance
        );
        assert!(
            (gap_after - glue_after.advance).abs() < 1e-6,
            "glue after the one-letter word must render unshrunk: gap={gap_after} advance={}",
            glue_after.advance
        );
        assert!(gap_after > 0.0, "the rendered gap after a one-letter word must be a real, positive advance");

        // The fix must have actually engaged: some OTHER glue on the
        // same line is measurably shrunk below its own natural width
        // (this line needs compression overall — proving the deficit
        // was redistributed elsewhere, not simply dropped).
        let mut glue_pairs: Vec<(f64, f64)> = Vec::new(); // (gap, natural_advance)
        for w in glyphs.windows(2) {
            if w[0].cluster == " " {
                glue_pairs.push((w[1].x - w[0].x, w[0].advance));
            }
        }
        assert!(
            glue_pairs.iter().any(|&(gap, natural)| gap + 1e-6 < natural),
            "line must still contain at least one genuinely shrunk glue elsewhere, proving the fix redistributed the deficit rather than dropping it: {glue_pairs:?}"
        );
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

    /// Typography-gap WAVE 2: an underlined run produces exactly one
    /// [`crate::layout::DecorationSpan`] whose `y` sits BELOW the line's
    /// own `baseline_y` by the documented fallback offset, spanning the
    /// run's own first-to-last glyph extent; an undecorated run in the
    /// same paragraph produces none.
    #[test]
    fn underlined_run_produces_a_decoration_span_at_the_expected_baseline_offset() {
        use crate::model::TextDecoration;

        let font = FontSpec::new(FontFamily::Roboto, 20.0);
        let plain = StyledRun::new("plain ", font);
        let underlined = StyledRun::new("underlined", font).with_decoration(TextDecoration::underline());
        let runs = [plain, underlined];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.decorations.len(), 1, "only the decorated run may produce a span");

        let span = &layout.decorations[0];
        assert_eq!(span.run_index, 1);
        assert_eq!(span.kind, crate::layout::DecorationKind::Underline);
        assert!(span.thickness > 0.0);

        let baseline_y = layout.lines[0].baseline_y;
        let expected_y = baseline_y + font.size_px * UNDERLINE_OFFSET_EM;
        assert!((span.y - expected_y).abs() < 1e-6, "underline y {} must equal baseline + fallback offset {expected_y}", span.y);

        // The span covers exactly the "underlined" run's own glyph extent.
        let underlined_glyphs: Vec<&GlyphLayout> = layout.glyphs.iter().filter(|g| g.run_index == 1).collect();
        let expected_start = underlined_glyphs.first().unwrap().x;
        let last = underlined_glyphs.last().unwrap();
        let expected_end = last.x + last.advance;
        assert!((span.x_start - expected_start).abs() < 1e-6);
        assert!((span.x_end - expected_end).abs() < 1e-6);
    }

    /// A strikethrough span sits ABOVE the baseline (smaller `y`, roughly
    /// mid x-height) — the opposite direction from underline.
    #[test]
    fn strikethrough_span_sits_above_the_baseline() {
        use crate::model::TextDecoration;

        let font = FontSpec::new(FontFamily::Roboto, 20.0);
        let runs = [StyledRun::new("struck", font).with_decoration(TextDecoration::strikethrough())];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.decorations.len(), 1);
        let span = &layout.decorations[0];
        assert_eq!(span.kind, crate::layout::DecorationKind::Strikethrough);
        assert!(span.y < layout.lines[0].baseline_y, "strikethrough must sit above the baseline");
    }

    /// A paragraph with no decorated runs at all produces zero spans —
    /// never a stray empty/degenerate entry.
    #[test]
    fn undecorated_paragraph_produces_no_decoration_spans() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("plain text, nothing decorated", font)];
        let paragraph = Paragraph::new(&runs, 1000.0);
        let shaper = CosmicShaper::headless();
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.decorations.is_empty());
    }

    /// A decorated run that has an [`InlineBox`] spliced INTO it produces
    /// TWO separate decoration spans (never one span painted straight
    /// through the box's own reserved gap).
    #[test]
    fn decoration_span_breaks_across_a_spliced_inline_box() {
        use crate::model::{InlineBox, InlineBoxSlot, TextDecoration};

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "before after";
        let split_at = "before".len();
        let runs = [StyledRun::new(text, font).with_decoration(TextDecoration::underline())];
        let icon = InlineBox::in_flow(9, 30.0, 10.0);
        let slots = [InlineBoxSlot::new(0, split_at, icon)];
        let paragraph = Paragraph::new(&runs, 1000.0).with_inline_boxes(&slots);
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert_eq!(layout.decorations.len(), 2, "the box must split the underline into two spans, never one span crossing its gap");
        assert!(layout.decorations[0].x_end <= layout.boxes[0].x + 1e-6, "the first span must end at/before the box");
        assert!(layout.decorations[1].x_start >= layout.boxes[0].x + layout.boxes[0].width - 1e-6, "the second span must start at/after the box");
    }

    // ── Typography track T4: protrusion / hanging punctuation (2026-07-25) ──

    /// T4's own required proof: a JUSTIFIED paragraph with protrusion ON
    /// must place the protruding punctuation PAST the measure — the
    /// trailing period of a non-last (justify-stretched) line's own last
    /// glyph must land with `x + advance` exceeding `max_width` by EXACTLY
    /// the period's own configured fraction (`ProtrusionFactors::end_only(1.0)`
    /// — its full own advance).
    #[test]
    fn protrusion_on_a_justified_line_hangs_the_trailing_period_past_the_measure_by_the_expected_fraction() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        // A forced break (`\n`) right after a period makes line 0 END in
        // "." while NOT being the paragraph's last line — Justify still
        // stretches it to reach `max_width` exactly (the "measure" this
        // test protrudes past).
        let text = "One two three four five.\nA second short line follows this one.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 260.0;
        let shaper = CosmicShaper::headless();

        let without = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify);
        let without_layout = layout_paragraph(&without, &shaper);
        assert!(without_layout.lines.len() >= 2, "fixture must produce at least two lines via the forced break");
        let without_last =
            without_layout.glyphs.iter().filter(|g| g.line_index == 0).last().expect("first line must have glyphs");
        assert_eq!(without_last.cluster, ".", "fixture's first line must end in a period");
        let measure_edge = without_last.x + without_last.advance;
        assert!((measure_edge - max_width).abs() < 1.0, "justify must stretch the non-last first line to reach max_width, got {measure_edge}");

        let table = ProtrusionTable::default_punctuation();
        let with = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify).with_protrusion(&table);
        let with_layout = layout_paragraph(&with, &shaper);
        let with_last = with_layout.glyphs.iter().filter(|g| g.line_index == 0).last().expect("first line must have glyphs");
        assert_eq!(with_last.cluster, ".");

        let rendered_edge = with_last.x + with_last.advance;
        assert!(rendered_edge > max_width, "the period must hang PAST the measure, got {rendered_edge} vs max_width {max_width}");
        let overhang = rendered_edge - measure_edge;
        assert!(
            (overhang - with_last.advance).abs() < 1e-6,
            "period (ProtrusionFactors::end_only(1.0)) must hang past the measure by EXACTLY its own full advance, got overhang={overhang} advance={}",
            with_last.advance
        );
    }

    /// T4's other required proof: an EMPTY (all-zero-factor) protrusion
    /// table must be a true no-op — byte-identical output to no table at
    /// all (`protrusion: None`, the default). Combined with
    /// `apply_protrusion` only ever being invoked when `paragraph.
    /// protrusion.is_some()`, this is the structural half of "protrusion
    /// OFF is byte-identical to today": every pre-T4 caller (which never
    /// touches `protrusion`, so it's `None`) skips `apply_protrusion`
    /// entirely, and even if it somehow ran with zero factors, the math
    /// itself changes nothing either.
    #[test]
    fn protrusion_none_and_an_explicit_empty_table_produce_byte_identical_output() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "A justified paragraph with a trailing period, and a comma, too.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 220.0;
        let shaper = CosmicShaper::headless();

        let none_paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify);
        assert_eq!(none_paragraph.protrusion, None, "T4 regression floor: default is no protrusion");
        let none_layout = layout_paragraph(&none_paragraph, &shaper);

        let empty_table = ProtrusionTable::new();
        let empty_paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify).with_protrusion(&empty_table);
        let empty_layout = layout_paragraph(&empty_paragraph, &shaper);

        assert_eq!(none_layout, empty_layout, "an empty (all-zero) protrusion table must be a true no-op — identical output to no table at all");
    }

    /// Protrusion never touches a MIDDLE glyph — only the first/last
    /// rendered glyph of each line moves, even when a protrusion-table
    /// character (a comma) sits in the middle of a line.
    #[test]
    fn protrusion_never_shifts_a_mid_line_glyph_even_if_it_is_a_table_character() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "First, middle, last.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 1000.0; // one line
        let shaper = CosmicShaper::headless();

        let without = Paragraph::new(&runs, max_width);
        let without_layout = layout_paragraph(&without, &shaper);
        assert_eq!(without_layout.lines.len(), 1);

        let table = ProtrusionTable::default_punctuation();
        let with = Paragraph::new(&runs, max_width).with_protrusion(&table);
        let with_layout = layout_paragraph(&with, &shaper);

        assert_eq!(without_layout.glyphs.len(), with_layout.glyphs.len());
        let last_i = without_layout.glyphs.len() - 1;
        for i in 0..without_layout.glyphs.len() {
            if i == 0 || i == last_i {
                continue; // the two edge glyphs are exactly what protrusion is allowed to move
            }
            assert_eq!(
                without_layout.glyphs[i].x, with_layout.glyphs[i].x,
                "glyph {i} ({:?}) is neither the first nor last glyph of its line — protrusion must never move it",
                without_layout.glyphs[i].cluster
            );
        }
    }

    /// **RE-PROVE, per owner review of `text_t4_protrusion_off_vs_on.png`**:
    /// protrusion must NEVER touch the paragraph's own ragged (non-flush)
    /// FINAL line. Before the flush-gate fix, `apply_protrusion` fired
    /// unconditionally on every line's own edge glyph, including a
    /// `Justify` paragraph's own conventionally-ragged last line — visibly
    /// detaching its trailing period from its word ("line after line ."
    /// instead of "line after line."), exactly what the owner caught by
    /// eye. Every glyph on the LAST line must be byte-identical (`x` in
    /// particular) between protrusion ON and OFF — the SAME multi-line,
    /// auto-wrapped Justify+KnuthPlass fixture the visual proof PNG uses,
    /// so this is a direct regression guard for that exact PNG. A second
    /// assertion (an EARLIER, genuinely-flush line's own trailing glyph
    /// still DOES move) proves this isn't merely "protrusion silently
    /// does nothing at all" masquerading as a pass.
    #[test]
    fn protrusion_never_shifts_glyphs_on_the_paragraphs_own_ragged_final_line() {
        use crate::linebreak::BreakStrategy;

        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "Good typography is invisible, or nearly so: a well-set \
            paragraph reads evenly, without ragged holes or crowded lines. \
            Hanging punctuation lets a period, comma, or hyphen protrude \
            slightly past the measure, so the column's right-hand edge \
            reads flush instead of ragged, line after line.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 320.0;
        let shaper = CosmicShaper::headless();

        let without =
            Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify).with_break_strategy(BreakStrategy::KnuthPlass);
        let without_layout = layout_paragraph(&without, &shaper);
        assert!(without_layout.lines.len() > 3, "fixture must wrap to several lines");
        let last_line_index = without_layout.lines.len() - 1;

        let table = ProtrusionTable::default_punctuation();
        let with = Paragraph::new(&runs, max_width)
            .with_align(ParagraphAlign::Justify)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_protrusion(&table);
        let with_layout = layout_paragraph(&with, &shaper);
        assert_eq!(with_layout.lines.len(), without_layout.lines.len(), "protrusion must not change the line count");

        let without_last_line: Vec<&GlyphLayout> = without_layout.glyphs.iter().filter(|g| g.line_index == last_line_index).collect();
        let with_last_line: Vec<&GlyphLayout> = with_layout.glyphs.iter().filter(|g| g.line_index == last_line_index).collect();
        assert_eq!(without_last_line.len(), with_last_line.len());
        for (a, b) in without_last_line.iter().zip(with_last_line.iter()) {
            assert_eq!(a.cluster, b.cluster);
            assert_eq!(
                a.x, b.x,
                "the paragraph's own ragged final line must be BYTE-IDENTICAL between protrusion on/off — cluster {:?} moved from {} to {}",
                a.cluster, a.x, b.x
            );
        }

        // Regression floor for the fix itself: an EARLIER, genuinely
        // justified line's own trailing glyph still DOES move — proves
        // this test can distinguish "correctly gated" from "protrusion
        // silently stopped doing anything at all."
        let moved_on_an_earlier_line = (0..last_line_index).any(|line_index| {
            let a = without_layout.glyphs.iter().filter(|g| g.line_index == line_index).last();
            let b = with_layout.glyphs.iter().filter(|g| g.line_index == line_index).last();
            matches!((a, b), (Some(a), Some(b)) if (a.x - b.x).abs() > 1e-6)
        });
        assert!(
            moved_on_an_earlier_line,
            "at least one non-final (justified) line's own trailing glyph must still protrude, or this test can't tell 'correctly gated' apart from 'silently broken'"
        );
    }
}
