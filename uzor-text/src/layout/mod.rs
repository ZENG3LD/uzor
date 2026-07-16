//! Layout output model + Phase 1/2 entry points.

mod baseline;
mod glyph_layout;
mod greedy;
mod paragraph;

pub use glyph_layout::{align_lines, layout_text, Align, GlyphLayout, LineBox, ParagraphLayout, PlacedInlineBox};
pub use paragraph::layout_paragraph;
