//! `build_scene` helpers — instanced sphere nodes + cylinder edges (W3D
//! arc plan §1.3, Wave 2). [`GraphEngine3D::build_scene`](crate::engine3d::GraphEngine3D::build_scene)
//! wires straight into [`build_scene`] here; the split exists so the
//! node/edge instance-construction logic is unit-testable without a
//! `GraphEngine3D` (or a GPU) at all.
//!
//! **Nodes**: one `uzor_urx_3d::Node::new_lit` per graph node, sharing
//! the caller-supplied unit-sphere `node_mesh` — translated to the
//! node's simulated `(x, y, z)`, scaled by its `radius`, tinted by its
//! category (reuses [`crate::render::category_color`]'s existing
//! deterministic hash-palette, converted from the 2D hex-string
//! convention to the `[f32; 4]` `uzor_urx_3d::Node::color_tint` needs).
//!
//! **Edges**: one `Node::new_lit` per graph edge, sharing the
//! caller-supplied unit-cylinder `edge_mesh`.
//!
//! **Divergence from the plan's literal text (`uzor-graph/CLAUDE.md`)**:
//! §1.3 says `translation = midpoint(a, b)`. That's only correct for a
//! cylinder mesh centred on its own local origin (base at
//! `y = -height/2`, top at `y = +height/2`) — three.js's
//! `CylinderGeometry`, for instance, is built that way. `uzor-urx-3d`'s
//! `MeshLit::cylinder` (`mesh.rs:520-580`, confirmed by direct read) is
//! NOT centred: base ring at local `y = 0`, top ring at local
//! `y = height`. With `Mat4::from_scale_rotation_translation` applying
//! scale-then-rotation-then-translation, a vertex at local `y = 0` lands
//! exactly at `translation` after the transform — so `translation` must
//! be the edge's FROM endpoint, not its midpoint, or the drawn cylinder
//! only covers the far half of the edge (translation..translation +
//! `length`·direction) and visibly floats away from the near endpoint.
//! Fixed forward here; `rotation`/`scale.y` still match the plan's own
//! text (`Quat::from_rotation_arc(Vec3::Y, dir)`, `scale.y = length`).

use std::sync::Arc;

use glam::{Quat, Vec3};
use uzor_urx_3d::{Light, Mesh, MeshLit, Node, Scene3D, Vertex};

use crate::graph::{Graph, NodeIndex};
use crate::particle::Particle;
use crate::render::category_color;

/// Edge cylinder radius/depth, world units — plan §1.3's `edge_width`.
/// Deliberately thin relative to a typical node radius (demo nodes run
/// `3.0..=12.0` world units, `force_graph_demo.rs`) so edges read as
/// links rather than competing with node spheres for visual weight.
pub const DEFAULT_EDGE_WIDTH: f32 = 0.6;

/// Convert [`category_color`]'s fixed `"#rrggbb"` palette into an opaque
/// `[f32; 4]` tint — `Node::color_tint` takes floats, not a CSS-style hex
/// string, and there's no shared hex-parser in this crate to reuse
/// (`uzor::ui::widgets::atomic::slider` has one, but it's `u8`-typed and
/// private to that module) — small enough to own here rather than reach
/// into an unrelated widget's internals for four `u8::from_str_radix`
/// calls.
fn category_tint(category: &str) -> [f32; 4] {
    let hex = category_color(category).trim_start_matches('#');
    if hex.len() != 6 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// One instanced `Node::new_lit` per graph node — see the module doc.
pub fn build_node_instances<N, E>(graph: &Graph<N, E>, particles: &[Particle], mesh: &Arc<MeshLit>) -> Vec<Node> {
    graph
        .nodes()
        .filter_map(|(id, node)| {
            let p = particles.get(id.index())?;
            Some(
                Node::new_lit(mesh.clone())
                    .with_translation(Vec3::new(p.x, p.y, p.z))
                    .with_scale(Vec3::splat(node.radius.max(0.01)))
                    .with_tint(category_tint(&node.category)),
            )
        })
        .collect()
}

/// One instanced `Node::new_lit` per graph edge — see the module doc's
/// divergence note for why `translation` is the FROM endpoint, not the
/// midpoint. A coincident (zero-length) edge has no well-defined
/// direction and is skipped rather than emitting a NaN rotation.
pub fn build_edge_instances<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    mesh: &Arc<MeshLit>,
    edge_width: f32,
) -> Vec<Node> {
    graph
        .edges()
        .filter_map(|(_, edge)| {
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
                Node::new_lit(mesh.clone())
                    .with_translation(from)
                    .with_rotation(rotation)
                    .with_scale(Vec3::new(edge_width.max(0.001), length, edge_width.max(0.001))),
            )
        })
        .collect()
}

/// Key light + ambient floor bright enough that `MeshLit` category tints
/// stay legible. Not specified by the plan's §1.3 text — `Scene3D::default()`'s
/// own dim ambient (`[0.08, 0.08, 0.10]`, `uzor-urx-3d/src/scene3d.rs`)
/// alone renders every `MeshLit` node near-black (proven by
/// `uzor-urx-3d/tests/lighting.rs`'s own "no lights pushed" case, which
/// only asserts a *visible* result because it bumps `scene.ambient` to
/// `[0.5, 0.5, 0.5]` first) — a genuinely required part of making
/// `build_scene`'s output visually distinct, not an optional flourish.
fn arm_default_lighting(scene: &mut Scene3D) {
    scene.ambient = [0.28, 0.28, 0.32];
    scene.push_light(Light::directional(Vec3::new(-0.4, -1.0, -0.3), [1.0, 1.0, 1.0], 1.0));
}

/// Build the full 3D scene (plan §1.3): every node as an instanced
/// sphere sharing `node_mesh`, every edge as an instanced cylinder
/// sharing `edge_mesh` — two draw calls total regardless of graph size
/// (`uzor-urx-3d`'s `MeshCache` Arc-identity dedup, confirmed real in
/// the plan's own substrate verdict §0) — plus a default key light.
pub fn build_scene<N, E>(
    graph: &Graph<N, E>,
    particles: &[Particle],
    node_mesh: &Arc<MeshLit>,
    edge_mesh: &Arc<MeshLit>,
    edge_width: f32,
) -> Scene3D {
    let mut scene = Scene3D::new();
    arm_default_lighting(&mut scene);
    scene.nodes.extend(build_edge_instances(graph, particles, edge_mesh, edge_width));
    scene.nodes.extend(build_node_instances(graph, particles, node_mesh));
    scene
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
pub fn build_id_pass_scene<N, E>(graph: &Graph<N, E>, particles: &[Particle], id_pass_mesh: &Arc<Mesh>) -> Scene3D {
    let mut scene = Scene3D::new();
    scene.clear_color = [1.0, 1.0, 1.0, 1.0];
    scene.nodes = graph
        .nodes()
        .filter_map(|(id, node)| {
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

    #[test]
    fn build_node_instances_emits_one_lit_node_per_graph_node_with_translation_scale_and_tint() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "cat-a", 2.0);
        let particles = vec![Particle::at3(1.0, 2.0, 3.0)];
        let mesh = unit_mesh();

        let nodes = build_node_instances(&graph, &particles, &mesh);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(nodes[0].scale, Vec3::splat(2.0));
        assert_eq!(nodes[0].color_tint, category_tint("cat-a"));
        assert!(nodes[0].is_lit());
    }

    #[test]
    fn build_edge_instances_places_the_translation_at_the_from_endpoint_not_the_midpoint() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 5.0, 0.0)];
        let mesh = unit_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, 0.5);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].translation, Vec3::ZERO, "translation must be the FROM endpoint, not the midpoint — see the module doc");
        assert!((edges[0].scale.y - 5.0).abs() < 1e-5);
        assert!((edges[0].scale.x - 0.5).abs() < 1e-5);
        // b - a is already +Y, the cylinder's own local axis, so the
        // rotation should be (near-)identity.
        let rotated_axis = edges[0].rotation * Vec3::Y;
        assert!((rotated_axis - Vec3::Y).length() < 1e-4);
    }

    #[test]
    fn build_edge_instances_rotation_aligns_the_cylinder_axis_to_an_arbitrary_edge_direction() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(3.0, 4.0, 0.0)];
        let mesh = unit_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, 0.5);

        let expected_dir = Vec3::new(3.0, 4.0, 0.0).normalize();
        let rotated_axis = edges[0].rotation * Vec3::Y;
        assert!((rotated_axis - expected_dir).length() < 1e-4);
        assert!((edges[0].scale.y - 5.0).abs() < 1e-4);
    }

    #[test]
    fn build_edge_instances_skips_a_coincident_degenerate_edge() {
        let mut graph = DemoGraph::new();
        let a = graph.push_node((), "a", "x", 1.0);
        let b = graph.push_node((), "b", "x", 1.0);
        graph.push_edge(a, b, 1.0, ());
        let particles = vec![Particle::at3(2.0, 2.0, 2.0), Particle::at3(2.0, 2.0, 2.0)];
        let mesh = unit_mesh();

        let edges = build_edge_instances(&graph, &particles, &mesh, 0.5);

        assert!(edges.is_empty(), "a zero-length edge has no well-defined direction — must not emit a NaN-rotation node");
    }

    #[test]
    fn build_scene_lights_the_scene_so_lit_tints_are_not_ambient_only_black() {
        let mut graph = DemoGraph::new();
        graph.push_node((), "a", "x", 1.0);
        let particles = vec![Particle::at3(0.0, 0.0, 0.0)];
        let node_mesh = unit_mesh();
        let edge_mesh = unit_mesh();

        let scene = build_scene(&graph, &particles, &node_mesh, &edge_mesh, 0.5);

        assert_eq!(scene.nodes.len(), 1);
        assert!(!scene.lights.is_empty());
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

        let scene = build_id_pass_scene(&graph, &particles, &id_pass_mesh);

        assert_eq!(scene.clear_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(scene.nodes.len(), 2);
        assert!(scene.nodes.iter().all(|n| !n.is_lit()), "the id-pass must use Unlit geometry, not the Lit/Phong pipeline");
        assert_eq!(scene.nodes[0].color_tint, encode_node_id_tint(a));
        assert_eq!(scene.nodes[0].translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(scene.nodes[1].color_tint, encode_node_id_tint(b));
        assert_eq!(scene.nodes[1].translation, Vec3::new(-4.0, 0.0, 5.0));
    }
}
