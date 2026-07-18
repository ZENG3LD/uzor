//! [`pack_lines`] — Knuth-Plass total-fit line breaking: a demerits-minimizing
//! dynamic program over the same [`crate::layout::greedy`] atom stream the
//! greedy packer consumes, simplified per the design doc's own Phase 5 note
//! ("no looseness/fitness classes... unless the doc demands them" — it
//! doesn't): a single-pass DP over feasible breakpoints, no multi-pass
//! looseness adjustment, no TeX fitness-class tiering.
//!
//! [`crate::layout::layout_paragraph`] only swaps which packer turns
//! [`crate::layout::greedy::build_atom_stream`]'s unchanged output into
//! `Vec<Vec<Atom>>` line groups, based on
//! [`crate::linebreak::BreakStrategy`]; every downstream pass (baseline,
//! alignment/justify, box placement) is untouched (design law 1: one
//! measure path — KP only changes *where* the atom stream gets cut).

use std::collections::HashMap;

use crate::layout::greedy::{self, Atom, AtomGlyph, TextAtom};
use crate::model::{FontSpec, Paragraph};
use crate::shape::LineShaper;

use super::hyphenate;
use super::Hyphenation;

/// Interword glue stretch/shrink as a fraction of its natural width — TeX's
/// own conventional interword-space ratios (`stretch = w/2`, `shrink = w/3`).
/// `pub(crate)`: [`crate::layout::paragraph`]'s glue-shrink render pass caps
/// itself at this same ratio, so a line this module's DP scored as "shrink
/// covers the gap" never renders with *more* compression than the cost
/// model actually assumed (never full glue collapse to an unreadable
/// zero-width space).
const GLUE_STRETCH_RATIO: f64 = 0.5;
pub(crate) const GLUE_SHRINK_RATIO: f64 = 1.0 / 3.0;
/// Cost of a discretionary hyphenation break (Knuth's `\hyphenpenalty`,
/// simplified here to one fixed constant rather than a tunable parameter).
const HYPHEN_PENALTY: f64 = 50.0;
/// Extra demerits when two consecutive chosen lines both end in a
/// discretionary hyphen (Knuth's `\doublehyphendemerits`) — discourages a
/// visual "staircase" of hyphens down the margin.
const DOUBLE_HYPHEN_DEMERIT: f64 = 3000.0;
/// Sentinel for a line with genuinely zero stretch/shrink to draw on (e.g. a
/// single unbreakable atom alone on its own line) — large enough that the DP
/// only ever routes through it when truly unavoidable, but still finite (no
/// fallible surface on the hot path): unlike the ordinary cubic badness
/// below, there's no ratio to even compute here (dividing by a zero pool).
const NO_GLUE_BADNESS: f64 = 1.0e12;

/// Knuth's badness: `0.0` for an exact fit, rising *unbounded* as `100 *
/// (needed/available)^3` toward whichever of stretch/shrink the line needs.
/// Deliberately **not** capped at a fixed ceiling: capping it would make a
/// catastrophically over-full line (e.g. the entire paragraph squeezed onto
/// one line) score identically to a merely tight one, letting that single
/// disastrous line's demerits lose a comparison it should always lose
/// against any reasonable multi-line accumulation. A non-finite `target`
/// (unconstrained width) is always `0.0` (matches
/// [`crate::layout::greedy::pack_lines`]'s own convention).
fn badness(natural: f64, target: f64, stretch: f64, shrink: f64) -> f64 {
    if !target.is_finite() {
        return 0.0;
    }
    let diff = target - natural;
    if diff.abs() < 1e-9 {
        return 0.0;
    }
    if diff > 0.0 {
        if stretch <= 0.0 {
            NO_GLUE_BADNESS
        } else {
            100.0 * (diff / stretch).powi(3)
        }
    } else {
        let need = -diff;
        if shrink <= 0.0 {
            NO_GLUE_BADNESS
        } else {
            100.0 * (need / shrink).powi(3)
        }
    }
}

/// Badness of the paragraph's true final line: `\parfillskip`'s effect is
/// *infinite trailing stretch*, so an under-full last line (`natural <=
/// target`) is always `0.0` — but infinite stretch cannot rescue an
/// over-full one (stretch never absorbs a negative gap), so a last-line
/// candidate that still doesn't fit is scored by [`badness`]'s ordinary
/// shrink side. Without this distinction the DP would treat "collapse the
/// entire paragraph into one over-full line" as free (any path ending at
/// the mandatory final breakpoint would look like a zero-badness "last
/// line"), which defeats wrapping entirely.
fn last_line_badness(natural: f64, target: f64, shrink: f64) -> f64 {
    if !target.is_finite() {
        return 0.0;
    }
    if natural <= target {
        0.0
    } else {
        badness(natural, target, 0.0, shrink)
    }
}

/// Knuth-Plass demerits for one chosen line: `(1 + badness + penalty)^2`.
/// This crate never assigns a negative ("encouraged") discretionary
/// penalty, so Knuth's own `p < 0` / `p = -infinity` branches collapse into
/// this single formula with `penalty = 0.0`.
fn demerits(badness: f64, penalty: f64) -> f64 {
    (1.0 + badness + penalty.max(0.0)).powi(2)
}

/// Trim trailing discardables (interword glue, a forced [`Atom::Break`])
/// from the *end* of `atoms` — both are control/whitespace tokens that
/// vanish at the break itself, never rendered and never counted toward the
/// line's width (matches [`crate::layout::greedy::pack_lines`]'s own
/// `flush_line` convention).
fn trim_trailing_discardables(atoms: &[Atom]) -> &[Atom] {
    let mut end = atoms.len();
    while end > 0 {
        match &atoms[end - 1] {
            Atom::Text(t) if t.is_glue => end -= 1,
            Atom::Break => end -= 1,
            _ => break,
        }
    }
    &atoms[..end]
}

/// `(natural_width, stretch, shrink)` of `atoms` after
/// [`trim_trailing_discardables`], adding `hyphen_width` when
/// `ends_in_hyphen` (the discretionary hyphen glyph's width, contributed
/// *only* when this break is actually chosen).
fn span_metrics(atoms: &[Atom], hyphen_width: f64, ends_in_hyphen: bool) -> (f64, f64, f64) {
    let trimmed = trim_trailing_discardables(atoms);
    let mut width = 0.0_f64;
    let mut stretch = 0.0_f64;
    let mut shrink = 0.0_f64;
    for atom in trimmed {
        width += greedy::atom_width(atom);
        if let Atom::Text(t) = atom {
            if t.is_glue {
                stretch += t.width * GLUE_STRETCH_RATIO;
                shrink += t.width * GLUE_SHRINK_RATIO;
            }
        }
    }
    if ends_in_hyphen {
        width += hyphen_width;
    }
    (width, stretch, shrink)
}

/// Legal breakpoints, expressed as "atoms consumed so far" counts (`0` =
/// the paragraph's start, `atoms.len()` = its mandatory end): right after
/// an interword-glue run's *last* glue atom (never mid-run, so the next
/// line never starts with leading whitespace — matches
/// [`crate::layout::greedy::pack_lines`]'s own rule), right after a
/// discretionary-hyphen fragment, right at a forced [`Atom::Break`], and
/// always at the paragraph's end.
fn legal_breaks(atoms: &[Atom]) -> Vec<usize> {
    let mut points = vec![0usize];
    for c in 1..atoms.len() {
        let legal = match &atoms[c - 1] {
            Atom::Text(t) if t.is_glue => !matches!(&atoms[c], Atom::Text(t2) if t2.is_glue),
            Atom::Text(t) if t.hyphen_break => true,
            Atom::Break => true,
            _ => false,
        };
        if legal {
            points.push(c);
        }
    }
    points.push(atoms.len());
    points
}

/// Does a forced [`Atom::Break`] fall *strictly inside* `atoms[start..end]`
/// (a break landing exactly at the slice's own last position is the
/// intended, legal use — ending the line right at that break)?
fn spans_a_break(atoms: &[Atom], start: usize, end: usize) -> bool {
    end > start + 1 && atoms[start..end - 1].iter().any(|a| matches!(a, Atom::Break))
}

fn run_index_of(atom: &Atom) -> usize {
    match atom {
        Atom::Text(t) => t.run_index,
        _ => 0,
    }
}

/// Shapes (and caches, per run) the discretionary-hyphen glyph — avoids
/// re-shaping `"-"` on every candidate transition in the `O(n^2)` DP when a
/// paragraph mixes fonts across runs.
struct HyphenCache<'a> {
    paragraph: &'a Paragraph<'a>,
    shaper: &'a dyn LineShaper,
    cache: HashMap<usize, (AtomGlyph, f64, FontSpec)>,
}

impl<'a> HyphenCache<'a> {
    fn new(paragraph: &'a Paragraph<'a>, shaper: &'a dyn LineShaper) -> Self {
        Self { paragraph, shaper, cache: HashMap::new() }
    }

    /// `(glyph, advance, shape_font)` for the hyphen glyph in `run_index`'s
    /// own font — `shape_font` resolves through the SAME
    /// `VerticalAlign::shape_font` a run's other atoms already use
    /// (typography-gap WAVE 2: a hyphen appended to a `Super`/`Sub` run's
    /// own last line shapes at that run's own shrunk size too, never the
    /// nominal one).
    fn get(&mut self, run_index: usize) -> (AtomGlyph, f64, FontSpec) {
        if let Some(entry) = self.cache.get(&run_index) {
            return entry.clone();
        }
        let (font, vertical_align) = self
            .paragraph
            .runs
            .get(run_index)
            .map(|r| (r.font, r.vertical_align))
            .unwrap_or_default();
        let shape_font = vertical_align.shape_font(font);
        let (glyph, advance) = shape_hyphen(&shape_font, self.shaper);
        let entry = (glyph, advance, shape_font);
        self.cache.insert(run_index, entry.clone());
        entry
    }
}

fn shape_hyphen(font: &FontSpec, shaper: &dyn LineShaper) -> (AtomGlyph, f64) {
    let lines = shaper.shape_wrapped("-", font, f64::MAX);
    if let Some(g) = lines.first().and_then(|l| l.glyphs.first()) {
        let glyph =
            AtomGlyph { cluster: g.cluster.clone(), x: 0.0, y_offset: g.y_offset, advance: g.advance, width: g.width };
        let advance = g.advance;
        (glyph, advance)
    } else {
        // No font shapes ASCII '-' as an empty glyph run in practice;
        // degrade to a zero-width hyphen rather than panic.
        (AtomGlyph { cluster: "-".to_string(), x: 0.0, y_offset: 0.0, advance: 0.0, width: 0.0 }, 0.0)
    }
}

/// Knuth-Plass total-fit breaker: dynamic program over [`legal_breaks`],
/// minimizing the sum of [`demerits`] (+ [`DOUBLE_HYPHEN_DEMERIT`] between
/// two consecutive hyphen-ending lines) across the whole paragraph — the
/// last line is scored via [`last_line_badness`] rather than [`badness`]
/// (`\parfillskip`'s effect: infinite trailing stretch means an under-full
/// final line is never penalized for falling short of `max_width`, but an
/// over-full one still is — see that function's own doc for why the
/// distinction matters).
///
/// When `paragraph.hyphenation` is anything but [`Hyphenation::None`],
/// `atoms` is first expanded via [`hyphenate::expand_hyphenation`] into
/// discretionary hyphen-fragment atoms (under that language's real
/// hyph-utf8 pattern automaton, via `hypher`) before the DP runs.
pub(crate) fn pack_lines(atoms: Vec<Atom>, paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> Vec<Vec<Atom>> {
    if atoms.is_empty() {
        return Vec::new();
    }
    let atoms = if paragraph.hyphenation != Hyphenation::None {
        hyphenate::expand_hyphenation(atoms, paragraph.hyphenation)
    } else {
        atoms
    };

    let max_width = if paragraph.max_width.is_finite() { paragraph.max_width.max(1.0) } else { f64::MAX };
    let candidates = legal_breaks(&atoms);
    let n = candidates.len();
    let mut hyphens = HyphenCache::new(paragraph, shaper);

    let mut best = vec![f64::INFINITY; n];
    let mut via = vec![0usize; n];
    let mut via_hyphen = vec![false; n];
    best[0] = 0.0;

    for j in 1..n {
        let c_j = candidates[j];
        let is_final = c_j == atoms.len();
        for i in 0..j {
            if !best[i].is_finite() {
                continue;
            }
            let c_i = candidates[i];
            if spans_a_break(&atoms, c_i, c_j) {
                continue;
            }

            let slice = &atoms[c_i..c_j];
            let ends_in_hyphen = matches!(slice.last(), Some(Atom::Text(t)) if t.hyphen_break);
            let hyphen_width =
                if ends_in_hyphen { hyphens.get(run_index_of(&slice[slice.len() - 1])).1 } else { 0.0 };
            let (width, stretch, shrink) = span_metrics(slice, hyphen_width, ends_in_hyphen);

            let b = if is_final { last_line_badness(width, max_width, shrink) } else { badness(width, max_width, stretch, shrink) };
            let penalty = if ends_in_hyphen { HYPHEN_PENALTY } else { 0.0 };
            let mut d = demerits(b, penalty);
            if ends_in_hyphen && via_hyphen[i] {
                d += DOUBLE_HYPHEN_DEMERIT;
            }

            let total = best[i] + d;
            if total < best[j] {
                best[j] = total;
                via[j] = i;
                via_hyphen[j] = ends_in_hyphen;
            }
        }
    }

    if !best[n - 1].is_finite() {
        // Never reached in practice (every j always has at least one finite
        // predecessor by construction — see this module's `CLAUDE.md`
        // notes) but a single-line fallback keeps this function total.
        return vec![trim_trailing_discardables(&atoms).to_vec()];
    }

    let mut path = vec![n - 1];
    let mut cur = n - 1;
    while cur != 0 {
        cur = via[cur];
        path.push(cur);
    }
    path.reverse();

    let mut lines = Vec::with_capacity(path.len().saturating_sub(1));
    for w in path.windows(2) {
        let c_i = candidates[w[0]];
        let c_j = candidates[w[1]];
        let slice = &atoms[c_i..c_j];
        let ends_in_hyphen = matches!(slice.last(), Some(Atom::Text(t)) if t.hyphen_break);

        let mut line: Vec<Atom> = trim_trailing_discardables(slice).to_vec();
        if ends_in_hyphen {
            let run_index = run_index_of(&slice[slice.len() - 1]);
            let (glyph, advance, shape_font) = hyphens.get(run_index);
            let (ascent, descent) = match slice.last() {
                Some(Atom::Text(t)) => (t.ascent, t.descent),
                _ => (0.0, 0.0),
            };
            line.push(Atom::Text(TextAtom {
                run_index,
                glyphs: vec![glyph],
                width: advance,
                ascent,
                descent,
                shape_font,
                is_glue: false,
                hyphen_break: false,
            }));
        }
        lines.push(line);
    }
    lines
}

/// Score an already-decided `Vec<Vec<Atom>>` line grouping (from either this
/// module's own [`pack_lines`] or [`crate::layout::greedy::pack_lines`])
/// under the exact same demerits formula [`pack_lines`]'s DP minimizes —
/// lets a test prove KP's search genuinely reaches the minimum reachable
/// under this metric, by scoring greedy's (a different, non-optimal)
/// grouping with the identical formula.
#[cfg(test)]
fn score_lines(lines: &[Vec<Atom>], max_width: f64) -> f64 {
    let max_width = if max_width.is_finite() { max_width.max(1.0) } else { f64::MAX };
    let last_index = lines.len().saturating_sub(1);
    let mut total = 0.0_f64;
    let mut prev_hyphen = false;
    for (i, line) in lines.iter().enumerate() {
        let width: f64 = line.iter().map(greedy::atom_width).sum();
        let mut stretch = 0.0_f64;
        let mut shrink = 0.0_f64;
        for atom in line {
            if let Atom::Text(t) = atom {
                if t.is_glue {
                    stretch += t.width * GLUE_STRETCH_RATIO;
                    shrink += t.width * GLUE_SHRINK_RATIO;
                }
            }
        }
        let b = if i == last_index { last_line_badness(width, max_width, shrink) } else { badness(width, max_width, stretch, shrink) };
        let ends_in_hyphen = matches!(line.last(), Some(Atom::Text(t)) if t.hyphen_break);
        let penalty = if ends_in_hyphen { HYPHEN_PENALTY } else { 0.0 };
        let mut d = demerits(b, penalty);
        if ends_in_hyphen && prev_hyphen {
            d += DOUBLE_HYPHEN_DEMERIT;
        }
        total += d;
        prev_hyphen = ends_in_hyphen;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FontSpec, StyledRun};
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    fn word_atom(hyphen_break: bool) -> Atom {
        Atom::Text(TextAtom {
            run_index: 0,
            glyphs: vec![AtomGlyph { cluster: "x".to_string(), x: 0.0, y_offset: 0.0, advance: 10.0, width: 10.0 }],
            width: 10.0,
            ascent: 12.0,
            descent: 4.0,
            shape_font: FontSpec::default(),
            is_glue: false,
            hyphen_break,
        })
    }

    /// The pure demerits math: two consecutive hyphen-ending lines cost
    /// strictly more than the same lines with only one of them flagged,
    /// all else held equal — [`DOUBLE_HYPHEN_DEMERIT`] is genuinely applied.
    #[test]
    fn double_hyphen_demerit_penalizes_two_consecutive_hyphen_ending_lines() {
        let with_double = vec![vec![word_atom(true)], vec![word_atom(true)], vec![word_atom(false)]];
        let without_double = vec![vec![word_atom(true)], vec![word_atom(false)], vec![word_atom(false)]];

        assert!(score_lines(&with_double, 1000.0) > score_lines(&without_double, 1000.0));
    }

    /// Sanity: Knuth-Plass is optimal for the objective it minimizes — its
    /// own line grouping must score at most as many total demerits as
    /// greedy's (a different, non-optimal) grouping of the identical atom
    /// stream, under the identical scoring formula.
    #[test]
    fn knuth_plass_total_demerits_is_at_most_greedys_for_the_same_atoms() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let text = "The quick brown fox jumps over the lazy dog and then keeps \
            running further down a very long and winding road without ever \
            stopping for a rest, which is exactly the kind of paragraph a \
            first-fit greedy packer tends to leave visibly raggeder than it \
            needs to.";
        let runs = [StyledRun::new(text, font)];
        let max_width = 150.0;
        let paragraph = Paragraph::new(&runs, max_width);
        let shaper = CosmicShaper::headless();

        let atoms = greedy::build_atom_stream(&paragraph, &shaper);
        let greedy_lines = greedy::pack_lines(atoms.clone(), max_width);
        let kp_lines = pack_lines(atoms, &paragraph, &shaper);

        assert!(greedy_lines.len() > 1 && kp_lines.len() > 1, "fixture must wrap to multiple lines");

        let greedy_score = score_lines(&greedy_lines, max_width);
        let kp_score = score_lines(&kp_lines, max_width);
        assert!(
            kp_score <= greedy_score + 1e-6,
            "KP ({kp_score}) must be at most as costly as greedy's grouping ({greedy_score})"
        );
    }
}
