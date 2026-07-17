//! [`AnchoredIsland`]/[`IslandAnchor`] — an image anchored in the flow,
//! with the following flow content running beside it in a side strip
//! (design doc §2.3's own file placement, `scene/island.rs`, though this
//! crate's own `AnchoredIsland` shape diverges from the doc's — see this
//! module's own "Divergence" doc comment below).
//!
//! ## Divergence from the design doc's own `AnchoredIsland` (report)
//!
//! The design doc's §2.3 `AnchoredIsland` is a FIXED-POSITION, NON-FLOW-
//! participating element living in `SceneSpec.islands` — its own explicit
//! `rect`/`z_index`/`reading_order`, independent of the flow's own reading
//! order (PowerPoint's own free-canvas model). `SceneSpec` itself is not
//! part of this crate yet (§2.1, deferred past every phase so far — see
//! this crate's `CLAUDE.md`). This feature's own task brief instead asks
//! for "images anchored IN THE FLOW with text running beside them" — a
//! genuinely different placement model (reading-order-driven, closer to
//! Word's own inline "square"/"tight" text wrap than PowerPoint's free
//! canvas). Implemented here as a NEW, additive [`crate::scene::Block::Island`]
//! flow variant instead: an `AnchoredIsland` reserves its own rect at
//! whatever the CURRENT flow cursor is when [`crate::compose::compose`]
//! reaches it (never an author-authored absolute `rect`), and the blocks
//! immediately following it in the SAME flow are the ones that run beside
//! it — see `compose::flow`'s own module docs for the exact strip-filling
//! mechanics. A future `SceneSpec`-based, truly non-flow-participating
//! `AnchoredIsland` (the doc's own original shape) can still be added
//! additively later without touching this one — they answer genuinely
//! different questions ("where does an image sit independent of reading
//! order" vs. "where does TEXT flow around an image").

use crate::scene::image_block::ImageBlock;

/// Which side of an [`AnchoredIsland`]'s own image sits, and therefore
/// which side(s) [`crate::compose::flow`] carves side-strip regions for
/// the flow content immediately following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandAnchor {
    /// Image on the LEFT — text runs in a strip to its RIGHT.
    Left,
    /// Image on the RIGHT — text runs in a strip to its LEFT.
    Right,
    /// Image horizontally CENTERED — text runs in a strip on EACH side
    /// (left strip filled first, then the right one — see
    /// `compose::flow`'s own module docs for why equal-width strips let a
    /// paragraph continue cleanly from one into the other).
    Center,
}

/// An image anchored in the flow, with the flow content immediately
/// following it running beside it in a side strip for the image's own
/// vertical band (this module's own "Divergence" doc comment explains why
/// this differs from the design doc's own free-canvas `AnchoredIsland`).
/// ATOMIC exactly like [`crate::scene::Block::Figure`]/
/// [`crate::scene::Block::Image`] — never split across regions; one
/// taller than the remaining region space defers WHOLE to the next
/// region.
pub struct AnchoredIsland<'a> {
    pub image: ImageBlock<'a>,
    pub anchor: IslandAnchor,
    /// This island's own width — NOT taken from `image.sizing` (which
    /// governs HEIGHT only, matching `Block::Figure`/`Block::Image`'s own
    /// "regions own the width" convention elsewhere in this crate). An
    /// island is the ONE exception: its width is a caller choice
    /// independent of the region it sits in, since the whole point of a
    /// side-strip layout is that the image does NOT fill the region's
    /// full width.
    pub width: f64,
    /// Horizontal gap between the island's own image rect and its side
    /// strip(s) — a required, explicit field (no implicit default gap),
    /// matching [`crate::scene::BlockSizing`]'s own "no field nothing
    /// reads" convention.
    pub margin: f64,
}

impl<'a> AnchoredIsland<'a> {
    pub fn new(image: ImageBlock<'a>, anchor: IslandAnchor, width: f64, margin: f64) -> Self {
        Self { image, anchor, width, margin }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{BlockSizing, ImageFit};

    fn stub_image(rgba: &[u8]) -> ImageBlock<'_> {
        ImageBlock::new(rgba, 100, 100, BlockSizing::FixedHeight(120.0), ImageFit::Cover)
    }

    #[test]
    fn constructor_carries_every_field_verbatim() {
        let rgba = [0u8; 16];
        let island = AnchoredIsland::new(stub_image(&rgba), IslandAnchor::Center, 200.0, 12.0);
        assert_eq!(island.anchor, IslandAnchor::Center);
        assert_eq!(island.width, 200.0);
        assert_eq!(island.margin, 12.0);
    }
}
