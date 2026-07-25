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

/// Line-breaking tuning knobs (typography track T5, 2026-07-25) — every
/// constant [`knuth_plass`]'s own DP + [`hyphenate`]'s own minimums used to
/// hardcode, exposed through [`crate::model::Paragraph::line_break_params`]/
/// `with_line_break_params` — the same builder surface the hyphenation
/// LANGUAGE already used (this enum's own `Lang(..)` escape hatch), closing
/// the inconsistency this task's own brief names directly.
///
/// [`Default`] reproduces every one of today's hardcoded values EXACTLY —
/// this crate's own doctrine (`CLAUDE.md`'s "never change rendered output
/// silently"): a debatable default becomes an option whose default
/// preserves today's behavior. Every caller that never touches
/// `line_break_params` gets byte-for-byte identical output to before this
/// wave (see `linebreak::knuth_plass`'s own
/// `default_line_break_params_reproduce_the_hardcoded_constants_exactly`
/// test).
///
/// **No `\tolerance`/`\looseness` field** — [`knuth_plass`]'s own module doc
/// already states this DP is deliberately simplified relative to real TeX
/// ("no looseness passes, no TeX fitness-class tiering... a single-pass DP
/// over feasible breakpoints"): there is no multi-pass looseness-retry loop
/// or fitness-class tiering anywhere in this implementation for such a
/// field to plumb into. Adding an inert `tolerance: f64` nothing reads
/// would be exactly the "field nothing reads" anti-pattern this workspace's
/// own conventions forbid (see this crate's `CLAUDE.md`, e.g. the Phase 2
/// `BreakStrategy` divergence note making the identical call the other
/// direction). If a future pass adds real looseness/fitness-class support,
/// its own knob belongs on `LineBreakParams` then, not as a placeholder now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineBreakParams {
    /// Cost of a discretionary hyphenation break (Knuth's
    /// `\hyphenpenalty`). Higher discourages hyphenation more strongly
    /// relative to a looser/tighter ordinary line.
    pub hyphen_penalty: f64,
    /// Extra demerits when two consecutive chosen lines both end in a
    /// discretionary hyphen (Knuth's `\doublehyphendemerits`) —
    /// discourages a visual "staircase" of hyphens down the margin.
    pub double_hyphen_demerit: f64,
    /// Interword glue stretch as a fraction of its own natural width (TeX's
    /// conventional `stretch = w/2`).
    pub glue_stretch_ratio: f64,
    /// Interword glue shrink as a fraction of its own natural width (TeX's
    /// conventional `shrink = w/3`) — also caps how far
    /// [`crate::layout::layout_paragraph`]'s own glue-shrink render pass may
    /// compress a line (for BOTH [`BreakStrategy::Greedy`] and
    /// [`BreakStrategy::KnuthPlass`] output), so a line the DP scored as
    /// "shrink covers the gap" never renders with MORE compression than the
    /// cost model actually assumed.
    pub glue_shrink_ratio: f64,
    /// Minimum letters a hyphenation point must leave BEFORE it (TeX's
    /// `\lefthyphenmin`), counted in characters (not bytes — see
    /// [`hyphenate::hyphenation_points`]'s own doc comment for why).
    pub left_min: usize,
    /// Minimum letters a hyphenation point must leave AFTER it (TeX's
    /// `\righthyphenmin`), counted in characters.
    pub right_min: usize,
}

impl Default for LineBreakParams {
    /// Reproduces [`knuth_plass`]'s pre-T5 hardcoded constants and
    /// [`hyphenate`]'s pre-T5 `LEFT_MIN`/`RIGHT_MIN` exactly.
    fn default() -> Self {
        Self { hyphen_penalty: 50.0, double_hyphen_demerit: 3000.0, glue_stretch_ratio: 0.5, glue_shrink_ratio: 1.0 / 3.0, left_min: 2, right_min: 3 }
    }
}
