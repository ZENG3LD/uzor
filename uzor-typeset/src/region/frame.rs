//! [`Frame`]/[`PlacedBlock`] — one per region consumed by
//! [`crate::compose::compose`], holding every block instance placed inside
//! it (position + the data needed to paint) — design doc §3.2.

use uzor::types::Rect;
use uzor_text::{FontSpec, ParagraphLayout};

use super::Region;
use crate::scene::{Block, BlockId, BlockNode};

/// One [`Block`] instance placed within a [`Frame`].
#[derive(Clone)]
pub struct PlacedBlock<'a> {
    /// This block's effective id (author-assigned or structural fallback —
    /// see [`crate::scene::resolve_block_ids`]). Every fragment of a block
    /// that splits across regions (a paragraph's remaining lines, a
    /// table's remaining rows) shares this SAME id on every fragment —
    /// "this content belongs to source block X," not a fresh identity per
    /// fragment.
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
    /// `Some` only for [`Block::Table`] — the rows placed in THIS frame
    /// (a subset when the table spans more than one region) plus the
    /// table's own resolved column widths.
    pub table_placement: Option<TablePlacement<'a>>,
    /// `Some` only for [`Block::List`] — every item's marker + placed
    /// content (a list is composed as one atomic unit this phase, so this
    /// is always the WHOLE list when present at all).
    pub list_placement: Option<ListPlacement<'a>>,
}

impl<'a> PlacedBlock<'a> {
    /// Shift this block's own rect by `(dx, dy)`, recursing into any
    /// nested table/list placement's own rects (and THEIR nested content)
    /// too — a table/list cell's content is originally positioned
    /// relative to that cell's own `(0, 0)` local origin, so placing the
    /// enclosing row/item into a frame requires shifting every
    /// descendant, not just this block's own top-level rect.
    pub(crate) fn translate(&mut self, dx: f64, dy: f64) {
        self.rect.x += dx;
        self.rect.y += dy;

        if let Some(table) = &mut self.table_placement {
            for row in &mut table.rows {
                row.rect.x += dx;
                row.rect.y += dy;
                for cell in &mut row.cells {
                    cell.rect.x += dx;
                    cell.rect.y += dy;
                    for inner in &mut cell.content {
                        inner.translate(dx, dy);
                    }
                }
            }
        }

        if let Some(list) = &mut self.list_placement {
            for item in &mut list.items {
                item.marker_rect.x += dx;
                item.marker_rect.y += dy;
                for inner in &mut item.content {
                    inner.translate(dx, dy);
                }
            }
        }
    }
}

/// One region's worth of composed content.
#[derive(Clone)]
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

/// One table cell's placement within a [`PlacedTableRow`] — frame-relative
/// (this cell's own content is already translated by both its row's `y`
/// AND its column's `x`, never a second position formula in `render.rs`).
#[derive(Clone)]
pub struct PlacedTableCell<'a> {
    pub column_index: usize,
    pub rect: Rect,
    pub content: Vec<PlacedBlock<'a>>,
}

/// One placed table row.
#[derive(Clone)]
pub struct PlacedTableRow<'a> {
    pub rect: Rect,
    pub cells: Vec<PlacedTableCell<'a>>,
}

/// A table's placement within ONE [`Frame`] — see [`PlacedBlock::table_placement`].
#[derive(Clone)]
pub struct TablePlacement<'a> {
    /// Final resolved widths, same order/length as `TableBlock::columns`.
    pub column_widths: Vec<f64>,
    /// Only the rows placed in THIS frame.
    pub rows: Vec<PlacedTableRow<'a>>,
}

/// One placed list item — see [`PlacedBlock::list_placement`].
#[derive(Clone)]
pub struct PlacedListItem<'a> {
    /// Where this item's marker text paints (frame-relative).
    pub marker_rect: Rect,
    pub marker_text: String,
    /// The font the marker paints with (baked in at compose time from
    /// `ComposeStyle::default_font` — painting never needs a second,
    /// separate style lookup, design law 1).
    pub marker_font: FontSpec,
    /// This item's own content, frame-relative, already indented past the
    /// marker gutter.
    pub content: Vec<PlacedBlock<'a>>,
}

/// A list's placement — see [`PlacedBlock::list_placement`].
#[derive(Clone)]
pub struct ListPlacement<'a> {
    pub items: Vec<PlacedListItem<'a>>,
}
