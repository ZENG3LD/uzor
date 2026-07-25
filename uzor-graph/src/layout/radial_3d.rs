//! `RadialLayout3D` — the 3D sibling of [`super::radial::RadialLayout`]:
//! BFS depth SHELLS instead of Kahn longest-path layering — each NON-root
//! depth `d` sits on the surface of a sphere of radius `d * shell_spacing`
//! centered on the root(s) (`d = 0` — every root — sits on its own
//! shared, nonzero-radius ring instead; see the Wave G3 item 1 fix below
//! for why). Unlike [`super::hierarchical_3d::HierarchicalLayout3D`] (which reuses
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
//! world-`Y`) so independent trees fan apart from each other too.
//!
//! **Wave G3 item 1 fix — roots no longer pin to the literal origin.**
//! Every root previously sat at `radius = depth * shell_spacing` with
//! `depth == 0`, i.e. the literal world origin — for a graph with more
//! than one root (routine for a filtered or naturally-fragmented graph)
//! every root collapsed onto the exact same point, indistinguishable on
//! screen and only one ever ray-pickable, while the per-root fan
//! direction computed above was multiplied by exactly zero — wasted
//! work. Roots now sit on a shared, non-zero-radius ring — [`root_ring_radius`]
//! mirrors [`super::hierarchical_3d::HierarchicalLayout3D`]'s own layer-0
//! ring formula (`(spacing * count / TAU).max(floor)`) exactly, reusing
//! [`RadialParams3D::shell_spacing`] as that spacing input (this
//! module's own existing "radius grows per depth" knob) and the new
//! [`RadialParams3D::min_root_ring_radius`] as the floor — so N roots
//! now land at N distinct points (their already-computed fan directions
//! finally do real work), and a single root sits at a small, sensible
//! nonzero radius instead of being pinned to a meaningless-anyway origin
//! point.
//!
//! **Wave G3 item 2 - `depth_metric`.** [`RadialParams3D::depth_metric`]
//! defaults to [`super::layering::DepthMetric::ShortestPath`] — this
//! module's own PRE-EXISTING behavior via [`compute_bfs_tree`], unchanged
//! — because [`super::radial::RadialLayout`] (2D) defaults to the
//! OPPOSITE metric (`LongestPath`), an unreconciled divergence the layout
//! audit traced concretely (see [`super::layering::DepthMetric`]'s own
//! doc comment). Selecting `LongestPath` here swaps ONLY which depth
//! NUMBER feeds the shell radius (via [`super::layering::compute_layering`]) —
//! the parent/child TREE the golden-angle fan direction walks always
//! comes from this module's own [`compute_bfs_tree`] regardless of
//! metric, since the fan algorithm structurally needs a genuine
//! single-parent tree, which only a real BFS spanning tree guarantees
//! for an arbitrary DAG (Kahn's own `children` can list more than one
//! parent pointing at the same node).
//!
//! **Wave G3 item 3 - `radius_aware_spacing`.** OFF by default (doctrine:
//! no silent output change) — see [`RadialParams3D::radius_aware_spacing`]'s
//! own doc comment. Unlike the other three one-shot layouts (which widen
//! ANGULAR allocation), this module deliberately pushes a large-radius
//! node further OUTWARD along its own already-computed fan direction
//! instead: the golden-angle fan's whole appeal is needing no known
//! upfront sibling count/width to stay well-distributed (see this
//! module's own doc comment above), and widening the fan CONE per
//! sibling would need exactly that — a larger redesign than this item's
//! own scope. A radial push-out reduces (does not fully eliminate)
//! same-shell overlap risk while preserving that design; a full angular
//! treatment is left to a future wave or the owner's own call once
//! they've seen this simpler version in practice.
//!
//! One-shot: [`Layout::tick`] computes positions once then reports
//! settled forever; [`Layout::reheat`] forces a recompute — same freeze
//! semantics as [`super::radial::RadialLayout`].

use std::collections::VecDeque;
use std::f32::consts::TAU;

use glam::Vec3;

use crate::graph::{NodeIndex, SimTopology};
use crate::particle::Particle;

use super::layering::{compute_layering, DepthMetric};
use super::{Layout, LayoutTickResult};

/// Default for [`RadialParams3D::golden_angle`] — irrational golden-angle
/// increment (`TAU / phi^2`, `phi` = the golden ratio), the standard
/// Fibonacci-spiral azimuth step: applying it `sibling_index` times
/// spreads any number of siblings around a circle without ever repeating
/// or clustering, and without needing to know the total sibling count
/// upfront.
const DEFAULT_GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Default for [`RadialParams3D::root_spread_half_angle`] — half-angle of
/// the cone multiple roots (treated as siblings of a virtual center) fan
/// around world-`Y` by — only visible when more than one root exists; a
/// single root's own seed direction is still deterministic (Wave G3 item
/// 1 fix: no longer meaningless either — it now genuinely places the
/// root on its own nonzero-radius ring, see [`root_ring_radius`]).
const DEFAULT_ROOT_SPREAD_HALF_ANGLE: f32 = 1.047_198; // 60 degrees, radians

/// Default for [`RadialParams3D::child_cone_half_angle`] — half-angle of
/// the cone a node's children fan around ITS OWN direction by — narrow
/// enough that a subtree reads as branches continuing outward from the
/// trunk rather than doubling back on themselves.
const DEFAULT_CHILD_CONE_HALF_ANGLE: f32 = 0.698_132; // 40 degrees, radians

/// Default for [`RadialParams3D::min_root_ring_radius`] — Wave G3 item 1
/// fix — floor on the shared ring every root sits on, mirroring
/// [`super::hierarchical_3d::DEFAULT_MIN_RING_RADIUS`] (same value,
/// same role: keeps even a single-root "ring" visibly off the central
/// vertical axis instead of collapsing toward it at a small
/// `shell_spacing`).
const DEFAULT_MIN_ROOT_RING_RADIUS: f32 = 20.0;

#[derive(Debug, Clone)]
pub struct RadialParams3D {
    /// World-unit radius added per BFS depth (`radius = depth *
    /// shell_spacing`) for every NON-root node; also the spacing input
    /// [`root_ring_radius`] reuses for the root ring itself (Wave G3 item
    /// 1 fix — see this module's own doc comment).
    pub shell_spacing: f32,
    /// Explicit root nodes; empty = auto-detect every zero-in-degree
    /// node as a root, same convention as [`super::layering::compute_layering`]
    /// (a fully-cyclic graph with no such node falls back to node `0`).
    pub roots: Vec<NodeIndex>,
    /// Sibling azimuth fan increment [`child_direction`] applies —
    /// see [`DEFAULT_GOLDEN_ANGLE`]'s own doc comment (Wave G2b
    /// configurability — was the private `GOLDEN_ANGLE` constant).
    pub golden_angle: f32,
    /// Half-angle of the cone multiple roots fan around world-`Y` by —
    /// see [`DEFAULT_ROOT_SPREAD_HALF_ANGLE`]'s own doc comment (Wave G2b
    /// configurability — was the private `ROOT_SPREAD_HALF_ANGLE`
    /// constant).
    pub root_spread_half_angle: f32,
    /// Half-angle of the cone a node's children fan around its own
    /// direction by — see [`DEFAULT_CHILD_CONE_HALF_ANGLE`]'s own doc
    /// comment (Wave G2b configurability — was the private
    /// `CHILD_CONE_HALF_ANGLE` constant).
    pub child_cone_half_angle: f32,
    /// Floor on the shared ring every root sits on — see
    /// [`DEFAULT_MIN_ROOT_RING_RADIUS`]'s own doc comment (Wave G3 item 1
    /// fix — new field, no prior constant existed since roots previously
    /// pinned to the literal origin unconditionally).
    pub min_root_ring_radius: f32,
    /// Which depth notion selects a node's shell — see
    /// [`super::layering::DepthMetric`]'s own doc comment (Wave G3 item
    /// 2 fix). Defaults to [`DepthMetric::ShortestPath`] — this module's
    /// own PRE-EXISTING behavior via [`compute_bfs_tree`], unchanged.
    pub depth_metric: DepthMetric,
    /// When `true`, a node's shell radius grows by its OWN
    /// [`SimTopology::radii`] value on top of the ordinary
    /// `depth * shell_spacing` (or root-ring) term, pushing a
    /// larger-than-average node (e.g. a collapsed-cluster supernode)
    /// further outward along its own already-computed fan direction —
    /// see this module's own doc comment (Wave G3 item 3) for why this
    /// is a radial push, not an angular-cone widening like the other
    /// three one-shot layouts. Defaults to `false`: this CHANGES layout
    /// output for any graph with varying node radii, so today's uniform
    /// depth-only radius is preserved unless a caller opts in (doctrine:
    /// no silent output change; recommended to the owner, not flipped
    /// here).
    pub radius_aware_spacing: bool,
}

impl Default for RadialParams3D {
    fn default() -> Self {
        Self {
            shell_spacing: 90.0,
            roots: Vec::new(),
            golden_angle: DEFAULT_GOLDEN_ANGLE,
            root_spread_half_angle: DEFAULT_ROOT_SPREAD_HALF_ANGLE,
            child_cone_half_angle: DEFAULT_CHILD_CONE_HALF_ANGLE,
            min_root_ring_radius: DEFAULT_MIN_ROOT_RING_RADIUS,
            depth_metric: DepthMetric::ShortestPath,
            radius_aware_spacing: false,
        }
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
/// an azimuth of `sibling_index * golden_angle` around `parent_dir`'s own
/// axis. Always unit-length by construction (`{parent_dir, u, v}` is
/// orthonormal, so `cos(theta)^2 + sin(theta)^2 * (cos(phi)^2 +
/// sin(phi)^2) == 1` exactly).
fn child_direction(parent_dir: Vec3, sibling_index: usize, cone_half_angle: f32, golden_angle: f32) -> Vec3 {
    let (u, v) = orthonormal_basis(parent_dir);
    let phi = sibling_index as f32 * golden_angle;
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
fn assign_directions(tree: &BfsTree, golden_angle: f32, root_spread_half_angle: f32, child_cone_half_angle: f32) -> Vec<Vec3> {
    let n = tree.depth.len();
    let mut dir = vec![Vec3::Y; n];
    for (i, &r) in tree.roots.iter().enumerate() {
        dir[r] = child_direction(Vec3::Y, i, root_spread_half_angle, golden_angle);
    }
    let mut queue: VecDeque<usize> = tree.roots.iter().copied().collect();
    while let Some(u) = queue.pop_front() {
        for (i, &v) in tree.children[u].iter().enumerate() {
            dir[v] = child_direction(dir[u], i, child_cone_half_angle, golden_angle);
            queue.push_back(v);
        }
    }
    dir
}

/// Shared ring radius every root sits on — Wave G3 item 1 fix, mirrors
/// [`super::hierarchical_3d::HierarchicalLayout3D`]'s own layer-0 ring
/// formula (`(spacing * count / TAU).max(floor)`) exactly. `root_count`
/// is the number of nodes actually assigned depth `0` under the ACTIVE
/// [`DepthMetric`] (not merely `BfsTree::roots.len()`) so the ring
/// always fits however many nodes really land on it, regardless of
/// which metric picked them.
fn root_ring_radius(root_count: usize, shell_spacing: f32, min_root_ring_radius: f32) -> f32 {
    let count = root_count.max(1) as f32;
    (shell_spacing * count / TAU).max(min_root_ring_radius)
}

impl Layout for RadialLayout3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], _dt: f32) -> LayoutTickResult {
        if self.computed {
            return LayoutTickResult { alpha: 0.0, max_displacement: 0.0, settled: true };
        }
        let tree = compute_bfs_tree(topo, &self.params.roots);
        let dir = assign_directions(&tree, self.params.golden_angle, self.params.root_spread_half_angle, self.params.child_cone_half_angle);

        // Wave G3 item 2 fix: `depth_for_radius` is whichever
        // `DepthMetric` the caller selected — the TREE (`tree.children`/
        // `tree.roots`) above always stays this module's own BFS
        // spanning tree regardless, since the fan-direction algorithm
        // needs a genuine single-parent tree (see this module's own doc
        // comment).
        let depth_for_radius: Vec<u32> = match self.params.depth_metric {
            DepthMetric::ShortestPath => tree.depth.clone(),
            DepthMetric::LongestPath => compute_layering(topo, &self.params.roots).layer,
        };
        let root_count = depth_for_radius.iter().filter(|&&d| d == 0).count();
        let root_ring = root_ring_radius(root_count, self.params.shell_spacing, self.params.min_root_ring_radius);

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
            let depth = depth_for_radius.get(i).copied().unwrap_or(0);
            let base_radius = if depth == 0 { root_ring } else { depth as f32 * self.params.shell_spacing };
            // Wave G3 item 3: radius-aware push-out — see this module's
            // own doc comment for why this is a RADIAL adjustment, not
            // an angular one, unlike the other three one-shot layouts.
            let radius = if self.params.radius_aware_spacing {
                base_radius + topo.radii.get(i).copied().unwrap_or(1.0).max(0.0)
            } else {
                base_radius
            };
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

    /// Wave G3 item 1 fix: renamed from `root_sits_exactly_at_the_origin`
    /// and rewritten — a single root NO LONGER pins to the literal
    /// origin. It now sits on the same shared, nonzero-radius ring
    /// [`super::hierarchical_3d::HierarchicalLayout3D`]'s own layer-0
    /// uses (`(shell_spacing * count / TAU).max(min_root_ring_radius)`,
    /// `count == 1` here) — this is a genuine, reported output change
    /// (the whole point of the fix: the OLD `radius == 0` pin is exactly
    /// what collapsed every root in a multi-root graph onto one point,
    /// see `multiple_disconnected_roots_land_on_a_shared_nonzero_ring_at_distinct_positions`
    /// below), not a loosened assertion.
    #[test]
    fn single_root_sits_at_a_small_nonzero_ring_radius_not_pinned_to_the_origin() {
        let edges = [e(0, 1)];
        let t = topo(2, &edges);
        let mut particles = vec![Particle::default(); 2];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let params = RadialParams3D::default();
        let expected = (params.shell_spacing / TAU).max(params.min_root_ring_radius);
        assert!(expected > 0.0, "sanity: the expected ring radius itself must be nonzero");
        assert!(
            (radius(&particles[0]) - expected).abs() < 1e-3,
            "a single root must land on the min_root_ring_radius-floored ring, got r={} expected={expected}",
            radius(&particles[0])
        );
    }

    /// Wave G3 item 1 gate: the actual defect this item fixes — four
    /// disconnected single-node components (every node its own
    /// zero-in-degree root) must land at four DISTINCT points on a
    /// shared ring, not collapse onto the literal origin.
    #[test]
    fn multiple_disconnected_roots_land_on_a_shared_nonzero_ring_at_distinct_positions() {
        let t = topo(4, &[]);
        let mut particles = vec![Particle::default(); 4];
        let mut layout = RadialLayout3D::default();
        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let radii: Vec<f32> = particles.iter().map(radius).collect();
        for r in &radii {
            assert!(*r > 0.0, "every root must sit on a nonzero-radius ring, not the literal origin: {radii:?}");
        }
        for r in &radii[1..] {
            assert!((r - radii[0]).abs() < 1e-3, "every depth-0 root must share the same ring radius: {radii:?}");
        }
        let mut positions: Vec<(f32, f32, f32)> = particles.iter().map(|p| (p.x, p.y, p.z)).collect();
        positions.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in positions.windows(2) {
            assert_ne!(pair[0], pair[1], "distinct roots must not collapse onto the same point");
        }
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
        // Wave G3 item 1 fix: the fallback root (node 0) no longer
        // pins to the literal origin — it sits on the nonzero root
        // ring, same as every other single-root case (see
        // `single_root_sits_at_a_small_nonzero_ring_radius_not_pinned_to_the_origin`).
        let radii: Vec<f32> = particles.iter().map(radius).collect();
        assert!(radii[0] > 0.0, "the fallback root must sit on the nonzero root ring, not the origin: {radii:?}");
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

    /// Wave G2b configurability gate: [`RadialParams3D::default`] must
    /// equal the prior hardcoded constants exactly. Extended in Wave G3
    /// for the three new fields — each must default to this module's
    /// own PRE-EXISTING behavior (doctrine: no silent output change for
    /// items 2/3; item 1's `min_root_ring_radius` is a genuinely new
    /// field with no prior constant to match, so it's checked against
    /// its own documented default instead).
    #[test]
    fn radial_params_3d_default_matches_the_prior_hardcoded_constants() {
        let p = RadialParams3D::default();
        assert_eq!(p.golden_angle, 2.399_963_2);
        assert!((p.root_spread_half_angle - 1.047_198).abs() < 1e-6);
        assert!((p.child_cone_half_angle - 0.698_132).abs() < 1e-6);
        assert_eq!(p.min_root_ring_radius, 20.0);
        assert_eq!(p.depth_metric, DepthMetric::ShortestPath, "RadialLayout3D's pre-existing behavior — must not change silently");
        assert!(!p.radius_aware_spacing, "must default OFF — doctrine: no silent output change");
    }

    // ── Wave G3 item 2 — `depth_metric` ─────────────────────────────────

    /// Selecting `LongestPath` must genuinely swap which depth NUMBER
    /// feeds the shell radius — proven on the same multi-parent fixture
    /// `layering.rs`'s own test uses (node D, index 5, gets Kahn
    /// longest-path layer 3 vs. this module's own BFS shortest-path
    /// depth 2).
    #[test]
    fn depth_metric_longest_path_places_a_multi_parent_node_on_a_different_shell_than_the_default_shortest_path() {
        let edges = [e(0, 1), e(2, 3), e(3, 4), e(1, 5), e(4, 5)];
        let t = topo(6, &edges);

        let mut shortest_particles = vec![Particle::default(); 6];
        let mut shortest_layout = RadialLayout3D::default(); // ShortestPath default
        shortest_layout.tick(&t, &mut shortest_particles, 1.0 / 60.0);

        let mut longest_particles = vec![Particle::default(); 6];
        let mut longest_layout = RadialLayout3D::new(RadialParams3D { depth_metric: DepthMetric::LongestPath, ..RadialParams3D::default() });
        longest_layout.tick(&t, &mut longest_particles, 1.0 / 60.0);

        let r_shortest = radius(&shortest_particles[5]); // D
        let r_longest = radius(&longest_particles[5]);
        assert!(
            r_longest > r_shortest,
            "D's LongestPath layer (3) must sit on a strictly larger shell than its ShortestPath depth (2): {r_longest} vs {r_shortest}"
        );

        let default_params = RadialParams3D::default();
        let expected_shortest = 2.0 * default_params.shell_spacing;
        let expected_longest = 3.0 * default_params.shell_spacing;
        assert!((r_shortest - expected_shortest).abs() < 1e-2, "got {r_shortest}, expected {expected_shortest}");
        assert!((r_longest - expected_longest).abs() < 1e-2, "got {r_longest}, expected {expected_longest}");
    }

    // ── Wave G3 item 3 — `radius_aware_spacing` ─────────────────────────

    /// `radius_aware_spacing` must actually push a large-radius node
    /// further out along its own fan direction — proven via a direct
    /// radius comparison against the same graph with uniform radii.
    #[test]
    fn radius_aware_spacing_pushes_a_large_radius_node_further_out_along_its_own_direction() {
        let edges = [e(0, 1), e(1, 2)];
        let mut radii = vec![1.0f32; 3];
        radii[2] = 40.0; // node 2 (depth 2) is a big supernode-like member
        let t = SimTopology { node_count: 3, edges: &edges, degree: &[], radii };

        let mut off_particles = vec![Particle::default(); 3];
        let mut off_layout = RadialLayout3D::default();
        off_layout.tick(&t, &mut off_particles, 1.0 / 60.0);

        let mut on_particles = vec![Particle::default(); 3];
        let mut on_layout = RadialLayout3D::new(RadialParams3D { radius_aware_spacing: true, ..RadialParams3D::default() });
        on_layout.tick(&t, &mut on_particles, 1.0 / 60.0);

        let r_off = radius(&off_particles[2]);
        let r_on = radius(&on_particles[2]);
        assert!((r_on - r_off - 40.0).abs() < 1e-2, "the push-out must add exactly the node's own radius: off={r_off} on={r_on}");

        // A node with the default radius (1.0) must be essentially
        // unaffected relative to the big node's own large shift.
        let r_off_small = radius(&off_particles[1]);
        let r_on_small = radius(&on_particles[1]);
        assert!(
            (r_on_small - r_off_small - 1.0).abs() < 1e-2,
            "a node at the default topology radius (1.0) must shift by its own small radius only: off={r_off_small} on={r_on_small}"
        );
    }

    /// Wave G2b configurability gate: a caller-tuned `child_cone_half_angle`
    /// must actually change sibling spread, not just exist as an unread
    /// field — a wider cone must place two same-depth siblings FARTHER
    /// apart (larger chord distance) than the narrower default cone does.
    #[test]
    fn a_wider_child_cone_half_angle_spreads_same_depth_siblings_farther_apart() {
        let edges = [e(0, 1), e(0, 2)];
        let t = topo(3, &edges);

        let mut default_particles = vec![Particle::default(); 3];
        let mut default_layout = RadialLayout3D::default();
        default_layout.tick(&t, &mut default_particles, 1.0 / 60.0);
        let default_gap = {
            let dx = default_particles[1].x - default_particles[2].x;
            let dy = default_particles[1].y - default_particles[2].y;
            let dz = default_particles[1].z - default_particles[2].z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        let mut wide_particles = vec![Particle::default(); 3];
        let mut wide_layout =
            RadialLayout3D::new(RadialParams3D { child_cone_half_angle: std::f32::consts::FRAC_PI_2, ..RadialParams3D::default() });
        wide_layout.tick(&t, &mut wide_particles, 1.0 / 60.0);
        let wide_gap = {
            let dx = wide_particles[1].x - wide_particles[2].x;
            let dy = wide_particles[1].y - wide_particles[2].y;
            let dz = wide_particles[1].z - wide_particles[2].z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        };

        assert!(
            wide_gap > default_gap,
            "a wider child_cone_half_angle must spread same-depth siblings farther apart: default={default_gap} wide={wide_gap}"
        );
    }
}
