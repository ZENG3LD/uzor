//! [`SlideMaster`]/[`SlideLayout`] — the slide-side sibling of
//! [`super::PageMaster`] (design doc §5). Neither is wired into a slicing
//! pipeline yet — `slice_slides` is a P3 deliverable (design doc §7) —
//! this phase builds the real, tested TYPE (placeholder declaration +
//! arrangement + instance-delta fill/re-map), the same "type now, slicing
//! wiring lands with its own consumer phase" convention P1 already used
//! for `FixedRegionSequence` (built P1, ahead of its OWN eventual P3
//! slide-mode consumer) — see this crate's `CLAUDE.md`.
//!
//! ## Instance delta model
//!
//! A [`SlideInstance`] stores ONLY which [`PlaceholderSlot`]s are filled
//! with which [`BlockNode`]s — never a full copy of its layout's own
//! content (design doc §5: "restyle 100 slides by editing one master").
//! [`SlideInstance::arrange`] re-maps fills against ANY [`SlideLayout`] by
//! `(kind, index)`, not by raw block id, so swapping a slide's layout is a
//! pure re-arrangement of the SAME content (design doc §5's own
//! `LayoutPlaceholderIdMapping` reference). A layout slot with no
//! matching fill is simply ABSENT from `arrange`'s output — this task's
//! own chosen "unfilled" rule (the design doc doesn't specify one; see
//! this crate's `CLAUDE.md` divergence log for why "skip," not
//! "prompt-text," was chosen: a `Chart`/`Media` placeholder has no
//! sensible TEXT-shaped prompt, and "ANY block kind may fill a slot"
//! already rules out one generic filler shape working for every kind).

use uzor::types::Rect;

use super::placeholder::PlaceholderSlot;
use crate::scene::{Block, BlockNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MasterId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutId(pub u32);

/// A named template: which placeholder slots exist + a shared background,
/// independent of any one layout's own arrangement of those slots.
pub struct SlideMaster<'a> {
    pub id: MasterId,
    pub placeholders: Vec<PlaceholderSlot>,
    pub background: Option<Block<'a>>,
}

impl<'a> SlideMaster<'a> {
    pub fn new(id: MasterId, placeholders: Vec<PlaceholderSlot>, background: Option<Block<'a>>) -> Self {
        Self { id, placeholders, background }
    }
}

/// One concrete arrangement of a [`SlideMaster`]'s declared slots — each
/// slot mapped to a fixed rect within the slide.
pub struct SlideLayout {
    pub id: LayoutId,
    pub master: MasterId,
    pub arrangement: Vec<(PlaceholderSlot, Rect)>,
}

impl SlideLayout {
    pub fn new(id: LayoutId, master: MasterId, arrangement: Vec<(PlaceholderSlot, Rect)>) -> Self {
        Self { id, master, arrangement }
    }

    /// This layout's own rect for `slot`, if it declares one.
    pub fn rect_for(&self, slot: PlaceholderSlot) -> Option<Rect> {
        self.arrangement.iter().find(|(s, _)| *s == slot).map(|(_, rect)| *rect)
    }
}

/// One placeholder fill — an instance's own delta entry.
pub struct PlaceholderFill<'a> {
    pub slot: PlaceholderSlot,
    pub content: BlockNode<'a>,
}

impl<'a> PlaceholderFill<'a> {
    pub fn new(slot: PlaceholderSlot, content: BlockNode<'a>) -> Self {
        Self { slot, content }
    }
}

/// A slide instance's own delta: which layout it currently uses + which
/// slots it fills (design doc §5: "a Slide stores only its own delta").
pub struct SlideInstance<'a> {
    pub layout: LayoutId,
    pub fills: Vec<PlaceholderFill<'a>>,
}

impl<'a> SlideInstance<'a> {
    pub fn new(layout: LayoutId, fills: Vec<PlaceholderFill<'a>>) -> Self {
        Self { layout, fills }
    }

    /// This instance's own fill for `slot`, if any.
    pub fn fill_for(&self, slot: PlaceholderSlot) -> Option<&BlockNode<'a>> {
        self.fills.iter().find(|f| f.slot == slot).map(|f| &f.content)
    }

    /// Re-map this instance's SAME fills against `layout` by `(kind,
    /// index)` — content is untouched, only which rect it lands in
    /// changes (design doc §5). A `layout` slot with no matching fill is
    /// skipped (this module's own doc comment).
    pub fn arrange<'s>(&'s self, layout: &SlideLayout) -> Vec<PlacedPlaceholder<'s, 'a>> {
        layout
            .arrangement
            .iter()
            .filter_map(|(slot, rect)| self.fill_for(*slot).map(|content| PlacedPlaceholder { slot: *slot, rect: *rect, content }))
            .collect()
    }
}

/// One instance fill, arranged against a specific [`SlideLayout`].
pub struct PlacedPlaceholder<'s, 'a> {
    pub slot: PlaceholderSlot,
    pub rect: Rect,
    pub content: &'s BlockNode<'a>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{FontSpec, Paragraph, StyledRun};

    use crate::master::PlaceholderKind;
    use crate::scene::Block;

    fn title_slot() -> PlaceholderSlot {
        PlaceholderSlot::new(PlaceholderKind::Title, 0)
    }
    fn body_slot() -> PlaceholderSlot {
        PlaceholderSlot::new(PlaceholderKind::Body, 0)
    }
    fn media_slot() -> PlaceholderSlot {
        PlaceholderSlot::new(PlaceholderKind::Media, 0)
    }

    #[test]
    fn swapping_the_layout_remaps_rects_by_kind_and_index_while_content_survives() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let title_run = [StyledRun::new("Deck Title", font)];
        let body_run = [StyledRun::new("Deck body copy.", font)];

        let fills = vec![
            PlaceholderFill::new(title_slot(), BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, 400.0)))),
            PlaceholderFill::new(body_slot(), BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, 400.0)))),
        ];
        let instance = SlideInstance::new(LayoutId(1), fills);

        let layout_a = SlideLayout::new(
            LayoutId(1),
            MasterId(1),
            vec![(title_slot(), Rect::new(0.0, 0.0, 400.0, 60.0)), (body_slot(), Rect::new(0.0, 60.0, 400.0, 200.0))],
        );
        let layout_b = SlideLayout::new(
            LayoutId(2),
            MasterId(1),
            vec![(body_slot(), Rect::new(0.0, 0.0, 400.0, 200.0)), (title_slot(), Rect::new(0.0, 200.0, 400.0, 60.0))],
        );

        let arranged_a = instance.arrange(&layout_a);
        let arranged_b = instance.arrange(&layout_b);

        assert_eq!(arranged_a.len(), 2);
        assert_eq!(arranged_b.len(), 2);

        let title_rect_a = arranged_a.iter().find(|p| p.slot == title_slot()).expect("title present under layout A").rect;
        let title_rect_b = arranged_b.iter().find(|p| p.slot == title_slot()).expect("title present under layout B").rect;
        assert_ne!(title_rect_a, title_rect_b, "swapping layouts must re-map the rect");

        let content_a = arranged_a.iter().find(|p| p.slot == title_slot()).expect("title present").content;
        let content_b = arranged_b.iter().find(|p| p.slot == title_slot()).expect("title present").content;
        assert!(matches!(content_a.kind, Block::Paragraph(p) if p.runs[0].text == "Deck Title"), "content itself must survive the swap verbatim");
        assert!(matches!(content_b.kind, Block::Paragraph(p) if p.runs[0].text == "Deck Title"));
    }

    #[test]
    fn a_layout_slot_with_no_matching_fill_is_skipped_not_a_placeholder_stub() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let title_run = [StyledRun::new("Only the title is filled", font)];
        let fills = vec![PlaceholderFill::new(title_slot(), BlockNode::new(Block::Paragraph(Paragraph::new(&title_run, 400.0))))];
        let instance = SlideInstance::new(LayoutId(1), fills);

        // Layout declares THREE slots; the instance only fills one.
        let layout = SlideLayout::new(
            LayoutId(1),
            MasterId(1),
            vec![
                (title_slot(), Rect::new(0.0, 0.0, 400.0, 60.0)),
                (body_slot(), Rect::new(0.0, 60.0, 400.0, 200.0)),
                (media_slot(), Rect::new(0.0, 260.0, 400.0, 100.0)),
            ],
        );

        let arranged = instance.arrange(&layout);
        assert_eq!(arranged.len(), 1, "unfilled slots must be skipped entirely, never a stub entry");
        assert_eq!(arranged[0].slot, title_slot());
    }

    #[test]
    fn fill_for_finds_the_right_slot_among_several() {
        let font = FontSpec::new(FontFamily::Roboto, 16.0);
        let run = [StyledRun::new("Body copy", font)];
        let fills = vec![PlaceholderFill::new(body_slot(), BlockNode::new(Block::Paragraph(Paragraph::new(&run, 400.0))))];
        let instance = SlideInstance::new(LayoutId(1), fills);

        assert!(instance.fill_for(body_slot()).is_some());
        assert!(instance.fill_for(title_slot()).is_none());
    }
}
