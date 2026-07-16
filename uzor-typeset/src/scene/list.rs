//! [`ListBlock`] — marker + indent list (design doc §3.6).
//!
//! "No new layout primitive: each item becomes one flow child with a
//! leading reserved marker gutter ahead of its content" (§3.6). P1 scope
//! narrowing (report, not silent): a `ListBlock` is composed as ONE
//! ATOMIC unit this phase (never split mid-list across a region
//! boundary) rather than literally flow-splitting at item granularity —
//! the design doc's own P1 gates/tests (§7) never test a list spanning
//! multiple regions, and no P1 consumer needs one; per-item splitting is
//! deferred to whenever a real long-list-across-pages consumer needs it,
//! the same "genuinely untestable before its own consumer exists"
//! reasoning P0 already used to defer `CardRegionSequence` (see this
//! crate's `CLAUDE.md`).
//!
//! Marker text paints using `ComposeStyle::default_font` — a real P1
//! consumer for that field, which P0 declared reserved-but-unread.

use crate::scene::block::BlockNode;

/// How each item's leading marker renders.
#[derive(Debug, Clone, PartialEq)]
pub enum MarkerStyle {
    /// The same character before every item (e.g. `'•'`).
    Bullet(char),
    /// `"{n}."`-formatted, `start`-based, incrementing by item index.
    Numbered { start: u32 },
    /// No marker glyph painted — the indent gutter is still reserved
    /// (a plain indented list), so item content lines up identically to
    /// a bulleted/numbered sibling.
    None,
}

/// One list item — content is `&'a [BlockNode<'a>]` (paragraphs this
/// phase, same "cell content" scoping `TableCell` uses).
pub struct ListItem<'a> {
    pub content: &'a [BlockNode<'a>],
}

impl<'a> ListItem<'a> {
    pub fn new(content: &'a [BlockNode<'a>]) -> Self {
        Self { content }
    }
}

/// A flow-participating list: `indent_px`-wide marker gutter ahead of
/// every item's own content.
pub struct ListBlock<'a> {
    pub items: &'a [ListItem<'a>],
    pub marker: MarkerStyle,
    pub indent_px: f64,
}

impl<'a> ListBlock<'a> {
    pub fn new(items: &'a [ListItem<'a>], marker: MarkerStyle, indent_px: f64) -> Self {
        Self { items, marker, indent_px }
    }

    /// The marker text for item `index` (0-based) under this list's
    /// [`MarkerStyle`] — `""` for [`MarkerStyle::None`].
    pub fn marker_text(&self, index: usize) -> String {
        match &self.marker {
            MarkerStyle::Bullet(ch) => ch.to_string(),
            MarkerStyle::Numbered { start } => format!("{}.", *start + index as u32),
            MarkerStyle::None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_marker_text_is_the_same_character_for_every_item() {
        let list = ListBlock::new(&[], MarkerStyle::Bullet('•'), 20.0);
        assert_eq!(list.marker_text(0), "•");
        assert_eq!(list.marker_text(3), "•");
    }

    #[test]
    fn numbered_marker_text_increments_from_start() {
        let list = ListBlock::new(&[], MarkerStyle::Numbered { start: 5 }, 24.0);
        assert_eq!(list.marker_text(0), "5.");
        assert_eq!(list.marker_text(2), "7.");
    }

    #[test]
    fn none_marker_text_is_empty() {
        let list = ListBlock::new(&[], MarkerStyle::None, 24.0);
        assert_eq!(list.marker_text(0), "");
    }
}
