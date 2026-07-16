//! `uzor-text` — pretext-pattern text layout engine for uzor (Arc 2).
//!
//! Pure, stateless functions from a plain string + [`FontSpec`] to
//! per-glyph positions ([`ParagraphLayout`]) via a swappable [`LineShaper`]
//! ([`CosmicShaper`] today — cosmic-text, the same substrate
//! `uzor::shaper` already uses for single-line text; parley could swap in
//! later behind the same trait).
//!
//! This crate currently implements **Phase 1 only**: plain-text greedy
//! word-wrap (no rich spans, no kinetics, no ascii mode yet — see
//! `nemo/docs/uzor-viz/uzor_text_arc2_design.md` for the full Arc 2 phase
//! plan and this crate's `CLAUDE.md` for which parts of that plan are and
//! aren't built).
//!
//! **NOT in this crate yet** (later phases — do not add without a plan doc):
//! - `model::span` (`StyledRun`, `Paragraph`, `ParagraphAlign`) and
//!   `model::inline_box` (`InlineBox`) — rich spans, Phase 2.
//! - `linebreak::{hyphenate, knuth_plass}` — Phase 5.
//! - `kinetics` (`GlyphState`/`MorphTransition`/`build_morph`/`sample`) —
//!   Phase 3.
//! - `ascii` (`cell_shader` absorption + `ParagraphAsciiShader`) — Phase 4.

pub mod draw;
pub mod layout;
pub mod measure;
pub mod model;
pub mod shape;

pub use draw::draw_layout;
pub use layout::{align_lines, layout_text, Align, GlyphLayout, LineBox, ParagraphLayout};
pub use measure::paragraph_intrinsic_size;
pub use model::FontSpec;
pub use shape::{CosmicShaper, LineShaper};
