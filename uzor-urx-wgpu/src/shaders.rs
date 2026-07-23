//! WGSL shader source strings for the native pipelines.
//!
//! House style matches the legacy crate (`uzor-render-wgpu-instanced/src/shaders.rs`):
//! inline `pub const &str`, no `.wgsl` files. Every shader shares the
//! same `Uniforms` bind group (group 0, binding 0) carrying
//! `screen_size` in physical pixels.
//!
//! Wave 1 Commit 1 shipped `QUAD_SHADER_NATIVE`. Commit 2 added
//! `LINE_SHADER_NATIVE`. Commit 3 added `PATH_SHADER_NATIVE`. Wave 2
//! Commit 2 added `GLYPH_SHADER_NATIVE`. Wave 3 Commit 2 adds
//! `STENCIL_MASK_SHADER_NATIVE`.

/// Quad shader — filled/bordered rounded rectangles with SDF AA.
///
/// Two real deviations from the legacy `QUAD_SHADER`
/// (`uzor-render-wgpu-instanced/src/shaders.rs:9-111`):
///
/// 1. **Centered border** (design §3): the border band straddles the
///    SDF=0 boundary (`abs(dist) - half_w`) instead of legacy's
///    inset-only `dist + border_width`, matching `uzor-urx-cpu`'s
///    `stroke_rect_aa` CSS-centered convention.
/// 2. **Border-aware AA padding**: because the centered border extends
///    OUTSIDE the base rect by `border_width * 0.5`, the vertex
///    shader's quad-expansion padding must grow with `border_width`
///    (legacy's fixed 1px pad only ever had to cover its own inset
///    border, which never left the rect bounds).
/// 3. **Premultiplied output built directly**: fill and border
///    contributions are each formed as `color.rgb * (color.a *
///    coverage)` and composited via premultiplied src-over — never a
///    "straight" intermediate that gets multiplied by coverage a
///    second time (that double-counts coverage and silently halves it
///    at `fill.a == 0`, exactly the StrokeRect case). Required by the
///    premultiplied blend state this pipeline is built with (design §7).
pub const QUAD_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match QuadInstance in pipelines/quad.rs (56 bytes).
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
    // The centered border straddles the rect edge, extending outward
    // by border_width * 0.5 — the enclosing quad must be padded enough
    // to cover that extension (plus a 1px AA fringe), not just a fixed
    // 1px like the legacy inset-only shader.
    let aa_pad = max(1.0, instance.border_width * 0.5 + 1.0);
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
    let aa   = fwidth(dist);

    let fill_cov = 1.0 - smoothstep(-aa, aa, dist);

    // Build the fragment's PREMULTIPLIED contribution directly (never
    // round-trip through a "straight" intermediate that then gets
    // premultiplied again — mixing two straight colors weighted by a
    // coverage fraction and THEN multiplying the mix by that same
    // fraction double-counts it, silently halving the effective
    // coverage at exactly the fill-alpha-0 StrokeRect case this
    // pipeline relies on).
    var out_rgb = in.color.rgb * (in.color.a * fill_cov);
    var out_a   = in.color.a * fill_cov;

    if in.border_width > 0.0 {
        // CSS-centered stroke band straddling the SDF=0 boundary
        // (design §3) — matches `uzor-urx-cpu::stroke_rect_aa`'s
        // outer/inner split, replacing legacy's inset-only border.
        let half_w     = in.border_width * 0.5;
        let band       = abs(dist) - half_w;
        let border_cov = 1.0 - smoothstep(-aa, aa, band);
        let border_a   = in.border_color.a * border_cov;

        // Composite the border OVER the fill — standard premultiplied
        // src-over, both operands already premultiplied.
        out_rgb = in.border_color.rgb * border_a + out_rgb * (1.0 - border_a);
        out_a   = border_a + out_a * (1.0 - border_a);
    }

    if out_a <= 0.0 { discard; }
    return vec4<f32>(out_rgb, out_a);
}
"#;

/// Line shader — capsule SDF segments. Byte-identical capsule-SDF
/// algorithm to legacy's `LINE_SHADER` (design §3 confirmed parity —
/// no `sdf_capsule`/vertex-quad-expansion changes). Two real
/// deviations, both purely output-construction, same class as
/// `QUAD_SHADER_NATIVE`'s deviation 3:
///
/// 1. **Premultiplied output built directly**: `color.rgb * (color.a *
///    coverage)` instead of legacy's straight `vec4(color.rgb,
///    color.a * coverage)` — required by this pipeline's premultiplied
///    blend state (design §7); legacy's straight-alpha output is
///    correct only for `ALPHA_BLENDING`, which this pipeline never
///    uses.
/// 2. **Clip-rect discard against absolute frag position**: same as
///    legacy (`in.frag_pos` is already the world/screen-space pixel
///    coordinate here, unlike the Quad shader's rect-local
///    `frag_pos`) — called out explicitly since it could be confused
///    with the Quad shader's different `frag_pos` convention.
pub const LINE_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match LineInstance in pipelines/line.rs (56 bytes).
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
    let cov = 1.0 - smoothstep(-aa, aa, dist);
    if cov <= 0.0 { discard; }

    // Butt caps via cap_flags (matches the legacy shader's behaviour).
    // 0 = round-round, 1 = butt-start, 2 = butt-end, 3 = butt-both
    let flags = u32(in.cap_flags + 0.5);
    let dir = in.seg_end - in.seg_start;
    let len_sq = max(dot(dir, dir), 1e-6);
    let h = dot(in.frag_pos - in.seg_start, dir) / len_sq;
    if (flags & 1u) != 0u && h < 0.0 { discard; }
    if (flags & 2u) != 0u && h > 1.0 { discard; }

    // Premultiplied output built directly (design §7) — the pipeline's
    // blend state expects this, unlike legacy's straight-alpha output.
    let a = in.color.a * cov;
    if a <= 0.0 { discard; }
    return vec4<f32>(in.color.rgb * a, a);
}
"#;

/// Path/triangle shader — flat- or per-vertex-gradient-coloured
/// triangles from lyon tessellation, deliberately WITHOUT the legacy
/// crate's per-triangle barycentric edge AA (design §4 "AA decision"):
/// that scheme fades every triangle edge independently, including
/// INTERNAL tessellation seams shared by two triangles from the same
/// fill/stroke, producing visible seam artefacts on curved/concave
/// paths (the literal seam bug this pipeline exists to fix). MSAA
/// (already armed from Commit 1) supplies the antialiasing instead —
/// both at the shape's true outer boundary and at every internal
/// tessellation seam, since MSAA operates on the actual triangle
/// geometry rather than a per-triangle distance heuristic.
pub const PATH_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match TriInstance in pipelines/path.rs (56 bytes).
struct TriInstance {
    @location(0) v0:            vec2<f32>,
    @location(1) v1:            vec2<f32>,
    @location(2) v2:            vec2<f32>,
    @location(3) color0_packed: u32,
    @location(4) color1_packed: u32,
    @location(5) color2_packed: u32,
    @location(6) _pad0:         f32,
    @location(7) clip_rect:     vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color:     vec4<f32>,
    @location(1) clip_rect: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    inst: TriInstance,
) -> VertexOut {
    var px: vec2<f32>;
    var color: vec4<f32>;
    switch vi {
        case 0u: { px = inst.v0; color = unpack4x8unorm(inst.color0_packed); }
        case 1u: { px = inst.v1; color = unpack4x8unorm(inst.color1_packed); }
        case 2u: { px = inst.v2; color = unpack4x8unorm(inst.color2_packed); }
        default: { px = inst.v0; color = unpack4x8unorm(inst.color0_packed); }
    }

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    // Interpolated across the triangle for free via this vertex→fragment
    // varying — no explicit lerp code needed (design §4).
    out.color     = color;
    out.clip_rect = inst.clip_rect;
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
    // Premultiplied output built directly (design §7), same discipline
    // as Quad/Line.
    let a = in.color.a;
    if a <= 0.0 { discard; }
    return vec4<f32>(in.color.rgb * a, a);
}
"#;

/// Glyph shader — textured quads sampling `NativeGlyphAtlas`'s R8
/// coverage texture (design §5). Two real deviations from legacy's
/// `GLYPH_SHADER` (`uzor-render-wgpu-instanced/src/shaders.rs:114-179`),
/// both required by this crate's premultiplied-blend doctrine:
///
/// 1. **TriangleList, not TriangleStrip.** Every native pipeline in
///    this crate uses `PrimitiveTopology::TriangleList` + a 6-vertex
///    procedural quad (the same `quad_vert_pos`-shaped helper Quad/Line
///    use, copied here — not imported, WGSL has no cross-shader-module
///    imports in this crate's inline-string style).
/// 2. **Premultiply BOTH rgb and alpha in the fragment shader**, not a
///    straight `vec4(color.rgb, color.a * alpha)` output. Legacy emits
///    straight alpha, correct only paired with legacy's straight-alpha
///    blend state; this pipeline's blend state is premultiplied (Quad's
///    `premultiplied_blend_state()`), so both channels must be
///    premultiplied here — matching every other native shader's
///    "straight-in, premultiply-in-shader" convention.
///
/// **Clip-rect discard against the ABSOLUTE screen position**
/// (`in.clip_pos.xy`, the `@builtin(position)` output) — same
/// convention as `QUAD_SHADER_NATIVE`'s `px_abs` (a separate `uv`
/// varying carries the LOCAL atlas-sample coordinate, analogous to
/// Quad's rect-local `frag_pos`; the two must never be conflated, this
/// shader keeps them as two distinct fields for exactly that reason).
///
/// **Sampler**: `wgpu::FilterMode::Linear` mag/min, `Nearest` mipmap —
/// identical to legacy's atlas sampler (`text_atlas.rs:69-75`), set up
/// by `NativeGlyphAtlas::new` (Wave 2 Commit 1). 1px padding on every
/// glyph allocation (`atlas.rs::get_or_insert`) prevents this filter
/// from bleeding a neighbour glyph's coverage into an edge sample.
pub const GLYPH_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0) var atlas_tex:     texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// Instance data — must match GlyphInstance in pipelines/glyph.rs (56 bytes).
struct GlyphInstance {
    @location(0) pos:         vec2<f32>,
    @location(1) size:        vec2<f32>,
    @location(2) uv_pos:      vec2<f32>,
    @location(3) uv_size:     vec2<f32>,
    @location(4) color_packed: u32,
    @location(5) _pad0:       f32,
    @location(6) clip_rect:   vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) color:     vec4<f32>,
    @location(2) clip_rect: vec4<f32>,
};

fn quad_vert_pos(vertex_index: u32) -> vec2<f32> {
    let xs = array<f32, 6>(0.0, 1.0, 0.0,  1.0, 1.0, 0.0);
    let ys = array<f32, 6>(0.0, 0.0, 1.0,  0.0, 1.0, 1.0);
    return vec2<f32>(xs[vertex_index], ys[vertex_index]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: GlyphInstance,
) -> VertexOut {
    let uv_local = quad_vert_pos(vertex_index);
    let px = instance.pos + uv_local * instance.size;

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.clip_pos  = vec4<f32>(ndc, 0.0, 1.0);
    out.uv        = instance.uv_pos + uv_local * instance.uv_size;
    out.color     = unpack4x8unorm(instance.color_packed);
    out.clip_rect = instance.clip_rect;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let px_abs = in.clip_pos.xy;
    let cr = in.clip_rect;
    if px_abs.x < cr.x || px_abs.y < cr.y
       || px_abs.x > cr.x + cr.z || px_abs.y > cr.y + cr.w {
        discard;
    }
    let coverage = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    if coverage < 0.0039 { discard; } // < 1/255 — same early-out legacy uses

    // Premultiply BOTH rgb and alpha here — `in.color` arrives STRAIGHT
    // (non-premultiplied brush colour, `GlyphInstance.color`); the
    // pipeline's blend state expects a premultiplied fragment output
    // (design §5, same discipline as Quad/Line/Path).
    let premul_rgb = in.color.rgb * in.color.a;
    let out_rgb = premul_rgb * coverage;
    let out_a   = in.color.a * coverage;
    if out_a <= 0.0 { discard; }
    return vec4<f32>(out_rgb, out_a);
}
"#;

/// Stencil mask-write shader — position + `clip_rect` discard ONLY
/// (design §2.3). Reuses `TriInstance`'s wire layout verbatim
/// (`pipelines/path.rs::tri_instance_layout`) so the mask-write
/// pipeline pair needs no new instance struct; `color0/1/2` are
/// present in the vertex-input layout (byte-compatibility with
/// `TriInstance`) but never read here — a mask write never touches
/// color (`ColorWrites::empty()` on the pipeline's color target blocks
/// the fragment's return value from ever reaching the attachment
/// regardless of what this shader returns). The fragment stage exists
/// purely to let the stencil TEST/OP machinery run per-fragment; its
/// dummy `vec4(1.0)` return is never observed.
pub const STENCIL_MASK_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Instance data — must match TriInstance in pipelines/path.rs (56
// bytes). color0/1/2 + _pad0 are declared for byte-layout parity with
// TriInstance's vertex buffer layout but never read.
struct TriInstance {
    @location(0) v0:            vec2<f32>,
    @location(1) v1:            vec2<f32>,
    @location(2) v2:            vec2<f32>,
    @location(3) color0_packed: u32,
    @location(4) color1_packed: u32,
    @location(5) color2_packed: u32,
    @location(6) _pad0:         f32,
    @location(7) clip_rect:     vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) clip_rect: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    inst: TriInstance,
) -> VertexOut {
    var px: vec2<f32>;
    switch vi {
        case 0u: { px = inst.v0; }
        case 1u: { px = inst.v1; }
        case 2u: { px = inst.v2; }
        default: { px = inst.v0; }
    }

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    out.clip_rect = inst.clip_rect;
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
    return vec4<f32>(1.0, 1.0, 1.0, 1.0); // dummy — ColorWrites::empty() blocks this from ever landing
}
"#;

/// Blend-layer composite shader — full-viewport procedural quad
/// sampling a just-closed layer's resolved RGBA texture, scaled by the
/// layer's own `alpha` (design §3.6, Wave 3 Commit 3). See
/// `pipelines::blend_composite`'s module doc for the corrected
/// premultiply formula (`out = texel * alpha`, NOT `texel.rgb *
/// texel.a` re-multiplied — the sampled texel is ALREADY premultiplied,
/// having been resolved from content rendered through this crate's own
/// premultiplied blend state).
pub const BLEND_COMPOSITE_SHADER_NATIVE: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0) var layer_tex:     texture_2d<f32>;
@group(1) @binding(1) var layer_sampler: sampler;

// Instance data — must match BlendCompositeInstance in
// pipelines/blend_composite.rs (32 bytes).
struct BlendCompositeInstance {
    @location(0) alpha:     f32,
    @location(1) _pad:      vec3<f32>,
    @location(2) clip_rect: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) alpha:     f32,
    @location(2) clip_rect: vec4<f32>,
};

fn quad_vert_pos(vertex_index: u32) -> vec2<f32> {
    let xs = array<f32, 6>(0.0, 1.0, 0.0,  1.0, 1.0, 0.0);
    let ys = array<f32, 6>(0.0, 0.0, 1.0,  0.0, 1.0, 1.0);
    return vec2<f32>(xs[vertex_index], ys[vertex_index]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: BlendCompositeInstance,
) -> VertexOut {
    // Full-viewport quad (design §0.2 — no position/size fields on the
    // instance at all; the layer's resolve texture IS viewport-sized,
    // so `uv_local` maps 1:1 onto it).
    let uv_local = quad_vert_pos(vertex_index);
    let px = uv_local * uniforms.screen_size;

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.clip_pos  = vec4<f32>(ndc, 0.0, 1.0);
    out.uv        = uv_local;
    out.alpha     = instance.alpha;
    out.clip_rect = instance.clip_rect;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let px_abs = in.clip_pos.xy;
    let cr = in.clip_rect;
    if px_abs.x < cr.x || px_abs.y < cr.y
       || px_abs.x > cr.x + cr.z || px_abs.y > cr.y + cr.w {
        discard;
    }
    // `texel` is ALREADY premultiplied (resolved from content rendered
    // through the premultiplied blend state every native pipeline
    // shares) — scaling an already-premultiplied value by a scalar is a
    // uniform multiply across ALL 4 channels, same reasoning
    // `uzor-urx-cpu::blend::composite_layer_srcover` uses. NOT
    // `texel.rgb * texel.a` re-premultiplied (design §3.6's sketch —
    // wrong for this data flow, see this file's module doc).
    let texel = textureSample(layer_tex, layer_sampler, in.uv);
    let out_rgba = texel * in.alpha;
    if out_rgba.a <= 0.0 { discard; }
    return out_rgba;
}
"#;
