//! `ForceDirectedLayout3D` — the 3D sibling of
//! [`super::force_directed::ForceDirectedLayout`] (W3D arc plan §1.1/§4
//! Wave 1). Same d3-force model (many-body repulsion via Barnes-Hut
//! octree, link spring, centering, optional collision), semi-implicit-
//! Euler integration, `alpha` cooling — generalized from `(x, y)` to
//! `(x, y, z)` and implementing the SAME [`super::Layout`] trait 2D
//! layouts do (no new trait needed, per the plan's own reasoning: `z`/
//! `vz`/`fz` already live on the shared [`Particle`] type).

use crate::graph::SimTopology;
use crate::particle::Particle;

use super::barnes_hut_3d::{self, Octree};
use super::{Layout, LayoutTickResult};

/// Tunable 3D force-model parameters — field-for-field parallel to
/// [`super::force_directed::ForceParams`], `center` generalized to a 3D
/// point.
#[derive(Debug, Clone, Copy)]
pub struct ForceParams3D {
    /// Many-body repulsion magnitude (`strength / distance²`).
    pub charge_strength: f32,
    /// Link spring strength.
    pub link_strength: f32,
    /// Link spring ideal length.
    pub link_distance: f32,
    /// Weak uniform pull toward `center`, keeps disconnected components
    /// from drifting to infinity.
    pub center_strength: f32,
    pub center: (f32, f32, f32),
    /// Velocity friction per normalized tick, `0.0..1.0`.
    pub velocity_decay: f32,
    /// Per-normalized-tick alpha decay toward `alpha_target = 0.0`.
    pub alpha_decay: f32,
    /// Below this alpha, the layout reports [`LayoutTickResult::settled`].
    pub alpha_min: f32,
    /// Barnes-Hut multipole acceptance criterion (θ).
    pub theta: f32,
    /// Particle-count threshold above which Barnes-Hut replaces
    /// brute-force repulsion.
    pub brute_force_threshold: usize,
    /// Enable pairwise collision resolution (O(n²) — see
    /// `brute_force_threshold` gate in [`ForceDirectedLayout3D::tick`]).
    pub collision: bool,
    pub collision_strength: f32,
}

impl Default for ForceParams3D {
    fn default() -> Self {
        Self {
            charge_strength: 900.0,
            link_strength: 0.08,
            link_distance: 60.0,
            center_strength: 0.02,
            center: (0.0, 0.0, 0.0),
            velocity_decay: 0.4,
            alpha_decay: 0.0228,
            alpha_min: 0.001,
            theta: barnes_hut_3d::DEFAULT_THETA,
            brute_force_threshold: barnes_hut_3d::BRUTE_FORCE_THRESHOLD,
            collision: true,
            collision_strength: 0.7,
        }
    }
}

/// Baseline frame rate the `step` normalization assumes — identical
/// convention to [`super::force_directed::ForceDirectedLayout`].
const NORMALIZE_FPS: f32 = 60.0;
/// Caps a single `tick()` call's effective step count.
const MAX_STEP: f32 = 4.0;
/// Below this per-tick max displacement, treat the sim as settled even
/// if `alpha` hasn't formally bottomed out yet.
const SETTLE_DISPLACEMENT_EPS: f32 = 0.05;

pub struct ForceDirectedLayout3D {
    params: ForceParams3D,
    alpha: f32,
    alpha_target: f32,
}

impl Default for ForceDirectedLayout3D {
    fn default() -> Self {
        Self::new(ForceParams3D::default())
    }
}

impl ForceDirectedLayout3D {
    pub fn new(params: ForceParams3D) -> Self {
        Self { params, alpha: 1.0, alpha_target: 0.0 }
    }

    pub fn params(&self) -> &ForceParams3D {
        &self.params
    }

    pub fn set_params(&mut self, params: ForceParams3D) {
        self.params = params;
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

/// Deterministic (index-pair-seeded, no `Math::random`/wall-clock time —
/// same splitmix/LCG convention this module's own tests use) unit-ish
/// nudge direction for two exactly-coincident particles. **Live-caught
/// Wave 2 defect, fixed in Wave 3** (`uzor-graph/CLAUDE.md`'s divergence
/// log): the 2D `force_directed.rs::apply_collision`'s coincident-nudge
/// only perturbs `x` — carried over verbatim here for Wave 1, this
/// nudged ONLY the x-axis in 3D too, which can never break a shared
/// z-plane symmetry (every coincident pair would separate along x,
/// staying at whatever z they started at). Spreads DIFFERENT coincident
/// pairs across DIFFERENT directions on the unit sphere (not a single
/// fixed axis) so a stack of coincident 3D nodes can't reconverge onto
/// one shared symmetry plane either.
fn coincident_nudge_direction(i: usize, j: usize) -> (f32, f32, f32) {
    let seed = ((i as u64) << 32 | j as u64) ^ 0x9E37_79B9_7F4A_7C15;
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (((state >> 40) as u32) as f32 / (1u32 << 24) as f32) * 2.0 - 1.0
    };
    let (x, y, z) = (next(), next(), next());
    let len = (x * x + y * y + z * z).sqrt().max(1e-6);
    (x / len, y / len, z / len)
}

fn apply_collision_3d(particles: &[Particle], radii: &[f32], strength: f32, force: &mut [(f32, f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = particles[j].x - particles[i].x;
            let dy = particles[j].y - particles[i].y;
            let dz = particles[j].z - particles[i].z;
            let dist2 = dx * dx + dy * dy + dz * dz;
            let min_dist = radii.get(i).copied().unwrap_or(1.0) + radii.get(j).copied().unwrap_or(1.0);
            if dist2 <= 1e-6 {
                // Coincident positions — deterministic nudge across ALL
                // THREE axes (see `coincident_nudge_direction`'s own doc
                // comment for why the 2D-derived x-only nudge this
                // replaced was a live-caught 3D defect) so they don't
                // stay locked together forever.
                let (nx, ny, nz) = coincident_nudge_direction(i, j);
                force[i].0 -= nx * 0.5;
                force[i].1 -= ny * 0.5;
                force[i].2 -= nz * 0.5;
                force[j].0 += nx * 0.5;
                force[j].1 += ny * 0.5;
                force[j].2 += nz * 0.5;
                continue;
            }
            if dist2 < min_dist * min_dist {
                let dist = dist2.sqrt();
                let overlap = (min_dist - dist) * strength;
                let nx = dx / dist;
                let ny = dy / dist;
                let nz = dz / dist;
                force[i].0 -= nx * overlap * 0.5;
                force[i].1 -= ny * overlap * 0.5;
                force[i].2 -= nz * overlap * 0.5;
                force[j].0 += nx * overlap * 0.5;
                force[j].1 += ny * overlap * 0.5;
                force[j].2 += nz * overlap * 0.5;
            }
        }
    }
}

impl Layout for ForceDirectedLayout3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
        let n = particles.len();
        if n == 0 {
            return LayoutTickResult { alpha: self.alpha, max_displacement: 0.0, settled: true };
        }

        let mut force = vec![(0f32, 0f32, 0f32); n];

        if n > self.params.brute_force_threshold {
            let ot = Octree::build(particles);
            ot.accumulate_forces(particles, self.params.theta, self.params.charge_strength, &mut force);
        } else {
            barnes_hut_3d::apply_repulsion_brute_force_3d(particles, self.params.charge_strength, &mut force);
        }

        for e in topo.edges {
            let a = e.from.index();
            let b = e.to.index();
            if a >= n || b >= n || a == b {
                continue;
            }
            let dx = particles[b].x - particles[a].x;
            let dy = particles[b].y - particles[a].y;
            let dz = particles[b].z - particles[a].z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
            let ideal = self.params.link_distance;
            let diff = (dist - ideal) / dist * self.params.link_strength;
            let fx = dx * diff;
            let fy = dy * diff;
            let fz = dz * diff;
            force[a].0 += fx;
            force[a].1 += fy;
            force[a].2 += fz;
            force[b].0 -= fx;
            force[b].1 -= fy;
            force[b].2 -= fz;
        }

        for i in 0..n {
            let dx = self.params.center.0 - particles[i].x;
            let dy = self.params.center.1 - particles[i].y;
            let dz = self.params.center.2 - particles[i].z;
            force[i].0 += dx * self.params.center_strength;
            force[i].1 += dy * self.params.center_strength;
            force[i].2 += dz * self.params.center_strength;
        }

        if self.params.collision && n <= self.params.brute_force_threshold {
            apply_collision_3d(particles, &topo.radii, self.params.collision_strength, &mut force);
        }

        // Framerate-independent step — same convention as the 2D layout.
        let step = (dt * NORMALIZE_FPS).clamp(0.0, MAX_STEP);
        let alpha = self.alpha;
        let decay_factor = (1.0 - self.params.velocity_decay).powf(step);

        let mut max_disp = 0f32;
        for i in 0..n {
            if particles[i].is_pinned_3d() {
                if let Some(fx) = particles[i].fx {
                    particles[i].x = fx;
                }
                if let Some(fy) = particles[i].fy {
                    particles[i].y = fy;
                }
                if let Some(fz) = particles[i].fz {
                    particles[i].z = fz;
                }
                particles[i].vx = 0.0;
                particles[i].vy = 0.0;
                particles[i].vz = 0.0;
                continue;
            }
            let (fx, fy, fz) = force[i];
            particles[i].vx = (particles[i].vx + fx * alpha) * decay_factor;
            particles[i].vy = (particles[i].vy + fy * alpha) * decay_factor;
            particles[i].vz = (particles[i].vz + fz * alpha) * decay_factor;
            let dx = particles[i].vx * step;
            let dy = particles[i].vy * step;
            let dz = particles[i].vz * step;
            particles[i].x += dx;
            particles[i].y += dy;
            particles[i].z += dz;
            let disp = (dx * dx + dy * dy + dz * dz).sqrt();
            if disp > max_disp {
                max_disp = disp;
            }
        }

        let alpha_decay_factor = (1.0 - self.params.alpha_decay).powf(step);
        self.alpha = self.alpha_target + (self.alpha - self.alpha_target) * alpha_decay_factor;
        if self.alpha < 0.0 {
            self.alpha = 0.0;
        }

        let settled = self.alpha < self.params.alpha_min || max_disp < SETTLE_DISPLACEMENT_EPS;
        LayoutTickResult { alpha: self.alpha, max_displacement: max_disp, settled }
    }

    fn reheat(&mut self, alpha: f32) {
        self.alpha = self.alpha.max(alpha.clamp(0.0, 1.0));
    }

    fn is_settled(&self) -> bool {
        self.alpha < self.params.alpha_min
    }

    fn set_alpha_target(&mut self, target: f32) {
        self.alpha_target = target.clamp(0.0, 1.0);
    }

    fn alpha_target(&self) -> f32 {
        self.alpha_target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SimEdge;

    fn topo<'a>(node_count: usize, edges: &'a [SimEdge], degree: &'a [u32], radii: Vec<f32>) -> SimTopology<'a> {
        SimTopology { node_count, edges, degree, radii }
    }

    /// Deterministic pseudo-random 3D particle scatter — same
    /// splitmix64-style LCG convention `barnes_hut_3d.rs`'s own test
    /// helper uses, kept as an independent local copy (test-only, no
    /// production coupling between the two modules' test suites).
    fn deterministic_particles_3d(n: usize) -> Vec<Particle> {
        let mut particles = Vec::with_capacity(n);
        let mut state: u64 = 0xD1B54A32D192ED03;
        for _ in 0..n {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rx = ((state >> 33) as u32 % 400) as f32 - 200.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let ry = ((state >> 33) as u32 % 400) as f32 - 200.0;
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let rz = ((state >> 33) as u32 % 400) as f32 - 200.0;
            particles.push(Particle::at3(rx, ry, rz));
        }
        particles
    }

    #[test]
    fn alpha_cools_monotonically_and_eventually_settles() {
        let mut layout = ForceDirectedLayout3D::default();
        let mut particles = vec![Particle::at3(-10.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0), Particle::at3(0.0, 15.0, 5.0)];
        let degree = vec![0u32; 3];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        let mut prev_alpha = f32::MAX;
        let mut settled_at = None;
        for i in 0..600 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            assert!(r.alpha <= prev_alpha + 1e-6, "alpha increased at tick {i}: {} > {}", r.alpha, prev_alpha);
            prev_alpha = r.alpha;
            if r.settled {
                settled_at = Some(i);
                break;
            }
        }
        assert!(settled_at.is_some(), "3D simulation never settled within 600 ticks");
    }

    #[test]
    fn pinned_particle_position_is_held_across_ticks() {
        let mut layout = ForceDirectedLayout3D::default();
        let mut particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(5.0, 0.0, 0.0)];
        particles[0].pin3(0.0, 0.0, 0.0);
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![4.0; 2]);

        for _ in 0..30 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }
        assert_eq!(particles[0].x, 0.0);
        assert_eq!(particles[0].y, 0.0);
        assert_eq!(particles[0].z, 0.0);
        assert!(particles[1].x.abs() > 5.0, "unpinned particle should have moved under repulsion");
    }

    #[test]
    fn reheat_raises_alpha_and_clears_settled() {
        let mut layout = ForceDirectedLayout3D::default();
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![4.0; 2]);
        let mut particles = vec![Particle::at3(-5.0, 0.0, 0.0), Particle::at3(5.0, 0.0, 0.0)];
        for _ in 0..500 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }
        assert!(layout.is_settled());
        layout.reheat(0.5);
        assert!(!layout.is_settled());
    }

    /// Live-caught Wave 2 defect, fixed in Wave 3: the coincident-particle
    /// collision nudge must break symmetry on ALL THREE axes, not just
    /// `x` (a pure-x nudge can never separate two z-coincident nodes off
    /// their shared z-plane).
    #[test]
    fn coincident_particle_collision_nudge_perturbs_all_three_axes() {
        let particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(0.0, 0.0, 0.0)];
        let radii = vec![4.0, 4.0];
        let mut force = vec![(0f32, 0f32, 0f32); 2];

        apply_collision_3d(&particles, &radii, 0.7, &mut force);

        assert_ne!(force[0].1, 0.0, "the y-axis component must be perturbed too, not left at 0.0");
        assert_ne!(force[0].2, 0.0, "the z-axis component must be perturbed too — this is the exact bug the owner caught live");
        // Newton's third law — the pair separates in opposite directions.
        assert_eq!(force[0].0, -force[1].0);
        assert_eq!(force[0].1, -force[1].1);
        assert_eq!(force[0].2, -force[1].2);
    }

    /// Two DIFFERENT coincident pairs must not nudge along the identical
    /// direction — otherwise a larger coincident stack would still
    /// collapse back onto one shared plane pair-by-pair.
    #[test]
    fn different_coincident_pairs_nudge_along_different_directions() {
        let dir_a = coincident_nudge_direction(0, 1);
        let dir_b = coincident_nudge_direction(2, 3);
        assert_ne!(dir_a, dir_b, "distinct index pairs must not collapse onto the same nudge direction");
    }

    #[test]
    fn empty_particle_set_reports_settled_immediately() {
        let mut layout = ForceDirectedLayout3D::default();
        let edges: Vec<SimEdge> = Vec::new();
        let degree: Vec<u32> = Vec::new();
        let t = topo(0, &edges, &degree, Vec::new());
        let mut particles: Vec<Particle> = Vec::new();
        let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r.settled);
    }

    /// W3D arc plan §4 Wave 1 gate: N particles, random 3D seed, run
    /// enough ticks to settle, assert `is_settled()` and that no
    /// coordinate/velocity ever goes NaN/inf — the `MIN_DIST2`/
    /// `MIN_SPLIT_DIST2` softening/merge guards are what this proves in
    /// 3D (mirrors `barnes_hut_3d.rs`'s own coincident-stack test, but
    /// exercised through the full integrated layout instead of the
    /// octree alone).
    #[test]
    fn random_3d_seed_settles_without_nan_or_inf() {
        let n = 60;
        let mut particles = deterministic_particles_3d(n);
        let mut layout = ForceDirectedLayout3D::default();
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);

        let mut settled = false;
        for _ in 0..2000 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            for p in &particles {
                assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "position went non-finite: {p:?}");
                assert!(p.vx.is_finite() && p.vy.is_finite() && p.vz.is_finite(), "velocity went non-finite: {p:?}");
            }
            if r.settled {
                settled = true;
                break;
            }
        }
        assert!(settled, "3D simulation with a random seed never settled within 2000 ticks");
    }
}
