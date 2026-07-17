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
//! Phase P3 + Phase P4** of the design doc: the scene model's flow-unit
//! (`scene::Block`/`BlockNode`/`BlockId`, `Paragraph`/`Spacer` from P0
//! plus `Figure`/`Image`/`Table`/`List` from P1), the region abstraction
//! (`region::{Region, RegionSequence, PageRegionSequence,
//! FixedRegionSequence, CardRegionSequence}`), keep/break-aware compose
//! (`compose::compose`, `compose::BreakControl`), page slicing with
//! header/footer/page-number (`slice::{PageMaster, slice_pages}`,
//! `master::page_master`), masters/placeholders (`master::{SlideMaster,
//! SlideLayout, PlaceholderKind, PlaceholderSlot, SlideInstance}`), style
//! resolution (`style::{PropertyState, Theme}`), slide slicing (P3) —
//! fluid card mode + fixed PowerPoint-viewport mode, both over ONE content
//! model (`slice::{Card, Slide, slice_cards, slice_slides, cards_to_slides,
//! slides_to_cards, slice_slide_instance, SlideOverflow, SliceError}`),
//! painted via `render::{draw_card, draw_slide}` — and, this phase (P4),
//! frames/build-steps + morph: `slice::{BuildStep, BlockOverride,
//! ComposedFrame, slice_build_steps}` slices a slide's content into a
//! sequence of build steps, and `kinetics::{build_frame_morph, FrameMorph,
//! FrameBlockState}` identity-matches two ADJACENT steps by `BlockId`
//! (author-assigned or structural fallback), delegating a matched
//! equal-text `Paragraph` pair wholesale to `uzor_text::kinetics` for
//! glyph-level interpolation, painted via `render::draw_frame_state`.
//!
//! Also implements **Phase P5** (PDF assembly, `export::pdf_adapter::
//! pages_to_pdf`) plus a later feature pass building real multi-column
//! pages (`region::ColumnRegionSequence`/`region::PageRegionSequence::
//! with_columns`, `master::PageMaster::with_columns`, `slice::Page::
//! extra_frames`) and anchored islands — images anchored in the flow with
//! text running beside them in a side strip (`scene::{AnchoredIsland,
//! IslandAnchor}`, `Block::Island`) — and closes the `ImagePainter` paint
//! gap for real (`Block::Image`/`Block::Island` composite real pixels
//! through `RenderContext::image_painter()` now, dashed-magenta
//! placeholder as a documented fallback only). See this crate's
//! `CLAUDE.md` for exactly what each phase/pass built vs. deferred, and
//! where its implementation diverges from the design doc's own
//! pseudocode.

pub mod compose;
pub mod export;
pub mod kinetics;
pub mod master;
pub mod region;
pub mod render;
pub mod scene;
pub mod slice;
pub mod style;
pub mod toc;

pub use compose::{compose, BreakControl, ComposeStyle};
pub use export::pages_to_pdf;
pub use kinetics::{build_frame_morph, FrameBlockState, FrameMorph};
pub use master::{
    LayoutId, MasterId, PlaceholderFill, PlaceholderKind, PlaceholderSlot, PlacedPlaceholder, PageNumberFormat, PageNumberStyle,
    SlideInstance, SlideLayout, SlideMaster,
};
pub use region::{
    CardRegionSequence, ColumnRegionSequence, FixedRegionSequence, Frame, ListPlacement, PageRegionSequence, PlacedBlock, PlacedListItem,
    PlacedTableCell, PlacedTableRow, Region, RegionSequence, TablePlacement,
};
pub use render::{draw_card, draw_frame_state, draw_page, draw_page_layers, draw_slide, DrawLayers};
pub use scene::{
    resolve_block_ids, AnchoredIsland, Block, BlockId, BlockNode, BlockSizing, CellPadding, ColumnSpec, FigureBlock, ImageBlock, ImageFit,
    IslandAnchor, ListBlock, ListItem, MarkerStyle, OutlineTag, TableBlock, TableCell, TableRow, TypesetFigure,
};
pub use slice::{
    cards_to_slides, renumber_pages, slice_build_steps, slice_cards, slice_pages, slice_slide_instance, slice_slides, slides_to_cards,
    BlockOverride, BuildStep, Card, ComposedFrame, LinkEntry, Margins, OutlineEntry, Page, PageMaster, PageNumberPlacement, SliceError,
    Slide, SlideOverflow,
};
pub use style::{
    resolve_property_chain, BrandTokens, ColorRole, ComponentStyle, DesignTokens, FigureThemeTokens, FontFileRef, FontRole, PropertyState,
    ResolvedProperty, RootPropertyState, TextStyle, Theme,
};
pub use toc::{build_toc, build_toc_rows, compose_document_with_toc, dotted_leader, TocRow, TocStyle};
