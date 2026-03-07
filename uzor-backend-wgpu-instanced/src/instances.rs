//! GPU instance data structures for instanced rendering.
//!
//! These structs are uploaded directly to GPU buffers via `bytemuck`.
//! All fields are `f32` and the structs are `repr(C)` to ensure a
//! well-defined memory layout compatible with WGSL vertex buffers.

use bytemuck::{Pod, Zeroable};

/// A filled/stroked rectangle instance.
///
/// Rendered as 2 triangles (6 vertices) with a rounded-rectangle SDF in the
/// fragment shader.  The vertex shader expands each instance to a screen-space
/// quad padded by 1 px on every side for anti-aliasing.
///
/// Memory layout (80 bytes, 16-byte aligned):
/// - pos:          8 bytes
/// - size:         8 bytes
/// - color:       16 bytes
/// - corner_radius: 4 bytes
/// - border_width:  4 bytes
/// - _pad0:         8 bytes  (alignment padding before border_color)
/// - border_color: 16 bytes
/// - clip_rect:    16 bytes
/// Total:          80 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// Position (top-left corner) in logical pixels.
    pub pos: [f32; 2],
    /// Size in logical pixels.
    pub size: [f32; 2],
    /// Fill color RGBA, each component in 0.0–1.0.
    pub color: [f32; 4],
    /// Corner radius in pixels (0 = sharp corners).
    pub corner_radius: f32,
    /// Border width in pixels (0 = no border).
    pub border_width: f32,
    /// Padding bytes to keep border_color 16-byte aligned.
    pub _pad0: [f32; 2],
    /// Border color RGBA.
    pub border_color: [f32; 4],
    /// Clip rectangle (x, y, w, h) in logical pixels.
    /// Fragments outside this region are discarded.
    pub clip_rect: [f32; 4],
}

/// A line segment instance.
///
/// Rendered as an oriented quad that fully encloses the segment (expanded by
/// `width/2 + 1` px for anti-aliasing).  The fragment shader uses a capsule
/// SDF (point-to-segment distance) with smooth-step AA, with optional butt
/// caps at each end to eliminate joint dots in polylines.
///
/// Memory layout (64 bytes, 16-byte aligned):
/// - start:      8 bytes
/// - end:        8 bytes
/// - color:     16 bytes
/// - width:      4 bytes
/// - cap_flags:  4 bytes  (0=round-round, 1=butt-start, 2=butt-end, 3=butt-both)
/// - _pad0:      8 bytes  (alignment padding before clip_rect)
/// - clip_rect: 16 bytes
/// Total:        64 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineInstance {
    /// Start point in logical pixels.
    pub start: [f32; 2],
    /// End point in logical pixels.
    pub end: [f32; 2],
    /// Line color RGBA, each component in 0.0–1.0.
    pub color: [f32; 4],
    /// Line width in logical pixels.
    pub width: f32,
    /// Cap style flags:
    /// - 0.0 = round caps at both ends (default capsule)
    /// - 1.0 = butt cap at start, round at end
    /// - 2.0 = round cap at start, butt at end
    /// - 3.0 = butt caps at both ends (interior polyline segment)
    pub cap_flags: f32,
    /// Padding to align clip_rect to 16-byte boundary.
    pub _pad0: [f32; 2],
    /// Clip rectangle (x, y, w, h) in logical pixels.
    pub clip_rect: [f32; 4],
}

/// A filled triangle instance.
///
/// Three vertices forming a single triangle. The fragment shader applies
/// clip-rect discard and outputs a flat color with edge anti-aliasing.
///
/// Memory layout (64 bytes, 16-byte aligned):
/// - v0:        8 bytes
/// - v1:        8 bytes
/// - v2:        8 bytes
/// - _pad0:     8 bytes (alignment)
/// - color:    16 bytes
/// - clip_rect:16 bytes
/// Total:       64 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct TriangleInstance {
    /// First vertex position in logical pixels.
    pub v0: [f32; 2],
    /// Second vertex position in logical pixels.
    pub v1: [f32; 2],
    /// Third vertex position in logical pixels.
    pub v2: [f32; 2],
    /// Padding for 16-byte alignment.
    pub _pad0: [f32; 2],
    /// Fill color RGBA, each component in 0.0–1.0.
    pub color: [f32; 4],
    /// Clip rectangle (x, y, w, h) in logical pixels.
    pub clip_rect: [f32; 4],
}

const _: () = assert!(
    std::mem::size_of::<QuadInstance>() % 16 == 0,
    "QuadInstance must be 16-byte aligned"
);

const _: () = assert!(
    std::mem::size_of::<LineInstance>() % 16 == 0,
    "LineInstance must be 16-byte aligned"
);

const _: () = assert!(
    std::mem::size_of::<TriangleInstance>() % 16 == 0,
    "TriangleInstance must be 16-byte aligned"
);

/// A single draw command that preserves painter's order (z-order).
///
/// The renderer processes these in sequence, batching consecutive same-type
/// commands into a single GPU draw call while maintaining the submission order.
/// This ensures correct visual layering: later commands draw on top of earlier ones.
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
    fn quad_instance_size_is_multiple_of_16() {
        let size = std::mem::size_of::<QuadInstance>();
        assert_eq!(size % 16, 0, "QuadInstance size {} is not 16-byte aligned", size);
    }

    #[test]
    fn line_instance_size_is_multiple_of_16() {
        let size = std::mem::size_of::<LineInstance>();
        assert_eq!(size % 16, 0, "LineInstance size {} is not 16-byte aligned", size);
    }

    #[test]
    fn triangle_instance_size_is_multiple_of_16() {
        let size = std::mem::size_of::<TriangleInstance>();
        assert_eq!(size % 16, 0, "TriangleInstance size {} is not 16-byte aligned", size);
    }
}
