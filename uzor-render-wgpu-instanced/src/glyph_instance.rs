//! GPU instance data for glyph (text) rendering.
//!
//! Each visible glyph in a frame becomes one `GlyphInstance`. The vertex
//! shader reads these instances to construct a screen-space quad; the
//! fragment shader samples the R8Unorm atlas and multiplies the alpha by the
//! text color.
//!
//! Memory layout (56 bytes — packed colour, breaking from 1.4.x):
//! - pos:       8 bytes  ([f32; 2])
//! - size:      8 bytes  ([f32; 2])
//! - uv_pos:    8 bytes  ([f32; 2])
//! - uv_size:   8 bytes  ([f32; 2])
//! - color:     4 bytes  (u32, packed RGBA8 little-endian — see
//!                        `crate::instances::pack_rgba8` / `pack_rgba_f32`)
//! - _pad0:     4 bytes
//! - clip_rect:16 bytes  ([f32; 4])
//! Total:      56 bytes

use bytemuck::{Pod, Zeroable};

use crate::instances::pack_rgba_f32;

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
    /// Packed RGBA8 (byte 0 = R, ..., byte 3 = A). Unpacked in
    /// `vs_glyph` via `unpack4x8unorm`.
    pub color: u32,
    /// Padding to keep `clip_rect` 16-byte aligned.
    pub _pad0: f32,
    /// Clip rectangle (x, y, w, h) in screen pixels.
    pub clip_rect: [f32; 4],
}

impl GlyphInstance {
    /// Build from float colour. Convenience for callers migrating
    /// from the 1.4.x API (which took `color: [f32; 4]`).
    #[inline]
    pub fn from_float_color(
        pos: [f32; 2],
        size: [f32; 2],
        uv_pos: [f32; 2],
        uv_size: [f32; 2],
        color: [f32; 4],
        clip_rect: [f32; 4],
    ) -> Self {
        Self {
            pos, size, uv_pos, uv_size,
            color: pack_rgba_f32(color),
            _pad0: 0.0,
            clip_rect,
        }
    }
}

const _: () = assert!(
    std::mem::size_of::<GlyphInstance>() == 56,
    "GlyphInstance must be exactly 56 bytes"
);

const _: () = assert!(
    std::mem::size_of::<GlyphInstance>() % 8 == 0,
    "GlyphInstance size must be a multiple of 8"
);
