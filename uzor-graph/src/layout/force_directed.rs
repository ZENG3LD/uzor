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
    /// Enable pairwise collision resolution (O(n²) — see
    /// `brute_force_threshold` gate in [`ForceDirectedLayout::tick`]).
    pub collision: bool,
    pub collision_strength: f32,
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
        }
    }
}

/// Baseline frame rate the `step` normalization assumes — `dt == 1/60`
/// behaves exactly like one classic d3-force tick.
const NORMALIZE_FPS: f32 = 60.0;
/// Caps a single `tick()` call's effective step count so a stalled
/// frame (window drag, alt-tab) can't blow the sim up.
const MAX_STEP: f32 = 4.0;
/// Below this per-tick max displacement, treat the sim as settled even
/// if `alpha` hasn't formally bottomed out yet.
const SETTLE_DISPLACEMENT_EPS: f32 = 0.05;

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

fn apply_collision(particles: &[Particle], radii: &[f32], strength: f32, force: &mut [(f32, f32)]) {
    let n = particles.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let dx = particles[j].x - particles[i].x;
            let dy = particles[j].y - particles[i].y;
            let dist2 = dx * dx + dy * dy;
            let min_dist = radii.get(i).copied().unwrap_or(1.0) + radii.get(j).copied().unwrap_or(1.0);
            if dist2 <= 1e-6 {
                // Coincident positions — deterministic nudge so they
                // don't stay locked together forever.
                force[i].0 -= 0.5;
                force[j].0 += 0.5;
                continue;
            }
            if dist2 < min_dist * min_dist {
                let dist = dist2.sqrt();
                let overlap = (min_dist - dist) * strength;
                let nx = dx / dist;
                let ny = dy / dist;
                force[i].0 -= nx * overlap * 0.5;
                force[i].1 -= ny * overlap * 0.5;
                force[j].0 += nx * overlap * 0.5;
                force[j].1 += ny * overlap * 0.5;
            }
        }
    }
}

impl Layout for ForceDirectedLayout {
    fn tick(&mut self, topo: &SimTopology<'_>, particles: &mut [Particle], dt: f32) -> LayoutTickResult {
        let n = particles.len();
        if n == 0 {
            return LayoutTickResult { alpha: self.alpha, max_displacement: 0.0, settled: true };
        }

        let mut force = vec![(0f32, 0f32); n];

        if n > self.params.brute_force_threshold {
            let qt = Quadtree::build(particles);
            qt.accumulate_forces(particles, self.params.theta, self.params.charge_strength, &mut force);
        } else {
            barnes_hut::apply_repulsion_brute_force(particles, self.params.charge_strength, &mut force);
        }

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

        if self.params.collision && n <= self.params.brute_force_threshold {
            apply_collision(particles, &topo.radii, self.params.collision_strength, &mut force);
        }

        // Framerate-independent step: `dt == 1/60` is exactly one
        // classic d3-force tick; longer/shorter frames scale decay and
        // displacement by the equivalent fractional tick count instead
        // of by raw seconds (raw-seconds scaling would need force
        // constants re-tuned per target frame rate).
        let step = (dt * NORMALIZE_FPS).clamp(0.0, MAX_STEP);
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

        let settled = self.alpha < self.params.alpha_min || max_disp < SETTLE_DISPLACEMENT_EPS;
        LayoutTickResult { alpha: self.alpha, max_displacement: max_disp, settled }
    }

    fn reheat(&mut self, alpha: f32) {
        self.alpha = self.alpha.max(alpha.clamp(0.0, 1.0));
    }

    fn is_settled(&self) -> bool {
        self.alpha < self.params.alpha_min
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
}
