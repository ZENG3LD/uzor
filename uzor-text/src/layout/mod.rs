//! Layout output model + Phase 1 entry point.

mod glyph_layout;

pub use glyph_layout::{align_lines, layout_text, Align, GlyphLayout, LineBox, ParagraphLayout};
