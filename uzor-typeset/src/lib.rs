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
//! This crate currently implements **Phase P0 + Phase P1 + Phase P2 +
//! Phase P3** of the design doc: the scene model's flow-unit
//! (`scene::Block`/`BlockNode`/`BlockId`, `Paragraph`/`Spacer` from P0
//! plus `Figure`/`Image`/`Table`/`List` from P1), the region abstraction
//! (`region::{Region, RegionSequence, PageRegionSequence,
//! FixedRegionSequence, CardRegionSequence}`), keep/break-aware compose
//! (`compose::compose`, `compose::BreakControl`), page slicing with
//! header/footer/page-number (`slice::{PageMaster, slice_pages}`,
//! `master::page_master`), masters/placeholders (`master::{SlideMaster,
//! SlideLayout, PlaceholderKind, PlaceholderSlot, SlideInstance}`), style
//! resolution (`style::{PropertyState, Theme}`), and — this phase (P3) —
//! slide slicing: fluid card mode + fixed PowerPoint-viewport mode, both
//! over ONE content model (`slice::{Card, Slide, slice_cards,
//! slice_slides, cards_to_slides, slides_to_cards, slice_slide_instance,
//! SlideOverflow, SliceError}`), painted via `render::{draw_card,
//! draw_slide}` — see this crate's `CLAUDE.md` for exactly what each
//! phase built vs. deferred, and where its implementation diverges from
//! the design doc's own pseudocode.

pub mod compose;
pub mod master;
pub mod region;
pub mod render;
pub mod scene;
pub mod slice;
pub mod style;

pub use compose::{compose, BreakControl, ComposeStyle};
pub use master::{
    LayoutId, MasterId, PlaceholderFill, PlaceholderKind, PlaceholderSlot, PlacedPlaceholder, PageNumberFormat, PageNumberStyle,
    SlideInstance, SlideLayout, SlideMaster,
};
pub use region::{
    CardRegionSequence, FixedRegionSequence, Frame, ListPlacement, PageRegionSequence, PlacedBlock, PlacedListItem, PlacedTableCell,
    PlacedTableRow, Region, RegionSequence, TablePlacement,
};
pub use render::{draw_card, draw_page, draw_slide};
pub use scene::{
    resolve_block_ids, Block, BlockId, BlockNode, BlockSizing, ColumnSpec, FigureBlock, ImageBlock, ImageFit, ListBlock, ListItem,
    MarkerStyle, TableBlock, TableCell, TableRow, TypesetFigure,
};
pub use slice::{
    cards_to_slides, slice_cards, slice_pages, slice_slide_instance, slice_slides, slides_to_cards, Card, Margins, Page, PageMaster,
    PageNumberPlacement, SliceError, Slide, SlideOverflow,
};
pub use style::{
    resolve_property_chain, BrandTokens, ColorRole, ComponentStyle, DesignTokens, FigureThemeTokens, FontFileRef, FontRole, PropertyState,
    ResolvedProperty, RootPropertyState, TextStyle, Theme,
};
