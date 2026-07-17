//! Scene model — the runtime, borrowed flow tree [`compose`](crate::compose::compose)
//! consumes. P0 built [`block`] (`Block`/`BlockNode`/`BlockId`, only the
//! `Paragraph`/`Spacer` variants); P1 adds the figure/image/table/list
//! scene types below. `SceneSpec`/`SceneDoc`/`AnchoredIsland`/
//! `InteractivePlaceholder` are still later-phase additions (design doc
//! §2.1-§2.4, not part of this phase's own deliverables list — see this
//! crate's `CLAUDE.md` "Divergences from the design doc").

pub mod block;
pub mod figure_block;
pub mod image_block;
pub mod island;
pub mod list;
pub mod table;

pub use block::{resolve_block_ids, Block, BlockId, BlockNode};
pub use figure_block::{BlockSizing, FigureBlock, TypesetFigure};
pub use image_block::{ImageBlock, ImageFit};
pub use island::{AnchoredIsland, IslandAnchor};
pub use list::{ListBlock, ListItem, MarkerStyle};
pub use table::{CellPadding, ColumnSpec, TableBlock, TableCell, TableRow};
