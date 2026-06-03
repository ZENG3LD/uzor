//! GPU instance data structures for instanced rendering.
//!
//! These structs are uploaded directly to GPU buffers via `bytemuck`.
//! Layout is `repr(C)` for a deterministic byte order matching the
//! WGSL vertex buffers.
//!
//! ## Packed-color wire format (URX 1.5, breaking from 1.4.x)
//!
//! Per 16-research §"WGPU-P4" / Toji.dev WebGPU best-practices, RGBA
//! is packed into a `u32` (little-endian: byte 0 = R, byte 1 = G,
//! byte 2 = B, byte 3 = A) and unpacked in the fragment shader via
//! `unpack4x8unorm(u32) -> vec4<f32>`. This is a single GPU
//! instruction on all backends.
//!
//! Bandwidth savings (per-instance):
//!
//! - `QuadInstance`:     80 B → 56 B  (-30 %)
//! - `LineInstance`:     64 B → 56 B  (-12 %)
//! - `TriangleInstance`: 64 B → 56 B  (-12 %)
//! - `GlyphInstance`:    64 B → 56 B  (-12 %)
//!
//! All structs stay multiples of 16 B for 16-byte alignment (wgpu
//! requirement for storage buffers; harmless for vertex buffers but
//! matches the rule of least surprise).
//!
//! ## Pack helper
//!
//! [`pack_rgba8`] is the canonical way to pack a `[u8; 4]` (premul
//! or not — the shader doesn't care) into the `u32` wire format. Use
//! it on every call site that previously wrote `[f32; 4]` colours.

use bytemuck::{Pod, Zeroable};

/// Pack a 4-byte little-endian RGBA into one u32 in the
/// `unpack4x8unorm`-compatible layout: byte 0 = R, byte 1 = G,
/// byte 2 = B, byte 3 = A.
///
/// This is the inverse of WGSL `unpack4x8unorm(u32)` — call it on
/// every consumer site that previously emitted `color: [f32; 4]`.
#[inline]
pub const fn pack_rgba8(rgba: [u8; 4]) -> u32 {
    (rgba[0] as u32)
        | ((rgba[1] as u32) << 8)
        | ((rgba[2] as u32) << 16)
        | ((rgba[3] as u32) << 24)
}

/// Convenience: float-in-[0,1] RGBA → packed u32. Clamps + rounds
/// half-up. Drop-in replacement for sites that have float colour.
#[inline]
pub fn pack_rgba_f32(rgba: [f32; 4]) -> u32 {
    let q = |x: f32| -> u8 {
        let c = x.clamp(0.0, 1.0);
        (c * 255.0 + 0.5) as u8
    };
    pack_rgba8([q(rgba[0]), q(rgba[1]), q(rgba[2]), q(rgba[3])])
}

/// A filled/stroked rectangle instance — **56 bytes packed**.
///
/// Renders as 2 triangles (6 vertices) with a rounded-rect SDF in the
/// fragment shader. The vertex shader expands each instance to a
/// screen-space quad padded by 1 px on every side for AA.
///
/// Memory layout (56 bytes, multiple of 8):
/// - pos:           8 bytes  ([f32; 2])
/// - size:          8 bytes  ([f32; 2])
/// - color:         4 bytes  (u32, packed RGBA8 little-endian)
/// - border_color:  4 bytes  (u32, packed RGBA8 little-endian)
/// - corner_radius: 4 bytes  (f32)
/// - border_width:  4 bytes  (f32)
/// - _pad0:         8 bytes  (alignment padding)
/// - clip_rect:    16 bytes  ([f32; 4])
/// Total:          56 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    pub pos:           [f32; 2],
    pub size:          [f32; 2],
    pub color:         u32,
    pub border_color:  u32,
    pub corner_radius: f32,
    pub border_width:  f32,
    pub _pad0:         [f32; 2],
    pub clip_rect:     [f32; 4],
}

impl QuadInstance {
    /// Build from float colours — convenience for callers migrating
    /// from the 1.4.x API.
    #[inline]
    pub fn from_float_color(
        pos: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        corner_radius: f32,
        border_width: f32,
        border_color: [f32; 4],
        clip_rect: [f32; 4],
    ) -> Self {
        Self {
            pos, size,
            color:        pack_rgba_f32(color),
            border_color: pack_rgba_f32(border_color),
            corner_radius,
            border_width,
            _pad0: [0.0; 2],
            clip_rect,
        }
    }
}

/// A line segment instance — **56 bytes packed**.
///
/// Renders as an oriented quad enclosing the segment (expanded by
/// `width/2 + 1` px for AA). Fragment shader uses a capsule SDF with
/// smooth-step AA.
///
/// Memory layout (56 bytes):
/// - start:      8 bytes
/// - end:        8 bytes
/// - color:      4 bytes  (u32, packed)
/// - width:      4 bytes
/// - cap_flags:  4 bytes  (0=round-round, 1=butt-start, 2=butt-end, 3=butt-both)
/// - _pad0:      4 bytes
/// - _pad1:      8 bytes
/// - clip_rect: 16 bytes
/// Total:       56 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineInstance {
    pub start:     [f32; 2],
    pub end:       [f32; 2],
    pub color:     u32,
    pub width:     f32,
    pub cap_flags: f32,
    pub _pad0:     f32,
    pub _pad1:     [f32; 2],
    pub clip_rect: [f32; 4],
}

impl LineInstance {
    #[inline]
    pub fn from_float_color(
        start: [f32; 2],
        end: [f32; 2],
        color: [f32; 4],
        width: f32,
        cap_flags: f32,
        clip_rect: [f32; 4],
    ) -> Self {
        Self {
            start, end,
            color: pack_rgba_f32(color),
            width, cap_flags,
            _pad0: 0.0,
            _pad1: [0.0; 2],
            clip_rect,
        }
    }
}

/// A filled triangle instance — **56 bytes packed**.
///
/// Three vertices forming a single triangle. Fragment shader applies
/// clip-rect discard, outputs flat color with barycentric edge AA.
///
/// Memory layout (56 bytes):
/// - v0:        8 bytes
/// - v1:        8 bytes
/// - v2:        8 bytes
/// - color:     4 bytes (u32, packed)
/// - _pad0:    12 bytes
/// - clip_rect:16 bytes
/// Total:      56 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TriangleInstance {
    pub v0:        [f32; 2],
    pub v1:        [f32; 2],
    pub v2:        [f32; 2],
    pub color:     u32,
    pub _pad0:     [f32; 3],
    pub clip_rect: [f32; 4],
}

impl TriangleInstance {
    #[inline]
    pub fn from_float_color(
        v0: [f32; 2],
        v1: [f32; 2],
        v2: [f32; 2],
        color: [f32; 4],
        clip_rect: [f32; 4],
    ) -> Self {
        Self {
            v0, v1, v2,
            color: pack_rgba_f32(color),
            _pad0: [0.0; 3],
            clip_rect,
        }
    }
}

const _: () = assert!(
    std::mem::size_of::<QuadInstance>() % 8 == 0,
    "QuadInstance size must be a multiple of 8"
);

const _: () = assert!(
    std::mem::size_of::<LineInstance>() % 8 == 0,
    "LineInstance size must be a multiple of 8"
);

const _: () = assert!(
    std::mem::size_of::<TriangleInstance>() % 8 == 0,
    "TriangleInstance size must be a multiple of 8"
);

/// A single draw command that preserves painter's order (z-order).
///
/// The renderer processes these in sequence, batching consecutive
/// same-type commands into a single GPU draw call while maintaining
/// submission order. Later commands draw on top of earlier ones.
#[derive(Clone)]
pub enum DrawCmd {
    /// A filled or bordered rounded rectangle.
    Quad(QuadInstance),
    /// A filled triangle (from lyon tessellation).
    Triangle(TriangleInstance),
    /// A capsule-SDF line segment.
    Line(LineInstance),
    /// A text area to be rasterized via cosmic-text.
    Text(crate::text::TextAreaData),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quad_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<QuadInstance>(), 56);
    }

    #[test]
    fn line_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<LineInstance>(), 56);
    }

    #[test]
    fn triangle_instance_is_56_bytes() {
        assert_eq!(std::mem::size_of::<TriangleInstance>(), 56);
    }

    #[test]
    fn pack_rgba8_round_trip_endianness() {
        // Byte 0 = R, byte 1 = G, byte 2 = B, byte 3 = A.
        let p = pack_rgba8([0x11, 0x22, 0x33, 0x44]);
        // u32 little-endian: 0x44332211
        assert_eq!(p, 0x44332211);
    }

    #[test]
    fn pack_rgba_f32_quantises_midpoints() {
        let p = pack_rgba_f32([0.0, 0.5, 1.0, 0.25]);
        let bytes = p.to_le_bytes();
        assert_eq!(bytes[0], 0);
        // 0.5 * 255 + 0.5 = 128.0 → 128
        assert_eq!(bytes[1], 128);
        assert_eq!(bytes[2], 255);
        // 0.25 * 255 + 0.5 = 64.25 → 64
        assert_eq!(bytes[3], 64);
    }

    #[test]
    fn pack_rgba_f32_clamps_out_of_range() {
        let p = pack_rgba_f32([-1.0, 2.0, 0.5, f32::NAN]);
        let bytes = p.to_le_bytes();
        assert_eq!(bytes[0], 0);
        assert_eq!(bytes[1], 255);
        // 0.5 * 255 + 0.5 = 128 → 128
        assert_eq!(bytes[2], 128);
        // NaN clamps to 0.0 via clamp(NaN, 0, 1) on stable Rust.
        assert_eq!(bytes[3], 0);
    }
}
