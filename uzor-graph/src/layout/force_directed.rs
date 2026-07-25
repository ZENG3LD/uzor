//! `ForceDirectedLayout` — the primary [`super::Layout`] implementation.
//!
//! d3-force model (engine design doc §1.1/§3): named composable forces
//! (many-body repulsion via Barnes-Hut, link spring, centering,
//! optional collision), semi-implicit-Euler integration, `alpha`
//! cooling that freezes the sim once settled and wakes on
//! [`ForceDirectedLayout::reheat`].

use crate::graph::SimTopology;
use crate::particle::Particle;

use super::barnes_hut::{self, Quadtree};
use super::{Layout, LayoutTickResult};

/// Tunable force-model parameters. Defaults are d3-force-shaped values
/// scaled for a world where 1 unit ≈ 1 px at `zoom = 1.0`; expect a
/// visual tuning pass once the demo is actually watched (see
/// `uzor-graph/RUN.md`).
#[derive(Debug, Clone, Copy)]
pub struct ForceParams {
    /// Many-body repulsion magnitude (`strength / distance²`).
    pub charge_strength: f32,
    /// Link spring strength.
    pub link_strength: f32,
    /// Link spring ideal length.
    pub link_distance: f32,
    /// Weak uniform pull toward `center`, keeps disconnected components
    /// from drifting to infinity.
    pub center_strength: f32,
    pub center: (f32, f32),
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
    /// `brute_force_threshold` (Wave G1 fix): below the threshold this
    /// runs the O(n²) brute-force path
    /// ([`super::barnes_hut::collision_pair_force`] looped directly),
    /// above it the SAME resolution runs through the already-built
    /// quadtree ([`super::barnes_hut::Quadtree::apply_collision`]) — this
    /// flag alone decides whether collision runs at all, never `n`.
    pub collision: bool,
    pub collision_strength: f32,
    /// Baseline frame rate the `step` normalization assumes — `dt ==
    /// 1/60` behaves exactly like one classic d3-force tick (Wave G2
    /// configurability — was a private constant).
    pub normalize_fps: f32,
    /// Caps a single [`ForceDirectedLayout::tick`] call's effective step
    /// count so a stalled frame (window drag, alt-tab) can't blow the sim
    /// up (Wave G2 configurability — was a private constant).
    pub max_step: f32,
    /// Below this per-tick max displacement, treat the sim as settled
    /// even if `alpha` hasn't formally bottomed out yet (Wave G2
    /// configurability — was a private constant).
    pub settle_displacement_eps: f32,
    /// Wave G1 fix: when every particle is still at the exact same
    /// (degenerate) position — the shape `vec![Particle::default(); n]`
    /// produces, e.g. `engine.rs`'s own [`crate::engine::GraphEngine::new`]
    /// — every OTHER force term evaluates to exactly zero for an
    /// identical pair (repulsion's/the link spring's direction term is
    /// `0/1`), so without this seed a caller who never externally
    /// pre-positions particles relies SOLELY on collision's coincidence
    /// nudge to ever separate. Seeding is applied ONLY when the WHOLE
    /// particle set is genuinely degenerate — see
    /// [`positions_are_degenerate`] — so a caller who already placed
    /// particles anywhere non-degenerate sees byte-identical behavior
    /// with this either `true` or `false`. Default `true`, since the
    /// un-seeded case this guards is the actual Wave G1 defect fix, not a
    /// debatable default.
    pub seed_degenerate_positions: bool,
}

impl Default for ForceParams {
    fn default() -> Self {
        Self {
            charge_strength: 900.0,
            link_strength: 0.08,
            link_distance: 60.0,
            center_strength: 0.02,
            center: (0.0, 0.0),
            velocity_decay: 0.4,
            alpha_decay: 0.0228,
            alpha_min: 0.001,
            theta: barnes_hut::DEFAULT_THETA,
            brute_force_threshold: barnes_hut::BRUTE_FORCE_THRESHOLD,
            collision: true,
            collision_strength: 0.7,
            normalize_fps: 60.0,
            max_step: 4.0,
            settle_displacement_eps: 0.05,
            seed_degenerate_positions: true,
        }
    }
}

/// Below this position spread (per-axis half-extent, world units), the
/// WHOLE particle set is treated as degenerate — every particle sits on
/// (or within float noise of) the exact same point, e.g.
/// `vec![Particle::default(); n]`. See [`ForceParams::seed_degenerate_positions`].
const DEGENERACY_EPS: f32 = 1e-6;

/// Growing-radius scale for [`seed_phyllotaxis_positions`] — matches
/// d3-force's own `initialRadius = 10` (`simulation.js::initializeNodes`),
/// this crate's own stated reference model.
const SEED_RADIUS_SCALE: f32 = 10.0;

/// Golden-angle azimuth increment (`TAU / phi^2`, `phi` = the golden
/// ratio) — same value as [`super::radial_3d::GOLDEN_ANGLE`] (private
/// there, re-declared here; d3-force's own `initialAngle = PI * (3 -
/// sqrt(5))`, the identical irrational constant under a different
/// derivation). See [`seed_phyllotaxis_positions`].
const SEED_GOLDEN_ANGLE: f32 = 2.399_963_2;

/// Whether EVERY particle in `particles` sits on the exact same point —
/// the shape a never-externally-seeded caller produces. Fewer than 2
/// particles is never degenerate (nothing to separate). See
/// [`ForceParams::seed_degenerate_positions`].
fn positions_are_degenerate(particles: &[Particle]) -> bool {
    if particles.len() < 2 {
        return false;
    }
    let (x0, y0) = (particles[0].x, particles[0].y);
    particles.iter().all(|p| (p.x - x0).abs() <= DEGENERACY_EPS && (p.y - y0).abs() <= DEGENERACY_EPS)
}

/// Deterministic golden-angle phyllotaxis spiral seed — no RNG, matching
/// d3-force's own internal fallback for un-positioned nodes
/// (`forceSimulation`'s `initializeNodes`, this crate's stated reference
/// model). Index `i` lands at `radius = SEED_RADIUS_SCALE *
/// sqrt(0.5 + i)`, `angle = i * SEED_GOLDEN_ANGLE` — an expanding spiral
/// that never revisits the same direction and never places two indices at
/// the same point.
fn seed_phyllotaxis_positions(particles: &mut [Particle], center: (f32, f32)) {
    for (i, p) in particles.iter_mut().enumerate() {
        let radius = SEED_RADIUS_SCALE * (0.5 + i as f32).sqrt();
        let angle = i as f32 * SEED_GOLDEN_ANGLE;
        p.x = center.0 + radius * angle.cos();
        p.y = center.1 + radius * angle.sin();
    }
}

pub struct ForceDirectedLayout {
    params: ForceParams,
    alpha: f32,
    alpha_target: f32,
}

impl Default for ForceDirectedLayout {
    fn default() -> Self {
        Self::new(ForceParams::default())
    }
}

impl ForceDirectedLayout {
    pub fn new(params: ForceParams) -> Self {
        Self { params, alpha: 1.0, alpha_target: 0.0 }
    }

    pub fn params(&self) -> &ForceParams {
        &self.params
    }

    pub fn set_params(&mut self, params: ForceParams) {
        self.params = params;
    }

    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

/// O(n²) brute-force collision — used below `brute_force_threshold`. Above
/// it, [`super::barnes_hut::Quadtree::apply_collision`] resolves the SAME
/// pairs (via [`super::barnes_hut::collision_pair_force`], the one shared
/// per-pair implementation both paths call) using the already-built
/// quadtree instead (Wave G1 fix).
fn apply_collision(particles: &[Particle], radii: &[f32], strength: f32, force: &mut [(f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        for j in (i + 1)..n {
            barnes_hut::collision_pair_force(i, j, particles, radii, strength, force);
        }
    }
}

impl Layout for ForceDirectedLayout {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
        let n = particles.len();
        if n == 0 {
            return LayoutTickResult { alpha: self.alpha, max_displacement: 0.0, settled: true };
        }

        // Wave G1 fix: an internally-unpositioned caller (every particle
        // still bit-identical, e.g. `vec![Particle::default(); n]`) gets a
        // deterministic starting spread here — matching d3-force's own
        // `initializeNodes` fallback — instead of relying solely on
        // collision's coincidence nudge to ever escape. A no-op for any
        // caller that already placed particles anywhere non-degenerate.
        if self.params.seed_degenerate_positions && positions_are_degenerate(particles) {
            seed_phyllotaxis_positions(particles, self.params.center);
        }

        let mut force = vec![(0f32, 0f32); n];

        // Wave G1 fix: the quadtree built for the above-threshold
        // repulsion path is KEPT (not dropped) so collision can reuse it
        // below instead of either being disabled or building a second
        // tree.
        let qt = if n > self.params.brute_force_threshold {
            let qt = Quadtree::build(particles);
            qt.accumulate_forces(particles, self.params.theta, self.params.charge_strength, &mut force);
            Some(qt)
        } else {
            barnes_hut::apply_repulsion_brute_force(particles, self.params.charge_strength, &mut force);
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
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let ideal = self.params.link_distance;
            let diff = (dist - ideal) / dist * self.params.link_strength;
            let fx = dx * diff;
            let fy = dy * diff;
            force[a].0 += fx;
            force[a].1 += fy;
            force[b].0 -= fx;
            force[b].1 -= fy;
        }

        for i in 0..n {
            let dx = self.params.center.0 - particles[i].x;
            let dy = self.params.center.1 - particles[i].y;
            force[i].0 += dx * self.params.center_strength;
            force[i].1 += dy * self.params.center_strength;
        }

        // Wave G1 fix: collision's enable is now independent of `n` —
        // below the threshold it runs brute-force, above it the SAME
        // resolution runs through `qt` (built above, never a second
        // tree), so it never has to be silently disabled.
        if self.params.collision {
            match &qt {
                Some(qt) => qt.apply_collision(particles, &topo.radii, self.params.collision_strength, &mut force),
                None => apply_collision(particles, &topo.radii, self.params.collision_strength, &mut force),
            }
        }

        // Framerate-independent step: `dt == 1/60` is exactly one
        // classic d3-force tick; longer/shorter frames scale decay and
        // displacement by the equivalent fractional tick count instead
        // of by raw seconds (raw-seconds scaling would need force
        // constants re-tuned per target frame rate).
        let step = (dt * self.params.normalize_fps).clamp(0.0, self.params.max_step);
        let alpha = self.alpha;
        let decay_factor = (1.0 - self.params.velocity_decay).powf(step);

        let mut max_disp = 0f32;
        for i in 0..n {
            if particles[i].fx.is_some() || particles[i].fy.is_some() {
                if let Some(fx) = particles[i].fx {
                    particles[i].x = fx;
                }
                if let Some(fy) = particles[i].fy {
                    particles[i].y = fy;
                }
                particles[i].vx = 0.0;
                particles[i].vy = 0.0;
                continue;
            }
            let (fx, fy) = force[i];
            particles[i].vx = (particles[i].vx + fx * alpha) * decay_factor;
            particles[i].vy = (particles[i].vy + fy * alpha) * decay_factor;
            let dx = particles[i].vx * step;
            let dy = particles[i].vy * step;
            particles[i].x += dx;
            particles[i].y += dy;
            let disp = (dx * dx + dy * dy).sqrt();
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

    #[test]
    fn alpha_cools_monotonically_and_eventually_settles() {
        let mut layout = ForceDirectedLayout::default();
        let mut particles = vec![Particle::at(-10.0, 0.0), Particle::at(10.0, 0.0), Particle::at(0.0, 15.0)];
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
        assert!(settled_at.is_some(), "simulation never settled within 600 ticks");
    }

    #[test]
    fn pinned_particle_position_is_held_across_ticks() {
        let mut layout = ForceDirectedLayout::default();
        let mut particles = vec![Particle::at(0.0, 0.0), Particle::at(5.0, 0.0)];
        particles[0].pin(0.0, 0.0);
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![4.0; 2]);

        for _ in 0..30 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }
        assert_eq!(particles[0].x, 0.0);
        assert_eq!(particles[0].y, 0.0);
        assert!(particles[1].x.abs() > 5.0, "unpinned particle should have moved under repulsion");
    }

    #[test]
    fn reheat_raises_alpha_and_clears_settled() {
        let mut layout = ForceDirectedLayout::default();
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![4.0; 2]);
        let mut particles = vec![Particle::at(-5.0, 0.0), Particle::at(5.0, 0.0)];
        for _ in 0..500 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }
        assert!(layout.is_settled());
        layout.reheat(0.5);
        assert!(!layout.is_settled());
    }

    #[test]
    fn empty_particle_set_reports_settled_immediately() {
        let mut layout = ForceDirectedLayout::default();
        let edges: Vec<SimEdge> = Vec::new();
        let degree: Vec<u32> = Vec::new();
        let t = topo(0, &edges, &degree, Vec::new());
        let mut particles: Vec<Particle> = Vec::new();
        let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
        assert!(r.settled);
    }

    /// Wave G1 gate — the equivalence test for Fix 1: the tree-based
    /// collision path (used above `brute_force_threshold`) must produce
    /// the SAME resolution as the brute-force path (used below it) for a
    /// graph small enough to run both. Deliberately overlapping fixture
    /// (three loose clusters, radius large enough to overlap neighbors
    /// within and across clusters).
    #[test]
    fn tree_collision_resolution_matches_brute_force_on_a_fixture_with_deliberate_overlaps() {
        let mut particles = Vec::new();
        for k in 0..6 {
            let (cx, cy) = (k as f32 * 15.0, (k % 2) as f32 * 12.0);
            particles.push(Particle::at(cx, cy));
            particles.push(Particle::at(cx + 3.0, cy + 2.0));
            particles.push(Particle::at(cx - 2.0, cy + 4.0));
        }
        let n = particles.len();
        let radii = vec![6.0; n];
        let strength = 0.7;

        let mut brute = vec![(0f32, 0f32); n];
        apply_collision(&particles, &radii, strength, &mut brute);

        let qt = Quadtree::build(&particles);
        let mut tree = vec![(0f32, 0f32); n];
        qt.apply_collision(&particles, &radii, strength, &mut tree);

        for i in 0..n {
            let dx = (brute[i].0 - tree[i].0).abs();
            let dy = (brute[i].1 - tree[i].1).abs();
            assert!(
                dx < 1e-3 && dy < 1e-3,
                "particle {i}: brute={:?} tree={:?} (diff {dx}, {dy})",
                brute[i],
                tree[i]
            );
        }
    }

    /// A second, larger-N variant of the same equivalence proof, with a
    /// coincident stack thrown in (dist2<=1e-6 branch) — proves the tree
    /// path's coincidence nudge matches brute force too, not just the
    /// ordinary-overlap branch.
    #[test]
    fn tree_collision_resolution_matches_brute_force_including_an_exactly_coincident_stack() {
        let mut particles = vec![Particle::at(0.0, 0.0); 5];
        particles.extend((0..20).map(|i| {
            let a = i as f32 * 0.9;
            Particle::at(40.0 + a.cos() * 20.0, a.sin() * 20.0)
        }));
        let n = particles.len();
        let radii = vec![3.0; n];
        let strength = 0.5;

        let mut brute = vec![(0f32, 0f32); n];
        apply_collision(&particles, &radii, strength, &mut brute);

        let qt = Quadtree::build(&particles);
        let mut tree = vec![(0f32, 0f32); n];
        qt.apply_collision(&particles, &radii, strength, &mut tree);

        for i in 0..n {
            let dx = (brute[i].0 - tree[i].0).abs();
            let dy = (brute[i].1 - tree[i].1).abs();
            assert!(dx < 1e-3 && dy < 1e-3, "particle {i}: brute={:?} tree={:?}", brute[i], tree[i]);
        }
    }

    /// Wave G1 fix 1 gate: collision must no longer be silently disabled
    /// above `brute_force_threshold` — a graph above the (overridden, for
    /// test speed) threshold, entirely coincident, must separate instead
    /// of staying frozen.
    #[test]
    fn collision_stays_active_above_the_brute_force_threshold_and_separates_a_coincident_cluster() {
        let params = ForceParams { brute_force_threshold: 2, collision: true, seed_degenerate_positions: false, ..ForceParams::default() };
        let mut layout = ForceDirectedLayout::new(params);
        let n = 6;
        let mut particles = vec![Particle::at(0.0, 0.0); n]; // n=6 > threshold=2
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);

        for _ in 0..5 {
            layout.tick(&t, &mut particles, 1.0 / 60.0);
        }

        let spread = particles.iter().map(|p| p.x.abs() + p.y.abs()).fold(0.0f32, f32::max);
        assert!(spread > 0.5, "particles must separate above the brute-force threshold, spread={spread}");
    }

    /// Wave G1 fix 2 gate: an unseeded sim above 500 nodes (the crate's
    /// own default `brute_force_threshold`) must separate on the very
    /// first tick instead of freezing forever — the exact
    /// `vec![Particle::default(); n]` shape `engine.rs::GraphEngine::new`
    /// produces for a caller that never calls `seed_positions`.
    #[test]
    fn unseeded_sim_above_500_nodes_separates_instead_of_freezing_forever() {
        let n = 600;
        let mut particles = vec![Particle::default(); n];
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);
        let mut layout = ForceDirectedLayout::default();

        layout.tick(&t, &mut particles, 1.0 / 60.0);

        let (x0, y0) = (particles[0].x, particles[0].y);
        let moved = particles.iter().any(|p| (p.x - x0).abs() > 1e-3 || (p.y - y0).abs() > 1e-3);
        assert!(moved, "particles above the collision brute-force threshold must separate after seeding, not stay coincident");
    }

    /// Wave G1 fix 2 gate, the other direction: a caller who already
    /// seeded a non-degenerate start must see byte-identical behavior
    /// whether or not `seed_degenerate_positions` is enabled.
    #[test]
    fn a_pre_seeded_non_degenerate_start_is_unaffected_by_the_seeding_fallback() {
        let make_particles = || vec![Particle::at(-10.0, 0.0), Particle::at(10.0, 0.0), Particle::at(0.0, 15.0)];
        let degree = vec![0u32; 3];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        let mut particles_on = make_particles();
        let mut layout_on = ForceDirectedLayout::new(ForceParams { seed_degenerate_positions: true, ..ForceParams::default() });
        let mut particles_off = make_particles();
        let mut layout_off = ForceDirectedLayout::new(ForceParams { seed_degenerate_positions: false, ..ForceParams::default() });

        for _ in 0..10 {
            layout_on.tick(&t, &mut particles_on, 1.0 / 60.0);
            layout_off.tick(&t, &mut particles_off, 1.0 / 60.0);
        }
        assert_eq!(particles_on, particles_off, "a non-degenerate start must be byte-identical regardless of the seeding fallback");
    }

    /// Wave G1 fix 3/4 gate: [`positions_are_degenerate`]/
    /// [`seed_phyllotaxis_positions`] direct unit coverage — a spiral
    /// never repeats a position and is fully deterministic.
    #[test]
    fn phyllotaxis_seed_places_every_particle_at_a_distinct_position_deterministically() {
        let mut a = vec![Particle::default(); 40];
        let mut b = vec![Particle::default(); 40];
        assert!(positions_are_degenerate(&a));
        seed_phyllotaxis_positions(&mut a, (0.0, 0.0));
        seed_phyllotaxis_positions(&mut b, (0.0, 0.0));
        assert_eq!(a, b, "phyllotaxis seeding must be deterministic — no RNG");
        assert!(!positions_are_degenerate(&a), "a real spiral must not be degenerate itself");
        for i in 0..a.len() {
            for j in (i + 1)..a.len() {
                let dx = a[i].x - a[j].x;
                let dy = a[i].y - a[j].y;
                assert!(dx * dx + dy * dy > 1e-6, "indices {i} and {j} landed on the same point");
            }
        }
    }

    /// Wave G2 configurability gate: the new public params default to
    /// EXACTLY the values the private constants they replace used to
    /// hardcode.
    #[test]
    fn new_public_step_params_default_to_the_prior_hardcoded_constants() {
        let p = ForceParams::default();
        assert_eq!(p.normalize_fps, 60.0);
        assert_eq!(p.max_step, 4.0);
        assert_eq!(p.settle_displacement_eps, 0.05);
        assert!(p.seed_degenerate_positions);
    }
}
