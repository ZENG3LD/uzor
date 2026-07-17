//! [`Block`]/[`BlockNode`]/[`BlockId`] — the runtime scene tree's flow-unit
//! (borrowed, matches `uzor_text::Paragraph<'a>`'s own convention).
//!
//! P0 built only the `Paragraph`/`Spacer` variants
//! (`nemo/docs/uzor-engines/uzor_typeset_arc4_design.md` §2.1/§7 P0 phase
//! entry) — additive-only law. P1 adds `Figure`/`Image`/`Table`/`List` as
//! NEW variants (this crate's own task scope narrows the doc's §2.1
//! sketch: `Interactive` is NOT built this phase either — no P1 consumer
//! or gate needs a reserved-rect placeholder yet, deferred with the same
//! "no field/variant nothing reads" reasoning P0 already used). `BlockNode`
//! gains `break_control: BreakControl` this phase (`compose::keep_break`);
//! `style_ref: Option<StyleRef>` (`style/theme.rs`) is still a P2 addition,
//! omitted here for the same reason P0 omitted it.

use uzor_text::Paragraph;

use crate::compose::keep_break::BreakControl;
use crate::scene::figure_block::FigureBlock;
use crate::scene::image_block::ImageBlock;
use crate::scene::island::AnchoredIsland;
use crate::scene::list::ListBlock;
use crate::scene::table::TableBlock;

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
    /// An erased figure + its required [`crate::scene::BlockSizing`] —
    /// ATOMIC (never split across regions, design doc §2.2/§7 P1).
    Figure(FigureBlock<'a>),
    /// A raster image + its required sizing/fit — ATOMIC, same
    /// all-or-nothing placement as [`Block::Figure`].
    Image(ImageBlock<'a>),
    /// An image anchored in the flow, with the flow content immediately
    /// following it running beside it in a side strip (design doc §2.3,
    /// adapted — see `scene::island`'s own module docs for the
    /// divergence from the doc's free-canvas original). ATOMIC — never
    /// split across regions, same all-or-nothing placement as
    /// [`Block::Figure`]/[`Block::Image`].
    Island(AnchoredIsland<'a>),
    /// A two-pass column-sized table — row-atomic splitting across
    /// regions (design doc §3.5).
    Table(TableBlock<'a>),
    /// A marker + indent list — composed as ONE atomic unit this phase
    /// (see `scene::list`'s own module docs).
    List(ListBlock<'a>),
    /// A fixed vertical gap between flow blocks (no content, no measure
    /// pass — a plain, caller-chosen `f64` height in the same units as
    /// every other block's rect).
    Spacer(f64),
}

/// A [`BlockNode`] tagged as a document-outline / table-of-contents entry
/// via [`BlockNode::with_outline`] (document-navigation feature pass —
/// `nemo/docs/uzor-engines/research_typesetting_sota_2026.md`'s
/// counter-introspection item: one mechanism powers counters/TOC/
/// cross-refs, this crate's own narrow slice of that is "collect every
/// tagged block's own resolved page number," the page-slicing pass
/// itself being the counter). `title` is explicit, never derived from the
/// tagged block's own content — the design doc's own outline/TOC sketch
/// names `(numbering, title, page-number)` triples resolved from
/// introspection, but a heading's PAINTED text and its OUTLINE label are
/// legitimately different strings in general (e.g. a numbered heading
/// like `"3. Results"` vs. an outline entry that drops the number) — an
/// explicit title keeps the caller in control rather than this crate
/// guessing by walking a `Paragraph`'s own runs.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineTag {
    pub level: u8,
    pub title: String,
}

/// One node in a [`crate::compose::compose`] flow: a [`Block`] plus its
/// (possibly author-assigned) identity and its keep/break preference.
pub struct BlockNode<'a> {
    pub id: Option<BlockId>,
    pub kind: Block<'a>,
    pub break_control: BreakControl,
    /// `Some` when this node is a document-outline/TOC entry — see
    /// [`OutlineTag`]/[`BlockNode::with_outline`]. `None` (the default)
    /// for every ordinary flow block.
    pub outline: Option<OutlineTag>,
    /// `Some(page_index)` when this node's own placed rect(s) should
    /// become an internal-link (GoTo) source pointing at `page_index`
    /// (0-based) — see [`BlockNode::with_link_target`]. Used by
    /// `crate::toc::build_toc`'s own generated rows (a TOC row links to
    /// the heading it names), but not restricted to TOC rows — any block
    /// can be tagged. `None` (the default) for every ordinary flow block.
    pub link_target: Option<u32>,
}

impl<'a> BlockNode<'a> {
    /// An unlabeled, `BreakControl::Auto` node — gets a structural id
    /// from [`resolve_block_ids`].
    pub fn new(kind: Block<'a>) -> Self {
        Self { id: None, kind, break_control: BreakControl::Auto, outline: None, link_target: None }
    }

    /// Builder: assign an explicit author id.
    pub fn with_id(mut self, id: BlockId) -> Self {
        self.id = Some(id);
        self
    }

    /// Builder: set this node's keep/break preference.
    pub fn with_break_control(mut self, break_control: BreakControl) -> Self {
        self.break_control = break_control;
        self
    }

    /// Builder: tag this node as a document-outline/TOC entry at `level`
    /// (1-based, matching heading-level convention: `1` = top-level
    /// section) with an explicit `title` (usually the heading paragraph's
    /// own text, but the caller decides — see [`OutlineTag`]'s own doc
    /// comment for why).
    pub fn with_outline(mut self, level: u8, title: impl Into<String>) -> Self {
        self.outline = Some(OutlineTag { level, title: title.into() });
        self
    }

    /// Builder: tag this node's own placed rect(s) as an internal-link
    /// (GoTo) source pointing at `page_index` (0-based) — see
    /// [`BlockNode::link_target`]'s own doc comment.
    pub fn with_link_target(mut self, page_index: u32) -> Self {
        self.link_target = Some(page_index);
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
