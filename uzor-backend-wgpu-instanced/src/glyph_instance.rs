//! GPU instance data for glyph (text) rendering.
//!
//! Each visible glyph in a frame becomes one `GlyphInstance`.  The vertex
//! shader reads these instances to construct a screen-space quad; the
//! fragment shader samples the R8Unorm atlas and multiplies the alpha by the
//! text color.
//!
//! Memory layout (64 bytes, 16-byte aligned):
//! - pos:       8 bytes  (vec2<f32>)
//! - size:      8 bytes  (vec2<f32>)
//! - uv_pos:    8 bytes  (vec2<f32>)
//! - uv_size:   8 bytes  (vec2<f32>)
//! - color:    16 bytes  (vec4<f32>)
//! - clip_rect: 16 bytes (vec4<f32>)
//! Total:       64 bytes

use bytemuck::{Pod, Zeroable};

/// A single glyph quad uploaded to the GPU as an instance.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GlyphInstance {
    /// Top-left corner of the glyph quad in screen pixels.
    pub pos: [f32; 2],
    /// Width and height of the glyph quad in screen pixels.
    pub size: [f32; 2],
    /// UV coordinates of the top-left corner in the glyph atlas (0.0–1.0).
    pub uv_pos: [f32; 2],
    /// UV dimensions of the glyph in the atlas (0.0–1.0).
    pub uv_size: [f32; 2],
    /// Text color RGBA, each component in 0.0–1.0.
    pub color: [f32; 4],
    /// Clip rectangle (x, y, w, h) in screen pixels.
    /// Fragments outside this region are discarded.
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<GlyphInstance>() == 64,
    "GlyphInstance must be exactly 64 bytes"
);

const _: () = assert!(
    std::mem::size_of::<GlyphInstance>() % 16 == 0,
    "GlyphInstance must be 16-byte aligned"
);
