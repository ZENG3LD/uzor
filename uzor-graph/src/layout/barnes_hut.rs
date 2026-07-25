//! Many-body (repulsion) force — brute-force O(n²) reference plus a
//! Barnes-Hut quadtree approximation for the O(n log n) path.
//!
//! Adaptive activation lives in [`super::force_directed::ForceDirectedLayout`]:
//! brute-force under [`BRUTE_FORCE_THRESHOLD`] particles (quadtree-build
//! overhead isn't worth it below that), Barnes-Hut above, θ = [`DEFAULT_THETA`]
//! by default — matches the benchmarking cited in the engine design doc §1.2/§3.3.

use crate::particle::Particle;

/// Barnes & Hut's own recommended θ (multipole acceptance criterion).
pub const DEFAULT_THETA: f32 = 1.0;

/// Below this many active particles, brute-force repulsion is cheaper
/// than building a quadtree.
pub const BRUTE_FORCE_THRESHOLD: usize = 500;

/// Softening term — avoids a divide-by-zero singularity for
/// coincident/near-coincident particles. Graph-strengthening arc Wave
/// G2b: promoted `pub` (was private) so [`super::force_directed::ForceParams::min_dist2`]
/// can default to it — every call site that used to read this constant
/// directly now takes it as an explicit parameter instead (see
/// [`apply_repulsion_brute_force`]/[`QuadNode::accumulate`]/[`apply_point`]/
/// [`Quadtree::accumulate_forces`]).
pub const MIN_DIST2: f32 = 1.0;

/// Two points closer (squared) than this can't be meaningfully separated
/// by subdividing — merged into one heavier leaf on insert. Cluster
/// collapse pins whole member stacks onto one exact centroid, so the
/// coincident case is routine, and without this guard `QuadNode::insert`
/// recurses forever (stack overflow — live-caught via the second
/// `collapse` action in force-graph-demo). Graph-strengthening arc Wave
/// G2b: promoted `pub`, see [`MIN_DIST2`]'s own doc comment for why.
pub const MIN_SPLIT_DIST2: f32 = 1e-8;

/// Subdivision floor: a cell this small is never split further, its
/// second point merges into the existing leaf. Backstop for
/// near-coincident (but not equal) floats that would take hundreds of
/// halvings to separate. Graph-strengthening arc Wave G2b: promoted
/// `pub`, see [`MIN_DIST2`]'s own doc comment for why.
pub const MIN_CELL_SIZE: f32 = 1e-3;

/// Floor on the distance-to-center-of-mass used by [`QuadNode::accumulate`]'s
/// θ (multipole acceptance) ratio test — a DIFFERENT floor from
/// [`MIN_DIST2`], not a duplicate of it, even though both guard "a
/// distance used as a divisor" (Wave G1 fix — the two were previously an
/// unnamed inline `0.001` sitting next to the named `MIN_DIST2`, exactly
/// the kind of drift that manufactures a twin disagreement if either is
/// ever tuned without noticing the other). [`MIN_DIST2`] softens the
/// PHYSICAL force magnitude at a 1.0-world-unit floor — large enough to
/// materially cap repulsion between near-coincident particles.
/// [`CELL_ACCEPTANCE_MIN_DIST`] only prevents an exact `0/0` when a query
/// point sits precisely at a cell's own center of mass — it stays three
/// orders of magnitude smaller than `MIN_DIST2.sqrt()` (`1.0`) on
/// purpose: raising it to `MIN_DIST2`'s scale would make the θ test
/// accept coarser (less accurate) cell approximations for any query point
/// within 1 world unit of a cell's center of mass, silently degrading
/// Barnes-Hut accuracy in exactly the regime (near-coincident query vs.
/// cell) where the approximation is already weakest. Unifying the two
/// would be a physics-output change (doctrine: never change rendered
/// output silently) — so they stay distinct, now both named and
/// cross-documented instead of one being an anonymous magic number.
const CELL_ACCEPTANCE_MIN_DIST: f32 = 0.001;

/// O(n²) reference implementation. Accumulates repulsion force into
/// `out[i]` for every particle `i` (does not clear `out` first — caller
/// combines with other forces in the same buffer). `min_dist2` is the
/// softening floor — was the private [`MIN_DIST2`] constant, now a
/// caller-supplied parameter (graph-strengthening arc Wave G2b).
pub fn apply_repulsion_brute_force(particles: &[Particle], strength: f32, min_dist2: f32, out: &mut [(f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        let (xi, yi) = (particles[i].x, particles[i].y);
        for j in (i + 1)..n {
            let dx = xi - particles[j].x;
            let dy = yi - particles[j].y;
            let d2 = (dx * dx + dy * dy).max(min_dist2);
            let d = d2.sqrt();
            let f = strength / d2;
            let fx = dx / d * f;
            let fy = dy / d * f;
            out[i].0 += fx;
            out[i].1 += fy;
            out[j].0 -= fx;
            out[j].1 -= fy;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct QuadBounds {
    min_x: f32,
    min_y: f32,
    size: f32,
}

impl QuadBounds {
    fn quadrant(&self, x: f32, y: f32) -> usize {
        let mid_x = self.min_x + self.size * 0.5;
        let mid_y = self.min_y + self.size * 0.5;
        match (x >= mid_x, y >= mid_y) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        }
    }

    fn child_bounds(&self, quadrant: usize) -> QuadBounds {
        let half = self.size * 0.5;
        let (ox, oy) = match quadrant {
            0 => (0.0, 0.0),
            1 => (half, 0.0),
            2 => (0.0, half),
            _ => (half, half),
        };
        QuadBounds { min_x: self.min_x + ox, min_y: self.min_y + oy, size: half }
    }

    /// Whether this cell's AABB comes within `r` of `(cx, cy)` — the
    /// standard clamp-to-box AABB/circle intersection test. Used by
    /// [`QuadNode::collect_within`] (Wave G1 fix) to prune subtrees a
    /// collision range-query can't possibly reach, the same pruning
    /// principle θ already uses for repulsion.
    fn intersects_circle(&self, cx: f32, cy: f32, r: f32) -> bool {
        let max_x = self.min_x + self.size;
        let max_y = self.min_y + self.size;
        let nearest_x = cx.clamp(self.min_x, max_x);
        let nearest_y = cy.clamp(self.min_y, max_y);
        let dx = cx - nearest_x;
        let dy = cy - nearest_y;
        dx * dx + dy * dy <= r * r
    }
}

enum NodeContent {
    Empty,
    /// `indices` (Wave G1 fix) — every particle index ever merged into
    /// this leaf, in insertion order. Additive to the pre-existing
    /// `x`/`y`/`mass` (still the ONLY fields [`QuadNode::accumulate`]'s
    /// repulsion pass reads, byte-identical to before this field
    /// existed) — needed because a merge collapses several particles
    /// onto one approximate point-mass for the repulsion force, but
    /// [`QuadNode::collect_within`]'s collision query still needs each
    /// individual particle's OWN identity (and, via that index, its real
    /// position from the caller's own particle slice) to resolve
    /// collisions between them.
    Leaf { x: f32, y: f32, mass: f32, indices: Vec<u32> },
    Internal { children: Box<[QuadNode; 4]> },
}

struct QuadNode {
    bounds: QuadBounds,
    mass: f32,
    com_x: f32,
    com_y: f32,
    content: NodeContent,
}

impl QuadNode {
    fn new_empty(bounds: QuadBounds) -> Self {
        Self { bounds, mass: 0.0, com_x: 0.0, com_y: 0.0, content: NodeContent::Empty }
    }

    fn insert(&mut self, x: f32, y: f32, mass: f32, index: u32, min_split_dist2: f32, min_cell_size: f32) {
        let new_mass = self.mass + mass;
        self.com_x = (self.com_x * self.mass + x * mass) / new_mass;
        self.com_y = (self.com_y * self.mass + y * mass) / new_mass;
        self.mass = new_mass;

        match &mut self.content {
            NodeContent::Empty => {
                self.content = NodeContent::Leaf { x, y, mass, indices: vec![index] };
            }
            NodeContent::Leaf { x: lx, y: ly, mass: lmass, indices } => {
                // Coincident (or indistinguishably close) points can never
                // be separated by subdividing — cluster collapse pins whole
                // member stacks onto one centroid, so this is a normal
                // state, not a degenerate one. Merge into a single heavier
                // leaf (exactly the "one point mass" equivalence
                // cluster.rs relies on) instead of recursing forever.
                let (dx, dy) = (x - *lx, y - *ly);
                if dx * dx + dy * dy <= min_split_dist2 || self.bounds.size <= min_cell_size {
                    *lmass += mass;
                    indices.push(index);
                    return;
                }
                let (lx, ly) = (*lx, *ly);
                // Every previously-merged index is re-homed into whichever
                // child `(lx, ly)` — the merged leaf's own approximate
                // point — falls into. Reinserted one at a time at mass
                // `1.0` each (every caller of `Quadtree::build` inserts at
                // a uniform mass of `1.0` — see that method) rather than as
                // one aggregate `insert(lx, ly, lmass)` call, so each
                // individual particle keeps its own index; the resulting
                // `mass`/center-of-mass is numerically IDENTICAL either way
                // (inserting N copies of the same point sequentially
                // accumulates to the same total as one N-mass insert).
                let prior_indices = std::mem::take(indices);
                let mut children = [
                    QuadNode::new_empty(self.bounds.child_bounds(0)),
                    QuadNode::new_empty(self.bounds.child_bounds(1)),
                    QuadNode::new_empty(self.bounds.child_bounds(2)),
                    QuadNode::new_empty(self.bounds.child_bounds(3)),
                ];
                let prior_quadrant = self.bounds.quadrant(lx, ly);
                for prior_index in prior_indices {
                    children[prior_quadrant].insert(lx, ly, 1.0, prior_index, min_split_dist2, min_cell_size);
                }
                children[self.bounds.quadrant(x, y)].insert(x, y, mass, index, min_split_dist2, min_cell_size);
                self.content = NodeContent::Internal { children: Box::new(children) };
            }
            NodeContent::Internal { children } => {
                children[self.bounds.quadrant(x, y)].insert(x, y, mass, index, min_split_dist2, min_cell_size);
            }
        }
    }

    fn accumulate(&self, x: f32, y: f32, theta: f32, strength: f32, min_dist2: f32, out: &mut (f32, f32)) {
        match &self.content {
            NodeContent::Empty => {}
            NodeContent::Leaf { x: lx, y: ly, mass, .. } => {
                apply_point(x, y, *lx, *ly, *mass, strength, min_dist2, out);
            }
            NodeContent::Internal { children } => {
                let dx = x - self.com_x;
                let dy = y - self.com_y;
                let d = (dx * dx + dy * dy).sqrt().max(CELL_ACCEPTANCE_MIN_DIST);
                if self.bounds.size / d < theta {
                    apply_point(x, y, self.com_x, self.com_y, self.mass, strength, min_dist2, out);
                } else {
                    for child in children.iter() {
                        child.accumulate(x, y, theta, strength, min_dist2, out);
                    }
                }
            }
        }
    }

    /// Spatial range query (Wave G1 fix) — appends every particle index
    /// whose leaf comes within `r` of `(qx, qy)` to `out`. Conservative by
    /// construction: a subtree is only skipped when its WHOLE bounding
    /// cell is farther than `r` away everywhere, so this can over-report
    /// (return an index whose exact position turns out to be farther than
    /// `r`) but never under-report — callers (collision resolution) still
    /// run an exact distance check on every returned candidate.
    fn collect_within(&self, qx: f32, qy: f32, r: f32, out: &mut Vec<u32>) {
        match &self.content {
            NodeContent::Empty => {}
            NodeContent::Leaf { indices, .. } => out.extend_from_slice(indices),
            NodeContent::Internal { children } => {
                for child in children.iter() {
                    if child.bounds.intersects_circle(qx, qy, r) {
                        child.collect_within(qx, qy, r, out);
                    }
                }
            }
        }
    }
}

fn apply_point(x: f32, y: f32, px: f32, py: f32, mass: f32, strength: f32, min_dist2: f32, out: &mut (f32, f32)) {
    let dx = x - px;
    let dy = y - py;
    let d2 = (dx * dx + dy * dy).max(min_dist2);
    let d = d2.sqrt();
    let f = strength * mass / d2;
    out.0 += dx / d * f;
    out.1 += dy / d * f;
}

/// A quadtree built once per tick over the current particle set, reused
/// for the many-body force approximation.
pub struct Quadtree {
    root: Option<QuadNode>,
}

impl Quadtree {
    /// `min_split_dist2`/`min_cell_size` were the private [`MIN_SPLIT_DIST2`]/
    /// [`MIN_CELL_SIZE`] constants — now caller-supplied parameters (graph-
    /// strengthening arc Wave G2b).
    pub fn build(particles: &[Particle], min_split_dist2: f32, min_cell_size: f32) -> Self {
        if particles.is_empty() {
            return Self { root: None };
        }
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in particles {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        let size = (max_x - min_x).max(max_y - min_y).max(1.0) * 1.01;
        let bounds = QuadBounds { min_x, min_y, size };
        let mut root = QuadNode::new_empty(bounds);
        for (i, p) in particles.iter().enumerate() {
            root.insert(p.x, p.y, 1.0, i as u32, min_split_dist2, min_cell_size);
        }
        Self { root: Some(root) }
    }

    /// Accumulate the approximated repulsion force for every particle
    /// into `out[i]` (added to, not overwritten). `min_dist2` was the
    /// private [`MIN_DIST2`] constant — now a caller-supplied parameter
    /// (graph-strengthening arc Wave G2b).
    pub fn accumulate_forces(&self, particles: &[Particle], theta: f32, strength: f32, min_dist2: f32, out: &mut [(f32, f32)]) {
        let Some(root) = &self.root else { return };
        for (i, p) in particles.iter().enumerate() {
            let mut acc = (0.0, 0.0);
            root.accumulate(p.x, p.y, theta, strength, min_dist2, &mut acc);
            out[i].0 += acc.0;
            out[i].1 += acc.1;
        }
    }

    /// Tree-accelerated collision resolution (Wave G1 fix) — the
    /// spatial-partitioned sibling of
    /// [`super::force_directed::apply_collision`]'s O(n²) brute-force
    /// loop, used above `brute_force_threshold` so collision never has to
    /// be disabled for a large particle count. Reuses THIS tree — already
    /// built for repulsion this same tick — rather than constructing a
    /// second spatial index. Proven (not just claimed) to produce the
    /// SAME resolution as the brute-force path by
    /// `force_directed::tests::tree_collision_resolution_matches_brute_force_on_a_fixture_with_deliberate_overlaps`.
    pub fn apply_collision(&self, particles: &[Particle], radii: &[f32], strength: f32, out: &mut [(f32, f32)]) {
        let Some(root) = &self.root else { return };
        let n = particles.len();
        if n < 2 {
            return;
        }
        // Any candidate whose collision disc could possibly reach particle
        // `i` lies within `radii[i] + max_radius` of it (the largest ANY
        // other particle's own radius could be) — a fixed, safe
        // over-approximation of the true per-pair reach, cheap to compute
        // once per tick. Folding from `1.0` (not `0.0`) keeps this in sync
        // with the `radii.get(i).unwrap_or(1.0)` fallback used everywhere
        // else in this function, in case `radii` is shorter than
        // `particles` for some caller.
        let max_radius = radii.iter().copied().fold(1.0f32, f32::max);
        let mut candidates: Vec<u32> = Vec::new();
        for i in 0..n {
            let query_radius = radii.get(i).copied().unwrap_or(1.0) + max_radius;
            candidates.clear();
            root.collect_within(particles[i].x, particles[i].y, query_radius, &mut candidates);
            for &j_u32 in &candidates {
                let j = j_u32 as usize;
                // Each unordered pair is resolved exactly once, from the
                // lower-indexed particle's own query — matching
                // `apply_collision`'s `for j in (i+1)..n` pairing exactly
                // (every genuine collision pair is symmetric: if `j` is
                // reachable from `i`'s query radius, `i` is equally
                // reachable from `j`'s, since both radii are bounded by
                // the same `max_radius`).
                if j <= i {
                    continue;
                }
                collision_pair_force(i, j, particles, radii, strength, out);
            }
        }
    }
}

/// Exact pairwise collision resolution for one particle pair — shared by
/// the brute-force O(n²) path
/// ([`super::force_directed::apply_collision`]) and the tree-accelerated
/// path ([`Quadtree::apply_collision`]) so the two can never numerically
/// drift apart (Wave G1 fix).
pub(crate) fn collision_pair_force(i: usize, j: usize, particles: &[Particle], radii: &[f32], strength: f32, out: &mut [(f32, f32)]) {
    let dx = particles[j].x - particles[i].x;
    let dy = particles[j].y - particles[i].y;
    let dist2 = dx * dx + dy * dy;
    let min_dist = radii.get(i).copied().unwrap_or(1.0) + radii.get(j).copied().unwrap_or(1.0);
    if dist2 <= 1e-6 {
        // Coincident positions — deterministic nudge so they don't stay
        // locked together forever.
        out[i].0 -= 0.5;
        out[j].0 += 0.5;
        return;
    }
    if dist2 < min_dist * min_dist {
        let dist = dist2.sqrt();
        let overlap = (min_dist - dist) * strength;
        let nx = dx / dist;
        let ny = dy / dist;
        out[i].0 -= nx * overlap * 0.5;
        out[i].1 -= ny * overlap * 0.5;
        out[j].0 += nx * overlap * 0.5;
        out[j].1 += ny * overlap * 0.5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random particle scatter — no time/OS
    /// randomness, seeded purely by index (splitmix64-style LCG).
    fn deterministic_particles(n: usize) -> Vec<Particle> {
        let mut particles = Vec::with_capacity(n);
        let mut state: u64 = 0x9E3779B97F4A7C15;
        for _ in 0..n {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rx = ((state >> 33) as u32 % 2000) as f32 - 1000.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let ry = ((state >> 33) as u32 % 2000) as f32 - 1000.0;
            particles.push(Particle::at(rx, ry));
        }
        particles
    }

    #[test]
    fn barnes_hut_matches_brute_force_within_tolerance() {
        let particles = deterministic_particles(96);
        let strength = 400.0;

        let mut brute = vec![(0f32, 0f32); particles.len()];
        apply_repulsion_brute_force(&particles, strength, MIN_DIST2, &mut brute);

        let qt = Quadtree::build(&particles, MIN_SPLIT_DIST2, MIN_CELL_SIZE);
        let mut approx = vec![(0f32, 0f32); particles.len()];
        qt.accumulate_forces(&particles, 0.6, strength, MIN_DIST2, &mut approx);

        let mut max_rel_err = 0f32;
        for (b, a) in brute.iter().zip(approx.iter()) {
            let bmag = (b.0 * b.0 + b.1 * b.1).sqrt();
            let diff = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            if bmag > 1e-3 {
                max_rel_err = max_rel_err.max(diff / bmag);
            }
        }
        assert!(max_rel_err < 0.35, "Barnes-Hut relative error too high: {max_rel_err}");
    }

    /// Layout audit A7 coverage gap, closed: the crate SHIPS
    /// `DEFAULT_THETA = 1.0` (wired as the literal default in
    /// `ForceParams::default()`/`ForceParams3D::default()`) — a
    /// materially COARSER (less accurate, cheaper) value than the
    /// `0.6` this file's own sibling test above exercises, and the
    /// live approximation error at the value that actually SHIPS was,
    /// until this test, completely unverified. Same fixture/strength as
    /// the sibling test, same measure (relative error against the exact
    /// brute-force answer), only `theta` differs.
    #[test]
    fn barnes_hut_matches_brute_force_within_tolerance_at_the_shipped_theta() {
        let particles = deterministic_particles(96);
        let strength = 400.0;

        let mut brute = vec![(0f32, 0f32); particles.len()];
        apply_repulsion_brute_force(&particles, strength, MIN_DIST2, &mut brute);

        let qt = Quadtree::build(&particles, MIN_SPLIT_DIST2, MIN_CELL_SIZE);
        let mut approx = vec![(0f32, 0f32); particles.len()];
        qt.accumulate_forces(&particles, DEFAULT_THETA, strength, MIN_DIST2, &mut approx);

        let mut max_rel_err = 0f32;
        for (b, a) in brute.iter().zip(approx.iter()) {
            let bmag = (b.0 * b.0 + b.1 * b.1).sqrt();
            let diff = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
            if bmag > 1e-3 {
                max_rel_err = max_rel_err.max(diff / bmag);
            }
        }
        // MEASURED on this exact fixture (2026-07-26, reported in full to
        // the owner as part of the graph-strengthening arc Wave G3
        // report — this bound is NOT tightened/loosened to make the test
        // pass, and the shipped default is NOT changed here regardless
        // of what this number says): the shipped theta=1.0 max relative
        // error on this same 96-particle fixture measures ~1.20 (120%) —
        // materially worse than the theta=0.6 sibling test's own 35%
        // bound. `1.5` gives this regression test real headroom above
        // the measured value while still catching a genuine further
        // regression.
        assert!(max_rel_err < 1.5, "Barnes-Hut relative error at the SHIPPED theta=1.0 too high: {max_rel_err}");
    }

    #[test]
    fn empty_quadtree_produces_no_force() {
        let particles: Vec<Particle> = Vec::new();
        let qt = Quadtree::build(&particles, MIN_SPLIT_DIST2, MIN_CELL_SIZE);
        let mut out: Vec<(f32, f32)> = Vec::new();
        qt.accumulate_forces(&particles, DEFAULT_THETA, 100.0, MIN_DIST2, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn single_particle_produces_no_self_force() {
        let particles = vec![Particle::at(3.0, 4.0)];
        let qt = Quadtree::build(&particles, MIN_SPLIT_DIST2, MIN_CELL_SIZE);
        let mut out = vec![(0f32, 0f32)];
        qt.accumulate_forces(&particles, DEFAULT_THETA, 100.0, MIN_DIST2, &mut out);
        assert_eq!(out[0], (0.0, 0.0));
    }

    /// Regression: cluster collapse pins every member onto one exact
    /// centroid, so the quadtree must ingest a stack of coincident
    /// points without subdividing forever (pre-fix this was a
    /// deterministic main-thread stack overflow — second `collapse`
    /// action in force-graph-demo). The merged stack must also act as
    /// ONE heavier point mass, per cluster.rs's documented equivalence.
    #[test]
    fn coincident_particle_stack_builds_and_acts_as_one_point_mass() {
        // Two collapsed-cluster stacks (8 members each, exact same
        // coordinates) plus one free probe particle.
        let mut particles = Vec::new();
        for _ in 0..8 {
            particles.push(Particle::at(-50.0, 0.0));
        }
        for _ in 0..8 {
            particles.push(Particle::at(40.0, 30.0));
        }
        particles.push(Particle::at(0.0, 0.0));

        let strength = 100.0;
        let qt = Quadtree::build(&particles, MIN_SPLIT_DIST2, MIN_CELL_SIZE); // pre-fix: never returns
        let mut out = vec![(0f32, 0f32); particles.len()];
        qt.accumulate_forces(&particles, DEFAULT_THETA, strength, MIN_DIST2, &mut out);

        let probe = out[16];
        assert!(probe.0.is_finite() && probe.1.is_finite());

        // The probe must feel each stack as an 8x-mass single point.
        let mut expected = (0.0f32, 0.0f32);
        apply_point(0.0, 0.0, -50.0, 0.0, 8.0, strength, MIN_DIST2, &mut expected);
        apply_point(0.0, 0.0, 40.0, 30.0, 8.0, strength, MIN_DIST2, &mut expected);
        let diff = ((probe.0 - expected.0).powi(2) + (probe.1 - expected.1).powi(2)).sqrt();
        let mag = (expected.0 * expected.0 + expected.1 * expected.1).sqrt();
        assert!(
            diff <= mag * 0.05,
            "coincident stack should act as one 8-mass point: got {probe:?}, expected {expected:?}"
        );
    }
}
