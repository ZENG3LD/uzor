//! Shared code for vello-based uzor rendering backends.
//!
//! This crate extracts the code that would otherwise be copy-pasted into every
//! vello backend (vello-context, wasm_wgpu, win-desktop, …):
//!
//! - **`color`** — CSS color string parsing to `peniko::Color`
//! - **`font`** — Embedded Roboto fonts, CSS font shorthand parsing, glyph
//!   layout and text measurement via skrifa
//! - **`state`** — [`state::VelloContextCore`] (shared drawing state) and
//!   [`state::SavedState`] (save/restore stack entry)
//! - **`path`** — [`path::PathBuilder`] wrapping `kurbo::BezPath` with a
//!   Canvas2D-style API

pub mod color;
pub mod font;
pub mod path;
pub mod state;

// ---------------------------------------------------------------------------
// Convenience re-exports
// ---------------------------------------------------------------------------

pub use color::{Color, parse_color, parse_color_with_alpha, parse_color_to_rgba_f32};
pub use font::{
    FontInfo, GlyphLayout,
    get_cached_font, to_font_ref,
    layout_glyphs, measure_text, parse_css_font,
};
pub use path::PathBuilder;
pub use state::{SavedState, VelloContextCore};
