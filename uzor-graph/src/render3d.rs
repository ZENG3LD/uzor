//! `build_scene` helpers — instanced sphere nodes + screen-space
//! billboarded edge quads (W3D arc plan §1.3, Wave 2; edges rebuilt
//! twice since — a GPU-native `LineList` pipeline in the Wave C
//! edge-quality overhaul, then REPLACED again by a billboarded
//! screen-space quad pipeline in Wave D, round 2 of the same overhaul,
//! after the LineList approach turned out to still alias/crook up close
//! — see both divergence notes below).
//! [`GraphEngine3D::build_scene`](crate::engine3d::GraphEngine3D::build_scene)
//! wires straight into [`build_scene`] here; the split exists so the
//! node/edge instance-construction logic is unit-testable without a
//! `GraphEngine3D` (or a GPU) at all.
//!
//! **Nodes**: one `uzor_urx_3d::Node::new_lit` per graph node, sharing
//! the caller-supplied unit-sphere `node_mesh` — translated to the
//! node's simulated `(x, y, z)`, scaled by its `radius`, tinted by its
//! category (reuses [`crate::render::category_color_default`]'s existing
//! deterministic hash-palette, converted from the 2D hex-string
//! convention to the `[f32; 4]` `uzor_urx_3d::Node::color_tint` needs).
//!
//! **Edges**: one `Node::new_line` per graph edge, sharing the
//! caller-supplied unit-edge-quad `edge_mesh` — `uzor_urx_3d`'s
//! dedicated always-alpha-blended, analytically-antialiased edge-quad
//! pipeline (`NodeMesh::Line`, Wave D — see
//! [`uzor_urx_3d::Mesh::unit_edge_quad`]'s own doc comment for the full
//! shader-level mechanics).
//!
//! ## Wave D — round 2 of the edge-quality overhaul (owner close-up
//! verdict: round-1's hardware `LineList` edges are still
//! ALIASED-crooked; large spheres show visible FACETING)
//!
//! **Diagnosis, no new headless GPU harness needed — this is a
//! documented property of the graphics APIs themselves, not something
//! this crate's own readback could have caught differently than round 1
//! already did.** wgpu's `PrimitiveTopology::LineList` compiles down to
//! the underlying platform's native line rasterizer (DX12/Vulkan/Metal
//! depending on backend) — that rasterizer's coverage decision for a
//! line is BINARY (a sample is either fully inside the 1-device-pixel
//! line or fully outside; there is no partial-coverage/analytic-AA
//! contribution from the line-fill rule itself, unlike a filled
//! triangle's edge, which DOES get antialiased by MSAA sample coverage).
//! Whether/how MSAA even TOUCHES line primitives at all is
//! IMPLEMENTATION-DEFINED per the D3D12/Vulkan specs (some drivers
//! multisample the 1px-wide coverage mask, some don't touch lines
//! specially at all) — this is exactly why round 1's own MSAA-armed
//! headless test still passed its coverage gate (a hardware line IS
//! continuously drawn) while the owner's own close-up visual verdict
//! still reported aliasing: "continuous coverage" and "antialiased
//! edge" are different properties, and round 1 only ever measured the
//! former.
//!
//! **Fix, round 2: screen-space billboarded quads with analytic AA in
//! the fragment shader — the three.js `Line2` / cosmos.gl approach,
//! previously declined in round 1 for its join-handling machinery.**
//! That objection is VOID here: graph edges are independent, single
//! straight 2-endpoint segments (never a connected polyline sharing a
//! vertex with another edge's own quad), so there is no join geometry
//! to build at all — [`uzor_urx_3d::Mesh::unit_edge_quad`]'s own doc
//! comment states this explicitly. [`build_edge_instances`] is
//! UNCHANGED from round 1 (same `Node::new_line`, same `translation =
//! from` / `rotation = Quat::from_rotation_arc(Vec3::Y, dir)` / `scale.y
//! = length` instance-transform convention) — only the shared mesh's
//! OWN constructor (`unit_edge_quad` instead of `unit_line`) and the
//! `uzor_urx_3d`-side pipeline/shader it draws through changed. The new
//! pipeline expands an ordinary `TriangleList` quad to a constant PIXEL
//! width in the VERTEX shader (viewport size + `width_px` uniform,
//! default `~1.75px`), then applies a smoothstep alpha falloff over a
//! ~1px feather band straddling the nominal edge in the FRAGMENT
//! shader, premultiplied-blended — genuinely antialiased regardless of
//! what any given driver's hardware line rasterizer would have done.
//! `DEFAULT_EDGE_WIDTH`/[`EDGE_ALPHA`] are re-tuned this wave (see
//! [`EDGE_ALPHA`]'s own doc comment) — `~0.45` alpha / `~1.75px` width
//! so the 534-node `clusters` demo fixture reads clean (thin enough
//! that the edge mass "recedes" behind the nodes, thick enough not to
//! vanish to a near-invisible hairline once the analytic feather is
//! applied).
//!
//! **Sphere faceting, same wave**: [`crate::engine3d`]'s shared
//! node-sphere tessellation (`NODE_SPHERE_RINGS`/`NODE_SPHERE_SLICES`)
//! is bumped for a smoother silhouette on large/close spheres — see
//! that module's own doc comment on those constants; normals were
//! already per-vertex (smooth), confirmed by direct read of
//! `MeshLit::sphere`, so this is a pure tessellation-density change, not
//! a shading-model change.
//!
//! ## Wave C — round 1 of the edge-quality overhaul (owner live-verdict:
//! cylinder edges were "пиздец хуйня" — fat lit pipes up close,
//! dotted/stippled breakup at distance)
//!
//! **Root cause, headless-GPU-proven** (full pixel evidence in
//! `uzor-graph/CLAUDE.md`'s divergence log): the old edge geometry was a
//! `MeshLit::cylinder` with a FIXED WORLD-SPACE radius
//! (the old `DEFAULT_EDGE_WIDTH = 0.6`). On a perspective camera, a
//! fixed world-space radius's APPARENT screen radius shrinks linearly
//! with distance — at this engine's own default orbit distance
//! (`Camera3D::default().distance = 500.0`), the cylinder's apparent
//! diameter was already only ~1px; at any greater distance (or simply a
//! longer edge spanning more of the graph) it drops well under 1px.
//! Standard (even 4×-multisampled) triangle rasterization only shades a
//! pixel where a sample point falls inside the triangle — for a
//! ROTATED, sub-pixel-wide quad (i.e. almost every real edge, since node
//! positions are effectively random, never screen-axis-aligned), that
//! sample-point test succeeds only sporadically as the quad sweeps
//! across the pixel grid at an angle, producing literal dotted/stippled
//! coverage instead of a continuous line. A headless readback proved
//! this directly: a screen-axis-aligned (broadside) thin cylinder
//! rasterized CONTINUOUSLY down to 0.52px apparent width, but the exact
//! SAME radius on a DIAGONAL edge, at the engine's own default distance,
//! left ~96% of its exact-centerline samples at background brightness
//! with only sparse isolated spikes — reproduced with 4× MSAA already
//! armed (this crate's own prior MSAA fix helps but cannot fix
//! sub-pixel-width ROTATED geometry once feature size drops under one
//! sample spacing). Depth-buffer precision and ACES/tonemap banding were
//! both ruled out: every test used production's own distance-scaled
//! `z_near`/`z_far` (see [`crate::camera3d::Camera3D::to_perspective`])
//! with a SINGLE mesh in the scene (nothing to z-fight against), and the
//! identical tonemap chain ran cleanly in the broadside case — only
//! on-screen ORIENTATION toggled the defect, which a color-space effect
//! cannot do.
//!
//! **Fix, round 1 (SUPERSEDED by Wave D above — kept here as the
//! historical record; the "still aliased up close" round-2 diagnosis is
//! precisely why a hardware line, despite satisfying every gate this
//! round's own headless test checked, wasn't actually the end of the
//! story):** edges no longer carried ANY world-space cross-section at
//! all. `build_edge_instances` built `Node::new_line` instances over a
//! shared unit-line mesh — GPU-native `LineList`
//! rasterization draws a constant, hardware-line-rasterized
//! device-pixel-width line regardless of distance or on-screen angle
//! (three.js `LineBasicMaterial`'s own approach — vasturiano
//! `3d-force-graph`'s actual default edge renderer). Candidates NOT
//! chosen at the time: (b) screen-space billboarded constant-pixel-width
//! quads (three Line2/cosmos.gl style) — genuinely the best-looking
//! option even then, declined only because it needed new per-edge
//! screen-space expansion geometry (join handling, doubled vertex
//! count, clip-space direction math in the vertex shader) that a
//! QUALITY-ONLY wave judged out of budget — **this is exactly the
//! option Wave D above adopted once the join-handling objection was
//! recognized as void for single straight graph-edge segments**; (c)
//! keeping cylinders but unlit + distance-compensated screen-constant
//! radius — still paid a per-frame trig/projection cost per edge to
//! keep the radius screen-constant AND still rasterized actual triangle
//! geometry (so still had SOME residual sub-pixel risk at extreme
//! angles/very small radii), strictly more machinery than (a) for a
//! worse worst-case guarantee, and STILL wouldn't have solved round 2's
//! binary-coverage/no-analytic-AA diagnosis either. `DEFAULT_EDGE_WIDTH`
//! (a world-space cylinder radius) is gone — nothing left to scale.
//!
//! **Translation-is-the-FROM-endpoint convention preserved unchanged
//! across BOTH edge-mesh replacements** — from the original
//! cylinder-edge divergence note through round 1's unit-line mesh to
//! round 2's [`uzor_urx_3d::Mesh::unit_edge_quad`] (current): each
//! mesh in turn was deliberately built so this crate's own
//! instance-transform math (`translation = from`, `rotation =
//! Quat::from_rotation_arc(Vec3::Y, dir)`, `scale.y = length`) carries
//! over byte-for-byte unchanged — only the mesh Arc's own constructor
//! and the pipeline/shader that draws it have ever changed;
//! `build_edge_instances` itself has been untouched since Wave 2.
//!
//! ## Wave 5 — 3D reference ground grid + axis tick labels (distance LOD)
//!
//! A world-space XZ ground grid, built through the exact SAME instanced
//! edge-quad line machinery graph edges already use ([`Node::new_line`]
//! over a shared [`Mesh::unit_edge_quad`]) — no new `uzor-urx-3d`
//! surface at all: a gridline and a graph edge are visually the SAME
//! primitive, only the endpoints and the tint differ. At the time this
//! wave landed, line WIDTH came entirely from `Renderer3D`'s own
//! per-frame `edge_width_px` uniform, not from any per-instance scale
//! (the 2026-07-22 per-instance-width wave below added that capability,
//! but [`build_grid_instances`] deliberately does not use it — see that
//! function's own doc comment) — so grid lines still share the exact
//! BASE rendered pixel width edges use, and only `color_tint`'s alpha
//! differentiates a dim/strong gridline from an edge.
//!
//! ## 2026-07-22 — 3D-parity-arc final wave: per-instance edge width
//! (round caps)
//!
//! [`build_edge_instances`] now packs a per-edge width MULTIPLIER into
//! the instance model matrix's `scale.x` (`edge_quad_instanced.wgsl`'s
//! own module doc has the full packing/recovery derivation:
//! `scale.x`/`scale.z` were provably unused by that shader's own vertex
//! math before this wave, since only `model * (0,0,0,1)`/`model *
//! (0,1,0,1)` are ever read to recover `from`/`to`) — [`edge_width_scale`]
//! mirrors 2D's own [`crate::render::draw_cluster_edges`] `width = (1.0
//! + weight.sqrt()).min(6.0)` formula's SPIRIT (the only existing
//! weight→width convention in this crate — plain `crate::render::
//! draw_edges` itself does NOT scale ordinary 2D edges by weight at all,
//! a real finding from reading it, not an assumption), normalized so
//! `edge_width_scale(1.0) == 1.0` exactly (byte/pixel compatibility for
//! every pre-existing weight-1.0 edge — the renderer's own BASE
//! `edge_width_px` uniform, still `~1.75px` by default, is untouched).
//! [`build_cluster_edge_instances`]/[`build_grid_instances`] deliberately
//! keep `scale.x = 1.0` (their own doc comments say why) — only real
//! per-graph-edge instances vary width by weight. The shader's own
//! fragment stage ALSO switched from distance-to-infinite-centerline to
//! true distance-to-SEGMENT this same wave (round caps at both ends,
//! `edge_quad_instanced.wgsl`'s own module doc) — the correct "joins"
//! answer for a node-link graph's independent 2-endpoint segments (every
//! real junction is a node, covered by its own sphere or by this cap).
//!
//! [`grid_step_for_scale`] picks a 1/2/5×10^k "nice number" world-unit
//! step so the on-screen spacing between adjacent gridlines lands close
//! to [`GRID_MIN_SCREEN_PX`]-[`GRID_MAX_SCREEN_PX`] at the CURRENT
//! camera distance — the same pinhole apparent-size math
//! [`crate::engine3d::GraphEngine3D::visible_labels`] already uses for
//! its own `screen_radius` LOD input, evaluated at world-unit scale
//! instead of a node's own radius. [`GraphEngine3D::build_scene`]
//! recomputes it fresh on every call (no memoization): a `log10` plus a
//! handful of trig ops is cheap next to the instance-generation work the
//! same call already does.
//!
//! **Tick labels are deliberately NOT a `label_grid::LabelGrid` pass.**
//! That module's crowded-cell quota/ranking machinery exists for
//! irregular, potentially-thousands-large node-label candidate sets;
//! axis ticks are already sparse and perfectly regular (one label per
//! STRONG line — every 5th — never per node), so
//! [`crate::engine3d::GraphEngine3D::draw_overlay`] just walks the same
//! [`GridLine`] list [`build_grid_plan`] produced, keeps `strong` lines
//! only, culls anything that fails to project or lands off-viewport, and
//! caps the total at [`Graph3DGridConfig::max_axis_labels`] — a
//! documented simplification, not a forgotten integration.

use std::collections::HashSet;
use std::sync::Arc;

use glam::{Quat, Vec3};
use uzor_urx_3d::{Light, Mesh, MeshLit, Node, PhongMaterial, Scene3D, Vertex};

use crate::cluster::ClusterRegistry;
use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;
use crate::render::category_color_default;

/// Edge line tint (RGB) — the SAME desaturated blue-gray as the 2D
/// engine's own default edge stroke (`crate::render::draw_edges`'s
/// `"#7c8496"`), for visual parity between the 2D and 3D modes.
pub const EDGE_TINT_RGB: [f32; 3] = [0.486, 0.518, 0.588];

/// Edge alpha — the owner's original spec range was "apparent width
/// ~1.5-2px... alpha ~0.35-0.6... the graph reads as a cloud of nodes,
/// edges recede," originally set to the range's midpoint (`0.5`).
/// Re-tuned DOWN to `0.45` in Wave D (round 2 of the edge-quality
/// overhaul) — the owner's own round-2 target, so the 534-node
/// `clusters` demo fixture's edge mass reads a touch further back
/// behind the node spheres now that the edges themselves are crisper
/// (analytically antialiased instead of a binary-coverage hardware
/// line) and would otherwise read slightly more prominent at the same
/// alpha than round 1's softer hardware-line edges did.
pub const EDGE_ALPHA: f32 = 0.45;

/// [`EDGE_TINT_RGB`]/[`EDGE_ALPHA`] packed into the `[f32; 4]`
/// `Node::color_tint` every edge instance shares (`uzor_urx_3d::Mesh::unit_edge_quad`'s
/// own vertex color is plain white, so this tint IS the edge's final
/// color — `edge_quad_instanced.wgsl`'s `out.color = in.color * in.tint`,
/// premultiplied by its own analytic-AA coverage in the fragment stage).
pub const EDGE_TINT: [f32; 4] = [EDGE_TINT_RGB[0], EDGE_TINT_RGB[1], EDGE_TINT_RGB[2], EDGE_ALPHA];

/// Default for [`Graph3DEdgeStyle::width_scale_max`] — 2026-07-22
/// (3D-parity-arc final wave) — absolute on-screen cap an edge's
/// per-instance width should ever reach (the owner's own "~6px" spec),
/// expressed as a SCALE multiplier against `uzor_urx_3d::pipeline`'s own
/// private `DEFAULT_EDGE_WIDTH_PX = 1.75` (mirrored here as a literal —
/// the SAME "redeclare a small cross-crate constant" convention
/// `DEFAULT_LOCAL_DEPTH_3D` already established for 2D's private
/// `DEFAULT_LOCAL_DEPTH`, since that constant isn't `pub`): `6.0 / 1.75
/// ≈ 3.4286`. A caller that changes the renderer's own BASE
/// `edge_width_px` via `Renderer3D::set_edge_width_px` shifts where this
/// scale-relative cap lands in absolute pixels too — same as every other
/// value this design derives from a per-instance SCALE rather than a
/// hardcoded pixel count.
pub const EDGE_WIDTH_SCALE_MAX: f32 = 6.0 / 1.75;

/// 2026-07-22 (3D-parity-arc final wave) — per-edge width MULTIPLIER
/// from a graph edge's own `weight`, packed into the edge-quad instance's
/// `scale.x` by [`build_edge_instances`] (`edge_quad_instanced.wgsl`'s
/// own module doc has the shader-side packing/recovery derivation).
///
/// Mirrors the SPIRIT of the only existing weight→width convention in
/// this crate — [`crate::render::draw_cluster_edges`]'s `width = (1.0 +
/// weight.sqrt()).min(6.0)` — but that formula computes an ABSOLUTE
/// pixel width directly, not a multiplier, and 2D's own plain
/// `crate::render::draw_edges` does NOT scale ordinary edges by weight
/// at all (a direct read of that function found a fixed `1.7px`/`1.3px`
/// stroke regardless of weight — a real finding, not an assumption this
/// wave carries over). Normalized by its own value at `weight == 1.0`
/// (`(1.0 + 1.0.sqrt()) / 2.0 == 1.0`) so a weight-1.0 edge — the
/// implicit weight every pre-existing graph-edge push used before this
/// wave — recovers `scale == 1.0` exactly, the byte/pixel-compatibility
/// this wave's own gate required (the renderer's BASE `edge_width_px`
/// uniform, `~1.75px` by default, is untouched either way). `max_scale`
/// clamps the result — was the private [`EDGE_WIDTH_SCALE_MAX`] constant
/// read directly; now a caller-supplied parameter (graph-strengthening
/// arc Wave G2b, via [`Graph3DEdgeStyle::width_scale_max`]) so no single
/// edge's weight can blow the line out past a caller-chosen sane maximum.
pub fn edge_width_scale(weight: f32, max_scale: f32) -> f32 {
    let w = weight.max(0.0);
    ((1.0 + w.sqrt()) * 0.5).min(max_scale)
}

/// Default for [`Graph3DEdgeStyle::cluster_edge_tint`] — aggregated
/// cross-cluster synthetic-edge tint (cluster-collapse wave) — the SAME
/// warm gold accent the 2D engine's own default
/// `GraphTheme::dark().cluster_accent` (`"#c9a94e"`) uses for its cluster
/// affordances, converted to a `[f32; 4]` tint (opaque — unlike the
/// desaturated, alpha-blended default [`Graph3DEdgeStyle::tint_rgb`]/
/// [`Graph3DEdgeStyle::alpha`], a cluster's cross-edges are meant to read
/// as a distinct, more prominent accent, mirroring 2D's own opaque
/// `cluster_accent` stroke).
pub const CLUSTER_EDGE_TINT: [f32; 4] = [0.788, 0.663, 0.306, 1.0];

/// Every edge/cluster-edge paint constant this render layer owns,
/// bundled into one caller-configurable struct (graph-strengthening arc
/// Wave G2b — the 3D audit's own configurability inventory flagged every
/// one of these `✗`, no override anywhere). [`Default`] reproduces
/// [`EDGE_TINT_RGB`]/[`EDGE_ALPHA`]/[`EDGE_WIDTH_SCALE_MAX`]/
/// [`CLUSTER_EDGE_TINT`] byte-identically. One `edge_style:
/// Graph3DEdgeStyle` field on [`crate::engine3d::GraphEngine3D`] with an
/// `edge_style()`/`set_edge_style()` accessor pair — the same established
/// shape [`crate::engine::GraphEngine::label_halo`]/`set_label_halo`
/// already use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graph3DEdgeStyle {
    /// Ordinary graph-edge tint (RGB) — was [`EDGE_TINT_RGB`].
    pub tint_rgb: [f32; 3],
    /// Ordinary graph-edge alpha — was [`EDGE_ALPHA`].
    pub alpha: f32,
    /// [`edge_width_scale`]'s own clamp ceiling — was
    /// [`EDGE_WIDTH_SCALE_MAX`].
    pub width_scale_max: f32,
    /// Aggregated cross-cluster synthetic-edge tint — was
    /// [`CLUSTER_EDGE_TINT`].
    pub cluster_edge_tint: [f32; 4],
}

impl Graph3DEdgeStyle {
    /// `tint_rgb`/`alpha` packed into the `[f32; 4]` `Node::color_tint`
    /// every ordinary edge instance shares — was the module-level
    /// `EDGE_TINT` constant, now derived from this struct's own fields.
    pub fn edge_tint(&self) -> [f32; 4] {
        [self.tint_rgb[0], self.tint_rgb[1], self.tint_rgb[2], self.alpha]
    }
}

impl Default for Graph3DEdgeStyle {
    fn default() -> Self {
        Self { tint_rgb: EDGE_TINT_RGB, alpha: EDGE_ALPHA, width_scale_max: EDGE_WIDTH_SCALE_MAX, cluster_edge_tint: CLUSTER_EDGE_TINT }
    }
}

/// Convert [`category_color_default`]'s fixed `"#rrggbb"` palette into an
/// opaque `[f32; 4]` tint — `Node::color_tint` takes floats, not a
/// CSS-style hex string, and there's no shared hex-parser in this crate
/// to reuse (`uzor::ui::widgets::atomic::slider` has one, but it's
/// `u8`-typed and private to that module) — small enough to own here
/// rather than reach into an unrelated widget's internals for four
/// `u8::from_str_radix` calls.
fn category_tint(category: &str) -> [f32; 4] {
    let color = category_color_default(category);
    let hex = color.trim_start_matches('#');
    if hex.len() != 6 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Default for [`Graph3DLighting::node_material`] — Wave C node material
/// softening (owner live-verdict: lit spheres read "muddy/concrete" — a
/// harsh lit/shadow split that washes out category saturation, worsened
/// downstream by the ACES tonemap's own highlight rolloff). The SHARED
/// `uzor_urx_3d::PhongMaterial::default()` (`ambient_strength: 0.1,
/// diffuse_strength: 0.85, specular_strength: 0.4, shininess: 32.0`) is
/// deliberately left untouched — that default is a crate-wide value
/// every OTHER `uzor-urx-3d` consumer also gets, out of this graph-only
/// quality wave's scope. `phong_instanced.wgsl`'s own `fs_main`
/// multiplies `lights.ambient` (`Graph3DLighting::ambient`, below) by
/// `material.ambient_strength` — at the OLD `0.1` default, even a bright
/// scene ambient barely lifts a sphere's unlit hemisphere (`0.28 * 0.1 =
/// 0.028` — near-black), which is exactly the harsh "concrete" look.
/// Raising `ambient_strength` here (own node material, not the shared
/// default) gives a real hemisphere fill; lowering
/// `specular_strength`/`shininess` softens the highlight that was
/// previously washing out saturated hues at its hot spot.
const DEFAULT_NODE_MATERIAL: PhongMaterial = PhongMaterial {
    ambient_strength: 0.35,
    diffuse_strength: 0.7,
    specular_strength: 0.15,
    shininess: 24.0,
};

/// Node lit-material response + default scene lighting rig (3D quality
/// audit B6), bundled into one caller-configurable struct
/// (graph-strengthening arc Wave G2b) — mirrors
/// [`crate::engine::GraphInteractionConfig`]'s own "bundle every
/// constant belonging to one visual subsystem" shape. [`Default`]
/// reproduces [`DEFAULT_NODE_MATERIAL`]/[`arm_default_lighting`]'s own
/// pre-existing values byte-identically. One `lighting: Graph3DLighting`
/// field on [`crate::engine3d::GraphEngine3D`] with a
/// `lighting()`/`set_lighting()` accessor pair — the same established
/// shape [`crate::engine::GraphEngine::label_halo`]/`set_label_halo`
/// already use.
#[derive(Debug, Clone, Copy)]
pub struct Graph3DLighting {
    /// Every node's shared lit-material response — was the private
    /// [`DEFAULT_NODE_MATERIAL`] constant.
    pub node_material: PhongMaterial,
    /// Scene-wide ambient floor — was `arm_default_lighting`'s hardcoded
    /// `scene.ambient = [0.5, 0.5, 0.55]`.
    pub ambient: [f32; 3],
    /// The single default directional key light's direction — was
    /// `arm_default_lighting`'s hardcoded `Vec3::new(-0.4, -1.0, -0.3)`.
    pub light_direction: Vec3,
    /// The key light's color — was the hardcoded `[1.0, 1.0, 1.0]`.
    pub light_color: [f32; 3],
    /// The key light's intensity — was the hardcoded `0.75`.
    pub light_intensity: f32,
}

impl Default for Graph3DLighting {
    fn default() -> Self {
        Self {
            node_material: DEFAULT_NODE_MATERIAL,
            ambient: [0.5, 0.5, 0.55],
            light_direction: Vec3::new(-0.4, -1.0, -0.3),
            light_color: [1.0, 1.0, 1.0],
            light_intensity: 0.75,
        }
    }
}

/// One instanced `Node::new_lit` per graph node — see the module doc.
/// `hidden` (cluster-collapse wave) is every node currently hidden by a
/// collapsed cluster (every member except that cluster's own
/// representative — see `crate::cluster`'s own doc comment); such a node
/// emits NO instance at all, mirroring the 2D engine's own
/// `render::draw_nodes` (which simply never iterates a hidden node,
/// since it isn't in `ctx.visible`). The representative itself is never
/// hidden and instances normally at its own (collapse-bumped) `node.radius`
/// — the graph's own radius field already reflects the supernode size
/// (`crate::cluster::ClusterRegistry::collapse_3d` bumps it in place), so
/// no separate supernode-scaling branch is needed here.
/// `material` was the private [`DEFAULT_NODE_MATERIAL`] constant — now a
/// caller-supplied parameter (graph-strengthening arc Wave G2b, via
/// [`Graph3DLighting::node_material`]).
pub fn build_node_instances<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    mesh: &Arc<MeshLit>,
    hidden: &HashSet<NodeIndex>,
    material: PhongMaterial,
) -> Vec<Node> {
    graph
        .nodes()
        .filter_map(|(id, node)| {
            if hidden.contains(&id) {
                return None;
            }
            let p = particles.get(id.index())?;
            Some(
                Node::new_lit(mesh.clone())
                    .with_translation(Vec3::new(p.x, p.y, p.z))
                    .with_scale(Vec3::splat(node.radius.max(0.01)))
                    .with_tint(category_tint(&node.category))
                    .with_material(material),
            )
        })
        .collect()
}

/// One instanced `Node::new_line` per graph edge (Wave C/D — see the
/// module doc's edge-quality-overhaul sections) — see the module doc's
/// original divergence note for why `translation` is the FROM endpoint,
/// not the midpoint (still true for [`uzor_urx_3d::Mesh::unit_edge_quad`],
/// built with the SAME not-centred base/top convention — this function
/// itself is byte-for-byte unchanged across both edge-mesh
/// replacements). A coincident (zero-length) edge has no well-defined
/// direction and is skipped rather than emitting a NaN rotation.
///
/// `hidden` (cluster-collapse wave) — an edge touching EITHER endpoint in
/// `hidden` is skipped entirely, mirroring the 2D engine's own
/// `render::draw_edges` (`ctx.hidden.contains(&edge.from) ||
/// ctx.hidden.contains(&edge.to)`). This naturally drops every intra-
/// cluster edge once a cluster is collapsed (every such edge touches at
/// least one non-representative member, which is always in `hidden`) —
/// [`build_cluster_edge_instances`] draws the aggregated cross-cluster
/// substitute separately, the 3D counterpart of `crate::render::
/// draw_cluster_edges`.
///
/// **2026-07-22 (3D-parity-arc final wave)**: `scale.x` — provably
/// unused by `edge_quad_instanced.wgsl`'s own vertex math before this
/// wave (see that shader's own module doc) — now carries
/// [`edge_width_scale`], a per-instance width MULTIPLIER against the
/// renderer's BASE `edge_width_px` uniform. A weight-1.0 edge recovers
/// `scale.x == 1.0` exactly, so every pre-existing graph built before
/// this wave (implicit weight 1.0) renders byte-for-byte unchanged.
///
/// `style` was the module-level [`EDGE_TINT`]/[`EDGE_WIDTH_SCALE_MAX`]
/// constants read directly — now a caller-supplied parameter
/// (graph-strengthening arc Wave G2b, via [`Graph3DEdgeStyle`]).
pub fn build_edge_instances<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    mesh: &Arc<Mesh>,
    hidden: &HashSet<NodeIndex>,
    style: &Graph3DEdgeStyle,
) -> Vec<Node> {
    graph
        .edges()
        .filter_map(|(_, edge)| {
            if hidden.contains(&edge.from) || hidden.contains(&edge.to) {
                return None;
            }
            let a = particles.get(edge.from.index())?;
            let b = particles.get(edge.to.index())?;
            let from = Vec3::new(a.x, a.y, a.z);
            let to = Vec3::new(b.x, b.y, b.z);
            let delta = to - from;
            let length = delta.length();
            if length < 1e-5 {
                return None;
            }
            let dir = delta / length;
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
            Some(
                Node::new_line(mesh.clone())
                    .with_translation(from)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(edge_width_scale(edge.weight, style.width_scale_max), length, 1.0))
                    .with_tint(style.edge_tint()),
            )
        })
        .collect()
}

/// Key light + ambient floor bright enough that `MeshLit` category tints
/// stay legible — raised further in the Wave C node-material-softening
/// pass above (owner: lit spheres read "muddy/concrete"). `Scene3D::default()`'s
/// own dim ambient (`[0.08, 0.08, 0.10]`, `uzor-urx-3d/src/scene3d.rs`)
/// alone renders every `MeshLit` node near-black (proven by
/// `uzor-urx-3d/tests/lighting.rs`'s own "no lights pushed" case, which
/// only asserts a *visible* result because it bumps `scene.ambient` to
/// `[0.5, 0.5, 0.5]` first) — a genuinely required part of making
/// `build_scene`'s output visually distinct, not an optional flourish.
/// The key light's own intensity is DOWN from the original `1.0` — a
/// "hemisphere-ish fill, gentler key" so a node's lit and shadowed
/// hemispheres sit closer together in brightness (with
/// [`Graph3DLighting::node_material`]'s raised `ambient_strength`, the
/// shadow side is no longer near-black either), instead of the old
/// high-contrast harsh-directional look. `lighting` was every field here
/// read directly from a hardcoded literal — now a caller-supplied
/// parameter (graph-strengthening arc Wave G2b, via [`Graph3DLighting`]).
fn arm_default_lighting(scene: &mut Scene3D, lighting: &Graph3DLighting) {
    scene.ambient = lighting.ambient;
    scene.push_light(Light::directional(lighting.light_direction, lighting.light_color, lighting.light_intensity));
}

/// Build the full 3D scene (plan §1.3): every node as an instanced
/// sphere sharing `node_mesh`, every edge as an instanced `LineList`
/// segment sharing `edge_mesh` (Wave C) — two draw calls total
/// regardless of graph size (`uzor-urx-3d`'s `MeshCache` Arc-identity
/// dedup, confirmed real in the plan's own substrate verdict §0) — plus
/// a default key light.
///
/// `hidden` (cluster-collapse wave) — forwarded unchanged to
/// [`build_node_instances`]/[`build_edge_instances`]; pass
/// `&HashSet::new()` for a caller with no cluster concept at all. Does
/// NOT append the aggregated cross-cluster substitute edges itself — see
/// [`build_cluster_edge_instances`], a separate additive call
/// (`GraphEngine3D::build_scene` composes both, mirroring the 2D engine's
/// own `GraphEngine::draw`'s separate `draw_edges`/`draw_cluster_edges`
/// calls). `lighting`/`edge_style` were module-level constants read
/// directly — now caller-supplied parameters (graph-strengthening arc
/// Wave G2b).
pub fn build_scene<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    node_mesh: &Arc<MeshLit>,
    edge_mesh: &Arc<Mesh>,
    hidden: &HashSet<NodeIndex>,
    lighting: &Graph3DLighting,
    edge_style: &Graph3DEdgeStyle,
) -> Scene3D {
    let mut scene = Scene3D::new();
    arm_default_lighting(&mut scene, lighting);
    scene.nodes.extend(build_edge_instances(graph, particles, edge_mesh, hidden, edge_style));
    scene.nodes.extend(build_node_instances(graph, particles, node_mesh, hidden, lighting.node_material));
    scene
}

/// Cluster-supernode aggregated cross-cluster edges (cluster-collapse
/// wave) — the 3D counterpart of `crate::render::draw_cluster_edges`: one
/// instanced edge-quad per outside neighbor of every currently-collapsed
/// cluster, from the representative's own current position, replacing
/// the raw intra/cross-cluster member edges [`build_edge_instances`]
/// already excludes via its `hidden` set. Unlike 2D's own per-synthetic-
/// edge WIDTH scaling by summed weight (`draw_cluster_edges`'s `width =
/// (1.0 + weight.sqrt()).min(6.0)`), this function deliberately keeps
/// `scale.x = 1.0` (the renderer's BASE `edge_width_px`) even though
/// `uzor_urx_3d`'s edge-quad pipeline gained real per-instance width
/// support in the 2026-07-22 wave ([`edge_width_scale`]/
/// [`build_edge_instances`]'s own doc comment) — every synthetic
/// cross-cluster edge here shares the SAME tint/width as an ordinary
/// weight-1.0 graph edge, just through `cluster_edge_tint` instead of the
/// ordinary edge tint, a documented divergence rather than an oversight
/// (out of scope for that wave — it named `build_edge_instances`
/// specifically). `cluster_edge_tint` was the module-level
/// [`CLUSTER_EDGE_TINT`] constant read directly — now a caller-supplied
/// parameter (graph-strengthening arc Wave G2b, via
/// [`Graph3DEdgeStyle::cluster_edge_tint`]).
pub fn build_cluster_edge_instances(particles: &[Particle], mesh: &Arc<Mesh>, clusters: &ClusterRegistry, cluster_edge_tint: [f32; 4]) -> Vec<Node> {
    let mut out = Vec::new();
    for cluster in clusters.collapsed_clusters() {
        let Some(rep) = particles.get(cluster.representative.index()) else { continue };
        let from = Vec3::new(rep.x, rep.y, rep.z);
        for edge in cluster.aggregated_edges() {
            let Some(other) = particles.get(edge.outside.index()) else { continue };
            let to = Vec3::new(other.x, other.y, other.z);
            let delta = to - from;
            let length = delta.length();
            if length < 1e-5 {
                continue;
            }
            let dir = delta / length;
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
            out.push(
                Node::new_line(mesh.clone())
                    .with_translation(from)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(1.0, length, 1.0))
                    .with_tint(cluster_edge_tint),
            );
        }
    }
    out
}

// ── Wave 5 — 3D reference ground grid + axis tick labels (distance LOD) ──

/// Default for [`Graph3DGridConfig::line_alpha`] — dim gridline alpha
/// (the owner's own spec: "~0.15").
const DEFAULT_GRID_LINE_ALPHA: f32 = 0.15;
/// Default for [`Graph3DGridConfig::strong_line_alpha`] — every 5th
/// line's own, more visible alpha (the owner's own spec: "~0.25").
const DEFAULT_GRID_STRONG_LINE_ALPHA: f32 = 0.25;
/// Default for [`Graph3DGridConfig::tint_rgb`] — neutral gray-blue tint,
/// visually distinct from [`EDGE_TINT_RGB`]'s own warmer blue-gray so a
/// grid line and a graph edge don't read as the exact same element even
/// though they share one rendering pipeline.
const DEFAULT_GRID_TINT_RGB: [f32; 3] = [0.60, 0.63, 0.70];
/// Default for [`Graph3DGridConfig::strong_line_every`] — every Nth tick
/// (index counted from world coordinate `0`, not from the range's own
/// start — so which lines are "strong" doesn't shift as the graph's own
/// AABB drifts) is a strong line, per the owner's own spec.
const DEFAULT_GRID_STRONG_LINE_EVERY: i64 = 5;
/// Default for [`Graph3DGridConfig::y_margin_fraction`] — ground-plane
/// drop below the AABB's own lowest point, as a fraction of the AABB's
/// largest dimension (floored by [`DEFAULT_GRID_Y_MARGIN_MIN`] so a very
/// flat/small graph still gets a visibly separated ground plane) — the
/// task's own "AABB min y minus a small margin" simplification of "the
/// XZ plane through the centroid."
const DEFAULT_GRID_Y_MARGIN_FRACTION: f32 = 0.08;
/// Default for [`Graph3DGridConfig::y_margin_min`].
const DEFAULT_GRID_Y_MARGIN_MIN: f32 = 4.0;

/// Target on-screen gridline-spacing BAND (the owner's own spec: "roughly
/// 40-160px"). [`grid_step_for_scale`] snaps to whichever 1/2/5×10^k
/// ladder rung lands closest (in log-RATIO terms, not linear difference)
/// to [`Graph3DGridConfig::target_screen_px`] — the band's own geometric
/// mean, the natural "center" of a multiplicative range. The worst-case
/// ladder gap (`5 -> 10`, or equivalently `1 -> 2`, both ratio `2.5`)
/// means the worst-case snap lands at `sqrt(2.5) ≈ 1.58×` off target in
/// either direction — `80 / 1.58 ≈ 50.6px` and `80 * 1.58 ≈ 126.5px` —
/// safely inside this band with real margin either side. `GRID_MIN/MAX_SCREEN_PX`
/// are documentation of the resulting band, not inputs any function reads
/// — the ladder snap against `target_screen_px` is what actually produces
/// it.
pub const GRID_MIN_SCREEN_PX: f64 = 40.0;
pub const GRID_MAX_SCREEN_PX: f64 = 160.0;
/// Default for [`Graph3DGridConfig::target_screen_px`].
const DEFAULT_GRID_TARGET_SCREEN_PX: f64 = 80.0;

/// Default for [`Graph3DGridConfig::max_axis_labels`] — axis-tick-label
/// budget per [`crate::engine3d::GraphEngine3D::draw_overlay`] call
/// (module doc's own "documented simplification, not a forgotten
/// integration" note) — cheap insurance against an extreme AABB/step
/// combination producing an unreasonable label count.
const DEFAULT_GRID_MAX_AXIS_LABELS: usize = 40;

/// Every reference-grid tuning constant this render layer owns, bundled
/// into one caller-configurable struct (graph-strengthening arc Wave
/// G2b — 3D quality audit B7: "grid tuning constants have zero exposed
/// configurability beyond the on/off toggle"). [`Default`] reproduces
/// every constant above byte-identically. One `grid_config:
/// Graph3DGridConfig` field on [`crate::engine3d::GraphEngine3D`] with a
/// `grid_config()`/`set_grid_config()` accessor pair — the same
/// established shape [`crate::engine::GraphEngine::label_halo`]/
/// `set_label_halo` already use. Distinct from
/// [`crate::engine3d::GraphEngine3D::grid_enabled`], which stays the
/// existing plain on/off toggle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Graph3DGridConfig {
    pub line_alpha: f32,
    pub strong_line_alpha: f32,
    pub tint_rgb: [f32; 3],
    pub strong_line_every: i64,
    pub y_margin_fraction: f32,
    pub y_margin_min: f32,
    /// [`grid_step_for_scale`]'s own target on-screen spacing — see
    /// [`GRID_MIN_SCREEN_PX`]/[`GRID_MAX_SCREEN_PX`]'s own doc comment
    /// for the resulting band this produces.
    pub target_screen_px: f64,
    pub max_axis_labels: usize,
}

impl Default for Graph3DGridConfig {
    fn default() -> Self {
        Self {
            line_alpha: DEFAULT_GRID_LINE_ALPHA,
            strong_line_alpha: DEFAULT_GRID_STRONG_LINE_ALPHA,
            tint_rgb: DEFAULT_GRID_TINT_RGB,
            strong_line_every: DEFAULT_GRID_STRONG_LINE_EVERY,
            y_margin_fraction: DEFAULT_GRID_Y_MARGIN_FRACTION,
            y_margin_min: DEFAULT_GRID_Y_MARGIN_MIN,
            target_screen_px: DEFAULT_GRID_TARGET_SCREEN_PX,
            max_axis_labels: DEFAULT_GRID_MAX_AXIS_LABELS,
        }
    }
}

/// Snap `raw` (a positive, unitless "world units per target pixel"
/// value) to the nearest 1/2/5×10^k "nice number" ladder rung — the same
/// convention d3/sigma-style axis-tick generators use. "Nearest" is by
/// LOG-ratio, not linear difference: a linear nearest-neighbor would
/// systematically favor the larger candidate at every magnitude (e.g.
/// `fraction=3` is linearly closer to `2` AND to `5` depending on how you
/// measure, but `3/2 = 1.5` is a smaller ratio than `5/3 ≈ 1.67`, so `2`
/// is the log-nearer, and therefore visually-truer, neighbor). Pure and
/// total across every magnitude a caller might pass — `raw <= 0` clamps
/// to a tiny positive floor before `log10` ever sees it.
fn snap_to_nice_step(raw: f64) -> f64 {
    let raw = raw.max(1e-12);
    let exponent = raw.log10().floor();
    let base = 10f64.powi(exponent as i32);
    let fraction = raw / base;
    const CANDIDATES: [f64; 4] = [1.0, 2.0, 5.0, 10.0];
    let mut best = CANDIDATES[0];
    let mut best_ratio = f64::MAX;
    for &candidate in &CANDIDATES {
        let ratio = (fraction / candidate).max(candidate / fraction);
        if ratio < best_ratio {
            best_ratio = ratio;
            best = candidate;
        }
    }
    best * base
}

/// World-space grid step for a ground grid viewed from `camera_distance`
/// with vertical field-of-view `fov_y_radians`, rendered into a
/// `viewport_height_px`-tall surface — see the module doc's own
/// distance-LOD writeup. Non-finite/non-positive inputs fall back to a
/// step of `1.0` rather than propagating NaN/inf into the grid geometry.
/// `config.target_screen_px` was the private [`DEFAULT_GRID_TARGET_SCREEN_PX`]
/// constant read directly — now a caller-supplied parameter
/// (graph-strengthening arc Wave G2b).
pub fn grid_step_for_scale(camera_distance: f32, fov_y_radians: f32, viewport_height_px: f64, config: &Graph3DGridConfig) -> f64 {
    let half_fov_tan = (fov_y_radians * 0.5).tan().max(1e-6) as f64;
    let distance = (camera_distance.max(1e-3)) as f64;
    let px_per_world_unit = (viewport_height_px.max(1.0) * 0.5) / (distance * half_fov_tan);
    if !px_per_world_unit.is_finite() || px_per_world_unit <= 0.0 {
        return 1.0;
    }
    snap_to_nice_step(config.target_screen_px / px_per_world_unit)
}

/// One planned gridline before it becomes an instanced [`Node`] — also
/// the shape [`crate::engine3d::GraphEngine3D::draw_overlay`] walks to
/// place axis tick labels (module doc: "NOT a `label_grid::LabelGrid`
/// pass").
#[derive(Debug, Clone, Copy)]
pub struct GridLine {
    pub from: Vec3,
    pub to: Vec3,
    /// The world-space coordinate this line represents along its OWN
    /// axis (the X value for a line that runs parallel to Z, or the Z
    /// value for a line that runs parallel to X) — the tick label's
    /// numeric text, via [`format_tick_value`].
    pub tick_value: f64,
    /// Every [`Graph3DGridConfig::strong_line_every`]th tick — drawn at
    /// [`Graph3DGridConfig::strong_line_alpha`] instead of
    /// [`Graph3DGridConfig::line_alpha`], and the only lines that ever
    /// get a tick label.
    pub strong: bool,
}

/// A fully planned ground grid — [`build_grid_instances`] turns this into
/// instanced [`Node`]s, and [`crate::engine3d::GraphEngine3D::draw_overlay`]
/// walks `lines` directly for tick-label placement (`step` alongside it
/// so [`format_tick_value`] can decide integer-vs-decimal formatting).
#[derive(Debug, Clone)]
pub struct GridPlan {
    pub lines: Vec<GridLine>,
    pub step: f64,
}

/// Ground-plane Y (module doc: "AABB min y minus a small margin").
/// `config.y_margin_fraction`/`config.y_margin_min` were private
/// constants read directly — now caller-supplied parameters
/// (graph-strengthening arc Wave G2b).
fn grid_ground_y(min: Vec3, max: Vec3, config: &Graph3DGridConfig) -> f32 {
    let extent = max - min;
    let largest = extent.x.max(extent.y).max(extent.z).max(0.0);
    let margin = (largest * config.y_margin_fraction).max(config.y_margin_min);
    min.y - margin
}

/// Tick positions covering `[min, max]`, ROUNDED OUT to `step` (the
/// task's own "grid extent covers the node AABB, rounded out to the
/// step") — `(value, strong)` pairs, `strong` counted from world
/// coordinate `0`, not from `min`, via [`i64::rem_euclid`] so a negative
/// tick index still lands on the correct 5-line cadence. Empty for a
/// non-finite/non-positive `step`, or `min > max` (shouldn't happen for a
/// real AABB, but a defensive empty result beats a panic). `strong_line_every`
/// was the private `GRID_STRONG_LINE_EVERY` constant read directly — now
/// a caller-supplied parameter (graph-strengthening arc Wave G2b).
fn grid_ticks(min: f64, max: f64, step: f64, strong_line_every: i64) -> Vec<(f64, bool)> {
    if !min.is_finite() || !max.is_finite() || !step.is_finite() || step <= 0.0 || min > max {
        return Vec::new();
    }
    let start_index = (min / step).floor() as i64;
    let end_index = (max / step).ceil() as i64;
    (start_index..=end_index)
        .map(|index| (index as f64 * step, index.rem_euclid(strong_line_every) == 0))
        .collect()
}

/// Plan a ground grid covering the AABB (`min`, `max`) at world-unit
/// `step` — see the module doc. Lines run BOTH ways: one per X tick
/// (running parallel to Z, spanning the full rounded-out Z range) and
/// one per Z tick (running parallel to X, spanning the full rounded-out
/// X range) — the ordinary two-family regular grid. `config` was every
/// grid-tuning constant read directly — now a caller-supplied parameter
/// (graph-strengthening arc Wave G2b, via [`Graph3DGridConfig`]).
pub fn build_grid_plan(min: Vec3, max: Vec3, step: f64, config: &Graph3DGridConfig) -> GridPlan {
    let step = if step.is_finite() && step > 0.0 { step } else { 1.0 };
    let grid_y = grid_ground_y(min, max, config);
    let x_ticks = grid_ticks(min.x as f64, max.x as f64, step, config.strong_line_every);
    let z_ticks = grid_ticks(min.z as f64, max.z as f64, step, config.strong_line_every);
    let z_min = z_ticks.first().map_or(min.z as f64, |t| t.0);
    let z_max = z_ticks.last().map_or(max.z as f64, |t| t.0);
    let x_min = x_ticks.first().map_or(min.x as f64, |t| t.0);
    let x_max = x_ticks.last().map_or(max.x as f64, |t| t.0);

    let mut lines = Vec::with_capacity(x_ticks.len() + z_ticks.len());
    for (x, strong) in x_ticks {
        lines.push(GridLine {
            from: Vec3::new(x as f32, grid_y, z_min as f32),
            to: Vec3::new(x as f32, grid_y, z_max as f32),
            tick_value: x,
            strong,
        });
    }
    for (z, strong) in z_ticks {
        lines.push(GridLine {
            from: Vec3::new(x_min as f32, grid_y, z as f32),
            to: Vec3::new(x_max as f32, grid_y, z as f32),
            tick_value: z,
            strong,
        });
    }
    GridPlan { lines, step }
}

/// Turn a [`GridPlan`] into instanced [`Node::new_line`]s sharing `mesh`
/// (the SAME [`Mesh::unit_edge_quad`] Arc graph edges already instance —
/// module doc's own "no new `uzor-urx-3d` surface" point). A degenerate
/// (zero-length) line is skipped, exactly like [`build_edge_instances`]
/// does for a coincident edge. **Deliberately keeps `scale.x = 1.0`**
/// (the per-instance-width-scale wave's default, see the module doc's
/// own "2026-07-22" section) — a grid line has no weight concept to
/// scale by, so it always renders at the renderer's BASE `edge_width_px`.
/// `config` was every grid-paint constant read directly — now a
/// caller-supplied parameter (graph-strengthening arc Wave G2b).
pub fn build_grid_instances(plan: &GridPlan, mesh: &Arc<Mesh>, config: &Graph3DGridConfig) -> Vec<Node> {
    plan.lines
        .iter()
        .filter_map(|line| {
            let delta = line.to - line.from;
            let length = delta.length();
            if length < 1e-5 {
                return None;
            }
            let dir = delta / length;
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
            let alpha = if line.strong { config.strong_line_alpha } else { config.line_alpha };
            let tint = [config.tint_rgb[0], config.tint_rgb[1], config.tint_rgb[2], alpha];
            Some(
                Node::new_line(mesh.clone())
                    .with_translation(line.from)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(1.0, length, 1.0))
                    .with_tint(tint),
            )
        })
        .collect()
}

/// Compact numeric label for a [`GridLine::tick_value`] — integers once
/// `step >= 1.0` (the owner's own spec), otherwise enough decimal places
/// to actually distinguish adjacent sub-unit ticks, derived from the
/// step's own magnitude rather than a fixed decimal count.
pub fn format_tick_value(value: f64, step: f64) -> String {
    if step >= 1.0 || step <= 0.0 {
        format!("{:.0}", value.round())
    } else {
        let decimals = (-step.log10()).ceil().max(0.0) as usize;
        format!("{value:.decimals$}")
    }
}

// ── Wave 4 — GPU color-ID picking escalation (plan §1.5/§4) ────────────
//
// The id-pass renders every node as a `NodeMesh::Unlit` sphere whose flat
// vertex color IS the tint (white vertex color * tint, per
// `unlit_instanced.wgsl`'s `out.color = in.color * in.tint`) — no lighting,
// no per-fragment gradient, so a pixel deep inside a node's silhouette
// reads back exactly `tint`... EXCEPT `tint` does NOT survive the render
// pipeline unchanged. `Renderer3D::render` (and therefore
// `render_to_texture`, which just calls it, `pipeline.rs:2726`)
// unconditionally runs `run_bloom_and_composite` at the end of every
// frame — even an Unlit-only, zero-light scene goes through the SAME HDR
// -> bloom -> ACES-filmic-tonemap -> gamma(1/2.2) composite chain the
// Wave 2 divergence log already caught for the LIT node/edge test. On TOP
// of that, `Texture3D`'s only public constructors (`render_target`/
// `from_rgba8`/etc, `uzor-urx-3d/src/texture.rs`) are ALL hardcoded to
// `wgpu::TextureFormat::Rgba8UnormSrgb` — so the offscreen id-pass target
// the composite pass writes into is itself an sRGB-format render
// attachment, and the GPU auto-encodes linear->sRGB on write to any
// `*Srgb` target (the SAME behavior `texture.rs`'s own
// `from_rgba8_mipped` doc comment already documents: "wgpu treats
// Rgba8UnormSrgb as gamma-encoded"). The composite shader's OWN
// `pow(tonemapped, 1/2.2)` gamma step is therefore followed by a SECOND,
// GPU-automatic sRGB encode — a genuine three-stage compound transform
// (ACES filmic -> manual gamma 2.2 -> GPU sRGB auto-encode) between a
// node's `color_tint` and the byte a readback actually observes. None of
// this is fixable here (zero `uzor-urx-3d` changes, plan §1.5's own
// constraint) — pre-existing shared-substrate behavior.
//
// Rather than derive one closed-form symbolic inverse of the full
// three-stage chain (real, but fragile — a real GPU/driver's hardware
// sRGB encode need not match a textbook formula to the ULP), the fix
// here is an EXACT, per-channel, bisection-derived inverse mapping TABLE
// (the plan's own explicitly-sanctioned alternative, §4: "either an
// exact inverse mapping table or encode in a tonemap-surviving way")
// combined with a coarse discrete LEVEL grid (`GPU_PICK_ID_LEVELS` per
// channel, spaced evenly across the full 0..=255 output byte range) so
// DECODE is a simple "snap to nearest grid line" — robust to a few bytes
// of real-hardware drift around this module's own `forward_channel_byte`
// model, not just to zero drift. `encode_node_id_tint` computes, for the
// TARGET output byte of each channel's level, the exact pre-tonemap
// linear `x` that THIS MODULE'S OWN forward model maps to that byte
// (bisection over `forward_channel_byte`, monotonic non-decreasing in
// `x`) — the inverse is computed ONCE, at encode time, so `decode_*`
// never inverts anything at all, it just reads the byte back and snaps
// it to the nearest known grid line. Proven correct two ways (this
// module's own tests): a pure-Rust round-trip property test (encode ->
// this module's own forward simulation -> decode, no GPU needed) AND a
// headless `wgpu` test (`tests/render3d_gpu.rs`) that renders a real
// id-pass scene and decodes the REAL readback bytes — "PROVE, don't
// assume."

/// Discrete levels per color channel the GPU id-pass encodes a
/// [`NodeIndex`] into — R,G,B only. The composite pass hardcodes output
/// alpha to `1.0` (`uzor-urx-3d/src/shaders/composite_aces.wgsl`'s own
/// `fs_main`: `return vec4<f32>(gamma, 1.0);`), so a node's tint alpha
/// never survives to a readback at all.
pub const GPU_PICK_ID_LEVELS: u32 = 32;

/// Largest [`NodeIndex`] the GPU id-pass can represent
/// (`GPU_PICK_ID_LEVELS^3 - 1`) — ALSO the reserved "background/no hit"
/// sentinel [`decode_gpu_pick_pixel`] excludes defensively. **The REAL
/// background/no-hit protection is [`decode_gpu_pick_pixel`]'s
/// `node_count` bounds check, not this literal value** — empirically
/// (headless GPU-verified, `tests/render3d_gpu.rs`), [`build_id_pass_scene`]'s
/// white (`[1,1,1,1]`) clear color does NOT decode to exactly this
/// top grid line: a raw linear `1.0` is well below the `~7.24` input
/// where `aces_filmic` actually saturates to its clamped ceiling (a
/// literal `1.0` tonemaps to only `~0.80`, observed real-hardware
/// readback ~`244`, which snaps to grid level `30` of `0..=31`, not
/// `31`). The exact `MAX_GPU_PICKABLE_NODE_INDEX` case is therefore only
/// reachable by a REAL node whose assigned index lands there — an
/// astronomically large graph (`>= GPU_PICK_ID_LEVELS^3` nodes) that has
/// already exhausted every other encodable index too, well past any
/// realistic GPU-pick-eligible graph size.
pub const MAX_GPU_PICKABLE_NODE_INDEX: u32 = GPU_PICK_ID_LEVELS * GPU_PICK_ID_LEVELS * GPU_PICK_ID_LEVELS - 1;

// ACES Narkowicz-fit constants — MUST mirror
// `uzor-urx-3d/src/shaders/composite_aces.wgsl`'s own `aces_filmic` exactly.
const ACES_A: f32 = 2.51;
const ACES_B: f32 = 0.03;
const ACES_C: f32 = 2.43;
const ACES_D: f32 = 0.59;
const ACES_E: f32 = 0.14;

fn aces_filmic(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (ACES_A * x + ACES_B)) / (x * (ACES_C * x + ACES_D) + ACES_E)).clamp(0.0, 1.0)
}

/// The GPU auto-encode a `*Srgb`-format render target applies on write —
/// same standard sRGB OETF `uzor-urx-3d/src/texture.rs`'s own (private)
/// `linear_to_srgb` reproduces for its mip-chain math; duplicated here
/// rather than reaching into that unrelated module's private helper
/// (same small-helper-duplication convention this file's own
/// `category_tint` already follows).
fn srgb_encode(linear: f32) -> f32 {
    let l = linear.clamp(0.0, 1.0);
    if l <= 0.0031308 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// This module's own forward-model simulation of the full compound
/// transform a node's `color_tint` channel goes through before a
/// readback observes it (module doc above): ACES filmic tonemap ->
/// manual gamma-2.2 encode (`composite_aces.wgsl`'s own `pow`) -> GPU
/// automatic sRGB encode (writing into a `*Srgb`-format target).
fn forward_channel_byte(x: f32) -> u8 {
    let tonemapped = aces_filmic(x);
    let gamma = tonemapped.powf(1.0 / 2.2);
    let encoded = srgb_encode(gamma);
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Upper bound of [`linear_for_target_byte`]'s bisection search domain —
/// comfortably past the `x` where `aces_filmic` saturates to its clamped
/// `1.0` ceiling (~7.24, solved analytically in `uzor-graph/CLAUDE.md`'s
/// own Wave 4 divergence log), so every target byte in `0..=255` has a
/// reachable root inside `[0, ACES_INVERSE_SEARCH_MAX]`.
const ACES_INVERSE_SEARCH_MAX: f32 = 16.0;
const ACES_INVERSE_SEARCH_ITERS: u32 = 40;

/// The exact inverse the module doc above talks about — computed ONCE
/// per encoded channel via bisection ([`forward_channel_byte`] is
/// monotonic non-decreasing in `x`, so bisection converges to the unique
/// `x` whose forward byte is `target_byte`, up to `f32` precision).
fn linear_for_target_byte(target_byte: u8) -> f32 {
    let mut lo = 0.0f32;
    let mut hi = ACES_INVERSE_SEARCH_MAX;
    for _ in 0..ACES_INVERSE_SEARCH_ITERS {
        let mid = (lo + hi) * 0.5;
        if forward_channel_byte(mid) < target_byte {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

/// Evenly-spaced grid point in `0..=255` for `level` (`level 0 -> byte
/// 0`, `level (GPU_PICK_ID_LEVELS - 1) -> byte 255`).
fn level_to_byte(level: u32) -> u8 {
    let level = level.min(GPU_PICK_ID_LEVELS - 1);
    ((level * 255) / (GPU_PICK_ID_LEVELS - 1)) as u8
}

/// Nearest-grid-line snap for an observed byte — robust to a few bytes
/// of drift between this module's own [`forward_channel_byte`] model and
/// whatever the real GPU/driver actually produced (module doc above).
fn byte_to_level(byte: u8) -> u32 {
    let numerator = byte as u32 * (GPU_PICK_ID_LEVELS - 1);
    // Round-to-nearest via integer math: round(n/255) == (n*2+255)/(255*2).
    ((numerator * 2 + 255) / (255 * 2)).min(GPU_PICK_ID_LEVELS - 1)
}

/// Encode `id` into an opaque `color_tint` such that, after the id-pass
/// render pipeline's full forward transform (module doc above), a
/// readback of the resulting pixel decodes back to `id` via
/// [`decode_node_id_pixel`]. Indices at/above [`MAX_GPU_PICKABLE_NODE_INDEX`]
/// clamp to that cap (documented ambiguity, see its own doc comment).
pub fn encode_node_id_tint(id: NodeIndex) -> [f32; 4] {
    let idx = id.0.min(MAX_GPU_PICKABLE_NODE_INDEX);
    let r_level = idx % GPU_PICK_ID_LEVELS;
    let g_level = (idx / GPU_PICK_ID_LEVELS) % GPU_PICK_ID_LEVELS;
    let b_level = (idx / (GPU_PICK_ID_LEVELS * GPU_PICK_ID_LEVELS)) % GPU_PICK_ID_LEVELS;
    [
        linear_for_target_byte(level_to_byte(r_level)),
        linear_for_target_byte(level_to_byte(g_level)),
        linear_for_target_byte(level_to_byte(b_level)),
        1.0,
    ]
}

/// Raw decode of an observed RGBA readback byte quad back into a
/// [`NodeIndex`] — no background/bounds check; see
/// [`decode_gpu_pick_pixel`] for the checked version an actual GPU pick
/// call site should use.
pub fn decode_node_id_pixel(rgba: [u8; 4]) -> NodeIndex {
    let r = byte_to_level(rgba[0]);
    let g = byte_to_level(rgba[1]);
    let b = byte_to_level(rgba[2]);
    NodeIndex(r + g * GPU_PICK_ID_LEVELS + b * GPU_PICK_ID_LEVELS * GPU_PICK_ID_LEVELS)
}

/// [`decode_node_id_pixel`] plus the two checks a real GPU pick call site
/// needs. The PRIMARY guard is the `node_count` bounds check — a
/// background pixel (or any hardware-noise byte quad) almost always
/// decodes to SOME grid combination, but that combination is essentially
/// never a real, currently-assigned [`NodeIndex`], so bounding against
/// the live graph size is what actually filters it out (see
/// [`MAX_GPU_PICKABLE_NODE_INDEX`]'s own doc comment — [`build_id_pass_scene`]'s
/// white clear color decodes NEAR, not exactly at, that top grid line).
/// The explicit `== MAX_GPU_PICKABLE_NODE_INDEX` check is a secondary,
/// belt-and-suspenders exclusion for the one case the bounds check alone
/// can't catch (a real node whose index happens to land exactly there).
pub fn decode_gpu_pick_pixel(rgba: [u8; 4], node_count: u32) -> Option<NodeIndex> {
    let id = decode_node_id_pixel(rgba);
    if id.0 >= node_count || id.0 == MAX_GPU_PICKABLE_NODE_INDEX {
        None
    } else {
        Some(id)
    }
}

/// Build the id-pass unlit sphere mesh — the SAME UV-sphere tessellation
/// [`MeshLit::sphere`] uses (rings/slices math not duplicated), just
/// re-packed into the flat [`Vertex`] format `NodeMesh::Unlit` needs (no
/// normal — the unlit fragment shader never reads one,
/// `unlit_instanced.wgsl`).
pub fn build_id_pass_mesh(rings: u32, slices: u32) -> Mesh {
    let lit = MeshLit::sphere(1.0, rings, slices, [1.0, 1.0, 1.0, 1.0]);
    let vertices = lit.vertices.iter().map(|v| Vertex { pos: v.pos, _pad0: 0.0, color: v.color }).collect();
    Mesh { vertices, indices: lit.indices }
}

/// Build the GPU color-ID pass scene (plan §1.5/§4 Wave 4): every node as
/// an instanced `NodeMesh::Unlit` sphere sharing `id_pass_mesh`, tinted
/// by [`encode_node_id_tint`] instead of its category color. No edges
/// (picking is node-only, mirrors 2D's own `pick::nearest_node`) and no
/// lights (the unlit pipeline ignores them entirely). Clear color is
/// pure white — deliberately as far as possible from every real node's
/// tint on the shared 0..255 grid, though [`decode_gpu_pick_pixel`]'s own
/// `node_count` bounds check (not exact byte alignment) is what actually
/// guarantees a background pixel is never mistaken for a real node — see
/// that function's own doc comment.
///
/// **Graph-strengthening arc G1.2**: `hidden` (cluster-collapse ∪
/// local-subgraph ∪ filter exclusion — the SAME union
/// [`GraphEngine3D::compute_excluded_nodes_3d`] feeds
/// [`build_node_instances`]/[`build_edge_instances`] from) is now a
/// required parameter — before this fix the id-pass scene included EVERY
/// graph node unconditionally, so a node invisible in the real rendered
/// scene (collapsed into a cluster, excluded by a local-subgraph
/// restriction or a filter) still painted a real, decodable-`NodeIndex`
/// sphere into the id-pass texture and could occlude a visible node's own
/// pixel there — a GPU pick could resolve to a `NodeIndex` the user could
/// never have clicked via CPU picking (`pick_candidates()` already
/// excludes it). Mirrors [`build_node_instances`]'s own `hidden` filter
/// exactly, including "a hidden node emits no instance at all."
pub fn build_id_pass_scene<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    id_pass_mesh: &Arc<Mesh>,
    hidden: &HashSet<NodeIndex>,
) -> Scene3D {
    let mut scene = Scene3D::new();
    scene.clear_color = [1.0, 1.0, 1.0, 1.0];
    scene.nodes = graph
        .nodes()
        .filter_map(|(id, node)| {
            if hidden.contains(&id) {
                return None;
            }
            let p = particles.get(id.index())?;
            Some(
                Node::new(id_pass_mesh.clone())
                    .with_translation(Vec3::new(p.x, p.y, p.z))
                    .with_scale(Vec3::splat(node.radius.max(0.01)))
                    .with_tint(encode_node_id_tint(id)),
            )
        })
        .collect();
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;

    type DemoGraph = Graph<(), ()>;

    fn unit_mesh() -> Arc<MeshLit> {
        Arc::new(MeshLit::sphere(1.0, 4, 4, [1.0, 1.0, 1.0, 1.0]))
    }

    fn unit_edge_quad_mesh() -> Arc<Mesh> {
        Arc::new(Mesh::unit_edge_quad([1.0, 1.0, 1.0, 1.0]))
    }

    #[test]
    fn build_node_instances_emits_one_lit_node_per_graph_node_with_translation_scale_and_tint() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "cat-a", 2.0);
        let particles = vec![Particle::at3(1.0, 2.0, 3.0)];
        let mesh = unit_mesh();

        let nodes = build_node_instances(&graph, &particles, &mesh, &HashSet::new(), DEFAULT_NODE_MATERIAL);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(nodes[0].scale, Vec3::splat(2.0));
        assert_eq!(nodes[0].color_tint, category_tint("cat-a"));
        assert!(nodes[0].is_lit());
        assert_eq!(nodes[0].material.ambient_strength, DEFAULT_NODE_MATERIAL.ambient_strength, "Wave C node-material softening must actually be wired into build_node_instances");
    }

    #[test]
    fn build_node_instances_uses_the_caller_supplied_material_not_a_hardcoded_one() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "cat-a", 2.0);
        let particles = vec![Particle::at3(1.0, 2.0, 3.0)];
        let mesh = unit_mesh();
        let custom = PhongMaterial { ambient_strength: 0.9, diffuse_strength: 0.1, specular_strength: 0.0, shininess: 4.0 };

        let nodes = build_node_instances(&graph, &particles, &mesh, &HashSet::new(), custom);

        assert_eq!(nodes[0].material.ambient_strength, 0.9, "Wave G2b configurability gate: the material parameter must actually be threaded through, not ignored");
    }

    #[test]
    fn build_edge_instances_places_the_translation_at_the_from_endpoint_not_the_midpoint() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 5.0, 0.0)];
        let mesh = unit_edge_quad_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, &HashSet::new(), &Graph3DEdgeStyle::default());

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].translation, Vec3::ZERO, "translation must be the FROM endpoint, not the midpoint — see the module doc");
        assert!((edges[0].scale.y - 5.0).abs() < 1e-5);
        assert_eq!(edges[0].scale.x, 1.0, "a weight-1.0 edge must recover scale.x == 1.0 exactly — the byte/pixel-compatibility the per-instance-width wave's own gate required");
        assert_eq!(edges[0].color_tint, EDGE_TINT);
        assert!(matches!(edges[0].geometry, uzor_urx_3d::NodeMesh::Line(_)), "edges must use the dedicated edge-quad geometry, not a cylinder");
        // b - a is already +Y, the line mesh's own local axis, so the
        // rotation should be (near-)identity.
        let rotated_axis = edges[0].rotation * Vec3::Y;
        assert!((rotated_axis - Vec3::Y).length() < 1e-4);
    }

    #[test]
    fn build_edge_instances_rotation_aligns_the_line_axis_to_an_arbitrary_edge_direction() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(3.0, 4.0, 0.0)];
        let mesh = unit_edge_quad_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, &HashSet::new(), &Graph3DEdgeStyle::default());

        let expected_dir = Vec3::new(3.0, 4.0, 0.0).normalize();
        let rotated_axis = edges[0].rotation * Vec3::Y;
        assert!((rotated_axis - expected_dir).length() < 1e-4);
        assert!((edges[0].scale.y - 5.0).abs() < 1e-4);
    }

    // ── 2026-07-22 (3D-parity-arc final wave) — per-instance edge width ──

    #[test]
    fn edge_width_scale_of_weight_1_is_the_identity_scale() {
        assert_eq!(edge_width_scale(1.0, EDGE_WIDTH_SCALE_MAX), 1.0, "a weight-1.0 edge must recover exactly scale 1.0 — the byte/pixel-compatibility gate");
    }

    #[test]
    fn edge_width_scale_of_a_lower_weight_is_visibly_thinner_than_weight_1() {
        // Mirrors the demo's own `clusters`/`sparse` fixtures: hub-ring
        // edges at weight 0.6 vs intra-cluster edges at weight 1.0.
        let thinner = edge_width_scale(0.6, EDGE_WIDTH_SCALE_MAX);
        let baseline = edge_width_scale(1.0, EDGE_WIDTH_SCALE_MAX);
        assert!(thinner < baseline, "weight 0.6 must scale visibly thinner than weight 1.0: {thinner} vs {baseline}");
        assert!(baseline - thinner > 0.05, "the difference must be large enough to actually READ as a different width on screen, not a sub-pixel rounding wash: {thinner} vs {baseline}");
    }

    #[test]
    fn edge_width_scale_is_monotonically_increasing_in_weight() {
        let low = edge_width_scale(0.1, EDGE_WIDTH_SCALE_MAX);
        let mid = edge_width_scale(1.0, EDGE_WIDTH_SCALE_MAX);
        let high = edge_width_scale(4.0, EDGE_WIDTH_SCALE_MAX);
        assert!(low < mid, "{low} should be < {mid}");
        assert!(mid < high, "{mid} should be < {high}");
    }

    #[test]
    fn edge_width_scale_clamps_at_the_sane_max_for_an_extreme_weight() {
        let extreme = edge_width_scale(10_000.0, EDGE_WIDTH_SCALE_MAX);
        assert_eq!(extreme, EDGE_WIDTH_SCALE_MAX, "an extreme weight must clamp at the caller-supplied max_scale, not blow the line out unbounded");
        // At the renderer's own default 1.75px base uniform, the default
        // max_scale must land close to the owner's own "~6px" sane
        // maximum.
        let clamped_px = 1.75 * EDGE_WIDTH_SCALE_MAX;
        assert!((clamped_px - 6.0).abs() < 0.01, "EDGE_WIDTH_SCALE_MAX against the 1.75px default base must land at ~6px, got {clamped_px}");
    }

    #[test]
    fn edge_width_scale_never_goes_negative_for_a_negative_weight() {
        assert!(edge_width_scale(-5.0, EDGE_WIDTH_SCALE_MAX) > 0.0, "a defensively-clamped negative weight must still produce a positive scale");
    }

    /// Wave G2b configurability gate: a caller-supplied `max_scale`
    /// actually clamps at a DIFFERENT ceiling than the default, not just
    /// exist as an unread parameter.
    #[test]
    fn edge_width_scale_clamps_at_a_caller_supplied_max_scale_not_just_the_default() {
        let custom_max = 2.0;
        assert_eq!(edge_width_scale(10_000.0, custom_max), custom_max);
        assert_ne!(edge_width_scale(10_000.0, custom_max), EDGE_WIDTH_SCALE_MAX);
    }

    #[test]
    fn build_edge_instances_threads_the_edge_weight_into_scale_x_via_edge_width_scale() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 0.6, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 5.0, 0.0)];
        let mesh = unit_edge_quad_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, &HashSet::new(), &Graph3DEdgeStyle::default());

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].scale.x, edge_width_scale(0.6, EDGE_WIDTH_SCALE_MAX), "scale.x must carry exactly edge_width_scale(edge.weight, style.width_scale_max), not the old hardcoded 1.0");
        assert_ne!(edges[0].scale.x, 1.0, "a weight-0.6 edge must NOT recover the identity scale");
    }

    /// Wave G2b configurability gate: a caller-supplied `Graph3DEdgeStyle`
    /// (distinct tint, not just default) must actually paint the edge —
    /// proving the parameter is threaded through, not silently ignored.
    #[test]
    fn build_edge_instances_uses_the_caller_supplied_style_tint() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 5.0, 0.0)];
        let mesh = unit_edge_quad_mesh();
        let style = Graph3DEdgeStyle { tint_rgb: [1.0, 0.0, 0.0], alpha: 0.9, ..Graph3DEdgeStyle::default() };

        let edges = build_edge_instances(&graph, &particles, &mesh, &HashSet::new(), &style);

        assert_eq!(edges[0].color_tint, [1.0, 0.0, 0.0, 0.9]);
        assert_ne!(edges[0].color_tint, EDGE_TINT);
    }

    #[test]
    fn build_cluster_edge_instances_keeps_scale_x_at_the_identity_regardless_of_summed_weight() {
        let mut graph = DemoGraph::new();
        let member = graph.push_node((), "member", "x", 1.0);
        let outside = graph.push_node((), "outside", "x", 4.0);
        graph.push_edge(member, outside, 5.0, ());
        let mut clusters = ClusterRegistry::default();
        let id = clusters.define(&graph, vec![member]).expect("define must succeed for a real member");
        let mut particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0)];
        assert!(clusters.collapse_3d(id, &mut graph, &mut particles), "collapse_3d must succeed");
        let mesh = unit_edge_quad_mesh();

        let edges = build_cluster_edge_instances(&particles, &mesh, &clusters, CLUSTER_EDGE_TINT);

        assert!(!edges.is_empty(), "expected at least one aggregated cross-cluster edge (summed weight 5.0)");
        for e in &edges {
            assert_eq!(e.scale.x, 1.0, "cluster synthetic edges deliberately stay at the BASE width regardless of summed weight — see the function's own doc comment");
        }
    }

    #[test]
    fn build_edge_instances_skips_a_coincident_degenerate_edge() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(2.0, 2.0, 2.0), Particle::at3(2.0, 2.0, 2.0)];
        let mesh = unit_edge_quad_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, &HashSet::new(), &Graph3DEdgeStyle::default());

        assert!(edges.is_empty(), "a zero-length edge has no well-defined direction — must not emit a NaN-rotation node");
    }

    #[test]
    fn build_edge_instances_skips_an_edge_touching_a_hidden_node() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        let c = graph.push_node((), "c", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        graph.push_edge(b, c, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 5.0, 0.0), Particle::at3(0.0, 10.0, 0.0)];
        let mesh = unit_edge_quad_mesh();
        let hidden: HashSet<NodeIndex> = [b].into_iter().collect();

        let edges = build_edge_instances(&graph, &particles, &mesh, &hidden, &Graph3DEdgeStyle::default());

        assert!(edges.is_empty(), "both edges touch the hidden node b — neither may be emitted");
    }

    #[test]
    fn build_node_instances_emits_no_instance_for_a_hidden_node() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(5.0, 0.0, 0.0)];
        let mesh = unit_mesh();
        let hidden: HashSet<NodeIndex> = [b].into_iter().collect();

        let nodes = build_node_instances(&graph, &particles, &mesh, &hidden, DEFAULT_NODE_MATERIAL);

        assert_eq!(nodes.len(), 1, "the hidden node must emit zero instances, the other node must still emit one");
        assert_eq!(nodes[0].translation, Vec3::new(0.0, 0.0, 0.0), "the surviving instance must belong to node a, not the hidden node b");
    }

    #[test]
    fn build_scene_lights_the_scene_so_lit_tints_are_not_ambient_only_black() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "x", 1.0);
        let particles = vec![Particle::at3(0.0, 0.0, 0.0)];
        let node_mesh = unit_mesh();
        let edge_mesh = unit_edge_quad_mesh();

        let scene = build_scene(&graph, &particles, &node_mesh, &edge_mesh, &HashSet::new(), &Graph3DLighting::default(), &Graph3DEdgeStyle::default());

        assert_eq!(scene.nodes.len(), 1);
        assert!(!scene.lights.is_empty());
    }

    #[test]
    fn build_scene_hides_a_node_and_its_touching_edges_when_given_a_non_empty_hidden_set() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(5.0, 0.0, 0.0)];
        let node_mesh = unit_mesh();
        let edge_mesh = unit_edge_quad_mesh();
        let hidden: HashSet<NodeIndex> = [b].into_iter().collect();

        let scene = build_scene(&graph, &particles, &node_mesh, &edge_mesh, &hidden, &Graph3DLighting::default(), &Graph3DEdgeStyle::default());

        assert_eq!(scene.nodes.len(), 1, "one surviving node sphere, zero edges (the edge touches the hidden node)");
    }

    #[test]
    fn build_cluster_edge_instances_draws_one_edge_per_aggregated_outside_neighbor() {
        let mut graph = DemoGraph::new();
        let outside = graph.push_node((), "outside", "x", 4.0);
        let mut members = Vec::new();
        for i in 0..3 {
            members.push(graph.push_node((), format!("m{i}"), "cluster", 4.0));
        }
        graph.push_edge(members[0], outside, 2.0, ());
        graph.push_edge(outside, members[1], 3.0, ());
        let mut particles = vec![Particle::at3(0.0, 0.0, 0.0); graph.node_count()];
        particles[outside.index()] = Particle::at3(50.0, 0.0, 0.0);

        let mut registry = ClusterRegistry::default();
        let id = registry.define(&graph, members.clone()).expect("non-empty cluster");
        assert!(registry.collapse_3d(id, &mut graph, &mut particles));

        let mesh = unit_edge_quad_mesh();
        let edges = build_cluster_edge_instances(&particles, &mesh, &registry, CLUSTER_EDGE_TINT);

        assert_eq!(edges.len(), 1, "both raw cross-cluster edges aggregate onto the same outside node, so exactly one synthetic edge is drawn");
        assert_eq!(edges[0].color_tint, CLUSTER_EDGE_TINT);
    }

    // ── Wave 4: GPU color-ID picking encode/decode ─────────────────────

    #[test]
    fn forward_channel_byte_round_trips_through_linear_for_target_byte_at_every_level_boundary() {
        for level in 0..GPU_PICK_ID_LEVELS {
            let target = level_to_byte(level);
            let x = linear_for_target_byte(target);
            let observed = forward_channel_byte(x);
            assert!(
                observed.abs_diff(target) <= 1,
                "level {level}: target byte {target}, bisection-derived x={x} forward-simulated back to {observed}"
            );
        }
    }

    #[test]
    fn encode_decode_node_id_round_trips_through_this_modules_own_forward_model_across_the_index_range() {
        let mut ids: Vec<u32> = vec![0, 1, 2, 10_000, MAX_GPU_PICKABLE_NODE_INDEX - 1, MAX_GPU_PICKABLE_NODE_INDEX];
        ids.extend((0..=MAX_GPU_PICKABLE_NODE_INDEX).step_by(97));
        for id in ids {
            let tint = encode_node_id_tint(NodeIndex(id));
            let rgba = [forward_channel_byte(tint[0]), forward_channel_byte(tint[1]), forward_channel_byte(tint[2]), 255];
            let decoded = decode_node_id_pixel(rgba);
            assert_eq!(decoded, NodeIndex(id), "round trip failed for id={id}, tint={tint:?}, rgba={rgba:?}");
        }
    }

    #[test]
    fn decode_gpu_pick_pixel_rejects_the_white_background_sentinel_and_out_of_bounds_indices() {
        let white_rgba = [255u8, 255, 255, 255];
        assert_eq!(decode_node_id_pixel(white_rgba).0, MAX_GPU_PICKABLE_NODE_INDEX, "a pure white readback must decode to the reserved sentinel index");
        assert_eq!(decode_gpu_pick_pixel(white_rgba, 50), None, "the sentinel index must never be reported as a real pick, regardless of node_count");

        let tint = encode_node_id_tint(NodeIndex(5));
        let rgba = [forward_channel_byte(tint[0]), forward_channel_byte(tint[1]), forward_channel_byte(tint[2]), 255];
        assert_eq!(decode_gpu_pick_pixel(rgba, 50), Some(NodeIndex(5)), "an in-bounds real id must decode through cleanly");
        assert_eq!(decode_gpu_pick_pixel(rgba, 3), None, "the SAME bytes must be rejected once node_count no longer covers that index");
    }

    #[test]
    fn build_id_pass_scene_emits_one_unlit_node_per_graph_node_tinted_by_its_encoded_id_with_a_white_clear_color() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        let particles = vec![Particle::at3(1.0, 2.0, 3.0), Particle::at3(-4.0, 0.0, 5.0)];
        let id_pass_mesh = Arc::new(build_id_pass_mesh(4, 4));

        let scene = build_id_pass_scene(&graph, &particles, &id_pass_mesh, &HashSet::new());

        assert_eq!(scene.clear_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(scene.nodes.len(), 2);
        assert!(scene.nodes.iter().all(|n| !n.is_lit()), "the id-pass must use Unlit geometry, not the Lit/Phong pipeline");
        assert_eq!(scene.nodes[0].color_tint, encode_node_id_tint(a));
        assert_eq!(scene.nodes[0].translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(scene.nodes[1].color_tint, encode_node_id_tint(b));
        assert_eq!(scene.nodes[1].translation, Vec3::new(-4.0, 0.0, 5.0));
    }

    /// Graph-strengthening arc G1.2: a hidden node (cluster-collapsed,
    /// local-subgraph-excluded, or filter-excluded) must emit NO id-pass
    /// sphere at all — before this fix it always did, so its invisible
    /// sphere could occlude a visible node's own pixel in the id-pass and
    /// a GPU pick could resolve to a `NodeIndex` the user could never have
    /// clicked via CPU picking.
    #[test]
    fn build_id_pass_scene_emits_no_node_for_a_hidden_node() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 2.0);
        let b = graph.push_node((), "b", "x", 2.0);
        let particles = vec![Particle::at3(1.0, 2.0, 3.0), Particle::at3(-4.0, 0.0, 5.0)];
        let id_pass_mesh = Arc::new(build_id_pass_mesh(4, 4));
        let hidden: HashSet<NodeIndex> = [b].into_iter().collect();

        let scene = build_id_pass_scene(&graph, &particles, &id_pass_mesh, &hidden);

        assert_eq!(scene.nodes.len(), 1, "the hidden node must not emit an id-pass sphere");
        assert_eq!(scene.nodes[0].color_tint, encode_node_id_tint(a), "the remaining visible node must still be present and correctly tinted");
    }

    // ── Wave 5: 3D reference grid + axis tick labels ────────────────────

    #[test]
    fn snap_to_nice_step_always_lands_on_a_1_2_5_ladder_rung_across_a_wide_magnitude_range() {
        fn is_nice(step: f64) -> bool {
            for k in -6..=6 {
                let base = 10f64.powi(k);
                for c in [1.0, 2.0, 5.0] {
                    if (step - c * base).abs() < base * 1e-6 {
                        return true;
                    }
                }
            }
            false
        }
        let mut raw = 0.001_f64;
        while raw <= 10_000.0 {
            let step = snap_to_nice_step(raw);
            assert!(is_nice(step), "raw={raw} produced a non-ladder step={step}");
            raw *= 1.31;
        }
    }

    #[test]
    fn snap_to_nice_step_is_monotonic_non_decreasing_in_raw() {
        let mut raw = 0.001_f64;
        let mut previous = snap_to_nice_step(raw);
        while raw <= 10_000.0 {
            let step = snap_to_nice_step(raw);
            assert!(step >= previous - 1e-9, "raw={raw}: step {step} regressed below previous {previous}");
            previous = step;
            raw *= 1.05;
        }
    }

    #[test]
    fn grid_step_for_scale_matches_the_snap_to_nice_step_of_the_target_px_formula() {
        let distance = 500.0_f32;
        let fov_y = 60_f32.to_radians();
        let viewport_height_px = 900.0_f64;
        let half_fov_tan = (fov_y * 0.5).tan() as f64;
        let px_per_world_unit = (viewport_height_px * 0.5) / (distance as f64 * half_fov_tan);
        let config = Graph3DGridConfig::default();
        let expected = snap_to_nice_step(config.target_screen_px / px_per_world_unit);
        assert_eq!(grid_step_for_scale(distance, fov_y, viewport_height_px, &config), expected);
    }

    #[test]
    fn grid_step_for_scale_produces_apparent_spacing_within_the_target_band() {
        let fov_y = 60_f32.to_radians();
        let viewport_height_px = 900.0_f64;
        let config = Graph3DGridConfig::default();
        for distance in [10.0_f32, 100.0, 500.0, 5_000.0, 50_000.0] {
            let step = grid_step_for_scale(distance, fov_y, viewport_height_px, &config);
            let half_fov_tan = (fov_y * 0.5).tan() as f64;
            let px_per_world_unit = (viewport_height_px * 0.5) / (distance as f64 * half_fov_tan);
            let apparent_px = step * px_per_world_unit;
            assert!(
                apparent_px >= GRID_MIN_SCREEN_PX - 1e-6 && apparent_px <= GRID_MAX_SCREEN_PX + 1e-6,
                "distance={distance} step={step} apparent_px={apparent_px} escaped [{GRID_MIN_SCREEN_PX}, {GRID_MAX_SCREEN_PX}]"
            );
        }
    }

    /// Wave G2b configurability gate: a caller-supplied `target_screen_px`
    /// must actually shift the snapped step, not just exist as an unread
    /// field.
    #[test]
    fn grid_step_for_scale_shifts_with_a_caller_supplied_target_screen_px() {
        let distance = 500.0_f32;
        let fov_y = 60_f32.to_radians();
        let viewport_height_px = 900.0_f64;
        let default_step = grid_step_for_scale(distance, fov_y, viewport_height_px, &Graph3DGridConfig::default());
        let wide_config = Graph3DGridConfig { target_screen_px: 400.0, ..Graph3DGridConfig::default() };
        let wide_step = grid_step_for_scale(distance, fov_y, viewport_height_px, &wide_config);
        assert!(wide_step > default_step, "a larger target_screen_px must produce a coarser (larger) step: default={default_step} wide={wide_step}");
    }

    #[test]
    fn build_grid_plan_line_count_matches_x_and_z_tick_counts_for_a_known_aabb_and_step() {
        let min = Vec3::new(-12.0, -3.0, -7.0);
        let max = Vec3::new(22.0, 5.0, 18.0);
        let step = 10.0;

        let plan = build_grid_plan(min, max, step, &Graph3DGridConfig::default());

        let expected_x_ticks = ((min.x as f64 / step).floor() as i64..=(max.x as f64 / step).ceil() as i64).count();
        let expected_z_ticks = ((min.z as f64 / step).floor() as i64..=(max.z as f64 / step).ceil() as i64).count();
        assert_eq!(plan.lines.len(), expected_x_ticks + expected_z_ticks);
        assert_eq!(plan.step, step);
    }

    #[test]
    fn build_grid_plan_marks_every_5th_tick_from_world_origin_as_strong() {
        let config = Graph3DGridConfig::default();
        let plan = build_grid_plan(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 10.0, &config);
        // A degenerate point AABB still yields exactly one x-tick and one
        // z-tick, both at world coordinate 0 — index 0 is always strong.
        assert!(plan.lines.iter().all(|l| l.strong), "tick index 0 (world origin) must always be a strong line");

        let plan = build_grid_plan(Vec3::new(0.0, 0.0, -5.0), Vec3::new(50.0, 0.0, 5.0), 10.0, &config);
        let x_strong: Vec<f64> = plan.lines.iter().filter(|l| l.strong && l.to.z != l.from.z).map(|l| l.tick_value).collect();
        for value in &x_strong {
            let index = (value / 10.0).round() as i64;
            assert_eq!(index.rem_euclid(5), 0, "strong tick {value} must land on a multiple-of-5 index");
        }
    }

    /// Wave G2b configurability gate: a caller-supplied `strong_line_every`
    /// must actually change which ticks are marked strong.
    #[test]
    fn build_grid_plan_honors_a_caller_supplied_strong_line_every() {
        let config = Graph3DGridConfig { strong_line_every: 3, ..Graph3DGridConfig::default() };
        let plan = build_grid_plan(Vec3::new(0.0, 0.0, -30.0), Vec3::new(0.0, 0.0, 30.0), 10.0, &config);
        for line in &plan.lines {
            let index = (line.tick_value / 10.0).round() as i64;
            assert_eq!(line.strong, index.rem_euclid(3) == 0, "tick {} strong={} must follow strong_line_every=3", line.tick_value, line.strong);
        }
    }

    #[test]
    fn build_grid_instances_emits_one_node_per_planned_line_tinted_by_strong_alpha() {
        let config = Graph3DGridConfig::default();
        let plan = build_grid_plan(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 0.0, 10.0), 10.0, &config);
        let mesh = unit_edge_quad_mesh();

        let nodes = build_grid_instances(&plan, &mesh, &config);

        assert_eq!(nodes.len(), plan.lines.len());
        for (node, line) in nodes.iter().zip(plan.lines.iter()) {
            assert!(matches!(node.geometry, uzor_urx_3d::NodeMesh::Line(_)));
            let expected_alpha = if line.strong { config.strong_line_alpha } else { config.line_alpha };
            assert!((node.color_tint[3] - expected_alpha).abs() < 1e-6);
        }
    }

    /// Wave G2b configurability gate: a caller-supplied grid tint/alpha
    /// must actually paint the grid lines.
    #[test]
    fn build_grid_instances_uses_the_caller_supplied_tint_and_alpha() {
        let config = Graph3DGridConfig::default();
        let plan = build_grid_plan(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 0.0, 10.0), 10.0, &config);
        let mesh = unit_edge_quad_mesh();
        let custom = Graph3DGridConfig { tint_rgb: [1.0, 0.0, 1.0], line_alpha: 0.9, strong_line_alpha: 0.99, ..Graph3DGridConfig::default() };

        let nodes = build_grid_instances(&plan, &mesh, &custom);

        for (node, line) in nodes.iter().zip(plan.lines.iter()) {
            let expected_alpha = if line.strong { 0.99 } else { 0.9 };
            assert_eq!(node.color_tint, [1.0, 0.0, 1.0, expected_alpha]);
        }
    }

    #[test]
    fn build_grid_instances_skips_a_degenerate_zero_length_line() {
        let plan = GridPlan { lines: vec![GridLine { from: Vec3::ZERO, to: Vec3::ZERO, tick_value: 0.0, strong: true }], step: 1.0 };
        let mesh = unit_edge_quad_mesh();
        assert!(build_grid_instances(&plan, &mesh, &Graph3DGridConfig::default()).is_empty());
    }

    #[test]
    fn format_tick_value_is_integer_at_or_above_a_step_of_one_and_decimal_below_it() {
        assert_eq!(format_tick_value(42.0, 10.0), "42");
        assert_eq!(format_tick_value(-3.0, 1.0), "-3");
        assert_eq!(format_tick_value(1.5, 0.5), "1.5");
        assert_eq!(format_tick_value(0.07, 0.05), "0.07");
    }
}
