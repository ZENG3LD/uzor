//! WGSL shader source strings for the instanced renderer.
//!
//! Four shaders: quad, line, triangle, glyph. All share the same
//! `Uniforms` bind group (group 0, binding 0) carrying screen_size
//! in logical pixels. Colours are packed RGBA8 in a `u32` and
//! unpacked via `unpack4x8unorm` — see `instances::pack_rgba8`.

/// Quad shader: filled / bordered rounded rectangles with SDF AA.
pub const QUAD_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match QuadInstance in instances.rs (56 bytes).
struct QuadInstance {
    @location(0) pos:           vec2<f32>,
    @location(1) size:          vec2<f32>,
    @location(2) color_packed:  u32,
    @location(3) border_packed: u32,
    @location(4) corner_radius: f32,
    @location(5) border_width:  f32,
    @location(6) _pad0:         vec2<f32>,
    @location(7) clip_rect:     vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) frag_pos:      vec2<f32>,
    @location(1) size:          vec2<f32>,
    @location(2) color:         vec4<f32>,
    @location(3) corner_radius: f32,
    @location(4) border_width:  f32,
    @location(5) border_color:  vec4<f32>,
    @location(6) clip_rect:     vec4<f32>,
};

fn quad_vert_pos(vertex_index: u32) -> vec2<f32> {
    // tris:  TL,TR,BL  TR,BR,BL
    let xs = array<f32, 6>(0.0, 1.0, 0.0,  1.0, 1.0, 0.0);
    let ys = array<f32, 6>(0.0, 0.0, 1.0,  0.0, 1.0, 1.0);
    return vec2<f32>(xs[vertex_index], ys[vertex_index]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: QuadInstance,
) -> VertexOut {
    let aa_pad = 1.0;
    let padded_pos  = instance.pos  - vec2<f32>(aa_pad, aa_pad);
    let padded_size = instance.size + vec2<f32>(aa_pad * 2.0, aa_pad * 2.0);

    let uv = quad_vert_pos(vertex_index);
    let px = padded_pos + uv * padded_size;
    let frag_pos = px - instance.pos;

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.clip_pos      = vec4<f32>(ndc, 0.0, 1.0);
    out.frag_pos      = frag_pos;
    out.size          = instance.size;
    out.color         = unpack4x8unorm(instance.color_packed);
    out.corner_radius = instance.corner_radius;
    out.border_width  = instance.border_width;
    out.border_color  = unpack4x8unorm(instance.border_packed);
    out.clip_rect     = instance.clip_rect;
    return out;
}

fn sdf_rounded_rect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let px_abs = in.clip_pos.xy;
    let cr = in.clip_rect;
    if px_abs.x < cr.x || px_abs.y < cr.y
       || px_abs.x > cr.x + cr.z || px_abs.y > cr.y + cr.w {
        discard;
    }

    let half = in.size * 0.5;
    let p    = in.frag_pos - half;
    let r    = clamp(in.corner_radius, 0.0, min(half.x, half.y));
    let dist = sdf_rounded_rect(p, half, r);

    let aa     = fwidth(dist);
    let fill_a = 1.0 - smoothstep(-aa, aa, dist);
    if fill_a <= 0.0 { discard; }

    var out_color = in.color;
    if in.border_width > 0.0 {
        let inner_dist = dist + in.border_width;
        let border_a   = 1.0 - smoothstep(-aa, aa, inner_dist);
        let on_border = clamp(border_a - (1.0 - smoothstep(-aa, aa, dist + 0.5)), 0.0, 1.0);
        out_color = mix(in.color, in.border_color, on_border);
        out_color.a = max(in.color.a, in.border_color.a * on_border);
    }

    out_color.a *= fill_a;
    return out_color;
}
"#;

/// Glyph shader — text quads sampled from R8Unorm atlas.
pub const GLYPH_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0) var atlas_tex:     texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// Instance data — must match GlyphInstance in glyph_instance.rs (56 bytes).
struct GlyphInstance {
    @location(0) pos:          vec2<f32>,
    @location(1) size:         vec2<f32>,
    @location(2) uv_pos:       vec2<f32>,
    @location(3) uv_size:      vec2<f32>,
    @location(4) color_packed: u32,
    @location(5) _pad0:        f32,
    @location(6) clip_rect:    vec4<f32>,
};

struct GlyphVsOut {
    @builtin(position) position:  vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) color:     vec4<f32>,
    @location(2) clip_rect: vec4<f32>,
    @location(3) frag_pos:  vec2<f32>,
};

@vertex
fn vs_glyph(
    @builtin(vertex_index) vi: u32,
    input: GlyphInstance,
) -> GlyphVsOut {
    // Triangle strip: 0=TL, 1=TR, 2=BL, 3=BR
    let x = f32(vi & 1u);
    let y = f32((vi >> 1u) & 1u);

    let pixel = input.pos + vec2<f32>(x, y) * input.size;

    let ndc = vec2<f32>(
        pixel.x / uniforms.screen_size.x *  2.0 - 1.0,
        1.0 - pixel.y / uniforms.screen_size.y * 2.0,
    );

    var out: GlyphVsOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    out.uv        = input.uv_pos + vec2<f32>(x, y) * input.uv_size;
    out.color     = unpack4x8unorm(input.color_packed);
    out.clip_rect = input.clip_rect;
    out.frag_pos  = pixel;
    return out;
}

@fragment
fn fs_glyph(in: GlyphVsOut) -> @location(0) vec4<f32> {
    let cr = in.clip_rect;
    if in.frag_pos.x < cr.x || in.frag_pos.y < cr.y
       || in.frag_pos.x > cr.x + cr.z || in.frag_pos.y > cr.y + cr.w {
        discard;
    }
    let alpha = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    if alpha < 0.01 { discard; }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

/// Triangle shader — flat-filled triangles with barycentric edge AA.
pub const TRIANGLE_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match TriangleInstance in instances.rs (56 bytes).
struct TriangleInstance {
    @location(0) v0:           vec2<f32>,
    @location(1) v1:           vec2<f32>,
    @location(2) v2:           vec2<f32>,
    @location(3) color_packed: u32,
    @location(4) _pad0:        vec3<f32>,
    @location(5) clip_rect:    vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color:     vec4<f32>,
    @location(1) clip_rect: vec4<f32>,
    @location(2) bary:      vec3<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    inst: TriangleInstance,
) -> VertexOut {
    var px: vec2<f32>;
    var bary: vec3<f32>;
    switch vi {
        case 0u: { px = inst.v0; bary = vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { px = inst.v1; bary = vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { px = inst.v2; bary = vec3<f32>(0.0, 0.0, 1.0); }
        default: { px = inst.v0; bary = vec3<f32>(1.0, 0.0, 0.0); }
    }

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    out.color     = unpack4x8unorm(inst.color_packed);
    out.clip_rect = inst.clip_rect;
    out.bary      = bary;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let px_abs = in.position.xy;
    let cr = in.clip_rect;
    if px_abs.x < cr.x || px_abs.y < cr.y
       || px_abs.x > cr.x + cr.z || px_abs.y > cr.y + cr.w {
        discard;
    }
    let edge_dist = min(in.bary.x, min(in.bary.y, in.bary.z));
    let aa = fwidth(edge_dist);
    let alpha = smoothstep(0.0, aa, edge_dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

/// Line shader — capsule SDF segments.
pub const LINE_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match LineInstance in instances.rs (56 bytes).
struct LineInstance {
    @location(0) start:        vec2<f32>,
    @location(1) end:          vec2<f32>,
    @location(2) color_packed: u32,
    @location(3) width:        f32,
    @location(4) cap_flags:    f32,
    @location(5) _pad0:        f32,
    @location(6) _pad1:        vec2<f32>,
    @location(7) clip_rect:    vec4<f32>,
};

struct LineVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color:     vec4<f32>,
    @location(1) seg_start: vec2<f32>,
    @location(2) seg_end:   vec2<f32>,
    @location(3) width:     f32,
    @location(4) cap_flags: f32,
    @location(5) frag_pos:  vec2<f32>,
    @location(6) clip_rect: vec4<f32>,
};

fn quad_vert_pos(vertex_index: u32) -> vec2<f32> {
    let xs = array<f32, 6>(0.0, 1.0, 0.0,  1.0, 1.0, 0.0);
    let ys = array<f32, 6>(0.0, 0.0, 1.0,  0.0, 1.0, 1.0);
    return vec2<f32>(xs[vertex_index], ys[vertex_index]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    inst: LineInstance,
) -> LineVsOut {
    let dir = inst.end - inst.start;
    let len = length(dir);
    let tangent = select(vec2<f32>(1.0, 0.0), dir / len, len > 0.0001);
    let normal = vec2<f32>(-tangent.y, tangent.x);

    let half_w = inst.width * 0.5 + 1.0; // +1 px AA fringe
    let along = quad_vert_pos(vi).x;
    let across = quad_vert_pos(vi).y;

    // Pad start/end by half_w along tangent so caps are inside the quad.
    let s = inst.start - tangent * half_w;
    let e = inst.end   + tangent * half_w;

    let base = mix(s, e, along);
    let px = base + normal * ((across - 0.5) * (inst.width + 2.0));

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: LineVsOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    out.color     = unpack4x8unorm(inst.color_packed);
    out.seg_start = inst.start;
    out.seg_end   = inst.end;
    out.width     = inst.width;
    out.cap_flags = inst.cap_flags;
    out.frag_pos  = px;
    out.clip_rect = inst.clip_rect;
    return out;
}

fn sdf_capsule(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

@fragment
fn fs_main(in: LineVsOut) -> @location(0) vec4<f32> {
    let cr = in.clip_rect;
    if in.frag_pos.x < cr.x || in.frag_pos.y < cr.y
       || in.frag_pos.x > cr.x + cr.z || in.frag_pos.y > cr.y + cr.w {
        discard;
    }
    let r = in.width * 0.5;
    let dist = sdf_capsule(in.frag_pos, in.seg_start, in.seg_end, r);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    if alpha <= 0.0 { discard; }

    // Butt caps via cap_flags (matches the old shader's behaviour).
    // 0 = round-round, 1 = butt-start, 2 = butt-end, 3 = butt-both
    let flags = u32(in.cap_flags + 0.5);
    if (flags & 1u) != 0u {
        // Butt at start: discard if projection < 0.
        let dir = in.seg_end - in.seg_start;
        let len_sq = max(dot(dir, dir), 1e-6);
        let h = dot(in.frag_pos - in.seg_start, dir) / len_sq;
        if h < 0.0 { discard; }
    }
    if (flags & 2u) != 0u {
        let dir = in.seg_end - in.seg_start;
        let len_sq = max(dot(dir, dir), 1e-6);
        let h = dot(in.frag_pos - in.seg_start, dir) / len_sq;
        if h > 1.0 { discard; }
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;
