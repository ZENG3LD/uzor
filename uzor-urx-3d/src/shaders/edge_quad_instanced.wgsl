// Screen-space billboarded edge quad — round 2 of the edge-quality
// overhaul (`uzor-graph/CLAUDE.md`'s divergence log). REPLACES the
// round-1 hardware `LineList` approach: wgpu/DX12/Vulkan line
// rasterization is BINARY coverage (a sample is either fully in or
// fully out of the 1-device-pixel-wide line, no partial-coverage
// blending of its own) and whether/how MSAA touches lines at all is
// implementation-defined per the underlying D3D12/Vulkan specs — so a
// hardware line stays visibly aliased/crooked up close no matter how
// MSAA is tuned, since MSAA only ever resolves TRIANGLE coverage, not
// a rasterizer's own line-fill decision. This shader instead expands an
// ordinary TRIANGLE quad (`Mesh::unit_edge_quad`) to a constant PIXEL
// width in screen space and applies its OWN analytic alpha falloff in
// the fragment shader (a smoothstep coverage function, independent of
// any hardware AA) — the three.js `Line2` / cosmos.gl approach, doable
// here without join geometry because graph edges are independent
// straight 2-endpoint segments, never a connected polyline.
//
// Per-frame: Frame (view_proj etc, binding 0) + edge params (viewport
// size + width_px, binding 1).
// Per-vertex (mesh-local, `Mesh::unit_edge_quad`): `corner.x` ∈ {-1,+1}
// selects which SIDE of the segment this corner expands toward,
// `corner.y` ∈ {0,1} selects which ENDPOINT (0 = `from`, 1 = `to`).
// Per-instance: model matrix + tint (SAME `InstanceRaw` layout every
// other instanced pipeline in this crate uses) — `model * (0,0,0,1)`
// and `model * (0,1,0,1)` recover the edge's world-space `from`/`to`
// endpoints, mirroring the exact convention the old `unit_line` mesh
// established (`translation = from`, `rotation =
// Quat::from_rotation_arc(Vec3::Y, dir)`, `scale.y = length`).

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params:   vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
// edge_params.x = viewport width (px), .y = viewport height (px),
// .z = width_px (full line width in device pixels), .w = unused.
@group(0) @binding(1) var<uniform> edge_params: vec4<f32>;

// Feather band width (device pixels), straddling the nominal edge of
// the line (distance `width_px * 0.5` from the centerline) — the
// analytic antialiasing this shader supplies in place of hardware line
// AA. Geometry is expanded by half of this beyond the nominal
// half-width so there is room for the falloff to actually happen.
const FEATHER_PX: f32 = 1.0;
// Smallest clip-space `w` a vertex is allowed to keep — anything at or
// below this is behind (or exactly on) the camera's near plane and
// gets trimmed toward the other endpoint first (see `trim_near`).
const NEAR_W: f32 = 1.0e-4;

struct VsIn {
    // x = side (-1/+1), y = endpoint (0 = from, 1 = to), z unused.
    @location(0) corner: vec3<f32>,
    @location(1) color:  vec4<f32>,
    // Instance model-matrix columns + tint (locations 2..6 — same
    // layout every other instanced pipeline in this crate shares).
    @location(2) model_c0: vec4<f32>,
    @location(3) model_c1: vec4<f32>,
    @location(4) model_c2: vec4<f32>,
    @location(5) model_c3: vec4<f32>,
    @location(6) tint:     vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Signed pixel distance from the segment's own centerline —
    // interpolated linearly across the (screen-space rectangular) quad,
    // exact at every fragment since both long edges of the quad share
    // the SAME screen-space direction (computed identically at both
    // endpoints below).
    @location(1) dist_px: f32,
};

// Move clip-space position `p` toward `other` until its `w` reaches
// `NEAR_W` — the standard robust near-plane trim for a straight
// 2-point segment. Valid because clip position is an affine function
// of world position along a straight line, so linear interpolation in
// clip space exactly reproduces the clip position of the corresponding
// world-space point on the segment.
fn trim_near(p: vec4<f32>, other: vec4<f32>) -> vec4<f32> {
    if (p.w >= NEAR_W || other.w < NEAR_W) {
        return p;
    }
    let t = (NEAR_W - p.w) / (other.w - p.w);
    return mix(p, other, t);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let model = mat4x4<f32>(in.model_c0, in.model_c1, in.model_c2, in.model_c3);
    let world_from = model * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let world_to   = model * vec4<f32>(0.0, 1.0, 0.0, 1.0);
    var clip_from = frame.view_proj * world_from;
    var clip_to   = frame.view_proj * world_to;

    if (clip_from.w < NEAR_W && clip_to.w < NEAR_W) {
        // Whole segment behind the eye — push this vertex fully outside
        // the clip volume so it contributes nothing (a w=0 position
        // would otherwise divide-by-zero on the perspective divide).
        out.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        out.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.dist_px = 0.0;
        return out;
    }
    clip_from = trim_near(clip_from, clip_to);
    clip_to   = trim_near(clip_to, clip_from);

    let is_to = in.corner.y > 0.5;
    let self_clip  = select(clip_from, clip_to, is_to);
    let other_clip = select(clip_to, clip_from, is_to);

    // Pixel-space (post-divide) positions — the perpendicular direction
    // MUST be computed after the perspective divide, in real screen
    // pixels, or a foreshortened endpoint would skew the on-screen
    // angle the quad expands along.
    let viewport_size = edge_params.xy;
    let self_ndc  = self_clip.xy  / self_clip.w;
    let other_ndc = other_clip.xy / other_clip.w;
    let self_px  = (self_ndc  * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5)) * viewport_size;
    let other_px = (other_ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5)) * viewport_size;

    // Direction always points from `from` toward `to` on screen,
    // regardless of which endpoint THIS vertex is — keeps the
    // perpendicular's handedness consistent across both ends of the
    // same segment so the quad doesn't twist.
    var dir_px = self_px - other_px;
    if (!is_to) {
        dir_px = other_px - self_px;
    }
    let len = length(dir_px);
    var normal_px = vec2<f32>(0.0, 1.0);
    if (len > 1.0e-5) {
        let d = dir_px / len;
        normal_px = vec2<f32>(-d.y, d.x);
    }

    let width_px = edge_params.z;
    let half_width = width_px * 0.5 + FEATHER_PX * 0.5;
    let offset_px = normal_px * in.corner.x * half_width;
    // Pixel space is y-down; NDC is y-up — flip back on the way out.
    let offset_ndc = vec2<f32>(offset_px.x, -offset_px.y) / viewport_size * 2.0;

    out.clip = vec4<f32>(self_clip.xy + offset_ndc * self_clip.w, self_clip.z, self_clip.w);
    out.color = in.color * in.tint;
    out.dist_px = in.corner.x * half_width;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let width_px = edge_params.z;
    let half_width = width_px * 0.5;
    let feather = FEATHER_PX * 0.5;
    let d = abs(in.dist_px);
    // Analytic AA: 1.0 well inside the nominal half-width, smoothly
    // (smoothstep) down to 0.0 by `half_width + feather` — a soft edge,
    // not the binary in/out a hardware-rasterized line would give.
    let coverage = 1.0 - smoothstep(half_width - feather, half_width + feather, d);
    let alpha = in.color.a * coverage;
    // Premultiplied output — pairs with
    // `wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING` on this
    // pipeline's color target.
    return vec4<f32>(in.color.rgb * alpha, alpha);
}
