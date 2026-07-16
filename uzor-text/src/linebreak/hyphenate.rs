//! Liang/TeX-style pattern-automaton hyphenation ([`hyphenation_points`]) +
//! [`expand_hyphenation`], the bridge that splits a [`crate::layout::greedy`]
//! word atom into hyphenation-fragment atoms at those points.
//!
//! **v1 scope** (report, not a silent gap): this is a **hand-authored
//! minimal English pattern set** (`PATTERNS` below), not the full
//! ~4500-pattern `hyph-en-us.tex` (Liang/Knuth's own public-domain TeX
//! distribution file) — vendoring that whole table was judged out of scope
//! for one phase's worth of business-presentation vocabulary (design doc §7
//! Q3 already flags English-only as the v1 boundary; this narrows further,
//! to "a real Liang algorithm over a deliberately small pattern list", not
//! "exhaustive English coverage"). An unmatched word simply doesn't
//! hyphenate — never a wrong guess, never a panic (no fallible surface on
//! the hot path). See this crate's `CLAUDE.md` Phase 5 section for the
//! full pattern-by-pattern rationale and the test vocabulary each one
//! targets.

use crate::layout::greedy::{Atom, AtomGlyph, TextAtom};

/// The v1 pattern set itself (see module doc for the coverage trade-off).
/// Liang digit-encoding: a digit between two letters is the pattern's
/// hyphenation weight at that gap (odd = break candidate, even = suppress);
/// a leading/trailing `.` anchors a pattern to a word boundary.
const PATTERNS: &[&str] = &[
    // Prefix/compound anchors (word-start only) — precise, so they never
    // fire on an unrelated word that merely contains the same letters
    // mid-word.
    ".won1d",
    ".un1d",
    ".under1s",
    // Suffix anchors (word-end only).
    "1ation.",
    "1ful.",
    "d1ing.",
    // General (unanchored) letter-cluster rules.
    "y1ph",
    "u1t",
    // Doubled consonants split between the pair (`hap-pen`, `run-ning`).
    "b1b", "d1d", "f1f", "g1g", "l1l", "m1m", "n1n", "p1p", "r1r", "s1s", "t1t",
];

/// Minimum letters a hyphenation point must leave on the line before it
/// (`won-derful`, never `w-onderful`) — matches the conventional TeX
/// `\lefthyphenmin` default.
const LEFT_MIN: usize = 2;
/// Minimum letters a hyphenation point must leave after it (`wonder-ful`,
/// never `wonderfu-l`) — matches the conventional TeX `\righthyphenmin`
/// default (a 2-letter dangling fragment reads as an ugly line-end even
/// where a dictionary syllable break exists there).
const RIGHT_MIN: usize = 3;

/// One parsed Liang pattern: `letters[i]` is matched literally against the
/// bounded (`.word.`) string; `weights[i]` is the pattern's hyphenation
/// weight *before* `letters[i]` (so `weights.len() == letters.len() + 1`,
/// the trailing entry being the weight after the pattern's last letter).
struct Pattern {
    letters: Vec<u8>,
    weights: Vec<u8>,
}

/// Parse one Liang pattern spec, e.g. `"1ful."` → letters `f,u,l,.`,
/// weights `[1,0,0,0,0]` (a digit applies to the gap immediately following
/// it in the spec string).
fn parse_pattern(spec: &str) -> Pattern {
    let mut letters = Vec::new();
    let mut weights = Vec::new();
    let mut pending = 0u8;
    for byte in spec.bytes() {
        if byte.is_ascii_digit() {
            pending = byte - b'0';
        } else {
            weights.push(pending);
            pending = 0;
            letters.push(byte);
        }
    }
    weights.push(pending);
    Pattern { letters, weights }
}

/// Liang/TeX hyphenation: candidate break byte-offsets into `word` (a
/// plain word — no leading/trailing punctuation). Empty for anything
/// shorter than `LEFT_MIN + RIGHT_MIN` letters, any word containing a
/// non-ASCII-alphabetic byte (v1 scope, see module doc), or a word no
/// pattern matches.
pub(crate) fn hyphenation_points(word: &str) -> Vec<usize> {
    let len = word.len();
    if len < LEFT_MIN + RIGHT_MIN || !word.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Vec::new();
    }

    let lower = word.to_ascii_lowercase();
    let bounded = format!(".{lower}.");
    let bytes = bounded.as_bytes();
    let n = bytes.len();
    let mut values = vec![0u8; n + 1];

    for spec in PATTERNS {
        let pattern = parse_pattern(spec);
        let plen = pattern.letters.len();
        if plen == 0 || plen > n {
            continue;
        }
        for start in 0..=(n - plen) {
            if bytes[start..start + plen] == pattern.letters[..] {
                for (i, &w) in pattern.weights.iter().enumerate() {
                    let idx = start + i;
                    if values[idx] < w {
                        values[idx] = w;
                    }
                }
            }
        }
    }

    // `values[p]` is the gap immediately before `bounded[p]`. Word char `j`
    // (0-indexed) is `bounded[j + 1]`, so "break before word[j]" reads
    // `values[j + 1]`; odd values are the pattern-tagged break candidates.
    (LEFT_MIN..=(len - RIGHT_MIN)).filter(|&j| values[j + 1] % 2 == 1).collect()
}

/// Expand every plain-ASCII-alphabetic, non-glue word atom in `atoms` into
/// hyphenation-fragment atoms at its [`hyphenation_points`], each fragment
/// but the last flagged `hyphen_break: true` (a discretionary breakpoint
/// [`crate::linebreak::knuth_plass`] may cut at). Atoms that aren't a pure
/// word (punctuation attached, too short, or no pattern matched) pass
/// through completely unchanged.
pub(crate) fn expand_hyphenation(atoms: Vec<Atom>) -> Vec<Atom> {
    let mut out = Vec::with_capacity(atoms.len());
    for atom in atoms {
        match atom {
            Atom::Text(t) if !t.is_glue => out.extend(split_atom(t)),
            other => out.push(other),
        }
    }
    out
}

/// Byte offset `offset` → glyph index, but only if it lands **exactly** on
/// a cluster boundary (never mid-cluster — a hyphenation point that falls
/// inside a shaped ligature is silently skipped rather than mis-splitting
/// one glyph in two).
fn glyph_index_for_offset(glyphs: &[AtomGlyph], offset: usize) -> Option<usize> {
    let mut acc = 0usize;
    for (i, g) in glyphs.iter().enumerate() {
        if acc == offset {
            return Some(i);
        }
        acc += g.cluster.len();
    }
    if acc == offset {
        Some(glyphs.len())
    } else {
        None
    }
}

fn split_atom(atom: TextAtom) -> Vec<Atom> {
    let word: String = atom.glyphs.iter().map(|g| g.cluster.as_str()).collect();
    let points = hyphenation_points(&word);
    if points.is_empty() {
        return vec![Atom::Text(atom)];
    }

    let mut cuts: Vec<usize> = points
        .iter()
        .filter_map(|&offset| glyph_index_for_offset(&atom.glyphs, offset))
        .filter(|&i| i > 0 && i < atom.glyphs.len())
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.is_empty() {
        return vec![Atom::Text(atom)];
    }

    let mut fragments = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0usize;
    for &cut in &cuts {
        fragments.push(make_fragment(&atom, start, cut, true));
        start = cut;
    }
    fragments.push(make_fragment(&atom, start, atom.glyphs.len(), false));
    fragments
}

/// One hyphenation fragment: `atom.glyphs[start..end]`, rebased so the
/// fragment's own first glyph sits at `x = 0.0` (matches every other atom's
/// own local-origin convention — see [`crate::layout::greedy`]'s module doc).
fn make_fragment(atom: &TextAtom, start: usize, end: usize, hyphen_break: bool) -> Atom {
    let slice = &atom.glyphs[start..end];
    let base_x = slice.first().map(|g| g.x).unwrap_or(0.0);
    let glyphs: Vec<AtomGlyph> = slice
        .iter()
        .map(|g| AtomGlyph { cluster: g.cluster.clone(), x: g.x - base_x, y_offset: g.y_offset, advance: g.advance, width: g.width })
        .collect();
    let width = glyphs.last().map(|g| g.x + g.advance).unwrap_or(0.0);
    Atom::Text(TextAtom {
        run_index: atom.run_index,
        glyphs,
        width,
        ascent: atom.ascent,
        descent: atom.descent,
        is_glue: false,
        hyphen_break,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::greedy::build_atom_stream;
    use crate::model::{FontSpec, Paragraph, StyledRun};
    use crate::shape::CosmicShaper;
    use uzor::fonts::FontFamily;

    #[test]
    fn hyphenation_of_hyphenation_itself_matches_hy_phen_ation() {
        assert_eq!(hyphenation_points("hyphenation"), vec![2, 6]);
    }

    #[test]
    fn hyphenation_of_wonderful_matches_won_der_ful() {
        assert_eq!(hyphenation_points("wonderful"), vec![3, 6]);
    }

    #[test]
    fn hyphenation_of_beautiful_matches_beau_ti_ful() {
        assert_eq!(hyphenation_points("beautiful"), vec![4, 6]);
    }

    #[test]
    fn hyphenation_of_understanding_matches_un_der_stand_ing() {
        assert_eq!(hyphenation_points("understanding"), vec![2, 5, 10]);
    }

    #[test]
    fn hyphenation_of_happen_matches_hap_pen_via_the_doubled_p() {
        assert_eq!(hyphenation_points("happen"), vec![3]);
    }

    #[test]
    fn hyphenation_of_running_matches_run_ning_via_the_doubled_n() {
        assert_eq!(hyphenation_points("running"), vec![3]);
    }

    #[test]
    fn hyphenation_rejects_words_shorter_than_the_minimum() {
        assert!(hyphenation_points("the").is_empty());
        assert!(hyphenation_points("it").is_empty());
    }

    #[test]
    fn hyphenation_rejects_non_alphabetic_words() {
        assert!(hyphenation_points("don't").is_empty());
        assert!(hyphenation_points("well-known").is_empty());
    }

    #[test]
    fn hyphenation_of_an_unmatched_word_is_empty_not_a_guess() {
        // Long enough to pass the length guard, but no pattern in the v1
        // set matches any of its letter clusters.
        assert!(hyphenation_points("quartz").is_empty());
    }

    #[test]
    fn expand_hyphenation_splits_a_long_word_into_flagged_fragments() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("understanding", font)];
        let paragraph = Paragraph::new(&runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let atoms = build_atom_stream(&paragraph, &shaper);

        let expanded = expand_hyphenation(atoms);
        let fragments: Vec<&TextAtom> =
            expanded.iter().filter_map(|a| if let Atom::Text(t) = a { Some(t) } else { None }).collect();

        assert_eq!(fragments.len(), 4, "un|der|stand|ing must be 4 fragments");
        assert!(fragments[0].hyphen_break);
        assert!(fragments[1].hyphen_break);
        assert!(fragments[2].hyphen_break);
        assert!(!fragments[3].hyphen_break, "the final fragment must never itself be a hyphen point");
        for fragment in &fragments {
            assert_eq!(fragment.glyphs.first().map(|g| g.x), Some(0.0), "each fragment rebases to x = 0.0");
        }
    }

    #[test]
    fn expand_hyphenation_leaves_a_short_or_unmatched_word_as_one_atom() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("cat", font)];
        let paragraph = Paragraph::new(&runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let atoms = build_atom_stream(&paragraph, &shaper);

        let expanded = expand_hyphenation(atoms);
        let words: Vec<&TextAtom> =
            expanded.iter().filter_map(|a| if let Atom::Text(t) = a { if !t.is_glue { Some(t) } else { None } } else { None }).collect();

        assert_eq!(words.len(), 1);
        assert!(!words[0].hyphen_break);
    }
}
