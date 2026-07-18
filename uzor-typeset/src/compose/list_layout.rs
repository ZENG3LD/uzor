//! [`measure_list_items`]/[`place_list_items`] — marker + indent list layout
//! (design doc §3.6), item-atomic splitting across regions (typography
//! quality wave — an item never splits mid-item this pass, but the list AS
//! A WHOLE splits BETWEEN items across regions, numbering continuing
//! across fragments).
//!
//! Mirrors `compose::table_layout`'s own "measure every item once, at the
//! width of the region the list first entered, cache across region
//! continuations, then translate a SLICE into place per fragment" shape
//! (design law 1: no second position formula) — a list now splits BETWEEN
//! items exactly like a table splits BETWEEN rows. [`ComposedListItem`]'s
//! own `blocks` are positioned relative to the item's own LOCAL `(0, 0)`
//! origin (same convention `compose::table_layout::ComposedCell` uses),
//! translated into frame-relative coordinates only when a slice of items
//! is actually placed (see [`place_list_items`]).
//!
//! Superseded P1 scope narrowing (see this crate's `CLAUDE.md`): a list
//! used to be composed as ONE atomic unit; this pass closes that gap per
//! the design doc's own §3.6 phrasing ("each item becomes one flow
//! child") — a long list spanning multiple regions is now a REAL,
//! per-item-split consumer.

use uzor::types::Rect;
use uzor_text::{FontSpec, LineShaper};

use super::flow::compose;
use super::ComposeStyle;
use crate::region::{FixedRegionSequence, PlacedBlock, PlacedListItem};
use crate::scene::ListBlock;

/// One item's already-composed content, measured at its own LOCAL `(0, 0)`
/// origin — cached across region continuations exactly like
/// `compose::table_layout::ComposedRow`.
pub(crate) struct ComposedListItem<'a> {
    /// This item's own content height (marker-gutter-independent) — the
    /// value every cross-region cursor/budget computation
    /// ([`items_fitting`]/[`list_total_height`]/[`place_list_items`])
    /// advances by. Never clamped to the marker font's own size (an empty
    /// item still advances the cursor by `0.0`).
    pub content_height: f64,
    /// This item's own marker RECT height — `content_height` clamped up
    /// to at least the marker font's own point size, so a marker glyph
    /// always has room to paint even beside a very short (or empty) item.
    /// Never used for cursor/budget math (see `content_height`).
    pub marker_height: f64,
    pub marker_text: String,
    pub marker_font: FontSpec,
    pub blocks: Vec<PlacedBlock<'a>>,
}

/// Measure + lay out every item of `list` at `content_width` (the list's
/// own width MINUS its marker gutter) — the ONE pass `compose::flow` runs
/// when it first encounters a `Block::List` node, cached across every
/// region the list subsequently spans (same "measured once, at the width
/// of the region it first entered" convention `compose::table_layout`
/// already established for `Block::Table`). Marker text is resolved here
/// too (via `list.marker_text(idx)`, using the item's GLOBAL index within
/// the whole list) — a fragment placed later via [`place_list_items`]
/// therefore always carries the CORRECT continuing marker (`5.`, `6.`,
/// ...), never restarting from the fragment's own local index `0`.
pub(crate) fn measure_list_items<'a>(list: &'a ListBlock<'a>, content_width: f64, style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<ComposedListItem<'a>> {
    list.items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, content_width, f64::MAX));
            let mut frames = compose(item.content, &mut regions, style, shaper);
            let blocks: Vec<PlacedBlock<'a>> = frames.pop().map(|f| f.blocks).unwrap_or_default();
            let content_height = blocks.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);
            let marker_height = content_height.max(style.default_font.size_px);
            ComposedListItem { content_height, marker_height, marker_text: list.marker_text(idx), marker_font: style.default_font, blocks }
        })
        .collect()
}

/// How many of `items`, starting at `from_item`, fit within
/// `remaining_height` — item-atomic splitting's own version of
/// `compose::lines_fitting`/`compose::table_layout::rows_fitting`, same
/// force-at-least-one-item-on-a-fresh-region degrade convention. `gap` (the
/// inter-item spacing, [`ComposeStyle::paragraph_spacing`]) is only ever
/// counted BETWEEN two items kept in the SAME fragment — never a trailing
/// gap after the last item of a fragment that continues, matching how
/// `ComposeStyle::paragraph_spacing` is never added after a block that
/// continues into the next region (`compose::flow`'s own convention).
pub(crate) fn items_fitting(items: &[ComposedListItem<'_>], from_item: usize, remaining_height: f64, gap: f64, force_at_least_one: bool) -> usize {
    let mut used = 0.0_f64;
    let mut count = 0usize;

    for (i, item) in items[from_item..].iter().enumerate() {
        let extra = if i == 0 { item.content_height } else { gap + item.content_height };
        let next = used + extra;
        if next > remaining_height {
            break;
        }
        used = next;
        count += 1;
    }

    if count == 0 && force_at_least_one && from_item < items.len() {
        count = 1;
    }
    count
}

/// This list's total height across EVERY item (never just what fits one
/// region) — used only by `AvoidInside`'s "would the whole list fit a
/// fresh region" pre-check and the `AvoidAfter` keep-with-next lookahead,
/// never by real per-region placement (which always goes through
/// [`items_fitting`] instead). Mirrors
/// `compose::table_layout::table_total_height`.
pub(crate) fn list_total_height(items: &[ComposedListItem<'_>], gap: f64) -> f64 {
    let mut total = 0.0_f64;
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            total += gap;
        }
        total += item.content_height;
    }
    total
}

/// Translate items `[from_item, from_item + count)` of the cached `items`
/// into frame-relative [`PlacedListItem`]s, stacked starting at `origin`
/// (`origin.0` = the list's own left edge, `origin.1` = the cursor `y`
/// this placement starts at) — the SAME item content [`measure_list_items`]
/// already computed, just shifted into place (design law 1). Returns the
/// placed items plus this placement's own total height (gap-aware, never
/// a trailing gap after the LAST item in this slice).
pub(crate) fn place_list_items<'a>(
    items: &[ComposedListItem<'a>],
    from_item: usize,
    count: usize,
    indent_px: f64,
    gap: f64,
    origin: (f64, f64),
) -> (Vec<PlacedListItem<'a>>, f64) {
    let (origin_x, origin_y) = origin;
    let mut placed = Vec::with_capacity(count);
    let mut cursor_y = 0.0_f64;

    for (i, item) in items[from_item..from_item + count].iter().enumerate() {
        if i > 0 {
            cursor_y += gap;
        }
        let marker_rect = Rect::new(origin_x, origin_y + cursor_y, indent_px, item.marker_height);
        let mut content: Vec<PlacedBlock<'a>> = item.blocks.clone();
        for placed_block in &mut content {
            placed_block.translate(origin_x + indent_px, origin_y + cursor_y);
        }
        placed.push(PlacedListItem { marker_rect, marker_text: item.marker_text.clone(), marker_font: item.marker_font, content });
        cursor_y += item.content_height;
    }

    (placed, cursor_y)
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
        let style = style();
        let content_width = 300.0 - INDENT;
        let composed = measure_list_items(&list, content_width, &style, &shaper);
        let (placed, total_height) = place_list_items(&composed, 0, composed.len(), INDENT, style.paragraph_spacing, (10.0, 5.0));

        assert_eq!(placed.len(), 2);
        assert!(total_height > 0.0);

        for item in &placed {
            assert_eq!(item.marker_rect.x, 10.0, "marker sits at the list's own left edge");
            let content_block = item.content.first().expect("each item has one paragraph block");
            assert_eq!(content_block.rect.x, 10.0 + INDENT, "item content must indent past the marker gutter exactly");
        }
    }

    #[test]
    fn markers_are_numbered_in_order_when_the_list_is_numbered() {
        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let run = [StyledRun::new("item text", font)];
        let nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, 200.0)))];
        let items = [ListItem::new(&nodes), ListItem::new(&nodes), ListItem::new(&nodes)];
        let list = ListBlock::new(&items, MarkerStyle::numbered(1), 24.0);
        let style = style();

        let composed = measure_list_items(&list, 300.0, &style, &shaper);
        let (placed, _) = place_list_items(&composed, 0, composed.len(), 24.0, style.paragraph_spacing, (0.0, 0.0));
        let markers: Vec<&str> = placed.iter().map(|p| p.marker_text.as_str()).collect();
        assert_eq!(markers, vec!["1.", "2.", "3."]);
    }

    #[test]
    fn items_fitting_matches_the_table_rows_fitting_budget_convention() {
        let items = [
            ComposedListItem { content_height: 10.0, marker_height: 10.0, marker_text: String::new(), marker_font: FontSpec::default(), blocks: vec![] },
            ComposedListItem { content_height: 10.0, marker_height: 10.0, marker_text: String::new(), marker_font: FontSpec::default(), blocks: vec![] },
            ComposedListItem { content_height: 10.0, marker_height: 10.0, marker_text: String::new(), marker_font: FontSpec::default(), blocks: vec![] },
        ];
        // No gap: identical arithmetic to `rows_fitting`.
        assert_eq!(items_fitting(&items, 0, 25.0, 0.0, false), 2, "2.5 item-heights of budget fits exactly 2 whole items");
        assert_eq!(items_fitting(&items, 0, 5.0, 0.0, false), 0, "nothing fits and the region already has content: defer whole");
        assert_eq!(items_fitting(&items, 0, 5.0, 0.0, true), 1, "a fresh region must make progress even if the item overflows it");
    }

    #[test]
    fn items_fitting_counts_the_gap_only_between_items_kept_in_the_same_fragment() {
        let items = [
            ComposedListItem { content_height: 10.0, marker_height: 10.0, marker_text: String::new(), marker_font: FontSpec::default(), blocks: vec![] },
            ComposedListItem { content_height: 10.0, marker_height: 10.0, marker_text: String::new(), marker_font: FontSpec::default(), blocks: vec![] },
        ];
        // Item 1 alone: budget = 10.0 exactly fits (no gap charged before
        // the FIRST item in the slice).
        assert_eq!(items_fitting(&items, 0, 10.0, 4.0, false), 1);
        // Both items: needs 10 + 4 (gap) + 10 = 24.0.
        assert_eq!(items_fitting(&items, 0, 23.9, 4.0, false), 1, "just short of the second item's gap-inclusive cost");
        assert_eq!(items_fitting(&items, 0, 24.0, 4.0, false), 2, "exactly enough for both items plus the one gap between them");
    }

    /// Splittable lists (typography quality wave): an 8-item numbered list
    /// worth ~1.5 regions must split BETWEEN items (never mid-item), and
    /// the continuation's own markers must continue counting from where
    /// the first fragment left off (`5.`, `6.`, ... — never restarting at
    /// `1.`).
    #[test]
    fn an_eight_item_list_worth_one_and_a_half_regions_splits_between_items_and_markers_continue() {
        use crate::region::PageRegionSequence;
        use crate::scene::BlockNode as SceneBlockNode;

        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let style = ComposeStyle::new(0.0, font);

        let item_run = [StyledRun::new("list item text", font)];
        let item_nodes = [SceneBlockNode::new(Block::Paragraph(Paragraph::new(&item_run, f64::MAX)))];
        let items: Vec<ListItem<'_>> = (0..8).map(|_| ListItem::new(&item_nodes)).collect();
        let list = ListBlock::new(&items, MarkerStyle::numbered(1), 24.0);

        const REGION_W: f64 = 300.0;
        let content_width = REGION_W - list.indent_px;
        let composed = measure_list_items(&list, content_width, &style, &shaper);
        let item_height = composed[0].content_height;
        assert!(composed.iter().all(|it| (it.content_height - item_height).abs() < 1e-6), "fixture items must be equal height");

        // Room for exactly 5 of the 8 items (~1.5 regions' worth of
        // content across the whole list).
        let region_height = item_height * 5.0 + 0.5;

        let flow = [SceneBlockNode::new(Block::List(list))];
        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, REGION_W, region_height));
        let frames = compose(&flow, &mut regions, &style, &shaper);

        assert_eq!(frames.len(), 2, "the list must split across exactly 2 regions");
        assert!(frames[0].overflow.is_some(), "the first frame must report the list as still overflowing");
        assert!(frames[1].overflow.is_none(), "the second frame must finish the list");

        let page1_list = frames[0].blocks[0].list_placement.as_ref().expect("list placement present");
        let page2_list = frames[1].blocks[0].list_placement.as_ref().expect("list placement present");

        assert_eq!(page1_list.items.len(), 5, "page 1 must hold exactly the 5 items that fit whole");
        assert_eq!(page2_list.items.len(), 3, "page 2 must hold the remaining 3 items");
        assert_eq!(page1_list.items.len() + page2_list.items.len(), items.len(), "every item must be conserved exactly once");

        let page1_markers: Vec<&str> = page1_list.items.iter().map(|it| it.marker_text.as_str()).collect();
        let page2_markers: Vec<&str> = page2_list.items.iter().map(|it| it.marker_text.as_str()).collect();
        assert_eq!(page1_markers, vec!["1.", "2.", "3.", "4.", "5."]);
        assert_eq!(page2_markers, vec!["6.", "7.", "8."], "the continuation's markers must continue counting, never restart at 1.");
    }

    /// Nested lists (typography-gap WAVE 3): a list item's own content
    /// containing ANOTHER `Block::List` composes with zero new engine code
    /// (see `scene::list`'s own module doc) — this test proves it as data,
    /// not just by inspection. The nested list uses
    /// [`crate::scene::NumberScheme::LowerRoman`] to also prove a non-decimal
    /// scheme survives a nesting depth.
    #[test]
    fn a_nested_list_indents_past_both_gutters_and_renders_its_own_roman_markers() {
        use crate::region::PageRegionSequence;
        use crate::scene::{BlockNode as SceneBlockNode, NumberScheme};

        let shaper = CosmicShaper::headless();
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let style = ComposeStyle::new(4.0, font);

        const OUTER_INDENT: f64 = 20.0;
        const INNER_INDENT: f64 = 18.0;
        const REGION_W: f64 = 400.0;

        let inner_run_a = [StyledRun::new("nested first", font)];
        let inner_run_b = [StyledRun::new("nested second", font)];
        let inner_nodes_a = [SceneBlockNode::new(Block::Paragraph(Paragraph::new(&inner_run_a, f64::MAX)))];
        let inner_nodes_b = [SceneBlockNode::new(Block::Paragraph(Paragraph::new(&inner_run_b, f64::MAX)))];
        let inner_items = [ListItem::new(&inner_nodes_a), ListItem::new(&inner_nodes_b)];
        let inner_list = ListBlock::new(&inner_items, MarkerStyle::Numbered { start: 1, scheme: NumberScheme::LowerRoman }, INNER_INDENT);
        let inner_flow = [SceneBlockNode::new(Block::List(inner_list))];

        let outer_run_a = [StyledRun::new("outer item one heading", font)];
        let outer_item_a_nodes = [SceneBlockNode::new(Block::Paragraph(Paragraph::new(&outer_run_a, f64::MAX)))];
        let outer_run_b = [StyledRun::new("outer item two", font)];
        let outer_item_b_nodes = [SceneBlockNode::new(Block::Paragraph(Paragraph::new(&outer_run_b, f64::MAX)))];

        // Item 0's own content is [paragraph, nested list]; item 1 is a
        // plain paragraph-only item (proves nesting is per-item, not
        // list-wide).
        let outer_items = [ListItem::new(&outer_item_a_nodes), ListItem::new(&inner_flow), ListItem::new(&outer_item_b_nodes)];
        let outer_list = ListBlock::new(&outer_items, MarkerStyle::Bullet('•'), OUTER_INDENT);
        let flow = [SceneBlockNode::new(Block::List(outer_list))];

        let mut regions = PageRegionSequence::new(Rect::new(0.0, 0.0, REGION_W, 2000.0));
        let frames = compose(&flow, &mut regions, &style, &shaper);
        assert_eq!(frames.len(), 1, "fixture must fit on one page");

        let outer_placement = frames[0].blocks[0].list_placement.as_ref().expect("outer list placement present");
        assert_eq!(outer_placement.items.len(), 3, "every outer item (incl. the nested-list-only one) must be conserved");

        let nested_item = &outer_placement.items[1];
        let nested_block = nested_item.content.first().expect("the nested list itself is item 1's own sole content block");
        assert!(matches!(nested_block.kind, Block::List(_)), "item 1's own content must resolve to a real Block::List placement");
        let inner_placement = nested_block.list_placement.as_ref().expect("nested list must carry its own list_placement");
        assert_eq!(inner_placement.items.len(), 2);

        // Indentation compounds: the OUTER list's own item content starts
        // at `region.x + OUTER_INDENT`; the NESTED list's own item content
        // must start `INNER_INDENT` further right than that — never the
        // same x, never re-based to 0.
        let outer_content_x = nested_item.content[0].rect.x; // the nested Block::List's own top-level rect
        assert!((outer_content_x - OUTER_INDENT).abs() < 1e-6, "the nested list's own top-level rect must sit exactly at the outer gutter");
        let inner_item_x = inner_placement.items[0].content[0].rect.x;
        assert!(
            (inner_item_x - (OUTER_INDENT + INNER_INDENT)).abs() < 1e-6,
            "a doubly-indented item's content must sit at outer + inner indent exactly, got {inner_item_x}"
        );

        // The nested list's own roman markers render correctly at this
        // nesting depth — never silently falling back to decimal.
        let inner_markers: Vec<&str> = inner_placement.items.iter().map(|it| it.marker_text.as_str()).collect();
        assert_eq!(inner_markers, vec!["i.", "ii."]);

        // Sibling outer items (0 and 2, plain paragraphs) are unaffected by
        // the nested list sitting between them — same top-level indent as
        // any ordinary item.
        let sibling_x = outer_placement.items[0].content[0].rect.x;
        assert!((sibling_x - OUTER_INDENT).abs() < 1e-6);
    }
}
