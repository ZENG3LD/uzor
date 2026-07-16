//! Typed input model.
//!
//! [`FontSpec`] (Phase 1, single font/plain-text entry) plus the Phase 2
//! rich-span model: [`StyledRun`]/[`Paragraph`]/[`ParagraphAlign`] and
//! [`InlineBox`]/[`InlineBoxKind`]/[`InlineBoxSlot`] — see the design
//! doc's crate-architecture section for the full module layout, and this
//! crate's `CLAUDE.md` for what Phase 2 actually built vs. deferred.

mod font_spec;
mod inline_box;
mod span;

pub use font_spec::FontSpec;
pub use inline_box::{InlineBox, InlineBoxKind, InlineBoxSlot};
pub use span::{Paragraph, ParagraphAlign, StyledRun};
