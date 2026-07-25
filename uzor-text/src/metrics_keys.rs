//! Flat catalog of `metrics` counter keys this crate emits — matches
//! `uzor_urx_core::metrics_keys`'s own "one flat catalog, never a scattered
//! magic string per call site" convention (this workspace's established
//! `metrics` crate usage; see that crate's own `Cargo.toml` doc comment for
//! the same `metrics = "0.24"` pin).

/// Incremented once per [`crate::linebreak::knuth_plass`] call that could
/// not find a fully feasible `KnuthPlass` breaking for the whole paragraph
/// (every candidate line grouping had at least one line whose required
/// interword-glue shrink exceeded its own render-time shrink capacity) and
/// fell back to the deliberate, TeX-style "overfull hbox" pass instead —
/// see that module's own doc comment for the two-pass mechanism this
/// signals. Always `0` for [`crate::linebreak::BreakStrategy::Greedy`]
/// (that packer never reaches this code path — see this crate's `CLAUDE.md`
/// Phase 5 notes).
pub const KEY_LINEBREAK_OVERFULL_FALLBACK: &str = "text.linebreak.overfull_fallback";
