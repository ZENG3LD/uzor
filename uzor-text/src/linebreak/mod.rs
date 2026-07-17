//! Phase 5 line-breaking: [`BreakStrategy`] switches
//! [`crate::layout::layout_paragraph`] between the greedy word-wrap packer
//! (`crate::layout::greedy`, Phase 1/2) and [`knuth_plass`]'s total-fit
//! demerits-minimizing dynamic-programming breaker; [`Hyphenation`] opts a
//! paragraph into [`hyphenate`]'s real hyph-utf8-derived discretionary
//! hyphen points (via the [`hypher`] crate — Typst's own hyphenator).
//!
//! [`Hyphenation`] is consulted **only** by [`BreakStrategy::KnuthPlass`] —
//! Phase 1/2's greedy packer (`crate::layout::greedy::pack_lines`) is
//! unchanged and never reads a word atom's hyphenation opportunities, so
//! `Greedy` (the still-default strategy) stays byte-identical to every
//! prior phase's output regardless of what `Hyphenation` a caller sets (see
//! this crate's `CLAUDE.md` Phase 5 section, and its typography-wave
//! section, for the full rationale).

pub mod hyphenate;
pub(crate) mod knuth_plass;

pub use hypher::Lang;

/// Which line-breaking algorithm [`crate::layout::layout_paragraph`] uses to
/// decide where a [`crate::model::Paragraph`]'s lines end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakStrategy {
    /// First-fit word-wrap (`crate::layout::greedy`) — cheap, and this
    /// crate's original (still default) Phase 1/2 behavior.
    #[default]
    Greedy,
    /// Total-fit, demerits-minimizing dynamic-programming breaker
    /// ([`knuth_plass`]) — costlier (`O(n^2)` over feasible breakpoints per
    /// paragraph), opted into for higher visual quality (evener line
    /// spacing, optional hyphenation).
    KnuthPlass,
}

/// Hyphenation strategy for a [`crate::model::Paragraph`].
///
/// Only consulted by [`BreakStrategy::KnuthPlass`] (see this module's own
/// doc comment). Backed by [`hyphenate`]'s real hyph-utf8-derived pattern
/// automata (the [`hypher`] crate, Typst's own hyphenator — see this
/// crate's `CLAUDE.md` typography-wave section for the hard-cutover
/// rationale, superseding the earlier hand-rolled ~20-pattern English-only
/// Liang engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hyphenation {
    /// No discretionary hyphen points — the default, matches every prior
    /// phase's behavior exactly.
    #[default]
    None,
    /// `en` hyphenation via [`Lang::English`] — kept as its own named
    /// variant (rather than requiring every caller to spell
    /// `Hyphenation::Lang(Lang::English)`) since every pre-typography-wave
    /// caller already writes `Hyphenation::English` — additive, byte-for-
    /// byte source compatible.
    English,
    /// `ru` hyphenation via [`Lang::Russian`] — same convenience shape as
    /// `English`, added this pass.
    Russian,
    /// Any other of [`hypher`]'s ~48 permissively-licensed languages —
    /// the generic escape hatch so this enum never needs a new named
    /// variant per language.
    Lang(Lang),
}
