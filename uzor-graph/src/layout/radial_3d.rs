//! `RadialLayout3D` — the 3D sibling of [`super::radial::RadialLayout`]:
//! BFS depth SHELLS instead of Kahn longest-path layering — each depth
//! `d` sits on the surface of a sphere of radius `d * shell_spacing`
//! centered on the root(s) (root itself AT the origin, `d = 0`). Unlike
//! [`super::hierarchical_3d::HierarchicalLayout3D`] (which reuses
//! [`super::layering::compute_layering`]), this layout runs its OWN
//! multi-source breadth-first search: a genuine shortest-path depth
//! (rather than Kahn's longest-path layer number) is the natural notion
//! of "shell" for a sphere-based radial layout, and the direction-fanning
//! scheme below needs an explicit parent/children TREE (not just a layer
//! number), which the BFS builds directly.
//!
//! Node DIRECTION (the unit vector from the origin a node's shell
//! position sits along) is inherited from its own parent's direction,
//! nudged by a deterministic GOLDEN-ANGLE azimuth spread among siblings —
//! literally "children fan around the parent's own radial direction."
//! The golden angle (`≈137.5°`, the irrational rotation increment behind
//! Fibonacci/sunflower spirals) is used instead of a plain `TAU / count`
//! even split because it needs no known upfront `count`: it never has to
//! be told how many siblings there'll be to stay well-distributed, sibling
//! `i`'s direction depends only on its own index. A node's own direction
//! then seeds ITS children's fan the same way, one BFS layer at a time —
//! so a subtree genuinely spreads outward through 3D space as depth
//! grows, not just around a single shared axis. Multiple roots are
//! themselves treated as siblings of a virtual center (seeded from
//! world-`Y`) so independent trees fan apart from each other too, while
//! every root still renders at the literal origin (`d = 0` has no
//! meaningful direction of its own). One-shot: [`Layout::tick`] computes
//! positions once then reports settled forever; [`Layout::reheat`] forces
//! a recompute — same freeze semantics as [`super::radial::RadialLayout`].

use std::collections::VecDeque;

use glam::Vec3;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::{Layout, LayoutTickResult};

/// Irrational golden-angle increment (`TAU / phi^2`, `phi` = the golden
/// ratio) — the standard Fibonacci-spiral azimuth step: applying it
/// `sibling_index` times spreads any number of siblings around a circle
/// without ever repeating or clustering, and without needing to know the
/// total sibling count upfront.
const GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Half-angle of the cone multiple roots (treated as siblings of a
/// virtual center) fan around world-`Y` by — only visible when more than
/// one root exists; a single root's own seed direction is still
/// deterministic but otherwise meaningless (the root itself renders at
/// the literal origin regardless).
const ROOT_SPREAD_HALF_ANGLE: f32 = 1.047_198; // 60 degrees, radians

/// Half-angle of the cone a node's children fan around ITS OWN direction
/// by — narrow enough that a subtree reads as branches continuing
/// outward from the trunk rather than doubling back on themselves.
const CHILD_CONE_HALF_ANGLE: f32 = 0.698_132; // 40 degrees, radians

#[derive(Debug, Clone)]
pub struct RadialParams3D {
    /// World-unit radius added per BFS depth (`radius = depth *
    /// shell_spacing`).
    pub shell_spacing: f32,
    /// Explicit root nodes; empty = auto-detect every zero-in-degree
    /// node as a root, same convention as [`super::layering::compute_layering`]
    /// (a fully-cyclic graph with no such node falls back to node `0`).
    pub roots: Vec<NodeIndex>,
}

impl Default for RadialParams3D {
    fn default() -> Self {
        Self { shell_spacing: 90.0, roots: Vec::new() }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadialLayout3D {
    params: RadialParams3D,
    computed: bool,
}

impl RadialLayout3D {
    pub fn new(params: RadialParams3D) -> Self {
        Self { params, computed: false }
    }

    pub fn params(&self) -> &RadialParams3D {
        &self.params
    }

    /// Replace the params and force a recompute on the next `tick`.
    pub fn set_params(&mut self, params: RadialParams3D) {
        self.params = params;
        self.computed = false;
    }
}

/// Multi-source BFS tree over `topo`'s directed edges — `depth`/`parent`/
/// `children` (children in edge-discovery order, i.e. deterministic given
/// `topo.edges`'s own order), `roots` in visitation order (explicit roots
/// first, then any auto-detected zero-in-degree node, then any node still
/// unreached — a disconnected component becomes its own secondary root,
/// same "then any still-unvisited node" convention
/// [`super::layering::compute_layering`]'s own algorithm uses). A back
/// edge (landing on an already-visited node) is simply never traversed —
/// real cycle-breaking, not a special case.
struct BfsTree {
    depth: Vec<u32>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
}

/// Drain `queue` via plain BFS over `adjacency`, filling `visited`/
/// `depth`/`children` as it goes — shared by the initial root-seeded
/// pass and the disconnected-component fallback pass below, so there's
/// exactly one "walk the frontier" code path.
fn bfs_fill(adjacency: &[Vec<usize>], visited: &mut [bool], depth: &mut [u32], children: &mut [Vec<usize>], queue: &mut VecDeque<usize>) {
    while let Some(u) = queue.pop_front() {
        for &v in &adjacency[u] {
            if !visited[v] {
                visited[v] = true;
                depth[v] = depth[u] + 1;
                children[u].push(v);
                queue.push_back(v);
            }
        }
    }
}

fn compute_bfs_tree(topo: &SimTopology<'_>, explicit_roots: &[NodeIndex]) -> BfsTree {
    let n = topo.node_count;
    let mut depth = vec![0u32; n];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut visited = vec![false; n];

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in topo.edges {
        let a = e.from.index();
        let b = e.to.index();
        if a < n && b < n && a != b {
            adjacency[a].push(b);
        }
    }

    let mut roots: Vec<usize> = explicit_roots.iter().map(|r| r.index()).filter(|&i| i < n).collect();
    if roots.is_empty() {
        let mut in_degree = vec![0u32; n];
        for e in topo.edges {
            let a = e.from.index();
            let b = e.to.index();
            if a < n && b < n && a != b {
                in_degree[b] += 1;
            }
        }
        roots = (0..n).filter(|&i| in_degree[i] == 0).collect();
    }
    if roots.is_empty() && n > 0 {
        roots = vec![0];
    }

    let mut root_order: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    for &r in &roots {
        if !visited[r] {
            visited[r] = true;
            depth[r] = 0;
            root_order.push(r);
            queue.push_back(r);
        }
    }
    bfs_fill(&adjacency, &mut visited, &mut depth, &mut children, &mut queue);

    // Disconnected components: any still-unvisited node becomes its own
    // secondary root (ascending index order — deterministic), same
    // "then any still-unvisited node" convention
    // `layering.rs`'s own algorithm uses.
    for i in 0..n {
        if !visited[i] {
            visited[i] = true;
            depth[i] = 0;
            root_order.push(i);
            queue.push_back(i);
            bfs_fill(&adjacency, &mut visited, &mut depth, &mut children, &mut queue);
        }
    }

    BfsTree { depth, children, roots: root_order }
}

/// A stable orthonormal `(u, v)` basis perpendicular to unit vector
/// `dir` — picks whichever world axis (`X` or `Y`) is LEAST aligned with
/// `dir` as a reference to cross against, so the construction never
/// degenerates regardless of `dir`'s own orientation.
fn orthonormal_basis(dir: Vec3) -> (Vec3, Vec3) {
    let reference = if dir.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    let u = dir.cross(reference).normalize_or_zero();
    let v = dir.cross(u);
    (u, v)
}

/// One child's direction: `parent_dir` tilted by `cone_half_angle` toward
/// an azimuth of `sibling_index * GOLDEN_ANGLE` around `parent_dir`'s own
/// axis. Always unit-length by construction (`{parent_dir, u, v}` is
/// orthonormal, so `cos(theta)^2 + sin(theta)^2 * (cos(phi)^2 +
/// sin(phi)^2) == 1` exactly).
fn child_direction(parent_dir: Vec3, sibling_index: usize, cone_half_angle: f32) -> Vec3 {
    let (u, v) = orthonormal_basis(parent_dir);
    let phi = sibling_index as f32 * GOLDEN_ANGLE;
    let (sin_c, cos_c) = cone_half_angle.sin_cos();
    let (sin_p, cos_p) = phi.sin_cos();
    let dir = parent_dir * cos_c + (u * cos_p + v * sin_p) * sin_c;
    dir.normalize_or_zero()
}

/// One outward unit direction per node — roots seeded from world-`Y` (a
/// virtual center every root fans around, see this module's own doc
/// comment), every other node's direction fanned from its own parent's
/// via [`child_direction`]. Processed in BFS order (roots first, then
/// each node's own children only once its own direction is known) so a
/// child's fan always has a real parent direction to nudge.
fn assign_directions(tree: &BfsTree) -> Vec<Vec3> {
    let n = tree.depth.len();
    let mut dir = vec![Vec3::Y; n];
    for (i, &r) in tree.roots.iter().enumerate() {
        dir[r] = child_direction(Vec3::Y, i, ROOT_SPREAD_HALF_ANGLE);
    }
    let mut queue: VecDeque<usize> = tree.roots.iter().copied().collect();
    while let Some(u) = queue.pop_front() {
        for (i, &v) in tree.children[u].iter().enumerate() {
            dir[v] = child_direction(dir[u], i, CHILD_CONE_HALF_ANGLE);
            queue.push_back(v);
        }
    }
    dir
}

impl Layout for RadialLayout3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        if self.computed {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
        let tree = compute_bfs_tree(topo, &self.params.roots);
        let dir = assign_directions(&tree);

        for i in 0..tree.depth.len() {
            let Some(p) = particles.get_mut(i) else { continue };
            if let (Some(fx), Some(fy), Some(fz)) = (p.fx, p.fy, p.fz) {
                p.x = fx;
                p.y = fy;
                p.z = fz;
                p.vx = 0.0;
                p.vy = 0.0;
                p.vz = 0.0;
                continue;
            }
            let radius = tree.depth[i] as f32 * self.params.shell_spacing;
            let d = dir[i];
            p.x = d.x * radius;
            p.y = d.y * radius;
            p.z = d.z * radius;
            p.vx = 0.0;
            p.vy = 0.0;
            p.vz = 0.0;
        }
        self.computed = true;
        LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true }
    }

    fn reheat(&mut self, _alpha: f32) {
        self.computed = false;
    }

    fn is_settled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SimEdge;

    fn topo(node_count: usize, edges: &[SimEdge]) -> SimTopology<'_> {
        SimTopology { node_count, edges, degree: &[], radii: vec![1.0; node_count] }
    }

    fn e(from: u32, to: u32) -> SimEdge {
        SimEdge { from: NodeIndex(from), to: NodeIndex(to), weight: 1.0 }
    }

    fn radius(p: &Particle) -> f32 {
        (p.x * p.x + p.y * p.y + p.z * p.z).sqrt()
    }

    #[test]
    fn root_sits_exactly_at_the_origin() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_eq!((particles[0].x, particles[0].y, particles[0].z), (0.0, 0.0, 0.0));
    }

    #[test]
    fn depth_shells_have_strictly_increasing_radius_with_depth_and_no_nan() {
        let edges = [e(0, 1), e(1, 2), e(2, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        for p in &particles {
            assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
        }
        let radii: Vec<f32> = particles.iter().map(radius).collect();
        for pair in radii.windows(2) {
            assert!(pair[1] > pair[0] + 1e-3, "shell radius must strictly increase with depth: {radii:?}");
        }
    }

    #[test]
    fn siblings_at_the_same_depth_dont_collapse_onto_one_point() {
        let edges = [e(0, 1), e(0, 2), e(0, 3), e(0, 4), e(0, 5)];
        let t = topo(6, &edges);
        let mut particles = vec![Particle::default(); 6];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let mut positions: Vec<(f32, f32, f32)> = (1..=5).map(|i| (particles[i].x, particles[i].y, particles[i].z)).collect();
        positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in positions.windows(2) {
            assert_ne!(pair[0], pair[1], "siblings at the same depth must not collapse onto one point");
        }
        // Every sibling shares the same shell radius (same depth).
        let r0 = radius(&particles[1]);
        for i in 2..=5 {
            assert!((radius(&particles[i]) - r0).abs() < 1e-3, "same-depth siblings must share the same shell radius");
        }
    }

    #[test]
    fn one_shot_layout_freezes_after_first_tick() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let after_first = particles.clone();

        particles[1].x = 999.0;
        let r2 = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r2.settled);
        assert_eq!(particles[1].x, 999.0);
        assert_eq!(particles[0], after_first[0]);
    }

    #[test]
    fn reheat_forces_a_fresh_recompute() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        particles[1].x = 999.0;
        layout.reheat(1.0);
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_ne!(particles[1].x, 999.0, "reheat must force a fresh compute, overwriting the manual nudge");
    }

    #[test]
    fn pinned_particle_is_never_overwritten_by_the_bfs_shell_math() {
        let edges = [e(0, 1), e(0, 2)];
        let t = topo(3, &edges);
        let mut particles = vec![Particle::default(); 3];
        particles[1].pin3(42.0, -7.0, 13.0);
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert_eq!((particles[1].x, particles[1].y, particles[1].z), (42.0, -7.0, 13.0));
    }

    #[test]
    fn radial_3d_layout_is_deterministic_across_repeated_ticks_after_reheat() {
        let edges = [e(0, 1), e(0, 2), e(1, 3)];
        let t = topo(4, &edges);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let first: Vec<(f32, f32, f32)> = particles.iter().map(|p| (p.x, p.y, p.z)).collect();

        layout.reheat(1.0);
        layout.tick(&t, &mut particles, 1.0 / 60.0);
        let second: Vec<(f32, f32, f32)> = particles.iter().map(|p| (p.x, p.y, p.z)).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn a_cyclic_graph_still_layers_cleanly_with_no_nan_or_panic() {
        // A pure 3-cycle, no zero-in-degree node — node 0 is the
        // fallback root, edge 2->0 is the back edge (never traversed).
        let edges = [e(0, 1), e(1, 2), e(2, 0)];
        let t = topo(3, &edges);
        let mut particles = vec![Particle::default(); 3];
        let mut layout = RadialLayout3D::default();
        let result = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(result.settled);
        for p in &particles {
            assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
        }
        let radii: Vec<f32> = particles.iter().map(radius).collect();
        assert_eq!(radii[0], 0.0);
        assert!(radii[1] > radii[0]);
        assert!(radii[2] > radii[1]);
    }

    #[test]
    fn empty_topology_produces_no_particles_touched_and_settles() {
        let t = topo(0, &[]);
        let mut particles: Vec<Particle> = Vec::new();
        let mut layout = RadialLayout3D::default();
        let result = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(result.settled);
    }
}
