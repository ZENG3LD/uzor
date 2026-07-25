//! `uzor-text` — pretext-pattern text layout engine for uzor (Arc 2).
//!
//! Pure, stateless functions from styled spans + [`FontSpec`] to
//! per-glyph positions ([`ParagraphLayout`]) via a swappable [`LineShaper`]
//! ([`CosmicShaper`] today — cosmic-text, the same substrate
//! `uzor::shaper` already uses for single-line text; parley could swap in
//! later behind the same trait).
//!
//! This crate currently implements **Phase 1 + Phase 2 + Phase 3 + Phase 4
//! + Phase 5** of `nemo/docs/uzor-engines/uzor_text_arc2_design.md`: plain-text
//! greedy word-wrap ([`layout_text`], Phase 1), rich multi-run spans,
//! [`InlineBox`], and a mixed-run baseline pass ([`layout_paragraph`],
//! Phase 2), the resize-morph kinetics ([`kinetics::build_morph`]/
//! [`kinetics::sample`], Phase 3), the ASCII cell-shader mode ([`ascii`],
//! Phase 4 — absorbed verbatim from `uzor` core's
//! `ui::effects::text::cell_shader`, plus the net-new
//! [`ascii::ParagraphAsciiShader`] bridge from a real
//! [`layout::ParagraphLayout`] into an ASCII grid), and English Liang-pattern
//! hyphenation + Knuth-Plass total-fit line breaking
//! ([`linebreak::BreakStrategy`]/[`linebreak::Hyphenation`] on
//! [`Paragraph`], Phase 5, opt-in — [`layout_paragraph`]'s default remains
//! Phase 1/2's greedy packer byte-for-byte) — see this crate's `CLAUDE.md`
//! for exactly what each phase built vs. deferred, and where its
//! implementation diverges from the design doc's own sketch.

pub mod ascii;
pub mod draw;
pub mod kinetics;
pub mod layout;
pub mod linebreak;
pub mod measure;
pub mod model;
pub mod shape;
#[cfg(test)]
mod tnum_audit;

pub use draw::{draw_decorations, draw_layout, draw_paragraph};
pub use kinetics::{build_morph, sample, sample_layout, GlyphState, MorphTransition};
pub use layout::{
    align_lines, layout_paragraph, layout_text, Align, DecorationKind, DecorationSpan, GlyphLayout, LineBox, ParagraphLayout,
    PlacedInlineBox,
};
pub use linebreak::{BreakStrategy, Hyphenation, LineBreakParams};
pub use measure::paragraph_intrinsic_size;
pub use model::{
    FontSpec, InlineBox, InlineBoxKind, InlineBoxSlot, Paragraph, ParagraphAlign, ProtrusionFactors, ProtrusionTable, StyledRun,
    TextDecoration, VerticalAlign,
};
pub use shape::{CosmicShaper, LineShaper};
