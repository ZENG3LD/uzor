//! ASCII cell-shader mode (Arc 2 Phase 4 of
//! `nemo/docs/uzor-engines/uzor_text_arc2_design.md`) — a character-grid
//! per-cell-shader engine, absorbed **verbatim** from `uzor` core
//! (`uzor::ui::effects::text::cell_shader`, hard cutover, §4 Phase 4: no
//! compat shim left behind in core).
//!
//! Re-exports the identical public surface `cell_shader.rs` shipped in
//! core, unchanged down to symbol names (this phase's two live consumers,
//! `mirage2app`'s `AuroraSkin`/`RadarSkin` centerpiece scenes, only needed a
//! `use` path change, never a rename): [`AsciiGrid`], [`CellShader`],
//! [`Cell`], [`Coord`], [`Cursor`], [`GridContext`], [`GlitchLetter`],
//! [`density_char`], [`hsl`], [`sd_box`], [`sd_circle`], [`DENSITY`].
//!
//! [`ParagraphAsciiShader`] (`paragraph_shader.rs`) is the net-new piece
//! this phase adds: the first bridge from a real laid-out
//! [`crate::layout::ParagraphLayout`] into [`AsciiGrid`] — before this
//! phase, `AsciiGrid` only ever rendered SDF/bitmap-procedural content
//! (`GlitchLetter`, `AuroraSkin`'s flow field, `RadarSkin`'s sweep), never
//! real text.
//!
//! [`figures`] is the ASCII dataviz collection: bar / curve / heatmap / dag
//! as [`CellShader`]s over borrowed slices. Play is still / live / arm
//! (static until the cursor wakes it). No dependency on `uzor-figures`.

mod cell_shader;
pub mod figures;
mod paragraph_shader;

pub use cell_shader::{
    density_char, hsl, sd_box, sd_circle, AsciiGrid, Cell, CellShader, Coord, Cursor, GlitchLetter,
    GridContext, DENSITY,
};
pub use paragraph_shader::{
    build_ascii_grid, draw_ascii_grid, grid_dims_for_layout, AsciiGridStyle, ParagraphAsciiShader,
};
