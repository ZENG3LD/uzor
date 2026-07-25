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
    /// Enable pairwise collision resolution — decoupled from
    /// `brute_force_threshold` (Wave G1 fix), the 3D mirror of
    /// [`super::force_directed::ForceParams::collision`]. See that
    /// field's own doc comment.
    pub collision: bool,
    pub collision_strength: f32,
    /// Baseline frame rate the `step` normalization assumes — identical
    /// convention to [`super::force_directed::ForceParams::normalize_fps`]
    /// (Wave G2 configurability — was a private constant).
    pub normalize_fps: f32,
    /// Caps a single [`ForceDirectedLayout3D::tick`] call's effective
    /// step count (Wave G2 configurability — was a private constant).
    pub max_step: f32,
    /// Below this per-tick max displacement, treat the sim as settled
    /// even if `alpha` hasn't formally bottomed out yet (Wave G2
    /// configurability — was a private constant).
    pub settle_displacement_eps: f32,
    /// Wave G1 fix — 3D mirror of
    /// [`super::force_directed::ForceParams::seed_degenerate_positions`].
    /// See that field's own doc comment.
    pub seed_degenerate_positions: bool,
    /// Repulsion/link-force softening floor — 3D mirror of
    /// [`super::force_directed::ForceParams::min_dist2`] (Wave G2b
    /// configurability — was the private [`barnes_hut_3d::MIN_DIST2`]
    /// constant).
    pub min_dist2: f32,
    /// Octree coincident-point merge threshold — 3D mirror of
    /// [`super::force_directed::ForceParams::min_split_dist2`] (Wave G2b
    /// configurability — was the private [`barnes_hut_3d::MIN_SPLIT_DIST2`]
    /// constant).
    pub min_split_dist2: f32,
    /// Octree subdivision floor — 3D mirror of
    /// [`super::force_directed::ForceParams::min_cell_size`] (Wave G2b
    /// configurability — was the private [`barnes_hut_3d::MIN_CELL_SIZE`]
    /// constant).
    pub min_cell_size: f32,
    /// Hard cap on a single particle's own per-tick displacement
    /// magnitude — 3D mirror of
    /// [`super::force_directed::ForceParams::max_displacement_per_tick`]
    /// (Wave G3 item 6 fix). Same default
    /// ([`DEFAULT_MAX_DISPLACEMENT_PER_TICK_3D`]) and measurement
    /// rationale as the 2D field's own doc comment — the 3D force model
    /// is the identical formula generalized to `(x, y, z)`, so the same
    /// measured headroom applies.
    pub max_displacement_per_tick: f32,
}

/// Default for [`ForceParams3D::max_displacement_per_tick`] — 3D mirror
/// of `super::force_directed::DEFAULT_MAX_DISPLACEMENT_PER_TICK` (Wave
/// G3 item 6 fix).
const DEFAULT_MAX_DISPLACEMENT_PER_TICK_3D: f32 = 20_000.0;

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
            normalize_fps: 60.0,
            max_step: 4.0,
            settle_displacement_eps: 0.05,
            seed_degenerate_positions: true,
            min_dist2: barnes_hut_3d::MIN_DIST2,
            min_split_dist2: barnes_hut_3d::MIN_SPLIT_DIST2,
            min_cell_size: barnes_hut_3d::MIN_CELL_SIZE,
            max_displacement_per_tick: DEFAULT_MAX_DISPLACEMENT_PER_TICK_3D,
        }
    }
}

/// Below this position spread (per-axis half-extent, world units), the
/// WHOLE particle set is treated as degenerate — 3D mirror of
/// `super::force_directed::DEGENERACY_EPS`, checking `x`/`y`/`z` all
/// three.
const DEGENERACY_EPS: f32 = 1e-6;

/// Growing-radius scale for [`seed_phyllotaxis_sphere_positions`] —
/// same value as `super::force_directed::SEED_RADIUS_SCALE`.
const SEED_RADIUS_SCALE: f32 = 10.0;

/// Golden-angle azimuth increment — same value as
/// `super::force_directed::SEED_GOLDEN_ANGLE`/
/// [`super::radial_3d::RadialParams3D::golden_angle`]'s own default (both
/// private in their own modules, re-declared here).
const SEED_GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Whether EVERY particle in `particles` sits on the exact same 3D point.
/// 3D mirror of `super::force_directed::positions_are_degenerate`,
/// checking `z` too. See [`ForceParams3D::seed_degenerate_positions`].
fn positions_are_degenerate_3d(particles: &[Particle]) -> bool {
    if particles.len() < 2 {
        return false;
    }
    let (x0, y0, z0) = (particles[0].x, particles[0].y, particles[0].z);
    particles
        .iter()
        .all(|p| (p.x - x0).abs() <= DEGENERACY_EPS && (p.y - y0).abs() <= DEGENERACY_EPS && (p.z - z0).abs() <= DEGENERACY_EPS)
}

/// Deterministic golden-angle phyllotaxis seed, generalized to 3D as an
/// expanding Fibonacci-sphere spiral (no RNG) — d3-force's own
/// `initializeNodes` fallback has no 3D form to mirror directly, so this
/// combines that same growing-radius-by-index MAGNITUDE with a Fibonacci-
/// sphere DIRECTION (the same golden-angle-azimuth technique
/// [`super::radial_3d`] already uses for its own child-direction fan) —
/// guarantees `x`, `y`, AND `z` all vary (unlike a flat XY spiral, which
/// would recreate the exact z-plane-degeneracy defect
/// `engine3d.rs::ensure_z_variance` exists to correct for at the ENGINE
/// level). Index `i`'s direction: `z_unit = 1 - 2*(i+0.5)/n` (monotonic
/// across `(-1, 1)`), `theta = i * SEED_GOLDEN_ANGLE` azimuth around that
/// z; magnitude: `SEED_RADIUS_SCALE * sqrt(0.5 + i)`, same growth curve as
/// the 2D seed.
fn seed_phyllotaxis_sphere_positions(particles: &mut [Particle], center: (f32, f32, f32)) {
    let n = particles.len().max(1) as f32;
    for (i, p) in particles.iter_mut().enumerate() {
        let t = (i as f32 + 0.5) / n;
        let z_unit = (1.0 - 2.0 * t).clamp(-1.0, 1.0);
        let ring_unit = (1.0 - z_unit * z_unit).max(0.0).sqrt();
        let theta = i as f32 * SEED_GOLDEN_ANGLE;
        let radius = SEED_RADIUS_SCALE * (0.5 + i as f32).sqrt();
        p.x = center.0 + ring_unit * theta.cos() * radius;
        p.y = center.1 + ring_unit * theta.sin() * radius;
        p.z = center.2 + z_unit * radius;
    }
}

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

/// O(n²) brute-force collision — used below `brute_force_threshold`.
/// Above it, [`Octree::apply_collision_3d`] resolves the SAME pairs (via
/// [`barnes_hut_3d::collision_pair_force_3d`], the one shared per-pair
/// implementation both paths call) using the already-built octree
/// instead (Wave G1 fix). `coincident_nudge_direction` moved to
/// `barnes_hut_3d.rs` alongside it — see that module for both.
fn apply_collision_3d(particles: &[Particle], radii: &[f32], strength: f32, force: &mut [(f32, f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        for j in (i + 1)..n {
            barnes_hut_3d::collision_pair_force_3d(i, j, particles, radii, strength, force);
        }
    }
}

impl Layout for ForceDirectedLayout3D {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
        let n = particles.len();
        if n == 0 {
            return LayoutTickResult { alpha: self.alpha, max_displacement: 0.0, settled: true };
        }

        // Wave G1 fix — 3D mirror of the 2D layout's own degenerate-start
        // seed. See `ForceParams3D::seed_degenerate_positions`.
        if self.params.seed_degenerate_positions && positions_are_degenerate_3d(particles) {
            seed_phyllotaxis_sphere_positions(particles, self.params.center);
        }

        let mut force = vec![(0f32, 0f32, 0f32); n];

        // Wave G1 fix: keep the octree (not just its output) so collision
        // can reuse it below instead of either being disabled or building
        // a second tree.
        let ot = if n > self.params.brute_force_threshold {
            let ot = Octree::build(particles, self.params.min_split_dist2, self.params.min_cell_size);
            ot.accumulate_forces(particles, self.params.theta, self.params.charge_strength, self.params.min_dist2, &mut force);
            Some(ot)
        } else {
            barnes_hut_3d::apply_repulsion_brute_force_3d(particles, self.params.charge_strength, self.params.min_dist2, &mut force);
            None
        };

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

        // Wave G1 fix: collision's enable is now independent of `n` — see
        // the 2D layout's own tick() for the identical reasoning.
        if self.params.collision {
            match &ot {
                Some(ot) => ot.apply_collision_3d(particles, &topo.radii, self.params.collision_strength, &mut force),
                None => apply_collision_3d(particles, &topo.radii, self.params.collision_strength, &mut force),
            }
        }

        // Framerate-independent step — same convention as the 2D layout.
        let step = (dt * self.params.normalize_fps).clamp(0.0, self.params.max_step);
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
            // Wave G3 item 6 fix — 3D mirror of the 2D layout's own
            // velocity clamp. See `ForceParams3D::max_displacement_per_tick`.
            if step > 1e-9 {
                let max_speed = self.params.max_displacement_per_tick / step;
                let speed2 = particles[i].vx * particles[i].vx + particles[i].vy * particles[i].vy + particles[i].vz * particles[i].vz;
                if speed2 > max_speed * max_speed {
                    let scale = max_speed / speed2.sqrt();
                    particles[i].vx *= scale;
                    particles[i].vy *= scale;
                    particles[i].vz *= scale;
                }
            }
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

        let settled = self.alpha < self.params.alpha_min || max_disp < self.params.settle_displacement_eps;
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

    // `different_coincident_pairs_nudge_along_different_directions` moved
    // to `barnes_hut_3d.rs` (Wave G1 fix) alongside
    // `coincident_nudge_direction` itself, which now lives there too.

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

    /// Wave G1 fix 1 gate — 3D mirror of
    /// `force_directed::tests::collision_stays_active_above_the_brute_force_threshold_and_separates_a_coincident_cluster`:
    /// collision must not be silently disabled above `brute_force_threshold`.
    #[test]
    fn collision_stays_active_above_the_brute_force_threshold_and_separates_a_coincident_cluster() {
        let params = ForceParams3D { brute_force_threshold: 2, collision: true, seed_degenerate_positions: false, ..ForceParams3D::default() };
        let mut layout = ForceDirectedLayout3D::new(params);
        let n = 6;
        let mut particles = vec![Particle::at3(0.0, 0.0, 0.0); n]; // n=6 > threshold=2
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);

        for _ in 0..5 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }

        let spread = particles.iter().map(|p| p.x.abs() + p.y.abs() + p.z.abs()).fold(0.0f32, f32::max);
        assert!(spread > 0.5, "particles must separate above the brute-force threshold, spread={spread}");
    }

    /// Wave G1 fix 2 gate — 3D mirror of
    /// `force_directed::tests::unseeded_sim_above_500_nodes_separates_instead_of_freezing_forever`.
    #[test]
    fn unseeded_sim_above_500_nodes_separates_instead_of_freezing_forever() {
        let n = 600;
        let mut particles = vec![Particle::default(); n];
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);
        let mut layout = ForceDirectedLayout3D::default();

        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let (x0, y0, z0) = (particles[0].x, particles[0].y, particles[0].z);
        let moved = particles.iter().any(|p| (p.x - x0).abs() > 1e-3 || (p.y - y0).abs() > 1e-3 || (p.z - z0).abs() > 1e-3);
        assert!(moved, "particles above the collision brute-force threshold must separate after seeding, not stay coincident");
    }

    /// Wave G1 fix 2 gate, the other direction — 3D mirror of
    /// `force_directed::tests::a_pre_seeded_non_degenerate_start_is_unaffected_by_the_seeding_fallback`.
    #[test]
    fn a_pre_seeded_non_degenerate_start_is_unaffected_by_the_seeding_fallback() {
        let make_particles = || vec![Particle::at3(-10.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0), Particle::at3(0.0, 15.0, 5.0)];
        let degree = vec![0u32; 3];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        let mut particles_on = make_particles();
        let mut layout_on = ForceDirectedLayout3D::new(ForceParams3D { seed_degenerate_positions: true, ..ForceParams3D::default() });
        let mut particles_off = make_particles();
        let mut layout_off = ForceDirectedLayout3D::new(ForceParams3D { seed_degenerate_positions: false, ..ForceParams3D::default() });

        for _ in 0..10 {
            layout_on.tick(&t, &mut particles_on, 1.0 / 60.0);
            layout_off.tick(&t, &mut particles_off, 1.0 / 60.0);
        }
        assert_eq!(particles_on, particles_off, "a non-degenerate start must be byte-identical regardless of the seeding fallback");
    }

    /// Wave G1 fix gate — 3D phyllotaxis seed produces distinct, x/y/z-
    /// varying, deterministic positions (no flat-z-plane regression, see
    /// `seed_phyllotaxis_sphere_positions`'s own doc comment).
    #[test]
    fn phyllotaxis_sphere_seed_varies_all_three_axes_deterministically_and_avoids_collisions() {
        let mut a = vec![Particle::default(); 40];
        let mut b = vec![Particle::default(); 40];
        assert!(positions_are_degenerate_3d(&a));
        seed_phyllotaxis_sphere_positions(&mut a, (0.0, 0.0, 0.0));
        seed_phyllotaxis_sphere_positions(&mut b, (0.0, 0.0, 0.0));
        assert_eq!(a, b, "phyllotaxis seeding must be deterministic — no RNG");
        assert!(!positions_are_degenerate_3d(&a));

        let z_min = a.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let z_max = a.iter().map(|p| p.z).fold(f32::MIN, f32::max);
        assert!(z_max - z_min > 1.0, "z must genuinely vary, not stay flat");

        for i in 0..a.len() {
            for j in (i + 1)..a.len() {
                let dx = a[i].x - a[j].x;
                let dy = a[i].y - a[j].y;
                let dz = a[i].z - a[j].z;
                assert!(dx * dx + dy * dy + dz * dz > 1e-6, "indices {i} and {j} landed on the same point");
            }
        }
    }

    /// Wave G2 configurability gate — 3D mirror of
    /// `force_directed::tests::new_public_step_params_default_to_the_prior_hardcoded_constants`.
    #[test]
    fn new_public_step_params_default_to_the_prior_hardcoded_constants() {
        let p = ForceParams3D::default();
        assert_eq!(p.normalize_fps, 60.0);
        assert_eq!(p.max_step, 4.0);
        assert_eq!(p.settle_displacement_eps, 0.05);
        assert!(p.seed_degenerate_positions);
        assert_eq!(p.min_dist2, barnes_hut_3d::MIN_DIST2);
        assert_eq!(p.min_split_dist2, barnes_hut_3d::MIN_SPLIT_DIST2);
        assert_eq!(p.min_cell_size, barnes_hut_3d::MIN_CELL_SIZE);
    }

    /// Wave G2b configurability gate — 3D mirror of `force_directed::tests::
    /// a_larger_min_dist2_caps_repulsion_between_near_coincident_particles_more_aggressively`.
    #[test]
    fn a_larger_min_dist2_caps_repulsion_between_near_coincident_particles_more_aggressively() {
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![0.001; 2]);

        let mut default_particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(1e-4, 0.0, 0.0)];
        let mut default_layout =
            ForceDirectedLayout3D::new(ForceParams3D { seed_degenerate_positions: false, collision: false, ..ForceParams3D::default() });
        default_layout.tick(&t, &mut default_particles, 1.0 / 60.0);

        let mut softened_particles = vec![Particle::at3(0.0, 0.0, 0.0), Particle::at3(1e-4, 0.0, 0.0)];
        let mut softened_layout = ForceDirectedLayout3D::new(ForceParams3D {
            seed_degenerate_positions: false,
            collision: false,
            min_dist2: barnes_hut_3d::MIN_DIST2 * 100.0,
            ..ForceParams3D::default()
        });
        softened_layout.tick(&t, &mut softened_particles, 1.0 / 60.0);

        let default_speed = (default_particles[0].vx.powi(2) + default_particles[0].vy.powi(2) + default_particles[0].vz.powi(2)).sqrt();
        let softened_speed =
            (softened_particles[0].vx.powi(2) + softened_particles[0].vy.powi(2) + softened_particles[0].vz.powi(2)).sqrt();
        assert!(
            softened_speed < default_speed,
            "a larger min_dist2 softening floor must cap the resulting velocity lower: default={default_speed} softened={softened_speed}"
        );
    }

    // ── Wave G3 item 6 — 3D mirror of `force_directed`'s own displacement clamp ──

    #[test]
    fn max_displacement_per_tick_default_matches_the_2d_layouts_own_measured_value() {
        assert_eq!(ForceParams3D::default().max_displacement_per_tick, 20_000.0);
    }

    /// 3D mirror of `force_directed::tests::
    /// max_displacement_per_tick_actually_bounds_output_when_overridden_low`.
    #[test]
    fn max_displacement_per_tick_actually_bounds_output_when_overridden_low() {
        let n = 600;
        let mut particles = vec![Particle::default(); n];
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);
        let mut layout = ForceDirectedLayout3D::new(ForceParams3D {
            seed_degenerate_positions: false,
            max_displacement_per_tick: 10.0,
            ..ForceParams3D::default()
        });

        for _ in 0..30 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            assert!(
                r.max_displacement <= 10.0 + 1e-3,
                "an overridden 10.0 clamp must never let any tick's own max_displacement exceed it: got {}",
                r.max_displacement
            );
        }
        for p in &particles {
            assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "every coordinate must stay finite under the clamp");
        }
    }

    /// 3D mirror of `force_directed::tests::
    /// an_ordinary_non_exploding_simulation_is_byte_identical_with_the_clamp_effectively_disabled`.
    #[test]
    fn an_ordinary_non_exploding_simulation_is_byte_identical_with_the_clamp_effectively_disabled() {
        let make_particles = || vec![Particle::at3(-10.0, 0.0, 0.0), Particle::at3(10.0, 0.0, 0.0), Particle::at3(0.0, 15.0, 5.0)];
        let degree = vec![0u32; 3];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        let mut particles_default = make_particles();
        let mut layout_default = ForceDirectedLayout3D::default();
        let mut particles_disabled = make_particles();
        let mut layout_disabled =
            ForceDirectedLayout3D::new(ForceParams3D { max_displacement_per_tick: f32::INFINITY, ..ForceParams3D::default() });

        for _ in 0..60 {
            layout_default.tick(&t, &mut particles_default, 1.0 / 60.0);
            layout_disabled.tick(&t, &mut particles_disabled, 1.0 / 60.0);
        }
        assert_eq!(particles_default, particles_disabled, "the default clamp must be a complete no-op for an ordinary simulation");
    }
}
