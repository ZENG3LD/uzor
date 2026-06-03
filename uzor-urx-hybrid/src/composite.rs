//! Compositor render pass — draws N textured quads (one per cached
//! region) over the swap chain target. One pipeline, one bind group
//! per region, indexed-quad geometry with screen-space coordinates.
//!
//! Shader is intentionally trivial: vertex shader passes through a
//! per-instance transform + UV; fragment shader samples the region
//! texture and writes premultiplied output. The complexity stays in
//! WHICH textures to feed it (the hybrid backend's region cache).

use bytemuck::{Pod, Zeroable};

/// One instance = one region quad. Position + size in pixel space;
/// the vertex shader converts to clip space using the uniform
/// screen dimensions.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct QuadInstance {
    /// `[dst_x, dst_y, dst_w, dst_h]` in pixel space.
    pub dst:    [f32; 4],
    /// `[u0, v0, u1, v1]` — sub-rect of the source texture (defaults
    /// to `[0, 0, 1, 1]` for whole-texture quads).
    pub uv:     [f32; 4],
    /// Per-instance tint × source.rgba. Use `[1,1,1,1]` for pass-through.
    pub tint:   [f32; 4],
}

/// Screen-size uniform. One per render pass.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ScreenUniform {
    pub w: f32,
    pub h: f32,
    pub _pad: [f32; 2],
}

pub const COMPOSITE_SHADER: &str = r#"
struct ScreenUniform { w: f32, h: f32, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> screen: ScreenUniform;
@group(1) @binding(0) var samp:    sampler;
@group(1) @binding(1) var src_tex: texture_2d<f32>;

struct InstanceIn {
    @location(0) dst:  vec4<f32>,
    @location(1) uv:   vec4<f32>,
    @location(2) tint: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0)       uv:   vec2<f32>,
    @location(1)       tint: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vid: u32, inst: InstanceIn) -> VsOut {
    // 4 vertices per quad, indexed via vid (0..6 via triangle list).
    let i = vid % 6u;
    var corner = vec2<f32>(0.0, 0.0);
    var uv     = vec2<f32>(0.0, 0.0);
    // triangle list: 0,1,2 + 0,2,3 (CCW)
    if      (i == 0u || i == 3u) { corner = vec2<f32>(0.0, 0.0); uv = vec2<f32>(inst.uv.x, inst.uv.y); }
    else if (i == 1u)             { corner = vec2<f32>(1.0, 0.0); uv = vec2<f32>(inst.uv.z, inst.uv.y); }
    else if (i == 2u || i == 4u)  { corner = vec2<f32>(1.0, 1.0); uv = vec2<f32>(inst.uv.z, inst.uv.w); }
    else if (i == 5u)             { corner = vec2<f32>(0.0, 1.0); uv = vec2<f32>(inst.uv.x, inst.uv.w); }

    let px = inst.dst.x + corner.x * inst.dst.z;
    let py = inst.dst.y + corner.y * inst.dst.w;
    // Pixel → NDC. y flipped (NDC y up, screen y down).
    let nx = (px / screen.w) * 2.0 - 1.0;
    let ny = 1.0 - (py / screen.h) * 2.0;

    var out: VsOut;
    out.clip = vec4<f32>(nx, ny, 0.0, 1.0);
    out.uv   = uv;
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let src = textureSample(src_tex, samp, in.uv);
    // Premultiplied source × premultiplied tint = source × tint
    // for the RGB channels; alpha = source.a × tint.a. Result is
    // already premultiplied → blend state should use "src + dst*(1-src.a)".
    return src * in.tint;
}
"#;
