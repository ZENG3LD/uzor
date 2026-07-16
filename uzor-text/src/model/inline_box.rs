//! [`InlineBox`] — a reserved rectangle spliced into a [`crate::model::Paragraph`]'s
//! run sequence (parley's taxonomy, lifted per
//! `research_text_presentation_engines.md` §1).
//!
//! The caller draws whatever goes inside the box (an icon, an inline
//! figure) — [`crate::draw::draw_paragraph`] only ever reserves the space
//! (and, behind its `debug_outline_boxes` flag, strokes an outline rect for
//! visual proof) via [`crate::layout::PlacedInlineBox`].

/// How an [`InlineBox`] participates in the paragraph's line flow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InlineBoxKind {
    /// Reserves `width` x `height` in the running text flow: it is an
    /// atomic (non-breakable) unit in the wrap accumulator, and — if
    /// taller than the surrounding text — grows the line's height.
    InFlow { width: f64, height: f64 },
    /// Does not participate in the line's wrap/advance accounting — zero
    /// width/height contribution, placed at the current pen position.
    /// The caller owns its actual visual placement (e.g. an absolutely
    /// positioned float); Phase 2 only builds `InFlow`'s reserved-space
    /// contract, so this variant is an inert placeholder for now.
    OutOfFlow,
    /// Same as [`OutOfFlow`](Self::OutOfFlow), tagged for app-defined
    /// custom placement logic (parley's taxonomy) — also inert in Phase 2.
    CustomOutOfFlow,
}

/// A box spliced into a paragraph's content stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineBox {
    /// Caller-assigned identity — echoed back on
    /// [`crate::layout::PlacedInlineBox::id`] so the caller can match a
    /// placement back to whatever it should draw there.
    pub id: u64,
    pub kind: InlineBoxKind,
}

impl InlineBox {
    /// An in-flow box reserving `width` x `height` (negative dimensions
    /// clamp to `0.0` — no fallible surface on the hot path).
    pub fn in_flow(id: u64, width: f64, height: f64) -> Self {
        Self { id, kind: InlineBoxKind::InFlow { width: width.max(0.0), height: height.max(0.0) } }
    }

    /// An out-of-flow box (zero width/height contribution to wrap).
    pub fn out_of_flow(id: u64) -> Self {
        Self { id, kind: InlineBoxKind::OutOfFlow }
    }

    /// An out-of-flow box tagged for app-defined custom placement.
    pub fn custom_out_of_flow(id: u64) -> Self {
        Self { id, kind: InlineBoxKind::CustomOutOfFlow }
    }

    /// Width this box reserves in the wrap accumulator (`0.0` unless
    /// [`InlineBoxKind::InFlow`]).
    pub fn width(&self) -> f64 {
        match self.kind {
            InlineBoxKind::InFlow { width, .. } => width,
            InlineBoxKind::OutOfFlow | InlineBoxKind::CustomOutOfFlow => 0.0,
        }
    }

    /// Height this box reserves (`0.0` unless [`InlineBoxKind::InFlow`]).
    pub fn height(&self) -> f64 {
        match self.kind {
            InlineBoxKind::InFlow { height, .. } => height,
            InlineBoxKind::OutOfFlow | InlineBoxKind::CustomOutOfFlow => 0.0,
        }
    }

    /// This box's contribution to its line's shared baseline pass
    /// (`(ascent, descent)` — see [`crate::layout`]'s baseline module).
    ///
    /// Baseline behavior: an in-flow box's **bottom edge sits on the
    /// line's baseline** (its full height counts as ascent, zero
    /// descent) — the common inline-image convention, and the interpretation
    /// chosen where the design doc leaves "baseline behavior" unspecified.
    pub fn ascent(&self) -> f64 {
        self.height()
    }

    /// See [`Self::ascent`] — always `0.0` under the bottom-on-baseline
    /// convention this crate implements.
    pub fn descent(&self) -> f64 {
        0.0
    }
}

/// Anchor splicing `inline_box` into [`crate::model::Paragraph::runs`]`[run_index]`'s
/// text, immediately before `byte_offset` (`byte_offset == text.len()`
/// splices after that run's text, before whatever comes next).
///
/// An out-of-range `run_index`/`byte_offset` is silently dropped by
/// [`crate::layout::layout_paragraph`] rather than panicking (design law:
/// no fallible surface on the hot path) — `byte_offset` is also rounded
/// down to the nearest `char` boundary if it lands mid-character.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InlineBoxSlot {
    pub run_index: usize,
    pub byte_offset: usize,
    pub inline_box: InlineBox,
}

impl InlineBoxSlot {
    pub fn new(run_index: usize, byte_offset: usize, inline_box: InlineBox) -> Self {
        Self { run_index, byte_offset, inline_box }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flow_reports_its_own_width_height_and_bottom_on_baseline_metrics() {
        let b = InlineBox::in_flow(1, 24.0, 40.0);
        assert_eq!(b.width(), 24.0);
        assert_eq!(b.height(), 40.0);
        assert_eq!(b.ascent(), 40.0);
        assert_eq!(b.descent(), 0.0);
    }

    #[test]
    fn in_flow_clamps_negative_dimensions_to_zero() {
        let b = InlineBox::in_flow(2, -5.0, -1.0);
        assert_eq!(b.width(), 0.0);
        assert_eq!(b.height(), 0.0);
    }

    #[test]
    fn out_of_flow_variants_contribute_nothing_to_wrap_or_baseline() {
        let a = InlineBox::out_of_flow(3);
        let b = InlineBox::custom_out_of_flow(4);
        for box_ in [a, b] {
            assert_eq!(box_.width(), 0.0);
            assert_eq!(box_.height(), 0.0);
            assert_eq!(box_.ascent(), 0.0);
            assert_eq!(box_.descent(), 0.0);
        }
    }
}
