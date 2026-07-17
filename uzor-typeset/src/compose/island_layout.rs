//! [`island_placement_rect`]/[`island_strip_rects`] — pure geometry for an
//! [`crate::scene::AnchoredIsland`]'s own reserved image rect + the side
//! strip(s) text runs beside it (design brief's own compose-level island
//! seam — see `compose::flow`'s own module docs for how these two are
//! actually wired into the flow-distribute loop).

use uzor::types::Rect;

use crate::scene::{AnchoredIsland, IslandAnchor};

/// This island's own reserved rect: `island.width` wide, `height` tall
/// (already resolved via [`crate::scene::BlockSizing::resolve_height`] at
/// the call site — this function is pure geometry, it never resolves a
/// height itself), anchored horizontally per [`IslandAnchor`] within
/// `region_rect`, at `cursor_y` (frame-relative — same coordinate space as
/// every other placed rect in this crate).
pub(crate) fn island_placement_rect(island: &AnchoredIsland<'_>, region_rect: Rect, cursor_y: f64, height: f64) -> Rect {
    let x = match island.anchor {
        IslandAnchor::Left => region_rect.x,
        IslandAnchor::Right => region_rect.x + region_rect.width - island.width,
        IslandAnchor::Center => region_rect.x + (region_rect.width - island.width) / 2.0,
    };
    Rect::new(x, cursor_y, island.width, height)
}

/// The side-strip region(s) text fills while this island's own vertical
/// band `[island_rect.y, band_bottom)` is active — `Left` anchor -> ONE
/// strip to the RIGHT of the image; `Right` anchor -> ONE strip to the
/// LEFT; `Center` -> the LEFT strip THEN the right strip, in that order,
/// both equal width: `(region_width - island.width - 2*margin) / 2` (this
/// feature's own brief, verbatim) — equal width matters: a paragraph long
/// enough to outgrow the left strip can continue cleanly into the right
/// one at the SAME width, reusing the exact cross-region continuation
/// mechanism [`super::flow`] already has for a paragraph spanning ordinary
/// page/column boundaries, no special-casing needed.
///
/// Degenerate strips (non-positive width or height — an island wider than
/// its own region, or a zero-height band) are simply OMITTED from the
/// returned list rather than handed to the caller to force content into a
/// strip that cannot actually hold anything.
pub(crate) fn island_strip_rects(island: &AnchoredIsland<'_>, region_rect: Rect, island_rect: Rect, band_bottom: f64) -> Vec<Rect> {
    let top = island_rect.y;
    let height = (band_bottom - top).max(0.0);
    let margin = island.margin;

    let candidates: Vec<Rect> = match island.anchor {
        IslandAnchor::Left => {
            let x = island_rect.x + island_rect.width + margin;
            let width = (region_rect.x + region_rect.width - x).max(0.0);
            vec![Rect::new(x, top, width, height)]
        }
        IslandAnchor::Right => {
            let width = (island_rect.x - margin - region_rect.x).max(0.0);
            vec![Rect::new(region_rect.x, top, width, height)]
        }
        IslandAnchor::Center => {
            let side_width = ((region_rect.width - island.width - 2.0 * margin) / 2.0).max(0.0);
            let left = Rect::new(region_rect.x, top, side_width, height);
            let right_x = island_rect.x + island_rect.width + margin;
            let right = Rect::new(right_x, top, side_width, height);
            vec![left, right]
        }
    };

    candidates.into_iter().filter(|r| r.width > 0.0 && r.height > 0.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_anchor_places_the_image_at_the_region_left_edge_with_one_right_strip() {
        let region = Rect::new(0.0, 0.0, 500.0, 300.0);
        let image = AnchoredIsland::new(
            crate::scene::ImageBlock::new(&[], 1, 1, crate::scene::BlockSizing::FixedHeight(100.0), crate::scene::ImageFit::Cover),
            IslandAnchor::Left,
            200.0,
            10.0,
        );

        let rect = island_placement_rect(&image, region, 20.0, 100.0);
        assert_eq!(rect, Rect::new(0.0, 20.0, 200.0, 100.0));

        let strips = island_strip_rects(&image, region, rect, 120.0);
        assert_eq!(strips.len(), 1, "Left anchor must yield exactly one strip");
        assert_eq!(strips[0], Rect::new(210.0, 20.0, 290.0, 100.0));
    }

    #[test]
    fn right_anchor_places_the_image_at_the_region_right_edge_with_one_left_strip() {
        let region = Rect::new(0.0, 0.0, 500.0, 300.0);
        let image = AnchoredIsland::new(
            crate::scene::ImageBlock::new(&[], 1, 1, crate::scene::BlockSizing::FixedHeight(100.0), crate::scene::ImageFit::Cover),
            IslandAnchor::Right,
            200.0,
            10.0,
        );

        let rect = island_placement_rect(&image, region, 20.0, 100.0);
        assert_eq!(rect, Rect::new(300.0, 20.0, 200.0, 100.0));

        let strips = island_strip_rects(&image, region, rect, 120.0);
        assert_eq!(strips.len(), 1, "Right anchor must yield exactly one strip");
        assert_eq!(strips[0], Rect::new(0.0, 20.0, 290.0, 100.0));
    }

    /// Center anchor's own side-strip width formula, verbatim from this
    /// feature's own brief: `(body - island - 2*margin) / 2`.
    #[test]
    fn center_anchor_yields_two_equal_width_strips_matching_the_briefs_own_formula() {
        let region = Rect::new(0.0, 0.0, 500.0, 300.0);
        let image = AnchoredIsland::new(
            crate::scene::ImageBlock::new(&[], 1, 1, crate::scene::BlockSizing::FixedHeight(100.0), crate::scene::ImageFit::Cover),
            IslandAnchor::Center,
            200.0,
            10.0,
        );

        let rect = island_placement_rect(&image, region, 20.0, 100.0);
        assert_eq!(rect, Rect::new(150.0, 20.0, 200.0, 100.0), "a centered island sits at (region_width - width) / 2");

        let strips = island_strip_rects(&image, region, rect, 120.0);
        assert_eq!(strips.len(), 2, "Center anchor must yield LEFT then RIGHT strips");

        let expected_side_width = (500.0 - 200.0 - 2.0 * 10.0) / 2.0;
        assert_eq!(strips[0].width, expected_side_width);
        assert_eq!(strips[1].width, expected_side_width);
        assert_eq!(strips[0].x, 0.0, "left strip starts at the region's own left edge");
        assert_eq!(strips[1].x, rect.x + rect.width + 10.0, "right strip starts one margin past the island's own right edge");

        // Neither strip overlaps the island rect itself.
        assert!(strips[0].x + strips[0].width <= rect.x + 1e-9);
        assert!(strips[1].x >= rect.x + rect.width - 1e-9);
    }

    #[test]
    fn degenerate_strips_are_omitted_not_force_included() {
        let region = Rect::new(0.0, 0.0, 210.0, 300.0);
        // Island nearly as wide as the whole region — Left anchor's own
        // right strip would be negative-width without the guard.
        let image = AnchoredIsland::new(
            crate::scene::ImageBlock::new(&[], 1, 1, crate::scene::BlockSizing::FixedHeight(100.0), crate::scene::ImageFit::Cover),
            IslandAnchor::Left,
            220.0,
            10.0,
        );
        let rect = island_placement_rect(&image, region, 0.0, 100.0);
        let strips = island_strip_rects(&image, region, rect, 100.0);
        assert!(strips.is_empty(), "a degenerate (non-positive width) strip must be omitted, never forced");
    }
}
