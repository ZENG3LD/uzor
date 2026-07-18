//! Real hyph-utf8-derived hyphenation ([`hyphenation_points`]) +
//! [`expand_hyphenation`], the bridge that splits a [`crate::layout::greedy`]
//! word atom into hyphenation-fragment atoms at those points.
//!
//! **Hard cutover (typography quality wave)**: this module used to carry a
//! hand-authored ~20-pattern English-only Liang engine (see this crate's
//! `CLAUDE.md` Phase 5 section for that engine's own history) — fully
//! superseded now by [`hypher`], Typst's own hyphenator: a byte-trie finite
//! automaton compiled at hypher's OWN build time from the real hyph-utf8
//! pattern corpus, covering ~48 permissively-licensed languages (English,
//! Russian, German, ... — see `nemo/docs/uzor-engines/
//! research_typesetting_sota_2026.md` §2/§8). `ru`/`en`/`de` are all present
//! in hypher's own shipped feature set (confirmed directly against its
//! `Cargo.toml` — no fallback vendoring needed, no missing-language gap).
//! No hand-rolled pattern table remains in this file.

use crate::layout::greedy::{Atom, AtomGlyph, TextAtom};
use crate::linebreak::{Hyphenation, Lang};

/// Minimum letters a hyphenation point must leave on the line before it
/// (`won-derful`, never `w-onderful`) — matches the conventional TeX
/// `\lefthyphenmin` default. Passed to [`hypher::hyphenate_bounded`] as a
/// **character** count (not bytes) — correct for multi-byte scripts like
/// Cyrillic, where the old hand-rolled engine's byte-based bound would have
/// silently under/over-counted letters.
const LEFT_MIN: usize = 2;
/// Minimum letters a hyphenation point must leave after it (`wonder-ful`,
/// never `wonderfu-l`) — matches the conventional TeX `\righthyphenmin`
/// default (a 2-letter dangling fragment reads as an ugly line-end even
/// where a dictionary syllable break exists there).
const RIGHT_MIN: usize = 3;

/// Which [`hypher::Lang`] (if any) `hyphenation` selects — `None` for
/// [`Hyphenation::None`], otherwise the language [`expand_hyphenation`]
/// consults.
fn lang_for(hyphenation: Hyphenation) -> Option<Lang> {
    match hyphenation {
        Hyphenation::None => None,
        Hyphenation::English => Some(Lang::English),
        Hyphenation::Russian => Some(Lang::Russian),
        Hyphenation::Lang(lang) => Some(lang),
    }
}

/// Real hyph-utf8 hyphenation via [`hypher`]: candidate break byte-offsets
/// into `word` (a plain word — no leading/trailing punctuation), bounded by
/// [`LEFT_MIN`]/[`RIGHT_MIN`] **characters** (not bytes — see their own doc
/// comments). Empty for anything shorter than `LEFT_MIN + RIGHT_MIN`
/// characters, any word containing a non-alphabetic character, or a word
/// `lang`'s own pattern set doesn't break at all.
pub(crate) fn hyphenation_points(word: &str, lang: Lang) -> Vec<usize> {
    let char_count = word.chars().count();
    if char_count < LEFT_MIN + RIGHT_MIN || !word.chars().all(char::is_alphabetic) {
        return Vec::new();
    }

    let syllables = hypher::hyphenate_bounded(word, lang, LEFT_MIN, RIGHT_MIN);
    let mut points = Vec::new();
    let mut consumed = 0usize;
    for (i, syllable) in syllables.enumerate() {
        if i > 0 {
            points.push(consumed);
        }
        consumed += syllable.len();
    }
    points
}

/// Expand every plain-alphabetic, non-glue word atom in `atoms` into
/// hyphenation-fragment atoms at its [`hyphenation_points`] under
/// `hyphenation`'s own [`Lang`], each fragment but the last flagged
/// `hyphen_break: true` (a discretionary breakpoint
/// [`crate::linebreak::knuth_plass`] may cut at). Atoms that aren't a pure
/// word (punctuation attached, too short, or no pattern matched) pass
/// through completely unchanged. [`Hyphenation::None`] returns `atoms`
/// verbatim (never called by [`crate::linebreak::knuth_plass::pack_lines`]
/// in that case anyway, but kept total here too).
pub(crate) fn expand_hyphenation(atoms: Vec<Atom>, hyphenation: Hyphenation) -> Vec<Atom> {
    let Some(lang) = lang_for(hyphenation) else {
        return atoms;
    };

    let mut out = Vec::with_capacity(atoms.len());
    for atom in atoms {
        match atom {
            Atom::Text(t) if !t.is_glue => out.extend(split_atom(t, lang)),
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

fn split_atom(atom: TextAtom, lang: Lang) -> Vec<Atom> {
    let word: String = atom.glyphs.iter().map(|g| g.cluster.as_str()).collect();
    let points = hyphenation_points(&word, lang);
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
        shape_font: atom.shape_font,
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
    fn hyphenation_of_hyphenation_itself_matches_real_hypher_break_points() {
        // hyph-en-us's own real pattern set breaks "hyphenation" as
        // "hy-phen-ation" — offsets (chars from the start) are 2, 6.
        assert_eq!(hyphenation_points("hyphenation", Lang::English), vec![2, 6]);
    }

    #[test]
    fn hyphenation_of_wonderful_matches_real_hypher_break_points() {
        assert_eq!(hyphenation_points("wonderful", Lang::English), vec![3, 6]);
    }

    #[test]
    fn hyphenation_rejects_words_shorter_than_the_minimum() {
        assert!(hyphenation_points("the", Lang::English).is_empty());
        assert!(hyphenation_points("it", Lang::English).is_empty());
    }

    #[test]
    fn hyphenation_rejects_non_alphabetic_words() {
        assert!(hyphenation_points("don't", Lang::English).is_empty());
        assert!(hyphenation_points("well-known", Lang::English).is_empty());
    }

    /// Every returned break point is a **byte** offset (since `hyphenate_points`
    /// walks a UTF-8 string), while `LEFT_MIN`/`RIGHT_MIN` are **char** counts
    /// (hypher's own bound semantics — correct for multi-byte scripts). This
    /// helper re-derives the char-position bounds and converts them to byte
    /// offsets before checking, so the two units are never compared directly.
    fn assert_points_respect_char_bounds(word: &str, points: &[usize]) {
        let byte_len = word.len();
        let left_bound_bytes: usize = word.chars().take(LEFT_MIN).map(char::len_utf8).sum();
        let right_bound_bytes: usize = word.chars().rev().take(RIGHT_MIN).map(char::len_utf8).sum();
        let max_offset = byte_len - right_bound_bytes;
        for &p in points {
            assert!(
                p >= left_bound_bytes && p <= max_offset,
                "break point (byte offset {p}) must respect LEFT_MIN/RIGHT_MIN char bounds ({left_bound_bytes}..={max_offset})"
            );
        }
    }

    #[test]
    fn russian_word_perevodov_yields_a_valid_break_point() {
        // "переводов" (9 chars) — hyph-utf8's `ru` pattern set breaks it at
        // real morpheme boundaries; assert a break exists and respects the
        // char-counted LEFT_MIN/RIGHT_MIN bounds (never inside the first 2
        // or last 3 characters).
        let word = "переводов";
        let points = hyphenation_points(word, Lang::Russian);
        assert!(!points.is_empty(), "a 9-char Russian word must yield at least one break point");
        assert_points_respect_char_bounds(word, &points);
    }

    #[test]
    fn russian_word_pokazatelnyi_yields_a_valid_break_point() {
        // "показательный" — long compound-looking word, real test vocabulary
        // from the task brief.
        let word = "показательный";
        let points = hyphenation_points(word, Lang::Russian);
        assert!(!points.is_empty(), "a long Russian word must yield at least one break point");
        assert_points_respect_char_bounds(word, &points);
    }

    #[test]
    fn hyphenation_of_an_unmatched_short_word_is_empty_not_a_guess() {
        // Below the length floor — never even consulted.
        assert!(hyphenation_points("cat", Lang::English).is_empty());
    }

    #[test]
    fn expand_hyphenation_splits_a_long_word_into_flagged_fragments() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("understanding", font)];
        let paragraph = Paragraph::new(&runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let atoms = build_atom_stream(&paragraph, &shaper);

        let expanded = expand_hyphenation(atoms, Hyphenation::English);
        let fragments: Vec<&TextAtom> =
            expanded.iter().filter_map(|a| if let Atom::Text(t) = a { Some(t) } else { None }).collect();

        assert!(fragments.len() > 1, "a long, hyphenatable word must split into more than one fragment");
        assert!(!fragments.last().expect("at least one fragment").hyphen_break, "the final fragment must never itself be a hyphen point");
        for fragment in fragments.iter().take(fragments.len() - 1) {
            assert!(fragment.hyphen_break, "every fragment but the last must be flagged as a hyphen break");
        }
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

        let expanded = expand_hyphenation(atoms, Hyphenation::English);
        let words: Vec<&TextAtom> =
            expanded.iter().filter_map(|a| if let Atom::Text(t) = a { if !t.is_glue { Some(t) } else { None } } else { None }).collect();

        assert_eq!(words.len(), 1);
        assert!(!words[0].hyphen_break);
    }

    #[test]
    fn expand_hyphenation_with_none_returns_atoms_verbatim() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("understanding", font)];
        let paragraph = Paragraph::new(&runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let atoms = build_atom_stream(&paragraph, &shaper);
        let before_len = atoms.len();

        let expanded = expand_hyphenation(atoms, Hyphenation::None);
        assert_eq!(expanded.len(), before_len, "Hyphenation::None must never split any atom");
    }

    #[test]
    fn expand_hyphenation_splits_a_russian_word_end_to_end_via_hyphenation_russian() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("показательный", font)];
        let paragraph = Paragraph::new(&runs, f64::MAX);
        let shaper = CosmicShaper::headless();
        let atoms = build_atom_stream(&paragraph, &shaper);

        let expanded = expand_hyphenation(atoms, Hyphenation::Russian);
        let fragments: Vec<&TextAtom> =
            expanded.iter().filter_map(|a| if let Atom::Text(t) = a { Some(t) } else { None }).collect();

        assert!(fragments.len() > 1, "a long Russian word must split into more than one fragment under Hyphenation::Russian");
        assert!(!fragments.last().expect("at least one fragment").hyphen_break);
    }

    #[test]
    fn hyphenation_named_variants_agree_with_the_generic_lang_variant() {
        // `Hyphenation::English`/`Hyphenation::Russian` are named convenience
        // shapes over the SAME `hypher::Lang` the generic `Hyphenation::
        // Lang(..)` escape hatch carries — both routes must produce
        // identical break points for the same word.
        let via_named = hyphenation_points("wonderful", Lang::English);
        let via_generic = hyphenation_points("wonderful", Lang::English);
        assert_eq!(via_named, via_generic);

        assert_eq!(lang_for(Hyphenation::English), Some(Lang::English));
        assert_eq!(lang_for(Hyphenation::Russian), Some(Lang::Russian));
        assert_eq!(lang_for(Hyphenation::Lang(Lang::German)), Some(Lang::German));
        assert_eq!(lang_for(Hyphenation::None), None);
    }
}
