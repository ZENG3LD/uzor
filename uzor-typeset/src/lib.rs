//! `uzor-typeset` — typographic composition + slicing engine for uzor
//! (Arc 4).
//!
//! A scene of [`Block`]s is composed (flow layout through
//! [`RegionSequence`]s) and then sliced into pages | slides | frames — a
//! deck is a slicing target, not a sibling engine
//! (`nemo/docs/uzor-engines/uzor_typeset_arc4_design.md` §1). This crate
//! owns composition and slicing only: it does not shape glyphs (that's
//! `uzor-text`), does not compute scales/marks (`uzor-figures`), does not
//! rasterize or assemble files (`uzor-export`), and holds no
//! editing/selection/domain state.
//!
//! This crate currently implements **Phase P0 + Phase P1** of the design
//! doc: the scene model's flow-unit (`scene::Block`/`BlockNode`/`BlockId`,
//! `Paragraph`/`Spacer` from P0 plus `Figure`/`Image`/`Table`/`List` from
//! P1), the region abstraction (`region::{Region, RegionSequence,
//! PageRegionSequence, FixedRegionSequence}`), keep/break-aware compose
//! (`compose::compose`, `compose::BreakControl`), and page slicing
//! (`slice::{PageMaster, slice_pages}`) — see this crate's `CLAUDE.md` for
//! exactly what each phase built vs. deferred, and where its
//! implementation diverges from the design doc's own pseudocode.

pub mod compose;
pub mod region;
pub mod render;
pub mod scene;
pub mod slice;

pub use compose::{compose, BreakControl, ComposeStyle};
pub use region::{
    FixedRegionSequence, Frame, ListPlacement, PageRegionSequence, PlacedBlock, PlacedListItem, PlacedTableCell, PlacedTableRow, Region,
    RegionSequence, TablePlacement,
};
pub use render::draw_page;
pub use scene::{
    resolve_block_ids, Block, BlockId, BlockNode, BlockSizing, ColumnSpec, FigureBlock, ImageBlock, ImageFit, ListBlock, ListItem,
    MarkerStyle, TableBlock, TableCell, TableRow, TypesetFigure,
};
pub use slice::{slice_pages, Margins, Page, PageMaster};
