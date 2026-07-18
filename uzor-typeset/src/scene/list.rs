//! [`ListBlock`] — marker + indent list (design doc §3.6).
//!
//! "No new layout primitive: each item becomes one flow child with a
//! leading reserved marker gutter ahead of its content" (§3.6). P1 scope
//! narrowing (report, not silent): a `ListBlock` is composed as ONE
//! ATOMIC unit this phase (never split mid-list across a region
//! boundary) rather than literally flow-splitting at item granularity —
//! the design doc's own P1 gates/tests (§7) never test a list spanning
//! multiple regions, and no P1 consumer needs one; per-item splitting is
//! deferred to whenever a real long-list-across-pages consumer needs it,
//! the same "genuinely untestable before its own consumer exists"
//! reasoning P0 already used to defer `CardRegionSequence` (see this
//! crate's `CLAUDE.md`).
//!
//! Marker text paints using `ComposeStyle::default_font` — a real P1
//! consumer for that field, which P0 declared reserved-but-unread.
//!
//! **Nested lists (typography-gap WAVE 3, report — not a new mechanism):**
//! [`crate::scene::ListItem::content`] is already `&'a [BlockNode<'a>]`,
//! and [`crate::scene::Block::List`] is already one of the block kinds that
//! content array may hold — so a list item whose own content contains
//! ANOTHER `Block::List` composes/splits/paints/exports through the
//! existing recursion with ZERO new code: `compose::list_layout::
//! measure_list_items` already calls `compose()` (the SAME entry point
//! every top-level flow uses) over each item's own content at that item's
//! own narrowed `content_width`, so a nested list's own `indent_px`
//! narrows a SECOND time on top of the outer list's — indentation
//! compounds automatically, never hardcoded. `crate::region::
//! PlacedBlock::translate` (`region/frame.rs`) already recurses into a
//! block's own `list_placement.items[].content` when shifting an outer
//! item's content into its final frame position, and that recursion is
//! ALREADY self-referential (a `PlacedBlock` with its own nested
//! `list_placement` translates its own nested items too) — so a
//! doubly-nested list's own rects end up correctly positioned with no
//! change needed there either. See `compose::list_layout`'s own test
//! module for the direct proof (indentation compounds, roman/alpha
//! markers render at the correct nesting depth).

use crate::scene::block::BlockNode;

/// Which numeral system [`MarkerStyle::Numbered`] renders (typography-gap
/// WAVE 3) — pure formatter, no locale/theme lookup (a caller wanting a
/// different convention picks a different scheme explicitly, matching this
/// crate's own "typed contract, no config bag" law).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberScheme {
    /// `"1."`, `"2."`, ... — the original (pre-WAVE-3) behavior.
    #[default]
    Decimal,
    /// `"a."`, `"b."`, ..., `"z."`, `"aa."`, `"ab."`, ... — 1-based,
    /// spreadsheet-column-style (never a 0th letter, never skips a letter).
    LowerAlpha,
    /// Same sequence as [`NumberScheme::LowerAlpha`], upper-cased.
    UpperAlpha,
    /// `"iv."`, `"ix."`, `"xl."`, ... — classic subtractive Roman numerals
    /// (see [`NumberScheme::format`]'s own doc comment for the exact
    /// subtractive-pair table).
    LowerRoman,
    /// Same numerals as [`NumberScheme::LowerRoman`], upper-cased.
    UpperRoman,
}

/// 1-based spreadsheet-column-style letters: `1 -> "a"`, `26 -> "z"`,
/// `27 -> "aa"`, `52 -> "az"`, `53 -> "ba"` — never a 0th letter (this is
/// NOT positional base-26: there is no `"a"` digit meaning zero, matching
/// how spreadsheet columns/outline levels are conventionally lettered, not
/// naive base-26 arithmetic, which would produce `"a"` `"b"` ... `"z"`
/// `"ba"` (skipping the `"aa"` a naive base-26 reading would never reach)).
/// `n == 0` degrades to an empty string — never a fallible surface.
fn to_alpha_lower(mut n: u32) -> String {
    if n == 0 {
        return String::new();
    }
    let mut letters = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push((b'a' + rem as u8) as char);
        n = (n - 1) / 26;
    }
    letters.iter().rev().collect()
}

/// Subtractive-pair Roman numeral conversion: `4 -> "IV"`, `9 -> "IX"`,
/// `40 -> "XL"`, `90 -> "XC"`, `400 -> "CD"`, `900 -> "CM"` — the six
/// classic subtractive pairs, plus every additive symbol
/// (`M`/`D`/`C`/`L`/`X`/`V`/`I`). `n == 0` degrades to an empty string
/// (Roman numerals have no native zero) — never a fallible surface.
fn to_roman_upper(mut n: u32) -> String {
    const VALUES: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for &(value, symbol) in &VALUES {
        while n >= value {
            out.push_str(symbol);
            n -= value;
        }
    }
    out
}

impl NumberScheme {
    /// Format `n` (1-based item ordinal, already `start`-offset by the
    /// caller — see [`ListBlock::marker_text`]) under this scheme — the
    /// bare label, WITHOUT the trailing `"."` [`ListBlock::marker_text`]
    /// appends uniformly across every scheme.
    pub fn format(self, n: u32) -> String {
        match self {
            NumberScheme::Decimal => n.to_string(),
            NumberScheme::LowerAlpha => to_alpha_lower(n),
            NumberScheme::UpperAlpha => to_alpha_lower(n).to_uppercase(),
            NumberScheme::LowerRoman => to_roman_upper(n).to_lowercase(),
            NumberScheme::UpperRoman => to_roman_upper(n),
        }
    }
}

/// How each item's leading marker renders.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerStyle {
    /// The same character before every item (e.g. `'•'`).
    Bullet(char),
    /// `"{scheme(start + index)}."`-formatted, `start`-based, incrementing
    /// by item index (typography-gap WAVE 3 adds `scheme` — see
    /// [`NumberScheme`]).
    Numbered { start: u32, scheme: NumberScheme },
    /// No marker glyph painted — the indent gutter is still reserved
    /// (a plain indented list), so item content lines up identically to
    /// a bulleted/numbered sibling.
    None,
}

impl MarkerStyle {
    /// Convenience constructor: [`NumberScheme::Decimal`] (the original,
    /// pre-WAVE-3 default) starting at `start`.
    pub fn numbered(start: u32) -> Self {
        MarkerStyle::Numbered { start, scheme: NumberScheme::Decimal }
    }
}

/// One list item — content is `&'a [BlockNode<'a>]` (paragraphs this
/// phase, same "cell content" scoping `TableCell` uses).
pub struct ListItem<'a> {
    pub content: &'a [BlockNode<'a>],
}

impl<'a> ListItem<'a> {
    pub fn new(content: &'a [BlockNode<'a>]) -> Self {
        Self { content }
    }
}

/// A flow-participating list: `indent_px`-wide marker gutter ahead of
/// every item's own content.
pub struct ListBlock<'a> {
    pub items: &'a [ListItem<'a>],
    pub marker: MarkerStyle,
    pub indent_px: f64,
}

impl<'a> ListBlock<'a> {
    pub fn new(items: &'a [ListItem<'a>], marker: MarkerStyle, indent_px: f64) -> Self {
        Self { items, marker, indent_px }
    }

    /// The marker text for item `index` (0-based) under this list's
    /// [`MarkerStyle`] — `""` for [`MarkerStyle::None`]. Every
    /// [`MarkerStyle::Numbered`] scheme shares the SAME trailing `"."`
    /// convention (only the label before it varies by scheme).
    pub fn marker_text(&self, index: usize) -> String {
        match &self.marker {
            MarkerStyle::Bullet(ch) => ch.to_string(),
            MarkerStyle::Numbered { start, scheme } => format!("{}.", scheme.format(*start + index as u32)),
            MarkerStyle::None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_marker_text_is_the_same_character_for_every_item() {
        let list = ListBlock::new(&[], MarkerStyle::Bullet('•'), 20.0);
        assert_eq!(list.marker_text(0), "•");
        assert_eq!(list.marker_text(3), "•");
    }

    #[test]
    fn numbered_marker_text_increments_from_start() {
        let list = ListBlock::new(&[], MarkerStyle::numbered(5), 24.0);
        assert_eq!(list.marker_text(0), "5.");
        assert_eq!(list.marker_text(2), "7.");
    }

    #[test]
    fn none_marker_text_is_empty() {
        let list = ListBlock::new(&[], MarkerStyle::None, 24.0);
        assert_eq!(list.marker_text(0), "");
    }

    #[test]
    fn numbered_marker_text_defaults_to_decimal_scheme() {
        let list = ListBlock::new(&[], MarkerStyle::numbered(1), 24.0);
        assert_eq!(list.marker_text(0), "1.");
    }

    /// Lower/upper alpha schemes: 1-based spreadsheet-column-style letters,
    /// including the `z -> aa` rollover (never a naive base-26 skip).
    #[test]
    fn alpha_scheme_marker_text_rolls_over_from_z_to_aa() {
        let lower = ListBlock::new(&[], MarkerStyle::Numbered { start: 1, scheme: NumberScheme::LowerAlpha }, 24.0);
        assert_eq!(lower.marker_text(0), "a.");
        assert_eq!(lower.marker_text(25), "z.");
        assert_eq!(lower.marker_text(26), "aa.");
        assert_eq!(lower.marker_text(51), "az.");
        assert_eq!(lower.marker_text(52), "ba.");

        let upper = ListBlock::new(&[], MarkerStyle::Numbered { start: 1, scheme: NumberScheme::UpperAlpha }, 24.0);
        assert_eq!(upper.marker_text(0), "A.");
        assert_eq!(upper.marker_text(26), "AA.");
    }

    /// Roman numeral edge cases (task gate): the six classic subtractive
    /// pairs (4/9/40/90/400/900), plus a compound number exercising several
    /// of them at once.
    #[test]
    fn roman_scheme_formats_every_subtractive_pair_edge_case() {
        assert_eq!(NumberScheme::UpperRoman.format(4), "IV");
        assert_eq!(NumberScheme::UpperRoman.format(9), "IX");
        assert_eq!(NumberScheme::UpperRoman.format(40), "XL");
        assert_eq!(NumberScheme::UpperRoman.format(90), "XC");
        assert_eq!(NumberScheme::UpperRoman.format(400), "CD");
        assert_eq!(NumberScheme::UpperRoman.format(900), "CM");
        assert_eq!(NumberScheme::UpperRoman.format(1994), "MCMXCIV", "classic compound-number regression fixture");
        assert_eq!(NumberScheme::UpperRoman.format(3), "III");
        assert_eq!(NumberScheme::UpperRoman.format(58), "LVIII");

        assert_eq!(NumberScheme::LowerRoman.format(4), "iv");
        assert_eq!(NumberScheme::LowerRoman.format(1994), "mcmxciv");
    }

    #[test]
    fn roman_and_alpha_schemes_render_through_marker_text_with_the_trailing_dot() {
        let list = ListBlock::new(&[], MarkerStyle::Numbered { start: 1, scheme: NumberScheme::UpperRoman }, 24.0);
        assert_eq!(list.marker_text(3), "IV.");
    }

    #[test]
    fn number_scheme_default_is_decimal() {
        assert_eq!(NumberScheme::default(), NumberScheme::Decimal);
    }
}
