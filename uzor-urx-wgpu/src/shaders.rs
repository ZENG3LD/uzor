//! WGSL shader source strings for the native pipelines.
//!
//! House style matches the legacy crate (`uzor-render-wgpu-instanced/src/shaders.rs`):
//! inline `pub const &str`, no `.wgsl` files. Every shader shares the
//! same `Uniforms` bind group (group 0, binding 0) carrying
//! `screen_size` in physical pixels.
//!
//! Commit 1 shipped `QUAD_SHADER_NATIVE`. Commit 2 adds
//! `LINE_SHADER_NATIVE`.

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
