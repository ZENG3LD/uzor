//! [`Frame`]/[`PlacedBlock`] — one per region consumed by
//! [`crate::compose::compose`], holding every block instance placed inside
//! it (position + the data needed to paint) — design doc §3.2.

use uzor::types::Rect;
use uzor_text::ParagraphLayout;

use super::Region;
use crate::scene::{Block, BlockId, BlockNode};

/// One [`Block`] instance placed within a [`Frame`].
pub struct PlacedBlock<'a> {
    /// This block's effective id (author-assigned or structural fallback —
    /// see [`crate::scene::resolve_block_ids`]).
    pub id: BlockId,
    /// This block's own placed rect within the frame (frame-relative
    /// origin — same coordinate space as [`Frame::region`]'s own rect).
    pub rect: Rect,
    /// Borrowed back from the source flow — painting reads straight from
    /// `Block` (design law 1: one transform).
    pub kind: &'a Block<'a>,
    /// `Some` only for [`Block::Paragraph`] — the already-measured layout
    /// for exactly the lines placed in THIS frame (re-based so its own
    /// first line's `y_top` is `0.0`; see
    /// `crate::compose::flow`'s module docs for how a paragraph spanning
    /// more than one region is sliced).
    pub paragraph_layout: Option<ParagraphLayout>,
}

/// One region's worth of composed content.
pub struct Frame<'a> {
    pub region: Region,
    pub blocks: Vec<PlacedBlock<'a>>,
    /// The block that didn't fully fit in THIS frame and continues in the
    /// next one `compose()` produces (or, for a finite [`super::RegionSequence`],
    /// the block a caller must carry into its own next `compose()` call).
    /// `None` when every block up to the end of `blocks` was placed in
    /// full.
    pub overflow: Option<&'a BlockNode<'a>>,
}
