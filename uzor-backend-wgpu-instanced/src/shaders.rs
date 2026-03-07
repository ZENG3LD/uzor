//! WGSL shader source strings for the instanced renderer.
//!
//! Two shaders are provided:
//! - `QUAD_SHADER`: renders filled/bordered rounded rectangles with SDF AA.
//! - `LINE_SHADER`: renders line segments with capsule SDF AA.
//!
//! Both shaders share the same `Uniforms` bind group layout (binding 0) that
//! provides the `screen_size` in logical pixels used for NDC conversion.

/// Shader for rendering quad (rectangle) instances with rounded corners.
///
/// Vertex stage: emits 6 vertices (2 triangles) per instance, expanding the
/// bounding box by 1 px on every side to allow anti-aliased edges.
///
/// Fragment stage: evaluates a rounded-rectangle SDF, discards fragments
/// outside the clip rectangle, and outputs anti-aliased color.
pub const QUAD_SHADER: &str = r#"
// ── Uniforms ───────────────────────────────────────────────────────────────
struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ── Instance data (matches QuadInstance in instances.rs) ──────────────────
struct QuadInstance {
    @location(0) pos:          vec2<f32>,   // top-left in px
    @location(1) size:         vec2<f32>,   // width × height in px
    @location(2) color:        vec4<f32>,   // fill RGBA
    @location(3) corner_radius: f32,
    @location(4) border_width:  f32,
    @location(5) _pad0:        vec2<f32>,
    @location(6) border_color: vec4<f32>,
    @location(7) clip_rect:    vec4<f32>,   // x, y, w, h in px
};

// ── Vertex output ─────────────────────────────────────────────────────────
struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    // pixel-space position of this fragment relative to quad's top-left
    @location(0) frag_pos:  vec2<f32>,
    // quad data forwarded to fragment
    @location(1) size:         vec2<f32>,
    @location(2) color:        vec4<f32>,
    @location(3) corner_radius: f32,
    @location(4) border_width:  f32,
    @location(5) border_color: vec4<f32>,
    @location(6) clip_rect:    vec4<f32>,
};

// Six vertex positions for a unit quad [0,1]², two triangles
fn quad_vert_pos(vertex_index: u32) -> vec2<f32> {
    // indices:  0,1,2  3,4,5
    // tris:     TL,TR,BL  TR,BR,BL
    let xs = array<f32, 6>(0.0, 1.0, 0.0,  1.0, 1.0, 0.0);
    let ys = array<f32, 6>(0.0, 0.0, 1.0,  0.0, 1.0, 1.0);
    return vec2<f32>(xs[vertex_index], ys[vertex_index]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: QuadInstance,
) -> VertexOut {
    let aa_pad = 1.0;   // 1-pixel AA fringe on each side
    let padded_pos  = instance.pos  - vec2<f32>(aa_pad, aa_pad);
    let padded_size = instance.size + vec2<f32>(aa_pad * 2.0, aa_pad * 2.0);

    let uv = quad_vert_pos(vertex_index);
    let px = padded_pos + uv * padded_size;

    // frag_pos = position relative to the un-padded quad's top-left
    let frag_pos = px - instance.pos;

    // NDC: top-left = (-1, 1), bottom-right = (1, -1)
    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.clip_pos     = vec4<f32>(ndc, 0.0, 1.0);
    out.frag_pos     = frag_pos;
    out.size         = instance.size;
    out.color        = instance.color;
    out.corner_radius = instance.corner_radius;
    out.border_width  = instance.border_width;
    out.border_color  = instance.border_color;
    out.clip_rect     = instance.clip_rect;
    return out;
}

// ── SDF helpers ───────────────────────────────────────────────────────────

/// Signed distance to a rounded rectangle centred at the origin.
/// `half` = half-extents, `r` = corner radius.
fn sdf_rounded_rect(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - half + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // ── Clip discard ──────────────────────────────────────────────────────
    // @builtin(position) in the fragment stage is framebuffer pixel coords
    // (x, y with origin at top-left), NOT NDC.
    let px_abs = in.clip_pos.xy;
    let cr = in.clip_rect;
    if px_abs.x < cr.x || px_abs.y < cr.y
       || px_abs.x > cr.x + cr.z || px_abs.y > cr.y + cr.w {
        discard;
    }

    // ── Rounded rect SDF ──────────────────────────────────────────────────
    let half = in.size * 0.5;
    let p    = in.frag_pos - half;   // position relative to centre
    let r    = clamp(in.corner_radius, 0.0, min(half.x, half.y));
    let dist = sdf_rounded_rect(p, half, r);

    // outer edge AA
    let aa     = fwidth(dist);
    let fill_a = 1.0 - smoothstep(-aa, aa, dist);
    if fill_a <= 0.0 { discard; }

    // ── Border ────────────────────────────────────────────────────────────
    var out_color = in.color;
    if in.border_width > 0.0 {
        let inner_dist = dist + in.border_width;
        let border_a   = 1.0 - smoothstep(-aa, aa, inner_dist);
        // blend: inside border region → border_color
        let on_border = clamp(border_a - (1.0 - smoothstep(-aa, aa, dist + 0.5)), 0.0, 1.0);
        out_color = mix(in.color, in.border_color, on_border);
        // Also blend the fill alpha
        out_color.a = max(in.color.a, in.border_color.a * on_border);
    }

    out_color.a *= fill_a;
    return out_color;
}
"#;

/// Shader for rendering line segment instances with capsule SDF.
///
/// Vertex stage: builds an oriented quad that encloses the segment, expanded
/// by `width/2 + 1 px` for AA fringe.
///
/// Fragment stage: evaluates capsule SDF, discards outside clip, outputs AA.
/// Shader for rendering glyph (text) instances sampled from a R8Unorm atlas.
///
/// Vertex stage: builds a screen-space quad from `vertex_index` (0–3,
/// triangle-strip) using `pos` and `size` from the instance buffer.
///
/// Fragment stage: samples the atlas, multiplies by text color, discards
/// fragments outside the clip rectangle.
pub const GLYPH_SHADER: &str = r#"
// ── Uniforms (same layout as quad/line shaders) ────────────────────────────
struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ── Atlas texture + sampler ────────────────────────────────────────────────
@group(1) @binding(0) var atlas_tex:     texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

// ── Instance data (matches GlyphInstance in glyph_instance.rs) ────────────
struct GlyphInstance {
    @location(0) pos:       vec2<f32>,  // screen top-left of the quad in px
    @location(1) size:      vec2<f32>,  // quad width×height in px
    @location(2) uv_pos:    vec2<f32>,  // atlas UV top-left  (0..1)
    @location(3) uv_size:   vec2<f32>,  // atlas UV size      (0..1)
    @location(4) color:     vec4<f32>,  // text RGBA
    @location(5) clip_rect: vec4<f32>,  // x, y, w, h in px
};

// ── Vertex output ─────────────────────────────────────────────────────────
struct GlyphVsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv:        vec2<f32>,
    @location(1) color:     vec4<f32>,
    @location(2) clip_rect: vec4<f32>,
    @location(3) frag_pos:  vec2<f32>,  // screen-space pixel position
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

    // NDC: top-left = (-1, +1), bottom-right = (+1, -1)
    let ndc = vec2<f32>(
        pixel.x / uniforms.screen_size.x *  2.0 - 1.0,
        1.0 - pixel.y / uniforms.screen_size.y * 2.0,
    );

    var out: GlyphVsOut;
    out.position  = vec4<f32>(ndc, 0.0, 1.0);
    out.uv        = input.uv_pos + vec2<f32>(x, y) * input.uv_size;
    out.color     = input.color;
    out.clip_rect = input.clip_rect;
    out.frag_pos  = pixel;
    return out;
}

@fragment
fn fs_glyph(in: GlyphVsOut) -> @location(0) vec4<f32> {
    // ── Clip discard (clip_rect is x, y, w, h) ────────────────────────────
    let cr = in.clip_rect;
    if in.frag_pos.x < cr.x || in.frag_pos.y < cr.y
       || in.frag_pos.x > cr.x + cr.z || in.frag_pos.y > cr.y + cr.w {
        discard;
    }

    // ── Sample atlas (R8Unorm → .r is the alpha mask) ─────────────────────
    let alpha = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

/// Shader for rendering filled triangle instances with barycentric edge AA.
///
/// Vertex stage: selects one of the three triangle vertices based on `vertex_index`
/// (0, 1, 2) and emits barycentric coordinates for edge anti-aliasing.
///
/// Fragment stage: discards fragments outside the clip rect, then applies
/// smooth edge AA via barycentric minimum distance.
pub const TRIANGLE_SHADER: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct TriangleInstance {
    @location(0) v0:        vec2<f32>,
    @location(1) v1:        vec2<f32>,
    @location(2) v2:        vec2<f32>,
    @location(3) _pad0:     vec2<f32>,
    @location(4) color:     vec4<f32>,
    @location(5) clip_rect: vec4<f32>,
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
    out.color     = inst.color;
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

    // Flat color — no edge AA for triangle fan fill.
    // Barycentric AA causes visible seams on internal fan edges.
    return in.color;
}
"#;

pub const LINE_SHADER: &str = r#"
// ── Uniforms ───────────────────────────────────────────────────────────────
struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ── Instance data (matches LineInstance in instances.rs) ──────────────────
struct LineInstance {
    @location(0) start:     vec2<f32>,
    @location(1) end:       vec2<f32>,
    @location(2) color:     vec4<f32>,
    @location(3) width:     f32,
    @location(4) cap_flags: f32,   // 0=round-round, 1=butt-start, 2=butt-end, 3=butt-both
    @location(5) _pad0:     vec2<f32>,
    @location(6) clip_rect: vec4<f32>,
};

// ── Vertex output ─────────────────────────────────────────────────────────
struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) frag_world: vec2<f32>,   // fragment in pixel-space
    @location(1) seg_start:  vec2<f32>,
    @location(2) seg_end:    vec2<f32>,
    @location(3) color:      vec4<f32>,
    @location(4) half_width: f32,
    @location(5) clip_rect:  vec4<f32>,
    @location(6) cap_flags:  f32,
};

// Signs for the 6 vertices of the oriented quad
fn quad_sign(idx: u32) -> vec2<f32> {
    //       along   across
    let alongs  = array<f32, 6>(-1.0,  1.0, -1.0,   1.0,  1.0, -1.0);
    let acrosss = array<f32, 6>(-1.0, -1.0,  1.0,  -1.0,  1.0,  1.0);
    return vec2<f32>(alongs[idx], acrosss[idx]);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    inst: LineInstance,
) -> VertexOut {
    let aa_pad   = 1.5;
    let hw       = inst.width * 0.5 + aa_pad;

    let dir = inst.end - inst.start;
    let len = length(dir);
    var along  = select(vec2<f32>(1.0, 0.0), dir / len, len > 0.0001);
    let across = vec2<f32>(-along.y, along.x);

    let sign = quad_sign(vertex_index);
    // expand from midpoint outward: along ± hw along the segment axis,
    // across ± hw perpendicular
    let mid  = (inst.start + inst.end) * 0.5;
    let half_len = len * 0.5 + hw;    // extends past endpoints for capsule cap
    let px   = mid
               + along  * (sign.x * half_len)
               + across * (sign.y * hw);

    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x *  2.0 - 1.0,
        px.y / uniforms.screen_size.y * -2.0 + 1.0,
    );

    var out: VertexOut;
    out.clip_pos   = vec4<f32>(ndc, 0.0, 1.0);
    out.frag_world = px;
    out.seg_start  = inst.start;
    out.seg_end    = inst.end;
    out.color      = inst.color;
    out.half_width = inst.width * 0.5;
    out.clip_rect  = inst.clip_rect;
    out.cap_flags  = inst.cap_flags;
    return out;
}

// ── SDF helpers ───────────────────────────────────────────────────────────

/// Signed distance to a line segment with configurable cap styles.
///
/// Uses the capsule SDF as the base, then adds half-plane clip constraints
/// for butt caps. A butt cap clips the capsule flat at the endpoint, creating
/// a hard perpendicular cutoff that eliminates round-cap overlap artifacts
/// at interior polyline joints.
///
/// cap_flags:
///   0.0 = round caps at both ends (standard capsule)
///   1.0 = butt cap at start (a), round cap at end (b)
///   2.0 = round cap at start (a), butt cap at end (b)
///   3.0 = butt caps at both ends (interior segment)
fn sdf_line_caps(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, r: f32, cap_flags: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    // Project p onto the segment: t in [0,1] is along the segment.
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.0001), 0.0, 1.0);
    // Capsule SDF: distance to nearest point on the segment minus radius.
    var d = length(pa - ba * h) - r;

    // For butt caps, clip the SDF with a half-plane at the endpoint.
    // max(d, -t_along) clips the capsule's round bulge past the endpoint.
    let len = length(ba);
    let along_dir = ba / max(len, 0.0001);
    let t = dot(pa, along_dir);  // signed distance along segment from a

    let butt_start = (cap_flags == 1.0 || cap_flags == 3.0);
    let butt_end   = (cap_flags == 2.0 || cap_flags == 3.0);

    if butt_start { d = max(d, -t); }           // clip before start endpoint
    if butt_end   { d = max(d, t - len); }      // clip after end endpoint

    return d;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // ── Clip discard ──────────────────────────────────────────────────────
    let cr = in.clip_rect;
    if in.frag_world.x < cr.x || in.frag_world.y < cr.y
       || in.frag_world.x > cr.x + cr.z || in.frag_world.y > cr.y + cr.w {
        discard;
    }

    // ── Line SDF with configurable caps ───────────────────────────────────
    let dist = sdf_line_caps(in.frag_world, in.seg_start, in.seg_end, in.half_width, in.cap_flags);
    // Clamp minimum AA kernel to 1px — prevents grainy diagonal lines
    // when fwidth underestimates at non-axis-aligned angles.
    let aa   = max(fwidth(dist), 1.0);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    if alpha < 0.01 { discard; }

    var out_color = in.color;
    out_color.a *= alpha;
    return out_color;
}
"#;
