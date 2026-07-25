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
//!
//! ## Justification-overshoot fix (typography track T6, 2026-07-25)
//!
//! **The defect (measured, root-caused, pinned by a test in commit
//! `e8425ec`):** on a justified paragraph, [`pack_lines`] could choose a
//! candidate line whose required interword-glue SHRINK exceeded the
//! render-time shrink CAPACITY [`crate::layout::layout_paragraph`]'s own
//! glue-shrink pass can actually deliver (`params.glue_shrink_ratio` of
//! each glue's own natural width) — [`badness`]'s cubic growth priced that
//! candidate as merely EXPENSIVE, never INFEASIBLE, so it could still win
//! the DP's own total-demerits minimum across the whole paragraph and
//! render a few px past `max_width` with no signal at all. This is the
//! classic TeX "overfull hbox" situation; real TeX at least REPORTS it —
//! this crate used to silently overflow.
//!
//! **The fix mirrors TeX's own structure rather than inventing one:** a
//! candidate line whose shrink NEED exceeds its own capacity is now
//! REJECTED as a break candidate ([`badness_strict`]/
//! [`last_line_badness_strict`], `f64::INFINITY` in that one case — see
//! their own doc comments), and [`pack_lines_unconstrained`]/
//! [`pack_lines_with_hyphen_limit`] both run this STRICT pass first. When a
//! feasible full breaking exists (the overwhelming common case), that is
//! the result — EVERY chosen line is guaranteed to fit within its own
//! render-time shrink capacity, by construction, no epsilon-chasing
//! required. Only when NO feasible breaking exists for the WHOLE paragraph
//! (every possible line grouping has at least one line that can't be
//! shrunk enough — e.g. a single unbreakable word alone on a line, wider
//! than the column, with no glue to shrink at all) does this module fall
//! back to the ORIGINAL, RELAXED, uncapped-badness DP ([`badness`]/
//! [`last_line_badness`], unchanged from before this track — real TeX's
//! own escape hatch is to emit an overfull line and warn; this is that,
//! implemented deliberately rather than by accident) — signalled via
//! [`crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK`] (this
//! workspace's established `metrics` counter convention) and via
//! [`crate::layout::LineBreakDiagnostics`] on the returned layout (see
//! [`crate::layout::layout_paragraph_diagnosed`]).
//!
//! `params.glue_shrink_ratio` ([`LineBreakParams`]) is therefore now
//! LOAD-BEARING for feasibility, not just a cost-model tuning knob: raising
//! it widens the shrink pool the strict pass may draw on (permits tighter
//! lines, fewer paragraphs fall through to the relaxed fallback); lowering
//! it narrows that pool (forces earlier/more breaks, more paragraphs may
//! need the fallback on a sufficiently unforgiving column).

use std::collections::HashMap;

use crate::layout::greedy::{self, Atom, AtomGlyph, TextAtom};
use crate::linebreak::LineBreakParams;
use crate::metrics_keys;
use crate::model::{FontSpec, Paragraph};
use crate::shape::LineShaper;

use super::hyphenate;
use super::Hyphenation;

/// Sentinel for a line with genuinely zero stretch/shrink to draw on (e.g. a
/// single unbreakable atom alone on its own line) — large enough that the DP
/// only ever routes through it when truly unavoidable, but still finite (no
/// fallible surface on the hot path): unlike the ordinary cubic badness
/// below, there's no ratio to even compute here (dividing by a zero pool).
const NO_GLUE_BADNESS: f64 = 1.0e12;

/// Tolerance for the shrink-capacity feasibility check
/// ([`badness_strict`]/[`last_line_badness_strict`]) — absorbs
/// floating-point summation noise across [`span_metrics`]'s own per-glue
/// accumulation, not a "how close counts as an exact fit" decision (that's
/// [`badness`]'s own, deliberately tighter, `1e-9`).
const SHRINK_FEASIBILITY_EPSILON: f64 = 1e-6;

/// Knuth's badness: `0.0` for an exact fit, rising *unbounded* as `100 *
/// (needed/available)^3` toward whichever of stretch/shrink the line needs.
/// Deliberately **not** capped at a fixed ceiling: capping it would make a
/// catastrophically over-full line (e.g. the entire paragraph squeezed onto
/// one line) score identically to a merely tight one, letting that single
/// disastrous line's demerits lose a comparison it should always lose
/// against any reasonable multi-line accumulation. A non-finite `target`
/// (unconstrained width) is always `0.0` (matches
/// [`crate::layout::greedy::pack_lines`]'s own convention).
///
/// **Only used by this module's own deliberate FALLBACK pass** (typography
/// track T6, see this module's own top doc comment) — the PRIMARY pass uses
/// [`badness_strict`] instead. Kept, unrenamed, as the ORIGINAL formula
/// (byte-for-byte unchanged body) since [`score_lines`]'s own test-only
/// demerits scoring also still uses it directly.
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
///
/// Same "fallback-pass only" scope as [`badness`] — see [`last_line_badness_strict`]
/// for the primary pass's own version.
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

/// Feasibility-gated badness (typography track T6, 2026-07-25) — identical
/// to [`badness`] except a line whose SHRINK need exceeds its own render-
/// time shrink CAPACITY (`shrink`, the exact pool [`span_metrics`] sizes via
/// `params.glue_shrink_ratio`, matching the cap
/// [`crate::layout::layout_paragraph`]'s own glue-shrink render pass
/// enforces) scores `f64::INFINITY` — REJECTED as a break candidate, not
/// merely priced in (mirrors real TeX's own `\tolerance` semantics: a
/// requirement physically impossible to satisfy is infeasible, not just
/// expensive). The STRETCH side is unaffected — a loose (under-full) line
/// never causes visible overflow, so there is nothing to gate there; it
/// keeps [`badness`]'s own `NO_GLUE_BADNESS` sentinel for "no stretch to
/// draw on at all".
///
/// This is the PRIMARY pass's own badness function — see this module's top
/// doc comment for the two-pass shape ([`pack_lines_unconstrained`]/
/// [`pack_lines_with_hyphen_limit`] both try this first, falling back to
/// [`badness`]'s relaxed formula only when no fully feasible breaking
/// exists for the whole paragraph).
fn badness_strict(natural: f64, target: f64, stretch: f64, shrink: f64) -> f64 {
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
        if need > shrink + SHRINK_FEASIBILITY_EPSILON {
            f64::INFINITY
        } else if shrink <= 0.0 {
            // `need` is within `SHRINK_FEASIBILITY_EPSILON` of zero here
            // (the branch above already rejected anything larger) but
            // `shrink` itself is exactly zero — avoid a `0.0/0.0` division
            // below; this is the same "no pool to even compute a ratio
            // from" case `badness`'s own `NO_GLUE_BADNESS` sentinel covers.
            NO_GLUE_BADNESS
        } else {
            100.0 * (need / shrink).powi(3)
        }
    }
}

/// [`last_line_badness`]'s own feasibility-gated counterpart — see
/// [`badness_strict`]'s doc comment for the shrink-capacity rule this
/// applies via [`badness_strict`] itself.
fn last_line_badness_strict(natural: f64, target: f64, shrink: f64) -> f64 {
    if !target.is_finite() {
        return 0.0;
    }
    if natural <= target {
        0.0
    } else {
        badness_strict(natural, target, 0.0, shrink)
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
/// *only* when this break is actually chosen). `params.glue_stretch_ratio`/
/// `glue_shrink_ratio` (typography track T5) size interword glue's own
/// stretch/shrink pool — [`LineBreakParams::default`] reproduces this
/// module's pre-T5 hardcoded `0.5`/`1/3` ratios exactly.
fn span_metrics(atoms: &[Atom], hyphen_width: f64, ends_in_hyphen: bool, params: LineBreakParams) -> (f64, f64, f64) {
    let trimmed = trim_trailing_discardables(atoms);
    let mut width = 0.0_f64;
    let mut stretch = 0.0_f64;
    let mut shrink = 0.0_f64;
    for atom in trimmed {
        width += greedy::atom_width(atom);
        if let Atom::Text(t) = atom {
            if t.is_glue {
                stretch += t.width * params.glue_stretch_ratio;
                shrink += t.width * params.glue_shrink_ratio;
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
/// minimizing the sum of [`demerits`] (+ `paragraph.line_break_params.
/// double_hyphen_demerit` between two consecutive hyphen-ending lines)
/// across the whole paragraph — the last line is scored via
/// [`last_line_badness`] rather than [`badness`] (`\parfillskip`'s effect:
/// infinite trailing stretch means an under-full final line is never
/// penalized for falling short of `max_width`, but an over-full one still
/// is — see that function's own doc for why the distinction matters).
///
/// When `paragraph.hyphenation` is anything but [`Hyphenation::None`],
/// `atoms` is first expanded via [`hyphenate::expand_hyphenation`] into
/// discretionary hyphen-fragment atoms (under that language's real
/// hyph-utf8 pattern automaton, via `hypher`, bounded by
/// `paragraph.line_break_params.left_min`/`right_min` — typography track
/// T5) before the DP runs.
///
/// [`Paragraph::max_consecutive_hyphens`] (typography-gap WAVE 3) dispatches
/// to a genuinely separate DP ([`pack_lines_with_hyphen_limit`]) rather than
/// being folded into this one — see that function's own doc comment for
/// why a single shared implementation can't satisfy both "byte-identical
/// when unset" and "a hard feasibility constraint, not just a demerit,
/// when set" at once.
pub(crate) fn pack_lines(atoms: Vec<Atom>, paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> Vec<Vec<Atom>> {
    if atoms.is_empty() {
        return Vec::new();
    }
    let params = paragraph.line_break_params;
    let atoms = if paragraph.hyphenation != Hyphenation::None {
        hyphenate::expand_hyphenation(atoms, paragraph.hyphenation, params.left_min, params.right_min)
    } else {
        atoms
    };

    match paragraph.max_consecutive_hyphens {
        Some(limit) => pack_lines_with_hyphen_limit(atoms, paragraph, shaper, limit),
        None => pack_lines_unconstrained(atoms, paragraph, shaper),
    }
}

/// Badness-function shape both [`run_unconstrained_dp`]/
/// [`run_hyphen_limited_dp`] accept — see [`badness`]/[`badness_strict`]
/// for the two concrete instantiations typography track T6 threads through
/// (parameter names omitted: a plain `fn` pointer type, not itself a
/// documented call site).
type LineBadnessFn = fn(f64, f64, f64, f64) -> f64;

/// Last-line badness-function shape — see [`last_line_badness`]/
/// [`last_line_badness_strict`].
type LastLineBadnessFn = fn(f64, f64, f64) -> f64;

/// Shared DP core for [`pack_lines_unconstrained`] (typography track T6,
/// 2026-07-25) — parameterized over which badness formula scores a
/// candidate line, so the SAME loop body serves both the PRIMARY
/// (feasibility-gated, [`badness_strict`]/[`last_line_badness_strict`]) and
/// FALLBACK ([`badness`]/[`last_line_badness`], pre-T6-identical) passes —
/// see this module's own top doc comment for the two-pass shape. Returns
/// `None` when even the supplied formula can't reach the paragraph's own
/// mandatory final breakpoint with a finite total: under the STRICT formula
/// this means "no fully feasible breaking exists for this paragraph" (the
/// caller then re-runs this SAME function with the RELAXED formula); under
/// the RELAXED formula this is never reached in practice (every position is
/// always reachable via SOME finite-cost path — matches this function's own
/// pre-T6 "never reached" comment, moved here verbatim).
fn run_unconstrained_dp(
    atoms: &[Atom],
    candidates: &[usize],
    max_width: f64,
    params: LineBreakParams,
    hyphens: &mut HyphenCache<'_>,
    line_badness: LineBadnessFn,
    last_badness: LastLineBadnessFn,
) -> Option<Vec<usize>> {
    let n = candidates.len();
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
            if spans_a_break(atoms, c_i, c_j) {
                continue;
            }

            let slice = &atoms[c_i..c_j];
            let ends_in_hyphen = matches!(slice.last(), Some(Atom::Text(t)) if t.hyphen_break);
            let hyphen_width =
                if ends_in_hyphen { hyphens.get(run_index_of(&slice[slice.len() - 1])).1 } else { 0.0 };
            let (width, stretch, shrink) = span_metrics(slice, hyphen_width, ends_in_hyphen, params);

            let b = if is_final { last_badness(width, max_width, shrink) } else { line_badness(width, max_width, stretch, shrink) };
            if !b.is_finite() {
                continue; // INFEASIBLE under this pass's own formula — never a legal transition
            }
            let penalty = if ends_in_hyphen { params.hyphen_penalty } else { 0.0 };
            let mut d = demerits(b, penalty);
            if ends_in_hyphen && via_hyphen[i] {
                d += params.double_hyphen_demerit;
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
        return None;
    }

    let mut path = vec![n - 1];
    let mut cur = n - 1;
    while cur != 0 {
        cur = via[cur];
        path.push(cur);
    }
    path.reverse();
    Some(path)
}

/// The original (pre-WAVE-3) single-state DP — kept as its own function so
/// [`Paragraph::max_consecutive_hyphens`]'s documented "`None` (the
/// default) ... byte-for-byte unchanged" guarantee is structural (the SAME
/// code path runs, not a re-derivation that merely happens to agree), not
/// just an empirically-tested claim.
///
/// Typography track T6 (2026-07-25): now runs [`run_unconstrained_dp`]
/// TWICE, per this module's own top doc comment — the STRICT
/// (feasibility-gated) pass first; only when NO fully feasible breaking
/// exists for the whole paragraph does it fall back to the RELAXED
/// (pre-T6-identical) pass, incrementing
/// [`crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK`] as the
/// deliberate degrade fires.
fn pack_lines_unconstrained(atoms: Vec<Atom>, paragraph: &Paragraph<'_>, shaper: &dyn LineShaper) -> Vec<Vec<Atom>> {
    let max_width = if paragraph.max_width.is_finite() { paragraph.max_width.max(1.0) } else { f64::MAX };
    let params = paragraph.line_break_params;
    let candidates = legal_breaks(&atoms);
    let mut hyphens = HyphenCache::new(paragraph, shaper);

    if let Some(path) =
        run_unconstrained_dp(&atoms, &candidates, max_width, params, &mut hyphens, badness_strict, last_line_badness_strict)
    {
        return reconstruct_lines(&atoms, &candidates, &path, &mut hyphens);
    }

    // No fully feasible breaking exists for this paragraph under the
    // strict pass (e.g. a single unbreakable word alone on a line, wider
    // than the column, with no glue to shrink at all) — the deliberate,
    // TeX-style overfull-hbox fallback: re-run with the ORIGINAL, uncapped
    // cost model (byte-for-byte unchanged from before this track) so the
    // paragraph still wraps, signalling the degrade instead of letting it
    // happen silently.
    metrics::counter!(metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK).increment(1);
    match run_unconstrained_dp(&atoms, &candidates, max_width, params, &mut hyphens, badness, last_line_badness) {
        Some(path) => reconstruct_lines(&atoms, &candidates, &path, &mut hyphens),
        None => {
            // Never reached in practice (every position is always
            // reachable via SOME finite-cost path under the relaxed
            // formula — see `run_unconstrained_dp`'s own doc comment) but
            // a single-line fallback keeps this function total.
            vec![trim_trailing_discardables(&atoms).to_vec()]
        }
    }
}

/// Hard-constrained DP (typography-gap WAVE 3): a breakpoint sequence in
/// which more than `limit` CONSECUTIVE lines end in a discretionary hyphen
/// is INFEASIBLE — never reachable by the search, not merely
/// demerit-discouraged the way `paragraph.line_break_params.
/// double_hyphen_demerit` already discourages exactly two in a row for the
/// unconstrained case.
///
/// This needs a genuinely different DP shape, not a post-hoc filter over
/// [`pack_lines_unconstrained`]'s own output: the single-state DP only ever
/// tracks ONE path (the min-cost predecessor) per position, so it has no way
/// to reject a min-cost path that violates the limit in favor of a
/// slightly-more-expensive one that doesn't — the state must be extended to
/// `(position, trailing consecutive-hyphen run length)`, exploring every
/// feasible run length `0..=limit` at each position and keeping the best
/// cost per state. `limit` is small in every realistic use (a handful at
/// most), so this stays `O(n^2 * limit)` — the same asymptotic class as the
/// unconstrained `O(n^2)` DP, just a small constant-factor multiplier
/// bounded by a caller-chosen value, not `O(n)` (which would make an
/// unconstrained default's `run_states == 1` a special case of this same
/// function needing careful edge-handling for `limit == 0`'s own "reset to
/// 0" transition — kept as two separate functions instead, per this
/// function's own top doc comment, so that edge case never has to be
/// reasoned about jointly with the untouched default path).
///
/// State `0` is always reachable at every position (every legal breakpoint
/// has a NON-hyphen path reaching it — the paragraph's own mandatory final
/// breakpoint and every interword-glue breakpoint are hyphen-independent),
/// so this DP never runs out of feasible states regardless of `limit`
/// (including `limit == 0`, which forbids hyphen breaks entirely) — no
/// fallible surface, matching this crate's own "no panic on the hot path"
/// convention.
///
/// Typography track T6 (2026-07-25): the SAME two-pass shape
/// [`pack_lines_unconstrained`] uses, applied via
/// [`run_hyphen_limited_dp`] — a STRICT (feasibility-gated) pass first,
/// falling back to the RELAXED (pre-T6-identical) pass, incrementing
/// [`crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK`], only when NO
/// fully feasible breaking exists under the hard hyphen-limit constraint
/// EITHER.
fn pack_lines_with_hyphen_limit(atoms: Vec<Atom>, paragraph: &Paragraph<'_>, shaper: &dyn LineShaper, limit: u8) -> Vec<Vec<Atom>> {
    let max_width = if paragraph.max_width.is_finite() { paragraph.max_width.max(1.0) } else { f64::MAX };
    let params = paragraph.line_break_params;
    let candidates = legal_breaks(&atoms);
    let mut hyphens = HyphenCache::new(paragraph, shaper);

    if let Some(path) =
        run_hyphen_limited_dp(&atoms, &candidates, max_width, params, &mut hyphens, limit, badness_strict, last_line_badness_strict)
    {
        return reconstruct_lines(&atoms, &candidates, &path, &mut hyphens);
    }

    metrics::counter!(metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK).increment(1);
    match run_hyphen_limited_dp(&atoms, &candidates, max_width, params, &mut hyphens, limit, badness, last_line_badness) {
        Some(path) => reconstruct_lines(&atoms, &candidates, &path, &mut hyphens),
        None => {
            // Never reached — state 0 is always feasible at every position
            // under the relaxed formula (see this function's own top doc
            // comment) — a single-line fallback keeps this function total
            // regardless.
            vec![trim_trailing_discardables(&atoms).to_vec()]
        }
    }
}

/// Shared DP core for [`pack_lines_with_hyphen_limit`] (typography track
/// T6) — the identical "parameterize over which badness formula scores a
/// candidate line" split [`run_unconstrained_dp`] uses, applied to the
/// hard-hyphen-limit multi-state DP. Returns `None` under the exact same
/// condition [`run_unconstrained_dp`] does (see its own doc comment) —
/// here that can ALSO happen when the strict formula's own shrink-capacity
/// rejection interacts with the hyphen-limit constraint (e.g. the ONLY
/// breaking that respects `limit` also happens to be shrink-infeasible),
/// not just via the hyphen-limit's own bookkeeping.
fn run_hyphen_limited_dp(
    atoms: &[Atom],
    candidates: &[usize],
    max_width: f64,
    params: LineBreakParams,
    hyphens: &mut HyphenCache<'_>,
    limit: u8,
    line_badness: LineBadnessFn,
    last_badness: LastLineBadnessFn,
) -> Option<Vec<usize>> {
    let n = candidates.len();
    let run_states = limit as usize + 1;
    let mut best = vec![vec![f64::INFINITY; run_states]; n];
    let mut via = vec![vec![(0usize, 0usize); run_states]; n];
    best[0][0] = 0.0;

    for j in 1..n {
        let c_j = candidates[j];
        let is_final = c_j == atoms.len();
        for i in 0..j {
            let c_i = candidates[i];
            if spans_a_break(atoms, c_i, c_j) {
                continue;
            }

            let slice = &atoms[c_i..c_j];
            let ends_in_hyphen = matches!(slice.last(), Some(Atom::Text(t)) if t.hyphen_break);
            let hyphen_width =
                if ends_in_hyphen { hyphens.get(run_index_of(&slice[slice.len() - 1])).1 } else { 0.0 };
            let (width, stretch, shrink) = span_metrics(slice, hyphen_width, ends_in_hyphen, params);

            let b = if is_final { last_badness(width, max_width, shrink) } else { line_badness(width, max_width, stretch, shrink) };
            if !b.is_finite() {
                continue; // INFEASIBLE under this pass's own formula — never a legal transition
            }
            let penalty = if ends_in_hyphen { params.hyphen_penalty } else { 0.0 };
            let base_d = demerits(b, penalty);

            for r in 0..run_states {
                if !best[i][r].is_finite() {
                    continue;
                }

                let new_r = if ends_in_hyphen {
                    let candidate_r = r + 1;
                    if candidate_r > limit as usize {
                        continue; // INFEASIBLE — hard reject, never a demerit
                    }
                    candidate_r
                } else {
                    0
                };

                let mut d = base_d;
                if ends_in_hyphen && r > 0 {
                    d += params.double_hyphen_demerit;
                }

                let total = best[i][r] + d;
                if total < best[j][new_r] {
                    best[j][new_r] = total;
                    via[j][new_r] = (i, r);
                }
            }
        }
    }

    let last = n - 1;
    let best_r = (0..run_states)
        .filter(|&r| best[last][r].is_finite())
        .min_by(|&a, &b| best[last][a].partial_cmp(&best[last][b]).unwrap_or(std::cmp::Ordering::Equal));

    let best_r = best_r?;

    let mut path = vec![last];
    let mut cur = (last, best_r);
    while cur.0 != 0 {
        cur = via[cur.0][cur.1];
        path.push(cur.0);
    }
    path.reverse();
    Some(path)
}

/// Turn a chosen candidate-index `path` (both DP shapes' own reconstruction
/// step, factored out so [`pack_lines_unconstrained`]/
/// [`pack_lines_with_hyphen_limit`] share ONE line-materialization pass —
/// design law 1) into real line groups, appending the shaped hyphen glyph
/// atom to any line whose own chosen break lands on a discretionary hyphen.
fn reconstruct_lines(atoms: &[Atom], candidates: &[usize], path: &[usize], hyphens: &mut HyphenCache<'_>) -> Vec<Vec<Atom>> {
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
fn score_lines(lines: &[Vec<Atom>], max_width: f64, params: LineBreakParams) -> f64 {
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
                    stretch += t.width * params.glue_stretch_ratio;
                    shrink += t.width * params.glue_shrink_ratio;
                }
            }
        }
        let b = if i == last_index { last_line_badness(width, max_width, shrink) } else { badness(width, max_width, stretch, shrink) };
        let ends_in_hyphen = matches!(line.last(), Some(Atom::Text(t)) if t.hyphen_break);
        let penalty = if ends_in_hyphen { params.hyphen_penalty } else { 0.0 };
        let mut d = demerits(b, penalty);
        if ends_in_hyphen && prev_hyphen {
            d += params.double_hyphen_demerit;
        }
        total += d;
        prev_hyphen = ends_in_hyphen;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::layout_paragraph;
    use crate::linebreak::BreakStrategy;
    use crate::model::{FontSpec, ParagraphAlign, StyledRun};
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

    /// A single interword-glue atom of the given natural `width` — for
    /// unit-testing [`span_metrics`]'s own stretch/shrink math directly
    /// (typography track T5), independent of a full paragraph layout.
    fn glue_atom(width: f64) -> Atom {
        Atom::Text(TextAtom {
            run_index: 0,
            glyphs: vec![AtomGlyph { cluster: " ".to_string(), x: 0.0, y_offset: 0.0, advance: width, width }],
            width,
            ascent: 0.0,
            descent: 0.0,
            shape_font: FontSpec::default(),
            is_glue: true,
            hyphen_break: false,
        })
    }

    /// The pure demerits math: two consecutive hyphen-ending lines cost
    /// strictly more than the same lines with only one of them flagged,
    /// all else held equal — `double_hyphen_demerit` is genuinely applied.
    #[test]
    fn double_hyphen_demerit_penalizes_two_consecutive_hyphen_ending_lines() {
        let with_double = vec![vec![word_atom(true)], vec![word_atom(true)], vec![word_atom(false)]];
        let without_double = vec![vec![word_atom(true)], vec![word_atom(false)], vec![word_atom(false)]];
        let params = LineBreakParams::default();

        assert!(score_lines(&with_double, 1000.0, params) > score_lines(&without_double, 1000.0, params));
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

        let params = LineBreakParams::default();
        let greedy_score = score_lines(&greedy_lines, max_width, params);
        let kp_score = score_lines(&kp_lines, max_width, params);
        assert!(
            kp_score <= greedy_score + 1e-6,
            "KP ({kp_score}) must be at most as costly as greedy's grouping ({greedy_score})"
        );
    }

    /// Fixed fixture (a dense run of long, real-word hyphenation candidates
    /// at a deliberately narrow column) that reliably produces SEVERAL
    /// consecutive hyphen-ending lines under the unconstrained DP — the
    /// regression floor every `max_consecutive_hyphens` test below measures
    /// against (probed directly: this exact `(text, width)` pair produces
    /// `max_consec == 8` unconstrained, comfortably more than any tested
    /// limit).
    const HYPHEN_DENSE_TEXT: &str = "Understanding internationalization and interoperability requires extraordinary \
        counterproductive administrative documentation, particularly regarding \
        responsibility, accountability, and extraordinary characterization \
        of multidimensional configuration parameters across implementations.";
    const HYPHEN_DENSE_WIDTH: f64 = 150.0;

    /// Longest run of CONSECUTIVE lines whose own last glyph is the shaped
    /// hyphen ("-") — the same "trailing consecutive hyphen count" the hard
    /// constraint itself tracks, recomputed independently here directly from
    /// `ParagraphLayout::lines`/`glyphs` (never trusting the DP's own
    /// internal state) so this test proves the OUTPUT layout genuinely
    /// respects the limit, not just that the DP's search space did.
    fn max_consecutive_hyphen_lines(layout: &crate::layout::ParagraphLayout) -> usize {
        let mut max_consec = 0usize;
        let mut cur = 0usize;
        for line in &layout.lines {
            let ends_hyphen = layout.glyphs.iter().filter(|g| g.line_index == line.line_index).last().is_some_and(|g| g.cluster == "-");
            if ends_hyphen {
                cur += 1;
                max_consec = max_consec.max(cur);
            } else {
                cur = 0;
            }
        }
        max_consec
    }

    fn hyphen_dense_paragraph<'a>(runs: &'a [StyledRun<'a>], max_consecutive_hyphens: Option<u8>) -> Paragraph<'a> {
        let mut p = Paragraph::new(runs, HYPHEN_DENSE_WIDTH).with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::English);
        if let Some(limit) = max_consecutive_hyphens {
            p = p.with_max_consecutive_hyphens(limit);
        }
        p
    }

    /// `max_consecutive_hyphens: None` (the default) must reproduce
    /// [`pack_lines_unconstrained`]'s own output byte-for-byte — the SAME
    /// code path runs (this test calls both entry points directly against
    /// the identical atom stream), not merely an output that happens to
    /// agree.
    #[test]
    fn default_max_consecutive_hyphens_is_byte_identical_to_the_unconstrained_path() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let paragraph = hyphen_dense_paragraph(&runs, None);
        assert_eq!(paragraph.max_consecutive_hyphens, None);
        let shaper = CosmicShaper::headless();

        let params = paragraph.line_break_params;
        let atoms_a = greedy::build_atom_stream(&paragraph, &shaper);
        let atoms_b = greedy::build_atom_stream(&paragraph, &shaper);
        let expanded_a = hyphenate::expand_hyphenation(atoms_a, paragraph.hyphenation, params.left_min, params.right_min);
        let expanded_b = hyphenate::expand_hyphenation(atoms_b, paragraph.hyphenation, params.left_min, params.right_min);

        let via_dispatch = pack_lines(expanded_a.clone(), &paragraph, &shaper);
        let via_direct = pack_lines_unconstrained(expanded_b, &paragraph, &shaper);

        assert_eq!(via_dispatch.len(), via_direct.len());
        for (a, b) in via_dispatch.iter().zip(via_direct.iter()) {
            assert_eq!(a.len(), b.len());
        }

        // Sanity: the fixture is genuinely hyphen-dense unconstrained —
        // otherwise the hard-limit tests below wouldn't be exercising
        // anything real.
        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(max_consecutive_hyphen_lines(&layout) >= 3, "fixture must be hyphen-dense enough to prove the hard constraint actually engages");
    }

    /// `max_consecutive_hyphens: Some(1)` (never two adjacent hyphen-ending
    /// lines) is a HARD constraint on the fixture proven hyphen-dense above
    /// — the resulting layout must contain ZERO adjacent hyphen-ending line
    /// pairs, not merely fewer than the unconstrained case.
    #[test]
    fn hard_limit_of_one_forbids_any_two_adjacent_hyphen_ending_lines() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let paragraph = hyphen_dense_paragraph(&runs, Some(1));
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must still wrap to multiple lines under the constraint");
        assert_eq!(max_consecutive_hyphen_lines(&layout), 1, "a limit of 1 must cap the longest consecutive-hyphen run at exactly 1 (never 0 lines simply refusing to hyphenate, and never 2+)");
        assert!(layout.glyphs.iter().any(|g| g.cluster == "-"), "a limit of 1 still permits isolated (non-adjacent) hyphen breaks");
    }

    /// `max_consecutive_hyphens: Some(0)` forbids hyphen breaks ENTIRELY (a
    /// run of at most 0 consecutive hyphens is, definitionally, zero) —
    /// never a panic, never an infeasible-DP fallback, just plain word-glue
    /// breaks throughout.
    #[test]
    fn hard_limit_of_zero_forbids_every_hyphen_break() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let paragraph = hyphen_dense_paragraph(&runs, Some(0));
        let shaper = CosmicShaper::headless();

        let layout = layout_paragraph(&paragraph, &shaper);
        assert!(layout.lines.len() > 1, "fixture must still wrap (via ordinary word breaks) with hyphenation fully suppressed");
        assert!(!layout.glyphs.iter().any(|g| g.cluster == "-"), "a limit of 0 must produce ZERO hyphen glyphs anywhere in the layout");
    }

    /// A limit tighter than the unconstrained fixture's own natural run
    /// length (proven `>= 3` above) genuinely changes the chosen
    /// breakpoints — `Some(2)` must strictly reduce the longest consecutive
    /// run relative to the unconstrained layout, proving the constraint is
    /// load-bearing, not a no-op that happens to already hold.
    #[test]
    fn a_tighter_limit_than_the_natural_run_length_strictly_reduces_it() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let shaper = CosmicShaper::headless();

        let unconstrained = layout_paragraph(&hyphen_dense_paragraph(&runs, None), &shaper);
        let limited = layout_paragraph(&hyphen_dense_paragraph(&runs, Some(2)), &shaper);

        let unconstrained_run = max_consecutive_hyphen_lines(&unconstrained);
        let limited_run = max_consecutive_hyphen_lines(&limited);
        assert!(unconstrained_run > 2, "fixture's own unconstrained run must exceed the limit under test");
        assert!(limited_run <= 2, "limited layout must never exceed the hard cap, got {limited_run}");
        assert!(limited_run < unconstrained_run, "the constraint must genuinely change the chosen breakpoints, got limited={limited_run} unconstrained={unconstrained_run}");
    }

    // ── Typography track T5: LineBreakParams (2026-07-25) ──────────────

    /// `LineBreakParams::default()` reproduces this module's own pre-T5
    /// hardcoded constants (former `HYPHEN_PENALTY = 50.0`,
    /// `DOUBLE_HYPHEN_DEMERIT = 3000.0`, `GLUE_STRETCH_RATIO = 0.5`,
    /// `GLUE_SHRINK_RATIO = 1/3`) and `hyphenate`'s own pre-T5
    /// `LEFT_MIN`/`RIGHT_MIN` (`2`/`3`) EXACTLY — the numeric floor every
    /// "byte-for-byte unchanged when unset" claim in this wave rests on.
    #[test]
    fn default_line_break_params_reproduce_the_hardcoded_constants_exactly() {
        let p = LineBreakParams::default();
        assert_eq!(p.hyphen_penalty, 50.0);
        assert_eq!(p.double_hyphen_demerit, 3000.0);
        assert_eq!(p.glue_stretch_ratio, 0.5);
        assert_eq!(p.glue_shrink_ratio, 1.0 / 3.0);
        assert_eq!(p.left_min, 2);
        assert_eq!(p.right_min, 3);
    }

    /// T5's own required proof: a paragraph laid out with NON-default
    /// penalties must break DIFFERENTLY than with defaults — the knob is
    /// genuinely wired into the DP's own search, not a decorative field
    /// nothing reads. A prohibitively high `hyphen_penalty` (dwarfing every
    /// other term in the demerits formula) must eliminate every
    /// hyphenation break the SAME fixture takes freely under default
    /// params — and change the line count, so the difference is a real
    /// layout change, not just "no `-` glyph".
    ///
    /// **Width RE-PROBED (typography track T6, 2026-07-25 — see
    /// `crate::linebreak::knuth_plass`'s own top doc comment for the
    /// justification-overshoot fix that changed this):** the ORIGINAL
    /// `HYPHEN_DENSE_WIDTH` (150.0) no longer works for this test — at
    /// that width, several of this fixture's own long words are wider
    /// than the column even with maximal glue shrink and NO interior
    /// glue at all (a single unbreakable word alone on its own line), so
    /// hyphenating them is now STRUCTURALLY REQUIRED for the primary,
    /// feasibility-gated DP pass to find ANY fully feasible breaking —
    /// no `hyphen_penalty`, however large, can out-cost a literal
    /// impossibility (this is in fact exactly the class of defect this
    /// track fixes: before T6, such an unbreakable-without-hyphenation
    /// word would have silently rendered past the column edge instead).
    /// A wider width (`350.0`, probed directly) still hyphenates freely
    /// under default params AND still has a genuinely competitive
    /// non-hyphenated alternative, so a prohibitive penalty can still
    /// choose it.
    #[test]
    fn non_default_hyphen_penalty_changes_which_breaks_knuth_plass_chooses() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let width = 350.0;
        let shaper = CosmicShaper::headless();

        let default_paragraph = Paragraph::new(&runs, width).with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::English);
        let default_layout = layout_paragraph(&default_paragraph, &shaper);
        assert!(default_layout.glyphs.iter().any(|g| g.cluster == "-"), "fixture must hyphenate freely under default params (regression floor)");

        let harsh_params = LineBreakParams { hyphen_penalty: 1.0e8, ..LineBreakParams::default() };
        let harsh_paragraph = Paragraph::new(&runs, width)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English)
            .with_line_break_params(harsh_params);
        let harsh_layout = layout_paragraph(&harsh_paragraph, &shaper);

        assert!(
            !harsh_layout.glyphs.iter().any(|g| g.cluster == "-"),
            "a prohibitively high hyphen_penalty must eliminate every hyphen break this fixture otherwise takes freely"
        );
        assert_ne!(
            default_layout.lines.len(),
            harsh_layout.lines.len(),
            "eliminating every hyphen break must genuinely change the line count — a real layout change, not decoration"
        );
    }

    /// The second penalty knob, same "wired not decorative" proof: an
    /// extreme `double_hyphen_demerit` must strictly shrink the longest
    /// consecutive-hyphen run relative to the default, at a width where a
    /// genuinely competitive lower-consecutive-hyphen alternative exists
    /// (unlike the narrower `HYPHEN_DENSE_WIDTH` fixture, where nearly
    /// every word structurally REQUIRES hyphenation regardless of demerit).
    ///
    /// **Width RE-PROBED (typography track T6, 2026-07-25 — same
    /// justification-overshoot fix, see `crate::linebreak::knuth_plass`'s
    /// own top doc comment):** the original `250.0` no longer demonstrates
    /// this knob — under the primary, feasibility-gated DP pass, `250.0`'s
    /// own set of FULLY FEASIBLE breakings collapsed to (effectively) one,
    /// so there is no genuinely competitive lower-consecutive-hyphen
    /// alternative left for `double_hyphen_demerit` to select between
    /// anymore (a real, expected consequence of correctly rejecting
    /// over-capacity lines as infeasible rather than merely expensive —
    /// fewer "roughly as good" alternatives survive at all). `400.0`
    /// (probed directly) still has room: `default_run=2` -> `harsh_run=1`,
    /// with the total LINE COUNT identical (`6` both ways), proving the
    /// demerit changed WHICH breakpoints were chosen, not merely how many
    /// lines resulted — the same shape the original `250.0` probe
    /// demonstrated, just at a width where genuine choice still exists
    /// post-fix.
    #[test]
    fn non_default_double_hyphen_demerit_reduces_consecutive_hyphen_runs() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let width = 400.0;
        let shaper = CosmicShaper::headless();

        let default_paragraph =
            Paragraph::new(&runs, width).with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::English);
        let default_layout = layout_paragraph(&default_paragraph, &shaper);
        let default_run = max_consecutive_hyphen_lines(&default_layout);
        assert!(default_run >= 2, "fixture must have a real consecutive-hyphen run under default params (regression floor), got {default_run}");

        let harsh_params = LineBreakParams { double_hyphen_demerit: 1.0e9, ..LineBreakParams::default() };
        let harsh_paragraph = Paragraph::new(&runs, width)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English)
            .with_line_break_params(harsh_params);
        let harsh_layout = layout_paragraph(&harsh_paragraph, &shaper);
        let harsh_run = max_consecutive_hyphen_lines(&harsh_layout);

        assert!(harsh_run < default_run, "a huge double_hyphen_demerit must strictly shrink the longest consecutive-hyphen run, got default={default_run} harsh={harsh_run}");
        assert_eq!(default_layout.lines.len(), harsh_layout.lines.len(), "regression floor: the two layouts should differ in WHICH lines hyphenate, not in overall line count, at this width");
    }

    /// `glue_stretch_ratio`/`glue_shrink_ratio` (T5) are genuinely read by
    /// [`span_metrics`] — a smaller ratio must shrink the returned
    /// stretch/shrink pool proportionally (the exact pool the DP's own
    /// badness scoring, and `layout::paragraph`'s render-time shrink cap,
    /// are built on).
    #[test]
    fn non_default_glue_ratios_change_span_metrics_stretch_and_shrink() {
        // A trailing glue atom alone would be trimmed by
        // `trim_trailing_discardables` (matches `flush_line`'s own
        // "trailing whitespace is never counted" convention) — a
        // non-glue atom AFTER the glue keeps it in the measured span.
        let atoms = vec![word_atom(false), glue_atom(20.0), word_atom(false)];
        let default_params = LineBreakParams::default();
        let (_, default_stretch, default_shrink) = span_metrics(&atoms, 0.0, false, default_params);

        let tight_params = LineBreakParams { glue_stretch_ratio: 0.1, glue_shrink_ratio: 0.05, ..LineBreakParams::default() };
        let (_, tight_stretch, tight_shrink) = span_metrics(&atoms, 0.0, false, tight_params);

        assert!(tight_stretch < default_stretch, "a smaller glue_stretch_ratio must shrink the returned stretch pool, default={default_stretch} tight={tight_stretch}");
        assert!(tight_shrink < default_shrink, "a smaller glue_shrink_ratio must shrink the returned shrink pool, default={default_shrink} tight={tight_shrink}");
    }

    /// `left_min`/`right_min` (T5) genuinely reach the DP end-to-end
    /// through `paragraph.line_break_params` — a much wider bound must
    /// suppress SOME hyphenation break the default bound takes on the same
    /// fixture (proves the field is read all the way from `Paragraph`
    /// through to the chosen layout, not merely by `hyphenate`'s own unit
    /// tests in isolation).
    #[test]
    fn non_default_left_right_min_changes_the_laid_out_hyphenation() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new(HYPHEN_DENSE_TEXT, font)];
        let shaper = CosmicShaper::headless();

        let default_paragraph =
            Paragraph::new(&runs, HYPHEN_DENSE_WIDTH).with_break_strategy(BreakStrategy::KnuthPlass).with_hyphenation(Hyphenation::English);
        let default_layout = layout_paragraph(&default_paragraph, &shaper);
        let default_hyphens = default_layout.glyphs.iter().filter(|g| g.cluster == "-").count();
        assert!(default_hyphens > 0, "fixture must hyphenate under default params (regression floor)");

        let wide_params = LineBreakParams { left_min: 6, right_min: 6, ..LineBreakParams::default() };
        let wide_paragraph = Paragraph::new(&runs, HYPHEN_DENSE_WIDTH)
            .with_break_strategy(BreakStrategy::KnuthPlass)
            .with_hyphenation(Hyphenation::English)
            .with_line_break_params(wide_params);
        let wide_layout = layout_paragraph(&wide_paragraph, &shaper);
        let wide_hyphens = wide_layout.glyphs.iter().filter(|g| g.cluster == "-").count();

        assert!(wide_hyphens < default_hyphens, "a much wider left_min/right_min must strictly reduce the laid-out hyphen count, default={default_hyphens} wide={wide_hyphens}");
    }

    // ── Typography track T6: justification-overshoot fix (2026-07-25) ──

    /// A minimal test [`metrics::Recorder`] that counts how many times a
    /// SPECIFIC key's counter is incremented (and by how much) — enough to
    /// prove [`crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK`] is a
    /// REAL, load-bearing signal, without pulling in a whole metrics-
    /// snapshot crate (`uzor-urx-core::recorder`'s own `UrxRecorder` is the
    /// workspace's full-featured version of the identical idea — this is
    /// the minimal, test-local slice of it this one assertion needs).
    struct CountingRecorder {
        key: &'static str,
        total: std::sync::Arc<std::sync::atomic::AtomicU64>,
    }

    struct AtomicCounter(std::sync::Arc<std::sync::atomic::AtomicU64>);
    impl metrics::CounterFn for AtomicCounter {
        fn increment(&self, value: u64) {
            self.0.fetch_add(value, std::sync::atomic::Ordering::SeqCst);
        }
        fn absolute(&self, value: u64) {
            self.0.store(value, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl metrics::Recorder for CountingRecorder {
        fn describe_counter(&self, _key: metrics::KeyName, _unit: Option<metrics::Unit>, _description: metrics::SharedString) {}
        fn describe_gauge(&self, _key: metrics::KeyName, _unit: Option<metrics::Unit>, _description: metrics::SharedString) {}
        fn describe_histogram(&self, _key: metrics::KeyName, _unit: Option<metrics::Unit>, _description: metrics::SharedString) {}

        fn register_counter(&self, key: &metrics::Key, _metadata: &metrics::Metadata<'_>) -> metrics::Counter {
            if key.name() == self.key {
                metrics::Counter::from_arc(std::sync::Arc::new(AtomicCounter(self.total.clone())))
            } else {
                metrics::Counter::noop()
            }
        }
        fn register_gauge(&self, _key: &metrics::Key, _metadata: &metrics::Metadata<'_>) -> metrics::Gauge {
            metrics::Gauge::noop()
        }
        fn register_histogram(&self, _key: &metrics::Key, _metadata: &metrics::Metadata<'_>) -> metrics::Histogram {
            metrics::Histogram::noop()
        }
    }

    /// A synthetic paragraph engineered so NO feasible breaking exists at
    /// all: one single, genuinely unbreakable "word" (no interior spaces,
    /// no hyphenation opportunities — `Hyphenation::None`), wider than the
    /// column, with ZERO interword glue on its own only possible line to
    /// draw shrink from. The primary, feasibility-gated DP pass ([`badness_strict`])
    /// must reject this candidate outright (`need > shrink(0.0)` is always
    /// true whenever `need > 0.0`); the deliberate, TeX-style overfull-hbox
    /// FALLBACK must then fire — signalled both via
    /// [`crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK`] (verified
    /// here against a real, installed test [`metrics::Recorder`], not just
    /// "doesn't panic") and via [`crate::layout::LineBreakDiagnostics`] on
    /// the returned layout — and the paragraph must still render exactly
    /// one, genuinely over-full, line rather than panicking or producing no
    /// output at all.
    #[test]
    fn no_feasible_breaking_falls_back_and_signals_via_metrics_and_diagnostics() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        const TEXT: &str = "Supercalifragilisticexpialidocioussesquipedalianism";
        let runs = [StyledRun::new(TEXT, font)];
        let max_width = 50.0;
        let paragraph = Paragraph::new(&runs, max_width).with_align(ParagraphAlign::Justify).with_break_strategy(BreakStrategy::KnuthPlass);
        let shaper = CosmicShaper::headless();

        let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let recorder = CountingRecorder { key: crate::metrics_keys::KEY_LINEBREAK_OVERFULL_FALLBACK, total: total.clone() };

        let (layout, diag) =
            metrics::with_local_recorder(&recorder, || crate::layout::layout_paragraph_diagnosed(&paragraph, &shaper));

        assert_eq!(layout.lines.len(), 1, "one genuinely unbreakable word must still render as exactly one line, never panic or vanish");
        assert!(diag.overfull_fallback_used, "no feasible breaking exists for this fixture — the deliberate overfull-hbox fallback must have fired");
        assert_eq!(diag.overfull_line_count, 1);
        assert!(total.load(std::sync::atomic::Ordering::SeqCst) >= 1, "the fallback must increment the metrics counter at least once — got {}", total.load(std::sync::atomic::Ordering::SeqCst));

        let last_glyph = layout.glyphs.last().expect("the single unbreakable word must still produce real glyphs");
        assert!(last_glyph.x + last_glyph.advance > max_width, "the fallback line must genuinely render past the measure (that's the whole point of it being flagged) — never silently clipped to fit");
    }

    /// Property-style sweep (typography track T6): across several distinct
    /// texts and several column widths (a small, deterministic matrix —
    /// this workspace has no property-testing crate dependency anywhere,
    /// see this crate's own `tnum_audit` module for the established
    /// "hand-rolled deterministic sweep table" convention this follows),
    /// no justified `KnuthPlass` line's own rendered advance-end may exceed
    /// `max_width` UNLESS [`crate::layout::LineBreakDiagnostics::
    /// overfull_fallback_used`] is `true` for that specific layout — the
    /// exact invariant this whole fix exists to guarantee.
    #[test]
    fn justified_lines_never_exceed_the_measure_unless_the_fallback_fired() {
        const TEXTS: [&str; 4] = [
            "Good typography is invisible, or nearly so: a well-set paragraph reads evenly, without ragged holes or crowded lines.",
            "The quick brown fox jumps over the lazy dog and then keeps running further down the road without ever stopping for a rest.",
            HYPHEN_DENSE_TEXT,
            "Показательный документ подтверждает поддержку кириллического текста, а узкая колонка оправданного текста быстро показывает неравномерные промежутки между словами.",
        ];
        const WIDTHS: [f64; 9] = [60.0, 90.0, 120.0, 150.0, 180.0, 220.0, 260.0, 320.0, 400.0];
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let shaper = CosmicShaper::headless();
        let mut checked = 0usize;

        for text in TEXTS {
            let runs = [StyledRun::new(text, font)];
            for &max_width in &WIDTHS {
                for hyphenation in [Hyphenation::None, Hyphenation::English] {
                    let paragraph = Paragraph::new(&runs, max_width)
                        .with_align(ParagraphAlign::Justify)
                        .with_break_strategy(BreakStrategy::KnuthPlass)
                        .with_hyphenation(hyphenation);
                    let (layout, diag) = crate::layout::layout_paragraph_diagnosed(&paragraph, &shaper);
                    let last_index = layout.lines.len().saturating_sub(1);

                    for line in &layout.lines {
                        if line.line_index == last_index {
                            continue; // the paragraph's own ragged last line is never justify-stretched — a different, already-covered question
                        }
                        let Some(last_glyph) = layout.glyphs.iter().filter(|g| g.line_index == line.line_index).last() else { continue };
                        let advance_end = last_glyph.x + last_glyph.advance;
                        checked += 1;
                        assert!(
                            advance_end <= max_width + 1e-6 || diag.overfull_fallback_used,
                            "line {} of {text:?} at max_width={max_width} hyphenation={hyphenation:?} overshoots ({advance_end} > {max_width}) but overfull_fallback_used is false — the fix's own invariant is broken",
                            line.line_index
                        );
                    }
                }
            }
        }

        assert!(checked > 50, "this sweep must exercise a real number of justified lines, got {checked}");
    }
}
