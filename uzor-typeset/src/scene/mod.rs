//! Scene model — the runtime, borrowed flow tree [`compose`](crate::compose::compose)
//! consumes. P0 builds only [`block`] (`Block`/`BlockNode`/`BlockId`);
//! `SceneSpec`/`SceneDoc`/`AnchoredIsland`/`TypesetFigure` are later-phase
//! additions (design doc §2.1-§2.4, not part of P0's own deliverables
//! list — see this crate's `CLAUDE.md` "Divergences from the design doc").

pub mod block;

pub use block::{resolve_block_ids, Block, BlockId, BlockNode};
