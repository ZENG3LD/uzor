//! 3D many-body (repulsion) force — brute-force O(n²) reference plus a
//! Barnes-Hut octree approximation for the O(n log n) path.
//!
//! Honest, byte-for-byte-shaped duplication of [`super::barnes_hut`],
//! generalized from a quadtree (4-way `QuadNode`) to an octree (8-way
//! `OctNode`) — per the W3D arc plan §1.1's own decision NOT to build a
//! generic `Tree<const D: usize>` (would force every hot-loop float op
//! through a `[f32; D]` array instead of named `x`/`y`/`z` fields, worse
//! for the optimizer and worse to read; this codebase's own established
//! idiom for this shape of duplication is literal, per `mesh_cache.rs`'s
//! `MeshCache`/`MeshLitCache`/`MeshUvCache`/`MeshPbrCache`).
//!
//! Adaptive activation lives in
//! [`super::force_directed_3d::ForceDirectedLayout3D`]: brute-force under
//! [`BRUTE_FORCE_THRESHOLD`] particles, Barnes-Hut above, θ =
//! [`DEFAULT_THETA`] by default — [`DEFAULT_THETA`]/[`BRUTE_FORCE_THRESHOLD`]
//! are re-exported from [`super::barnes_hut`] rather than re-declared,
//! since those two constants are already `pub` there (the coincident-
//! point merge guard constants below are private in `barnes_hut.rs`, so
//! they're re-declared verbatim instead — see the plan's own note on
//! this).

use crate::particle::Particle;

pub use super::barnes_hut::{BRUTE_FORCE_THRESHOLD, DEFAULT_THETA};

/// Softening term — avoids a divide-by-zero singularity for
/// coincident/near-coincident particles. Verbatim copy of
/// `barnes_hut::MIN_DIST2` (private there, so re-declared here rather
/// than reached via `pub use`).
const MIN_DIST2: f32 = 1.0;

/// Two points closer (squared) than this can't be meaningfully separated
/// by subdividing — merged into one heavier leaf on insert. Verbatim
/// copy of `barnes_hut::MIN_SPLIT_DIST2` — see that constant's doc
/// comment for the coincident-cluster-collapse rationale, which applies
/// identically in 3D.
const MIN_SPLIT_DIST2: f32 = 1e-8;

/// Subdivision floor: a cell this small is never split further, its
/// second point merges into the existing leaf. Verbatim copy of
/// `barnes_hut::MIN_CELL_SIZE`.
const MIN_CELL_SIZE: f32 = 1e-3;

/// O(n²) reference implementation. Accumulates repulsion force into
/// `out[i]` for every particle `i` (does not clear `out` first — caller
/// combines with other forces in the same buffer).
pub fn apply_repulsion_brute_force_3d(particles: &[Particle], strength: f32, out: &mut [(f32, f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        let (xi, yi, zi) = (particles[i].x, particles[i].y, particles[i].z);
        for j in (i + 1)..n {
            let dx = xi - particles[j].x;
            let dy = yi - particles[j].y;
            let dz = zi - particles[j].z;
            let d2 = (dx * dx + dy * dy + dz * dz).max(MIN_DIST2);
            let d = d2.sqrt();
            let f = strength / d2;
            let fx = dx / d * f;
            let fy = dy / d * f;
            let fz = dz / d * f;
            out[i].0 += fx;
            out[i].1 += fy;
            out[i].2 += fz;
            out[j].0 -= fx;
            out[j].1 -= fy;
            out[j].2 -= fz;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct OctBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    size: f32,
}

impl OctBounds {
    fn octant(&self, x: f32, y: f32, z: f32) -> usize {
        let mid_x = self.min_x + self.size * 0.5;
        let mid_y = self.min_y + self.size * 0.5;
        let mid_z = self.min_z + self.size * 0.5;
        match (x >= mid_x, y >= mid_y, z >= mid_z) {
            (false, false, false) => 0,
            (true, false, false) => 1,
            (false, true, false) => 2,
            (true, true, false) => 3,
            (false, false, true) => 4,
            (true, false, true) => 5,
            (false, true, true) => 6,
            (true, true, true) => 7,
        }
    }

    fn child_bounds(&self, octant: usize) -> OctBounds {
        let half = self.size * 0.5;
        let (ox, oy, oz) = match octant {
            0 => (0.0, 0.0, 0.0),
            1 => (half, 0.0, 0.0),
            2 => (0.0, half, 0.0),
            3 => (half, half, 0.0),
            4 => (0.0, 0.0, half),
            5 => (half, 0.0, half),
            6 => (0.0, half, half),
            _ => (half, half, half),
        };
        OctBounds { min_x: self.min_x + ox, min_y: self.min_y + oy, min_z: self.min_z + oz, size: half }
    }
}

enum NodeContent {
    Empty,
    Leaf { x: f32, y: f32, z: f32, mass: f32 },
    Internal { children: Box<[OctNode; 8]> },
}

struct OctNode {
    bounds: OctBounds,
    mass: f32,
    com_x: f32,
    com_y: f32,
    com_z: f32,
    content: NodeContent,
}

impl OctNode {
    fn new_empty(bounds: OctBounds) -> Self {
        Self { bounds, mass: 0.0, com_x: 0.0, com_y: 0.0, com_z: 0.0, content: NodeContent::Empty }
    }

    fn insert(&mut self, x: f32, y: f32, z: f32, mass: f32) {
        let new_mass = self.mass + mass;
        self.com_x = (self.com_x * self.mass + x * mass) / new_mass;
        self.com_y = (self.com_y * self.mass + y * mass) / new_mass;
        self.com_z = (self.com_z * self.mass + z * mass) / new_mass;
        self.mass = new_mass;

        match &mut self.content {
            NodeContent::Empty => {
                self.content = NodeContent::Leaf { x, y, z, mass };
            }
            NodeContent::Leaf { x: lx, y: ly, z: lz, mass: lmass } => {
                // Coincident (or indistinguishably close) points can never
                // be separated by subdividing — cluster collapse pins whole
                // member stacks onto one centroid, so this is a normal
                // state, not a degenerate one (same rationale as
                // `barnes_hut.rs`'s 2D guard). Merge into a single heavier
                // leaf instead of recursing forever.
                let (dx, dy, dz) = (x - *lx, y - *ly, z - *lz);
                if dx * dx + dy * dy + dz * dz <= MIN_SPLIT_DIST2 || self.bounds.size <= MIN_CELL_SIZE {
                    *lmass += mass;
                    return;
                }
                let (lx, ly, lz, lmass) = (*lx, *ly, *lz, *lmass);
                let mut children = [
                    OctNode::new_empty(self.bounds.child_bounds(0)),
                    OctNode::new_empty(self.bounds.child_bounds(1)),
                    OctNode::new_empty(self.bounds.child_bounds(2)),
                    OctNode::new_empty(self.bounds.child_bounds(3)),
                    OctNode::new_empty(self.bounds.child_bounds(4)),
                    OctNode::new_empty(self.bounds.child_bounds(5)),
                    OctNode::new_empty(self.bounds.child_bounds(6)),
                    OctNode::new_empty(self.bounds.child_bounds(7)),
                ];
                children[self.bounds.octant(lx, ly, lz)].insert(lx, ly, lz, lmass);
                children[self.bounds.octant(x, y, z)].insert(x, y, z, mass);
                self.content = NodeContent::Internal { children: Box::new(children) };
            }
            NodeContent::Internal { children } => {
                children[self.bounds.octant(x, y, z)].insert(x, y, z, mass);
            }
        }
    }

    fn accumulate(&self, x: f32, y: f32, z: f32, theta: f32, strength: f32, out: &mut (f32, f32, f32)) {
        match &self.content {
            NodeContent::Empty => {}
            NodeContent::Leaf { x: lx, y: ly, z: lz, mass } => {
                apply_point(x, y, z, *lx, *ly, *lz, *mass, strength, out);
            }
            NodeContent::Internal { children } => {
                let dx = x - self.com_x;
                let dy = y - self.com_y;
                let dz = z - self.com_z;
                let d = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
                if self.bounds.size / d < theta {
                    apply_point(x, y, z, self.com_x, self.com_y, self.com_z, self.mass, strength, out);
                } else {
                    for child in children.iter() {
                        child.accumulate(x, y, z, theta, strength, out);
                    }
                }
            }
        }
    }
}

fn apply_point(x: f32, y: f32, z: f32, px: f32, py: f32, pz: f32, mass: f32, strength: f32, out: &mut (f32, f32, f32)) {
    let dx = x - px;
    let dy = y - py;
    let dz = z - pz;
    let d2 = (dx * dx + dy * dy + dz * dz).max(MIN_DIST2);
    let d = d2.sqrt();
    let f = strength * mass / d2;
    out.0 += dx / d * f;
    out.1 += dy / d * f;
    out.2 += dz / d * f;
}

/// An octree built once per tick over the current particle set, reused
/// for the many-body force approximation.
pub struct Octree {
    root: Option<OctNode>,
}

impl Octree {
    pub fn build(particles: &[Particle]) -> Self {
        if particles.is_empty() {
            return Self { root: None };
        }
        let (mut min_x, mut min_y, mut min_z, mut max_x, mut max_y, mut max_z) =
            (f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN);
        for p in particles {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
        }
        let size = (max_x - min_x).max(max_y - min_y).max(max_z - min_z).max(1.0) * 1.01;
        let bounds = OctBounds { min_x, min_y, min_z, size };
        let mut root = OctNode::new_empty(bounds);
        for p in particles {
            root.insert(p.x, p.y, p.z, 1.0);
        }
        Self { root: Some(root) }
    }

    /// Accumulate the approximated repulsion force for every particle
    /// into `out[i]` (added to, not overwritten).
    pub fn accumulate_forces(&self, particles: &[Particle], theta: f32, strength: f32, out: &mut [(f32, f32, f32)]) {
        let Some(root) = &self.root else { return };
        for (i, p) in particles.iter().enumerate() {
            let mut acc = (0.0, 0.0, 0.0);
            root.accumulate(p.x, p.y, p.z, theta, strength, &mut acc);
            out[i].0 += acc.0;
            out[i].1 += acc.1;
            out[i].2 += acc.2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random 3D particle scatter — no time/OS
    /// randomness, seeded purely by index (splitmix64-style LCG),
    /// extended from `barnes_hut.rs`'s 2D `deterministic_particles` with
    /// a third draw for `z`.
    fn deterministic_particles_3d(n: usize) -> Vec<Particle> {
        let mut particles = Vec::with_capacity(n);
        let mut state: u64 = 0x9E3779B97F4A7C15;
        for _ in 0..n {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rx = ((state >> 33) as u32 % 2000) as f32 - 1000.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let ry = ((state >> 33) as u32 % 2000) as f32 - 1000.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rz = ((state >> 33) as u32 % 2000) as f32 - 1000.0;
            particles.push(Particle::at3(rx, ry, rz));
        }
        particles
    }

    #[test]
    fn barnes_hut_3d_matches_brute_force_within_tolerance() {
        let particles = deterministic_particles_3d(96);
        let strength = 400.0;

        let mut brute = vec![(0f32, 0f32, 0f32); particles.len()];
        apply_repulsion_brute_force_3d(&particles, strength, &mut brute);

        let ot = Octree::build(&particles);
        let mut approx = vec![(0f32, 0f32, 0f32); particles.len()];
        ot.accumulate_forces(&particles, 0.6, strength, &mut approx);

        let mut max_rel_err = 0f32;
        for (b, a) in brute.iter().zip(approx.iter()) {
            let bmag = (b.0 * b.0 + b.1 * b.1 + b.2 * b.2).sqrt();
            let diff = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt();
            if bmag > 1e-3 {
                max_rel_err = max_rel_err.max(diff / bmag);
            }
        }
        assert!(max_rel_err < 0.35, "Barnes-Hut 3D relative error too high: {max_rel_err}");
    }

    #[test]
    fn empty_octree_produces_no_force() {
        let particles: Vec<Particle> = Vec::new();
        let ot = Octree::build(&particles);
        let mut out: Vec<(f32, f32, f32)> = Vec::new();
        ot.accumulate_forces(&particles, DEFAULT_THETA, 100.0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn single_particle_produces_no_self_force() {
        let particles = vec![Particle::at3(3.0, 4.0, 5.0)];
        let ot = Octree::build(&particles);
        let mut out = vec![(0f32, 0f32, 0f32)];
        ot.accumulate_forces(&particles, DEFAULT_THETA, 100.0, &mut out);
        assert_eq!(out[0], (0.0, 0.0, 0.0));
    }

    /// Regression: 3D port of `barnes_hut.rs`'s
    /// `coincident_particle_stack_builds_and_acts_as_one_point_mass` —
    /// cluster collapse pins every member onto one exact 3D centroid, so
    /// the octree must ingest a stack of coincident points without
    /// subdividing forever, and the merged stack must act as ONE heavier
    /// point mass.
    #[test]
    fn coincident_particle_stack_builds_and_acts_as_one_point_mass() {
        // Two collapsed-cluster stacks (8 members each, exact same 3D
        // coordinates) plus one free probe particle. Directions from the
        // probe to each stack are chosen orthogonal (`(1,0,0)` vs.
        // `(0,0.6,0.8)`, a 3-4-5 unit triangle on the y/z plane) so the
        // two repulsion contributions ADD rather than partially cancel —
        // an arbitrary 3D placement can land the net expected force
        // arbitrarily close to zero, which would make the relative-error
        // tolerance below meaningless (dividing by a near-zero
        // magnitude).
        let mut particles = Vec::new();
        for _ in 0..8 {
            particles.push(Particle::at3(-50.0, 0.0, 0.0));
        }
        for _ in 0..8 {
            particles.push(Particle::at3(0.0, -30.0, -40.0));
        }
        particles.push(Particle::at3(0.0, 0.0, 0.0));

        let strength = 100.0;
        let ot = Octree::build(&particles); // pre-fix-equivalent: never returns
        let mut out = vec![(0f32, 0f32, 0f32); particles.len()];
        ot.accumulate_forces(&particles, DEFAULT_THETA, strength, &mut out);

        let probe = out[16];
        assert!(probe.0.is_finite() && probe.1.is_finite() && probe.2.is_finite());

        // The probe must feel each stack as an 8x-mass single point.
        let mut expected = (0.0f32, 0.0f32, 0.0f32);
        apply_point(0.0, 0.0, 0.0, -50.0, 0.0, 0.0, 8.0, strength, &mut expected);
        apply_point(0.0, 0.0, 0.0, 0.0, -30.0, -40.0, 8.0, strength, &mut expected);
        let diff =
            ((probe.0 - expected.0).powi(2) + (probe.1 - expected.1).powi(2) + (probe.2 - expected.2).powi(2)).sqrt();
        let mag = (expected.0 * expected.0 + expected.1 * expected.1 + expected.2 * expected.2).sqrt();
        assert!(
            diff <= mag * 0.05,
            "coincident stack should act as one 8-mass point: got {probe:?}, expected {expected:?}"
        );
    }
}
