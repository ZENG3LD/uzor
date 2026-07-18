//! Simulated point-mass state — kept in a separate SoA-style array from
//! node payload so the physics loop never touches app data every tick.
//! Mirrors d3-force's own `{x, y, vx, vy, fx, fy}` node fields (see the
//! engine design doc §2.3/§3.1).
//!
//! `z`/`vz`/`fz` (W3D arc plan §1.1) are additive 3D fields shared by
//! every `Particle` regardless of which [`crate::layout::Layout`]
//! consumes it — 2D layouts never read or write them (they stay `0.0`
//! forever via `#[derive(Default)]`), so 2D behavior is byte-identical.
//! See `docs/uzor-engines/uzor_graph_3d_arc_plan.md` §1.1 for why this is
//! an in-place extension rather than a parallel `Particle3` type.

/// One simulated point mass.
///
/// `fx`/`fy`/`fz` hold the pinned position per axis (`Some` while a node
/// is being dragged, or persistently after an explicit pin). When set,
/// the integrator skips force accumulation for that axis and holds the
/// value directly — the same `fx`/`fy` convention d3-force uses,
/// generalized to `fz` for 3D layouts. This is the single source of
/// truth for "is this node pinned" — no separate `Layout::pin`/`unpin`
/// bookkeeping is needed since any code that can see the particle slice
/// can set/clear these fields directly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub fx: Option<f32>,
    pub fy: Option<f32>,
    /// 3D depth axis — untouched by every 2D [`crate::layout::Layout`]
    /// impl, read/written only by
    /// [`crate::layout::force_directed_3d::ForceDirectedLayout3D`].
    pub z: f32,
    pub vz: f32,
    pub fz: Option<f32>,
}

impl Particle {
    /// Construct a particle at rest at `(x, y)` — `z`/`vz`/`fz` default
    /// to `0.0`/`0.0`/`None`.
    pub fn at(x: f32, y: f32) -> Self {
        Self { x, y, ..Default::default() }
    }

    /// Construct a particle at rest at `(x, y, z)` — the 3D counterpart
    /// of [`Particle::at`].
    pub fn at3(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z, ..Default::default() }
    }

    pub fn is_pinned(&self) -> bool {
        self.fx.is_some() || self.fy.is_some()
    }

    /// Whether ANY axis (`fx`/`fy`/`fz`) is pinned — the 3D counterpart
    /// of [`Particle::is_pinned`], used by
    /// [`crate::layout::force_directed_3d::ForceDirectedLayout3D`] to
    /// decide whether to hold a particle instead of integrating it.
    pub fn is_pinned_3d(&self) -> bool {
        self.fx.is_some() || self.fy.is_some() || self.fz.is_some()
    }

    /// Pin the particle at `(x, y)` — position is held, velocity zeroed.
    pub fn pin(&mut self, x: f32, y: f32) {
        self.fx = Some(x);
        self.fy = Some(y);
        self.x = x;
        self.y = y;
        self.vx = 0.0;
        self.vy = 0.0;
    }

    /// Pin the particle at `(x, y, z)` — the 3D counterpart of
    /// [`Particle::pin`], position held on all three axes, velocity
    /// zeroed on all three.
    pub fn pin3(&mut self, x: f32, y: f32, z: f32) {
        self.fx = Some(x);
        self.fy = Some(y);
        self.fz = Some(z);
        self.x = x;
        self.y = y;
        self.z = z;
        self.vx = 0.0;
        self.vy = 0.0;
        self.vz = 0.0;
    }

    /// Release the pin — the particle rejoins the simulation next tick.
    pub fn unpin(&mut self) {
        self.fx = None;
        self.fy = None;
    }

    /// Release the 3D pin (`fx`/`fy`/`fz`) — the 3D counterpart of
    /// [`Particle::unpin`].
    pub fn unpin3(&mut self) {
        self.fx = None;
        self.fy = None;
        self.fz = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at3_sets_z_and_leaves_the_rest_at_default() {
        let p = Particle::at3(1.0, 2.0, 3.0);
        assert_eq!((p.x, p.y, p.z), (1.0, 2.0, 3.0));
        assert_eq!((p.vx, p.vy, p.vz), (0.0, 0.0, 0.0));
        assert!(!p.is_pinned_3d());
    }

    #[test]
    fn pin3_holds_all_three_axes_and_zeroes_velocity() {
        let mut p = Particle::at3(0.0, 0.0, 0.0);
        p.vx = 5.0;
        p.vy = 5.0;
        p.vz = 5.0;
        p.pin3(1.0, 2.0, 3.0);
        assert_eq!((p.x, p.y, p.z), (1.0, 2.0, 3.0));
        assert_eq!((p.vx, p.vy, p.vz), (0.0, 0.0, 0.0));
        assert!(p.is_pinned_3d());
        p.unpin3();
        assert!(!p.is_pinned_3d());
    }

    #[test]
    fn is_pinned_3d_is_true_if_any_single_axis_is_set() {
        let mut p = Particle::at3(0.0, 0.0, 0.0);
        assert!(!p.is_pinned_3d());
        p.fz = Some(4.0);
        assert!(p.is_pinned_3d());
        assert!(!p.is_pinned(), "2D is_pinned must ignore fz");
    }
}
