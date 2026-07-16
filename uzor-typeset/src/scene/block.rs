//! [`Block`]/[`BlockNode`]/[`BlockId`] — the runtime scene tree's flow-unit
//! (borrowed, matches `uzor_text::Paragraph<'a>`'s own convention).
//!
//! P0 scope (`nemo/docs/uzor-engines/uzor_typeset_arc4_design.md` §2.1/§7
//! P0 phase entry): only the `Paragraph`/`Spacer` variants exist —
//! additive-only law, P1 adds `Figure`/`Image`/`Table`/`List`/`Interactive`
//! as NEW variants later, never a pre-declared placeholder now.
//! `BlockNode` likewise carries only `id`/`kind` this phase —
//! `break_control: BreakControl` (`compose/keep_break.rs`) and
//! `style_ref: Option<StyleRef>` (`style/theme.rs`) are P1/P2 additions,
//! omitted here for the same reason.

use uzor_text::Paragraph;

/// Stable identity anchor for a [`BlockNode`], used by later phases' morph
/// matching (§4.3 of the design doc).
///
/// §7 Q3's coordinator decision: authoring is a hybrid — an author/importer
/// MAY assign a `BlockId` explicitly (`BlockNode::id: Some(..)`); an
/// unlabeled block (`None`) still gets a stable identity via
/// [`resolve_block_ids`]'s deterministic structural-ordinal fallback, so
/// every placed block has a concrete, always-`Some`-shaped effective id by
/// the time it reaches a [`crate::region::PlacedBlock`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u64);

impl BlockId {
    /// High bit tags an engine-assigned structural ordinal, so it can never
    /// collide with an author-assigned id that happens to reuse a small
    /// integer — author ids and structural ordinals occupy disjoint halves
    /// of the `u64` space.
    const STRUCTURAL_TAG: u64 = 1 << 63;

    /// Construct a structural (engine-assigned) id from a 0-based ordinal
    /// (the count of unlabeled blocks seen before this one, in flow order).
    pub fn structural(ordinal: u64) -> Self {
        BlockId(Self::STRUCTURAL_TAG | ordinal)
    }

    /// `true` for an id produced by [`BlockId::structural`] (as opposed to
    /// an author-assigned one).
    pub fn is_structural(&self) -> bool {
        self.0 & Self::STRUCTURAL_TAG != 0
    }
}

/// One flow-participating content unit.
///
/// Runtime tree, borrowed (design law 3: stateless layout over borrowed
/// snapshots) — mirrors `uzor_text::Paragraph<'a>`'s own borrow
/// convention so `Block::Paragraph` holds a real `Paragraph<'a>` with zero
/// copying.
pub enum Block<'a> {
    /// A paragraph of styled text, measured/laid out via
    /// `uzor_text::layout_paragraph` — the content_size seam (design doc
    /// §3.3).
    Paragraph(Paragraph<'a>),
    /// A fixed vertical gap between flow blocks (no content, no measure
    /// pass — a plain, caller-chosen `f64` height in the same units as
    /// every other block's rect).
    Spacer(f64),
}

/// One node in a [`crate::compose::compose`] flow: a [`Block`] plus its
/// (possibly author-assigned) identity.
pub struct BlockNode<'a> {
    pub id: Option<BlockId>,
    pub kind: Block<'a>,
}

impl<'a> BlockNode<'a> {
    /// An unlabeled node — gets a structural id from [`resolve_block_ids`].
    pub fn new(kind: Block<'a>) -> Self {
        Self { id: None, kind }
    }

    /// Builder: assign an explicit author id.
    pub fn with_id(mut self, id: BlockId) -> Self {
        self.id = Some(id);
        self
    }
}

/// Effective id for every node in `flow`, in order: the author's
/// [`BlockNode::id`] when present, else a deterministic
/// [`BlockId::structural`] ordinal (the count of unlabeled blocks seen so
/// far, 0-based) — stable across two [`crate::compose::compose`] runs over
/// the same tree shape (§7 Q3), since it depends only on `flow`'s own
/// `id`-vs-`None` pattern, never on layout/region state.
pub fn resolve_block_ids(flow: &[BlockNode<'_>]) -> Vec<BlockId> {
    let mut next_ordinal = 0u64;
    flow.iter()
        .map(|node| match node.id {
            Some(id) => id,
            None => {
                let id = BlockId::structural(next_ordinal);
                next_ordinal += 1;
                id
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{FontSpec, StyledRun};

    fn spacer(gap: f64) -> BlockNode<'static> {
        BlockNode::new(Block::Spacer(gap))
    }

    #[test]
    fn structural_ids_never_collide_with_the_author_id_space() {
        let author = BlockId(3);
        let structural = BlockId::structural(3);
        assert_ne!(author, structural);
        assert!(!author.is_structural());
        assert!(structural.is_structural());
    }

    #[test]
    fn author_id_is_preserved_verbatim() {
        let flow = [spacer(1.0).with_id(BlockId(42)), spacer(2.0)];
        let ids = resolve_block_ids(&flow);
        assert_eq!(ids[0], BlockId(42));
        assert!(ids[1].is_structural());
    }

    #[test]
    fn unlabeled_blocks_get_stable_structural_ordinals_across_two_runs() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let runs = [StyledRun::new("hello", font)];
        let flow = [
            BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 200.0))).with_id(BlockId(7)),
            spacer(4.0),
            spacer(8.0),
        ];

        let first_run = resolve_block_ids(&flow);
        let second_run = resolve_block_ids(&flow);
        assert_eq!(first_run, second_run, "same tree shape must resolve to the same ids every compose run");

        assert_eq!(first_run[0], BlockId(7), "author id preserved");
        assert_eq!(first_run[1], BlockId::structural(0));
        assert_eq!(first_run[2], BlockId::structural(1));
    }
}
