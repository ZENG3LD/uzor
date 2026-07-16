//! [`place_list`] — marker + indent list layout (design doc §3.6).
//!
//! P1 scope narrowing (see `scene::list`'s own module docs): a list is
//! composed as ONE atomic unit — `place_list` is called fresh every time
//! `compose::flow` attempts to place a `Block::List` node (no cross-region
//! continuation cache, unlike `Block::Paragraph`/`Block::Table`), since an
//! atomic block by definition never needs one.

use uzor::types::Rect;
use uzor_text::LineShaper;

use super::flow::compose;
use super::ComposeStyle;
use crate::region::{FixedRegionSequence, PlacedBlock, PlacedListItem};
use crate::scene::ListBlock;

/// Lay out every item of `list` at `origin.width` (minus the marker
/// gutter), stacking top-to-bottom starting at `(origin.x, origin.y)`.
/// Returns the placed items (frame-relative, ready to paint verbatim) and
/// the list's own total content height (marker-gutter-independent — the
/// gutter never adds to height, only to each item's own left indent).
///
/// Marker text paints using `style.default_font` — see `scene::list`'s
/// module docs for why no separate marker-font field exists on
/// [`ListBlock`] itself.
pub(crate) fn place_list<'a>(list: &'a ListBlock<'a>, origin: Rect, style: &ComposeStyle, shaper: &dyn LineShaper) -> (Vec<PlacedListItem<'a>>, f64) {
    let content_width = (origin.width - list.indent_px).max(0.0);
    let mut items = Vec::with_capacity(list.items.len());
    let mut cursor_y = 0.0_f64;

    for (idx, item) in list.items.iter().enumerate() {
        let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, content_width, f64::MAX));
        let mut frames = compose(item.content, &mut regions, style, shaper);
        let mut content: Vec<PlacedBlock<'a>> = frames.pop().map(|f| f.blocks).unwrap_or_default();
        let item_height = content.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);

        for placed in &mut content {
            placed.translate(origin.x + list.indent_px, origin.y + cursor_y);
        }

        let marker_text = list.marker_text(idx);
        let marker_height = item_height.max(style.default_font.size_px);
        let marker_rect = Rect::new(origin.x, origin.y + cursor_y, list.indent_px, marker_height);

        items.push(PlacedListItem { marker_rect, marker_text, marker_font: style.default_font, content });

        cursor_y += item_height;
        if idx + 1 < list.items.len() {
            cursor_y += style.paragraph_spacing;
        }
    }

    (items, cursor_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, Paragraph, StyledRun};

    use crate::scene::{Block, BlockNode, ListItem, MarkerStyle};

    fn style() -> ComposeStyle {
        ComposeStyle::new(6.0, FontSpec::new(FontFamily::Roboto, 16.0))
    }

    #[test]
    fn each_items_content_is_indented_past_the_marker_gutter() {
        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let run_a = [StyledRun::new("first item", font)];
        let run_b = [StyledRun::new("second item", font)];
        let nodes_a = [BlockNode::new(Block::Paragraph(Paragraph::new(&run_a, 200.0)))];
        let nodes_b = [BlockNode::new(Block::Paragraph(Paragraph::new(&run_b, 200.0)))];
        let items = [ListItem::new(&nodes_a), ListItem::new(&nodes_b)];

        const INDENT: f64 = 24.0;
        let list = ListBlock::new(&items, MarkerStyle::Bullet('•'), INDENT);
        let origin = Rect::new(10.0, 5.0, 300.0, f64::MAX);

        let (placed, total_height) = place_list(&list, origin, &style(), &shaper);
        assert_eq!(placed.len(), 2);
        assert!(total_height > 0.0);

        for item in &placed {
            assert_eq!(item.marker_rect.x, origin.x, "marker sits at the list's own left edge");
            let content_block = item.content.first().expect("each item has one paragraph block");
            assert_eq!(content_block.rect.x, origin.x + INDENT, "item content must indent past the marker gutter exactly");
        }
    }

    #[test]
    fn markers_are_numbered_in_order_when_the_list_is_numbered() {
        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let run = [StyledRun::new("item text", font)];
        let nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, 200.0)))];
        let items = [ListItem::new(&nodes), ListItem::new(&nodes), ListItem::new(&nodes)];
        let list = ListBlock::new(&items, MarkerStyle::Numbered { start: 1 }, 24.0);
        let origin = Rect::new(0.0, 0.0, 300.0, f64::MAX);

        let (placed, _) = place_list(&list, origin, &style(), &shaper);
        let markers: Vec<&str> = placed.iter().map(|p| p.marker_text.as_str()).collect();
        assert_eq!(markers, vec!["1.", "2.", "3."]);
    }
}
