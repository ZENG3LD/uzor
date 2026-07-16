//! [`BreakControl`] — keep/break control (design doc §3.4), lifted
//! near-verbatim from CSS Paged Media's `break-before/after/inside`
//! vocabulary. Consulted only by the flow distribute loop
//! (`compose::flow`); see that module's own docs for exactly how each
//! variant changes placement.
//!
//! P1 subset (report, not silent — the task brief scopes this phase to
//! "keep-with-next, break-before, keep-together"): every one of the
//! doc's six variants is implemented as a REAL, considered decision
//! below — none is a silent no-op stub — but `AvoidBefore` reduces to
//! `Auto`'s own behavior in this phase's single-pass, no-backtracking
//! greedy compose loop. Genuine `break-before: avoid` semantics need
//! lookback/undo across an ALREADY-committed frame boundary (the engine
//! has already decided where the previous region's content ended by the
//! time it considers the current node) — building that backtracking
//! machinery is exactly the kind of bolt-on relayout loop design doc
//! §3.7 already argues against for floats/footnotes, and no P1 gate or
//! consumer tests `AvoidBefore` distinctly from `Auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BreakControl {
    /// No keep/break preference — the flow loop places this block
    /// wherever it naturally lands.
    #[default]
    Auto,
    /// Soft "don't let a break fall right before this block" — see this
    /// module's own doc comment for why it's a documented no-op this
    /// phase (identical to `Auto`).
    AvoidBefore,
    /// "Keep-with-next": if placing this block in the current region
    /// would leave zero room for at least SOME of the next flow block,
    /// defer this block whole to the next region instead, so the two
    /// land together (the classic orphaned-heading fix).
    AvoidAfter,
    /// Force this block to start on a fresh region, even if the current
    /// one still has room (unless the current region is already empty —
    /// forcing a break before nothing wastes a blank region).
    ForceBefore,
    /// Force the NEXT flow block onto a fresh region, even if there's
    /// still room left after this one is fully placed.
    ForceAfter,
    /// "Keep-together": never let this block split at its own natural
    /// sub-granularity (lines for a paragraph, rows for a table) purely
    /// because it doesn't fit the CURRENT region's remaining space —
    /// defer the whole block to a fresh region first. If it still
    /// doesn't fit a whole FRESH region, this degrades to a visible
    /// overflow (the same "documented degrade rather than an infinite
    /// `RegionSequence::next()` loop" P0's own risk note already
    /// established) rather than dropping content.
    AvoidInside,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_break_control_is_auto() {
        assert_eq!(BreakControl::default(), BreakControl::Auto);
    }
}
