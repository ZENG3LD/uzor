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
    /// Repulsion/link-force softening floor — avoids a divide-by-zero
    /// singularity for coincident/near-coincident particles (Wave G2b
    /// configurability — was the private [`barnes_hut::MIN_DIST2`]
    /// constant, read directly by every call site below).
    pub min_dist2: f32,
    /// Quadtree coincident-point merge threshold — two points closer
    /// (squared) than this can't be meaningfully separated by
    /// subdividing (Wave G2b configurability — was the private
    /// [`barnes_hut::MIN_SPLIT_DIST2`] constant).
    pub min_split_dist2: f32,
    /// Quadtree subdivision floor — a cell this small is never split
    /// further (Wave G2b configurability — was the private
    /// [`barnes_hut::MIN_CELL_SIZE`] constant).
    pub min_cell_size: f32,
    /// Wave G3 item 6 fix — hard cap on a single particle's own per-tick
    /// displacement magnitude (world units), enforced by clamping
    /// velocity BEFORE integrating position (so a clamped tick's own
    /// velocity stays consistent going into the NEXT tick too, not just
    /// this one). [`ForceParams::min_dist2`] softening already bounds a
    /// SINGLE pair's own repulsion contribution — nothing previously
    /// bounded the SUM over many simultaneously-close neighbors (layout
    /// audit A2): a locally dense cluster (many nodes within a few world
    /// units of each other — a real shape for an externally-imported
    /// graph with near-identical starting coordinates) could still
    /// produce a displacement of thousands of world units in a single
    /// tick, with every individual term staying finite the whole time
    /// (no NaN) — "flung off-screen," not a crash.
    ///
    /// Default ([`DEFAULT_MAX_DISPLACEMENT_PER_TICK`]) chosen by
    /// MEASURING, not guessing — see this crate's own
    /// `tests::the_real_534_node_clusters_demo_fixture_never_comes_close_
    /// to_the_default_displacement_clamp` for the harness these numbers
    /// come from: the crate's own real 534-node `clusters` demo fixture
    /// (the one Wave G1's collision fix targeted) peaks at **~624** world
    /// units/tick under default params; the SAME fixture with every
    /// particle deliberately re-collapsed onto the exact origin (the
    /// adversarial "external caller pre-seeded everything coincident"
    /// case this item exists for) peaks at **~2475**. `20_000.0` is
    /// ~8x the measured adversarial worst case and ~32x the real-fixture
    /// worst case — comfortably above both, so no well-behaved
    /// simulation this crate has ever produced reaches it (today's
    /// output is preserved, doctrine), while a genuinely pathological
    /// input still lands bounded in the low tens of thousands of world
    /// units instead of unbounded.
    pub max_displacement_per_tick: f32,
    /// Wave G4 fix — [`crate::graph::SimEdge::weight`] is read as
    /// meaningful by the RENDER layer (`render3d.rs`'s
    /// `edge_width_scale`: a heavier edge draws thicker) but was silently
    /// ignored by both force layouts — `link_distance`/`link_strength`
    /// applied identically to every edge regardless of its own weight, a
    /// render/simulation inconsistency (the same field means "thicker
    /// line" visually and "nothing at all" physically). When `true`, an
    /// edge's own `link_strength` contribution is scaled by
    /// `weight.max(0.0)` (weight `1.0` — the implicit weight every
    /// pre-existing edge carries — is the identity multiplier, so a
    /// uniformly-weighted graph is byte-identical whether this is `true`
    /// or `false`). Default `true` (graph-strengthening arc, owner-
    /// approved flip, 2026-07-26 — was `false` at Wave G4 introduction):
    /// the render layer ALREADY treats `edge.weight` as meaningful
    /// (`render3d.rs`'s `edge_width_scale`), so leaving this `false`
    /// meant the picture claimed a relationship the simulation didn't
    /// model — the image lied about the model. A caller can still pass
    /// `weighted_links: false` explicitly to recover the old discard-the-
    /// weight behavior; see `uzor-graph/CLAUDE.md`'s Wave G4 entry for the
    /// original before/after description and the flip's own report.
    pub weighted_links: bool,
    /// Wave G4 fix — [`crate::graph::SimTopology::degree`] is computed
    /// (`Graph::degree`) but was never read by any layout; tree insertion
    /// always used a hardcoded mass of `1.0`, so a high-degree hub
    /// repelled exactly as weakly as a degree-0 leaf. Every mature
    /// force-layout (d3-force's `forceManyBody().strength(d => k * (1 +
    /// d.degree))`, ForceAtlas2's degree-scaled repulsion) scales
    /// repulsion by degree instead. When `true`, particle `i`'s repulsive
    /// mass/charge becomes `1.0 + degree[i]` (both the Barnes-Hut tree
    /// path and the brute-force path below `brute_force_threshold` — see
    /// [`degree_masses`]) instead of the uniform `1.0` every particle
    /// used before this field existed. Default `false` — a real,
    /// visible layout-output change for any graph with degree variance,
    /// so per doctrine it ships opt-in; see `uzor-graph/CLAUDE.md`'s Wave
    /// G4 entry for the before/after description reported to the owner.
    pub mass_from_degree: bool,
}

/// Default for [`ForceParams::max_displacement_per_tick`] — see that
/// field's own doc comment for the measured numbers this derives from
/// (Wave G3 item 6 fix).
const DEFAULT_MAX_DISPLACEMENT_PER_TICK: f32 = 20_000.0;

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
            min_dist2: barnes_hut::MIN_DIST2,
            min_split_dist2: barnes_hut::MIN_SPLIT_DIST2,
            min_cell_size: barnes_hut::MIN_CELL_SIZE,
            max_displacement_per_tick: DEFAULT_MAX_DISPLACEMENT_PER_TICK,
            weighted_links: true,
            mass_from_degree: false,
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
/// ratio) — same value as [`super::radial_3d::RadialParams3D::golden_angle`]'s
/// own default (private there, re-declared here; d3-force's own
/// `initialAngle = PI * (3 - sqrt(5))`, the identical irrational constant
/// under a different derivation). See [`seed_phyllotaxis_positions`].
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

/// Per-particle repulsive "mass"/charge derived from graph degree — Wave
/// G4 fix, used only when [`ForceParams::mass_from_degree`] is enabled.
/// A degree-0 leaf keeps the pre-existing uniform mass of `1.0`; a
/// higher-degree hub gets proportionally MORE repulsive charge. `n` is
/// `particles.len()`, not `degree.len()` — a caller's `degree` slice
/// shorter than the particle count (shouldn't happen for a well-formed
/// [`SimTopology`], but this stays a defensive lookup rather than a
/// panic) falls back to `0` for any missing index.
fn degree_masses(degree: &[u32], n: usize) -> Vec<f32> {
    (0..n).map(|i| 1.0 + degree.get(i).copied().unwrap_or(0) as f32).collect()
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

        // Wave G4 fix: degree-scaled repulsive mass, opt-in via
        // `mass_from_degree` — see that field's own doc comment. Computed
        // once and shared by whichever repulsion path actually runs below
        // so a graph's own physics doesn't discontinuously change
        // depending on which side of `brute_force_threshold` it falls on.
        let masses = self.params.mass_from_degree.then(|| degree_masses(topo.degree, n));

        // Wave G1 fix: the quadtree built for the above-threshold
        // repulsion path is KEPT (not dropped) so collision can reuse it
        // below instead of either being disabled or building a second
        // tree.
        let qt = if n > self.params.brute_force_threshold {
            let qt = Quadtree::build_weighted(particles, masses.as_deref(), self.params.min_split_dist2, self.params.min_cell_size);
            qt.accumulate_forces(particles, self.params.theta, self.params.charge_strength, self.params.min_dist2, &mut force);
            Some(qt)
        } else {
            barnes_hut::apply_repulsion_brute_force(particles, self.params.charge_strength, self.params.min_dist2, masses.as_deref(), &mut force);
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
            // Wave G4 fix: `SimEdge::weight` modulates the link spring's
            // own strength, opt-in via `weighted_links` — see that
            // field's own doc comment. Weight `1.0` (the implicit weight
            // of any pre-existing edge) is the identity multiplier.
            let link_strength = if self.params.weighted_links {
                self.params.link_strength * e.weight.max(0.0)
            } else {
                self.params.link_strength
            };
            let diff = (dist - ideal) / dist * link_strength;
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
            // Wave G3 item 6 fix: clamp the VELOCITY (not just the
            // resulting displacement) so a clamped tick's own speed
            // stays consistent going into the next tick too — see
            // `ForceParams::max_displacement_per_tick`'s own doc
            // comment. `step > 0` is required for `max_speed` to be
            // meaningful; a zero/negative step already produces zero
            // displacement regardless, so skipping the clamp there is a
            // no-op either way.
            if step > 1e-9 {
                let max_speed = self.params.max_displacement_per_tick / step;
                let speed2 = particles[i].vx * particles[i].vx + particles[i].vy * particles[i].vy;
                if speed2 > max_speed * max_speed {
                    let scale = max_speed / speed2.sqrt();
                    particles[i].vx *= scale;
                    particles[i].vy *= scale;
                }
            }
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
    use crate::graph::{NodeIndex, SimEdge};
    use std::f32::consts::TAU;

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

        let qt = Quadtree::build(&particles, barnes_hut::MIN_SPLIT_DIST2, barnes_hut::MIN_CELL_SIZE);
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

        let qt = Quadtree::build(&particles, barnes_hut::MIN_SPLIT_DIST2, barnes_hut::MIN_CELL_SIZE);
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
        assert_eq!(p.min_dist2, barnes_hut::MIN_DIST2);
        assert_eq!(p.min_split_dist2, barnes_hut::MIN_SPLIT_DIST2);
        assert_eq!(p.min_cell_size, barnes_hut::MIN_CELL_SIZE);
    }

    /// Wave G2b configurability gate: a caller-tuned `min_dist2` must
    /// actually change the softening applied at NEAR (not exact)
    /// coincidence — proof, not just a default-equality assertion. Two
    /// particles a tiny, nonzero distance apart have a real (nonzero)
    /// direction, so the softening floor `d2 = max(dist2, min_dist2)`
    /// governs the force MAGNITUDE (`f = strength / d2`) without hitting
    /// the exact-zero-direction singularity a truly coincident pair would.
    #[test]
    fn a_larger_min_dist2_caps_repulsion_between_near_coincident_particles_more_aggressively() {
        let degree = vec![0u32; 2];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(2, &edges, &degree, vec![0.001; 2]);

        let mut default_particles = vec![Particle::at(0.0, 0.0), Particle::at(1e-4, 0.0)];
        let mut default_layout =
            ForceDirectedLayout::new(ForceParams { seed_degenerate_positions: false, collision: false, ..ForceParams::default() });
        default_layout.tick(&t, &mut default_particles, 1.0 / 60.0);

        let mut softened_particles = vec![Particle::at(0.0, 0.0), Particle::at(1e-4, 0.0)];
        let mut softened_layout = ForceDirectedLayout::new(ForceParams {
            seed_degenerate_positions: false,
            collision: false,
            min_dist2: barnes_hut::MIN_DIST2 * 100.0,
            ..ForceParams::default()
        });
        softened_layout.tick(&t, &mut softened_particles, 1.0 / 60.0);

        let default_speed = (default_particles[0].vx.powi(2) + default_particles[0].vy.powi(2)).sqrt();
        let softened_speed = (softened_particles[0].vx.powi(2) + softened_particles[0].vy.powi(2)).sqrt();
        assert!(
            softened_speed < default_speed,
            "a larger min_dist2 softening floor must cap the resulting velocity lower: default={default_speed} softened={softened_speed}"
        );
    }

    /// Deterministic splitmix64-style LCG, matching every other
    /// measurement/test fixture in this file's own convention.
    struct MeasureRng(u64);
    impl MeasureRng {
        fn next_f32(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 33) as u32 % 1_000_000) as f32 / 1_000_000.0
        }
    }

    /// Faithful reproduction of `uzor-examples/src/l4/force_graph_demo.rs`'s
    /// own `build_clusters_graph` — the 534-node fixture (6 clusters × 88
    /// members + 1 hub each) G1a's own collision fix targeted: golden-
    /// angle-spread cluster centers (radius 420), per-member jitter up to
    /// radius 140 around its own center, `EDGES_PER_NODE = 3` random
    /// intra-cluster edges, a hub wired to every 6th member, hubs chained
    /// in a ring. Reproduced here (not imported — `uzor-examples` is a
    /// separate crate this crate doesn't depend on) specifically so this
    /// measurement instruments the REAL shape, not a synthetic guess.
    fn clusters_534_fixture() -> (Vec<Particle>, Vec<SimEdge>) {
        const NUM_CLUSTERS: usize = 6;
        const CLUSTER_SIZE: usize = 88;
        const EDGES_PER_NODE: usize = 3;

        let mut positions: Vec<(f32, f32)> = Vec::new();
        let mut cluster_members: Vec<Vec<usize>> = vec![Vec::new(); NUM_CLUSTERS];
        let mut hubs = Vec::with_capacity(NUM_CLUSTERS);
        let mut edges = Vec::new();

        for cluster in 0..NUM_CLUSTERS {
            let angle = cluster as f32 * 2.399_963;
            let cx = angle.cos() * 420.0;
            let cy = angle.sin() * 420.0;
            for member in 0..CLUSTER_SIZE {
                let mut rng = MeasureRng((cluster as u64) << 32 | member as u64);
                let jitter_r = rng.next_f32() * 140.0;
                let jitter_a = rng.next_f32() * TAU;
                let x = cx + jitter_a.cos() * jitter_r;
                let y = cy + jitter_a.sin() * jitter_r;
                let idx = positions.len();
                positions.push((x, y));
                cluster_members[cluster].push(idx);
            }
            let hub_idx = positions.len();
            positions.push((cx, cy));
            hubs.push(hub_idx);
        }

        for cluster in 0..NUM_CLUSTERS {
            let members = &cluster_members[cluster];
            for (i, &node) in members.iter().enumerate() {
                let mut rng = MeasureRng(0xC0FF_EE00 ^ ((cluster as u64) << 20) ^ i as u64);
                for _ in 0..EDGES_PER_NODE {
                    let j = (rng.next_f32() * members.len() as f32) as usize % members.len().max(1);
                    if j != i {
                        edges.push(SimEdge { from: NodeIndex(node as u32), to: NodeIndex(members[j] as u32), weight: 1.0 });
                    }
                }
            }
            for (i, &node) in members.iter().enumerate() {
                if i % 6 == 0 {
                    edges.push(SimEdge { from: NodeIndex(hubs[cluster] as u32), to: NodeIndex(node as u32), weight: 1.0 });
                }
            }
        }
        for cluster in 0..NUM_CLUSTERS {
            let next = (cluster + 1) % NUM_CLUSTERS;
            edges.push(SimEdge { from: NodeIndex(hubs[cluster] as u32), to: NodeIndex(hubs[next] as u32), weight: 0.6 });
        }

        let particles = positions.into_iter().map(|(x, y)| Particle::at(x, y)).collect();
        (particles, edges)
    }

    /// Wave G3 item 6 gate: the default clamp must be exactly the
    /// measured-and-documented value — see
    /// [`ForceParams::max_displacement_per_tick`]'s own doc comment for
    /// the full measurement writeup.
    #[test]
    fn max_displacement_per_tick_default_matches_the_measured_and_documented_value() {
        assert_eq!(ForceParams::default().max_displacement_per_tick, 20_000.0);
    }

    /// Wave G3 item 6 — MEASURED, not guessed, proof that today's real
    /// output is preserved: the crate's own real 534-node `clusters`
    /// demo fixture (the exact one Wave G1's collision fix targeted, real
    /// edges included so spring forces contribute too) never gets
    /// remotely close to the default clamp under default params — every
    /// tick's own `max_displacement` stays two full orders of magnitude
    /// below it. This is the actual harness
    /// [`ForceParams::max_displacement_per_tick`]'s own doc comment
    /// reports numbers from (measured max ~624 on a full settle run).
    #[test]
    fn the_real_534_node_clusters_demo_fixture_never_comes_close_to_the_default_displacement_clamp() {
        let (mut particles, edges) = clusters_534_fixture();
        let n = particles.len();
        assert_eq!(n, 534, "sanity: the reproduced fixture must match the real demo's own node count");
        let degree = vec![0u32; n];
        let radii = vec![4.0; n];
        let t = topo(n, &edges, &degree, radii);
        let mut layout = ForceDirectedLayout::default();

        let mut max = 0f32;
        for _ in 0..600 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            max = max.max(r.max_displacement);
            if r.settled {
                break;
            }
        }
        assert!(
            max < 5_000.0,
            "the real demo fixture's own per-tick displacement must stay comfortably below the default 20_000.0 clamp: measured max = {max}"
        );
    }

    /// Wave G3 item 6 — the deliberately ADVERSARIAL worst case this
    /// item exists for: the SAME 534-node fixture, but every particle
    /// re-collapsed onto the exact origin before the sim ever runs (the
    /// "external caller pre-seeded everything coincident" shape —
    /// `seed_degenerate_positions` is disabled deliberately, since that
    /// safety net only fires when NOTHING has positioned the particles
    /// at all, and this fixture's whole point is proving what happens
    /// WITHOUT it). Even this adversarial case still settles well below
    /// the DEFAULT clamp (measured max ~2475) — proving the default is
    /// chosen with real headroom above every fixture this crate can
    /// currently produce, not just the well-behaved one above.
    #[test]
    fn the_534_node_fixture_collapsed_to_the_origin_stays_below_the_default_clamp_too() {
        let (mut particles, edges) = clusters_534_fixture();
        for p in &mut particles {
            p.x = 0.0;
            p.y = 0.0;
        }
        let n = particles.len();
        let degree = vec![0u32; n];
        let radii = vec![4.0; n];
        let t = topo(n, &edges, &degree, radii);
        let mut layout = ForceDirectedLayout::new(ForceParams { seed_degenerate_positions: false, ..ForceParams::default() });

        let mut max = 0f32;
        for _ in 0..600 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            max = max.max(r.max_displacement);
            if r.settled {
                break;
            }
        }
        assert!(
            max < ForceParams::default().max_displacement_per_tick,
            "even the adversarial collapsed-to-origin fixture must stay below the default clamp: measured max = {max}"
        );
        for p in &particles {
            assert!(p.x.is_finite() && p.y.is_finite(), "no coordinate may ever go non-finite, clamped or not");
        }
    }

    /// Wave G3 item 6 gate: proves the clamp MECHANISM actually engages
    /// and bounds output, using a caller-overridden LOW value (the
    /// default is deliberately never reached by any fixture above, so a
    /// direct override is the only way to prove the clamp isn't dead
    /// code) — a 600-particle fully-degenerate start (every layout's own
    /// worst-case unseeded shape) with the phyllotaxis seed DISABLED,
    /// so the whole burst of separating force actually has to run
    /// through the clamp instead of starting from an already-spread
    /// seed.
    #[test]
    fn max_displacement_per_tick_actually_bounds_output_when_overridden_low() {
        let n = 600;
        let mut particles = vec![Particle::default(); n];
        let degree = vec![0u32; n];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(n, &edges, &degree, vec![4.0; n]);
        let mut layout =
            ForceDirectedLayout::new(ForceParams { seed_degenerate_positions: false, max_displacement_per_tick: 10.0, ..ForceParams::default() });

        for _ in 0..30 {
            let r = layout.tick(&t, &mut particles, 1.0 / 60.0);
            assert!(
                r.max_displacement <= 10.0 + 1e-3,
                "an overridden 10.0 clamp must never let any tick's own max_displacement exceed it: got {}",
                r.max_displacement
            );
        }
        for p in &particles {
            assert!(p.x.is_finite() && p.y.is_finite(), "every coordinate must stay finite under the clamp");
        }
    }

    /// Wave G3 item 6 gate: a caller who never sets an unusually small
    /// `min_dist2` and stays well under the clamp must see BYTE-IDENTICAL
    /// behavior whether the clamp is at its default or effectively
    /// disabled (`f32::INFINITY`) — the clamp must never perturb an
    /// ordinary, non-exploding simulation.
    #[test]
    fn an_ordinary_non_exploding_simulation_is_byte_identical_with_the_clamp_effectively_disabled() {
        let make_particles = || vec![Particle::at(-10.0, 0.0), Particle::at(10.0, 0.0), Particle::at(0.0, 15.0)];
        let degree = vec![0u32; 3];
        let edges: Vec<SimEdge> = Vec::new();
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        let mut particles_default = make_particles();
        let mut layout_default = ForceDirectedLayout::default();
        let mut particles_disabled = make_particles();
        let mut layout_disabled =
            ForceDirectedLayout::new(ForceParams { max_displacement_per_tick: f32::INFINITY, ..ForceParams::default() });

        for _ in 0..60 {
            layout_default.tick(&t, &mut particles_default, 1.0 / 60.0);
            layout_disabled.tick(&t, &mut particles_disabled, 1.0 / 60.0);
        }
        assert_eq!(particles_default, particles_disabled, "the default clamp must be a complete no-op for an ordinary simulation");
    }

    // ── Wave G4 item 2 — `weighted_links` (flipped to default `true`,
    // graph-strengthening arc, owner-approved, 2026-07-26) ──

    /// Graph-strengthening arc gate: `weighted_links` now defaults to
    /// `true` — `ForceParams::default()`, untouched, must ALREADY make a
    /// heavier edge pull harder than a lighter one, with no caller opt-in
    /// required. Closes the render/simulation inconsistency the layout
    /// audit flagged (`render3d.rs`'s `edge_width_scale` already treated
    /// `edge.weight` as meaningful; the layout layer silently discarded
    /// it at Wave G4 introduction). Same isolated single-edge,
    /// no-repulsion/no-center fixture as the explicit-`true` test below,
    /// but built from a bare `ForceParams::default()` plus only the
    /// force-isolation overrides — `weighted_links` itself is never
    /// touched here, proving the DEFAULT itself does the work.
    #[test]
    fn weighted_links_defaults_to_true_and_a_default_layout_already_scales_link_strength_by_edge_weight() {
        assert!(ForceParams::default().weighted_links, "weighted_links must default to true — graph-strengthening arc flip");

        let degree = vec![1u32; 2];
        let params = ForceParams { charge_strength: 0.0, center_strength: 0.0, collision: false, seed_degenerate_positions: false, ..ForceParams::default() };

        let mut particles_light = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_light = ForceDirectedLayout::new(params);
        let edges_light = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 1.0 }];
        let t_light = topo(2, &edges_light, &degree, vec![4.0; 2]);
        layout_light.tick(&t_light, &mut particles_light, 1.0 / 60.0);

        let mut particles_heavy = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_heavy = ForceDirectedLayout::new(params);
        let edges_heavy = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 5.0 }];
        let t_heavy = topo(2, &edges_heavy, &degree, vec![4.0; 2]);
        layout_heavy.tick(&t_heavy, &mut particles_heavy, 1.0 / 60.0);

        let speed_light = (particles_light[0].vx.powi(2) + particles_light[0].vy.powi(2)).sqrt();
        let speed_heavy = (particles_heavy[0].vx.powi(2) + particles_heavy[0].vy.powi(2)).sqrt();
        assert!(speed_light > 1e-6, "sanity: the light edge must produce SOME motion to compare against");
        assert!(
            (speed_heavy - 5.0 * speed_light).abs() < speed_light * 0.01,
            "with weighted_links at its default (true), a 5x-heavier edge must pull ~5x harder: light={speed_light} heavy={speed_heavy}"
        );
    }

    /// Wave G4 gate, preserved: `weighted_links: false` (an explicit
    /// caller opt-OUT, now that the default flipped to `true`) must still
    /// make edge weight variance a complete no-op — the escape hatch for
    /// a caller who wants the pre-flip discard-the-weight behavior.
    #[test]
    fn weighted_links_explicitly_disabled_ignores_edge_weight_variance() {
        let degree = vec![1u32; 2];
        let params = ForceParams {
            charge_strength: 0.0,
            center_strength: 0.0,
            collision: false,
            seed_degenerate_positions: false,
            weighted_links: false,
            ..ForceParams::default()
        };

        let mut particles_light = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_light = ForceDirectedLayout::new(params);
        let edges_light = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 1.0 }];
        let t_light = topo(2, &edges_light, &degree, vec![4.0; 2]);

        let mut particles_heavy = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_heavy = ForceDirectedLayout::new(params);
        let edges_heavy = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 5.0 }];
        let t_heavy = topo(2, &edges_heavy, &degree, vec![4.0; 2]);

        for _ in 0..10 {
            layout_light.tick(&t_light, &mut particles_light, 1.0 / 60.0);
            layout_heavy.tick(&t_heavy, &mut particles_heavy, 1.0 / 60.0);
        }
        assert_eq!(particles_light, particles_heavy, "weighted_links: false must still fully ignore edge weight variance");
    }

    /// Wave G4 gate: `weighted_links: true` actually modulates the link
    /// spring's own strength by `SimEdge::weight` — proven by an isolated
    /// single-edge, no-repulsion/no-center fixture where the link spring
    /// is the ONLY force acting, so the resulting velocity is an exact
    /// linear multiple of the edge's own weight.
    #[test]
    fn weighted_links_true_scales_the_link_forces_strength_by_edge_weight() {
        let degree = vec![1u32; 2];
        let params = ForceParams {
            charge_strength: 0.0,
            center_strength: 0.0,
            collision: false,
            seed_degenerate_positions: false,
            weighted_links: true,
            ..ForceParams::default()
        };

        let mut particles_light = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_light = ForceDirectedLayout::new(params);
        let edges_light = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 1.0 }];
        let t_light = topo(2, &edges_light, &degree, vec![4.0; 2]);
        layout_light.tick(&t_light, &mut particles_light, 1.0 / 60.0);

        let mut particles_heavy = vec![Particle::at(0.0, 0.0), Particle::at(100.0, 0.0)];
        let mut layout_heavy = ForceDirectedLayout::new(params);
        let edges_heavy = vec![SimEdge { from: NodeIndex(0), to: NodeIndex(1), weight: 5.0 }];
        let t_heavy = topo(2, &edges_heavy, &degree, vec![4.0; 2]);
        layout_heavy.tick(&t_heavy, &mut particles_heavy, 1.0 / 60.0);

        let speed_light = (particles_light[0].vx.powi(2) + particles_light[0].vy.powi(2)).sqrt();
        let speed_heavy = (particles_heavy[0].vx.powi(2) + particles_heavy[0].vy.powi(2)).sqrt();
        assert!(speed_light > 1e-6, "sanity: the light edge must produce SOME motion to compare against");
        assert!(
            (speed_heavy - 5.0 * speed_light).abs() < speed_light * 0.01,
            "a 5x-heavier edge weight must pull almost exactly 5x harder: light={speed_light} heavy={speed_heavy}"
        );
    }

    // ── Wave G4 item 3 — `mass_from_degree` ──

    /// [`degree_masses`] direct gate: a degree-0 leaf keeps mass `1.0`
    /// (the pre-existing uniform default); a degree-N node's mass is
    /// `1.0 + N`.
    #[test]
    fn degree_masses_gives_a_higher_degree_particle_more_repulsive_mass() {
        let degree = vec![10u32, 0, 3];
        let masses = degree_masses(&degree, 3);
        assert_eq!(masses, vec![11.0, 1.0, 4.0]);
    }

    /// Wave G4 gate: `mass_from_degree` defaults to `false` — degree
    /// variance must have zero effect on the sim (preserves today's
    /// behavior, doctrine 1), through the BRUTE-FORCE repulsion path
    /// (`n` below the default `brute_force_threshold`).
    #[test]
    fn mass_from_degree_default_false_ignores_degree_variance() {
        let edges: Vec<SimEdge> = Vec::new();
        let degree_uniform = vec![0u32, 0, 0];
        let degree_skewed = vec![50u32, 0, 0];
        let params = ForceParams { collision: false, seed_degenerate_positions: false, ..ForceParams::default() };
        let make_particles = || vec![Particle::at(-100.0, 0.0), Particle::at(100.0, 0.0), Particle::at(0.0, 0.0)];

        let mut particles_uniform = make_particles();
        let mut layout_uniform = ForceDirectedLayout::new(params);
        let t_uniform = topo(3, &edges, &degree_uniform, vec![4.0; 3]);

        let mut particles_skewed = make_particles();
        let mut layout_skewed = ForceDirectedLayout::new(params);
        let t_skewed = topo(3, &edges, &degree_skewed, vec![4.0; 3]);

        layout_uniform.tick(&t_uniform, &mut particles_uniform, 1.0 / 60.0);
        layout_skewed.tick(&t_skewed, &mut particles_skewed, 1.0 / 60.0);

        assert_eq!(particles_uniform, particles_skewed, "mass_from_degree defaults to false — degree variance must have zero effect on the sim");
    }

    /// Same gate as above, through the BARNES-HUT TREE repulsion path
    /// (`brute_force_threshold` overridden below `n`) — degree-scaled
    /// mass must stay a no-op on that path too when the flag is off,
    /// since [`ForceDirectedLayout::tick`] shares one `masses` value
    /// between both repulsion paths.
    #[test]
    fn mass_from_degree_default_false_ignores_degree_variance_through_the_tree_path_too() {
        let edges: Vec<SimEdge> = Vec::new();
        let degree_uniform = vec![0u32, 0, 0];
        let degree_skewed = vec![50u32, 0, 0];
        let params = ForceParams { collision: false, seed_degenerate_positions: false, brute_force_threshold: 2, ..ForceParams::default() };
        let make_particles = || vec![Particle::at(-100.0, 0.0), Particle::at(100.0, 0.0), Particle::at(0.0, 0.0)];

        let mut particles_uniform = make_particles();
        let mut layout_uniform = ForceDirectedLayout::new(params);
        let t_uniform = topo(3, &edges, &degree_uniform, vec![4.0; 3]);

        let mut particles_skewed = make_particles();
        let mut layout_skewed = ForceDirectedLayout::new(params);
        let t_skewed = topo(3, &edges, &degree_skewed, vec![4.0; 3]);

        layout_uniform.tick(&t_uniform, &mut particles_uniform, 1.0 / 60.0);
        layout_skewed.tick(&t_skewed, &mut particles_skewed, 1.0 / 60.0);

        assert_eq!(particles_uniform, particles_skewed, "mass_from_degree=false must be a no-op via the tree path too");
    }

    /// Wave G4 gate: `mass_from_degree: true` actually makes a
    /// higher-degree hub repel more strongly than a same-distance leaf —
    /// a hub at `(-100, 0)` (degree 50) and a leaf at `(100, 0)` (degree
    /// 0) are EQUIDISTANT from a probe at the origin; with uniform mass
    /// their repulsion on the probe cancels exactly (symmetric
    /// magnitude, opposite direction). A nonzero, positive
    /// (away-from-the-heavier-hub) probe velocity after one tick proves
    /// the hub's higher degree genuinely repels harder — brute-force
    /// path (`n` below the default threshold).
    #[test]
    fn mass_from_degree_true_makes_a_higher_degree_hub_repel_a_probe_more_strongly_than_a_leaf() {
        let edges: Vec<SimEdge> = Vec::new();
        let degree = vec![50u32, 0, 0];
        let params =
            ForceParams { collision: false, seed_degenerate_positions: false, mass_from_degree: true, ..ForceParams::default() };
        let mut particles = vec![Particle::at(-100.0, 0.0), Particle::at(100.0, 0.0), Particle::at(0.0, 0.0)];
        let mut layout = ForceDirectedLayout::new(params);
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        layout.tick(&t, &mut particles, 1.0 / 60.0);

        assert!(particles[2].vx > 0.0, "the probe must be pushed away from the higher-mass hub, not stay put: vx={}", particles[2].vx);
    }

    /// Same gate as above, through the BARNES-HUT TREE repulsion path.
    #[test]
    fn mass_from_degree_true_produces_the_same_asymmetric_push_via_the_tree_path_above_the_threshold() {
        let edges: Vec<SimEdge> = Vec::new();
        let degree = vec![50u32, 0, 0];
        let params = ForceParams {
            collision: false,
            seed_degenerate_positions: false,
            mass_from_degree: true,
            brute_force_threshold: 2,
            ..ForceParams::default()
        };
        let mut particles = vec![Particle::at(-100.0, 0.0), Particle::at(100.0, 0.0), Particle::at(0.0, 0.0)];
        let mut layout = ForceDirectedLayout::new(params);
        let t = topo(3, &edges, &degree, vec![4.0; 3]);

        layout.tick(&t, &mut particles, 1.0 / 60.0);

        assert!(
            particles[2].vx > 0.0,
            "the tree path must apply the same degree-scaled repulsion as brute force: vx={}",
            particles[2].vx
        );
    }
}
