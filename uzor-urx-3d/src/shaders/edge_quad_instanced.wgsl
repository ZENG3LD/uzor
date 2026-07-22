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
//
// ── Per-instance width scale (2026-07-22 3D-parity-arc final wave) ──────
//
// `scale.x`/`scale.z` of a Line node's model matrix are otherwise
// UNUSED by this shader: only `model * (0,0,0,1)` and `model * (0,1,0,1)`
// are ever read to recover `from`/`to`, and both `(0,0,0)` and `(0,1,0)`
// are invariant to `scale.x`/`scale.z` entirely (they only ever multiply
// the mesh-local X/Z axes, which the two probed points never carry a
// nonzero component along). `scale.x` is therefore repurposed to carry a
// PER-INSTANCE width multiplier: `Quat::from_rotation_arc` always
// returns a unit (length-preserving) rotation, so `length(model_c0.xyz)`
// (the model matrix's first column) exactly recovers `scale.x`
// regardless of the edge's own world-space direction — `model_c0 =
// rotation * (scale.x, 0, 0)`, and a pure rotation preserves vector
// length. `edge_params.z` (the per-frame BASE width_px uniform, still
// `Renderer3D::set_edge_width_px`'s own knob, default `~1.75px`) is
// multiplied by this recovered per-instance scale in the vertex shader,
// then carried to the fragment shader through the new `width_px`
// varying below (NOT re-read from `edge_params` in the fragment stage —
// the fragment's own analytic-AA feather must center on the SAME
// per-instance-scaled half-width the vertex stage already used to
// place the geometry, not the raw uniform alone, or the feather band
// and the actual quad extent would disagree). A caller that never moves
// `scale.x` away from its historical `1.0` default (`uzor_graph::render3d::
// build_cluster_edge_instances`/grid lines/every other `Node::new_line`
// call site the per-instance-width wave did NOT touch) recovers
// `width_scale = 1.0` exactly — zero pixel/behavior change for those
// paths, and likewise for a graph edge of weight `1.0`
// (`uzor_graph::render3d::edge_width_scale(1.0) == 1.0`) — the
// byte/pixel-compatibility the wave's own gate required.
//
// ── Round caps via distance-to-SEGMENT, not distance-to-centerline
// (same wave) ────────────────────────────────────────────────────────
//
// A node-link graph's edges are independent 2-endpoint segments with no
// polyline join geometry to build — but a THICK edge's own quad
// previously ended in a flat, perpendicular BUTT cut exactly at `from`/
// `to` (the geometry never extended past either endpoint at all), which
// left a visible wedge-shaped gap/notch where two differently-angled
// thick edges met at a shared node. The standard cheap fix: extend the
// quad by `half_width` (the SAME feather-inclusive half-width the
// perpendicular expansion already uses) past BOTH endpoints along the
// segment's own screen-space direction, and compute per-fragment
// distance to the SEGMENT (clamped to `[0, seg_len_px]` along that
// axis) rather than distance to the infinite centerline alone — the
// round cap falls out of the SAME smoothstep coverage function for
// free, no separate cap-drawing pass needed. Every joint in this
// engine's graphs is covered either by a node sphere or by this round
// cap, which is the complete "joins" answer for independent node-link
// segments (there is never a third edge meeting at a non-node point to
// need a real miter/bevel join).

struct Frame {
    view_proj:       mat4x4<f32>,
    eye:             vec4<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params:   vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
// edge_params.x = viewport width (px), .y = viewport height (px),
// .z = width_px (full line width in device pixels, the BASE/default
// value — an instance's own `scale.x` multiplies this, see the module
// doc above), .w = unused.
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
    // Signed pixel distance ALONG the segment's own direction, measured
    // from `from` (0 at `from`, `seg_len_px` at `to`, negative/`>
    // seg_len_px` inside the half-width-extended cap regions) — the
    // round-cap ingredient (see the module doc above). Interpolated
    // linearly for the same reason `dist_px` is: the extended quad's
    // edges stay aligned to the SAME fixed (per-instance) screen-space
    // direction basis every vertex's offset was built from.
    @location(2) along_px: f32,
    // This instance's own screen-space segment length (px) — constant
    // across all 4 vertices of one instance, carried purely so the
    // fragment stage can clamp `along_px` into `[0, seg_len_px]` without
    // a second uniform round-trip.
    @location(3) seg_len_px: f32,
    // This instance's own EFFECTIVE (base uniform × per-instance scale)
    // line width in device pixels — see the module doc's "per-instance
    // width scale" section for why the fragment stage needs this rather
    // than re-deriving it from `edge_params.z` alone.
    @location(4) width_px: f32,
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
        out.along_px = 0.0;
        out.seg_len_px = 0.0;
        out.width_px = 0.0;
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
    let seg_len_px = length(dir_px);
    var normal_px = vec2<f32>(0.0, 1.0);
    var dir_unit_px = vec2<f32>(1.0, 0.0);
    if (seg_len_px > 1.0e-5) {
        dir_unit_px = dir_px / seg_len_px;
        normal_px = vec2<f32>(-dir_unit_px.y, dir_unit_px.x);
    }

    // Per-instance width scale (module doc above): `scale.x` survives in
    // the model matrix's first column as `rotation * (scale.x, 0, 0)`,
    // and `Quat::from_rotation_arc` is always a unit (length-preserving)
    // rotation, so its magnitude recovers `scale.x` exactly.
    let width_scale = length(in.model_c0.xyz);
    let width_px = edge_params.z * width_scale;
    let half_width = width_px * 0.5 + FEATHER_PX * 0.5;

    // Round caps: extend this vertex's own along-segment position by
    // `half_width` PAST whichever endpoint it belongs to (backward past
    // `from`, forward past `to`) so there is real geometry for the
    // fragment stage's distance-to-segment falloff to feather across.
    let along_sign = select(-1.0, 1.0, is_to);
    let offset_px = normal_px * in.corner.x * half_width + dir_unit_px * along_sign * half_width;
    // Pixel space is y-down; NDC is y-up — flip back on the way out.
    let offset_ndc = vec2<f32>(offset_px.x, -offset_px.y) / viewport_size * 2.0;

    out.clip = vec4<f32>(self_clip.xy + offset_ndc * self_clip.w, self_clip.z, self_clip.w);
    out.color = in.color * in.tint;
    out.dist_px = in.corner.x * half_width;
    out.along_px = select(0.0, seg_len_px, is_to) + along_sign * half_width;
    out.seg_len_px = seg_len_px;
    out.width_px = width_px;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let half_width = in.width_px * 0.5;
    let feather = FEATHER_PX * 0.5;
    // True distance-to-SEGMENT, not distance-to-infinite-centerline: the
    // along-axis term is only ever nonzero PAST an endpoint (inside the
    // half-width-extended cap region the vertex shader built above) —
    // this is exactly what turns a butt cut into a round cap for free.
    let along_overshoot = max(-in.along_px, 0.0) + max(in.along_px - in.seg_len_px, 0.0);
    let d = sqrt(in.dist_px * in.dist_px + along_overshoot * along_overshoot);
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
