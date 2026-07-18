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
use uzor_urx_3d::{Light, MeshLit, Node, Scene3D};

use crate::graph::Graph;
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
}
