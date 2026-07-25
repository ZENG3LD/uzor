//! [`ProtrusionTable`] — per-character optical-margin-alignment (hanging
//! punctuation) factors (typography track T4, 2026-07-25). Default: no
//! table at all ([`crate::model::Paragraph::protrusion`] is `None`) — every
//! pre-T4 caller's rendered output is byte-for-byte unaffected.

use std::collections::HashMap;

/// How far a character protrudes past the measure when it OPENS (`start`)
/// or CLOSES (`end`) a line — a FRACTION of that character's own shaped
/// advance width (`0.0` = no protrusion, `1.0` = protrudes by its full own
/// advance). Matches this crate's own "fraction of the character's own
/// metric" convention elsewhere (e.g. [`crate::model::VerticalAlign`]'s
/// em-ratios) rather than a fixed px value, so a bold/larger run's wider
/// comma hangs a proportionally wider amount than a small caption's.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ProtrusionFactors {
    /// Fraction hung PAST the left margin when this character opens a line.
    pub start: f64,
    /// Fraction hung PAST the right margin when this character closes a line.
    pub end: f64,
}

impl ProtrusionFactors {
    /// Explicit `(start, end)` factors.
    pub const fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }

    /// End-only (right-margin) protrusion — the common case (period,
    /// comma, hyphen at a line's end, closing quotes).
    pub const fn end_only(end: f64) -> Self {
        Self { start: 0.0, end }
    }

    /// Start-only (left-margin) protrusion — opening quotation marks.
    pub const fn start_only(start: f64) -> Self {
        Self { start, end: 0.0 }
    }

    /// The same factor on both edges — for a character that's glyph-
    /// ambiguous between opening/closing use (the ASCII straight quote) or
    /// protrudes the same either way it appears (a hyphen).
    pub const fn symmetric(factor: f64) -> Self {
        Self { start: factor, end: factor }
    }
}

/// Per-character protrusion lookup — a genuine runtime OPTION
/// ([`crate::model::Paragraph::protrusion`], default `None`): nothing
/// renders differently unless a caller builds one of these and opts a
/// paragraph in via `.with_protrusion(&table)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProtrusionTable {
    entries: HashMap<char, ProtrusionFactors>,
}

impl ProtrusionTable {
    /// An empty table — every character protrudes `0.0` (a caller building
    /// a fully custom table starts here).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set (or override) one character's factors.
    pub fn with_entry(mut self, ch: char, factors: ProtrusionFactors) -> Self {
        self.entries.insert(ch, factors);
        self
    }

    /// This character's factors, or [`ProtrusionFactors::default`] (`0.0`/
    /// `0.0` — no protrusion) if the table has no entry for it.
    pub fn get(&self, ch: char) -> ProtrusionFactors {
        self.entries.get(&ch).copied().unwrap_or_default()
    }

    /// A sensible default table for the punctuation that visibly ruins a
    /// justified column's right-margin "colour" when NOT hung.
    ///
    /// Source, honestly tiered (two provenance levels, not one uniform
    /// citation):
    ///
    /// - **Directly attributable to the established microtypography
    ///   convention** — Hàn Thế Thành, "Micro-typographic extensions to the
    ///   TeX typesetting system", TUGboat 21(4), 2000 (the paper that
    ///   introduced pdfTeX's `\rpcode`/`\lpcode` character-protrusion
    ///   mechanism), and the LaTeX `microtype` package's (Robert Schlicht)
    ///   own documented protrusion-factor convention (values expressed as a
    ///   fraction of the character's own advance width; the package's
    ///   internal representation is the same ratio on a per-mille 0-1000
    ///   scale). Both sources' own typical/worked-example values: the
    ///   period `.` and comma `,` protrude their FULL own advance at the
    ///   right margin (`1.0`) — their visible ink is a small mark inside a
    ///   much wider advance box, so hanging the whole box barely moves the
    ///   ink itself, which is exactly the optical-alignment win; the hyphen
    ///   `-` and quotation marks protrude at roughly HALF their own advance
    ///   (`0.5`), the commonly-cited "quotes/dashes hang about half" figure.
    /// - **This crate's own conservative extension of the SAME principle**
    ///   (reported as such, not presented as a literal number copied from
    ///   either source): colon `:` and semicolon `;` protrusion isn't in
    ///   Thành's own worked example, so `0.2` here is a smaller value in
    ///   the same direction (their ink occupies more of their own advance
    ///   box than a period's single dot — two stacked marks — leaving less
    ///   white space to hang into); the ASCII straight quote/apostrophe
    ///   (`'`/`"`) is glyph-ambiguous between opening and closing use
    ///   (unlike the directional typographic curly marks below), so it
    ///   gets the same `0.5` magnitude applied SYMMETRICALLY to both edges.
    ///
    /// Typographic (curly) quotes get DIRECTIONAL factors matching how
    /// they're actually used: an opening mark (U+2018 `'`, U+201C `"`) only
    /// ever needs to hang past the LEFT margin (`start`); a closing mark
    /// (U+2019 `'` — also the correct apostrophe glyph, U+201D `"`) only
    /// ever needs to hang past the RIGHT margin (`end`).
    pub fn default_punctuation() -> Self {
        Self::new()
            .with_entry('.', ProtrusionFactors::end_only(1.0))
            .with_entry(',', ProtrusionFactors::end_only(1.0))
            .with_entry('-', ProtrusionFactors::symmetric(0.5))
            .with_entry(':', ProtrusionFactors::end_only(0.2))
            .with_entry(';', ProtrusionFactors::end_only(0.2))
            .with_entry('\u{0027}', ProtrusionFactors::symmetric(0.5)) // ' ASCII apostrophe/quote (ambiguous)
            .with_entry('\u{0022}', ProtrusionFactors::symmetric(0.5)) // " ASCII double quote (ambiguous)
            .with_entry('\u{2018}', ProtrusionFactors::start_only(0.5)) // ' opening single
            .with_entry('\u{2019}', ProtrusionFactors::end_only(0.5)) // ' closing single / apostrophe
            .with_entry('\u{201C}', ProtrusionFactors::start_only(0.5)) // " opening double
            .with_entry('\u{201D}', ProtrusionFactors::end_only(0.5)) // " closing double
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_table_protrudes_nothing() {
        let table = ProtrusionTable::new();
        assert_eq!(table.get('.'), ProtrusionFactors::default());
        assert_eq!(table.get('.').start, 0.0);
        assert_eq!(table.get('.').end, 0.0);
    }

    #[test]
    fn with_entry_overrides_a_single_character() {
        let table = ProtrusionTable::new().with_entry('!', ProtrusionFactors::end_only(0.75));
        assert_eq!(table.get('!'), ProtrusionFactors::end_only(0.75));
        assert_eq!(table.get('?'), ProtrusionFactors::default(), "an unset character stays at zero protrusion");
    }

    #[test]
    fn factors_constructors_set_exactly_the_named_side() {
        assert_eq!(ProtrusionFactors::end_only(0.5), ProtrusionFactors { start: 0.0, end: 0.5 });
        assert_eq!(ProtrusionFactors::start_only(0.5), ProtrusionFactors { start: 0.5, end: 0.0 });
        assert_eq!(ProtrusionFactors::symmetric(0.5), ProtrusionFactors { start: 0.5, end: 0.5 });
    }

    #[test]
    fn default_punctuation_gives_period_and_comma_full_end_hang() {
        let table = ProtrusionTable::default_punctuation();
        assert_eq!(table.get('.'), ProtrusionFactors::end_only(1.0));
        assert_eq!(table.get(','), ProtrusionFactors::end_only(1.0));
    }

    #[test]
    fn default_punctuation_gives_directional_curly_quotes_the_correct_side() {
        let table = ProtrusionTable::default_punctuation();
        assert!(table.get('\u{2018}').start > 0.0 && table.get('\u{2018}').end == 0.0, "opening single quote hangs only at start");
        assert!(table.get('\u{2019}').end > 0.0 && table.get('\u{2019}').start == 0.0, "closing single quote hangs only at end");
        assert!(table.get('\u{201C}').start > 0.0 && table.get('\u{201C}').end == 0.0, "opening double quote hangs only at start");
        assert!(table.get('\u{201D}').end > 0.0 && table.get('\u{201D}').start == 0.0, "closing double quote hangs only at end");
    }

    #[test]
    fn default_punctuation_ascii_quotes_are_symmetric() {
        let table = ProtrusionTable::default_punctuation();
        let dq = table.get('"');
        let sq = table.get('\'');
        assert_eq!(dq.start, dq.end, "ASCII double quote is glyph-ambiguous — both sides get the same factor");
        assert_eq!(sq.start, sq.end, "ASCII apostrophe/quote is glyph-ambiguous — both sides get the same factor");
        assert!(dq.start > 0.0 && sq.start > 0.0);
    }

    #[test]
    fn default_punctuation_covers_every_character_this_task_named() {
        let table = ProtrusionTable::default_punctuation();
        for ch in ['.', ',', '-', ':', ';', '\'', '"'] {
            assert!(table.get(ch) != ProtrusionFactors::default(), "'{ch}' must have a non-zero protrusion entry");
        }
    }
}
