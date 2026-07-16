//! [`BuildStep`]/[`ComposedFrame`]/[`slice_build_steps`] — frame slicing
//! (design doc §4.3, Arc 4 Phase P4): a slide's content sliced into a
//! sequence of BUILD STEPS (blocks appear/disappear/move between steps),
//! each yielding one fully composed [`ComposedFrame`] snapshot ready for
//! [`crate::kinetics::build_frame_morph`] between any two ADJACENT steps.
//!
//! ## One `compose()` call, N step-level filters — not N re-composes
//!
//! The design doc's own §4.3 prose is explicit: "same `compose()` call,
//! filtered to visible blocks + overrides applied." `slice_build_steps`
//! therefore composes the WHOLE `flow` through a SINGLE bounded
//! [`crate::region::FixedRegionSequence`] exactly once (every block gets
//! its own natural placed rect, precisely as [`crate::slice::slice_slides`]'s
//! own fixed mode would place the WHOLE flow) — design law 5:
//! `compose()` runs once per content/region-size change, never once per
//! step. Each [`BuildStep`] is then a pure post-hoc FILTER over that ONE
//! composed result: `visible` selects which of the base frame's already-
//! placed blocks appear at all (an invisible block simply isn't included
//! in that step's `ComposedFrame` — it doesn't reserve space, doesn't
//! shift anything else, and is treated by [`crate::kinetics::morph`] as
//! genuinely ABSENT from that step, i.e. a fade candidate, never a
//! zero-height stub); `overrides` then repositions/resizes a SPECIFIC
//! block for that one step only (e.g. "the figure sits further right
//! during step 1, then moves to its own natural composed position on step
//! 2" — the concrete mechanism [`crate::kinetics::morph::FrameMorph`]'s
//! own proof strip uses to show a block genuinely moving between steps,
//! since flow composition alone only ever stacks blocks vertically at a
//! shared `x`, never separately per step).
//!
//! ## `BlockOverride`'s own shape (coordinator decision, not in the doc)
//!
//! The design doc names `BuildStep.overrides: &'a [(BlockId,
//! BlockOverride)]` without specifying `BlockOverride`'s own fields.
//! Implemented here as a single explicit `rect: Rect` — an absolute
//! placement this block occupies during THIS step, replacing whatever
//! `compose()` would otherwise have placed it at. A container block
//! (`Block::Table`/`Block::List`) with an override has its nested cell/
//! item content translated by the SAME delta (via [`crate::region::
//! PlacedBlock`]'s own `pub(crate) translate`, not a second position
//! formula — design law 1); only the TOP-level rect's width/height are
//! overridden verbatim, nested content keeps its own original size (a
//! minor, documented approximation — nothing in this phase's own gates/
//! proof needs a table/list to also RESCALE its nested grid when
//! overridden, only move).

use uzor::types::Rect;
use uzor_text::LineShaper;

use crate::compose::{compose, ComposeStyle};
use crate::region::{FixedRegionSequence, Frame, PlacedBlock, Region};
use crate::scene::{BlockId, BlockNode};

/// One block's explicit placement override for a single [`BuildStep`] —
/// see this module's own "`BlockOverride`'s own shape" doc comment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockOverride {
    pub rect: Rect,
}

/// One build step: which blocks are visible, plus any per-block explicit
/// placement override (design doc §4.3, field names verbatim).
pub struct BuildStep<'a> {
    pub visible: &'a [BlockId],
    pub overrides: &'a [(BlockId, BlockOverride)],
}

impl<'a> BuildStep<'a> {
    pub fn new(visible: &'a [BlockId], overrides: &'a [(BlockId, BlockOverride)]) -> Self {
        Self { visible, overrides }
    }
}

/// One fully composed slide/page snapshot for a single [`BuildStep`]
/// (design doc §4.3) — a plain filtered/overridden view over the ONE
/// underlying `compose()` result [`slice_build_steps`] computes, not a
/// fresh composition of its own.
pub struct ComposedFrame<'a> {
    pub width: f64,
    pub height: f64,
    pub blocks: Vec<PlacedBlock<'a>>,
}

/// Slice `flow` into one [`ComposedFrame`] per entry of `steps` — see this
/// module's own top doc comment for why this is ONE `compose()` call
/// followed by N cheap filters, never a re-compose per step.
pub fn slice_build_steps<'a>(
    flow: &'a [BlockNode<'a>],
    steps: &[BuildStep<'_>],
    width: f64,
    height: f64,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> Vec<ComposedFrame<'a>> {
    let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, width, height));
    let base = compose(flow, &mut regions, style, shaper).into_iter().next().unwrap_or_else(|| Frame {
        region: Region { rect: Rect::new(0.0, 0.0, width, height) },
        blocks: Vec::new(),
        overflow: None,
    });

    steps
        .iter()
        .map(|step| {
            let mut blocks: Vec<PlacedBlock<'a>> = base.blocks.iter().filter(|b| step.visible.contains(&b.id)).cloned().collect();

            for &(id, over) in step.overrides {
                if let Some(placed) = blocks.iter_mut().find(|b| b.id == id) {
                    let dx = over.rect.x - placed.rect.x;
                    let dy = over.rect.y - placed.rect.y;
                    placed.translate(dx, dy);
                    placed.rect.width = over.rect.width;
                    placed.rect.height = over.rect.height;
                }
            }

            ComposedFrame { width, height, blocks }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::compose::ComposeStyle;
    use crate::scene::Block;

    fn font() -> FontSpec {
        FontSpec::new(FontFamily::Roboto, 16.0)
    }

    /// A block absent from a step's own `visible` list simply isn't in
    /// that step's `ComposedFrame` — never a zero-sized stub entry.
    #[test]
    fn a_block_absent_from_visible_does_not_appear_in_that_steps_frame() {
        let shaper = CosmicShaper::headless();
        let run_a = [StyledRun::new("Title.", font())];
        let run_b = [StyledRun::new("Body copy.", font())];

        let title_id = BlockId(1);
        let body_id = BlockId(2);

        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_a, 300.0))).with_id(title_id),
            BlockNode::new(Block::Spacer(10.0)),
            BlockNode::new(Block::Paragraph(Paragraph::new(&run_b, 300.0))).with_id(body_id),
        ];

        let style = ComposeStyle::new(6.0, font());
        let step0_visible = [title_id];
        let step1_visible = [title_id, body_id];
        let step0 = BuildStep::new(&step0_visible, &[]);
        let step1 = BuildStep::new(&step1_visible, &[]);

        let frames = slice_build_steps(&flow, &[step0, step1], 300.0, 400.0, &style, &shaper);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].blocks.len(), 1, "step 0 only shows the title");
        assert_eq!(frames[1].blocks.len(), 2, "step 1 shows both blocks");
        assert!(frames[0].blocks.iter().all(|b| b.id == title_id));
    }

    /// A per-block override replaces the naturally composed rect for its
    /// OWN step only — a step with no override for that same block keeps
    /// whatever `compose()` actually placed it at.
    #[test]
    fn a_per_block_override_replaces_the_naturally_composed_rect_for_that_step_only() {
        let shaper = CosmicShaper::headless();
        let run = [StyledRun::new("Movable caption.", font())];
        let block_id = BlockId(1);

        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, 300.0))).with_id(block_id)];

        let style = ComposeStyle::new(6.0, font());
        let override_rect = Rect::new(250.0, 300.0, 40.0, 20.0);
        let overrides = [(block_id, BlockOverride { rect: override_rect })];
        let visible = [block_id];
        let step0 = BuildStep::new(&visible, &overrides);
        let step1 = BuildStep::new(&visible, &[]);

        let frames = slice_build_steps(&flow, &[step0, step1], 300.0, 400.0, &style, &shaper);
        let step0_rect = frames[0].blocks[0].rect;
        let step1_rect = frames[1].blocks[0].rect;

        assert_eq!(step0_rect, override_rect, "the override must replace the natural composed rect exactly for its own step");
        assert_ne!(step1_rect, override_rect, "a step with no override must keep the naturally composed rect");
    }

    /// The SAME source block carries an IDENTICAL effective id across
    /// every step (all derived from the one shared `compose()` call) —
    /// exactly what `kinetics::identity`'s downstream matching needs.
    #[test]
    fn the_same_source_block_carries_an_identical_id_across_every_step() {
        let shaper = CosmicShaper::headless();
        let run = [StyledRun::new("Stable id caption.", font())];
        let flow = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, 300.0)))]; // unlabeled -> structural id

        let style = ComposeStyle::new(6.0, font());
        let step1_visible = [BlockId::structural(0)];
        let step0 = BuildStep::new(&[], &[]);
        let step1 = BuildStep::new(&step1_visible, &[]);

        let frames = slice_build_steps(&flow, &[step0, step1], 300.0, 400.0, &style, &shaper);
        assert!(frames[0].blocks.is_empty(), "no visible ids -> an empty frame, never a stub placeholder");
        assert_eq!(frames[1].blocks.len(), 1);
        assert_eq!(frames[1].blocks[0].id, BlockId::structural(0));
    }
}
