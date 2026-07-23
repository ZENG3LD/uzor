//! Native wgpu pipelines — one module per primitive family.
//!
//! Wave 1 Commit 1 shipped the Quad SDF pipeline (`FillRect`/
//! `StrokeRect`, solid brush). Commit 2 added the Line/capsule
//! pipeline. Commit 3 added the Path/triangle pipeline (lyon
//! tessellation). Wave 2 Commit 2 adds the Glyph pipeline (textured
//! quads sampling `crate::atlas::NativeGlyphAtlas`).

pub(crate) mod glyph;
pub(crate) mod line;
pub(crate) mod path;
pub(crate) mod quad;
