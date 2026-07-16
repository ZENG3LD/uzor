//! Phase 5 line-breaking: [`BreakStrategy`] switches
//! [`crate::layout::layout_paragraph`] between the greedy word-wrap packer
//! (`crate::layout::greedy`, Phase 1/2) and [`knuth_plass`]'s total-fit
//! demerits-minimizing dynamic-programming breaker; [`Hyphenation`] opts a
//! paragraph into [`hyphenate`]'s Liang-pattern discretionary hyphen points.
//!
//! [`Hyphenation`] is consulted **only** by [`BreakStrategy::KnuthPlass`] —
//! Phase 1/2's greedy packer (`crate::layout::greedy::pack_lines`) is
//! unchanged and never reads a word atom's hyphenation opportunities, so
//! `Greedy` (the still-default strategy) stays byte-identical to every
//! prior phase's output regardless of what `Hyphenation` a caller sets (see
//! this crate's `CLAUDE.md` Phase 5 section for the full rationale).

pub mod hyphenate;
pub(crate) mod knuth_plass;

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
/// doc comment). English-only in v1, per the design doc's own open
/// question (§7 Q3) — this crate does not claim CJK/Arabic reflow quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hyphenation {
    /// No discretionary hyphen points — the default, matches every prior
    /// phase's behavior exactly.
    #[default]
    None,
    /// Liang-pattern discretionary hyphen points, English (`en-US`) only —
    /// see [`hyphenate`] for the pattern set and its v1 coverage scope.
    English,
}
