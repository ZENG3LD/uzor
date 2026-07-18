//! [`Footnote`] — one footnote's own body content (typography-gap WAVE 3).
//!
//! A footnote is spliced into a [`crate::scene::Block::Paragraph`] via an
//! INLINE MARKER, not a new [`crate::scene::Block`] variant: the author
//! writes the visible superscript digit as an ordinary
//! `uzor_text::StyledRun` with `vertical_align: uzor_text::VerticalAlign::
//! Super` (wave-2 machinery, reused verbatim per this feature's own brief),
//! then splices a zero-size `uzor_text::InlineBox::out_of_flow(local_index)`
//! at the SAME byte offset — `local_index` being this footnote's own
//! 0-based position within [`crate::scene::BlockNode::footnotes`]. The
//! `InlineBox` contributes NOTHING to wrap/advance (see that type's own doc
//! comment); its only job is recoverability: once a paragraph is placed
//! into a page, `crate::slice::pages` reads `ParagraphLayout::boxes` back
//! off the placed layout to learn WHICH page each footnote marker actually
//! landed on (the same `InlineBox`-as-correlation-token convention
//! `crate::toc`'s own indent spacer already established, applied here to
//! recover page ASSIGNMENT instead of a paint position).
//!
//! This is a documented authoring convention (like `crate::toc`'s own
//! leading-`InlineBox`-not-literal-whitespace rule) — not automatically
//! wired the way [`crate::scene::BlockNode::with_outline`] is, because a
//! footnote marker's position is inherently mid-run, not a whole-block
//! property.

use crate::scene::block::BlockNode;

/// One footnote's own body content — `&'a [BlockNode<'a>]`, the same "cell
/// content" scoping [`crate::scene::TableCell`]/[`crate::scene::ListItem`]
/// already use. No `number` field: like [`crate::scene::OutlineTag`], the
/// resolved number is a property of WHERE this footnote sits in document
/// order, computed by [`crate::slice::pages`] at slice time, never
/// authored.
pub struct Footnote<'a> {
    pub content: &'a [BlockNode<'a>],
}

impl<'a> Footnote<'a> {
    pub fn new(content: &'a [BlockNode<'a>]) -> Self {
        Self { content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{FontSpec, Paragraph, StyledRun};

    use crate::scene::block::Block;

    #[test]
    fn footnote_carries_its_content_verbatim() {
        let font = FontSpec::new(FontFamily::Roboto, 12.0);
        let runs = [StyledRun::new("a footnote body", font)];
        let nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&runs, 200.0)))];
        let footnote = Footnote::new(&nodes);
        assert_eq!(footnote.content.len(), 1);
    }
}
