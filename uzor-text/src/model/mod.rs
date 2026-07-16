//! Typed input model.
//!
//! Phase 1 only ships [`FontSpec`] (plain-text entry, one font per
//! paragraph). Rich spans (`StyledRun`, `Paragraph`, `ParagraphAlign`) and
//! `InlineBox` are Phase 2 — see the design doc's crate-architecture
//! section for the full module layout.

mod font_spec;

pub use font_spec::FontSpec;
