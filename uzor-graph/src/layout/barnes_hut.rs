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
/// coincident/near-coincident particles.
const MIN_DIST2: f32 = 1.0;

/// O(n²) reference implementation. Accumulates repulsion force into
/// `out[i]` for every particle `i` (does not clear `out` first — caller
/// combines with other forces in the same buffer).
pub fn apply_repulsion_brute_force(particles: &[Particle], strength: f32, out: &mut [(f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        let (xi, yi) = (particles[i].x, particles[i].y);
        for j in (i + 1)..n {
            let dx = xi - particles[j].x;
            let dy = yi - particles[j].y;
            let d2 = (dx * dx + dy * dy).max(MIN_DIST2);
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
}

enum NodeContent {
    Empty,
    Leaf { x: f32, y: f32, mass: f32 },
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

    fn insert(&mut self, x: f32, y: f32, mass: f32) {
        let new_mass = self.mass + mass;
        self.com_x = (self.com_x * self.mass + x * mass) / new_mass;
        self.com_y = (self.com_y * self.mass + y * mass) / new_mass;
        self.mass = new_mass;

        match &mut self.content {
            NodeContent::Empty => {
                self.content = NodeContent::Leaf { x, y, mass };
            }
            NodeContent::Leaf { x: lx, y: ly, mass: lmass } => {
                let (lx, ly, lmass) = (*lx, *ly, *lmass);
                let mut children = [
                    QuadNode::new_empty(self.bounds.child_bounds(0)),
                    QuadNode::new_empty(self.bounds.child_bounds(1)),
                    QuadNode::new_empty(self.bounds.child_bounds(2)),
                    QuadNode::new_empty(self.bounds.child_bounds(3)),
                ];
                children[self.bounds.quadrant(lx, ly)].insert(lx, ly, lmass);
                children[self.bounds.quadrant(x, y)].insert(x, y, mass);
                self.content = NodeContent::Internal { children: Box::new(children) };
            }
            NodeContent::Internal { children } => {
                children[self.bounds.quadrant(x, y)].insert(x, y, mass);
            }
        }
    }

    fn accumulate(&self, x: f32, y: f32, theta: f32, strength: f32, out: &mut (f32, f32)) {
        match &self.content {
            NodeContent::Empty => {}
            NodeContent::Leaf { x: lx, y: ly, mass } => {
                apply_point(x, y, *lx, *ly, *mass, strength, out);
            }
            NodeContent::Internal { children } => {
                let dx = x - self.com_x;
                let dy = y - self.com_y;
                let d = (dx * dx + dy * dy).sqrt().max(0.001);
                if self.bounds.size / d < theta {
                    apply_point(x, y, self.com_x, self.com_y, self.mass, strength, out);
                } else {
                    for child in children.iter() {
                        child.accumulate(x, y, theta, strength, out);
                    }
                }
            }
        }
    }
}

fn apply_point(x: f32, y: f32, px: f32, py: f32, mass: f32, strength: f32, out: &mut (f32, f32)) {
    let dx = x - px;
    let dy = y - py;
    let d2 = (dx * dx + dy * dy).max(MIN_DIST2);
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
    pub fn build(particles: &[Particle]) -> Self {
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
        for p in particles {
            root.insert(p.x, p.y, 1.0);
        }
        Self { root: Some(root) }
    }

    /// Accumulate the approximated repulsion force for every particle
    /// into `out[i]` (added to, not overwritten).
    pub fn accumulate_forces(&self, particles: &[Particle], theta: f32, strength: f32, out: &mut [(f32, f32)]) {
        let Some(root) = &self.root else { return };
        for (i, p) in particles.iter().enumerate() {
            let mut acc = (0.0, 0.0);
            root.accumulate(p.x, p.y, theta, strength, &mut acc);
            out[i].0 += acc.0;
            out[i].1 += acc.1;
        }
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
        apply_repulsion_brute_force(&particles, strength, &mut brute);

        let qt = Quadtree::build(&particles);
        let mut approx = vec![(0f32, 0f32); particles.len()];
        qt.accumulate_forces(&particles, 0.6, strength, &mut approx);

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

    #[test]
    fn empty_quadtree_produces_no_force() {
        let particles: Vec<Particle> = Vec::new();
        let qt = Quadtree::build(&particles);
        let mut out: Vec<(f32, f32)> = Vec::new();
        qt.accumulate_forces(&particles, DEFAULT_THETA, 100.0, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn single_particle_produces_no_self_force() {
        let particles = vec![Particle::at(3.0, 4.0)];
        let qt = Quadtree::build(&particles);
        let mut out = vec![(0f32, 0f32)];
        qt.accumulate_forces(&particles, DEFAULT_THETA, 100.0, &mut out);
        assert_eq!(out[0], (0.0, 0.0));
    }
}
