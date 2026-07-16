//! Block-level identity match between two [`crate::slice::frames::
//! ComposedFrame`]s (design doc §4.3): every block is matched by
//! [`crate::scene::BlockId`] equality first, with an explicit caller-
//! supplied `force_matches` override winning FIRST when present (the
//! `!!Name`-equivalent escape hatch for two blocks that don't share an id
//! but should still interpolate) — never positional/geometric guessing
//! (design doc's own "not a special-case DSL, a general primitive" law).
//! Everything left unmatched on either side is reported separately —
//! [`crate::kinetics::morph`] fades it in/out.

use std::collections::{HashMap, HashSet};

use crate::scene::BlockId;
use crate::slice::frames::ComposedFrame;

/// The result of matching every block in `from` against every block in
/// `to` — indices into each frame's own `blocks` vec.
pub(super) struct FrameMatch {
    /// `(from_index, to_index)` pairs.
    pub matched: Vec<(usize, usize)>,
    /// Indices into `from.blocks` with no partner in `to` — fade out.
    pub from_only: Vec<usize>,
    /// Indices into `to.blocks` with no partner in `from` — fade in.
    pub to_only: Vec<usize>,
}

/// Match `from`'s blocks against `to`'s. `force_matches` is a list of
/// `(from_id, to_id)` pairs the caller explicitly wants paired even when
/// the two ids differ — consulted FIRST for every `from` block, so it
/// always wins over that block's own plain-id lookup. Plain [`BlockId`]
/// equality (author-assigned OR the [`crate::scene::resolve_block_ids`]
/// structural fallback — this function treats both identically, since by
/// construction they can never collide) is the rule for every `from`
/// block not named in `force_matches`. A `to` block can be claimed at
/// most once — a caller-built `force_matches` list that would double-claim
/// one `to` block never panics, the later claimant simply falls back to
/// unmatched (a caller-construction error, not a fallible surface this
/// function exposes).
pub(super) fn match_frames(from: &ComposedFrame<'_>, to: &ComposedFrame<'_>, force_matches: &[(BlockId, BlockId)]) -> FrameMatch {
    let mut forced: HashMap<BlockId, BlockId> = HashMap::with_capacity(force_matches.len());
    for &(from_id, to_id) in force_matches {
        forced.insert(from_id, to_id);
    }

    let mut to_index_by_id: HashMap<BlockId, usize> = HashMap::with_capacity(to.blocks.len());
    for (idx, block) in to.blocks.iter().enumerate() {
        to_index_by_id.entry(block.id).or_insert(idx);
    }

    let mut matched = Vec::new();
    let mut matched_to: HashSet<usize> = HashSet::new();
    let mut from_only = Vec::new();

    for (from_idx, block) in from.blocks.iter().enumerate() {
        let target_id = forced.get(&block.id).copied().unwrap_or(block.id);
        match to_index_by_id.get(&target_id) {
            Some(&to_idx) if !matched_to.contains(&to_idx) => {
                matched.push((from_idx, to_idx));
                matched_to.insert(to_idx);
            }
            _ => from_only.push(from_idx),
        }
    }

    let to_only: Vec<usize> = (0..to.blocks.len()).filter(|idx| !matched_to.contains(idx)).collect();

    FrameMatch { matched, from_only, to_only }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::types::Rect;

    use crate::region::PlacedBlock;
    use crate::scene::Block;

    fn frame_of(blocks: Vec<PlacedBlock<'_>>) -> ComposedFrame<'_> {
        ComposedFrame { width: 100.0, height: 100.0, blocks }
    }

    fn placed<'a>(id: BlockId, x: f64, kind: &'a Block<'a>) -> PlacedBlock<'a> {
        PlacedBlock { id, rect: Rect::new(x, 0.0, 10.0, 10.0), kind, paragraph_layout: None, table_placement: None, list_placement: None }
    }

    #[test]
    fn matches_by_plain_id_equality_regardless_of_how_far_the_rects_are_apart() {
        let spacer = Block::Spacer(1.0);
        let from = frame_of(vec![placed(BlockId(1), 0.0, &spacer)]);
        let to = frame_of(vec![placed(BlockId(1), 900.0, &spacer)]);

        let result = match_frames(&from, &to, &[]);
        assert_eq!(result.matched, vec![(0, 0)], "same id must match even when the two rects are far apart -- never geometry-based");
        assert!(result.from_only.is_empty());
        assert!(result.to_only.is_empty());
    }

    #[test]
    fn blocks_at_the_same_position_with_different_ids_never_match() {
        let spacer = Block::Spacer(1.0);
        let from = frame_of(vec![placed(BlockId(1), 5.0, &spacer)]);
        let to = frame_of(vec![placed(BlockId(2), 5.0, &spacer)]);

        let result = match_frames(&from, &to, &[]);
        assert!(result.matched.is_empty(), "geometric proximity alone must never create a match");
        assert_eq!(result.from_only, vec![0]);
        assert_eq!(result.to_only, vec![0]);
    }

    #[test]
    fn force_match_pairs_two_unrelated_ids_that_plain_equality_would_never_pair() {
        let spacer = Block::Spacer(1.0);
        let from = frame_of(vec![placed(BlockId(1), 0.0, &spacer)]);
        let to = frame_of(vec![placed(BlockId(99), 200.0, &spacer)]);

        let result = match_frames(&from, &to, &[(BlockId(1), BlockId(99))]);
        assert_eq!(result.matched, vec![(0, 0)], "an explicit force-match pair must override plain id equality's own 'no match' answer");
        assert!(result.from_only.is_empty());
        assert!(result.to_only.is_empty());
    }

    /// Both a structural-ordinal id and an author-assigned id match
    /// correctly within the SAME pair — the hybrid identity model (§7 Q3)
    /// end to end.
    #[test]
    fn structural_and_author_ids_both_match_correctly_in_the_same_pair() {
        use crate::scene::{resolve_block_ids, BlockNode};

        let spacer = Block::Spacer(1.0);
        let flow = [BlockNode::new(Block::Spacer(1.0)), BlockNode::new(Block::Spacer(2.0)).with_id(BlockId(50)), BlockNode::new(Block::Spacer(3.0))];
        let ids = resolve_block_ids(&flow);

        let from = frame_of(vec![placed(ids[0], 0.0, &spacer), placed(ids[1], 10.0, &spacer), placed(ids[2], 20.0, &spacer)]);
        let to = frame_of(vec![placed(ids[0], 0.0, &spacer), placed(ids[1], 999.0, &spacer), placed(ids[2], 20.0, &spacer)]);

        let result = match_frames(&from, &to, &[]);
        assert_eq!(result.matched.len(), 3, "both structural-ordinal ids and the author id must match correctly in the same pair");
    }

    /// An author-assigned id keeps matching its own counterpart correctly
    /// even when an unrelated NEW block inserted earlier in `to`'s own
    /// flow shifts every structural ordinal after it — the concrete
    /// reason author ids exist at all (structural ordinals are positional,
    /// author ids are not).
    #[test]
    fn author_id_stays_correctly_matched_even_when_unrelated_structural_ordinals_shift_around_it() {
        use crate::scene::{resolve_block_ids, BlockNode};

        let spacer = Block::Spacer(1.0);

        let from_flow = [BlockNode::new(Block::Spacer(1.0)), BlockNode::new(Block::Spacer(2.0)).with_id(BlockId(50))];
        let from_ids = resolve_block_ids(&from_flow);

        let to_flow = [
            BlockNode::new(Block::Spacer(9.0)), // a NEW unlabeled block inserted first
            BlockNode::new(Block::Spacer(1.0)),
            BlockNode::new(Block::Spacer(2.0)).with_id(BlockId(50)),
        ];
        let to_ids = resolve_block_ids(&to_flow);

        let from = frame_of(vec![placed(from_ids[0], 0.0, &spacer), placed(from_ids[1], 10.0, &spacer)]);
        let to = frame_of(vec![placed(to_ids[0], 0.0, &spacer), placed(to_ids[1], 5.0, &spacer), placed(to_ids[2], 10.0, &spacer)]);

        let result = match_frames(&from, &to, &[]);

        let (_, author_to_idx) = *result.matched.iter().find(|&&(f, _)| from.blocks[f].id == BlockId(50)).expect("the author id must be matched");
        assert_eq!(to.blocks[author_to_idx].id, BlockId(50), "the author id must match its own counterpart despite the structural shift around it");
    }

    #[test]
    fn unmatched_blocks_land_in_the_correct_from_only_or_to_only_bucket() {
        let spacer = Block::Spacer(1.0);
        let from = frame_of(vec![placed(BlockId(1), 0.0, &spacer), placed(BlockId(2), 10.0, &spacer)]);
        let to = frame_of(vec![placed(BlockId(1), 0.0, &spacer), placed(BlockId(3), 10.0, &spacer)]);

        let result = match_frames(&from, &to, &[]);
        assert_eq!(result.matched, vec![(0, 0)]);
        assert_eq!(result.from_only, vec![1], "block 2 disappears -- from-only, fades out");
        assert_eq!(result.to_only, vec![1], "block 3 is new -- to-only, fades in");
    }
}
