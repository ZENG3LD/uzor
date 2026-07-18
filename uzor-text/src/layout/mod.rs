//! Layout output model + Phase 1/2 entry points.

mod baseline;
mod glyph_layout;
pub(crate) mod greedy;
mod paragraph;

pub use glyph_layout::{align_lines, layout_text, Align, DecorationKind, DecorationSpan, GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};
pub use paragraph::layout_paragraph;
